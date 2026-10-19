[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-PEER-REENROLLMENT-OLD-HISTORY-RECOVERY-PLAN.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-DESKTOP-CEREMONY.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery\Cargo.toml'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0242-m1218-explicit-old-history-recovery-plan.md'

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
        [string]$Cargo,
        [string]$Source,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.18' -or
        [string]$Manifest.status -cne 'peer-reenrollment-old-history-recovery-plan-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.17' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'old-history recovery plan identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.17' -or
        [string]$Predecessor.status -cne 'canonical-runtime-ipc-peer-reenrollment-desktop-ceremony-verified') {
        throw 'old-history recovery predecessor changed'
    }

    $inventory = $Manifest.inventory
    if (-not [bool]$inventory.read_only -or
        [int]$inventory.max_sources -ne 8 -or
        [int]$inventory.max_conversations -ne 64 -or
        [int]$inventory.max_ranges -ne 256 -or
        [int]$inventory.max_checkpoints -ne 256 -or
        -not [bool]$inventory.signed_source_claims -or
        -not [bool]$inventory.recipient_signed_existing_checkpoints -or
        -not [bool]$inventory.privacy_safe_view -or
        [bool]$inventory.message_bodies_visible -or
        -not [bool]$inventory.divergent_claims_visible -or
        [bool]$inventory.divergent_claims_plannable) {
        throw 'old-history read-only inventory boundary changed'
    }

    foreach ($required in @(
        'content_addressed', 'source_device_signed',
        'requires_consumed_reenrollment_transition', 'binds_acceptance_receipt',
        'binds_reenrollment_transition', 'binds_replacement_device',
        'binds_authority_revision_and_digest', 'binds_membership_revision_and_digest',
        'binds_source_claim_and_inventory', 'binds_conversation_ranges',
        'binds_existing_checkpoint'
    )) {
        if (-not [bool]$Manifest.plan.$required) { throw "plan property '$required' changed" }
    }
    if ([int]$Manifest.plan.validity_seconds -ne 900) {
        throw 'old-history plan validity changed'
    }

    foreach ($required in @(
        'separate_from_reenrollment_approval', 'separate_from_peer_acceptance',
        'explicit_history_owner_approval',
        'replacement_device_cannot_self_approve_source_history'
    )) {
        if (-not [bool]$Manifest.consent.$required) { throw "consent property '$required' changed" }
    }
    if ([string]$Manifest.consent.exact_typed_phrase_prefix -cne 'RECOVER HISTORY') {
        throw 'old-history consent phrase changed'
    }

    foreach ($required in @(
        'changed_source_inventory', 'changed_authority', 'changed_membership',
        'changed_recipient', 'changed_range', 'changed_checkpoint',
        'changed_reenrollment_transition'
    )) {
        if (-not [bool]$Manifest.invalidation.$required) {
            throw "invalidation property '$required' changed"
        }
    }

    foreach ($forbidden in @(
        'rewrap_or_transfer_performed', 'checkpoint_mutation_performed',
        'durable_state_written', 'network_opened', 'desktop_control_added',
        'automatic_recovery'
    )) {
        if ([bool]$Manifest.execution.$forbidden) {
            throw "planning stage gained execution authority '$forbidden'"
        }
    }

    if ([int]$Manifest.verification.pure_planner_tests -ne 6 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '144/144' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'old-history verification boundary changed'
    }

    if ([int]$Completion.active_postcard.version -ne 33 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        @($Completion.canonical.commands).Count -ne 38 -or
        @($Completion.canonical.results).Count -ne 37 -or
        @($Completion.active_postcard.commands).Count -ne 33 -or
        @($Completion.active_postcard.responses).Count -ne 32 -or
        @($Schema.semantic_rules).Count -ne 192) {
        throw 'pure planning stage changed active runtime IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 33) {
        throw 'old-history planning changed compatibility binding'
    }

    Assert-ContainsAll $Cargo @(
        'name = "kilogram-peer-history-recovery"',
        'kilogram-peer-reenrollment-acceptance',
        'kilogram-peer-reenrollment-runtime',
        'kilogram-protocol'
    ) 'old-history planner Cargo manifest'
    foreach ($forbidden in @('iroh', 'reqwest', 'redb', 'tokio', 'eframe')) {
        if ($Cargo.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "old-history planner gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Source @(
        'pub struct SignedHistorySourceClaim',
        'pub struct OldHistoryRecoveryInventory',
        'pub struct OldHistoryRecoveryInventoryView',
        'pub enum SourceClaimAgreement',
        'pub struct PeerReenrollmentHistoryBinding',
        'pub fn from_consumed_transition',
        'if !transition.consumed()',
        'pub struct OldHistoryRecoveryPlan',
        'pub struct SignedOldHistoryRecoveryPlan',
        'ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved',
        'RECOVER HISTORY',
        'PeerHistoryRecoveryError::DivergentSourceClaims',
        'contains_message_bodies: false',
        'SignedHistoryRecoveryCheckpoint',
        'changed_inventory_membership_recipient_range_or_transition_invalidates_plan',
        'tamper_expiry_and_checkpoint_drift_fail_closed'
    ) 'old-history planner source'
    foreach ($forbidden in @(
        'use std::fs', 'std::net', 'use iroh', 'use redb', 'use reqwest',
        'use tokio', 'HistoryRewrapBundle::seal', 'RuntimeIpcCommand',
        'kilogram_windows'
    )) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "old-history planner source contains forbidden authority '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Re-enrollment approval, peer acceptance and old-history recovery consent are three distinct acts',
        'The claim type has no message-body field',
        'divergence remains visible and that conversation cannot be planned',
        'The transition must have been consumed by a fresh runtime',
        'The replacement Device cannot self-approve source history',
        'does not read plaintext, build a rewrap bundle, advance a checkpoint',
        'Aggregate accepted-tag readiness contains 144 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "old-history RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $cargoPath, $sourcePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath $cargoPath -Raw
$source = Get-Content -LiteralPath $sourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $predecessor, $completion, $schema, $compatibility, $cargo, $source, $rfc)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.inventory.message_bodies_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.inventory.divergent_claims_plannable = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.plan.requires_consumed_reenrollment_transition = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.consent.separate_from_peer_acceptance = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.invalidation.changed_membership = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.invalidation.changed_checkpoint = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.execution.rewrap_or_transfer_performed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $rfc }
    $changedCargo = "$cargo`nredb.workspace = true"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $changedCargo $source $rfc }
    $changedSource = $source.Replace('if !transition.consumed()', 'if false')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $changedSource $rfc }
    $changedRfc = $rfc.Replace('Aggregate accepted-tag readiness contains 144 closed gates', 'Aggregate readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $source $changedRfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'negative mutation unexpectedly passed' }
        catch {
            if ($_.Exception.Message -ceq 'negative mutation unexpectedly passed') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'peer_reenrollment_old_history_recovery_plan_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'peer_reenrollment_old_history_recovery_plan_boundary=verified'
Write-Output 'inventory=read-only-signed-body-free'
Write-Output 'consent=separate-source-device-signed'
Write-Output 'execution_authority=none'
Write-Output 'active_runtime_ipc=postcard-v33-json-descriptor-v14'
Write-Output 'changes_active_wire=false'
Write-Output 'changes_public_default=false'
Write-Output 'public_default=legacy-only'
Write-Output 'second_host_required=false'
Write-Output 'product_network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'automatic_github_build=false'
