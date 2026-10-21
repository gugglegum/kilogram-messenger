[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-AUTHORIZATION-LEDGER.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$crateLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_authorization.rs'
$cliManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopManifestPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0263-m1239-recipient-import-consumption-authorization-ledger.md'

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

function Assert-Boundary {
    param(
        [object]$Manifest,
        [string]$CrateManifest,
        [string]$CrateLib,
        [string]$Source,
        [string]$CliManifest,
        [string]$DesktopManifest,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.39' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-authorization-ledger-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'consumption authorization identity or public scope changed'
    }

    foreach ($property in @(
        'dormant_library_api', 'exact_ephemeral_plan_required',
        'exact_commit_request_required', 'exact_completed_evidence_required',
        'exact_membership_set_digest_required', 'user_confirmed_required',
        'derived_confirmation_phrase_required', 'current_owner_device_signature_required',
        'plan_timestamps_reused_exactly', 'planning_read_only',
        'durably_authorizes_exact_future_consumption'
    )) {
        if (-not [bool]$Manifest.authorization_boundary.$property) {
            throw "consumption authorization property '$property' changed"
        }
    }
    if ([int]$Manifest.authorization_boundary.maximum_lifetime_seconds -ne 300 -or
        [bool]$Manifest.authorization_boundary.planning_creates_database -or
        [bool]$Manifest.authorization_boundary.consumes_intent) {
        throw 'consumption authorization lifetime or mutation boundary changed'
    }

    if ([string]$Manifest.durability.separate_database -cne
            'history-recovery/recipient-import-consumption-authorizations-v1.redb' -or
        [int]$Manifest.durability.maximum_records -ne 4096) {
        throw 'consumption authorization durable bounds changed'
    }
    foreach ($property in @(
        'state_directory_lock_required', 'immediate_durability',
        'content_addressed_device_signed_records', 'single_monotonic_chain',
        'nonce_index_exact', 'exact_replay_idempotent',
        'precommit_failure_retryable', 'stale_concurrent_plan_rejected',
        'reused_nonce_for_other_binding_rejected',
        'internal_fork_rollback_corruption_rejected'
    )) {
        if (-not [bool]$Manifest.durability.$property) {
            throw "consumption authorization durability '$property' changed"
        }
    }
    if ([bool]$Manifest.durability.whole_database_rollback_witnessed) {
        throw 'consumption authorization overclaims whole-database rollback detection'
    }

    foreach ($property in @(
        'owner_account_and_device', 'exact_completed_evidence', 'exact_consumption_plan',
        'membership_set_digest', 'nonce_sequence_previous_id_and_expiry'
    )) {
        if (-not [bool]$Manifest.stored_content.$property) {
            throw "consumption authorization stored binding '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot', 'message_body_or_plaintext',
        'ciphertext_history_artifact', 'filesystem_path', 'transport_authority',
        'raw_runtime_token'
    )) {
        if ([bool]$Manifest.stored_content.$property) {
            throw "consumption authorization stores forbidden '$property'"
        }
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_server_dispatch',
        'cli_or_desktop_control', 'background_worker_or_timer', 'network',
        'history_import', 'intent_consumption', 'signed_runtime_response_dispatch'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "consumption authorization gained authority '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.targeted_ledger_tests -cne '7/7' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '165/165' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'consumption authorization verification claim changed'
    }

    Assert-ContainsAll $CrateManifest @(
        'name = "kilogram-peer-history-recovery-runtime"',
        'kilogram-state = { path = "../kilogram-state" }',
        'postcard.workspace = true', 'redb.workspace = true', 'serde.workspace = true'
    ) 'recipient import runtime manifest'
    foreach ($forbiddenDependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($CrateManifest.IndexOf($forbiddenDependency, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption authorization crate gained '$forbiddenDependency'"
        }
    }
    Assert-ContainsAll $CrateLib @(
        'mod recipient_import_consumption_authorization;',
        'RecipientImportConsumptionAuthorizationLedger',
        'MAX_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATION_LIFETIME_SECONDS'
    ) 'recipient import runtime library'
    Assert-ContainsAll $Source @(
        'recipient-import-consumption-authorizations-v1.redb',
        'MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATIONS: u64 = 4_096',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS',
        'StateDirectoryLock::acquire', 'Durability::Immediate',
        'RuntimeIpcRecipientImportConsumptionCommand::Commit',
        'RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed',
        'recipient_import_consumption_confirmation_phrase',
        'RuntimeIpcRecipientImportConsumptionPlanSummary::issue(',
        'owner.sign(&record_signing_bytes', '.verify(&record_signing_bytes',
        'RecipientImportConsumptionAuthorizationLedgerError::NonceConflict',
        'RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged',
        'RecipientImportConsumptionAuthorizationLedgerError::AuthorizationExpired',
        'pub fn contains_membership_snapshot(&self) -> bool',
        'pub fn grants_consumption_authority(&self) -> bool',
        'commit_with_fault_before_commit',
        'planning_is_read_only_and_commit_survives_restart',
        'exact_replay_is_idempotent_without_a_second_record',
        'plan_command_and_mismatched_ephemeral_plan_fail_closed',
        'wrong_device_and_expired_commit_fail_closed',
        'competing_head_and_nonce_rebinding_fail_closed',
        'crash_before_atomic_commit_leaves_no_authorization',
        'corrupted_head_and_wrong_owner_are_rejected_after_restart'
    ) 'recipient import consumption authorization ledger'

    $productionSource = ($Source -split '(?m)^#\[cfg\(test\)\]\s*$', 2)[0]
    foreach ($forbidden in @(
        'tokio::', 'iroh::', 'reqwest::', 'TcpListener', 'TcpStream',
        'apply_next(', 'apply_bounded(', '.consume(',
        'RuntimeIpcCommand::RecipientImport'
    )) {
        if ($productionSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption authorization gained forbidden surface '$forbidden'"
        }
    }
    foreach ($appManifest in @($CliManifest, $DesktopManifest)) {
        if ($appManifest.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
            throw 'consumption authorization was wired into an active application'
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'records one exact Device-signed authorization',
        'splits authority from mutation',
        'validated M1.2.37 `Commit` request',
        'missing storage remains missing',
        'no more than 300 seconds',
        'one monotonic previous-ID chain',
        'same nonce with another plan fails closed',
        'does not claim detection of a whole-file rollback',
        'stores no membership snapshot',
        'no intent consume method is called',
        'Aggregate accepted-tag readiness contains 165 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "consumption authorization RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $crateManifestPath, $crateLibPath, $sourcePath,
    $cliManifestPath, $desktopManifestPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$crateLib = Get-Content -LiteralPath $crateLibPath -Raw
$source = Get-Content -LiteralPath $sourcePath -Raw
$cliManifest = Get-Content -LiteralPath $cliManifestPath -Raw
$desktopManifest = Get-Content -LiteralPath $desktopManifestPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.milestone = 'M1.2.40'
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.authorization_boundary.maximum_lifetime_seconds = 900
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.authorization_boundary.planning_creates_database = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.authorization_boundary.consumes_intent = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.durability.reused_nonce_for_other_binding_rejected = $false
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.durability.whole_database_rollback_witnessed = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.stored_content.membership_snapshot = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.activation_boundary.intent_consumption = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.targeted_ledger_tests = '6/7'
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "consumption authorization boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_consumption_authorization_ledger_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_consumption_authorization_ledger_boundary=verified'
Write-Output 'planning_read_only=true'
Write-Output 'intent_consumed=false'
Write-Output 'membership_snapshot_persisted=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
