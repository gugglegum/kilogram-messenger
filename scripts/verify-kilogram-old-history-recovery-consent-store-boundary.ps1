[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-OLD-HISTORY-RECOVERY-CONSENT-STORE.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-PEER-REENROLLMENT-OLD-HISTORY-RECOVERY-PLAN.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-consent\Cargo.toml'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-consent\src\lib.rs'
$plannerSourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0243-m1219-durable-old-history-recovery-consent.md'

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
        [string]$PlannerSource,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.19' -or
        [string]$Manifest.status -cne 'durable-old-history-recovery-consent-store-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.18' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'durable old-history consent identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.18' -or
        [string]$Predecessor.status -cne 'peer-reenrollment-old-history-recovery-plan-verified') {
        throw 'durable old-history consent predecessor changed'
    }

    $store = $Manifest.store
    if ([string]$store.database_file -cne 'old-history-recovery-consent-v1.redb' -or
        -not [bool]$store.separate_from_reenrollment_stores -or
        -not [bool]$store.owner_account_bound -or
        -not [bool]$store.owner_device_bound -or
        -not [bool]$store.immediate_durability -or
        -not [bool]$store.immutable_signed_records -or
        -not [bool]$store.single_head_per_transition -or
        [int]$store.max_records -ne 4096 -or
        [int]$store.max_record_bytes -ne 98304 -or
        [int]$store.max_head_bytes -ne 1024) {
        throw 'durable old-history consent store boundary changed'
    }

    foreach ($required in @(
        'reverifies_signed_plan', 'reverifies_consumed_transition',
        'reverifies_current_authority', 'reverifies_memberships',
        'reverifies_body_free_inventory', 'reverifies_ranges_and_checkpoints',
        'requires_unexpired_plan', 'atomic_record_and_head'
    )) {
        if (-not [bool]$Manifest.first_commit.$required) {
            throw "first-commit property '$required' changed"
        }
    }
    foreach ($required in @(
        'exact_replay_idempotent', 'exact_committed_replay_after_expiry',
        'competing_plan_rejected', 'foreign_owner_rejected',
        'fork_and_orphan_rejected', 'corruption_rejected',
        'new_existing_database_never_repaired'
    )) {
        if (-not [bool]$Manifest.replay.$required) {
            throw "replay property '$required' changed"
        }
    }

    if (-not [bool]$Manifest.observation.read_only_open_existing -or
        [bool]$Manifest.observation.missing_store_created -or
        [bool]$Manifest.observation.message_bodies_visible -or
        [bool]$Manifest.observation.raw_identity_visible -or
        [bool]$Manifest.observation.filesystem_paths_visible -or
        [bool]$Manifest.observation.network_coordinates_visible -or
        [bool]$Manifest.observation.execution_authorized) {
        throw 'read-only consent observation boundary changed'
    }
    foreach ($forbidden in @(
        'rewrap_or_transfer_performed', 'checkpoint_mutation_performed',
        'network_opened', 'active_ipc_command_added',
        'desktop_control_added', 'automatic_recovery'
    )) {
        if ([bool]$Manifest.execution.$forbidden) {
            throw "consent store gained execution authority '$forbidden'"
        }
    }

    if ([int]$Manifest.verification.durable_store_tests -ne 6 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '145/145' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'durable old-history consent verification boundary changed'
    }
    if ('whole-database-rollback-detection-without-external-witness' -notin @($Manifest.non_claims)) {
        throw 'whole-database rollback non-claim is missing'
    }

    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38 -or
        @($Completion.active_postcard.commands).Count -ne 34 -or
        @($Completion.active_postcard.responses).Count -ne 33 -or
        @($Schema.semantic_rules).Count -ne 192) {
        throw 'consent-only stage changed active runtime IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'consent-only stage changed compatibility binding'
    }

    Assert-ContainsAll $Cargo @(
        'name = "kilogram-peer-history-recovery-consent"',
        'kilogram-peer-history-recovery',
        'redb.workspace = true',
        'postcard.workspace = true'
    ) 'durable old-history consent Cargo manifest'
    foreach ($forbidden in @('iroh', 'reqwest', 'tokio', 'eframe', 'kilogram-runtime-ipc')) {
        if ($Cargo.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "durable consent store gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Source @(
        'const DATABASE_FILE: &str = "old-history-recovery-consent-v1.redb"',
        'Durability::Immediate',
        'pub struct SignedOldHistoryRecoveryConsentRecord',
        'pub struct DurableOldHistoryRecoveryConsent',
        'pub struct OldHistoryRecoveryConsentStore',
        'pub struct OldHistoryRecoveryConsentReadStore',
        'pub fn open_existing',
        'plan.verify_context(',
        'OldHistoryRecoveryConsentOutcome::AlreadyCommitted',
        'OldHistoryRecoveryConsentStoreError::ConsentAlreadyCommitted',
        'OldHistoryRecoveryConsentStoreError::OrphanRecord',
        'contains_message_bodies: false',
        'execution_authorized: false',
        'fn atomic_commit_reopen_and_privacy_safe_status',
        'fn exact_replay_after_expiry_is_idempotent',
        'fn competing_plan_for_same_transition_fails_closed',
        'fn first_commit_rechecks_expiry_context_and_owner',
        'fn pre_commit_failure_is_empty_and_retryable',
        'fn corruption_and_orphaned_record_fail_closed'
    ) 'durable old-history consent source'
    foreach ($forbidden in @(
        'use std::net', 'use iroh', 'use reqwest', 'use tokio',
        'HistoryRewrapBundle::seal', 'RuntimeIpcCommand', 'kilogram_windows'
    )) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "durable consent store source contains forbidden authority '$forbidden'"
        }
    }
    Assert-ContainsAll $PlannerSource @(
        'pub fn acceptance_receipt_id(&self)',
        'pub fn history_account_id(&self)',
        'pub fn history_authority_revision(&self)',
        'pub fn source_claim_id(&self)',
        'pub fn prepared_at_unix_seconds(&self)',
        'pub fn expires_at_unix_seconds(&self)'
    ) 'old-history planner accessor surface'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'records one exact consent before any later history rewrap or transfer may begin',
        'The plan must still be within its 15-minute validity window',
        'One immediate-durability transaction inserts both an immutable signed record and its single head',
        'An exact replay of the already committed plan is idempotent, including after the plan''s original expiry',
        'An existing database is never initialized or repaired implicitly',
        'Detecting whole-database rollback requires an external monotonic witness',
        '`execution_authorized` is always false at this stage',
        'Aggregate accepted-tag readiness contains 145 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "durable old-history consent RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $cargoPath, $sourcePath, $plannerSourcePath, $rfcPath
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
$plannerSource = Get-Content -LiteralPath $plannerSourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $schema, $compatibility,
    $cargo, $source, $plannerSource, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.store.owner_account_bound = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.store.immediate_durability = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.first_commit.reverifies_consumed_transition = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.first_commit.requires_unexpired_plan = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.replay.exact_committed_replay_after_expiry = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.replay.fork_and_orphan_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.observation.message_bodies_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.execution.rewrap_or_transfer_performed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $rfc }
    $changedCargo = "$cargo`niroh.workspace = true"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $changedCargo $source $plannerSource $rfc }
    $changedRfc = $rfc.Replace('Aggregate accepted-tag readiness contains 145 closed gates', 'Aggregate readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $source $plannerSource $changedRfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'negative mutation unexpectedly passed' }
        catch {
            if ($_.Exception.Message -ceq 'negative mutation unexpectedly passed') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'old_history_recovery_consent_store_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'old_history_recovery_consent_store_boundary=verified'
Write-Output 'durability=immediate-record-and-head'
Write-Output 'replay=exact-idempotent-after-expiry'
Write-Output 'observation=bounded-read-only-no-create'
Write-Output 'execution_authority=none'
Write-Output 'whole_database_rollback_detection=external-witness-required'
Write-Output 'active_runtime_ipc=postcard-v34-json-descriptor-v15'
Write-Output 'changes_active_wire=false'
Write-Output 'changes_public_default=false'
Write-Output 'public_default=legacy-only'
Write-Output 'second_host_required=false'
Write-Output 'product_network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'automatic_github_build=false'
