[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-MUTATION-ADAPTER.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$contractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\peer_reenrollment_contract.rs'
$planPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment\src\lib.rs'
$acceptancePath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-acceptance\src\lib.rs'
$runtimePath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-runtime\src\lib.rs'
$ceremonyPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-runtime\src\ceremony.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0240-m1216-fresh-start-peer-reenrollment-mutation-adapter.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Ipc,
        [string]$Contract,
        [string]$Plan,
        [string]$Acceptance,
        [string]$Runtime,
        [string]$Ceremony,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.16' -or
        [string]$Manifest.status -cne 'canonical-runtime-ipc-peer-reenrollment-mutation-adapter-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.14' -or
        [string]$Manifest.read_adapter_source -cne 'M1.2.15' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        -not [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer re-enrollment mutation adapter identity or scope changed'
    }

    $runtimeIpc = $Manifest.runtime_ipc
    if ([string]$runtimeIpc.command -cne 'PeerReenrollmentMutation' -or
        [string]$runtimeIpc.response -cne 'PeerReenrollmentMutation' -or
        (@($runtimeIpc.allowed_nested_commands) -join ',') -cne 'plan,approve,accept' -or
        (@($runtimeIpc.rejected_nested_commands) -join ',') -cne 'inspect,status' -or
        [int]$runtimeIpc.version -ne 33 -or
        [string]$runtimeIpc.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        [int]$runtimeIpc.previous_version -ne 32 -or
        [string]$runtimeIpc.previous_descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v13\0' -or
        [string]$runtimeIpc.migration -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [bool]$runtimeIpc.dual_reader -or
        [int]$runtimeIpc.maximum_nested_request_bytes -ne 4096 -or
        [int]$runtimeIpc.maximum_signed_response_bytes -ne 16384) {
        throw 'peer re-enrollment mutation runtime IPC boundary changed'
    }

    $authority = $Manifest.authority
    foreach ($required in @(
        'mutation_runs_inside_active_runtime', 'exact_contact_binding',
        'exact_staged_invitation_binding', 'exact_quarantine_record_binding',
        'exact_plan_binding', 'exact_tombstone_binding'
    )) {
        if (-not [bool]$authority.$required) { throw "authority property '$required' changed" }
    }
    foreach ($forbidden in @(
        'client_receives_device_secret', 'desktop_mutation_authority',
        'remote_mutation_authority', 'implicit_approval'
    )) {
        if ([bool]$authority.$forbidden) { throw "authority gained forbidden property '$forbidden'" }
    }
    if ([string]$authority.typed_confirmation_prefix -cne 'REENROLL' -or
        [string]$authority.explicit_approval -cne 'UserConfirmed') {
        throw 'explicit approval contract changed'
    }

    $journal = $Manifest.durable_ceremony
    if ([string]$journal.store -cne 'peer-reenrollment-runtime-ceremony-v1.redb' -or
        (@($journal.phases) -join ',') -cne 'planned,approved,acceptance-committed,complete' -or
        [int]$journal.maximum_records -ne 4096) {
        throw 'peer re-enrollment ceremony shape changed'
    }
    foreach ($required in @(
        'owner_bound_to_account_and_current_device', 'records_signed_by_current_device',
        'canonical_reencode_required', 'monotonic_phase_progression',
        'exact_replay_idempotent', 'divergent_replay_fails_closed',
        'acceptance_receipt_id_bound', 'runtime_transition_id_bound',
        'incomplete_state_observable'
    )) {
        if (-not [bool]$journal.$required) { throw "ceremony property '$required' changed" }
    }

    $freshStart = $Manifest.fresh_start
    foreach ($required in @(
        'restart_required_only_after_complete_journal', 'incomplete_journal_skipped_at_startup',
        'predecessor_contact_retained_until_complete', 'predecessor_quarantine_retained',
        'old_readable_evidence_retained', 'queued_messages_rebound_only_at_fresh_start',
        'history_recovery_separate'
    )) {
        if (-not [bool]$freshStart.$required) { throw "fresh-start property '$required' changed" }
    }
    if ([bool]$freshStart.hot_apply) { throw 'peer re-enrollment unexpectedly gained hot apply' }

    $cliBoundary = $Manifest.cli
    if ([string]$cliBoundary.plan_command -cne 'runtime-ipc-peer-reenrollment-plan' -or
        [string]$cliBoundary.approve_command -cne 'runtime-ipc-peer-reenrollment-approve' -or
        [string]$cliBoundary.accept_command -cne 'runtime-ipc-peer-reenrollment-accept' -or
        -not [bool]$cliBoundary.approve_flag_required -or
        -not [bool]$cliBoundary.exact_confirmation_phrase_required -or
        -not [bool]$cliBoundary.uses_authenticated_runtime_descriptor) {
        throw 'peer re-enrollment CLI mutation boundary changed'
    }
    foreach ($forbidden in @(
        'plan_action_present', 'approve_action_present', 'accept_action_present',
        'clear_action_present', 'retry_action_present', 'hot_apply_action_present',
        'automatic_recovery_present'
    )) {
        if ([bool]$Manifest.desktop.$forbidden) { throw "desktop gained forbidden authority '$forbidden'" }
    }

    if ([int]$Completion.active_postcard.version -ne 33 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        @($Completion.canonical.commands).Count -ne 38 -or
        @($Completion.canonical.results).Count -ne 37 -or
        @($Completion.canonical.command_result_pairs).Count -ne 38 -or
        @($Completion.active_postcard.commands).Count -ne 33 -or
        @($Completion.active_postcard.responses).Count -ne 32 -or
        'peer-reenrollment-mutation' -cnotin @($Completion.canonical.commands) -or
        'peer-reenrollment-mutation-result' -cnotin @($Completion.canonical.results) -or
        'PeerReenrollmentMutation' -cnotin @($Completion.active_postcard.commands.variant) -or
        'PeerReenrollmentMutation' -cnotin @($Completion.active_postcard.responses.variant)) {
        throw 'peer re-enrollment mutation completion inventory changed'
    }
    $pair = @($Completion.canonical.command_result_pairs | Where-Object {
        [string]$_.command -ceq 'peer-reenrollment-mutation'
    })
    if ($pair.Count -ne 1 -or
        [string]$pair[0].result -cne 'peer-reenrollment-mutation-result' -or
        [string]$pair[0].module -cne 'peer-reenrollment' -or
        [string]$pair[0].kind -cne 'mutation') {
        throw 'peer re-enrollment mutation command/result pair changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard-v33-with-json-descriptor-v14' -or
        @($Schema.covered_commands).Count -ne 38 -or
        @($Schema.covered_results).Count -ne 37 -or
        @($Schema.semantic_rules).Count -ne 192 -or
        'peer-reenrollment-mutation' -cnotin @($Schema.covered_commands) -or
        'peer-reenrollment-mutation-result' -cnotin @($Schema.covered_results)) {
        throw 'peer re-enrollment mutation canonical schema changed'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 33 -or
        [string](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v14\0') {
        throw 'peer re-enrollment compatibility binding changed'
    }

    Assert-ContainsAll $Ipc @(
        'const IPC_VERSION: u8 = 33;', 'kilogram:runtime-ipc-descriptor:v14\0',
        'PeerReenrollmentMutation {', 'version_32_request_is_rejected_before_actor_dispatch'
    ) 'runtime IPC source'
    Assert-ContainsAll $Contract @(
        'RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION: u8 = 2',
        'RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed',
        'RuntimeIpcPeerReenrollmentStatus::AcceptanceIncomplete',
        'RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceIncomplete'
    ) 'nested peer re-enrollment contract'
    Assert-ContainsAll $Plan @(
        'pub fn verify_context(', 'pub fn local_device_id(&self) -> DeviceId',
        'pub fn prepared_at_unix_seconds(&self) -> u64'
    ) 'peer re-enrollment plan source'
    Assert-ContainsAll $Acceptance @(
        'pub fn matches_tombstone(', 'pub fn verify_bound_context('
    ) 'peer re-enrollment acceptance source'
    Assert-ContainsAll $Runtime @(
        'mod ceremony;', 'pub use ceremony::*;',
        'fn same_preparation_as(&self, other: &Self) -> bool',
        'existing.transition.same_preparation_as(&candidate)'
    ) 'peer re-enrollment runtime source'
    Assert-ContainsAll $Ceremony @(
        'peer-reenrollment-runtime-ceremony-v1.redb',
        'pub enum PeerReenrollmentRuntimeCeremonyPhase', 'AcceptanceCommitted',
        'pub fn prepare(', 'pub fn approve(', 'pub fn mark_acceptance_committed(',
        'pub fn complete(', 'PeerReenrollmentRuntimeCeremonyWriteOutcome::AlreadyPresent'
    ) 'durable ceremony journal'
    Assert-ContainsAll $Cli @(
        'RuntimeIpcPeerReenrollmentPlan {', 'RuntimeIpcPeerReenrollmentApprove {',
        'RuntimeIpcPeerReenrollmentAccept {', 'fn execute_runtime_peer_reenrollment_mutation(',
        'RuntimeIpcCommand::PeerReenrollmentMutation { request }',
        'RuntimePeerReenrollmentMutationFault::AfterAcceptanceCommit',
        'RuntimePeerReenrollmentMutationFault::AfterAcceptanceJournal',
        'RuntimePeerReenrollmentMutationFault::AfterTransitionPrepare',
        'peer_reenrollment_consumes_only_at_fresh_start_and_retains_old_evidence'
    ) 'CLI runtime mutation adapter'
    if ($Desktop.Contains('RuntimeIpcCommand::PeerReenrollmentMutation')) {
        throw 'desktop unexpectedly gained peer re-enrollment mutation authority'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'version 32 / descriptor domain v13',
        'version 33 / domain v14',
        'plan`, `approve`, and `accept`',
        'acceptance-committed',
        'fresh runtime start',
        'old readable evidence remains retained',
        'desktop remains read-only',
        'Aggregate readiness contains 142 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment mutation adapter RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $completionPath, $schemaPath, $compatibilityPath, $ipcPath,
    $contractPath, $planPath, $acceptancePath, $runtimePath, $ceremonyPath,
    $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$contract = Get-Content -LiteralPath $contractPath -Raw
$plan = Get-Content -LiteralPath $planPath -Raw
$acceptance = Get-Content -LiteralPath $acceptancePath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ceremony = Get-Content -LiteralPath $ceremonyPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

$arguments = @(
    $manifest, $completion, $schema, $compatibility, $ipc, $contract, $plan,
    $acceptance, $runtime, $ceremony, $cli, $desktop, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.version = 32
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.allowed_nested_commands = @('inspect', 'plan', 'approve', 'accept')
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.authority.implicit_approval = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.authority.client_receives_device_secret = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.durable_ceremony.exact_replay_idempotent = $false
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.fresh_start.hot_apply = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.desktop.accept_action_present = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changedCompletion = Copy-JsonObject $completion
    $changedCompletion.active_postcard.version = 32
    $mutations += { Assert-Contract $manifest $changedCompletion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changedSchema = Copy-JsonObject $schema
    $changedSchema.covered_commands = @($changedSchema.covered_commands | Where-Object { $_ -cne 'peer-reenrollment-mutation' })
    $mutations += { Assert-Contract $manifest $completion $changedSchema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $desktop $rfc }
    $changedDesktop = $desktop + "`nRuntimeIpcCommand::PeerReenrollmentMutation"
    $mutations += { Assert-Contract $manifest $completion $schema $compatibility $ipc $contract $plan $acceptance $runtime $ceremony $cli $changedDesktop $rfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne 10) { throw "peer re-enrollment mutation adapter mutations rejected $rejected/10" }
    Write-Output 'negative_mutations=10/10'
}

Write-Output 'canonical_runtime_ipc_peer_reenrollment_mutation_adapter_boundary=verified'
Write-Output 'active_runtime_ipc_version=33'
Write-Output 'mutation_authority=current-device-runtime-only'
Write-Output 'fresh_start_consumption=true'
Write-Output 'network_executed=false'
Write-Output 'second_host_required=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
