[CmdletBinding()]
param(
    [string] $ManifestPath,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$CandidateAlpn = 'kilogram/m1/sync/cbor/2'
$LegacyAlpn = 'kilogram/m0/sync/8'
$ReportProperties = @(
    'schema_version', 'source_revision', 'run_id', 'role', 'scenario', 'status',
    'protocol', 'route_policy', 'selected_path', 'remote_address', 'inventory_count',
    'inventory_digest', 'client_action', 'responder_action', 'canonical_selected',
    'legacy_dial_attempts', 'fallback_attempts', 'detail'
)

function Assert-ExactProperties {
    param(
        [Parameter(Mandatory)] [object] $Value,
        [Parameter(Mandatory)] [string[]] $Names,
        [Parameter(Mandatory)] [string] $Context
    )
    if ($null -eq $Value) { throw "$Context is missing" }
    $actual = @($Value.PSObject.Properties.Name | Sort-Object)
    $expected = @($Names | Sort-Object)
    if (($actual -join "`n") -cne ($expected -join "`n")) {
        throw "$Context properties differ; expected [$($expected -join ', ')], got [$($actual -join ', ')]"
    }
}

function Assert-Hex {
    param(
        [Parameter(Mandatory)] [string] $Value,
        [Parameter(Mandatory)] [int] $Length,
        [Parameter(Mandatory)] [string] $Context
    )
    if ($Value -cnotmatch "^[0-9a-f]{$Length}$") { throw "$Context is not exact lowercase hex" }
}

function Assert-Report {
    param(
        [Parameter(Mandatory)] [object] $Report,
        [Parameter(Mandatory)] [string] $Revision,
        [Parameter(Mandatory)] [string] $Role,
        [Parameter(Mandatory)] [string] $Scenario,
        [Parameter(Mandatory)] [string] $Status,
        [Parameter(Mandatory)] [string] $Protocol,
        [AllowNull()] [object] $Path,
        [Parameter(Mandatory)] [bool] $CanonicalSelected,
        [Parameter(Mandatory)] [int] $InventoryCount,
        [Parameter(Mandatory)] [string] $Context
    )
    Assert-ExactProperties $Report $ReportProperties $Context
    if ([int]$Report.schema_version -ne 1 -or
        [string]$Report.source_revision -cne $Revision -or
        [string]$Report.role -cne $Role -or
        [string]$Report.scenario -cne $Scenario -or
        [string]$Report.status -cne $Status -or
        [string]$Report.protocol -cne $Protocol -or
        [bool]$Report.canonical_selected -ne $CanonicalSelected -or
        [int]$Report.inventory_count -ne $InventoryCount) {
        throw "$Context identity, state or protocol does not match the evidence contract"
    }
    if ([string]$Report.run_id -cnotmatch '^[A-Za-z0-9_-]{1,96}$') {
        throw "$Context has an invalid run ID"
    }
    Assert-Hex ([string]$Report.inventory_digest) 64 "$Context inventory digest"
    if ([int64]$Report.legacy_dial_attempts -ne 0 -or [int64]$Report.fallback_attempts -ne 0) {
        throw "$Context attempted an unapproved legacy dial or fallback"
    }
    if ($null -eq $Path) {
        if ($null -ne $Report.selected_path -or $null -ne $Report.remote_address) {
            throw "$Context unexpectedly contains network path telemetry"
        }
    }
    else {
        if ([string]$Report.route_policy -notin @('direct-only', 'relay-only') -or
            [string]$Report.selected_path -cne $Path -or
            [string]::IsNullOrWhiteSpace([string]$Report.remote_address)) {
            throw "$Context does not prove the expected $Path route"
        }
    }
    if ([string]::IsNullOrWhiteSpace([string]$Report.client_action) -or
        [string]::IsNullOrWhiteSpace([string]$Report.responder_action) -or
        [string]::IsNullOrWhiteSpace([string]$Report.detail)) {
        throw "$Context contains an empty durable-state field"
    }
}

function Assert-ReportPair {
    param(
        [Parameter(Mandatory)] [object] $Pair,
        [Parameter(Mandatory)] [string] $Revision,
        [Parameter(Mandatory)] [string] $AliceScenario,
        [Parameter(Mandatory)] [string] $BobScenario,
        [Parameter(Mandatory)] [string] $Status,
        [Parameter(Mandatory)] [string] $Protocol,
        [AllowNull()] [string] $Path,
        [Parameter(Mandatory)] [bool] $CanonicalSelected,
        [Parameter(Mandatory)] [int] $AliceInventoryCount,
        [Parameter(Mandatory)] [int] $BobInventoryCount,
        [Parameter(Mandatory)] [bool] $RequireConvergence,
        [Parameter(Mandatory)] [string] $Context
    )
    Assert-ExactProperties $Pair @('alice', 'bob') $Context
    Assert-Report $Pair.alice $Revision 'alice' $AliceScenario $Status $Protocol $Path `
        $CanonicalSelected $AliceInventoryCount "$Context Alice"
    Assert-Report $Pair.bob $Revision 'bob' $BobScenario $Status $Protocol $Path `
        $CanonicalSelected $BobInventoryCount "$Context Bob"
    if ([string]$Pair.alice.run_id -cne [string]$Pair.bob.run_id) {
        throw "$Context roles do not share one run ID"
    }
    if ($RequireConvergence -and
        [string]$Pair.alice.inventory_digest -cne [string]$Pair.bob.inventory_digest) {
        throw "$Context histories did not converge"
    }
}

function Test-SyncV2NetworkEvidence {
    param([Parameter(Mandatory)] [string] $Path)

    $resolved = [IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "sync v2 evidence manifest is missing: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $item.Length -gt 2MB) {
        throw 'sync v2 evidence manifest must be one bounded regular file'
    }
    $text = (Get-Content -LiteralPath $resolved -Raw).Replace("`r`n", "`n")
    foreach ($forbidden in @(
        'local_device_secret', 'endpoint_secret', 'requester_authorization',
        'listener_authorization', 'membership', 'candidate_offer',
        'candidate_acceptance', 'evidence-role.json', 'message_body', 'plaintext'
    )) {
        if ($text.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "sync v2 evidence leaks or names forbidden material: $forbidden"
        }
    }
    $manifest = $text | ConvertFrom-Json
    Assert-ExactProperties $manifest @(
        'schema_version', 'milestone', 'evidence_kind', 'source_revision',
        'collected_at_utc', 'binary', 'topology', 'production_boundary', 'scenarios'
    ) 'manifest'
    if ([int]$manifest.schema_version -ne 1 -or [string]$manifest.milestone -cne 'M1.1.80') {
        throw 'sync v2 evidence schema or milestone is not exact'
    }
    $kind = [string]$manifest.evidence_kind
    if ($kind -notin @('hosted-windows', 'physical-windows')) {
        throw 'sync v2 evidence kind is unsupported'
    }
    $revision = [string]$manifest.source_revision
    Assert-Hex $revision 40 'source revision'
    try {
        $timestamp = $manifest.collected_at_utc
        if ($timestamp -is [DateTime]) {
            if ($timestamp.Kind -ne [DateTimeKind]::Utc) {
                throw 'parsed DateTime is not UTC'
            }
            $collected = [DateTimeOffset]::new($timestamp)
        }
        elseif ($timestamp -is [DateTimeOffset]) {
            $collected = $timestamp
        }
        else {
            $collected = [DateTimeOffset]::Parse(
                [string]$timestamp,
                [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::RoundtripKind
            )
        }
    }
    catch { throw 'sync v2 evidence timestamp is invalid' }
    if ($collected.Offset -ne [TimeSpan]::Zero) { throw 'sync v2 evidence timestamp must be UTC' }

    Assert-ExactProperties $manifest.binary @(
        'name', 'sha256', 'bytes', 'profile', 'target', 'feature'
    ) 'binary'
    if ([string]$manifest.binary.name -cne 'kilogram-cli.exe' -or
        [string]$manifest.binary.profile -cne 'debug' -or
        [string]$manifest.binary.target -cne 'x86_64-pc-windows-msvc' -or
        [string]$manifest.binary.feature -cne 'sync-v2-evidence' -or
        [int64]$manifest.binary.bytes -le 0) {
        throw 'sync v2 evidence binary identity is invalid'
    }
    Assert-Hex ([string]$manifest.binary.sha256) 64 'binary SHA-256'

    Assert-ExactProperties $manifest.topology @(
        'alice_host', 'bob_host', 'distinct_physical_hosts',
        'alice_binary_sha256', 'bob_binary_sha256'
    ) 'topology'
    $aliceHost = [string]$manifest.topology.alice_host
    $bobHost = [string]$manifest.topology.bob_host
    if ([string]::IsNullOrWhiteSpace($aliceHost) -or [string]::IsNullOrWhiteSpace($bobHost)) {
        throw 'sync v2 evidence host identity is empty'
    }
    foreach ($name in @('alice_binary_sha256', 'bob_binary_sha256')) {
        Assert-Hex ([string]$manifest.topology.$name) 64 "topology $name"
        if ([string]$manifest.topology.$name -cne [string]$manifest.binary.sha256) {
            throw "topology $name differs from the declared evidence binary"
        }
    }
    if ($kind -ceq 'physical-windows') {
        if (-not [bool]$manifest.topology.distinct_physical_hosts -or
            $aliceHost.Equals($bobHost, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'physical evidence does not prove two distinct host names'
        }
    }
    elseif ([bool]$manifest.topology.distinct_physical_hosts -or
        -not $aliceHost.Equals($bobHost, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'hosted evidence must identify one hosted runner'
    }

    Assert-ExactProperties $manifest.production_boundary @(
        'active_sync_alpn', 'candidate_sync_alpn', 'candidate_registered_by_default',
        'candidate_dialed_by_default', 'evidence_only',
        'legacy_fallback_after_selection', 'changes_active_wire'
    ) 'production boundary'
    $boundary = $manifest.production_boundary
    if ([string]$boundary.active_sync_alpn -cne $LegacyAlpn -or
        [string]$boundary.candidate_sync_alpn -cne $CandidateAlpn -or
        [bool]$boundary.candidate_registered_by_default -or
        [bool]$boundary.candidate_dialed_by_default -or
        -not [bool]$boundary.evidence_only -or
        [bool]$boundary.legacy_fallback_after_selection -or
        [bool]$boundary.changes_active_wire) {
        throw 'production transport boundary was weakened'
    }

    $scenarios = $manifest.scenarios
    Assert-ExactProperties $scenarios @(
        'direct', 'restart_resume', 'relay', 'mixed_version', 'post_selection_failure', 'rollback'
    ) 'scenarios'
    Assert-ReportPair $scenarios.direct $revision 'canonical-v2-dialer' 'canonical-v2-listener' `
        'synchronized' $CandidateAlpn 'direct' $true 2 2 $true 'direct scenario'
    if ([string]$scenarios.direct.alice.client_action -cne 'Complete' -or
        [string]$scenarios.direct.bob.responder_action -cne 'Complete') {
        throw 'direct scenario did not reach durable Complete on both active roles'
    }
    if ($kind -ceq 'physical-windows' -and
        (([string]$scenarios.direct.alice.remote_address).Contains('127.0.0.1') -or
         ([string]$scenarios.direct.bob.remote_address).Contains('127.0.0.1'))) {
        throw 'physical direct evidence unexpectedly used loopback'
    }

    Assert-ExactProperties $scenarios.restart_resume @('checkpoint', 'resumed') 'restart/resume scenario'
    Assert-ReportPair $scenarios.restart_resume.checkpoint $revision `
        'restart-checkpoint-dialer' 'restart-checkpoint-listener' 'checkpoint-stop' `
        $CandidateAlpn 'direct' $true 2 1 $false 'restart checkpoint'
    if ([string]$scenarios.restart_resume.checkpoint.alice.client_action -cne 'SendEventBatch' -or
        [string]$scenarios.restart_resume.checkpoint.bob.responder_action -cne 'ReceiveEventBatch') {
        throw 'restart checkpoint is not the intended durable inter-frame boundary'
    }
    Assert-ReportPair $scenarios.restart_resume.resumed $revision `
        'canonical-v2-dialer' 'canonical-v2-listener' 'synchronized' `
        $CandidateAlpn 'direct' $true 2 2 $true 'restart resume'

    Assert-ExactProperties $scenarios.relay @('status', 'reason', 'alice', 'bob') 'relay scenario'
    if ([string]$scenarios.relay.status -ceq 'verified') {
        if ($null -ne $scenarios.relay.reason) { throw 'verified relay scenario must not contain a reason' }
        Assert-Report $scenarios.relay.alice $revision 'alice' 'canonical-v2-dialer' `
            'synchronized' $CandidateAlpn 'relay' $true 2 'relay scenario Alice'
        Assert-Report $scenarios.relay.bob $revision 'bob' 'canonical-v2-listener' `
            'synchronized' $CandidateAlpn 'relay' $true 2 'relay scenario Bob'
        if ([string]$scenarios.relay.alice.run_id -cne [string]$scenarios.relay.bob.run_id -or
            [string]$scenarios.relay.alice.inventory_digest -cne [string]$scenarios.relay.bob.inventory_digest) {
            throw 'relay scenario roles did not share one converged run'
        }
    }
    elseif ([string]$scenarios.relay.status -ceq 'unavailable') {
        if ($null -ne $scenarios.relay.alice -or $null -ne $scenarios.relay.bob -or
            [string]::IsNullOrWhiteSpace([string]$scenarios.relay.reason) -or
            ([string]$scenarios.relay.reason).Length -gt 240) {
            throw 'unavailable relay scenario is not a bounded explicit observation'
        }
    }
    else { throw 'relay scenario status is unsupported' }

    Assert-ReportPair $scenarios.mixed_version $revision `
        'mixed-version-legacy-dialer' 'mixed-version-legacy-listener' 'legacy-agreed' `
        $LegacyAlpn 'direct' $false 1 1 $false 'mixed-version scenario'
    Assert-ExactProperties $scenarios.post_selection_failure @('alice', 'bob') 'post-selection failure'
    Assert-Report $scenarios.post_selection_failure.alice $revision 'alice' `
        'post-selection-corrupt-dialer' 'corrupt-frame-sent' $CandidateAlpn 'direct' $true 1 `
        'post-selection failure Alice'
    Assert-Report $scenarios.post_selection_failure.bob $revision 'bob' `
        'post-selection-corrupt-listener' 'failed-closed' $CandidateAlpn 'direct' $true 1 `
        'post-selection failure Bob'
    if ([string]$scenarios.post_selection_failure.alice.run_id -cne
        [string]$scenarios.post_selection_failure.bob.run_id) {
        throw 'post-selection failure roles do not share one run ID'
    }

    Assert-ExactProperties $scenarios.rollback @('alice', 'bob') 'rollback scenario'
    foreach ($entry in @(@('alice', $scenarios.rollback.alice), @('bob', $scenarios.rollback.bob))) {
        Assert-Report $entry[1] $revision $entry[0] 'explicit-one-step-rollback' `
            'rollback-verified' $LegacyAlpn $null $false 1 "rollback $($entry[0])"
    }

    $runIds = @(
        [string]$scenarios.direct.alice.run_id,
        [string]$scenarios.restart_resume.checkpoint.alice.run_id,
        [string]$scenarios.mixed_version.alice.run_id,
        [string]$scenarios.post_selection_failure.alice.run_id,
        [string]$scenarios.rollback.alice.run_id
    )
    if ([string]$scenarios.relay.status -ceq 'verified') {
        $runIds += [string]$scenarios.relay.alice.run_id
    }
    if (@($runIds | Sort-Object -Unique).Count -ne $runIds.Count) {
        throw 'independent evidence scenarios unexpectedly reuse a run ID'
    }

    [PSCustomObject]@{
        milestone = 'M1.1.80'
        evidence_kind = $kind
        source_revision = $revision
        binary_sha256 = [string]$manifest.binary.sha256
        direct_path = 'verified'
        restart_resume = 'verified'
        relay = [string]$scenarios.relay.status
        mixed_version = 'verified'
        post_selection_failure = 'failed-closed'
        rollback = 'verified'
        changes_active_wire = $false
        result = 'verified'
    }
}

function New-TestReport {
    param(
        [string] $Revision, [string] $RunId, [string] $Role, [string] $Scenario,
        [string] $Status, [string] $Protocol, [AllowNull()] [object] $Path,
        [bool] $Canonical, [int] $Count, [string] $Digest,
        [string] $ClientAction, [string] $ResponderAction
    )
    [ordered]@{
        schema_version = 1; source_revision = $Revision; run_id = $RunId; role = $Role
        scenario = $Scenario; status = $Status; protocol = $Protocol
        route_policy = if ($null -eq $Path) { $null } elseif ($Path -ceq 'direct') { 'direct-only' } else { 'relay-only' }
        selected_path = $Path; remote_address = if ($null -eq $Path) { $null } else { "$Path`:test" }
        inventory_count = $Count; inventory_digest = $Digest
        client_action = $ClientAction; responder_action = $ResponderAction
        canonical_selected = $Canonical; legacy_dial_attempts = 0; fallback_attempts = 0
        detail = 'bounded self-test detail'
    }
}

function New-TestManifest {
    $revision = 'ab' * 20
    $digest = 'cd' * 32
    $direct = [ordered]@{
        alice = New-TestReport $revision 'self-direct' 'alice' 'canonical-v2-dialer' 'synchronized' $CandidateAlpn 'direct' $true 2 $digest 'Complete' 'AwaitInventory'
        bob = New-TestReport $revision 'self-direct' 'bob' 'canonical-v2-listener' 'synchronized' $CandidateAlpn 'direct' $true 2 $digest 'SendInventory' 'Complete'
    }
    $checkpoint = [ordered]@{
        alice = New-TestReport $revision 'self-restart' 'alice' 'restart-checkpoint-dialer' 'checkpoint-stop' $CandidateAlpn 'direct' $true 2 ('11' * 32) 'SendEventBatch' 'ReceiveInventory'
        bob = New-TestReport $revision 'self-restart' 'bob' 'restart-checkpoint-listener' 'checkpoint-stop' $CandidateAlpn 'direct' $true 1 ('22' * 32) 'SendInventory' 'ReceiveEventBatch'
    }
    $resumed = [ordered]@{
        alice = New-TestReport $revision 'self-restart' 'alice' 'canonical-v2-dialer' 'synchronized' $CandidateAlpn 'direct' $true 2 $digest 'Complete' 'AwaitInventory'
        bob = New-TestReport $revision 'self-restart' 'bob' 'canonical-v2-listener' 'synchronized' $CandidateAlpn 'direct' $true 2 $digest 'SendInventory' 'Complete'
    }
    $legacy = [ordered]@{
        alice = New-TestReport $revision 'self-legacy' 'alice' 'mixed-version-legacy-dialer' 'legacy-agreed' $LegacyAlpn 'direct' $false 1 ('11' * 32) 'SendInventory' 'AwaitInventory'
        bob = New-TestReport $revision 'self-legacy' 'bob' 'mixed-version-legacy-listener' 'legacy-agreed' $LegacyAlpn 'direct' $false 1 ('22' * 32) 'SendInventory' 'AwaitInventory'
    }
    $corrupt = [ordered]@{
        alice = New-TestReport $revision 'self-corrupt' 'alice' 'post-selection-corrupt-dialer' 'corrupt-frame-sent' $CandidateAlpn 'direct' $true 1 ('11' * 32) 'SendInventory' 'AwaitInventory'
        bob = New-TestReport $revision 'self-corrupt' 'bob' 'post-selection-corrupt-listener' 'failed-closed' $CandidateAlpn 'direct' $true 1 ('22' * 32) 'SendInventory' 'AwaitInventory'
    }
    $rollback = [ordered]@{
        alice = New-TestReport $revision 'self-rollback' 'alice' 'explicit-one-step-rollback' 'rollback-verified' $LegacyAlpn $null $false 1 ('11' * 32) 'SendInventory' 'AwaitInventory'
        bob = New-TestReport $revision 'self-rollback' 'bob' 'explicit-one-step-rollback' 'rollback-verified' $LegacyAlpn $null $false 1 ('22' * 32) 'SendInventory' 'AwaitInventory'
    }
    [ordered]@{
        schema_version = 1; milestone = 'M1.1.80'; evidence_kind = 'hosted-windows'
        source_revision = $revision; collected_at_utc = '2026-10-18T00:00:00Z'
        binary = [ordered]@{ name = 'kilogram-cli.exe'; sha256 = 'ef' * 32; bytes = 123; profile = 'debug'; target = 'x86_64-pc-windows-msvc'; feature = 'sync-v2-evidence' }
        topology = [ordered]@{
            alice_host = 'runner'; bob_host = 'runner'; distinct_physical_hosts = $false
            alice_binary_sha256 = 'ef' * 32; bob_binary_sha256 = 'ef' * 32
        }
        production_boundary = [ordered]@{
            active_sync_alpn = $LegacyAlpn; candidate_sync_alpn = $CandidateAlpn
            candidate_registered_by_default = $false; candidate_dialed_by_default = $false
            evidence_only = $true; legacy_fallback_after_selection = $false; changes_active_wire = $false
        }
        scenarios = [ordered]@{
            direct = $direct; restart_resume = [ordered]@{ checkpoint = $checkpoint; resumed = $resumed }
            relay = [ordered]@{ status = 'unavailable'; reason = 'relay is not configured in bounded self-test'; alice = $null; bob = $null }
            mixed_version = $legacy; post_selection_failure = $corrupt; rollback = $rollback
        }
    }
}

if ($SelfTest) {
    $directory = Join-Path ([IO.Path]::GetTempPath()) ('kilogram-sync-v2-evidence-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $directory | Out-Null
    try {
        $path = Join-Path $directory 'manifest.json'
        $manifest = New-TestManifest
        $manifest | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding utf8
        $verified = Test-SyncV2NetworkEvidence $path
        if ($verified.result -cne 'verified') { throw 'positive evidence self-test did not verify' }

        $mutations = @(
            @{ name = 'wrong-revision'; apply = { param($m) $m.scenarios.direct.alice.source_revision = '00' * 20 } },
            @{ name = 'fallback'; apply = { param($m) $m.scenarios.post_selection_failure.bob.fallback_attempts = 1 } },
            @{ name = 'same-physical-host'; apply = { param($m) $m.evidence_kind = 'physical-windows'; $m.topology.distinct_physical_hosts = $true } },
            @{ name = 'secret-field'; apply = { param($m) $m.scenarios.direct.alice | Add-Member -NotePropertyName local_device_secret -NotePropertyValue ('00' * 32) } },
            @{ name = 'checkpoint-count'; apply = { param($m) $m.scenarios.restart_resume.checkpoint.alice.inventory_count = 1 } },
            @{ name = 'checkpoint-action'; apply = { param($m) $m.scenarios.restart_resume.checkpoint.alice.client_action = 'SendEvents' } },
            @{ name = 'rollback-telemetry'; apply = {
                param($m)
                $m.scenarios.rollback.alice.route_policy = 'direct-only'
                $m.scenarios.rollback.alice.selected_path = 'direct'
                $m.scenarios.rollback.alice.remote_address = 'direct:test'
            } }
        )
        foreach ($mutation in $mutations) {
            $candidate = (New-TestManifest | ConvertTo-Json -Depth 20) | ConvertFrom-Json
            & $mutation['apply'] $candidate
            $candidate | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding utf8
            $failedClosed = $false
            try { $null = Test-SyncV2NetworkEvidence $path }
            catch { $failedClosed = $true }
            if (-not $failedClosed) { throw "negative evidence self-test did not fail closed: $($mutation.name)" }
        }
        Write-Output 'sync_v2_network_evidence_self_test=verified'
        Write-Output "negative_mutations=$($mutations.Count)"
    }
    finally {
        if (Test-Path -LiteralPath $directory) {
            Remove-Item -LiteralPath $directory -Recurse -Force
        }
    }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    throw 'ManifestPath is required unless SelfTest is used'
}
Test-SyncV2NetworkEvidence $ManifestPath | Format-List
