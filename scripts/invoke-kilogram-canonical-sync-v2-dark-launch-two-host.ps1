[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $SourceRevision,
    [string] $RemoteHost = '192.168.0.152',
    [string] $RemoteUser = 'test',
    [string] $PlinkPath = 'C:\Program Files\Utils\plink.exe',
    [string] $PscpPath = 'C:\Program Files\Utils\pscp.exe',
    [string] $RunId = ('m1184-field-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')),
    [switch] $BasicPairPreparationOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$remotePassword = [Environment]::GetEnvironmentVariable('KILOGRAM_TEST_SSH_PASSWORD')
if ([string]::IsNullOrWhiteSpace($remotePassword)) {
    throw 'KILOGRAM_TEST_SSH_PASSWORD is required for the disposable field-test account'
}
if ($SourceRevision -cnotmatch '^[0-9a-f]{40}$') { throw 'SourceRevision must be an exact lowercase Git commit ID' }
if ($RunId -cnotmatch '^[A-Za-z0-9_-]{1,80}$') { throw 'RunId contains unsupported characters' }

$binary = [IO.Path]::GetFullPath($BinaryPath)
$output = [IO.Path]::GetFullPath($OutputDirectory)
foreach ($path in @($binary, $PlinkPath, $PscpPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required file is missing: $path" }
}
if (Test-Path -LiteralPath $output) { throw "field evidence directory already exists: $output" }
New-Item -ItemType Directory -Path $output | Out-Null
$controllerLogs = Join-Path $output 'controller-logs'
New-Item -ItemType Directory -Path $controllerLogs | Out-Null
$remoteRoot = "C:/Users/$RemoteUser/KilogramLab/$RunId"
$remoteBinary = 'C:/Users/test/KilogramLab/field-kit-current/kilogram-cli.exe'
$script:processCounter = 0
$script:activeRuntime = $null

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
        [Parameter(Mandatory)] [string] $Prefix,
        [hashtable] $Environment = @{}
    )
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $FilePath
    $start.Arguments = (($Arguments | ForEach-Object { ConvertTo-NativeArgument ([string]$_) }) -join ' ')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($entry in $Environment.GetEnumerator()) {
        $start.EnvironmentVariables[[string]$entry.Key] = [string]$entry.Value
    }
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
        [switch] $AllowFailure,
        [hashtable] $Environment = @{}
    )
    $script:processCounter++
    $safe = $Description -replace '[^A-Za-z0-9_-]', '-'
    $prefix = Join-Path $controllerLogs ('{0:D3}-{1}' -f $script:processCounter, $safe)
    $started = Start-CapturedProcess $FilePath $Arguments $prefix $Environment
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

