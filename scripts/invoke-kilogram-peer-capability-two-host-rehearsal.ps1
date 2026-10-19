[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $SourceRevision,
    [string] $RemoteHost = '192.168.0.152',
    [string] $RemoteUser = 'test',
    [string] $PlinkPath = 'C:\Program Files\Utils\plink.exe',
    [string] $PscpPath = 'C:\Program Files\Utils\pscp.exe',
    [string] $RunId = ('m1207-capability-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))
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
$baseHarness = Join-Path $PSScriptRoot 'invoke-kilogram-canonical-sync-v2-dark-launch-two-host.ps1'
foreach ($path in @($binary, $baseHarness, $PlinkPath, $PscpPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required file is missing: $path" }
}
if (Test-Path -LiteralPath $output) { throw "field evidence directory already exists: $output" }
New-Item -ItemType Directory -Path $output | Out-Null
$baseOutput = Join-Path $output 'base-rehearsal'
$controllerLogs = Join-Path $output 'controller-logs'
New-Item -ItemType Directory -Path $controllerLogs | Out-Null
$remoteRoot = "C:/Users/$RemoteUser/KilogramLab/$RunId"
$remoteBinary = "C:/Users/$RemoteUser/KilogramLab/field-kit-current/kilogram-cli.exe"
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

function Invoke-RemoteScript {
    param(
        [Parameter(Mandatory)] [string] $Script,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300,
        [switch] $AllowFailure
    )
    return Invoke-CapturedProcess $PlinkPath @(
        '-ssh', '-batch', '-pw', $remotePassword, "$RemoteUser@$RemoteHost",
        'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
        '-EncodedCommand', (New-EncodedRemoteCommand $Script)
    ) $Description $TimeoutSeconds -AllowFailure:$AllowFailure
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

function Invoke-LocalCli {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300
    )
    return Invoke-CapturedProcess $binary $Arguments $Description $TimeoutSeconds
}

function Invoke-PscpUpload {
    param([string] $LocalPath, [string] $RemotePath, [string] $Description)
    $null = Invoke-CapturedProcess $PscpPath @(
        '-batch', '-pw', $remotePassword, $LocalPath, "${RemoteUser}@${RemoteHost}:$RemotePath"
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

function Get-HistoryEventIds {
    param([string] $Text)
    return @([regex]::Matches($Text, '(?m)^event_id=([0-9a-f]{64}) ') | ForEach-Object { $_.Groups[1].Value } | Sort-Object)
}

function Get-PairContext {
    param([Parameter(Mandatory)] [string] $Name)
    $directory = Join-Path $baseOutput $Name
    $bobAccount = Join-Path $directory 'bob\account'
    $bobState = Join-Path $directory 'bob\state'
    $bobDevices = Join-Path $directory 'bob\devices.list'
    $remoteScenario = "$remoteRoot/$Name"
    $aliceAccount = "$remoteScenario/alice/account"
    $aliceState = "$remoteScenario/alice/state"
    $bobAccountResult = Invoke-LocalCli @('account-show', '--account-dir', $bobAccount) "$Name-bob-account-show"
    $bobIdentityResult = Invoke-LocalCli @('identity', '--state-dir', $bobState) "$Name-bob-identity"
    $aliceAccountResult = Invoke-RemoteCli @('account-show', '--account-dir', $aliceAccount) "$Name-alice-account-show"
    return [PSCustomObject]@{
        Name = $Name
        Directory = $directory
        BobState = $bobState
        BobDevices = $bobDevices
        BobAccountId = Get-Field $bobAccountResult.Stdout 'account_id'
        BobDeviceId = Get-Field $bobIdentityResult.Stdout 'device_id'
        AliceAccountId = Get-Field $aliceAccountResult.Stdout 'account_id'
        AliceState = $aliceState
        RemoteScenario = $remoteScenario
        Conversation = "$RunId-$Name"
    }
}

function Start-LocalRuntime {
    param([object] $Pair, [string] $TicketPath, [string] $Prefix)
    $started = Start-CapturedProcess $binary @(
        'runtime', '--state-dir', $Pair.BobState, '--allow-account', $Pair.AliceAccountId,
        '--device-list-file', $Pair.BobDevices, '--ticket-file', $TicketPath,
        '--route-policy', 'direct-only', '--relay-wait-seconds', '0',
        '--max-sessions', '1', '--idle-seconds', '120', '--poll-milliseconds', '5000',
        '--auto-sync-seconds', '0'
    ) (Join-Path $controllerLogs $Prefix)
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

function New-ExtensionFreeTicket {
    param([string] $InputPath, [string] $OutputPath)
    $ticket = Get-Content -LiteralPath $InputPath -Raw | ConvertFrom-Json
    $property = $ticket.content.PSObject.Properties['sync_transport_capability']
    if ($null -eq $property) { throw "source ticket has no sync_transport_capability property: $InputPath" }
    $ticket.content.PSObject.Properties.Remove('sync_transport_capability')
    [IO.File]::WriteAllText(
        $OutputPath,
        ($ticket | ConvertTo-Json -Depth 100 -Compress),
        [Text.UTF8Encoding]::new($false)
    )
}

function Get-RemoteStorePresence {
    param([object] $Pair, [string] $Description)
    $store = "$($Pair.AliceState)/peer-sync-capability"
    $result = Invoke-RemoteScript "Write-Output ('peer_capability_store_present=' + (Test-Path -LiteralPath $(Quote-PowerShellLiteral $store) -PathType Container).ToString().ToLowerInvariant())" $Description
    return Get-Field $result.Stdout 'peer_capability_store_present'
}

function Get-RemotePeerStatus {
    param([object] $Pair, [string] $Description)
    return Invoke-RemoteCli @(
        'peer-capability-status', '--state-dir', $Pair.AliceState,
        '--peer-account-id', $Pair.BobAccountId, '--peer-device-id', $Pair.BobDeviceId
    ) $Description
}

function Assert-HistoryConvergence {
    param([object] $Pair, [string] $Description)
    $bob = Invoke-LocalCli @('history', '--state-dir', $Pair.BobState, '--conversation', $Pair.Conversation) "$Description-bob-history"
    $alice = Invoke-RemoteCli @('history', '--state-dir', $Pair.AliceState, '--conversation', $Pair.Conversation) "$Description-alice-history"
    $bobIds = @(Get-HistoryEventIds $bob.Stdout)
    $aliceIds = @(Get-HistoryEventIds $alice.Stdout)
    if ($bobIds.Count -eq 0 -or ($bobIds -join ',') -cne ($aliceIds -join ',')) {
        throw "$Description histories differ across physical hosts"
    }
    return $bobIds.Count
}

function Invoke-LegacyNoCreateScenario {
    $pair = Get-PairContext '01-default-default'
    if ((Get-RemoteStorePresence $pair 'legacy-initial-store-presence') -cne 'false') {
        throw 'legacy baseline unexpectedly created the peer capability store'
    }
    $ticket = Join-Path $output 'legacy-live.ticket'
    $stripped = Join-Path $output 'legacy-extension-free.ticket'
    $remoteTicket = "$($pair.RemoteScenario)/legacy-extension-free.ticket"
    $runtime = $null
    try {
        $runtime = Start-LocalRuntime $pair $ticket 'legacy-bob-runtime'
        Wait-TicketFile $ticket $runtime
        New-ExtensionFreeTicket $ticket $stripped
        Invoke-PscpUpload $stripped $remoteTicket 'upload-legacy-extension-free-ticket'
        $sync = Invoke-RemoteCli @(
            'sync', '--state-dir', $pair.AliceState, '--ticket-file', $remoteTicket,
            '--conversation', $pair.Conversation, '--expect-account', $pair.BobAccountId
        ) 'legacy-extension-free-sync' 300
        $runtimeResult = Wait-CapturedProcess $runtime 'legacy Bob runtime' 300
        $runtime = $null
        $script:activeRuntime = $null
        Assert-Field $sync.Stdout 'peer_capability_transport_evidence_source' 'connection-ticket'
        Assert-Field $sync.Stdout 'peer_capability_transport_use' 'legacy-compatibility-unasserted'
        Assert-Field $sync.Stdout 'peer_capability_record_present' 'false'
        Assert-Field $sync.Stdout 'sync_transport_selected_alpn' 'kilogram/m0/sync/8'
        Assert-Field $sync.Stdout 'status' 'synchronized'
        Assert-Field $runtimeResult.Stdout 'status' 'runtime-stopped'
    }
    finally {
        Stop-CapturedProcess $runtime
        if ($null -ne $runtime) { $script:activeRuntime = $null }
    }
    if ((Get-RemoteStorePresence $pair 'legacy-final-store-presence') -cne 'false') {
        throw 'extension-free legacy sync created the peer capability store'
    }
    $events = Assert-HistoryConvergence $pair 'legacy-extension-free'
    return [ordered]@{
        name = 'extension-free-first-contact'
        synchronized = $true
        selected_alpn = 'kilogram/m0/sync/8'
        semantic_use = 'legacy-compatibility-unasserted'
        store_created = $false
        event_count = $events
        histories_equal = $true
    }
}

function Invoke-PinAndQuarantineScenario {
    $pair = Get-PairContext '02-optin-default'
    $pinned = Get-RemotePeerStatus $pair 'candidate-pinned-status'
    Assert-Field $pinned.Stdout 'peer_capability_record_present' 'true'
    Assert-Field $pinned.Stdout 'peer_capability_record_sequence' '1'
    Assert-Field $pinned.Stdout 'peer_capability_floor' 'canonical-candidate-pinned'

    $sourceTicket = Join-Path $pair.Directory 'network.ticket'
    $stripped = Join-Path $output 'candidate-stripped.ticket'
    $remoteTicket = "$($pair.RemoteScenario)/candidate-stripped.ticket"
    New-ExtensionFreeTicket $sourceTicket $stripped
    Invoke-PscpUpload $stripped $remoteTicket 'upload-stripped-candidate-ticket'
    $sync = Invoke-RemoteCli @(
        'sync', '--state-dir', $pair.AliceState, '--ticket-file', $remoteTicket,
        '--conversation', $pair.Conversation, '--expect-account', $pair.BobAccountId
    ) 'stripped-candidate-sync' 60 -AllowFailure
    if ($sync.ExitCode -eq 0) { throw 'stripped capability unexpectedly reached a successful sync' }
    if ($sync.Stderr -cnotmatch 'peer sync capability floor is quarantined: pinned-canonical-capability-missing') {
        throw "stripped capability did not report the exact quarantine reason: $($sync.Stderr)"
    }
    if ($sync.Stdout -cmatch '(?m)^transport_endpoint_id=') {
        throw 'stripped capability reached endpoint creation before quarantine'
    }

    $quarantined = Get-RemotePeerStatus $pair 'candidate-quarantined-status'
    Assert-Field $quarantined.Stdout 'peer_capability_record_present' 'true'
    Assert-Field $quarantined.Stdout 'peer_capability_record_sequence' '2'
    Assert-Field $quarantined.Stdout 'peer_capability_floor' 'quarantined'
    Assert-Field $quarantined.Stdout 'peer_capability_quarantine_reason' 'pinned-canonical-capability-missing'
    return [ordered]@{
        name = 'pin-then-strip'
        initial_floor = 'canonical-candidate-pinned'
        initial_record_sequence = 1
        stripped_sync_succeeded = $false
        endpoint_created = $false
        final_floor = 'quarantined'
        quarantine_reason = 'pinned-canonical-capability-missing'
        final_record_sequence = 2
    }
}

function Invoke-SelectedResumeScenario {
    param([string] $ExpectedEvidence)
    if ($ExpectedEvidence -cnotmatch '^[0-9a-f]{64}$') { throw 'base canonical selection evidence is invalid' }
    $pair = Get-PairContext '03-optin-optin'
    $ticket = Join-Path $output 'selected-resume-live.ticket'
    $stripped = Join-Path $output 'selected-resume-extension-free.ticket'
    $remoteTicket = "$($pair.RemoteScenario)/selected-resume-extension-free.ticket"
    $runtime = $null
    try {
        $runtime = Start-LocalRuntime $pair $ticket 'selected-resume-bob-runtime'
        Wait-TicketFile $ticket $runtime
        New-ExtensionFreeTicket $ticket $stripped
        Invoke-PscpUpload $stripped $remoteTicket 'upload-selected-resume-extension-free-ticket'
        $sync = Invoke-RemoteCli @(
            'sync', '--state-dir', $pair.AliceState, '--ticket-file', $remoteTicket,
            '--conversation', $pair.Conversation, '--expect-account', $pair.BobAccountId
        ) 'selected-resume-sync' 300
        $runtimeResult = Wait-CapturedProcess $runtime 'selected resume Bob runtime' 300
        $runtime = $null
        $script:activeRuntime = $null
        Assert-Field $sync.Stdout 'peer_capability_transport_evidence_source' 'persisted-canonical-selection'
        Assert-Field $sync.Stdout 'peer_capability_transport_use' 'authenticated-canonical-candidate'
        Assert-Field $sync.Stdout 'peer_capability_record_present' 'true'
        Assert-Field $sync.Stdout 'peer_capability_record_sequence' '1'
        Assert-Field $sync.Stdout 'peer_capability_record_changed' 'false'
        Assert-Field $sync.Stdout 'sync_transport_selected_alpn' 'kilogram/m1/sync/cbor/2'
        Assert-Field $sync.Stdout 'sync_transport_fallback_after_selection' 'false'
        Assert-Field $sync.Stdout 'sync_transport_selection_evidence_id' $ExpectedEvidence
        Assert-Field $sync.Stdout 'status' 'synchronized'
        Assert-Field $runtimeResult.Stdout 'sync_transport_selection_evidence_id' $ExpectedEvidence
        Assert-Field $runtimeResult.Stdout 'status' 'runtime-stopped'
    }
    finally {
        Stop-CapturedProcess $runtime
        if ($null -ne $runtime) { $script:activeRuntime = $null }
    }
    $status = Get-RemotePeerStatus $pair 'selected-resume-peer-status'
    Assert-Field $status.Stdout 'peer_capability_record_sequence' '1'
    Assert-Field $status.Stdout 'peer_capability_floor' 'canonical-candidate-pinned'
    $events = Assert-HistoryConvergence $pair 'selected-resume'
    return [ordered]@{
        name = 'selected-canonical-resume-with-extension-free-ticket'
        evidence_source = 'persisted-canonical-selection'
        selected_alpn = 'kilogram/m1/sync/cbor/2'
        fallback_after_selection = $false
        capability_record_changed = $false
        record_sequence = 1
        synchronized = $true
        event_count = $events
        histories_equal = $true
        selection_evidence_consistent = $true
    }
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()

try {
    Write-Output 'Running the established two-host canonical baseline...'
    $baseLines = @(& $baseHarness -BinaryPath $binary -OutputDirectory $baseOutput -SourceRevision $SourceRevision -RemoteHost $RemoteHost -RemoteUser $RemoteUser -PlinkPath $PlinkPath -PscpPath $PscpPath -RunId $RunId -BasicPairPreparationOnly 2>&1)
    if ('status=canonical-sync-v2-two-host-basic-pairs-prepared' -notin @($baseLines | ForEach-Object { [string]$_ })) {
        throw "base two-host pair preparation did not report success: $($baseLines -join [Environment]::NewLine)"
    }
    $baseEvidencePath = Join-Path $baseOutput 'M1207-BASIC-PAIR-EVIDENCE.json'
    $baseEvidence = Get-Content -LiteralPath $baseEvidencePath -Raw | ConvertFrom-Json
    if ([string]$baseEvidence.source_revision -cne $SourceRevision -or
        [string]$baseEvidence.binary.sha256 -cne $binaryHash -or
        -not [bool]$baseEvidence.topology.distinct_physical_hosts) {
        throw 'base two-host evidence does not match the exact binary and topology'
    }
    $canonicalScenario = @($baseEvidence.scenarios | Where-Object { [string]$_.name -ceq '03-optin-optin' })
    if ($canonicalScenario.Count -ne 1) { throw 'base evidence contains no unique opt-in canonical scenario' }
    $expectedSelection = [string]$canonicalScenario[0].selection_evidence_id

    Write-Output 'Checking extension-free legacy first contact without store creation...'
    $legacy = Invoke-LegacyNoCreateScenario
    Write-Output 'Checking durable candidate pin and pre-network quarantine...'
    $pinAndQuarantine = Invoke-PinAndQuarantineScenario
    Write-Output 'Checking selected canonical resume with the current ticket extension removed...'
    $selectedResume = Invoke-SelectedResumeScenario $expectedSelection

    $manifest = [ordered]@{
        schema_version = 1
        milestone = 'M1.2.7'
        status = 'canonical-sync-v2-peer-capability-two-host-rehearsal-verified'
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
        base_pair_preparation = [ordered]@{
            status = 'canonical-sync-v2-two-host-basic-pairs-prepared'
            scenarios = 3
        }
        scenarios = @($legacy, $pinAndQuarantine, $selectedResume)
        privacy = [ordered]@{
            raw_logs_tracked = $false
            state_tracked = $false
            ticket_bodies_tracked = $false
            account_or_device_ids_tracked = $false
            endpoint_addresses_tracked = $false
            selection_evidence_ids_tracked = $false
            secrets_tracked = $false
        }
    }
    $manifestPath = Join-Path $output 'M1207-PEER-CAPABILITY-TWO-HOST-EVIDENCE.json'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output 'extension_free_first_contact=legacy-no-create'
    Write-Output 'authenticated_candidate=durably-pinned'
    Write-Output 'stripped_after_pin=durably-quarantined-before-endpoint'
    Write-Output 'selected_canonical_resume=persisted-selection-no-fallback'
    Write-Output "binary_sha256=$binaryHash"
    Write-Output "evidence_manifest=$manifestPath"
    Write-Output 'status=canonical-sync-v2-peer-capability-two-host-rehearsal-verified'
}
finally {
    Stop-CapturedProcess $script:activeRuntime
    try {
        $null = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; exit 0" 'remote-final-cleanup' 30
    }
    catch {
        Write-Warning "remote cleanup could not be confirmed: $($_.Exception.Message)"
    }
}
