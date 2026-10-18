[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $SourceRevision,
    [string] $ExperimentId = ('m1180-hosted-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')),
    [switch] $AttemptRelay
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($SourceRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw 'SourceRevision must be an exact lowercase 40-character commit ID'
}
if ($ExperimentId -cnotmatch '^[A-Za-z0-9_-]{1,72}$') {
    throw 'ExperimentId contains unsupported characters'
}

$sourceBinary = [IO.Path]::GetFullPath($BinaryPath)
if (-not (Test-Path -LiteralPath $sourceBinary -PathType Leaf)) {
    throw "evidence binary is missing: $sourceBinary"
}
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$binDirectory = Join-Path $output 'bin'
New-Item -ItemType Directory -Force -Path $binDirectory | Out-Null
$binary = Join-Path $binDirectory 'kilogram-cli.exe'
Copy-Item -LiteralPath $sourceBinary -Destination $binary -Force

function Start-EvidenceProcess {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPrefix
    )
    $logDirectory = Split-Path -Parent $LogPrefix
    if (-not [string]::IsNullOrWhiteSpace($logDirectory)) {
        New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
    }
    $stdout = "$LogPrefix.stdout.log"
    $stderr = "$LogPrefix.stderr.log"
    $process = Start-Process -FilePath $binary -ArgumentList $Arguments -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    [PSCustomObject]@{ Process = $process; Stdout = $stdout; Stderr = $stderr }
}

function Read-BoundedLog {
    param([Parameter(Mandatory)] [string] $Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return '' }
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.Length -gt 1MB) { return "<log exceeds 1 MiB: $Path>" }
    return (Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue)
}

function Stop-EvidenceProcess {
    param([AllowNull()] [object] $Started)
    if ($null -ne $Started -and -not $Started.Process.HasExited) {
        Stop-Process -Id $Started.Process.Id -Force -ErrorAction SilentlyContinue
        $Started.Process.WaitForExit(5000) | Out-Null
    }
}

function Wait-EvidenceProcess {
    param(
        [Parameter(Mandatory)] [object] $Started,
        [int] $TimeoutSeconds = 180,
        [Parameter(Mandatory)] [string] $Description
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $Started.Process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
        $Started.Process.Refresh()
    }
    if (-not $Started.Process.HasExited) {
        Stop-EvidenceProcess $Started
        throw "$Description timed out after $TimeoutSeconds seconds"
    }
    if ($Started.Process.ExitCode -ne 0) {
        $stdout = Read-BoundedLog $Started.Stdout
        $stderr = Read-BoundedLog $Started.Stderr
        throw "$Description failed with exit code $($Started.Process.ExitCode). stdout=[$stdout] stderr=[$stderr]"
    }
}

function Wait-Ticket {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [object] $Listener,
        [int] $TimeoutSeconds = 90
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($Listener.Process.HasExited) {
            Wait-EvidenceProcess $Listener 1 'listener before ticket publication'
        }
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            try {
                $ticket = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
                if ([string]$ticket.run_id -match '^[A-Za-z0-9_-]{1,96}$') { return }
            }
            catch { }
        }
        Start-Sleep -Milliseconds 250
        $Listener.Process.Refresh()
    }
    Stop-EvidenceProcess $Listener
    throw "listener did not publish a complete ticket within $TimeoutSeconds seconds"
}

function Invoke-EvidenceCommand {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPrefix,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 180
    )
    $started = Start-EvidenceProcess $Arguments $LogPrefix
    Wait-EvidenceProcess $started $TimeoutSeconds $Description
}

function Initialize-Scenario {
    param([Parameter(Mandatory)] [string] $Name)
    $directory = Join-Path $output $Name
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $runId = "$ExperimentId-$Name"
    Invoke-EvidenceCommand @(
        'prepare', '--run-dir', $directory, '--source-revision', $SourceRevision,
        '--run-id', $runId
    ) (Join-Path $directory 'prepare') "prepare $Name"
    return $directory
}

function Invoke-NetworkPair {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [Parameter(Mandatory)] [string] $Prefix,
        [Parameter(Mandatory)] [string] $ListenerCommand,
        [Parameter(Mandatory)] [string] $DialerCommand,
        [Parameter(Mandatory)] [string] $Route,
        [switch] $HostedLoopback,
        [switch] $StopAfterDiff
    )
    $ticket = Join-Path $Directory "$Prefix.ticket.json"
    $aliceReport = Join-Path $Directory "$Prefix-alice.json"
    $bobReport = Join-Path $Directory "$Prefix-bob.json"
    $listenerArguments = @(
        $ListenerCommand, '--role-dir', (Join-Path $Directory 'bob'),
        '--ticket-file', $ticket, '--route-policy', $Route,
        '--report-file', $bobReport
    )
    $dialerArguments = @(
        $DialerCommand, '--role-dir', (Join-Path $Directory 'alice'),
        '--ticket-file', $ticket, '--route-policy', $Route,
        '--report-file', $aliceReport
    )
    if ($HostedLoopback) {
        $listenerArguments += '--hosted-loopback'
        $dialerArguments += '--hosted-loopback'
    }
    if ($StopAfterDiff) {
        $listenerArguments += '--stop-after-diff'
        $dialerArguments += '--stop-after-diff'
    }
    $listener = $null
    try {
        $listener = Start-EvidenceProcess $listenerArguments (Join-Path $Directory "$Prefix-bob")
        Wait-Ticket $ticket $listener
        Invoke-EvidenceCommand $dialerArguments (Join-Path $Directory "$Prefix-alice") "$Prefix dialer"
        Wait-EvidenceProcess $listener 180 "$Prefix listener"
    }
    finally {
        Stop-EvidenceProcess $listener
    }
    [PSCustomObject]@{
        alice = Get-Content -LiteralPath $aliceReport -Raw | ConvertFrom-Json
        bob = Get-Content -LiteralPath $bobReport -Raw | ConvertFrom-Json
    }
}

