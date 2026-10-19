[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-DESKTOP-CEREMONY.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-MUTATION-ADAPTER.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$stateMachinePath = Join-Path $workspace 'apps\kilogram-windows\src\peer_reenrollment.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0241-m1217-guided-desktop-peer-reenrollment-ceremony.md'

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
        [object]$Predecessor,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Desktop,
        [string]$StateMachine,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.17' -or
        [string]$Manifest.status -cne 'canonical-runtime-ipc-peer-reenrollment-desktop-ceremony-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.16' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'desktop peer re-enrollment ceremony identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.16' -or
        [string]$Predecessor.status -cne 'canonical-runtime-ipc-peer-reenrollment-mutation-adapter-verified') {
        throw 'desktop ceremony predecessor changed'
    }

    $ipc = $Manifest.runtime_ipc
    if ([int]$ipc.version -ne 33 -or
        [string]$ipc.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        [string]$ipc.read_command -cne 'PeerReenrollmentRead' -or
        [string]$ipc.mutation_command -cne 'PeerReenrollmentMutation') {
        throw 'desktop ceremony runtime IPC identity changed'
    }
    foreach ($required in @(
        'signed_response_required', 'descriptor_binding_required',
        'fresh_request_id_per_operation'
    )) {
        if (-not [bool]$ipc.$required) { throw "runtime IPC property '$required' changed" }
    }

    $projection = $Manifest.desktop_projection
    foreach ($required in @(
        'opaque_contact_id', 'opaque_artifact_ids', 'bounded_replacement_fingerprint'
    )) {
        if (-not [bool]$projection.$required) { throw "desktop projection '$required' changed" }
    }
    foreach ($forbidden in @(
        'device_or_root_secret', 'filesystem_path', 'transport_ticket_or_endpoint',
        'raw_account_or_device_id', 'reusable_capability', 'message_or_history_body'
    )) {
        if ([bool]$projection.$forbidden) { throw "desktop projection exposed '$forbidden'" }
    }

    $ceremony = $Manifest.ceremony
    foreach ($required in @(
        'starts_from_signed_status', 'exact_staged_invitation_selection',
        'plan_is_separate', 'approval_requires_exact_typed_phrase',
        'approval_requires_separate_checkbox', 'accept_is_separate'
    )) {
        if (-not [bool]$ceremony.$required) { throw "ceremony property '$required' changed" }
    }
    foreach ($forbidden in @(
        'implicit_consent', 'automatic_click_or_retry', 'clear_quarantine_action',
        'automatic_history_rewrap'
    )) {
        if ([bool]$ceremony.$forbidden) { throw "ceremony gained forbidden property '$forbidden'" }
    }

    foreach ($required in @(
        'changed_contact', 'changed_signed_status', 'changed_plan',
        'changed_descriptor_bytes', 'runtime_disconnect_or_restart',
        'typed_phrase_and_checkbox_cleared', 'refresh_required_after_mutation_error'
    )) {
        if (-not [bool]$Manifest.invalidation.$required) {
            throw "invalidation property '$required' changed"
        }
    }

    $fresh = $Manifest.fresh_start
    foreach ($required in @(
        'acceptance_incomplete_is_not_success', 'acceptance_incomplete_restart_forbidden',
        'accepted_restart_required_is_not_consumed', 'desktop_owned_runtime_required',
        'clean_shutdown_requested', 'fresh_process_required',
        'exact_receipt_and_transition_verified', 'mismatch_stops_fresh_runtime',
        'bounded_exact_retry_after_failed_verification'
    )) {
        if (-not [bool]$fresh.$required) { throw "fresh-start property '$required' changed" }
    }
    if ([bool]$fresh.hot_apply) { throw 'desktop ceremony unexpectedly gained hot apply' }

    $verification = $Manifest.verification
    if ([int]$verification.pure_state_machine_tests -ne 5 -or
        -not [bool]$verification.authenticated_loopback_ceremony_test -or
        -not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$verification.m1_readiness -cne '143/143' -or
        [bool]$verification.second_host_required -or
        [bool]$verification.product_network_executed -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build) {
        throw 'desktop ceremony verification boundary changed'
    }

    if ([int]$Completion.active_postcard.version -ne 33 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        @($Completion.canonical.commands).Count -ne 38 -or
        @($Completion.canonical.results).Count -ne 37 -or
        @($Completion.active_postcard.commands).Count -ne 33 -or
        @($Completion.active_postcard.responses).Count -ne 32 -or
        @($Schema.semantic_rules).Count -ne 192) {
        throw 'desktop-only stage changed the canonical IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 33) {
        throw 'desktop ceremony compatibility binding changed'
    }

    Assert-ContainsAll $Desktop @(
        'mod peer_reenrollment;', 'PeerReenrollmentView',
        'Operation::PeerReenrollmentPlan', 'Operation::PeerReenrollmentApprove',
        'Operation::PeerReenrollmentAccept',
        'RuntimeIpcCommand::PeerReenrollmentMutation',
        'SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify',
        'start_guided_peer_reenrollment_restart',
        'abort_peer_reenrollment_fresh_runtime',
        'desktop_peer_reenrollment_ceremony_uses_authenticated_ipc_for_each_stage'
    ) 'desktop adapter'
    Assert-ContainsAll $StateMachine @(
        'pub(crate) struct PeerReenrollmentView',
        'pub(crate) enum PeerReenrollmentRestartState',
        'pub(crate) fn plan_command', 'pub(crate) fn approve_command',
        'pub(crate) fn accept_command', 'pub(crate) fn begin_guided_restart',
        'pub(crate) fn verify_fresh_response',
        'Type the exact displayed confirmation phrase',
        'Separate explicit approval is required',
        'The observed ceremony is not ready for planning',
        'plan_requires_exact_staged_invitation',
        'approval_requires_phrase_and_separate_confirmation',
        'changed_status_or_descriptor_invalidates_confirmation',
        'incomplete_acceptance_cannot_begin_restart',
        'guided_restart_verifies_exact_consumed_transition'
    ) 'desktop ceremony state machine'
    foreach ($forbidden in @(
        'use kilogram_identity', 'PathBuf', 'DeviceIdentity',
        'RuntimeIpcEndpoint', 'RuntimeIpcTicket', 'ClearQuarantine', 'HotApply'
    )) {
        if ($StateMachine.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "desktop ceremony state machine contains forbidden authority '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'status -> plan -> approve -> accept -> fresh start -> consumed',
        'never receives a Device or Root secret',
        'both an exact typed phrase and a separate checkbox',
        'AcceptanceIncomplete` is rendered as failure/incomplete state',
        'Hot apply is forbidden',
        'exact retained acceptance receipt and runtime transition IDs',
        'Aggregate readiness contains 143 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "desktop ceremony RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $desktopPath, $stateMachinePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$stateMachine = Get-Content -LiteralPath $stateMachinePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $predecessor, $completion, $schema, $compatibility, $desktop, $stateMachine, $rfc)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.version = 34
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.desktop_projection.device_or_root_secret = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.ceremony.approval_requires_separate_checkbox = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.ceremony.clear_quarantine_action = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.invalidation.changed_descriptor_bytes = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.fresh_start.acceptance_incomplete_restart_forbidden = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.fresh_start.hot_apply = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $desktop $stateMachine $rfc }
    $changedStateMachine = $stateMachine.Replace('pub(crate) fn verify_fresh_response', 'fn removed_verify_fresh_response')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $desktop $changedStateMachine $rfc }
    $changedStateMachine = "$stateMachine`nuse std::path::PathBuf;"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $desktop $changedStateMachine $rfc }
    $changedRfc = $rfc.Replace('Aggregate readiness contains 143 closed gates', 'Aggregate readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $desktop $stateMachine $changedRfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'negative mutation unexpectedly passed' }
        catch {
            if ($_.Exception.Message -ceq 'negative mutation unexpectedly passed') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'canonical_runtime_ipc_peer_reenrollment_desktop_ceremony_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'canonical_runtime_ipc_peer_reenrollment_desktop_ceremony_boundary=verified'
Write-Output 'active_runtime_ipc=postcard-v33-json-descriptor-v14'
Write-Output 'desktop_authority=authenticated-explicit-staged-only'
Write-Output 'guided_restart=exact-fresh-process-transition-verification'
Write-Output 'changes_active_wire=false'
Write-Output 'changes_public_default=false'
Write-Output 'public_default=legacy-only'
Write-Output 'second_host_required=false'
Write-Output 'product_network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'automatic_github_build=false'
