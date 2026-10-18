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
        [Parameter(Mandatory)] [string] $LogPrefix,
        [hashtable] $Environment = @{}
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
    foreach ($entry in $Environment.GetEnumerator()) {
        $start.EnvironmentVariables[[string]$entry.Key] = [string]$entry.Value
    }
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
        [int] $TimeoutSeconds = 180,
        [switch] $AllowFailure
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
    if ($result.ExitCode -ne 0 -and -not $AllowFailure) {
        throw "$Description failed ($($result.ExitCode)): stdout=[$($result.Stdout)] stderr=[$($result.Stderr)]"
    }
    return $result
}

function Invoke-Kilogram {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPrefix,
        [int] $TimeoutSeconds = 180,
        [hashtable] $Environment = @{}
    )
    $started = Start-KilogramProcess $Arguments $LogPrefix $Environment
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
        [Parameter(Mandatory)] [string] $Prefix,
        [ValidateSet('normal', 'emergency-containment')]
        [string] $Disposition = 'normal',
        [string] $CanonicalEvidenceId = ''
    )
    $reason = "M1.1.84 local rehearsal transition to $State/$Disposition"
    $arguments = @(
        '--state-dir', $StateDirectory,
        '--migration-state', $State,
        '--disposition', $Disposition,
        '--reason', $reason
    )
    if ($CanonicalEvidenceId) {
        $arguments += @('--canonical-evidence-id', $CanonicalEvidenceId)
    }
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
        AliceAccount = $aliceAccount
        BobAccount = $bobAccount
        AliceAccountId = $aliceAccountId
        BobAccountId = $bobAccountId
        AliceCertificate = $aliceCertificate
        BobCertificate = $bobCertificate
        AliceDevices = $aliceDevices
        BobDevices = $bobDevices
        AlicePool = $alicePool
        BobPool = $bobPool
        Membership = $membership
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

function Assert-HistoryConvergence {
    param(
        [Parameter(Mandatory)] [object] $Pair,
        [Parameter(Mandatory)] [string] $Prefix,
        [string] $Conversation = ''
    )
    if (-not $Conversation) { $Conversation = $Pair.Conversation }
    $aliceHistory = Invoke-Kilogram @(
        'history', '--state-dir', $Pair.AliceState, '--conversation', $Conversation
    ) "$Prefix-alice-history"
    $bobHistory = Invoke-Kilogram @(
        'history', '--state-dir', $Pair.BobState, '--conversation', $Conversation
    ) "$Prefix-bob-history"
    $aliceIds = @(Get-HistoryEventIds $aliceHistory.Stdout)
    $bobIds = @(Get-HistoryEventIds $bobHistory.Stdout)
    if ($aliceIds.Count -eq 0 -or ($aliceIds -join ',') -cne ($bobIds -join ',')) {
        throw "$Prefix histories did not converge exactly"
    }
    return [PSCustomObject]@{
        EventCount = $aliceIds.Count
        Digest = ($aliceIds -join ',')
    }
}

function Start-BobRuntime {
    param(
        [Parameter(Mandatory)] [object] $Pair,
        [Parameter(Mandatory)] [string] $Ticket,
        [Parameter(Mandatory)] [string] $Prefix,
        [int] $MaxSessions = 1,
        [hashtable] $Environment = @{}
    )
    return Start-KilogramProcess @(
        'runtime', '--state-dir', $Pair.BobState,
        '--allow-account', $Pair.AliceAccountId,
        '--device-list-file', $Pair.BobDevices,
        '--ticket-file', $Ticket,
        '--route-policy', 'direct-only',
        '--relay-wait-seconds', '0',
        '--max-sessions', ([string]$MaxSessions),
        '--idle-seconds', '120',
        '--poll-milliseconds', '5000',
        '--auto-sync-seconds', '0'
    ) $Prefix $Environment
}