$directDirectory = Initialize-Scenario 'direct'
$direct = Invoke-NetworkPair $directDirectory 'direct' 'listen' 'dial' 'direct-only' -HostedLoopback

$restartDirectory = Initialize-Scenario 'restart'
$checkpoint = Invoke-NetworkPair $restartDirectory 'checkpoint' 'listen' 'dial' 'direct-only' `
    -HostedLoopback -StopAfterDiff
$resumed = Invoke-NetworkPair $restartDirectory 'resumed' 'listen' 'dial' 'direct-only' -HostedLoopback

$legacyDirectory = Initialize-Scenario 'legacy'
$legacy = Invoke-NetworkPair $legacyDirectory 'legacy' 'legacy-listen' 'legacy-dial' 'direct-only' `
    -HostedLoopback

$corruptDirectory = Initialize-Scenario 'corrupt'
$corrupt = Invoke-NetworkPair $corruptDirectory 'corrupt' 'corrupt-listen' 'corrupt-dial' `
    'direct-only' -HostedLoopback

$rollbackDirectory = Initialize-Scenario 'rollback'
$aliceRollback = Join-Path $rollbackDirectory 'rollback-alice.json'
$bobRollback = Join-Path $rollbackDirectory 'rollback-bob.json'
Invoke-EvidenceCommand @(
    'rollback-drill', '--role-dir', (Join-Path $rollbackDirectory 'alice'),
    '--report-file', $aliceRollback
) (Join-Path $rollbackDirectory 'rollback-alice') 'Alice rollback drill'
Invoke-EvidenceCommand @(
    'rollback-drill', '--role-dir', (Join-Path $rollbackDirectory 'bob'),
    '--report-file', $bobRollback
) (Join-Path $rollbackDirectory 'rollback-bob') 'Bob rollback drill'
$rollback = [PSCustomObject]@{
    alice = Get-Content -LiteralPath $aliceRollback -Raw | ConvertFrom-Json
    bob = Get-Content -LiteralPath $bobRollback -Raw | ConvertFrom-Json
}

$relay = [ordered]@{
    status = 'unavailable'
    reason = 'relay experiment was not requested'
    alice = $null
    bob = $null
}
if ($AttemptRelay) {
    try {
        $relayDirectory = Initialize-Scenario 'relay'
        $relayPair = Invoke-NetworkPair $relayDirectory 'relay' 'listen' 'dial' 'relay-only'
        $relay = [ordered]@{ status = 'verified'; reason = $null; alice = $relayPair.alice; bob = $relayPair.bob }
    }
    catch {
        $reason = [string]$_.Exception.Message
        if ($reason.Length -gt 220) { $reason = $reason.Substring(0, 220) }
        $relay = [ordered]@{ status = 'unavailable'; reason = $reason; alice = $null; bob = $null }
    }
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$hostName = [Environment]::MachineName
$manifest = [ordered]@{
    schema_version = 1
    milestone = 'M1.1.80'
    evidence_kind = 'hosted-windows'
    source_revision = $SourceRevision
    collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    binary = [ordered]@{
        name = 'kilogram-cli.exe'
        sha256 = $binaryHash
        bytes = [int64]$binaryItem.Length
        profile = 'debug'
        target = 'x86_64-pc-windows-msvc'
        feature = 'sync-v2-evidence'
    }
    topology = [ordered]@{
        alice_host = $hostName
        bob_host = $hostName
        distinct_physical_hosts = $false
        alice_binary_sha256 = $binaryHash
        bob_binary_sha256 = $binaryHash
    }
    production_boundary = [ordered]@{
        active_sync_alpn = 'kilogram/m0/sync/8'
        candidate_sync_alpn = 'kilogram/m1/sync/cbor/2'
        candidate_registered_by_default = $false
        candidate_dialed_by_default = $false
        evidence_only = $true
        legacy_fallback_after_selection = $false
        changes_active_wire = $false
    }
    scenarios = [ordered]@{
        direct = $direct
        restart_resume = [ordered]@{ checkpoint = $checkpoint; resumed = $resumed }
        relay = $relay
        mixed_version = $legacy
        post_selection_failure = $corrupt
        rollback = $rollback
    }
}
$manifestPath = Join-Path $output 'manifest.json'
$manifest | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $manifestPath -Encoding utf8

& (Join-Path $PSScriptRoot 'verify-kilogram-sync-v2-network-evidence.ps1') -ManifestPath $manifestPath
if ($LASTEXITCODE -ne 0) { throw 'hosted evidence verifier failed' }
Write-Output "manifest=$manifestPath"
Write-Output 'status=hosted-sync-v2-evidence-verified'
