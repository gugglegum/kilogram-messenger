[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-PLAN.json'
$workspaceManifestPath = Join-Path $workspace 'Cargo.toml'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment\Cargo.toml'
$reenrollmentPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment\src\lib.rs'
$storePath = Join-Path $workspace 'crates\kilogram-peer-capability-store\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0234-m1210-device-authorized-peer-reenrollment-plan.md'

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
        [string]$WorkspaceManifest,
        [string]$CrateManifest,
        [string]$Reenrollment,
        [string]$Store,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.10' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-reenrollment-plan-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer re-enrollment plan identity or scope changed'
    }

    $context = $Manifest.quarantine_context
    if (-not [bool]$context.read_only -or
        -not [bool]$context.verifies_complete_retained_chain -or
        -not [bool]$context.binds_owner_device -or
        -not [bool]$context.binds_exact_peer_key -or
        -not [bool]$context.binds_exact_head_record_and_sequence -or
        -not [bool]$context.binds_exact_predecessor_record_and_floor -or
        -not [bool]$context.missing_or_non_quarantined_state_fails_closed -or
        [bool]$context.observation_mutates_database) {
        throw 'peer re-enrollment quarantine context changed'
    }

    $authority = $Manifest.authority_prerequisites
    foreach ($property in @(
        'local_root_signed_device_list', 'local_signer_must_be_active_sync_device',
        'peer_root_signed_device_list', 'fresh_device_signed_current_invitation',
        'invitation_must_target_local_account', 'peer_authority_revision_must_match_exactly',
        'replacement_device_must_be_active_sync_device', 'authority_regression_rejected',
        'same_device_ticket_epoch_regression_rejected',
        'same_device_non_advancing_invitation_rejected'
    )) {
        if (-not [bool]$authority.$property) {
            throw "peer re-enrollment authority prerequisite '$property' changed"
        }
    }

    $confirmation = $Manifest.explicit_confirmation
    if ([int]$confirmation.fresh_nonce_bytes -ne 32 -or
        [int]$confirmation.maximum_validity_seconds -ne 900 -or
        -not [bool]$confirmation.validity_cannot_exceed_invitation -or
        -not [bool]$confirmation.exact_confirmation_phrase_required -or
        -not [bool]$confirmation.explicit_user_or_test_operator_enum_required -or
        [string]$confirmation.signature_authority -cne 'current-local-device-ed25519') {
        throw 'peer re-enrollment explicit confirmation changed'
    }

    $binding = $Manifest.artifact_binding
    if (-not [bool]$binding.old_quarantine_head -or
        -not [bool]$binding.old_verified_predecessor_floor -or
        -not [bool]$binding.local_and_peer_authority_digests -or
        -not [bool]$binding.replacement_peer_device -or
        -not [bool]$binding.invitation_and_required_capability_ids -or
        -not [bool]$binding.publication_epoch -or
        -not [bool]$binding.fresh_nonce_and_expiry -or
        -not [bool]$binding.canonical_bounded_encoding -or
        [int]$binding.maximum_encoded_bytes -ne 4096) {
        throw 'peer re-enrollment artifact binding changed'
    }

    $privacy = $Manifest.privacy_boundary
    foreach ($property in @(
        'contains_connection_ticket_body', 'contains_endpoint_or_address',
        'contains_relay_url', 'contains_message_or_ciphertext',
        'contains_filesystem_path', 'contains_account_root_or_device_secret'
    )) {
        if ([bool]$privacy.$property) {
            throw "peer re-enrollment artifact gained forbidden content '$property'"
        }
    }

    $activation = $Manifest.activation_boundary
    if ([bool]$activation.artifact_applies_itself -or
        [bool]$activation.store_writer_dependency -or
        [bool]$activation.runtime_ipc_dependency -or
        [bool]$activation.transport_or_network_dependency -or
        [bool]$activation.cli_or_desktop_integration -or
        [bool]$activation.automatic_quarantine_clear_enabled -or
        [bool]$activation.remote_quarantine_clear_enabled -or
        -not [bool]$activation.old_quarantine_is_retained -or
        [bool]$activation.public_canonical_rollout_enabled) {
        throw 'peer re-enrollment planning artifact gained apply authority'
    }

    Assert-ContainsAll $WorkspaceManifest @('"crates/kilogram-peer-reenrollment"') 'workspace manifest'
    Assert-ContainsAll $CrateManifest @(
        'kilogram-identity =',
        'kilogram-peer-capability-store =',
        'kilogram-sync-capability ='
    ) 'peer re-enrollment crate manifest'
    foreach ($forbidden in @(
        'redb', 'reqwest', 'iroh', 'tokio', 'kilogram-runtime-ipc',
        'kilogram-transport-iroh', 'kilogram-cli', 'kilogram-windows'
    )) {
        if ($CrateManifest.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "peer re-enrollment crate gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Store @(
        'pub struct DurablePeerCapabilityQuarantineHead',
        'pub fn load_quarantined_head(',
        'load_quarantined_head_from_database',
        'PeerCapabilityStoreError::PeerStateMissing',
        'PeerCapabilityStoreError::PeerStateNotQuarantined',
        'PeerCapabilityStoreError::InvalidQuarantinePredecessor',
        'read_only_open_observes_durable_quarantine_without_mutation'
    ) 'peer capability quarantine context'

    Assert-ContainsAll $Reenrollment @(
        'pub enum ExplicitPeerReenrollmentApproval',
        'UserOrTestOperatorApproved',
        'pub struct PeerReenrollmentPlan',
        'pub struct SignedPeerReenrollmentTombstone',
        'pub const DEFAULT_PEER_REENROLLMENT_PLAN_VALIDITY_SECONDS: u64 = 15 * 60;',
        'pub const MAX_PEER_REENROLLMENT_PLAN_BYTES: usize = 4 * 1024;',
        'pub fn confirmation_phrase(',
        'pub fn confirm_and_sign(',
        'pub fn verify_context(',
        'local_authority_digest',
        'peer_authority_digest',
        'quarantine_record_id',
        'predecessor_record_id',
        'invitation_id',
        'required_capability_id',
        'fn validate_successor(',
        'PeerReenrollmentError::AuthorityRegression',
        'PeerReenrollmentError::TicketEpochRegression',
        'PeerReenrollmentError::NotASuccessor',
        'explicit_confirmation_yields_bounded_signed_context_without_store_mutation',
        'missing_approval_wrong_phrase_wrong_signer_and_tamper_fail_closed',
        'substituted_invitation_authority_and_expiry_fail_context_verification',
        'same_device_stale_or_regressed_invitation_is_not_a_successor'
    ) 'peer re-enrollment implementation'
    foreach ($forbidden in @(
        'pub fn apply(', 'pub fn clear(', 'Database::create', 'begin_write(',
        'reqwest::', 'iroh::', 'tokio::net', 'RuntimeIpc'
    )) {
        if ($Reenrollment.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "peer re-enrollment implementation gained forbidden authority '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'does not clear or rewrite the quarantined chain',
        'public default remains `legacy-only`',
        'short exact `REENROLL` confirmation phrase',
        'contains no connection ticket body',
        'offers no apply or clear API',
        'old quarantined chain remains evidence'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $workspaceManifestPath, $crateManifestPath,
    $reenrollmentPath, $storePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$workspaceManifest = Get-Content -LiteralPath $workspaceManifestPath -Raw
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$reenrollment = Get-Content -LiteralPath $reenrollmentPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $workspaceManifest $crateManifest $reenrollment $store $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.activation_boundary.automatic_quarantine_clear_enabled = $true
    $mutations += [pscustomobject]@{ Label = 'automatic_clear'; Manifest = $changed; Crate = $crateManifest; Source = $reenrollment }

    $changed = Copy-JsonObject $manifest
    $changed.authority_prerequisites.peer_authority_revision_must_match_exactly = $false
    $mutations += [pscustomobject]@{ Label = 'inexact_peer_authority'; Manifest = $changed; Crate = $crateManifest; Source = $reenrollment }

    $changed = Copy-JsonObject $manifest
    $changed.explicit_confirmation.explicit_user_or_test_operator_enum_required = $false
    $mutations += [pscustomobject]@{ Label = 'implicit_confirmation'; Manifest = $changed; Crate = $crateManifest; Source = $reenrollment }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_boundary.contains_connection_ticket_body = $true
    $mutations += [pscustomobject]@{ Label = 'embedded_ticket'; Manifest = $changed; Crate = $crateManifest; Source = $reenrollment }

    $changed = Copy-JsonObject $manifest
    $changed.public_default = 'dual-reader-prefer-canonical'
    $mutations += [pscustomobject]@{ Label = 'public_default_change'; Manifest = $changed; Crate = $crateManifest; Source = $reenrollment }

    $mutations += [pscustomobject]@{
        Label = 'runtime_dependency'; Manifest = $manifest
        Crate = "$crateManifest`r`nkilogram-runtime-ipc = { path = '../kilogram-runtime-ipc' }"
        Source = $reenrollment
    }
    $mutations += [pscustomobject]@{
        Label = 'apply_api'; Manifest = $manifest; Crate = $crateManifest
        Source = "$reenrollment`r`npub fn apply() {}"
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            Assert-Contract $mutation.Manifest $workspaceManifest $mutation.Crate $mutation.Source $store $rfc
        }
        catch {
            Write-Output "$($mutation.Label)=rejected"
            $rejected++
            continue
        }
        throw "$($mutation.Label) mutation was unexpectedly accepted"
    }
    if ($rejected -ne 7) {
        throw "peer re-enrollment mutation matrix incomplete: $rejected/7"
    }
    Write-Output 'peer_reenrollment_plan_negative_mutations=7/7'
    Write-Output 'canonical_sync_v2_peer_reenrollment_plan_self_test=passed'
    exit 0
}

Write-Output 'network_executed=false'
Write-Output 'runtime_ipc_changed=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_reenrollment_plan_boundary=verified'
