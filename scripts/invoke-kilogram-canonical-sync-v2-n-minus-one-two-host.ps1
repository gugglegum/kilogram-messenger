[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $CurrentBinaryPath,
    [Parameter(Mandatory)] [string] $PreviousBinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [Parameter(Mandatory)] [string] $CurrentRevision,
    [Parameter(Mandatory)] [string] $PreviousRevision,
    [string] $PreviousTag = 'M1.0.0',
    [string] $RemoteHost = '192.168.0.152',
    [string] $RemoteUser = 'test',
    [string] $PlinkPath = 'C:\Program Files\Utils\plink.exe',
    [string] $PscpPath = 'C:\Program Files\Utils\pscp.exe',
    [string] $RelayUrl = 'https://aps1-1.relay.n0.iroh.link./',
    [string] $RunId = ('m1191-interop-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$remotePassword = [Environment]::GetEnvironmentVariable('KILOGRAM_TEST_SSH_PASSWORD')
if ([string]::IsNullOrWhiteSpace($remotePassword)) {
    throw 'KILOGRAM_TEST_SSH_PASSWORD is required for the disposable field-test account'
}
foreach ($revision in @($CurrentRevision, $PreviousRevision)) {
    if ($revision -cnotmatch '^[0-9a-f]{40}$') { throw 'Both revisions must be exact lowercase Git commit IDs' }
}
if ($PreviousTag -cnotmatch '^M[0-9]+\.[0-9]+\.[0-9]+$') { throw 'PreviousTag must be an exact milestone tag' }
if ($RunId -cnotmatch '^[A-Za-z0-9_-]{1,96}$') { throw 'RunId contains unsupported characters' }
if ($RelayUrl -cnotmatch '^https://[A-Za-z0-9.-]+/$') { throw 'RelayUrl must be one exact HTTPS relay URL ending in slash' }

$currentBinary = [IO.Path]::GetFullPath($CurrentBinaryPath)
$previousBinary = [IO.Path]::GetFullPath($PreviousBinaryPath)
$output = [IO.Path]::GetFullPath($OutputDirectory)
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
foreach ($path in @($currentBinary, $previousBinary, $PlinkPath, $PscpPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required file is missing: $path" }
}
if (Test-Path -LiteralPath $output) { throw "interop evidence directory already exists: $output" }
New-Item -ItemType Directory -Path $output | Out-Null
$controllerLogs = Join-Path $output 'controller-logs'
$runnerDirectory = Join-Path $workspace '.tmp\kilogram-field-runner'
New-Item -ItemType Directory -Path $controllerLogs, $runnerDirectory | Out-Null
$localRunner = Join-Path $runnerDirectory 'kilogram-cli.exe'
$remoteRoot = "C:/Users/$RemoteUser/KilogramLab/$RunId"
$remoteRunnerDirectory = "C:/Users/$RemoteUser/KilogramLab/n-minus-one-stable"
$remoteRunner = "$remoteRunnerDirectory/kilogram-cli.exe"
$script:processCounter = 0
$script:activeListener = $null
$script:localActiveHash = ''
$script:remoteActiveHash = ''

$versions = [ordered]@{
    N = [PSCustomObject]@{
        Label = 'N'
        Revision = $CurrentRevision
        Binary = $currentBinary
        Hash = (Get-FileHash -LiteralPath $currentBinary -Algorithm SHA256).Hash.ToLowerInvariant()
        Bytes = [int64](Get-Item -LiteralPath $currentBinary).Length
    }
    'N-1' = [PSCustomObject]@{
        Label = 'N-1'
        Revision = $PreviousRevision
        Binary = $previousBinary
        Hash = (Get-FileHash -LiteralPath $previousBinary -Algorithm SHA256).Hash.ToLowerInvariant()
        Bytes = [int64](Get-Item -LiteralPath $previousBinary).Length
    }
}
if ($versions.N.Hash -ceq $versions.'N-1'.Hash) { throw 'N and N-1 executables unexpectedly have the same SHA-256' }

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
        [int] $TimeoutSeconds = 300
    )
    if (-not $Started.Process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-CapturedProcess $Started
        throw "$Description timed out after $TimeoutSeconds seconds"
    }
    $Started.Process.WaitForExit()
    $result = Complete-CapturedProcess $Started
    if ($result.ExitCode -ne 0) {
        throw "$Description failed ($($result.ExitCode)): stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
    }
    return $result
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300
    )
    $script:processCounter++
    $safe = $Description -replace '[^A-Za-z0-9_-]', '-'
    $prefix = Join-Path $controllerLogs ('{0:D3}-{1}' -f $script:processCounter, $safe)
    $started = Start-CapturedProcess $FilePath $Arguments $prefix
    return Wait-CapturedProcess $started $Description $TimeoutSeconds
}

