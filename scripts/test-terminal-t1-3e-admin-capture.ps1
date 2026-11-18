[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Apply','Rollback')][string]$Mode,
    [Parameter(Mandatory)][ValidateSet('A','B')][string]$Side,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9][a-z0-9-]{0,30}$')][string]$Attempt,
    [Parameter(Mandatory)][ValidatePattern('^S-1-5-21-(\d+-){3}\d+$')][string]$TestSid,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$FirewallSha,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$AuditFileSha,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$PolicySha
)

$ErrorActionPreference='Stop'
$env:PSModulePath="$PSHOME\Modules"
$admin=[Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$system=[Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$reader=[Security.Principal.SecurityIdentifier]::new($TestSid)
$hostName=if ($Side -eq 'A') { 'DESKTOP-PAUL-RY' } else { 'PAUL_HP_OMEN' }
if ([Environment]::MachineName -ine $hostName -or
    !([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole($admin)) {
    throw 'Expected the pinned host and elevated token'
}

function Assert-CapturePath([string]$Path, [bool]$IsDirectory, [bool]$ShareReport) {
    $entry=Get-Item -LiteralPath $Path -Force
    if ([bool]$entry.PSIsContainer -ne $IsDirectory) { throw "Unexpected path type: $Path" }
    while ($entry) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point: $Path" }
        $entry=if ($entry -is [IO.FileInfo]) { $entry.Directory } else { $entry.Parent }
    }
    $acl=Get-Acl -LiteralPath $Path
    if (!$acl.AreAccessRulesProtected -or $acl.GetOwner([Security.Principal.SecurityIdentifier]) -notin @($admin,$system)) {
        throw "Unexpected owner or inheritance: $Path"
    }
    $allowed=[Security.AccessControl.FileSystemRights]'ReadAndExecute, Synchronize'
    $canRead=$false
    foreach ($ace in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        if ($ace.AccessControlType -ne 'Allow' -or $ace.IdentityReference -in @($admin,$system)) { continue }
        if (!$ShareReport -or $ace.IdentityReference -ne $reader -or ($ace.FileSystemRights -band (-bnot $allowed))) {
            throw "Unexpected non-admin access: $Path"
        }
        if (($ace.FileSystemRights -band [Security.AccessControl.FileSystemRights]::ReadData) -and
            ($ace.InheritanceFlags -band [Security.AccessControl.InheritanceFlags]::ObjectInherit)) { $canRead=$true }
    }
    if ($ShareReport -and !$canRead) { throw 'Pinned ordinary SID cannot read the captured files' }
}

function Save-CaptureFacts([string]$Path, $Facts) {
    $bytes=[Text.Encoding]::UTF8.GetBytes(($Facts | ConvertTo-Json -Depth 6) + "`n")
    $file=[IO.File]::Open($Path,'CreateNew','Write','None')
    try { $file.Write($bytes,0,$bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
}

function Invoke-T13eCapturedChild($StartInfo, [IO.Stream]$Output, [IO.Stream]$ErrorOutput, $Facts) {
    $p=[Diagnostics.Process]::new()
    $p.StartInfo=$StartInfo
    try {
        if (!$p.Start()) { throw 'The pinned child was not started' }
        $Facts.child_pid=$p.Id
        $copies=[Threading.Tasks.Task[]]@(
            $p.StandardOutput.BaseStream.CopyToAsync($Output)
            $p.StandardError.BaseStream.CopyToAsync($ErrorOutput)
        )
        while (!$p.WaitForExit(250)) {
            if (@($copies | Where-Object { $_.IsFaulted -or $_.IsCanceled }).Count) {
                # The captured process handle identifies this exact child. Its watchdog owns policy rollback.
                $p.Kill()
                [void]$p.WaitForExit(5000)
                throw 'Output capture failed while the child was active'
            }
        }
        $Facts.child_exit=$p.ExitCode
        if (![Threading.Tasks.Task]::WaitAll($copies,30000)) { throw 'Child output did not reach EOF' }
        $Output.Flush($true)
        $ErrorOutput.Flush($true)
        $Facts.streams_complete=$true
    } finally { $p.Dispose() }
}

$reports='C:\Program Files\Kilogram-T13e-Audit'
Assert-CapturePath $reports $true $true
$prefix=Join-Path $reports "$Side-$Attempt-$($Mode.ToLowerInvariant())-$([Guid]::NewGuid().ToString('N'))"
$facts=[ordered]@{side=$Side; attempt=$Attempt; mode=$Mode; launcher_pid=$PID; started_utc=[DateTime]::UtcNow.ToString('o')
    child_pid=$null; child_exit=$null; streams_complete=$false; bootstrap_error=$null}
Save-CaptureFacts "$prefix-start.json" $facts
$stdout=$null
$stderr=$null
try {
    $stdout=[IO.File]::Open("$prefix.stdout",'CreateNew','Write','Read')
    $stderr=[IO.File]::Open("$prefix.stderr",'CreateNew','Write','Read')
    $root='C:\Program Files\Kilogram-T13e'
    $fw=Join-Path $root 'test-terminal-t1-3e-firewall.ps1'
    Assert-CapturePath $root $true $false
    Assert-CapturePath $fw $false $false
    $auditPath=Join-Path $reports "$Side-$Attempt-audit.json"
    $auditBytes=[IO.File]::ReadAllBytes($auditPath)
    $stream=[IO.MemoryStream]::new($auditBytes)
    try { $auditSha=(Get-FileHash -InputStream $stream).Hash } finally { $stream.Dispose() }
    if ((Get-FileHash -LiteralPath $fw).Hash -ine $FirewallSha -or $auditSha -ine $AuditFileSha) {
        throw 'Reviewed firewall source or Audit bytes differ'
    }
    $audit=[Text.Encoding]::UTF8.GetString($auditBytes) | ConvertFrom-Json
    if ($audit.sha256 -cne $PolicySha -or $audit.policy.host -ine $hostName -or
        $audit.policy.side -cne $Side -or $audit.policy.test_sid -cne $TestSid) { throw 'Reviewed Audit identity differs' }
    $psi=[Diagnostics.ProcessStartInfo]::new()
    $psi.FileName='C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe'
    $psi.Arguments="-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$fw`" -Mode $Mode -Side $Side -Attempt $Attempt"
    if ($Mode -eq 'Apply') { $psi.Arguments += " -TestSid $TestSid -AuditSha256 $PolicySha" }
    $psi.WorkingDirectory=$root
    $psi.UseShellExecute=$false
    $psi.CreateNoWindow=$true
    $psi.RedirectStandardOutput=$true
    $psi.RedirectStandardError=$true
    Invoke-T13eCapturedChild $psi $stdout $stderr $facts
} catch {
    $facts.bootstrap_error=$_.Exception.Message
} finally {
    if ($stdout) { $stdout.Dispose() }
    if ($stderr) { $stderr.Dispose() }
    $facts.ended_utc=[DateTime]::UtcNow.ToString('o')
    foreach ($extension in 'stdout','stderr') {
        $path="$prefix.$extension"
        $facts[$extension]=if (Test-Path -LiteralPath $path) {
            @{file=[IO.Path]::GetFileName($path); sha256=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
        } else { $null }
    }
    Save-CaptureFacts "$prefix-result.json" $facts
}
if (!$facts.streams_complete -or $facts.bootstrap_error) { exit 1 }
exit $facts.child_exit
