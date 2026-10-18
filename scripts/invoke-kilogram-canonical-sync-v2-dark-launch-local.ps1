[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $BinaryPath,
    [Parameter(Mandatory)] [string] $OutputDirectory,
    [string] $SourceRevision = '',
    [string] $RunId = ('m1184-local-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($RunId -cnotmatch '^[A-Za-z0-9_-]{1,80}$') {
    throw 'RunId contains unsupported characters'
}
if ($SourceRevision -and $SourceRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw 'SourceRevision must be an exact lowercase Git commit ID'
}
$binary = [IO.Path]::GetFullPath($BinaryPath)
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw "kilogram-cli binary is missing: $binary"
}
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) {
    throw "dark-launch evidence directory already exists: $output"
}
New-Item -ItemType Directory -Path $output | Out-Null

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

function Start-KilogramProcess {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPrefix
    )
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $binary
    $start.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-NativeArgument ([string]$_)
    }) -join ' ')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) { throw "failed to start $binary" }
    return [PSCustomObject]@{
        Process = $process
        StdoutTask = $process.StandardOutput.ReadToEndAsync()
        StderrTask = $process.StandardError.ReadToEndAsync()
        StdoutPath = "$LogPrefix.stdout.log"
        StderrPath = "$LogPrefix.stderr.log"
        Completed = $false
    }
}

function Complete-KilogramProcess {
    param([Parameter(Mandatory)] [object] $Started)
    if ($Started.Completed) { throw 'process result was already collected' }
    $Started.Process.WaitForExit()
    $stdout = $Started.StdoutTask.Result
    $stderr = $Started.StderrTask.Result
    [IO.File]::WriteAllText($Started.StdoutPath, $stdout, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($Started.StderrPath, $stderr, [Text.UTF8Encoding]::new($false))
    $Started.Completed = $true
    return [PSCustomObject]@{
        ExitCode = [int]$Started.Process.ExitCode
        Stdout = $stdout
        Stderr = $stderr
    }
}

function Stop-KilogramProcess {
    param([AllowNull()] [object] $Started)
    if ($null -eq $Started -or $Started.Completed) { return }
    if (-not $Started.Process.HasExited) {
        Stop-Process -Id $Started.Process.Id -Force -ErrorAction SilentlyContinue
        [void]$Started.Process.WaitForExit(5000)
    }
    if ($Started.Process.HasExited) { $null = Complete-KilogramProcess $Started }
}

function Wait-KilogramProcess {
    param(
        [Parameter(Mandatory)] [object] $Started,
        [Parameter(Mandatory)] [string] $Description,
        [int] $TimeoutSeconds = 180
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $Started.Process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 200
        $Started.Process.Refresh()
    }
    if (-not $Started.Process.HasExited) {
        Stop-KilogramProcess $Started
        throw "$Description timed out after $TimeoutSeconds seconds"
    }
    $result = Complete-KilogramProcess $Started
    if ($result.ExitCode -ne 0) {
        throw "$Description failed ($($result.ExitCode)): stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
    }
    return $result
}

function Invoke-Kilogram {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPrefix,
        [int] $TimeoutSeconds = 180
    )
    $started = Start-KilogramProcess $Arguments $LogPrefix
    return Wait-KilogramProcess $started ($Arguments -join ' ') $TimeoutSeconds
}

function Get-Field {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Name
    )
    $matches = @([regex]::Matches($Text, "(?m)^$([regex]::Escape($Name))=([^`r`n]*)$") | ForEach-Object {
        $_.Groups[1].Value
    })
    if ($matches.Count -eq 0) { throw "required field is missing: $Name" }
    return [string]$matches[-1]
}

function Assert-Field {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $Expected
    )
    $actual = Get-Field $Text $Name
    if ($actual -cne $Expected) { throw "$Name expected [$Expected], got [$actual]" }
}

function Wait-TicketFile {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [object] $Runtime,
        [int] $TimeoutSeconds = 90
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($Runtime.Process.HasExited) {
            $result = Complete-KilogramProcess $Runtime
            throw "runtime exited before ticket publication: stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
        }
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = [IO.File]::ReadAllText($Path).Trim()
            if ($text.Length -gt 64) { return }
        }
        Start-Sleep -Milliseconds 200
        $Runtime.Process.Refresh()
    }
    Stop-KilogramProcess $Runtime
    throw "runtime did not publish ticket within $TimeoutSeconds seconds"
}