function Quote-PowerShellLiteral {
    param([Parameter(Mandatory)] [string] $Value)
    return "'" + $Value.Replace("'", "''") + "'"
}

function Invoke-RemoteScript {
    param(
        [Parameter(Mandatory)] [string] $Script,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 300
    )
    $body = "`$ErrorActionPreference = 'Stop'; " + $Script
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
    return Invoke-CapturedProcess $PlinkPath @(
        '-ssh', '-batch', '-pw', $remotePassword, "$RemoteUser@$RemoteHost",
        'powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
        '-EncodedCommand', $encoded
    ) $Description $TimeoutSeconds
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

function Set-LocalVersion {
    param([Parameter(Mandatory)] [object] $Version)
    if ($script:localActiveHash -ceq $Version.Hash) { return }
    Copy-Item -LiteralPath $Version.Binary -Destination $localRunner -Force
    $actual = (Get-FileHash -LiteralPath $localRunner -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -cne $Version.Hash) { throw "stable local runner does not match $($Version.Label)" }
    $script:localActiveHash = $actual
}

function Set-RemoteVersion {
    param([Parameter(Mandatory)] [object] $Version)
    if ($script:remoteActiveHash -ceq $Version.Hash) { return }
    $incoming = "$remoteRunner.incoming"
    Invoke-PscpUpload $Version.Binary $incoming "upload-$($Version.Label)"
    $probe = Invoke-RemoteScript "Move-Item -Force -LiteralPath $(Quote-PowerShellLiteral $incoming) -Destination $(Quote-PowerShellLiteral $remoteRunner); Write-Output ('remote_runner_sha256=' + (Get-FileHash -LiteralPath $(Quote-PowerShellLiteral $remoteRunner) -Algorithm SHA256).Hash.ToLowerInvariant())" "activate-$($Version.Label)"
    $actual = Get-Field $probe.Stdout 'remote_runner_sha256'
    if ($actual -cne $Version.Hash) { throw "stable remote runner does not match $($Version.Label)" }
    $script:remoteActiveHash = $actual
}

function Invoke-LocalCli {
    param([object] $Version, [string[]] $Arguments, [string] $Description, [int] $TimeoutSeconds = 300)
    Set-LocalVersion $Version
    return Invoke-CapturedProcess $localRunner $Arguments $Description $TimeoutSeconds
}

function Invoke-RemoteCli {
    param([object] $Version, [string[]] $Arguments, [string] $Description, [int] $TimeoutSeconds = 300)
    Set-RemoteVersion $Version
    $command = @('& ' + (Quote-PowerShellLiteral $remoteRunner))
    $command += $Arguments | ForEach-Object { Quote-PowerShellLiteral ([string]$_) }
    $scriptText = (($command -join ' ') + '; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }')
    return Invoke-RemoteScript $scriptText $Description $TimeoutSeconds
}

function Get-Field {
    param([string] $Text, [string] $Name)
    $matches = @([regex]::Matches($Text, "(?m)^$([regex]::Escape($Name))=([^`r`n]*)`r?$") | ForEach-Object { $_.Groups[1].Value })
    if ($matches.Count -eq 0) { throw "required field is missing: $Name" }
    return [string]$matches[-1]
}

function Get-OptionalFields {
    param([string] $Text, [string] $Name)
    return @([regex]::Matches($Text, "(?m)^$([regex]::Escape($Name))=([^`r`n]*)`r?$") | ForEach-Object { $_.Groups[1].Value })
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

function Assert-LegacySelection {
    param([string[]] $Outputs, [string] $Context)
    $joined = $Outputs -join "`n"
    if ($joined -cmatch '(?m)^sync_transport_selected_alpn=kilogram/m1/sync/cbor/2$') {
        throw "$Context selected canonical sync with an N-1 peer"
    }
    foreach ($value in @(Get-OptionalFields $joined 'sync_transport_selected_alpn')) {
        if ($value -cne 'kilogram/m0/sync/8') { throw "$Context reported an unexpected sync ALPN: $value" }
    }
    foreach ($value in @(Get-OptionalFields $joined 'sync_transport_selection_evidence_id')) {
        if ($value -cne 'none') { throw "$Context created canonical selection evidence with an N-1 peer" }
    }
    foreach ($value in @(Get-OptionalFields $joined 'sync_transport_fallback_after_selection')) {
        if ($value -cne 'false') { throw "$Context reported fallback after canonical selection" }
    }
}

function Initialize-Pair {
    param([string] $Name, [object] $ResponderVersion, [object] $InitiatorVersion)
    $directory = Join-Path $output $Name
    $bob = Join-Path $directory 'bob'
    $alicePublic = Join-Path $directory 'alice-public'
    New-Item -ItemType Directory -Path $directory, $bob, $alicePublic | Out-Null
    $remoteScenario = "$remoteRoot/$Name"
    $aliceAccount = "$remoteScenario/alice/account"
    $aliceState = "$remoteScenario/alice/state"
    $aliceCertificateRemote = "$remoteScenario/alice/device.cert"
    $aliceDevicesRemote = "$remoteScenario/alice/devices.list"
    $alicePoolRemote = "$remoteScenario/alice/prekeys.pool"
    $null = Invoke-RemoteScript "New-Item -ItemType Directory -Force -Path $(Quote-PowerShellLiteral "$remoteScenario/alice") | Out-Null" "$Name-remote-directory"
    $aliceAccountResult = Invoke-RemoteCli $InitiatorVersion @('account-create', '--account-dir', $aliceAccount) "$Name-alice-account"
    $aliceAccountId = Get-Field $aliceAccountResult.Stdout 'account_id'
    $null = Invoke-RemoteCli $InitiatorVersion @('device-enroll', '--account-dir', $aliceAccount, '--state-dir', $aliceState, '--certificate-file', $aliceCertificateRemote) "$Name-alice-enroll"
    $null = Invoke-RemoteCli $InitiatorVersion @('account-device-list', '--account-dir', $aliceAccount, '--device-certificate-file', $aliceCertificateRemote, '--device-list-file', $aliceDevicesRemote) "$Name-alice-devices"
    $null = Invoke-RemoteCli $InitiatorVersion @('ratchet-prekey-pool', '--state-dir', $aliceState, '--pool-file', $alicePoolRemote, '--count', '32') "$Name-alice-prekeys"
    $aliceCertificate = Join-Path $alicePublic 'device.cert'
    $aliceDevices = Join-Path $alicePublic 'devices.list'
    $alicePool = Join-Path $alicePublic 'prekeys.pool'
    Invoke-PscpDownload $aliceCertificateRemote $aliceCertificate "$Name-download-certificate"
    Invoke-PscpDownload $aliceDevicesRemote $aliceDevices "$Name-download-devices"
    Invoke-PscpDownload $alicePoolRemote $alicePool "$Name-download-prekeys"

    $bobAccount = Join-Path $bob 'account'
    $bobState = Join-Path $bob 'state'
    $bobCertificate = Join-Path $bob 'device.cert'
    $bobDevices = Join-Path $bob 'devices.list'
    $bobPool = Join-Path $bob 'prekeys.pool'
    $bobAccountResult = Invoke-LocalCli $ResponderVersion @('account-create', '--account-dir', $bobAccount) "$Name-bob-account"
    $bobAccountId = Get-Field $bobAccountResult.Stdout 'account_id'
    $null = Invoke-LocalCli $ResponderVersion @('device-enroll', '--account-dir', $bobAccount, '--state-dir', $bobState, '--certificate-file', $bobCertificate) "$Name-bob-enroll"
    $null = Invoke-LocalCli $ResponderVersion @('account-device-list', '--account-dir', $bobAccount, '--device-certificate-file', $bobCertificate, '--device-list-file', $bobDevices) "$Name-bob-devices"
    $null = Invoke-LocalCli $ResponderVersion @('ratchet-prekey-pool', '--state-dir', $bobState, '--pool-file', $bobPool, '--count', '32') "$Name-bob-prekeys"

    $conversation = "$RunId-$Name"
    $membership = Join-Path $directory 'conversation.membership'
    $membershipRemote = "$remoteScenario/conversation.membership"
    $null = Invoke-LocalCli $ResponderVersion @('conversation-create', '--account-dir', $bobAccount, '--conversation', $conversation, '--member-account', $bobAccountId, '--member-account', $aliceAccountId, '--membership-file', $membership) "$Name-conversation"
    $null = Invoke-LocalCli $ResponderVersion @('conversation-membership-install', '--state-dir', $bobState, '--membership-file', $membership) "$Name-bob-membership"
    Invoke-PscpUpload $membership $membershipRemote "$Name-upload-membership"
    $null = Invoke-RemoteCli $InitiatorVersion @('conversation-membership-install', '--state-dir', $aliceState, '--membership-file', $membershipRemote) "$Name-alice-membership"

    return [PSCustomObject]@{
        Name = $Name; Directory = $directory; RemoteScenario = $remoteScenario
        ResponderVersion = $ResponderVersion; InitiatorVersion = $InitiatorVersion
        BobAccount = $bobAccount; BobState = $bobState; BobAccountId = $bobAccountId
        BobDevices = $bobDevices; AliceState = $aliceState; AliceAccountId = $aliceAccountId
        AliceCertificate = $aliceCertificate; AliceDevices = $aliceDevices; AlicePool = $alicePool
        Conversation = $conversation
    }
}

function Copy-PairWithVersions {
    param([object] $Pair, [object] $ResponderVersion, [object] $InitiatorVersion)
    return [PSCustomObject]@{
        Name = $Pair.Name; Directory = $Pair.Directory; RemoteScenario = $Pair.RemoteScenario
        ResponderVersion = $ResponderVersion; InitiatorVersion = $InitiatorVersion
        BobAccount = $Pair.BobAccount; BobState = $Pair.BobState; BobAccountId = $Pair.BobAccountId
        BobDevices = $Pair.BobDevices; AliceState = $Pair.AliceState; AliceAccountId = $Pair.AliceAccountId
        AliceCertificate = $Pair.AliceCertificate; AliceDevices = $Pair.AliceDevices; AlicePool = $Pair.AlicePool
        Conversation = $Pair.Conversation
    }
}

function Start-LocalListener {
    param([object] $Pair, [string] $Ticket, [string] $Prefix, [string] $RoutePolicy)
    Set-LocalVersion $Pair.ResponderVersion
    $arguments = @(
        'listen', '--state-dir', $Pair.BobState, '--allow-account', $Pair.AliceAccountId,
        '--device-list-file', $Pair.BobDevices, '--ticket-file', $Ticket,
        '--route-policy', $RoutePolicy
    )
    if ($RoutePolicy -ceq 'relay-only') {
        $arguments += @('--relay-url', $RelayUrl, '--relay-wait-seconds', '30')
    }
    else {
        $arguments += @('--relay-wait-seconds', '0')
    }
    $started = Start-CapturedProcess $localRunner $arguments $Prefix
    $script:activeListener = $started
    return $started
}

function Wait-TicketFile {
    param([string] $Path, [object] $Listener, [int] $TimeoutSeconds = 90)
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $Listener.Process.Refresh()
        if ($Listener.Process.HasExited) {
            $result = Complete-CapturedProcess $Listener
            throw "listener exited before ticket publication: stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
        }
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = [IO.File]::ReadAllText($Path).Trim()
            if ($text.Length -gt 64) { return }
        }
        Start-Sleep -Milliseconds 200
    }
    Stop-CapturedProcess $Listener
    throw "listener did not publish ticket within $TimeoutSeconds seconds"
}