function Invoke-AliceSync {
    param(
        [Parameter(Mandatory)] [object] $Pair,
        [Parameter(Mandatory)] [string] $Ticket,
        [Parameter(Mandatory)] [string] $Prefix,
        [string] $Conversation = '',
        [hashtable] $Environment = @{}
    )
    if (-not $Conversation) { $Conversation = $Pair.Conversation }
    return Invoke-Kilogram @(
        'sync', '--state-dir', $Pair.AliceState, '--ticket-file', $Ticket,
        '--conversation', $Conversation, '--expect-account', $Pair.BobAccountId
    ) $Prefix 300 $Environment
}

function Assert-CanonicalSelection {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [string] $ExpectedEvidence = ''
    )
    Assert-Field $Text 'sync_transport_selected_alpn' 'kilogram/m1/sync/cbor/2'
    Assert-Field $Text 'sync_transport_fallback_after_selection' 'false'
    $evidence = Get-Field $Text 'sync_transport_selection_evidence_id'
    if ($evidence -cnotmatch '^[0-9a-f]{64}$') {
        throw "canonical selection evidence is malformed: $evidence"
    }
    if ($ExpectedEvidence -and $evidence -cne $ExpectedEvidence) {
        throw "canonical selection changed across the bounded attempt: expected $ExpectedEvidence, got $evidence"
    }
    return $evidence
}

function New-AdditionalConversation {
    param(
        [Parameter(Mandatory)] [object] $Pair,
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $Prefix
    )
    $membership = "$Prefix-membership.snapshot"
    $bobPool = "$Prefix-bob-prekeys.pool"
    $null = Invoke-Kilogram @(
        'conversation-create', '--account-dir', $Pair.AliceAccount, '--conversation', $Name,
        '--member-account', $Pair.AliceAccountId, '--member-account', $Pair.BobAccountId,
        '--membership-file', $membership
    ) "$Prefix-create"
    $null = Invoke-Kilogram @(
        'conversation-membership-install', '--state-dir', $Pair.AliceState,
        '--membership-file', $membership
    ) "$Prefix-alice-install"
    $null = Invoke-Kilogram @(
        'conversation-membership-install', '--state-dir', $Pair.BobState,
        '--membership-file', $membership
    ) "$Prefix-bob-install"
    $null = Invoke-Kilogram @(
        'ratchet-prekey-pool', '--state-dir', $Pair.BobState,
        '--pool-file', $bobPool, '--count', '16'
    ) "$Prefix-bob-prekeys"
    $null = Invoke-Kilogram @(
        'seed-history', '--state-dir', $Pair.AliceState, '--conversation', $Name,
        '--count', '4', '--message-prefix', "M1.1.84-containment-$RunId",
        '--peer-certificate-file', $Pair.BobCertificate,
        '--peer-device-list-file', $Pair.BobDevices,
        '--peer-prekey-pool-file', $bobPool
    ) "$Prefix-seed" 300
    return $membership
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
    $history = Assert-HistoryConvergence $pair (Join-Path $directory '10')
    return [ordered]@{
        name = $Name
        policy = $Policy
        selected_alpn = $expectedAlpn
        event_count = $history.EventCount
        histories_equal = $true
        fallback_after_selection = $false
        selection_evidence_id = if ($Policy -ceq 'optin-optin') {
            Get-Field $sync.Stdout 'sync_transport_selection_evidence_id'
        } else { $null }
    }
}

