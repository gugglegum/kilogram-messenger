$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
$env:PSModulePath="$PSHOME\Modules"
if ($PSVersionTable.PSEdition -ne 'Desktop' -or $PSVersionTable.PSVersion.Major -ne 5) {
    throw 'Run this check with Windows PowerShell 5.1'
}
$repo=Split-Path -Parent $PSScriptRoot
$fixture=Join-Path $repo ".tmp\terminal-t1-3e\launch-check-$([Guid]::NewGuid().ToString('N'))"
$null=[IO.Directory]::CreateDirectory($fixture)
$parser=[Management.Automation.Language.Parser]
$tokens=$null
$errors=$null
$fw=$parser::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-firewall.ps1'),[ref]$tokens,[ref]$errors)
if ($errors) { throw ($errors | Out-String) }
$capture=$parser::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-admin-capture.ps1'),[ref]$tokens,[ref]$errors)
if ($errors) { throw ($errors | Out-String) }
foreach ($name in 'Find-Rule','Restore-Firewall','Invoke-T13eCapturedChild') {
    $ast=if ($name -eq 'Invoke-T13eCapturedChild') {$capture} else {$fw}
    $fn=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name},$true)
    . ([scriptblock]::Create($fn.Extent.Text))
}

$script:failure='missing'
$script:missingProperty='InstanceID'
function Get-NetFirewallRule {
    [CmdletBinding()]
    param($PolicyStore,$Name)
    if ($script:failure -eq 'denied') {
        $record=[Management.Automation.ErrorRecord]::new([UnauthorizedAccessException]::new('denied'),
            'Windows System Error 5','PermissionDenied',$Name)
        $PSCmdlet.ThrowTerminatingError($record)
    }
    $target=if ($script:failure -eq 'foreign') {'some-other-rule'} else {$Name}
    $record=[Management.Automation.ErrorRecord]::new([Exception]::new('NetSecurity InstanceID was absent'),
        "CmdletizationQuery_NotFound_$($script:missingProperty)",'ObjectNotFound',$target)
    $PSCmdlet.ThrowTerminatingError($record)
}
foreach ($missingProperty in 'InstanceID','Name') {
    $script:missingProperty=$missingProperty
    foreach ($store in 'PersistentStore','ActiveStore') {
        if (@(Find-Rule 'Kilogram-T13e-IrohIn-B' $store).Count) { throw 'An absent rule returned a match' }
    }
}
foreach ($scenario in 'denied','foreign') {
    $script:failure=$scenario
    $blocked=$false
    try { Find-Rule 'Kilogram-T13e-IrohIn-B' ActiveStore } catch { $blocked=$true }
    if (!$blocked) { throw "Find-Rule hid failure: $scenario" }
}
$script:failure='missing'
$script:rollback=$null
function Assert-Rule { throw 'An absent rule must not reach ownership checks' }
function Remove-NetFirewallRule { throw 'An absent rule must not be removed' }
function Tick { 1 }
function Write-State { param($Path,$Value) $script:rollback=$Value }
$run=$fixture
$specs=@(@{Name='Kilogram-T13e-DiscoveryIn-B'},@{Name='Kilogram-T13e-IrohIn-B'})
$journal=@{profile_started=$false;tag='synthetic'}
Restore-Firewall $journal
if (!$script:rollback.ok -or $script:rollback.errors.Count) { throw 'Absent rules made rollback fail' }

# Run the production Apply handlers around a controlled primary failure.
$handler=$fw.Find({param($n) $n -is [Management.Automation.Language.TryStatementAst] -and
    $null -ne $n.Finally -and $n.Finally.Extent.Text -match 'Read-State \$receipt' -and
    $n.Body.Extent.Text -match 'foreach \(\$s in \$specs\)'},$true)
$code="try { throw 'synthetic primary error' } " +
    ($handler.CatchClauses.Extent.Text -join "`n") + ' finally ' + $handler.Finally.Extent.Text
function Restore-Firewall { throw 'synthetic rollback error' }
$original=[Console]::Error
$writer=[IO.StringWriter]::new()
$caught=$null
$applyError=$null
[Console]::SetError($writer)
try { . ([scriptblock]::Create($code)) } catch { $caught=$_.Exception.Message }
finally { [Console]::SetError($original) }
if ($caught -cne 'synthetic primary error' -or $writer.ToString() -notmatch 'rollback also failed: synthetic rollback error') {
    throw 'Rollback masked the primary error'
}
$writer.Dispose()