function Invoke-RemoteAgainstListener {
    param([object] $Pair, [string] $Prefix, [string] $RoutePolicy, [string[]] $RemoteArguments)
    $ticket = Join-Path $Pair.Directory "$Prefix.ticket"
    $remoteTicket = "$($Pair.RemoteScenario)/$Prefix.ticket"
    $listener = $null
    try {
        $listener = Start-LocalListener $Pair $ticket (Join-Path $Pair.Directory "$Prefix-listener") $RoutePolicy
        Wait-TicketFile $ticket $listener
        Invoke-PscpUpload $ticket $remoteTicket "$Prefix-upload-ticket"
        $client = Invoke-RemoteCli $Pair.InitiatorVersion ($RemoteArguments + @('--ticket-file', $remoteTicket, '--expect-account', $Pair.BobAccountId)) "$Prefix-client" 300
        $listenerResult = Wait-CapturedProcess $listener "$Prefix listener" 300
        $script:activeListener = $null
        $listener = $null
        return [PSCustomObject]@{ Client = $client; Listener = $listenerResult }
    }
    finally {
        Stop-CapturedProcess $listener
        if ($null -ne $listener) { $script:activeListener = $null }
    }
}

function Assert-Path {
    param([object] $Result, [string] $Expected, [string] $Context)
    Assert-Field $Result.Client.Stdout 'transport_path' $Expected
    Assert-Field $Result.Listener.Stdout 'transport_path' $Expected
    $clientAddress = Get-Field $Result.Client.Stdout 'transport_remote_address'
    $listenerAddress = Get-Field $Result.Listener.Stdout 'transport_remote_address'
    $prefix = if ($Expected -ceq 'relay') { 'relay:' } else { 'ip:' }
    if (-not $clientAddress.StartsWith($prefix, [StringComparison]::Ordinal) -or
        -not $listenerAddress.StartsWith($prefix, [StringComparison]::Ordinal)) {
        throw "$Context remote-address diagnostics do not match $Expected"
    }
}