function Invoke-RestartResumeScenario {
    $directory = Join-Path $output '04-restart-resume'
    $pair = Initialize-Pair $directory
    Enable-CanonicalListener $pair.BobState (Join-Path $directory 'bob-activation')
    Enable-CanonicalInitiator $pair.AliceState (Join-Path $directory 'alice-activation')

    $firstTicket = Join-Path $directory 'bob-first.ticket'
    $firstRuntime = $null
    try {
        $firstRuntime = Start-BobRuntime $pair $firstTicket (Join-Path $directory '08-bob-first-runtime') 2
        Wait-TicketFile $firstTicket $firstRuntime
        $interrupted = Invoke-AliceSync $pair $firstTicket (Join-Path $directory '09-alice-interrupted') '' @{
            KILOGRAM_TEST_STOP_CANONICAL_SYNC_V2_AFTER_DIFF_ONCE = '1'
        }
        $firstRuntimeResult = Wait-KilogramProcess $firstRuntime 'Bob first restart/resume runtime' 300
        $firstRuntime = $null
    }
    finally {
        Stop-KilogramProcess $firstRuntime
    }
    Assert-Field $interrupted.Stdout 'runtime_test_fault_status' 'canonical-sync-v2-stopped-after-diff'
    Assert-Field $interrupted.Stdout 'status' 'canonical-sync-v2-interrupted-after-durable-diff'
    $evidence = Assert-CanonicalSelection $interrupted.Stdout
    if ($firstRuntimeResult.Stderr -cnotmatch '(?m)^runtime_canonical_sync_v2_session_status=failed ') {
        throw 'responder did not record the intentionally interrupted selected canonical session'
    }

    $secondTicket = Join-Path $directory 'bob-second.ticket'
    $secondRuntime = $null
    try {
        $secondRuntime = Start-BobRuntime $pair $secondTicket (Join-Path $directory '10-bob-second-runtime') 1
        Wait-TicketFile $secondTicket $secondRuntime
        $resumed = Invoke-AliceSync $pair $secondTicket (Join-Path $directory '11-alice-resumed')
        $secondRuntimeResult = Wait-KilogramProcess $secondRuntime 'Bob resumed runtime' 300
        $secondRuntime = $null
    }
    finally {
        Stop-KilogramProcess $secondRuntime
    }
    Assert-Field $resumed.Stdout 'status' 'synchronized'
    $null = Assert-CanonicalSelection $resumed.Stdout $evidence
    Assert-Field $secondRuntimeResult.Stdout 'runtime_canonical_sync_v2_session_status' 'completed'
    $history = Assert-HistoryConvergence $pair (Join-Path $directory '12')
    return [ordered]@{
        name = '04-restart-resume'
        selected_alpn = 'kilogram/m1/sync/cbor/2'
        interruption_after_durable_diff = $true
        process_restart = $true
        exact_selection_resumed = $true
        selection_evidence_id = $evidence
        event_count = $history.EventCount
        histories_equal = $true
        duplicate_or_loss = $false
        fallback_after_selection = $false
    }
}

function Invoke-SelectedLossScenario {
    $directory = Join-Path $output '05-selected-loss-fail-closed'
    $pair = Initialize-Pair $directory
    Enable-CanonicalListener $pair.BobState (Join-Path $directory 'bob-activation')
    Enable-CanonicalInitiator $pair.AliceState (Join-Path $directory 'alice-activation')
    $ticket = Join-Path $directory 'bob.ticket'
    $runtime = $null
    try {
        $runtime = Start-BobRuntime $pair $ticket (Join-Path $directory '08-bob-runtime') 2 @{
            KILOGRAM_TEST_STOP_CANONICAL_SYNC_V2_AFTER_DIFF_ONCE = '1'
        }
        Wait-TicketFile $ticket $runtime
        $syncProcess = Start-KilogramProcess @(
            'sync', '--state-dir', $pair.AliceState, '--ticket-file', $ticket,
            '--conversation', $pair.Conversation, '--expect-account', $pair.BobAccountId
        ) (Join-Path $directory '09-alice-sync')
        $failedSync = Wait-KilogramProcess $syncProcess 'Alice selected-loss sync' 300 -AllowFailure
        $runtimeResult = Wait-KilogramProcess $runtime 'Bob selected-loss runtime' 300
        $runtime = $null
    }
    finally {
        Stop-KilogramProcess $runtime
    }
    if ($failedSync.ExitCode -eq 0) {
        throw 'selected canonical connection loss unexpectedly succeeded'
    }
    $evidence = Assert-CanonicalSelection $failedSync.Stdout
    if ($failedSync.Stdout -cmatch '(?m)^status=synchronized$') {
        throw 'selected canonical connection loss reported a false synchronized status'
    }
    Assert-Field $runtimeResult.Stdout 'runtime_test_fault_status' 'canonical-sync-v2-stopped-after-diff'
    if ($runtimeResult.Stderr -cnotmatch '(?m)^runtime_canonical_sync_v2_session_status=failed ') {
        throw 'selected canonical responder loss did not fail its canonical session'
    }
    return [ordered]@{
        name = '05-selected-loss-fail-closed'
        selected_alpn = 'kilogram/m1/sync/cbor/2'
        selection_evidence_id = $evidence
        client_exit_code = $failedSync.ExitCode
        synchronized = $false
        legacy_retry = $false
        fallback_after_selection = $false
    }
}

