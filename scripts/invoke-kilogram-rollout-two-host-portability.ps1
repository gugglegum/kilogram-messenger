[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $RolloutTestBinaryPath,
    [Parameter(Mandatory)] [string] $CliBinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $SourceRevision,
    [Parameter(Mandatory)] [string] $ReleaseAuthorityHex,
    [Parameter(Mandatory)] [string] $ReleaseTrustRootHex,
    [int] $ExpectedRolloutTestCount = 56,
    [string] $WorkingDirectory,
    [string] $RemoteHost = '192.168.0.152',
    [string] $RemoteUser = 'test',
    [string] $PlinkPath = 'C:\Program Files\Utils\plink.exe',
    [string] $PscpPath = 'C:\Program Files\Utils\pscp.exe',
    [string] $RunId = ('m1198-rollout-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$remotePassword = [Environment]::GetEnvironmentVariable('KILOGRAM_TEST_SSH_PASSWORD')
if ([string]::IsNullOrWhiteSpace($remotePassword)) {
    throw 'KILOGRAM_TEST_SSH_PASSWORD is required for the disposable field-test account'
}
if ($SourceRevision -cnotmatch '^[0-9a-f]{40}$') { throw 'SourceRevision must be an exact lowercase Git commit ID' }
if ($ReleaseAuthorityHex -cnotmatch '^[0-9a-f]{64}$') { throw 'ReleaseAuthorityHex must be an exact lowercase Ed25519 public key' }
if ($ReleaseTrustRootHex -cnotmatch '^[0-9a-f]{64}$') { throw 'ReleaseTrustRootHex must be an exact lowercase Ed25519 public key' }
if ($ExpectedRolloutTestCount -lt 1) { throw 'ExpectedRolloutTestCount must be positive' }
if ($RunId -cnotmatch '^[A-Za-z0-9_-]{1,96}$') { throw 'RunId contains unsupported characters' }

$rolloutTestBinary = [IO.Path]::GetFullPath($RolloutTestBinaryPath)
$cliBinary = [IO.Path]::GetFullPath($CliBinaryPath)
$output = [IO.Path]::GetFullPath($OutputDirectory)
$work = if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
    [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.tmp\rollout-field\$RunId"))
}
else {
    [IO.Path]::GetFullPath($WorkingDirectory)
}
foreach ($path in @($rolloutTestBinary, $cliBinary, $PlinkPath, $PscpPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required file is missing: $path" }
}
if (Test-Path -LiteralPath $output) { throw "portability evidence directory already exists: $output" }
if (Test-Path -LiteralPath $work) { throw "portability working directory already exists: $work" }
New-Item -ItemType Directory -Path $output | Out-Null
New-Item -ItemType Directory -Path $work | Out-Null

$testHash = (Get-FileHash -LiteralPath $rolloutTestBinary -Algorithm SHA256).Hash.ToLowerInvariant()
$cliHash = (Get-FileHash -LiteralPath $cliBinary -Algorithm SHA256).Hash.ToLowerInvariant()
$remoteStable = "C:/Users/$RemoteUser/KilogramLab/rollout-portability-stable"
$remoteTestBinary = "$remoteStable/kilogram-rollout-tests.exe"
$remoteCliBinary = "$remoteStable/kilogram-cli.exe"
$remoteRunRoot = "C:/Users/$RemoteUser/AppData/Local/Kilogram/rollout-portability/$RunId"
$remoteState = "$remoteRunRoot/state"
$remoteAbsentRollout = "$remoteRunRoot/absent-rollout"
$remoteAbsentTrustedPolicy = "$remoteRunRoot/absent-trusted-policy"
$localState = Join-Path $work 'local-state'
$localAbsentRollout = Join-Path $work 'local-absent-rollout'
$localAbsentTrustedPolicy = Join-Path $work 'local-absent-trusted-policy'

function ConvertTo-NativeArgument {
    param([AllowEmptyString()] [string] $Value)
    if ($Value.Length -gt 0 -and $Value -cnotmatch '[\s"]') { return $Value }
    $builder = [Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $slashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq [char]'\') { $slashes++; continue }
        if ($character -eq [char]'"') {
            if ($slashes) { [void]$builder.Append(('\' * ($slashes * 2))) }
            [void]$builder.Append('\"')
            $slashes = 0
            continue
        }
        if ($slashes) { [void]$builder.Append(('\' * $slashes)); $slashes = 0 }
        [void]$builder.Append($character)
    }
    if ($slashes) { [void]$builder.Append(('\' * ($slashes * 2))) }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Invoke-NativeCaptured {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300
    )
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $FilePath
    $start.Arguments = (($Arguments | ForEach-Object { ConvertTo-NativeArgument ([string]$_) }) -join ' ')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "failed to start $Description" }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        if (-not $process.HasExited) { $process.Kill() }
        throw "$Description timed out after $TimeoutSeconds seconds"
    }
    $process.WaitForExit()
    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result
    if ($process.ExitCode -ne 0) {
        throw "$Description failed ($($process.ExitCode)): stdout=[$stdout] stderr=[$stderr]"
    }
    return [PSCustomObject]@{ Stdout = $stdout; Stderr = $stderr }
}

function Quote-PowerShellLiteral {
    param([Parameter(Mandatory)] [string] $Value)
    return "'" + $Value.Replace("'", "''") + "'"
}

function Invoke-RemoteScript {
    param([Parameter(Mandatory)] [string] $Script, [string] $Description = 'remote command', [int] $TimeoutSeconds = 300)
    $body = "`$ErrorActionPreference = 'Stop'; " + $Script
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
    return Invoke-NativeCaptured $PlinkPath @(
        '-ssh', '-batch', '-pw', $remotePassword, "$RemoteUser@$RemoteHost",
        'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
        '-EncodedCommand', $encoded
    ) $Description $TimeoutSeconds
}

function Invoke-PscpUpload {
    param([Parameter(Mandatory)] [string] $LocalPath, [Parameter(Mandatory)] [string] $RemotePath)
    $null = Invoke-NativeCaptured $PscpPath @(
        '-batch', '-pw', $remotePassword, $LocalPath, "${RemoteUser}@${RemoteHost}:$RemotePath"
    ) "upload $(Split-Path -Leaf $LocalPath)" 300
}

function Assert-TestResult {
    param([Parameter(Mandatory)] [string] $OutputText, [Parameter(Mandatory)] [string] $Label)
    $expected = [regex]::Escape("test result: ok. $ExpectedRolloutTestCount passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;")
    if ($OutputText -cnotmatch "(?m)^${expected}") {
        throw "$Label did not report the exact $ExpectedRolloutTestCount/$ExpectedRolloutTestCount rollout test result"
    }
}

function Assert-EndToEndResult {
    param([Parameter(Mandatory)] [string] $OutputText, [Parameter(Mandatory)] [string] $Label)
    foreach ($line in @(
        'key_rotation=verified',
        'trusted_policy_chain=5/5',
        'device_approval_transitions=3/3',
        'containment=verified',
        'explicit_recovery=verified',
        'negative_foreign_root=verified',
        'negative_stale_truncated_fork=3/3',
        'replay_idempotence=verified',
        'default_public_activation=legacy-only',
        'status=canonical-sync-v2-rollout-end-to-end-rehearsal-verified'
    )) {
        if ($OutputText -cnotmatch "(?m)^$([regex]::Escape($line))`r?$") {
            throw "$Label is missing exact end-to-end output '$line'"
        }
    }
}

function Assert-ObserverResult {
    param([Parameter(Mandatory)] [string] $OutputText, [Parameter(Mandatory)] [string] $Label)
    foreach ($line in @(
        'rollout_store_present=false',
        'rollout_policy_present=false',
        'rollout_health_present=false',
        'rollout_decision=hold',
        'rollout_hold_reason=missing-signed-policy',
        'rollout_runtime_activation_authorized=false',
        'status=rollout-controller-observed-read-only'
    )) {
        if ($OutputText -cnotmatch "(?m)^$([regex]::Escape($line))`r?$") {
            throw "$Label is missing exact observer output '$line'"
        }
    }
}

function Assert-OperatorObserverResult {
    param([Parameter(Mandatory)] [string] $OutputText, [Parameter(Mandatory)] [string] $Label)
    foreach ($line in @(
        'rollout_operator_read_only=true',
        'rollout_operator_trusted_store_present=false',
        'rollout_operator_trusted_policy_present=false',
        'rollout_operator_decision=hold',
        'rollout_operator_hold_reason=missing-trusted-policy',
        'rollout_operator_writes_state=false',
        'status=rollout-operator-observed-read-only'
    )) {
        if ($OutputText -cnotmatch "(?m)^$([regex]::Escape($line))`r?$") {
            throw "$Label is missing exact operator-observer output '$line'"
        }
    }
}

Write-Output 'Running exact stable-name rollout test harness locally...'
$localTests = Invoke-NativeCaptured $rolloutTestBinary @('--test-threads=2') 'local rollout tests' 300
Assert-TestResult $localTests.Stdout 'local rollout harness'
$localEndToEnd = Invoke-NativeCaptured $rolloutTestBinary @(
    'end_to_end_rehearsal::canonical_sync_v2_rollout_end_to_end_rehearsal', '--exact', '--nocapture', '--test-threads=1'
) 'local rollout end-to-end rehearsal' 300
Assert-EndToEndResult $localEndToEnd.Stdout 'local rollout end-to-end rehearsal'

$null = Invoke-NativeCaptured $cliBinary @('identity', '--state-dir', $localState) 'initialize local disposable Device' 120
$localObserver = Invoke-NativeCaptured $cliBinary @(
    'rollout-controller-status', '--state-dir', $localState,
    '--rollout-dir', $localAbsentRollout, '--release-authority', $ReleaseAuthorityHex
) 'local rollout observer' 120
Assert-ObserverResult $localObserver.Stdout 'local rollout observer'
if (Test-Path -LiteralPath $localAbsentRollout) { throw 'local read-only observer created the absent rollout directory' }
$localOperatorObserver = Invoke-NativeCaptured $cliBinary @(
    'rollout-operator-status', '--state-dir', $localState,
    '--rollout-dir', $localAbsentRollout,
    '--trusted-policy-dir', $localAbsentTrustedPolicy,
    '--release-trust-root', $ReleaseTrustRootHex
) 'local rollout operator observer' 120
Assert-OperatorObserverResult $localOperatorObserver.Stdout 'local rollout operator observer'
if (Test-Path -LiteralPath $localAbsentRollout) { throw 'local operator observer created the absent rollout directory' }
if (Test-Path -LiteralPath $localAbsentTrustedPolicy) { throw 'local operator observer created the absent trusted-policy directory' }

Write-Output 'Uploading the exact same stable-name binaries to the Windows laptop...'
$remoteSetup = "New-Item -ItemType Directory -Force -Path $(Quote-PowerShellLiteral $remoteStable),$(Quote-PowerShellLiteral $remoteRunRoot) | Out-Null"
$null = Invoke-RemoteScript $remoteSetup 'prepare remote bounded directories' 120
Invoke-PscpUpload $rolloutTestBinary $remoteTestBinary
Invoke-PscpUpload $cliBinary $remoteCliBinary

$remoteHashesScript = @(
    "(Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteTestBinary) -Algorithm SHA256).Hash.ToLowerInvariant()",
    "(Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteCliBinary) -Algorithm SHA256).Hash.ToLowerInvariant()"
) -join '; '
$remoteHashes = (Invoke-RemoteScript $remoteHashesScript 'hash remote binaries' 120).Stdout -split "`r?`n" | Where-Object { $_ -cmatch '^[0-9a-f]{64}$' }
if ($remoteHashes.Count -ne 2 -or $remoteHashes[0] -cne $testHash -or $remoteHashes[1] -cne $cliHash) {
    throw 'remote stable binaries do not match the local SHA-256 values'
}

Write-Output 'Running exact stable-name rollout test harness on the Windows laptop...'
$remoteTestScript = "& $(Quote-PowerShellLiteral $remoteTestBinary) '--test-threads=2'; if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }"
$remoteTests = Invoke-RemoteScript $remoteTestScript 'remote rollout tests' 300
Assert-TestResult $remoteTests.Stdout 'remote rollout harness'
$remoteEndToEndScript = "& $(Quote-PowerShellLiteral $remoteTestBinary) 'end_to_end_rehearsal::canonical_sync_v2_rollout_end_to_end_rehearsal' '--exact' '--nocapture' '--test-threads=1'; if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }"
$remoteEndToEnd = Invoke-RemoteScript $remoteEndToEndScript 'remote rollout end-to-end rehearsal' 300
Assert-EndToEndResult $remoteEndToEnd.Stdout 'remote rollout end-to-end rehearsal'

$remoteIdentityScript = "& $(Quote-PowerShellLiteral $remoteCliBinary) 'identity' '--state-dir' $(Quote-PowerShellLiteral $remoteState); if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }"
$null = Invoke-RemoteScript $remoteIdentityScript 'initialize remote disposable Device' 120
$remoteObserverScript = @(
    "& $(Quote-PowerShellLiteral $remoteCliBinary)",
    "'rollout-controller-status' '--state-dir' $(Quote-PowerShellLiteral $remoteState)",
    "'--rollout-dir' $(Quote-PowerShellLiteral $remoteAbsentRollout)",
    "'--release-authority' $(Quote-PowerShellLiteral $ReleaseAuthorityHex)",
    "; if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }",
    "; if (Test-Path -LiteralPath $(Quote-PowerShellLiteral $remoteAbsentRollout)) { throw 'observer created absent rollout directory' }"
) -join ' '
$remoteObserver = Invoke-RemoteScript $remoteObserverScript 'remote rollout observer' 120
Assert-ObserverResult $remoteObserver.Stdout 'remote rollout observer'
$remoteOperatorObserverScript = @(
    "& $(Quote-PowerShellLiteral $remoteCliBinary)",
    "'rollout-operator-status' '--state-dir' $(Quote-PowerShellLiteral $remoteState)",
    "'--rollout-dir' $(Quote-PowerShellLiteral $remoteAbsentRollout)",
    "'--trusted-policy-dir' $(Quote-PowerShellLiteral $remoteAbsentTrustedPolicy)",
    "'--release-trust-root' $(Quote-PowerShellLiteral $ReleaseTrustRootHex)",
    "; if (`$LASTEXITCODE -ne 0) { exit `$LASTEXITCODE }",
    "; if (Test-Path -LiteralPath $(Quote-PowerShellLiteral $remoteAbsentRollout)) { throw 'operator observer created absent rollout directory' }",
    "; if (Test-Path -LiteralPath $(Quote-PowerShellLiteral $remoteAbsentTrustedPolicy)) { throw 'operator observer created absent trusted-policy directory' }"
) -join ' '
$remoteOperatorObserver = Invoke-RemoteScript $remoteOperatorObserverScript 'remote rollout operator observer' 120
Assert-OperatorObserverResult $remoteOperatorObserver.Stdout 'remote rollout operator observer'

$evidence = [ordered]@{
    schema_version = 1
    milestone = 'M1.2.5'
    status = 'canonical-sync-v2-rollout-end-to-end-two-host-verified'
    run_id = $RunId
    source_revision = $SourceRevision
    public_default = 'legacy-only'
    changes_peer_network_wire = $false
    changes_sync_wire = $false
    changes_runtime_ipc = $false
    changes_public_default = $false
    build = [ordered]@{
        rollout_test_harness_profile = 'verification'
        cli_profile = 'debug'
        target = 'x86_64-pc-windows-msvc'
        release_build = $false
        archive_created = $false
    }
    artifacts = [ordered]@{
        rollout_test_harness = [ordered]@{ stable_name = 'kilogram-rollout-tests.exe'; sha256 = $testHash; bytes = [int64](Get-Item -LiteralPath $rolloutTestBinary).Length }
        cli = [ordered]@{ stable_name = 'kilogram-cli.exe'; sha256 = $cliHash; bytes = [int64](Get-Item -LiteralPath $cliBinary).Length }
    }
    hosts = @(
        [ordered]@{ label = 'local-windows-host'; harness_tests = "$ExpectedRolloutTestCount/$ExpectedRolloutTestCount"; end_to_end_rehearsal = 'verified'; absent_store_observer = 'hold-without-create'; operator_observer = 'hold-without-create' },
        [ordered]@{ label = 'remote-windows-host'; harness_tests = "$ExpectedRolloutTestCount/$ExpectedRolloutTestCount"; end_to_end_rehearsal = 'verified'; absent_store_observer = 'hold-without-create'; operator_observer = 'hold-without-create' }
    )
    transport = [ordered]@{ ssh_control_channel_only = $true; p2p_network_executed = $false; listening_socket_created = $false; firewall_prompt_expected = $false }
    privacy = [ordered]@{ raw_logs_tracked = $false; state_tracked = $false; account_or_device_ids_tracked = $false; secrets_tracked = $false }
}
$evidencePath = Join-Path $output 'rollout-two-host-portability-evidence.json'
[IO.File]::WriteAllText($evidencePath, ($evidence | ConvertTo-Json -Depth 10) + "`n", [Text.UTF8Encoding]::new($false))

Write-Output "rollout_test_harness=$ExpectedRolloutTestCount/$ExpectedRolloutTestCount-local-and-remote"
Write-Output 'rollout_end_to_end_rehearsal=verified-local-and-remote'
Write-Output 'absent_store_observer=hold-without-create-local-and-remote'
Write-Output 'operator_observer=hold-without-create-local-and-remote'
Write-Output "rollout_test_sha256=$testHash"
Write-Output "cli_sha256=$cliHash"
Write-Output "evidence_path=$evidencePath"
Write-Output 'status=canonical-sync-v2-rollout-end-to-end-two-host-verified'