function Assert-HistoryConvergence {
    param([object] $Pair, [string] $Prefix)
    $bob = Invoke-LocalCli $Pair.ResponderVersion @('history', '--state-dir', $Pair.BobState, '--conversation', $Pair.Conversation) "$Prefix-bob-history"
    $alice = Invoke-RemoteCli $Pair.InitiatorVersion @('history', '--state-dir', $Pair.AliceState, '--conversation', $Pair.Conversation) "$Prefix-alice-history"
    $bobIds = @(Get-HistoryEventIds $bob.Stdout)
    $aliceIds = @(Get-HistoryEventIds $alice.Stdout)
    if ($bobIds.Count -eq 0 -or ($bobIds -join ',') -cne ($aliceIds -join ',')) {
        throw "$Prefix histories differ across physical hosts"
    }
    $bobCount = [int](Get-Field $bob.Stdout 'event_count')
    $aliceCount = [int](Get-Field $alice.Stdout 'event_count')
    if ($bobCount -ne $aliceCount -or $bobCount -ne $bobIds.Count) { throw "$Prefix history counts differ" }
    $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($bobIds -join "`n")))).ToLowerInvariant()
    return [PSCustomObject]@{ Count = $bobCount; Digest = $digest }
}

function Invoke-Exchange {
    param([object] $Pair, [string] $Prefix, [ValidateSet('direct-only','relay-only')] [string] $RoutePolicy)
    $expectedPath = if ($RoutePolicy -ceq 'relay-only') { 'relay' } else { 'direct' }
    $delivery = Invoke-RemoteAgainstListener $Pair "$Prefix-delivery" $RoutePolicy @(
        'connect', '--state-dir', $Pair.AliceState, '--conversation', $Pair.Conversation,
        '--message', "M1.1.91-$RunId-$Prefix"
    )
    Assert-Field $delivery.Client.Stdout 'status' 'acknowledged'
    Assert-Field $delivery.Listener.Stdout 'status' 'acknowledged'
    Assert-Path $delivery $expectedPath "$Prefix delivery"
    Assert-LegacySelection @($delivery.Client.Stdout, $delivery.Listener.Stdout) "$Prefix delivery"

    $null = Invoke-LocalCli $Pair.ResponderVersion @(
        'seed-history', '--state-dir', $Pair.BobState, '--conversation', $Pair.Conversation,
        '--count', '4', '--message-prefix', "M1.1.91-$RunId-$Prefix-seed",
        '--peer-certificate-file', $Pair.AliceCertificate,
        '--peer-device-list-file', $Pair.AliceDevices,
        '--peer-prekey-pool-file', $Pair.AlicePool
    ) "$Prefix-seed" 300

    $sync = Invoke-RemoteAgainstListener $Pair "$Prefix-sync" $RoutePolicy @(
        'sync', '--state-dir', $Pair.AliceState, '--conversation', $Pair.Conversation
    )
    Assert-Field $sync.Client.Stdout 'status' 'synchronized'
    Assert-Field $sync.Listener.Stdout 'status' 'synchronized'
    Assert-Path $sync $expectedPath "$Prefix sync"
    Assert-LegacySelection @($sync.Client.Stdout, $sync.Listener.Stdout) "$Prefix sync"
    $history = Assert-HistoryConvergence $Pair "$Prefix-final"
    return [ordered]@{
        name = $Prefix
        initiator_version = $Pair.InitiatorVersion.Label
        responder_version = $Pair.ResponderVersion.Label
        route_policy = $RoutePolicy
        selected_path = $expectedPath
        protocol = 'kilogram/m0/sync/8'
        delivery_acknowledged = $true
        listener_restart = $true
        synchronized = $true
        histories_equal = $true
        event_count = $history.Count
        inventory_digest = $history.Digest
        canonical_selected = $false
        fallback_after_selection = $false
    }
}