function Invoke-PreSelectionRollbackScenario {
    $directory = Join-Path $output '06-pre-selection-rollback'
    $pair = Initialize-Pair $directory
    Enable-CanonicalInitiator $pair.AliceState (Join-Path $directory 'alice-activation')
    Apply-ActivationState $pair.AliceState 'dual-reader-prefer-legacy' (Join-Path $directory 'alice-rollback')
    $status = Invoke-Kilogram @(
        'canonical-sync-policy-status', '--state-dir', $pair.AliceState
    ) (Join-Path $directory '08-status')
    Assert-Field $status.Stdout 'activation_migration_epoch' '4'
    Assert-Field $status.Stdout 'activation_migration_state' 'dual-reader-prefer-legacy'
    Assert-Field $status.Stdout 'activation_disposition' 'normal'
    Assert-Field $status.Stdout 'activation_canonical_evidence_id' 'none'
    Assert-Field $status.Stdout 'activation_initiate_new_canonical_session' 'false'
    return [ordered]@{
        name = '06-pre-selection-rollback'
        from = 'dual-reader-prefer-canonical'
        to = 'dual-reader-prefer-legacy'
        migration_epoch = 4
        canonical_evidence = 'none'
        rollback = 'explicit-one-step'
    }
}

function Invoke-ContainmentScenario {
    $directory = Join-Path $output '07-post-selection-containment'
    $pair = Initialize-Pair $directory
    Enable-CanonicalListener $pair.BobState (Join-Path $directory 'bob-activation')
    Enable-CanonicalInitiator $pair.AliceState (Join-Path $directory 'alice-activation')

    $initialTicket = Join-Path $directory 'bob-initial.ticket'
    $initialRuntime = $null
    try {
        $initialRuntime = Start-BobRuntime $pair $initialTicket (Join-Path $directory '08-bob-initial-runtime') 2
        Wait-TicketFile $initialTicket $initialRuntime
        $initialSync = Invoke-AliceSync $pair $initialTicket (Join-Path $directory '09-alice-initial-sync')
        $initialRuntimeResult = Wait-KilogramProcess $initialRuntime 'Bob pre-containment runtime' 300
        $initialRuntime = $null
    }
    finally {
        Stop-KilogramProcess $initialRuntime
    }
    Assert-Field $initialSync.Stdout 'status' 'synchronized'
    $evidence = Assert-CanonicalSelection $initialSync.Stdout
    $null = Assert-CanonicalSelection $initialRuntimeResult.Stdout $evidence

    Apply-ActivationState $pair.AliceState 'dual-reader-prefer-canonical' (Join-Path $directory 'alice-evidence') 'normal' $evidence
    Apply-ActivationState $pair.BobState 'dual-reader-prefer-legacy' (Join-Path $directory 'bob-evidence') 'normal' $evidence
    Apply-ActivationState $pair.AliceState 'dual-reader-prefer-canonical' (Join-Path $directory 'alice-containment') 'emergency-containment' $evidence
    Apply-ActivationState $pair.BobState 'dual-reader-prefer-legacy' (Join-Path $directory 'bob-containment') 'emergency-containment' $evidence

    $resumeTicket = Join-Path $directory 'bob-resume.ticket'
    $resumeRuntime = $null
    try {
        $resumeRuntime = Start-BobRuntime $pair $resumeTicket (Join-Path $directory '10-bob-resume-runtime') 1
        Wait-TicketFile $resumeTicket $resumeRuntime
        $resumed = Invoke-AliceSync $pair $resumeTicket (Join-Path $directory '11-alice-resume')
        $resumeRuntimeResult = Wait-KilogramProcess $resumeRuntime 'Bob contained resume runtime' 300
        $resumeRuntime = $null
    }
    finally {
        Stop-KilogramProcess $resumeRuntime
    }
    Assert-Field $resumed.Stdout 'status' 'synchronized'
    $null = Assert-CanonicalSelection $resumed.Stdout $evidence
    Assert-Field $resumed.Stdout 'canonical_sync_v2_containment' 'true'
    Assert-Field $resumed.Stdout 'canonical_sync_v2_capability_published' 'false'
    Assert-Field $resumed.Stdout 'canonical_sync_v2_new_initiate_enabled' 'false'
    Assert-Field $resumed.Stdout 'canonical_sync_v2_selected_resume_enabled' 'true'
    Assert-Field $resumeRuntimeResult.Stdout 'canonical_sync_v2_containment' 'true'
    Assert-Field $resumeRuntimeResult.Stdout 'canonical_sync_v2_capability_published' 'false'
    Assert-Field $resumeRuntimeResult.Stdout 'runtime_canonical_sync_v2_session_status' 'completed'
    $existingHistory = Assert-HistoryConvergence $pair (Join-Path $directory '12-existing')

    $newConversation = "$RunId-containment-new-conversation"
    $null = New-AdditionalConversation $pair $newConversation (Join-Path $directory '13-new-conversation')
    $newTicket = Join-Path $directory 'bob-new-conversation.ticket'
    $newRuntime = $null
    try {
        $newRuntime = Start-BobRuntime $pair $newTicket (Join-Path $directory '14-bob-new-runtime') 1
        Wait-TicketFile $newTicket $newRuntime
        $newSync = Invoke-AliceSync $pair $newTicket (Join-Path $directory '15-alice-new-sync') $newConversation
        $newRuntimeResult = Wait-KilogramProcess $newRuntime 'Bob contained new-conversation runtime' 300
        $newRuntime = $null
    }
    finally {
        Stop-KilogramProcess $newRuntime
    }
    Assert-Field $newSync.Stdout 'status' 'synchronized'
    Assert-Field $newSync.Stdout 'sync_transport_selected_alpn' 'kilogram/m0/sync/8'
    Assert-Field $newSync.Stdout 'sync_transport_selection_evidence_id' 'none'
    Assert-Field $newSync.Stdout 'canonical_sync_v2_containment' 'true'
    Assert-Field $newRuntimeResult.Stdout 'canonical_sync_v2_capability_published' 'false'
    $newHistory = Assert-HistoryConvergence $pair (Join-Path $directory '16-new') $newConversation

    return [ordered]@{
        name = '07-post-selection-containment'
        selection_evidence_id = $evidence
        retained_existing_selection = $true
        existing_selected_alpn = 'kilogram/m1/sync/cbor/2'
        existing_event_count = $existingHistory.EventCount
        existing_histories_equal = $true
        new_canonical_attempts_enabled = $false
        new_conversation_selected_alpn = 'kilogram/m0/sync/8'
        new_conversation_event_count = $newHistory.EventCount
        new_conversation_histories_equal = $true
        fallback_after_selection = $false
    }
}

$binaryItem = Get-Item -LiteralPath $binary
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$scenarios = @(
    Invoke-Scenario '01-default-default' 'default-default'
    Invoke-Scenario '02-optin-default' 'optin-default'
    Invoke-Scenario '03-optin-optin' 'optin-optin'
    Invoke-RestartResumeScenario
    Invoke-SelectedLossScenario
    Invoke-PreSelectionRollbackScenario
    Invoke-ContainmentScenario
)
$manifest = [ordered]@{
    schema_version = 1
    milestone = 'M1.1.84'
    status = 'canonical-sync-v2-local-resilience-rehearsal-verified'
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
    test_faults = [ordered]@{
        debug_build_only = $true
        release_available = $false
        environment_authority = $false
    }
    scenarios = $scenarios
}
$manifestPath = Join-Path $output 'M1184-LOCAL-EVIDENCE.json'
[IO.File]::WriteAllText(
    $manifestPath,
    ($manifest | ConvertTo-Json -Depth 8),
    [Text.UTF8Encoding]::new($false)
)
Write-Output "evidence_manifest=$manifestPath"
Write-Output 'status=canonical-sync-v2-local-resilience-rehearsal-verified'
