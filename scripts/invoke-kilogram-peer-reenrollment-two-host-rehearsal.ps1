[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $SourceRevision,
    [string] $RemoteHost = '192.168.0.152',
    [string] $RemoteUser = 'test',
    [string] $PlinkPath = 'C:\Program Files\Utils\plink.exe',
    [string] $PscpPath = 'C:\Program Files\Utils\pscp.exe',
    [string] $RunId = ('m1213-reenrollment-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$remotePassword = [Environment]::GetEnvironmentVariable('KILOGRAM_TEST_SSH_PASSWORD')
if ([string]::IsNullOrWhiteSpace($remotePassword)) {
    throw 'KILOGRAM_TEST_SSH_PASSWORD is required for the disposable field-test account'
}
if ($SourceRevision -cnotmatch '^[0-9a-f]{40}$') { throw 'SourceRevision must be an exact lowercase Git commit ID' }
if ($RunId -cnotmatch '^[A-Za-z0-9_-]{1,96}$') { throw 'RunId contains unsupported characters' }

$binary = [IO.Path]::GetFullPath($BinaryPath)
$output = [IO.Path]::GetFullPath($OutputDirectory)
$capabilityHarness = Join-Path $PSScriptRoot 'invoke-kilogram-peer-capability-two-host-rehearsal.ps1'
foreach ($path in @($binary, $capabilityHarness, $PlinkPath, $PscpPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required file is missing: $path" }
}
if (Test-Path -LiteralPath $output) { throw "field evidence directory already exists: $output" }
New-Item -ItemType Directory -Path $output | Out-Null
$controllerLogs = Join-Path $output 'controller-logs'
New-Item -ItemType Directory -Path $controllerLogs | Out-Null
$prerequisiteOutput = Join-Path $output 'peer-capability-prerequisite'
$baseOutput = Join-Path $prerequisiteOutput 'base-rehearsal'
$remoteRoot = "C:/Users/$RemoteUser/KilogramLab/$RunId"
$remoteBinary = "C:/Users/$RemoteUser/KilogramLab/field-kit-current/kilogram-cli.exe"
$script:processCounter = 0
$script:activeLocalRuntime = $null
$script:activeRemoteRuntime = $null

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

function Start-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Prefix
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
    if (-not $process.Start()) { throw "failed to start $FilePath" }
    return [PSCustomObject]@{
        Process = $process
        StdoutTask = $process.StandardOutput.ReadToEndAsync()
        StderrTask = $process.StandardError.ReadToEndAsync()
        StdoutPath = "$Prefix.stdout.log"
        StderrPath = "$Prefix.stderr.log"
        Completed = $false
    }
}

function Complete-CapturedProcess {
    param([Parameter(Mandatory)] [object] $Started)
    if ($Started.Completed) { throw 'native process result was already collected' }
    $stdout = $Started.StdoutTask.Result
    $stderr = $Started.StderrTask.Result
    [IO.File]::WriteAllText($Started.StdoutPath, $stdout, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($Started.StderrPath, $stderr, [Text.UTF8Encoding]::new($false))
    $Started.Completed = $true
    return [PSCustomObject]@{ ExitCode = [int]$Started.Process.ExitCode; Stdout = $stdout; Stderr = $stderr }
}

function Stop-CapturedProcess {
    param([AllowNull()] [object] $Started)
    if ($null -eq $Started -or $Started.Completed) { return }
    if (-not $Started.Process.HasExited) {
        Stop-Process -Id $Started.Process.Id -Force -ErrorAction SilentlyContinue
        [void]$Started.Process.WaitForExit(5000)
    }
    if ($Started.Process.HasExited) { $null = Complete-CapturedProcess $Started }
}

function Wait-CapturedProcess {
    param(
        [Parameter(Mandatory)] [object] $Started,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    if (-not $Started.Process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-CapturedProcess $Started
        throw "$Description timed out after $TimeoutSeconds seconds"
    }
    $Started.Process.WaitForExit()
    $result = Complete-CapturedProcess $Started
    if ($result.ExitCode -ne 0 -and -not $AllowFailure) {
        throw "$Description failed ($($result.ExitCode)): stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
    }
    return $result
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    $script:processCounter++
    $safe = $Description -replace '[^A-Za-z0-9_-]', '-'
    $prefix = Join-Path $controllerLogs ('{0:D3}-{1}' -f $script:processCounter, $safe)
    $started = Start-CapturedProcess $FilePath $Arguments $prefix
    return Wait-CapturedProcess $started $Description $TimeoutSeconds -AllowFailure:$AllowFailure
}

function Quote-PowerShellLiteral {
    param([Parameter(Mandatory)] [string] $Value)
    return "'" + $Value.Replace("'", "''") + "'"
}

function New-EncodedRemoteCommand {
    param([Parameter(Mandatory)] [string] $Script)
    $body = "`$ErrorActionPreference = 'Stop'; " + $Script
    return [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
}

function New-RemoteProcessArguments {
    param([Parameter(Mandatory)] [string] $Script)
    return @(
        '-ssh', '-batch', '-pw', $remotePassword, "$RemoteUser@$RemoteHost",
        'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
        '-EncodedCommand', (New-EncodedRemoteCommand $Script)
    )
}

function Invoke-RemoteScript {
    param(
        [Parameter(Mandatory)] [string] $Script,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    return Invoke-CapturedProcess $PlinkPath (New-RemoteProcessArguments $Script) $Description $TimeoutSeconds -AllowFailure:$AllowFailure
}

function Start-RemoteScript {
    param([Parameter(Mandatory)] [string] $Script, [Parameter(Mandatory)] [string] $Prefix)
    return Start-CapturedProcess $PlinkPath (New-RemoteProcessArguments $Script) (Join-Path $controllerLogs $Prefix)
}

function New-RemoteCliScript {
    param([Parameter(Mandatory)] [string[]] $Arguments)
    $command = @('& ' + (Quote-PowerShellLiteral $remoteBinary))
    $command += $Arguments | ForEach-Object { Quote-PowerShellLiteral ([string]$_) }
    return (($command -join ' ') + '; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }')
}

function Invoke-RemoteCli {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    return Invoke-RemoteScript (New-RemoteCliScript $Arguments) $Description $TimeoutSeconds -AllowFailure:$AllowFailure
}

function Start-RemoteCli {
    param([Parameter(Mandatory)] [string[]] $Arguments, [Parameter(Mandatory)] [string] $Prefix)
    return Start-RemoteScript (New-RemoteCliScript $Arguments) $Prefix
}

function Invoke-LocalCli {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    return Invoke-CapturedProcess $binary $Arguments $Description $TimeoutSeconds -AllowFailure:$AllowFailure
}

function Invoke-PscpUpload {
    param([string] $LocalPath, [string] $RemotePath, [string] $Description)
    $null = Invoke-CapturedProcess $PscpPath @(
        '-batch', '-pw', $remotePassword, $LocalPath, "${RemoteUser}@${RemoteHost}:$RemotePath"
    ) $Description 300
}

function Invoke-PscpDownload {
    param([string] $RemotePath, [string] $LocalPath, [string] $Description)
    $null = Invoke-CapturedProcess $PscpPath @(
        '-batch', '-pw', $remotePassword, "${RemoteUser}@${RemoteHost}:$RemotePath", $LocalPath
    ) $Description 300
}

function Get-Field {
    param([string] $Text, [string] $Name)
    $matches = @([regex]::Matches($Text, "(?m)^$([regex]::Escape($Name))=([^`r`n]*)`r?$") | ForEach-Object { $_.Groups[1].Value })
    if ($matches.Count -eq 0) { throw "required field is missing: $Name" }
    return [string]$matches[-1]
}

function Assert-Field {
    param([string] $Text, [string] $Name, [string] $Expected)
    $actual = Get-Field $Text $Name
    if ($actual -cne $Expected) { throw "$Name expected [$Expected], got [$actual]" }
}

function Assert-NoPeerNetworkAttempt {
    param([string] $Text, [string] $Label)
    foreach ($pattern in @(
        '(?m)^runtime_endpoint_attempt=',
        '(?m)^runtime_endpoint_candidate_device_id=',
        '(?m)^runtime_endpoint_candidate_status=',
        '(?m)^transport_ready_path=',
        '(?m)^peer_id='
    )) {
        if ($Text -cmatch $pattern) { throw "$Label unexpectedly reached peer networking ($pattern)" }
    }
}

function Get-HistoryEventIds {
    param([string] $Text)
    return @([regex]::Matches($Text, '(?m)^event_id=([0-9a-f]{64}) ') | ForEach-Object { $_.Groups[1].Value } | Sort-Object)
}

function Apply-LocalActivation {
    param([string] $State, [string] $MigrationState, [string] $Description)
    $arguments = @('--state-dir', $State, '--migration-state', $MigrationState, '--disposition', 'normal', '--reason', "M1.2.13 field $Description")
    $plan = Invoke-LocalCli (@('canonical-sync-policy-plan') + $arguments) "$Description-plan"
    $record = Get-Field $plan.Stdout 'activation_plan_record_id'
    $apply = Invoke-LocalCli (@('canonical-sync-policy-apply') + $arguments + @('--expected-record-id', $record)) "$Description-apply"
    Assert-Field $apply.Stdout 'status' 'canonical-sync-v2-activation-applied'
}

function Wait-LocalFile {
    param([string] $Path, [object] $Runtime, [int] $TimeoutSeconds = 90)
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $Runtime.Process.Refresh()
        if ($Runtime.Process.HasExited) {
            $result = Complete-CapturedProcess $Runtime
            throw "runtime exited before ticket publication: stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
        }
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = [IO.File]::ReadAllText($Path).Trim()
            if ($text.Length -gt 64) { return }
        }
        Start-Sleep -Milliseconds 200
    }
    throw "runtime did not publish ticket within $TimeoutSeconds seconds"
}

function Wait-RemoteFile {
    param([string] $Path, [object] $Runtime, [int] $TimeoutSeconds = 90)
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $Runtime.Process.Refresh()
        if ($Runtime.Process.HasExited) {
            $result = Complete-CapturedProcess $Runtime
            throw "remote runtime exited before ticket publication: stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
        }
        $probe = Invoke-RemoteScript "if (Test-Path -LiteralPath $(Quote-PowerShellLiteral $Path) -PathType Leaf) { `$value = [IO.File]::ReadAllText($(Quote-PowerShellLiteral $Path)).Trim(); if (`$value.Length -gt 64) { Write-Output 'remote_ticket_ready=true' } }" 'remote-ticket-probe' 30
        if ($probe.Stdout -cmatch '(?m)^remote_ticket_ready=true\r?$') { return }
        Start-Sleep -Milliseconds 300
    }
    throw "remote runtime did not publish ticket within $TimeoutSeconds seconds"
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()

try {
    Write-Output 'Running the accepted peer-capability two-host prerequisite...'
    $prerequisiteLines = @(& $capabilityHarness -BinaryPath $binary -OutputDirectory $prerequisiteOutput -SourceRevision $SourceRevision -RemoteHost $RemoteHost -RemoteUser $RemoteUser -PlinkPath $PlinkPath -PscpPath $PscpPath -RunId $RunId 2>&1)
    $prerequisiteText = @($prerequisiteLines | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
    [IO.File]::WriteAllText((Join-Path $controllerLogs '000-peer-capability-prerequisite.log'), $prerequisiteText, [Text.UTF8Encoding]::new($false))
    if ('status=canonical-sync-v2-peer-capability-two-host-rehearsal-verified' -notin @($prerequisiteLines | ForEach-Object { [string]$_ })) {
        throw "peer-capability prerequisite did not report success: $prerequisiteText"
    }

    $prerequisiteEvidencePath = Join-Path $prerequisiteOutput 'M1207-PEER-CAPABILITY-TWO-HOST-EVIDENCE.json'
    $prerequisiteEvidence = Get-Content -LiteralPath $prerequisiteEvidencePath -Raw | ConvertFrom-Json
    if ([string]$prerequisiteEvidence.source_revision -cne $SourceRevision -or
        [string]$prerequisiteEvidence.binary.sha256 -cne $binaryHash -or
        -not [bool]$prerequisiteEvidence.topology.distinct_physical_hosts) {
        throw 'peer-capability prerequisite evidence does not match the exact binary and topology'
    }
    $remoteProbe = Invoke-RemoteScript "Write-Output ('remote_hostname=' + [Environment]::MachineName); Write-Output ('remote_binary_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteBinary) -Algorithm SHA256).Hash.ToLowerInvariant())" 'remote-exact-binary-probe'
    $remoteMachine = Get-Field $remoteProbe.Stdout 'remote_hostname'
    $remoteHash = Get-Field $remoteProbe.Stdout 'remote_binary_sha256'
    if ($remoteHash -cne $binaryHash) { throw 'remote binary SHA-256 differs from the local executable' }
    if ($remoteMachine -ceq [Environment]::MachineName) { throw 'two-host rehearsal requires distinct physical hosts' }

    $pairDirectory = Join-Path $baseOutput '02-optin-default'
    $bobDirectory = Join-Path $pairDirectory 'bob'
    $bobAccount = Join-Path $bobDirectory 'account'
    $oldBobState = Join-Path $bobDirectory 'state'
    $oldBobCertificate = Join-Path $bobDirectory 'device.cert'
    $oldBobDevices = Join-Path $bobDirectory 'devices.list'
    $oldBobPool = Join-Path $bobDirectory 'prekeys.pool'
    $membership = Join-Path $pairDirectory 'conversation.membership'
    $oldBobTicket = Join-Path $pairDirectory 'network.ticket'
    foreach ($path in @($bobAccount, $oldBobState, $oldBobCertificate, $oldBobDevices, $oldBobPool, $membership, $oldBobTicket)) {
        if (-not (Test-Path -LiteralPath $path)) { throw "prepared pair artifact is missing: $path" }
    }
    $remoteScenario = "$remoteRoot/02-optin-default"
    $aliceState = "$remoteScenario/alice/state"
    $aliceDevices = "$remoteScenario/alice/devices.list"
    $oldBobTicketRemote = "$remoteScenario/network.ticket"
    $conversation = "$RunId-02-optin-default"

    $bobAccountResult = Invoke-LocalCli @('account-show', '--account-dir', $bobAccount) 'bob-account-show'
    $bobAccountId = Get-Field $bobAccountResult.Stdout 'account_id'
    $oldBobIdentity = Invoke-LocalCli @('identity', '--state-dir', $oldBobState) 'old-bob-identity'
    $oldBobDeviceId = Get-Field $oldBobIdentity.Stdout 'device_id'
    $aliceAccountResult = Invoke-RemoteCli @('account-show', '--account-dir', "$remoteScenario/alice/account") 'alice-account-show'
    $aliceAccountId = Get-Field $aliceAccountResult.Stdout 'account_id'

    Write-Output 'Enrolling the quarantined predecessor as Alice runtime contact...'
    $oldContact = Invoke-RemoteCli @(
        'runtime-contact-add', '--state-dir', $aliceState, '--conversation', $conversation,
        '--expect-account', $bobAccountId, '--descriptor-file', $oldBobTicketRemote
    ) 'alice-add-old-contact'
    Assert-Field $oldContact.Stdout 'status' 'runtime-contact-ready'
    Assert-Field $oldContact.Stdout 'peer_device_id' $oldBobDeviceId
    $contactId = Get-Field $oldContact.Stdout 'runtime_contact_id'
    $remoteContactPath = "$aliceState/runtime/contacts/$contactId.contact"
    $contactHashBeforeResult = Invoke-RemoteScript "Write-Output ('contact_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteContactPath) -Algorithm SHA256).Hash.ToLowerInvariant())" 'predecessor-contact-hash-before'
    $contactHashBefore = Get-Field $contactHashBeforeResult.Stdout 'contact_sha256'

    $message = "M1.2.13 queued across peer re-enrollment $RunId"
    $queued = Invoke-RemoteCli @(
        'runtime-queue-message', '--state-dir', $aliceState, '--conversation', $conversation,
        '--peer-account', $bobAccountId, '--message', $message
    ) 'alice-queue-before-reenrollment'
    Assert-Field $queued.Stdout 'status' 'runtime-message-queued'
    Assert-Field $queued.Stdout 'runtime_queue_store' 'Inserted'
    $queueId = Get-Field $queued.Stdout 'runtime_queue_id'

    Write-Output 'Proving the quarantined predecessor blocks queued delivery before networking...'
    $preAcceptRuntime = Invoke-RemoteCli @(
        'runtime', '--state-dir', $aliceState, '--allow-account', $bobAccountId,
        '--device-list-file', $aliceDevices, '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '0', '--idle-seconds', '30', '--poll-milliseconds', '100',
        '--retry-base-seconds', '1', '--retry-max-seconds', '1', '--auto-sync-seconds', '0',
        '--max-outbound-actions', '1'
    ) 'alice-preaccept-capability-block' 90
    Assert-Field $preAcceptRuntime.Stdout 'runtime_queue_id' $queueId
    Assert-Field $preAcceptRuntime.Stdout 'runtime_peer_capability_status' 'blocked'
    Assert-Field $preAcceptRuntime.Stdout 'runtime_outbound_network_attempted' 'false'
    Assert-Field $preAcceptRuntime.Stdout 'runtime_outbound_status' 'capability-blocked'
    Assert-Field $preAcceptRuntime.Stdout 'runtime_stop_reason' 'outbound-action-limit'
    Assert-NoPeerNetworkAttempt ($preAcceptRuntime.Stdout + $preAcceptRuntime.Stderr) 'pre-accept runtime'

    Write-Output 'Creating a Root-authorized replacement Bob Device...'
    $replacementDirectory = Join-Path $output 'replacement-bob'
    New-Item -ItemType Directory -Path $replacementDirectory | Out-Null
    $replacementState = Join-Path $replacementDirectory 'state'
    $replacementCertificate = Join-Path $replacementDirectory 'device.cert'
    $replacementDevices = Join-Path $replacementDirectory 'devices.list'
    $replacementSnapshot = Join-Path $replacementDirectory 'authority.snapshot'
    $replacementPool = Join-Path $replacementDirectory 'prekeys.pool'
    $replacementHistory = Join-Path $replacementDirectory 'predecessor-history.rewrap'
    $replacementTicket = Join-Path $replacementDirectory 'runtime.ticket'
    $replacementInvitation = Join-Path $replacementDirectory 'current-invitation.json'
    $enroll = Invoke-LocalCli @(
        'device-enroll', '--account-dir', $bobAccount, '--state-dir', $replacementState,
        '--certificate-file', $replacementCertificate
    ) 'replacement-bob-enroll'
    Assert-Field $enroll.Stdout 'status' 'device-enrolled'
    $replacementDeviceId = Get-Field $enroll.Stdout 'device_id'
    $deviceList = Invoke-LocalCli @(
        'account-device-list', '--account-dir', $bobAccount,
        '--device-certificate-file', $oldBobCertificate,
        '--device-certificate-file', $replacementCertificate,
        '--device-list-file', $replacementDevices
    ) 'replacement-bob-device-list'
    Assert-Field $deviceList.Stdout 'device_count' '2'
    $null = Invoke-LocalCli @('account-snapshot', '--account-dir', $bobAccount, '--snapshot-file', $replacementSnapshot) 'replacement-bob-account-snapshot'
    $null = Invoke-LocalCli @('device-authority-update', '--state-dir', $oldBobState, '--snapshot-file', $replacementSnapshot) 'old-bob-authority-update'
    $null = Invoke-LocalCli @('device-authority-update', '--state-dir', $replacementState, '--snapshot-file', $replacementSnapshot) 'replacement-bob-authority-update'
    $null = Invoke-LocalCli @('conversation-membership-install', '--state-dir', $replacementState, '--membership-file', $membership) 'replacement-bob-membership'
    $null = Invoke-LocalCli @('ratchet-prekey-pool', '--state-dir', $replacementState, '--pool-file', $replacementPool, '--count', '16') 'replacement-bob-prekeys'
    $historyExport = Invoke-LocalCli @(
        'history-rewrap-export', '--state-dir', $oldBobState, '--conversation', $conversation,
        '--device-list-file', $replacementDevices, '--recipient-device', $replacementDeviceId,
        '--range-start', '0', '--count', '64', '--bundle-file', $replacementHistory
    ) 'old-bob-history-rewrap-export'
    Assert-Field $historyExport.Stdout 'rewrapped_event_count' '6'
    Assert-Field $historyExport.Stdout 'source_inventory_complete' 'true'
    Assert-Field $historyExport.Stdout 'status' 'history-rewrap-exported'
    $historyImport = Invoke-LocalCli @(
        'history-rewrap-import', '--state-dir', $replacementState, '--conversation', $conversation,
        '--bundle-file', $replacementHistory
    ) 'replacement-bob-history-rewrap-import'
    Assert-Field $historyImport.Stdout 'rewrapped_event_count' '6'
    Assert-Field $historyImport.Stdout 'inserted_event_count' '6'
    Assert-Field $historyImport.Stdout 'inserted_projection_count' '6'
    Assert-Field $historyImport.Stdout 'source_inventory_complete' 'true'
    Assert-Field $historyImport.Stdout 'status' 'history-rewrap-imported'
    Apply-LocalActivation $replacementState 'advertise-cbor-v2' 'replacement-bob-advertise'
    Apply-LocalActivation $replacementState 'dual-reader-prefer-legacy' 'replacement-bob-listener'

    $replacementRuntime = Start-CapturedProcess $binary @(
        'runtime', '--state-dir', $replacementState, '--allow-account', $aliceAccountId,
        '--device-list-file', $replacementDevices, '--peer-prekey-pool-file', $oldBobPool,
        '--ticket-file', $replacementTicket, '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '1', '--idle-seconds', '180', '--poll-milliseconds', '5000', '--auto-sync-seconds', '0'
    ) (Join-Path $controllerLogs 'replacement-bob-runtime')
    $script:activeLocalRuntime = $replacementRuntime
    Wait-LocalFile $replacementTicket $replacementRuntime
    $invitation = Invoke-LocalCli @(
        'current-peer-invitation-create', '--state-dir', $replacementState,
        '--ticket-file', $replacementTicket, '--output-file', $replacementInvitation
    ) 'replacement-bob-current-invitation'
    Assert-Field $invitation.Stdout 'status' 'current-peer-invitation-created'

    $replacementDevicesRemote = "$remoteScenario/replacement-bob.devices.list"
    $replacementTicketRemote = "$remoteScenario/replacement-bob.ticket"
    $replacementInvitationRemote = "$remoteScenario/replacement-bob.invitation.json"
    Invoke-PscpUpload $replacementDevices $replacementDevicesRemote 'upload-replacement-device-list'
    Invoke-PscpUpload $replacementTicket $replacementTicketRemote 'upload-replacement-ticket'
    Invoke-PscpUpload $replacementInvitation $replacementInvitationRemote 'upload-replacement-invitation'
    $candidate = Invoke-RemoteCli @(
        'runtime-contact-add', '--state-dir', $aliceState, '--conversation', $conversation,
        '--expect-account', $bobAccountId, '--descriptor-file', $replacementTicketRemote
    ) 'alice-add-replacement-candidate'
    Assert-Field $candidate.Stdout 'status' 'runtime-contact-ready'
    Assert-Field $candidate.Stdout 'peer_device_id' $replacementDeviceId
    Assert-Field $candidate.Stdout 'endpoint_candidate_added' 'true'

    Write-Output 'Running the explicit offline peer re-enrollment ceremony...'
    $planRemote = "$remoteScenario/peer-reenrollment.plan"
    $tombstoneRemote = "$remoteScenario/peer-reenrollment.tombstone"
    $plan = Invoke-RemoteCli @(
        'peer-reenrollment-plan', '--state-dir', $aliceState,
        '--local-device-list-file', $aliceDevices, '--peer-device-list-file', $replacementDevicesRemote,
        '--invitation-file', $replacementInvitationRemote,
        '--quarantined-peer-device-id', $oldBobDeviceId, '--output-file', $planRemote
    ) 'alice-peer-reenrollment-plan'
    Assert-Field $plan.Stdout 'state_changed' 'false'
    Assert-Field $plan.Stdout 'network_executed' 'false'
    Assert-Field $plan.Stdout 'status' 'peer-reenrollment-planned'
    $confirmation = Get-Field $plan.Stdout 'peer_reenrollment_confirmation_phrase'
    $approve = Invoke-RemoteCli @(
        'peer-reenrollment-approve', '--state-dir', $aliceState, '--plan-file', $planRemote,
        '--confirm-phrase', $confirmation, '--approve', '--output-file', $tombstoneRemote
    ) 'alice-peer-reenrollment-approve'
    Assert-Field $approve.Stdout 'explicit_approval' 'true'
    Assert-Field $approve.Stdout 'network_executed' 'false'
    Assert-Field $approve.Stdout 'status' 'peer-reenrollment-approved'
    $accept = Invoke-RemoteCli @(
        'peer-reenrollment-accept', '--state-dir', $aliceState, '--tombstone-file', $tombstoneRemote,
        '--local-device-list-file', $aliceDevices, '--peer-device-list-file', $replacementDevicesRemote,
        '--invitation-file', $replacementInvitationRemote
    ) 'alice-peer-reenrollment-accept'
    Assert-Field $accept.Stdout 'peer_reenrollment_acceptance_outcome' 'accepted'
    Assert-Field $accept.Stdout 'peer_reenrollment_runtime_transition_prepared' '1'
    Assert-Field $accept.Stdout 'runtime_restart_required' 'true'
    Assert-Field $accept.Stdout 'runtime_hot_apply' 'false'
    Assert-Field $accept.Stdout 'network_executed' 'false'
    Assert-Field $accept.Stdout 'status' 'peer-reenrollment-successor-prepared'
    $preparedStatus = Invoke-RemoteCli @('peer-reenrollment-status', '--state-dir', $aliceState, '--peer-account-id', $bobAccountId) 'alice-prepared-status'
    Assert-Field $preparedStatus.Stdout 'peer_reenrollment_runtime_transition_present' 'true'
    Assert-Field $preparedStatus.Stdout 'peer_reenrollment_runtime_transition_consumed' 'false'

    Write-Output 'Substituting the accepted descriptor to prove fail-closed pre-network startup...'
    $replacementTicketBackupRemote = "$remoteScenario/replacement-bob.ticket.exact-backup"
    $null = Invoke-RemoteScript "Copy-Item -LiteralPath $(Quote-PowerShellLiteral $replacementTicketRemote) -Destination $(Quote-PowerShellLiteral $replacementTicketBackupRemote); Copy-Item -LiteralPath $(Quote-PowerShellLiteral $oldBobTicketRemote) -Destination $(Quote-PowerShellLiteral $replacementTicketRemote) -Force" 'substitute-replacement-descriptor'
    $substitution = Invoke-RemoteCli @(
        'runtime', '--state-dir', $aliceState, '--allow-account', $bobAccountId,
        '--device-list-file', $aliceDevices, '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '0', '--idle-seconds', '5', '--poll-milliseconds', '100', '--auto-sync-seconds', '0'
    ) 'alice-substituted-descriptor-start' 60 -AllowFailure
    if ($substitution.ExitCode -eq 0) { throw 'substituted descriptor unexpectedly reached runtime startup' }
    if (($substitution.Stdout + $substitution.Stderr) -cnotmatch 'accepted peer re-enrollment endpoint descriptor changed') {
        throw "substituted descriptor failed for the wrong reason: $($substitution.Stderr)"
    }
    Assert-NoPeerNetworkAttempt ($substitution.Stdout + $substitution.Stderr) 'substituted descriptor startup'
    $stillPrepared = Invoke-RemoteCli @('peer-reenrollment-status', '--state-dir', $aliceState, '--peer-account-id', $bobAccountId) 'alice-status-after-substitution'
    Assert-Field $stillPrepared.Stdout 'peer_reenrollment_runtime_transition_consumed' 'false'
    $null = Invoke-RemoteScript "Copy-Item -LiteralPath $(Quote-PowerShellLiteral $replacementTicketBackupRemote) -Destination $(Quote-PowerShellLiteral $replacementTicketRemote) -Force; Write-Output ('restored_descriptor_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $replacementTicketRemote) -Algorithm SHA256).Hash.ToLowerInvariant())" 'restore-replacement-descriptor'

    Write-Output 'Starting Alice fresh and delivering the sealed queue item to replacement Bob...'
    $aliceDelivery = Invoke-RemoteCli @(
        'runtime', '--state-dir', $aliceState, '--allow-account', $bobAccountId,
        '--device-list-file', $aliceDevices, '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '0', '--idle-seconds', '120', '--poll-milliseconds', '100',
        '--retry-base-seconds', '1', '--retry-max-seconds', '1', '--auto-sync-seconds', '0',
        '--max-outbound-actions', '1'
    ) 'alice-fresh-start-delivery' 180
    Assert-Field $aliceDelivery.Stdout 'peer_reenrollment_runtime_transitions_prepared' '1'
    Assert-Field $aliceDelivery.Stdout 'peer_reenrollment_runtime_transitions_consumed' '1'
    Assert-Field $aliceDelivery.Stdout 'peer_reenrollment_runtime_transitions_already_consumed' '0'
    Assert-Field $aliceDelivery.Stdout 'peer_reenrollment_runtime_consumption_phase' 'before-network-endpoint'
    Assert-Field $aliceDelivery.Stdout 'runtime_queue_id' $queueId
    Assert-Field $aliceDelivery.Stdout 'runtime_endpoint_candidate_device_id' $replacementDeviceId
    Assert-Field $aliceDelivery.Stdout 'transport_ready_path' 'direct'
    Assert-Field $aliceDelivery.Stdout 'runtime_outbound_status' 'delivered'
    Assert-Field $aliceDelivery.Stdout 'runtime_stop_reason' 'outbound-action-limit'
    $replacementRuntimeResult = Wait-CapturedProcess $replacementRuntime 'replacement Bob delivery runtime' 180
    $replacementRuntime = $null
    $script:activeLocalRuntime = $null
    Assert-Field $replacementRuntimeResult.Stdout 'received' $message
    Assert-Field $replacementRuntimeResult.Stdout 'transport_ready_path' 'direct'
    Assert-Field $replacementRuntimeResult.Stdout 'runtime_session_status' 'completed'
    Assert-Field $replacementRuntimeResult.Stdout 'runtime_stop_reason' 'session-limit'

    $consumedStatus = Invoke-RemoteCli @('peer-reenrollment-status', '--state-dir', $aliceState, '--peer-account-id', $bobAccountId) 'alice-consumed-status'
    Assert-Field $consumedStatus.Stdout 'peer_reenrollment_runtime_transition_consumed' 'true'
    $outbox = Invoke-RemoteCli @('runtime-outbox-status', '--state-dir', $aliceState) 'alice-outbox-after-delivery'
    Assert-Field $outbox.Stdout 'runtime_queue_count' '1'
    Assert-Field $outbox.Stdout 'runtime_pending_count' '0'
    Assert-Field $outbox.Stdout 'runtime_delivered_count' '1'

    Write-Output 'Restarting Alice to prove exact replay and no duplicate delivery...'
    $replay = Invoke-RemoteCli @(
        'runtime', '--state-dir', $aliceState, '--allow-account', $bobAccountId,
        '--device-list-file', $aliceDevices, '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '0', '--idle-seconds', '3', '--poll-milliseconds', '100',
        '--retry-base-seconds', '1', '--retry-max-seconds', '1', '--auto-sync-seconds', '0'
    ) 'alice-idempotent-replay' 60
    Assert-Field $replay.Stdout 'peer_reenrollment_runtime_transitions_prepared' '1'
    Assert-Field $replay.Stdout 'peer_reenrollment_runtime_transitions_consumed' '0'
    Assert-Field $replay.Stdout 'peer_reenrollment_runtime_transitions_already_consumed' '1'
    Assert-Field $replay.Stdout 'runtime_outbound_actions' '0'
    Assert-Field $replay.Stdout 'runtime_stop_reason' 'idle-timeout'
    if (($replay.Stdout + $replay.Stderr) -cmatch '(?m)^runtime_outbound_status=delivered\r?$') {
        throw 'idempotent replay duplicated the delivered queue item'
    }

    Write-Output 'Synchronizing replacement Bob with Alice and comparing complete history...'
    $aliceSyncTicketRemote = "$remoteScenario/alice-post-reenrollment-sync.ticket"
    $aliceSyncRuntime = Start-RemoteCli @(
        'runtime', '--state-dir', $aliceState, '--allow-account', $bobAccountId,
        '--device-list-file', $aliceDevices, '--ticket-file', $aliceSyncTicketRemote,
        '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '1', '--idle-seconds', '120', '--poll-milliseconds', '5000', '--auto-sync-seconds', '0'
    ) 'alice-post-reenrollment-sync-runtime'
    $script:activeRemoteRuntime = $aliceSyncRuntime
    Wait-RemoteFile $aliceSyncTicketRemote $aliceSyncRuntime
    $aliceSyncTicket = Join-Path $replacementDirectory 'alice-post-reenrollment-sync.ticket'
    Invoke-PscpDownload $aliceSyncTicketRemote $aliceSyncTicket 'download-alice-post-reenrollment-ticket'
    $replacementSync = Invoke-LocalCli @(
        'sync', '--state-dir', $replacementState, '--ticket-file', $aliceSyncTicket,
        '--conversation', $conversation, '--expect-account', $aliceAccountId
    ) 'replacement-bob-sync-from-alice' 300
    Assert-Field $replacementSync.Stdout 'status' 'synchronized'
    Assert-Field $replacementSync.Stdout 'transport_path' 'direct'
    $aliceSyncRuntimeResult = Wait-CapturedProcess $aliceSyncRuntime 'Alice post-reenrollment sync runtime' 300
    $aliceSyncRuntime = $null
    $script:activeRemoteRuntime = $null
    Assert-Field $aliceSyncRuntimeResult.Stdout 'runtime_session_status' 'completed'
    Assert-Field $aliceSyncRuntimeResult.Stdout 'runtime_stop_reason' 'session-limit'

    $bobHistory = Invoke-LocalCli @('history', '--state-dir', $replacementState, '--conversation', $conversation) 'replacement-bob-final-history'
    $aliceHistory = Invoke-RemoteCli @('history', '--state-dir', $aliceState, '--conversation', $conversation) 'alice-final-history'
    $bobEventIds = @(Get-HistoryEventIds $bobHistory.Stdout)
    $aliceEventIds = @(Get-HistoryEventIds $aliceHistory.Stdout)
    if ($bobEventIds.Count -lt 8 -or ($bobEventIds -join ',') -cne ($aliceEventIds -join ',')) {
        throw 'post-re-enrollment histories differ across physical hosts'
    }
    $messageOccurrences = @([regex]::Matches($bobHistory.Stdout, [regex]::Escape($message))).Count
    if ($messageOccurrences -ne 1) { throw "replacement Bob history contains queued message $messageOccurrences times" }

    $contactHashAfterResult = Invoke-RemoteScript "Write-Output ('contact_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteContactPath) -Algorithm SHA256).Hash.ToLowerInvariant())" 'predecessor-contact-hash-after'
    $contactHashAfter = Get-Field $contactHashAfterResult.Stdout 'contact_sha256'
    if ($contactHashAfter -cne $contactHashBefore) { throw 'retained predecessor contact file changed after consumption' }
    $quarantineAfter = Invoke-RemoteCli @(
        'peer-capability-status', '--state-dir', $aliceState,
        '--peer-account-id', $bobAccountId, '--peer-device-id', $oldBobDeviceId
    ) 'old-bob-quarantine-after'
    Assert-Field $quarantineAfter.Stdout 'peer_capability_floor' 'quarantined'
    Assert-Field $quarantineAfter.Stdout 'peer_capability_quarantine_reason' 'pinned-canonical-capability-missing'

    $manifest = [ordered]@{
        schema_version = 1
        milestone = 'M1.2.13'
        status = 'canonical-sync-v2-peer-reenrollment-two-host-rehearsal-verified'
        run_id = $RunId
        source_revision = $SourceRevision
        collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        public_default = 'legacy-only'
        changes_connection_ticket_wire = $false
        changes_sync_wire = $false
        changes_runtime_ipc = $false
        changes_public_default = $false
        binary = [ordered]@{
            name = 'kilogram-cli.exe'
            stable_local_path = '.tmp/kilogram-field-runner/kilogram-cli.exe'
            stable_remote_path = 'KilogramLab/field-kit-current/kilogram-cli.exe'
            sha256 = $binaryHash
            bytes = [int64]$binaryItem.Length
            profile = 'debug'
            target = 'x86_64-pc-windows-msvc'
        }
        topology = [ordered]@{
            local_host = 'local-windows-host'
            remote_host = 'remote-windows-host'
            distinct_physical_hosts = $true
            binary_hashes_equal = $true
            transport = 'iroh-direct-lan'
            ssh_is_control_channel_only = $true
        }
        prerequisite = [ordered]@{
            status = 'canonical-sync-v2-peer-capability-two-host-rehearsal-verified'
            old_peer_floor = 'quarantined'
            old_peer_quarantine_reason = 'pinned-canonical-capability-missing'
        }
        reenrollment = [ordered]@{
            predecessor_contact_enrolled = $true
            replacement_device_root_authorized = $true
            predecessor_history_rewrapped_offline = $true
            predecessor_history_rewrapped_event_count = 6
            replacement_endpoint_candidate_enrolled = $true
            explicit_plan_approval_acceptance = $true
            ceremony_network_executed = $false
            transition_prepared_before_restart = $true
            transition_consumed_before_restart = $false
            descriptor_substitution_rejected = $true
            descriptor_substitution_peer_network_attempted = $false
            fresh_start_consumed = $true
            consumption_phase = 'before-network-endpoint'
            queued_delivery_status = 'delivered'
            queued_delivery_path = 'direct'
            queued_delivery_count = 1
            exact_replay_already_consumed = $true
            replay_outbound_actions = 0
            complete_history_event_count = $bobEventIds.Count
            histories_equal = $true
            queued_message_occurrences = 1
            predecessor_contact_byte_exact = $true
            old_quarantine_retained = $true
        }
        privacy = [ordered]@{
            raw_logs_tracked = $false
            state_tracked = $false
            ticket_bodies_tracked = $false
            account_or_device_ids_tracked = $false
            endpoint_addresses_tracked = $false
            event_or_queue_ids_tracked = $false
            secrets_tracked = $false
        }
    }
    $manifestPath = Join-Path $output 'M1213-PEER-REENROLLMENT-TWO-HOST-EVIDENCE.json'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output 'predecessor_quarantine=retained'
    Write-Output 'predecessor_history_rewrap=6/6-offline'
    Write-Output 'descriptor_substitution=rejected-before-network'
    Write-Output 'fresh_start_successor=consumed-before-network-endpoint'
    Write-Output 'queued_delivery=exactly-once-direct'
    Write-Output 'restart_replay=idempotent-no-outbound-action'
    Write-Output "history_events=$($bobEventIds.Count)"
    Write-Output 'histories_equal=true'
    Write-Output "binary_sha256=$binaryHash"
    Write-Output "evidence_manifest=$manifestPath"
    Write-Output 'status=canonical-sync-v2-peer-reenrollment-two-host-rehearsal-verified'
}
finally {
    Stop-CapturedProcess $script:activeLocalRuntime
    Stop-CapturedProcess $script:activeRemoteRuntime
    try {
        $null = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; exit 0" 'remote-final-cleanup' 30
    }
    catch {
        Write-Warning "remote cleanup could not be confirmed: $($_.Exception.Message)"
    }
}