function Invoke-MatrixScenario {
    param([string] $Name, [object] $ResponderVersion, [object] $InitiatorVersion, [string] $RoutePolicy)
    $pair = Initialize-Pair $Name $ResponderVersion $InitiatorVersion
    return Invoke-Exchange $pair $Name $RoutePolicy
}

function Invoke-UpgradeChain {
    $pairPrevious = Initialize-Pair '05-forward-upgrade' $versions.'N-1' $versions.'N-1'
    $baseline = Invoke-Exchange $pairPrevious '05a-nminus1-baseline' 'direct-only'
    $responderUpgradedPair = Copy-PairWithVersions $pairPrevious $versions.N $versions.'N-1'
    $responderUpgrade = Invoke-Exchange $responderUpgradedPair '05b-responder-upgraded' 'direct-only'
    $fullyUpgradedPair = Copy-PairWithVersions $pairPrevious $versions.N $versions.N
    $initiatorUpgrade = Invoke-Exchange $fullyUpgradedPair '05c-initiator-upgraded' 'relay-only'
    return [ordered]@{
        name = '05-forward-upgrade'
        state_reused = $true
        forward_only = $true
        downgrade_after_upgrade_attempted = $false
        baseline = $baseline
        responder_upgrade = $responderUpgrade
        initiator_upgrade = $initiatorUpgrade
        final_histories_equal = $true
        final_event_count = $initiatorUpgrade.event_count
        final_inventory_digest = $initiatorUpgrade.inventory_digest
    }
}

