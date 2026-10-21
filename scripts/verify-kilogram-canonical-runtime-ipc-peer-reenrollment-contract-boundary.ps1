[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-CONTRACT.json'
$adapterManifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-READ-ADAPTER.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\Cargo.toml'
$libPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$contractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\peer_reenrollment_contract.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0238-m1214-canonical-runtime-ipc-peer-reenrollment-contract.md'

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
        [object]$AdapterManifest,
        [string]$CrateManifest,
        [string]$Lib,
        [string]$Contract,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.14' -or
        [string]$Manifest.status -cne 'canonical-runtime-ipc-peer-reenrollment-contract-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'runtime IPC peer re-enrollment contract identity or scope changed'
    }

    $contractBoundary = $Manifest.contract
    if ([int]$contractBoundary.version -ne 1 -or
        [int]$contractBoundary.maximum_request_bytes -ne 4096 -or
        [int]$contractBoundary.maximum_signed_response_bytes -ne 16384 -or
        [int]$contractBoundary.maximum_process_local_queue_count -ne 4096 -or
        [string]$contractBoundary.canonical_codec -cne 'postcard-reencode-exact' -or
        [int]$contractBoundary.active_runtime_ipc_version_retained -ne 31 -or
        [bool]$contractBoundary.active_runtime_command_variant_added -or
        [bool]$contractBoundary.active_adapter_added -or
        [bool]$contractBoundary.desktop_control_added) {
        throw 'runtime IPC peer re-enrollment codec or dormant boundary changed'
    }

    if ((@($Manifest.ceremony.stages) -join ',') -cne 'inspect,plan,approve,accept,status' -or
        -not [bool]$Manifest.ceremony.explicit_approval_required -or
        -not [bool]$Manifest.ceremony.exact_confirmation_phrase_required -or
        -not [bool]$Manifest.ceremony.stage_skip_rejected -or
        [bool]$Manifest.ceremony.implicit_consent_allowed -or
        [bool]$Manifest.ceremony.automatic_quarantine_clear_allowed -or
        [bool]$Manifest.ceremony.hot_apply_allowed) {
        throw 'peer re-enrollment ceremony separation changed'
    }

    foreach ($property in @(
        'opaque_contact_id', 'verified_invitation_summary', 'exact_artifact_ids',
        'confirmation_phrase'
    )) {
        if (-not [bool]$Manifest.privacy_projection.$property) {
            throw "privacy projection property '$property' changed"
        }
    }
    foreach ($property in @(
        'raw_account_id_exposed', 'raw_device_id_exposed',
        'root_or_device_secret_exposed', 'ticket_or_endpoint_exposed',
        'queue_or_message_body_exposed', 'capability_bytes_exposed'
    )) {
        if ([bool]$Manifest.privacy_projection.$property) {
            throw "privacy projection leaked '$property'"
        }
    }

    foreach ($property in @(
        'request_digest', 'exact_runtime_descriptor', 'current_descriptor_device',
        'opaque_contact', 'predecessor_quarantine_summary',
        'verified_successor_invitation_summary', 'durable_outcome',
        'domain_separated_device_signature'
    )) {
        if (-not [bool]$Manifest.signed_response_binding.$property) {
            throw "signed response binding '$property' changed"
        }
    }

    if ((@($Manifest.states) -join ',') -cne 'missing-invitation,unsafe-or-divergent,ready,approval-required,approved-prepared,accepted-restart-required,consumed') {
        throw 'peer re-enrollment status vocabulary changed'
    }

    foreach ($property in @(
        'durable_quarantine_distinct_from_process_local_suppression',
        'restart_required_is_explicit', 'retained_quarantine_is_not_reported_as_cleared',
        'unsafe_or_divergent_context_is_fail_closed'
    )) {
        if (-not [bool]$Manifest.authority_boundary.$property) {
            throw "authority boundary '$property' changed"
        }
    }

    foreach ($property in @(
        'peer_reenrollment_restores_future_communication',
        'old_readable_history_is_not_implicitly_recovered',
        'separate_same_account_history_rewrap_approval_required',
        'empty_rewrap_inventory_cannot_be_reported_complete'
    )) {
        if (-not [bool]$Manifest.history_boundary.$property) {
            throw "history boundary '$property' changed"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.targeted_contract_tests -cne '8/8' -or
        [string]$verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$verification.m1_readiness -cne '140/140' -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'runtime IPC peer re-enrollment verification claim changed'
    }

    if ([string]$AdapterManifest.milestone -cne 'M1.2.15' -or
        [string]$AdapterManifest.contract_source -cne 'M1.2.14' -or
        (@($AdapterManifest.runtime_ipc.allowed_nested_commands) -join ',') -cne 'inspect,status' -or
        (@($AdapterManifest.runtime_ipc.rejected_nested_commands) -join ',') -cne 'plan,approve,accept' -or
        [bool]$AdapterManifest.runtime_ipc.mutation_or_recovery_authority) {
        throw 'historical dormant contract lacks an exact read-only activation successor'
    }

    Assert-ContainsAll $CrateManifest @('blake3.workspace = true') 'runtime IPC crate manifest'
    Assert-ContainsAll $Lib @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15',
        'mod peer_reenrollment_contract;',
        'pub use peer_reenrollment_contract::*;',
        'PeerReenrollmentRead {'
    ) 'active runtime IPC library'

    Assert-ContainsAll $Contract @(
        'RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION: u8 = 2',
        'MAX_RUNTIME_IPC_PEER_REENROLLMENT_REQUEST_BYTES: usize = 4 * 1024',
        'MAX_RUNTIME_IPC_PEER_REENROLLMENT_RESPONSE_BYTES: usize = 16 * 1024',
        'pub enum RuntimeIpcPeerReenrollmentCommand',
        'Inspect {', 'Plan {', 'Approve {', 'Accept {', 'Status {',
        'RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed',
        'confirmation phrase does not match the plan ID',
        'pub enum RuntimeIpcPeerReenrollmentStatus',
        'MissingInvitation', 'UnsafeOrDivergent', 'AcceptanceIncomplete',
        'AcceptedRestartRequired', 'Consumed',
        'pub struct SignedRuntimeIpcPeerReenrollmentResponse',
        'request_digest: request.request_digest()?',
        'runtime_descriptor_binding: descriptor_binding(descriptor)?',
        'current_device_binding: device_binding(descriptor.content.device_id.as_bytes())',
        'verify runtime IPC peer re-enrollment response signature',
        'runtime IPC peer re-enrollment response skips or rewinds the requested stage',
        'SeparateApprovalRequired',
        'history rewrap cannot claim an empty inventory',
        'process-local state',
        'all_five_request_stages_round_trip_canonically',
        'implicit_approval_and_malformed_confirmation_fail_closed',
        'descriptor_substitution_rejects_request_before_adapter_dispatch',
        'signed_response_exact_binds_request_descriptor_and_current_device',
        'stage_skips_and_inconsistent_durable_claims_fail_closed',
        'process_local_suppression_never_substitutes_for_durable_quarantine',
        'incomplete_acceptance_is_observable_but_never_restart_authority',
        'history_rewrap_is_explicit_and_independent_from_reenrollment',
        'public_projection_contains_no_raw_identity_transport_or_message_fields'
    ) 'peer re-enrollment IPC contract'

    foreach ($forbidden in @(
        'tokio::net', 'reqwest::', 'iroh::', 'std::net::Tcp',
        'RuntimeIpcCommand::PeerReenrollment', 'RuntimeIpcResponse::PeerReenrollment',
        'remove_file(', 'remove_dir_all(', 'clear_quarantine', 'hot_apply'
    )) {
        if ($Contract.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "peer re-enrollment IPC contract gained forbidden active surface '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'not a variant of the active `RuntimeIpcCommand`',
        'request vocabulary keeps five operations distinct',
        'no combined toggle, implicit consent, automatic quarantine clear or hot runtime activation',
        'does not expose raw Account IDs, raw Device IDs',
        'signed by the current Device named by that verified runtime descriptor',
        'Durable predecessor quarantine and process-local scheduler suppression are separate',
        'does not give the replacement Device old pairwise ratchet keys',
        'Aggregate readiness contains 140 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment IPC RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $adapterManifestPath, $crateManifestPath, $libPath, $contractPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$adapterManifest = Get-Content -LiteralPath $adapterManifestPath -Raw | ConvertFrom-Json
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$contract = Get-Content -LiteralPath $contractPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $adapterManifest $crateManifest $lib $contract $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.contract.version = 2
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.ceremony.implicit_consent_allowed = $true
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_projection.raw_device_id_exposed = $true
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.signed_response_binding.current_descriptor_device = $false
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.history_boundary.separate_same_account_history_rewrap_approval_required = $false
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Contract $changed $adapterManifest $crateManifest $lib $contract $rfc }

    $changedAdapter = Copy-JsonObject $adapterManifest
    $changedAdapter.runtime_ipc.allowed_nested_commands = @('inspect', 'plan', 'status')
    $mutations += { Assert-Contract $manifest $changedAdapter $crateManifest $lib $contract $rfc }

    $changedRfc = $rfc.Replace('does not give the replacement Device old pairwise ratchet keys', 'recovers all old history automatically')
    $mutations += { Assert-Contract $manifest $adapterManifest $crateManifest $lib $contract $changedRfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            & $mutation
        }
        catch {
            $rejected++
        }
    }
    if ($rejected -ne 8) {
        throw "runtime IPC peer re-enrollment negative mutations rejected $rejected/8"
    }
    Write-Output 'negative_mutations=8/8'
}

Write-Output 'canonical_runtime_ipc_peer_reenrollment_contract_boundary=verified'
Write-Output 'active_runtime_ipc_changed=false'
Write-Output 'network_executed=false'
Write-Output 'second_host_required=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