function PatternBytes([int]$Count, [int]$Offset) {
    $result=New-Object byte[] $Count
    for ($i=0; $i -lt $Count; $i++) { $result[$i]=[byte](($i+$Offset)%256) }
    return ,$result
}
function BytesSha([byte[]]$Bytes){
    $m=[IO.MemoryStream]::new($Bytes)
    try { (Get-FileHash -InputStream $m).Hash } finally { $m.Dispose() }
}
$source=@'
$ProgressPreference='SilentlyContinue'
$n=131072
$out=[byte[]]::new($n)
$err=[byte[]]::new($n)
for ($i=0;$i -lt $n;$i++) { $out[$i]=[byte](($i+27)%256); $err[$i]=[byte](($i+43)%256) }
$s=[Console]::OpenStandardOutput()
$s.Write($out,0,$out.Length)
$s.Flush()
$s=[Console]::OpenStandardError()
$s.Write($err,0,$err.Length)
$s.Flush()
exit 1
'@
$psi=[Diagnostics.ProcessStartInfo]::new()
$psi.FileName="$env:windir\System32\WindowsPowerShell\v1.0\powershell.exe"
$psi.Arguments='-NoProfile -NonInteractive -EncodedCommand ' + [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
$psi.UseShellExecute=$false
$psi.CreateNoWindow=$true
$psi.RedirectStandardOutput=$true
$psi.RedirectStandardError=$true
$outPath=Join-Path $fixture 'child.stdout'
$errPath=Join-Path $fixture 'child.stderr'
$stdout=[IO.File]::Open($outPath,'CreateNew','Write','Read')
$stderr=[IO.File]::Open($errPath,'CreateNew','Write','Read')
$facts=@{streams_complete=$false}
try { Invoke-T13eCapturedChild $psi $stdout $stderr $facts }
finally {$stdout.Dispose();$stderr.Dispose()}
if (!$facts.streams_complete -or $facts.child_exit -ne 1 -or
    (Get-FileHash -LiteralPath $outPath).Hash -cne (BytesSha (PatternBytes 131072 27)) -or
    (Get-FileHash -LiteralPath $errPath).Hash -cne (BytesSha (PatternBytes 131072 43))) {
    throw 'Native child exit or captured raw bytes differ'
}
foreach ($scenario in 'successful-child','failed-start') {
    $out=[IO.File]::Open((Join-Path $fixture "$scenario.stdout"),'CreateNew','Write','Read')
    $err=[IO.File]::Open((Join-Path $fixture "$scenario.stderr"),'CreateNew','Write','Read')
    $actual=@{streams_complete=$false;child_exit=$null;child_pid=$null}
    $pinned=$psi.FileName
    $didThrow=$false
    try {
        if ($scenario -eq 'failed-start') { $psi.FileName=Join-Path $fixture 'missing.exe' }
        else { $psi.Arguments='-NoProfile -NonInteractive -Command "exit 0"' }
        Invoke-T13eCapturedChild $psi $out $err $actual
    } catch { $didThrow=$true }
    finally { $psi.FileName=$pinned; $out.Dispose(); $err.Dispose() }
    if ($scenario -eq 'successful-child') {
        if ($didThrow -or !$actual.streams_complete -or $actual.child_exit -ne 0) { throw 'Exit 0 capture failed' }
    } elseif (!$didThrow -or $actual.streams_complete -or $actual.child_pid -or $null -ne $actual.child_exit) {
        throw 'Failed child start was recorded as complete'
    }
}
@{fixture=$fixture; missing_query_properties=@('InstanceID','Name'); denied_and_wrong_name_preserved=$true; absent_rollback=$true
 primary_error_preserved=$true; secondary_error_in_stderr=$true; streams_bound_bytes=131072;
 child_exit=$facts.child_exit; child_exit_0=$true; failed_start_incomplete=$true
 stderr_sha256=(Get-FileHash -LiteralPath $errPath).Hash.ToLowerInvariant()} |
 ConvertTo-Json -Compress