try {
    if (-not (Test-NetConnection -ComputerName $RemoteHost -Port 22 -InformationLevel Quiet)) {
        throw "SSH is not reachable at ${RemoteHost}:22"
    }
    $preflight = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; New-Item -ItemType Directory -Force -Path $(Quote-PowerShellLiteral $remoteRunnerDirectory), $(Quote-PowerShellLiteral $remoteRoot) | Out-Null; Write-Output ('remote_hostname=' + [Environment]::MachineName)" 'remote-preflight'
    $remoteMachine = Get-Field $preflight.Stdout 'remote_hostname'
    if ($remoteMachine -ceq [Environment]::MachineName) { throw 'two-host interop requires distinct physical hosts' }

    $matrix = @(
        Invoke-MatrixScenario '01-current-to-previous-direct' $versions.'N-1' $versions.N 'direct-only'
        Invoke-MatrixScenario '02-previous-to-current-direct' $versions.N $versions.'N-1' 'direct-only'
        Invoke-MatrixScenario '03-current-to-previous-relay' $versions.'N-1' $versions.N 'relay-only'
        Invoke-MatrixScenario '04-previous-to-current-relay' $versions.N $versions.'N-1' 'relay-only'
    )
    $upgrade = Invoke-UpgradeChain
    $manifest = [ordered]@{
        schema_version = 1
        milestone = 'M1.1.91'
        status = 'canonical-sync-v2-n-minus-one-production-interop-verified'
        run_id = $RunId
        collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        accepted = [ordered]@{ tag = $PreviousTag; revision = $PreviousRevision; binary_sha256 = $versions.'N-1'.Hash; binary_bytes = $versions.'N-1'.Bytes }
        successor = [ordered]@{ revision = $CurrentRevision; binary_sha256 = $versions.N.Hash; binary_bytes = $versions.N.Bytes }
        build = [ordered]@{ profile = 'debug'; target = 'x86_64-pc-windows-msvc'; cargo_jobs = 2; release_build = $false; archive_created = $false }
        topology = [ordered]@{ controller_host = [Environment]::MachineName; remote_host = $remoteMachine; distinct_physical_hosts = $true }
        protocol = [ordered]@{ common_sync_alpn = 'kilogram/m0/sync/8'; canonical_sync_alpn = 'kilogram/m1/sync/cbor/2'; canonical_selected = $false; public_default = 'legacy-only' }
        matrix = $matrix
        upgrade_chain = $upgrade
        privacy = [ordered]@{ raw_logs_committed = $false; state_committed = $false; account_or_device_ids_exported = $false; message_bodies_exported = $false }
    }
    $manifestPath = Join-Path $output 'M1191-N-MINUS-ONE-EVIDENCE.json'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 14), [Text.UTF8Encoding]::new($false))
    Write-Output "evidence_manifest=$manifestPath"
    Write-Output 'status=canonical-sync-v2-n-minus-one-production-interop-verified'
}
finally {
    Stop-CapturedProcess $script:activeListener
    try {
        $null = Invoke-RemoteScript "Get-Process -Name 'kilogram-cli' -ErrorAction SilentlyContinue | Stop-Process -Force; exit 0" 'remote-final-cleanup' 30
    }
    catch { Write-Warning "remote cleanup could not be confirmed: $($_.Exception.Message)" }
}