function Apply-ActivationState {
    param(
        [Parameter(Mandatory)] [string] $StateDirectory,
        [Parameter(Mandatory)] [string] $State,
        [Parameter(Mandatory)] [string] $Prefix
    )
    $reason = "M1.1.84 local rehearsal transition to $State"
    $arguments = @(
        '--state-dir', $StateDirectory,
        '--migration-state', $State,
        '--reason', $reason
    )
    $plan = Invoke-Kilogram (@('canonical-sync-policy-plan') + $arguments) "$Prefix-plan"
    Assert-Field $plan.Stdout 'activation_plan_writes_state' 'false'
    $recordId = Get-Field $plan.Stdout 'activation_plan_record_id'
    $apply = Invoke-Kilogram (@('canonical-sync-policy-apply') + $arguments + @(
        '--expected-record-id', $recordId
    )) "$Prefix-apply"
    Assert-Field $apply.Stdout 'status' 'canonical-sync-v2-activation-applied'
}

function Enable-CanonicalListener {
    param([string] $StateDirectory, [string] $Prefix)
    Apply-ActivationState $StateDirectory 'advertise-cbor-v2' "$Prefix-1"
    Apply-ActivationState $StateDirectory 'dual-reader-prefer-legacy' "$Prefix-2"
}

function Enable-CanonicalInitiator {
    param([string] $StateDirectory, [string] $Prefix)
    Apply-ActivationState $StateDirectory 'advertise-cbor-v2' "$Prefix-1"
    Apply-ActivationState $StateDirectory 'dual-reader-prefer-legacy' "$Prefix-2"
    Apply-ActivationState $StateDirectory 'dual-reader-prefer-canonical' "$Prefix-3"
}

function Initialize-Pair {
    param([Parameter(Mandatory)] [string] $ScenarioDirectory)
    New-Item -ItemType Directory -Path $ScenarioDirectory | Out-Null
    $alice = Join-Path $ScenarioDirectory 'alice'
    $bob = Join-Path $ScenarioDirectory 'bob'
    New-Item -ItemType Directory -Path $alice | Out-Null
    New-Item -ItemType Directory -Path $bob | Out-Null
    $aliceAccount = Join-Path $alice 'account'
    $bobAccount = Join-Path $bob 'account'
    $aliceState = Join-Path $alice 'state'
    $bobState = Join-Path $bob 'state'
    $aliceCertificate = Join-Path $alice 'device.cert'
    $bobCertificate = Join-Path $bob 'device.cert'
    $aliceDevices = Join-Path $alice 'devices.list'
    $bobDevices = Join-Path $bob 'devices.list'
    $alicePool = Join-Path $alice 'prekeys.pool'
    $bobPool = Join-Path $bob 'prekeys.pool'
    $membership = Join-Path $ScenarioDirectory 'conversation.membership'
    $conversation = "$RunId-$([IO.Path]::GetFileName($ScenarioDirectory))"

    $aliceAccountResult = Invoke-Kilogram @('account-create', '--account-dir', $aliceAccount) (Join-Path $alice '01-account')
    $bobAccountResult = Invoke-Kilogram @('account-create', '--account-dir', $bobAccount) (Join-Path $bob '01-account')
    $aliceAccountId = Get-Field $aliceAccountResult.Stdout 'account_id'
    $bobAccountId = Get-Field $bobAccountResult.Stdout 'account_id'
    $null = Invoke-Kilogram @(
        'device-enroll', '--account-dir', $aliceAccount, '--state-dir', $aliceState,
        '--certificate-file', $aliceCertificate
    ) (Join-Path $alice '02-enroll')
    $null = Invoke-Kilogram @(
        'device-enroll', '--account-dir', $bobAccount, '--state-dir', $bobState,
        '--certificate-file', $bobCertificate
    ) (Join-Path $bob '02-enroll')
    $null = Invoke-Kilogram @(
        'account-device-list', '--account-dir', $aliceAccount,
        '--device-certificate-file', $aliceCertificate, '--device-list-file', $aliceDevices
    ) (Join-Path $alice '03-devices')
    $null = Invoke-Kilogram @(
        'account-device-list', '--account-dir', $bobAccount,
        '--device-certificate-file', $bobCertificate, '--device-list-file', $bobDevices
    ) (Join-Path $bob '03-devices')
    $null = Invoke-Kilogram @(
        'ratchet-prekey-pool', '--state-dir', $aliceState, '--pool-file', $alicePool, '--count', '16'
    ) (Join-Path $alice '04-prekeys')
    $null = Invoke-Kilogram @(
        'ratchet-prekey-pool', '--state-dir', $bobState, '--pool-file', $bobPool, '--count', '16'
    ) (Join-Path $bob '04-prekeys')
    $null = Invoke-Kilogram @(
        'conversation-create', '--account-dir', $aliceAccount, '--conversation', $conversation,
        '--member-account', $aliceAccountId, '--member-account', $bobAccountId,
        '--membership-file', $membership
    ) (Join-Path $ScenarioDirectory '05-conversation')
    $null = Invoke-Kilogram @(
        'conversation-membership-install', '--state-dir', $aliceState,
        '--membership-file', $membership
    ) (Join-Path $alice '06-membership')
    $null = Invoke-Kilogram @(
        'conversation-membership-install', '--state-dir', $bobState,
        '--membership-file', $membership
    ) (Join-Path $bob '06-membership')
    $null = Invoke-Kilogram @(
        'seed-history', '--state-dir', $aliceState, '--conversation', $conversation,
        '--count', '6', '--message-prefix', "M1.1.84-$RunId",
        '--peer-certificate-file', $bobCertificate,
        '--peer-device-list-file', $bobDevices,
        '--peer-prekey-pool-file', $bobPool
    ) (Join-Path $alice '07-seed') 300

    return [PSCustomObject]@{
        AliceState = $aliceState
        BobState = $bobState
        AliceAccountId = $aliceAccountId
        BobAccountId = $bobAccountId
        BobDevices = $bobDevices
        Conversation = $conversation
        ScenarioDirectory = $ScenarioDirectory
    }
}