function Invoke-RemoteScript {
    param(
        [Parameter(Mandatory)] [string] $Script,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    $encoded = New-EncodedRemoteCommand $Script
    return Invoke-CapturedProcess $PlinkPath @(
        '-ssh', '-batch', '-pw', $remotePassword, "$RemoteUser@$RemoteHost",
        'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
        '-EncodedCommand', $encoded
    ) $Description $TimeoutSeconds -AllowFailure:$AllowFailure
}

function New-RemoteCliScript {
    param([Parameter(Mandatory)] [string[]] $Arguments, [hashtable] $Environment = @{})
    $parts = @()
    foreach ($entry in $Environment.GetEnumerator()) {
        $parts += "`$env:$([string]$entry.Key) = $(Quote-PowerShellLiteral ([string]$entry.Value));"
    }
    $command = @('& ' + (Quote-PowerShellLiteral $remoteBinary))
    $command += $Arguments | ForEach-Object { Quote-PowerShellLiteral ([string]$_) }
    $parts += (($command -join ' ') + '; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }')
    return ($parts -join ' ')
}

function Invoke-RemoteCli {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure,
        [hashtable] $Environment = @{}
    )
    return Invoke-RemoteScript (New-RemoteCliScript $Arguments $Environment) $Description $TimeoutSeconds -AllowFailure:$AllowFailure
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

function Invoke-LocalCli {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    return Invoke-CapturedProcess $binary $Arguments $Description $TimeoutSeconds -AllowFailure:$AllowFailure
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

function Get-HistoryEventIds {
    param([string] $Text)
    return @([regex]::Matches($Text, '(?m)^event_id=([0-9a-f]{64}) ') | ForEach-Object { $_.Groups[1].Value } | Sort-Object)
}

function Apply-LocalActivation {
    param([string] $State, [string] $MigrationState, [string] $Description, [string] $Disposition = 'normal', [string] $Evidence = '')
    $arguments = @('--state-dir', $State, '--migration-state', $MigrationState, '--disposition', $Disposition, '--reason', "M1.1.84 field $Description")
    if ($Evidence) { $arguments += @('--canonical-evidence-id', $Evidence) }
    $plan = Invoke-LocalCli (@('canonical-sync-policy-plan') + $arguments) "$Description-plan"
    $record = Get-Field $plan.Stdout 'activation_plan_record_id'
    $apply = Invoke-LocalCli (@('canonical-sync-policy-apply') + $arguments + @('--expected-record-id', $record)) "$Description-apply"
    Assert-Field $apply.Stdout 'status' 'canonical-sync-v2-activation-applied'
}

function Apply-RemoteActivation {
    param([string] $State, [string] $MigrationState, [string] $Description, [string] $Disposition = 'normal', [string] $Evidence = '')
    $arguments = @('--state-dir', $State, '--migration-state', $MigrationState, '--disposition', $Disposition, '--reason', "M1.1.84 field $Description")
    if ($Evidence) { $arguments += @('--canonical-evidence-id', $Evidence) }
    $plan = Invoke-RemoteCli (@('canonical-sync-policy-plan') + $arguments) "$Description-plan"
    $record = Get-Field $plan.Stdout 'activation_plan_record_id'
    $apply = Invoke-RemoteCli (@('canonical-sync-policy-apply') + $arguments + @('--expected-record-id', $record)) "$Description-apply"
    Assert-Field $apply.Stdout 'status' 'canonical-sync-v2-activation-applied'
}

function Enable-LocalListener {
    param([object] $Pair, [string] $Prefix)
    Apply-LocalActivation $Pair.BobState 'advertise-cbor-v2' "$Prefix-advertise"
    Apply-LocalActivation $Pair.BobState 'dual-reader-prefer-legacy' "$Prefix-listener"
}

function Enable-RemoteInitiator {
    param([object] $Pair, [string] $Prefix)
    Apply-RemoteActivation $Pair.AliceState 'advertise-cbor-v2' "$Prefix-advertise"
    Apply-RemoteActivation $Pair.AliceState 'dual-reader-prefer-legacy' "$Prefix-reader"
    Apply-RemoteActivation $Pair.AliceState 'dual-reader-prefer-canonical' "$Prefix-initiator"
}

function Initialize-Pair {
    param([string] $Name)
    $directory = Join-Path $output $Name
    $bob = Join-Path $directory 'bob'
    $artifacts = Join-Path $directory 'alice-public'
    New-Item -ItemType Directory -Path $bob, $artifacts | Out-Null
    $remoteScenario = "$remoteRoot/$Name"
    $aliceAccount = "$remoteScenario/alice/account"
    $aliceState = "$remoteScenario/alice/state"
    $aliceCertificateRemote = "$remoteScenario/alice/device.cert"
    $aliceDevicesRemote = "$remoteScenario/alice/devices.list"
    $alicePoolRemote = "$remoteScenario/alice/prekeys.pool"
    $null = Invoke-RemoteScript "New-Item -ItemType Directory -Force -Path $(Quote-PowerShellLiteral "$remoteScenario/alice") | Out-Null" "$Name-remote-directory"
    $aliceAccountResult = Invoke-RemoteCli @('account-create', '--account-dir', $aliceAccount) "$Name-alice-account"
    $aliceAccountId = Get-Field $aliceAccountResult.Stdout 'account_id'
    $null = Invoke-RemoteCli @('device-enroll', '--account-dir', $aliceAccount, '--state-dir', $aliceState, '--certificate-file', $aliceCertificateRemote) "$Name-alice-enroll"
    $null = Invoke-RemoteCli @('account-device-list', '--account-dir', $aliceAccount, '--device-certificate-file', $aliceCertificateRemote, '--device-list-file', $aliceDevicesRemote) "$Name-alice-devices"
    $null = Invoke-RemoteCli @('ratchet-prekey-pool', '--state-dir', $aliceState, '--pool-file', $alicePoolRemote, '--count', '16') "$Name-alice-prekeys"
    $aliceCertificate = Join-Path $artifacts 'device.cert'
    $aliceDevices = Join-Path $artifacts 'devices.list'
    $alicePool = Join-Path $artifacts 'prekeys.pool'
    Invoke-PscpDownload $aliceCertificateRemote $aliceCertificate "$Name-download-certificate"
    Invoke-PscpDownload $aliceDevicesRemote $aliceDevices "$Name-download-devices"
    Invoke-PscpDownload $alicePoolRemote $alicePool "$Name-download-prekeys"

    $bobAccount = Join-Path $bob 'account'
    $bobState = Join-Path $bob 'state'
    $bobCertificate = Join-Path $bob 'device.cert'
    $bobDevices = Join-Path $bob 'devices.list'
    $bobPool = Join-Path $bob 'prekeys.pool'
    $bobAccountResult = Invoke-LocalCli @('account-create', '--account-dir', $bobAccount) "$Name-bob-account"
    $bobAccountId = Get-Field $bobAccountResult.Stdout 'account_id'
    $null = Invoke-LocalCli @('device-enroll', '--account-dir', $bobAccount, '--state-dir', $bobState, '--certificate-file', $bobCertificate) "$Name-bob-enroll"
    $null = Invoke-LocalCli @('account-device-list', '--account-dir', $bobAccount, '--device-certificate-file', $bobCertificate, '--device-list-file', $bobDevices) "$Name-bob-devices"
    $null = Invoke-LocalCli @('ratchet-prekey-pool', '--state-dir', $bobState, '--pool-file', $bobPool, '--count', '16') "$Name-bob-prekeys"

    $conversation = "$RunId-$Name"
    $membership = Join-Path $directory 'conversation.membership'
    $membershipRemote = "$remoteScenario/conversation.membership"
    $null = Invoke-LocalCli @('conversation-create', '--account-dir', $bobAccount, '--conversation', $conversation, '--member-account', $bobAccountId, '--member-account', $aliceAccountId, '--membership-file', $membership) "$Name-conversation"
    $null = Invoke-LocalCli @('conversation-membership-install', '--state-dir', $bobState, '--membership-file', $membership) "$Name-bob-membership"
    Invoke-PscpUpload $membership $membershipRemote "$Name-upload-membership"
    $null = Invoke-RemoteCli @('conversation-membership-install', '--state-dir', $aliceState, '--membership-file', $membershipRemote) "$Name-alice-membership"
    $null = Invoke-LocalCli @('seed-history', '--state-dir', $bobState, '--conversation', $conversation, '--count', '6', '--message-prefix', "M1.1.84-field-$RunId", '--peer-certificate-file', $aliceCertificate, '--peer-device-list-file', $aliceDevices, '--peer-prekey-pool-file', $alicePool) "$Name-seed" 300

    return [PSCustomObject]@{
        Name = $Name; Directory = $directory; RemoteScenario = $remoteScenario
        BobAccount = $bobAccount; BobState = $bobState; BobAccountId = $bobAccountId
        BobDevices = $bobDevices; AliceState = $aliceState; AliceAccountId = $aliceAccountId
        AliceCertificate = $aliceCertificate; AliceDevices = $aliceDevices
        Conversation = $conversation
    }
}

function Start-LocalRuntime {
    param([object] $Pair, [string] $Ticket, [string] $Prefix, [int] $MaxSessions, [hashtable] $Environment = @{})
    $started = Start-CapturedProcess $binary @(
        'runtime', '--state-dir', $Pair.BobState, '--allow-account', $Pair.AliceAccountId,
        '--device-list-file', $Pair.BobDevices, '--ticket-file', $Ticket,
        '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', ([string]$MaxSessions), '--idle-seconds', '120',
        '--poll-milliseconds', '5000', '--auto-sync-seconds', '0'
    ) $Prefix $Environment
    $script:activeRuntime = $started
    return $started
}

function Wait-TicketFile {
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
    Stop-CapturedProcess $Runtime
    throw "runtime did not publish ticket within $TimeoutSeconds seconds"
}

function Invoke-RemoteSyncWithRuntime {
    param(
        [object] $Pair, [string] $Prefix, [int] $MaxSessions,
        [hashtable] $RuntimeEnvironment = @{}, [hashtable] $SyncEnvironment = @{},
        [switch] $AllowSyncFailure, [string] $Conversation = ''
    )
    if (-not $Conversation) { $Conversation = $Pair.Conversation }
    $ticket = Join-Path $Pair.Directory "$Prefix.ticket"
    $remoteTicket = "$($Pair.RemoteScenario)/$Prefix.ticket"
    $runtime = $null
    try {
        $runtime = Start-LocalRuntime $Pair $ticket (Join-Path $Pair.Directory "$Prefix-bob-runtime") $MaxSessions $RuntimeEnvironment
        Wait-TicketFile $ticket $runtime
        Invoke-PscpUpload $ticket $remoteTicket "$Prefix-upload-ticket"
        $sync = Invoke-RemoteCli @('sync', '--state-dir', $Pair.AliceState, '--ticket-file', $remoteTicket, '--conversation', $Conversation, '--expect-account', $Pair.BobAccountId) "$Prefix-alice-sync" 300 -AllowFailure:$AllowSyncFailure -Environment $SyncEnvironment
        $runtimeResult = Wait-CapturedProcess $runtime "$Prefix Bob runtime" 300
        $script:activeRuntime = $null
        $runtime = $null
        return [PSCustomObject]@{ Sync = $sync; Runtime = $runtimeResult }
    }
    finally {
        Stop-CapturedProcess $runtime
        if ($null -ne $runtime) { $script:activeRuntime = $null }
    }
}

function Assert-CanonicalPair {
    param([object] $Result, [string] $ExpectedEvidence = '')
    Assert-Field $Result.Sync.Stdout 'sync_transport_selected_alpn' 'kilogram/m1/sync/cbor/2'
    Assert-Field $Result.Sync.Stdout 'sync_transport_fallback_after_selection' 'false'
    $aliceEvidence = Get-Field $Result.Sync.Stdout 'sync_transport_selection_evidence_id'
    $bobEvidence = Get-Field $Result.Runtime.Stdout 'sync_transport_selection_evidence_id'
    if ($aliceEvidence -cne $bobEvidence -or $aliceEvidence -cnotmatch '^[0-9a-f]{64}$') { throw 'two-host canonical selection evidence differs between roles' }
    if ($ExpectedEvidence -and $aliceEvidence -cne $ExpectedEvidence) { throw 'two-host canonical selection changed across restart/containment' }
    return $aliceEvidence
}

function Assert-HistoryConvergence {
    param([object] $Pair, [string] $Prefix, [string] $Conversation = '')
    if (-not $Conversation) { $Conversation = $Pair.Conversation }
    $bob = Invoke-LocalCli @('history', '--state-dir', $Pair.BobState, '--conversation', $Conversation) "$Prefix-bob-history"
    $alice = Invoke-RemoteCli @('history', '--state-dir', $Pair.AliceState, '--conversation', $Conversation) "$Prefix-alice-history"
    $bobIds = @(Get-HistoryEventIds $bob.Stdout)
    $aliceIds = @(Get-HistoryEventIds $alice.Stdout)
    if ($bobIds.Count -eq 0 -or ($bobIds -join ',') -cne ($aliceIds -join ',')) { throw "$Prefix histories differ across physical hosts" }
    return $bobIds.Count
}

function Invoke-BasicScenario {
    param([string] $Name, [ValidateSet('default-default','optin-default','optin-optin')] [string] $Policy)
    $pair = Initialize-Pair $Name
    if ($Policy -in @('optin-default','optin-optin')) { Enable-LocalListener $pair "$Name-bob" }
    if ($Policy -ceq 'optin-optin') { Enable-RemoteInitiator $pair "$Name-alice" }
    $max = if ($Policy -ceq 'optin-optin') { 2 } else { 1 }
    $result = Invoke-RemoteSyncWithRuntime $pair 'network' $max
    $expectedAlpn = if ($Policy -ceq 'optin-optin') { 'kilogram/m1/sync/cbor/2' } else { 'kilogram/m0/sync/8' }
    Assert-Field $result.Sync.Stdout 'sync_transport_selected_alpn' $expectedAlpn
    Assert-Field $result.Sync.Stdout 'status' 'synchronized'
    $evidence = if ($Policy -ceq 'optin-optin') { Assert-CanonicalPair $result } else { $null }
    $events = Assert-HistoryConvergence $pair "$Name-final"
    return [ordered]@{ name = $Name; policy = $Policy; selected_alpn = $expectedAlpn; event_count = $events; histories_equal = $true; selection_evidence_id = $evidence; fallback_after_selection = $false }
}

function Invoke-RestartContainmentScenario {
    $pair = Initialize-Pair '04-restart-containment'
    Enable-LocalListener $pair 'restart-bob'
    Enable-RemoteInitiator $pair 'restart-alice'
    $interrupted = Invoke-RemoteSyncWithRuntime $pair 'interrupted' 2 @{} @{ KILOGRAM_TEST_STOP_CANONICAL_SYNC_V2_AFTER_DIFF_ONCE = '1' }
    Assert-Field $interrupted.Sync.Stdout 'status' 'canonical-sync-v2-interrupted-after-durable-diff'
    $evidence = Get-Field $interrupted.Sync.Stdout 'sync_transport_selection_evidence_id'
    if ($interrupted.Runtime.Stderr -cnotmatch '(?m)^runtime_canonical_sync_v2_session_status=failed ') { throw 'two-host restart responder did not observe the interrupted selected session' }
    $resumed = Invoke-RemoteSyncWithRuntime $pair 'resumed' 1
    Assert-Field $resumed.Sync.Stdout 'status' 'synchronized'
    $null = Assert-CanonicalPair $resumed $evidence
    $events = Assert-HistoryConvergence $pair 'restart-final'

    Apply-RemoteActivation $pair.AliceState 'dual-reader-prefer-canonical' 'containment-alice-evidence' 'normal' $evidence
    Apply-LocalActivation $pair.BobState 'dual-reader-prefer-legacy' 'containment-bob-evidence' 'normal' $evidence
    Apply-RemoteActivation $pair.AliceState 'dual-reader-prefer-canonical' 'containment-alice' 'emergency-containment' $evidence
    Apply-LocalActivation $pair.BobState 'dual-reader-prefer-legacy' 'containment-bob' 'emergency-containment' $evidence
    $contained = Invoke-RemoteSyncWithRuntime $pair 'contained-resume' 1
    Assert-Field $contained.Sync.Stdout 'canonical_sync_v2_containment' 'true'
    Assert-Field $contained.Sync.Stdout 'canonical_sync_v2_capability_published' 'false'
    $null = Assert-CanonicalPair $contained $evidence

    $conversation = "$RunId-containment-new"
    $membership = Join-Path $pair.Directory 'new.membership'
    $remoteMembership = "$($pair.RemoteScenario)/new.membership"
    $null = Invoke-LocalCli @('conversation-create', '--account-dir', $pair.BobAccount, '--conversation', $conversation, '--member-account', $pair.BobAccountId, '--member-account', $pair.AliceAccountId, '--membership-file', $membership) 'containment-new-conversation'
    $null = Invoke-LocalCli @('conversation-membership-install', '--state-dir', $pair.BobState, '--membership-file', $membership) 'containment-new-bob-membership'
    Invoke-PscpUpload $membership $remoteMembership 'containment-upload-new-membership'
    $null = Invoke-RemoteCli @('conversation-membership-install', '--state-dir', $pair.AliceState, '--membership-file', $remoteMembership) 'containment-new-alice-membership'
    $remotePool = "$($pair.RemoteScenario)/alice/prekeys-next.pool"
    $localPool = Join-Path $pair.Directory 'alice-public\prekeys-next.pool'
    $null = Invoke-RemoteCli @('ratchet-prekey-pool', '--state-dir', $pair.AliceState, '--pool-file', $remotePool, '--count', '16') 'containment-new-alice-prekeys'
    Invoke-PscpDownload $remotePool $localPool 'containment-download-new-prekeys'
    $null = Invoke-LocalCli @('seed-history', '--state-dir', $pair.BobState, '--conversation', $conversation, '--count', '4', '--message-prefix', "M1.1.84-field-containment-$RunId", '--peer-certificate-file', $pair.AliceCertificate, '--peer-device-list-file', $pair.AliceDevices, '--peer-prekey-pool-file', $localPool) 'containment-seed-new' 300
    $newResult = Invoke-RemoteSyncWithRuntime $pair 'contained-new' 1 @{} @{} -Conversation $conversation
    Assert-Field $newResult.Sync.Stdout 'status' 'synchronized'
    Assert-Field $newResult.Sync.Stdout 'sync_transport_selected_alpn' 'kilogram/m0/sync/8'
    Assert-Field $newResult.Sync.Stdout 'sync_transport_selection_evidence_id' 'none'
    Assert-Field $newResult.Runtime.Stdout 'canonical_sync_v2_capability_published' 'false'
    $newEvents = Assert-HistoryConvergence $pair 'containment-new-final' $conversation
    return [ordered]@{ name = '04-restart-containment'; selected_alpn = 'kilogram/m1/sync/cbor/2'; interruption_after_durable_diff = $true; process_restart = $true; exact_selection_resumed = $true; selection_evidence_id = $evidence; event_count = $events; histories_equal = $true; containment_resume = $true; new_canonical_attempts_enabled = $false; new_conversation_selected_alpn = 'kilogram/m0/sync/8'; new_conversation_event_count = $newEvents; fallback_after_selection = $false }
}

function Invoke-SelectedLossScenario {
    $pair = Initialize-Pair '05-selected-loss'
    Enable-LocalListener $pair 'loss-bob'
    Enable-RemoteInitiator $pair 'loss-alice'
    $result = Invoke-RemoteSyncWithRuntime $pair 'selected-loss' 2 @{ KILOGRAM_TEST_STOP_CANONICAL_SYNC_V2_AFTER_DIFF_ONCE = '1' } @{} -AllowSyncFailure
    if ($result.Sync.ExitCode -eq 0) { throw 'two-host selected canonical responder loss unexpectedly succeeded' }
    Assert-Field $result.Sync.Stdout 'sync_transport_selected_alpn' 'kilogram/m1/sync/cbor/2'
    Assert-Field $result.Sync.Stdout 'sync_transport_fallback_after_selection' 'false'
    Assert-Field $result.Runtime.Stdout 'runtime_test_fault_status' 'canonical-sync-v2-stopped-after-diff'
    if ($result.Sync.Stdout -cmatch '(?m)^status=synchronized$') { throw 'two-host selected loss reported synchronized' }
    return [ordered]@{ name = '05-selected-loss'; selected_alpn = 'kilogram/m1/sync/cbor/2'; client_exit_code = $result.Sync.ExitCode; synchronized = $false; legacy_retry = $false; fallback_after_selection = $false }
}

function Invoke-RollbackScenario {
    $pair = Initialize-Pair '06-pre-selection-rollback'
    Enable-RemoteInitiator $pair 'rollback-alice'
    Apply-RemoteActivation $pair.AliceState 'dual-reader-prefer-legacy' 'rollback-alice-one-step'
    $status = Invoke-RemoteCli @('canonical-sync-policy-status', '--state-dir', $pair.AliceState) 'rollback-alice-status'
    Assert-Field $status.Stdout 'activation_migration_epoch' '4'
    Assert-Field $status.Stdout 'activation_migration_state' 'dual-reader-prefer-legacy'
    Assert-Field $status.Stdout 'activation_canonical_evidence_id' 'none'
    return [ordered]@{ name = '06-pre-selection-rollback'; migration_epoch = 4; from = 'dual-reader-prefer-canonical'; to = 'dual-reader-prefer-legacy'; canonical_evidence = 'none'; rollback = 'explicit-one-step' }
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()

try {
    if (-not (Test-NetConnection -ComputerName $RemoteHost -Port 22 -InformationLevel Quiet)) { throw "SSH is not reachable at ${RemoteHost}:22" }
    $null = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; New-Item -ItemType Directory -Force -Path 'C:/Users/test/KilogramLab/field-kit-current', $(Quote-PowerShellLiteral $remoteRoot) | Out-Null" 'remote-preflight'
    Invoke-PscpUpload $binary $remoteBinary 'upload-exact-binary'
    $probe = Invoke-RemoteScript "Write-Output ('remote_hostname=' + [Environment]::MachineName); Write-Output ('remote_binary_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteBinary) -Algorithm SHA256).Hash.ToLowerInvariant())" 'remote-binary-probe'
    $remoteMachine = Get-Field $probe.Stdout 'remote_hostname'
    $remoteHash = Get-Field $probe.Stdout 'remote_binary_sha256'
    if ($remoteHash -cne $binaryHash) { throw 'remote binary SHA-256 differs from the local executable' }
    if ($remoteMachine -ceq [Environment]::MachineName) { throw 'two-host rehearsal requires distinct physical hosts' }

    $scenarios = @(
        Invoke-BasicScenario '01-default-default' 'default-default'
        Invoke-BasicScenario '02-optin-default' 'optin-default'
        Invoke-BasicScenario '03-optin-optin' 'optin-optin'
    )
    if ($BasicPairPreparationOnly) {
        $manifest = [ordered]@{
            schema_version = 1
            milestone = 'M1.2.7-fixture'
            status = 'canonical-sync-v2-two-host-basic-pairs-prepared'
            source_revision = $SourceRevision
            run_id = $RunId
            collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
            binary = [ordered]@{ name = 'kilogram-cli.exe'; sha256 = $binaryHash; bytes = [int64]$binaryItem.Length; profile = 'debug'; target = 'x86_64-pc-windows-msvc' }
            topology = [ordered]@{ alice_host = $remoteMachine; bob_host = [Environment]::MachineName; distinct_physical_hosts = $true; alice_binary_sha256 = $remoteHash; bob_binary_sha256 = $binaryHash; transport = 'iroh-direct-lan' }
            public_default = 'legacy-only'
            scenarios = $scenarios
        }
        $manifestPath = Join-Path $output 'M1207-BASIC-PAIR-EVIDENCE.json'
        [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
        Write-Output "evidence_manifest=$manifestPath"
        Write-Output 'status=canonical-sync-v2-two-host-basic-pairs-prepared'
        return
    }
    $scenarios += @(
        Invoke-RestartContainmentScenario
        Invoke-SelectedLossScenario
        Invoke-RollbackScenario
    )
    $manifest = [ordered]@{
        schema_version = 1
        milestone = 'M1.1.84'
        status = 'canonical-sync-v2-two-host-activation-rehearsal-verified'
        source_revision = $SourceRevision
        run_id = $RunId
        collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        binary = [ordered]@{ name = 'kilogram-cli.exe'; sha256 = $binaryHash; bytes = [int64]$binaryItem.Length; profile = 'debug'; target = 'x86_64-pc-windows-msvc' }
        topology = [ordered]@{ alice_host = $remoteMachine; bob_host = [Environment]::MachineName; distinct_physical_hosts = $true; alice_binary_sha256 = $remoteHash; bob_binary_sha256 = $binaryHash; transport = 'iroh-direct-lan' }
        public_default = 'legacy-only'
        test_faults = [ordered]@{ debug_build_only = $true; release_available = $false; environment_authority = $false }
        scenarios = $scenarios
    }
    $manifestPath = Join-Path $output 'M1184-TWO-HOST-EVIDENCE.json'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Write-Output "evidence_manifest=$manifestPath"
    Write-Output 'status=canonical-sync-v2-two-host-activation-rehearsal-verified'
}
finally {
    Stop-CapturedProcess $script:activeRuntime
    try { $null = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; exit 0" 'remote-final-cleanup' 30 } catch { Write-Warning "remote cleanup could not be confirmed: $($_.Exception.Message)" }
}