function Get-HistoryEventIds {
    param([Parameter(Mandatory)] [string] $Text)
    return @([regex]::Matches($Text, '(?m)^event_id=([0-9a-f]{64}) ') | ForEach-Object {
        $_.Groups[1].Value
    } | Sort-Object)
}

function Invoke-Scenario {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [ValidateSet('default-default', 'optin-default', 'optin-optin')]
        [string] $Policy
    )
    $directory = Join-Path $output $Name
    $pair = Initialize-Pair $directory
    switch ($Policy) {
        'optin-default' {
            Enable-CanonicalListener $pair.BobState (Join-Path $directory 'bob-activation')
        }
        'optin-optin' {
            Enable-CanonicalListener $pair.BobState (Join-Path $directory 'bob-activation')
            Enable-CanonicalInitiator $pair.AliceState (Join-Path $directory 'alice-activation')
        }
    }
    $ticket = Join-Path $directory 'bob.ticket'
    $runtimeArguments = @(
        'runtime', '--state-dir', $pair.BobState,
        '--allow-account', $pair.AliceAccountId,
        '--device-list-file', $pair.BobDevices,
        '--ticket-file', $ticket,
        '--route-policy', 'direct-only',
        '--relay-wait-seconds', '0',
        '--max-sessions', $(if ($Policy -ceq 'optin-optin') { '2' } else { '1' }),
        '--idle-seconds', '120',
        '--poll-milliseconds', '5000',
        '--auto-sync-seconds', '0'
    )
    $runtime = $null
    try {
        $runtime = Start-KilogramProcess $runtimeArguments (Join-Path $directory '08-bob-runtime')
        Wait-TicketFile $ticket $runtime
        $sync = Invoke-Kilogram @(
            'sync', '--state-dir', $pair.AliceState, '--ticket-file', $ticket,
            '--conversation', $pair.Conversation, '--expect-account', $pair.BobAccountId
        ) (Join-Path $directory '09-alice-sync') 300
        $runtimeResult = Wait-KilogramProcess $runtime 'Bob runtime' 300
        $runtime = $null
    }
    finally {
        Stop-KilogramProcess $runtime
    }
    $expectedAlpn = if ($Policy -ceq 'optin-optin') {
        'kilogram/m1/sync/cbor/2'
    }
    else {
        'kilogram/m0/sync/8'
    }
    Assert-Field $sync.Stdout 'sync_transport_selected_alpn' $expectedAlpn
    Assert-Field $sync.Stdout 'sync_transport_fallback_after_selection' 'false'
    Assert-Field $sync.Stdout 'status' 'synchronized'
    if ($Policy -ceq 'default-default') {
        Assert-Field $sync.Stdout 'canonical_sync_v2_policy_state' 'legacy-only'
        Assert-Field $runtimeResult.Stdout 'canonical_sync_v2_capability_published' 'false'
    }
    elseif ($Policy -ceq 'optin-default') {
        Assert-Field $sync.Stdout 'canonical_sync_v2_policy_state' 'legacy-only'
        Assert-Field $runtimeResult.Stdout 'canonical_sync_v2_capability_published' 'true'
    }
    else {
        Assert-Field $sync.Stdout 'canonical_sync_v2_policy_state' 'dual-reader-prefer-canonical'
        Assert-Field $runtimeResult.Stdout 'canonical_sync_v2_policy_state' 'dual-reader-prefer-legacy'
        Assert-Field $runtimeResult.Stdout 'runtime_canonical_sync_v2_session_status' 'completed'
        $aliceEvidence = Get-Field $sync.Stdout 'sync_transport_selection_evidence_id'
        $bobEvidence = Get-Field $runtimeResult.Stdout 'sync_transport_selection_evidence_id'
        if ($aliceEvidence -cne $bobEvidence -or $aliceEvidence -cnotmatch '^[0-9a-f]{64}$') {
            throw 'canonical selection evidence differs between roles'
        }
    }
    $aliceHistory = Invoke-Kilogram @(
        'history', '--state-dir', $pair.AliceState, '--conversation', $pair.Conversation
    ) (Join-Path $directory '10-alice-history')
    $bobHistory = Invoke-Kilogram @(
        'history', '--state-dir', $pair.BobState, '--conversation', $pair.Conversation
    ) (Join-Path $directory '10-bob-history')
    $aliceIds = Get-HistoryEventIds $aliceHistory.Stdout
    $bobIds = Get-HistoryEventIds $bobHistory.Stdout
    if ($aliceIds.Count -eq 0 -or ($aliceIds -join ',') -cne ($bobIds -join ',')) {
        throw "$Name histories did not converge exactly"
    }
    return [ordered]@{
        name = $Name
        policy = $Policy
        selected_alpn = $expectedAlpn
        event_count = $aliceIds.Count
        histories_equal = $true
        fallback_after_selection = $false
        selection_evidence_id = if ($Policy -ceq 'optin-optin') {
            Get-Field $sync.Stdout 'sync_transport_selection_evidence_id'
        } else { $null }
    }
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$scenarios = @(
    Invoke-Scenario '01-default-default' 'default-default'
    Invoke-Scenario '02-optin-default' 'optin-default'
    Invoke-Scenario '03-optin-optin' 'optin-optin'
)
$manifest = [ordered]@{
    schema_version = 1
    milestone = 'M1.1.84'
    status = 'canonical-sync-v2-local-dark-launch-rehearsal-verified'
    source_revision = if ($SourceRevision) { $SourceRevision } else { $null }
    run_id = $RunId
    collected_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    host = [Environment]::MachineName
    binary = [ordered]@{
        name = 'kilogram-cli.exe'
        sha256 = $binaryHash
        bytes = [int64]$binaryItem.Length
        profile = 'debug'
    }
    public_default = 'legacy-only'
    scenarios = $scenarios
}
$manifestPath = Join-Path $output 'M1184-LOCAL-EVIDENCE.json'
[IO.File]::WriteAllText(
    $manifestPath,
    ($manifest | ConvertTo-Json -Depth 8),
    [Text.UTF8Encoding]::new($false)
)
Write-Output "evidence_manifest=$manifestPath"
Write-Output 'status=canonical-sync-v2-local-dark-launch-rehearsal-verified'
