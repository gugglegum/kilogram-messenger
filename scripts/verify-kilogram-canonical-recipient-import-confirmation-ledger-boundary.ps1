[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONFIRMATION-LEDGER.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$crateLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_confirmation.rs'
$cliManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopManifestPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0259-m1235-durable-recipient-import-confirmation-ledger.md'

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
        [string]$Manifest.milestone -cne 'M1.2.35' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-confirmation-ledger-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import confirmation identity or scope changed'
    }

    foreach ($property in @(
        'exact_ephemeral_plan_required', 'exact_membership_references_required',
        'exact_membership_set_digest_required', 'exact_intent_and_execution_plan_required',
        'exact_coordinator_id_and_sequence_required', 'explicit_confirm_command_required',
        'current_owner_device_signature_required', 'planning_read_only'
    )) {
        if (-not [bool]$Manifest.confirmation_boundary.$property) {
            throw "recipient import confirmation property '$property' changed"
        }
    }
    if ([int]$Manifest.confirmation_boundary.maximum_lifetime_seconds -ne 900 -or
        [bool]$Manifest.confirmation_boundary.planning_creates_database -or
        [bool]$Manifest.confirmation_boundary.confirmation_grants_import_authority -or
        [bool]$Manifest.confirmation_boundary.confirmation_consumes_intent) {
        throw 'recipient import confirmation lifetime or authority boundary changed'
    }

    if ([string]$Manifest.durability.separate_database -cne 'history-recovery/recipient-import-confirmations-v1.redb' -or
        [int]$Manifest.durability.maximum_records -ne 4096) {
        throw 'recipient import confirmation durable bounds changed'
    }
    foreach ($property in @(
        'state_directory_lock_required', 'immediate_durability',
        'content_addressed_signed_records', 'single_monotonic_chain',
        'nonce_index_exact', 'exact_replay_idempotent',
        'precommit_failure_retryable', 'stale_concurrent_plan_rejected',
        'reused_nonce_for_other_binding_rejected',
        'internal_fork_rollback_corruption_rejected'
    )) {
        if (-not [bool]$Manifest.durability.$property) {
            throw "recipient import confirmation durability '$property' changed"
        }
    }
    if ([bool]$Manifest.durability.whole_database_rollback_witnessed) {
        throw 'recipient import confirmation overclaims whole-database rollback detection'
    }

    foreach ($property in @(
        'owner_account_and_device', 'intent_execution_coordinator_ids',
        'membership_set_digest', 'nonce_sequence_previous_id_and_expiry'
    )) {
        if (-not [bool]$Manifest.stored_content.$property) {
            throw "recipient import confirmation stored binding '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot', 'message_body_or_plaintext',
        'ciphertext_history_artifact', 'filesystem_path', 'transport_authority'
    )) {
        if ([bool]$Manifest.stored_content.$property) {
            throw "recipient import confirmation stored forbidden content '$property'"
        }
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_server_dispatch',
        'cli_or_desktop_control', 'background_worker_or_timer', 'network',
        'mutation_command_dispatch', 'history_import', 'intent_consumption'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "recipient import confirmation gained authority '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.targeted_ledger_tests -cne '8/8' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '161/161' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'recipient import confirmation verification claim changed'
    }

    Assert-ContainsAll $CrateManifest @(
        'name = "kilogram-peer-history-recovery-runtime"',
        'kilogram-state = { path = "../kilogram-state" }',
        'postcard.workspace = true', 'redb.workspace = true', 'serde.workspace = true'
    ) 'recipient import runtime manifest'
    foreach ($forbiddenDependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($CrateManifest.IndexOf($forbiddenDependency, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import confirmation crate gained '$forbiddenDependency'"
        }
    }
    Assert-ContainsAll $CrateLib @(
        'mod recipient_import_confirmation;',
        'RecipientImportConfirmationLedger',
        'MAX_RECIPIENT_IMPORT_CONFIRMATION_LIFETIME_SECONDS'
    ) 'recipient import runtime library'
    Assert-ContainsAll $Source @(
        'recipient-import-confirmations-v1.redb',
        'MAX_DURABLE_RECIPIENT_IMPORT_CONFIRMATIONS: u64 = 4_096',
        'MAX_RECIPIENT_IMPORT_CONFIRMATION_LIFETIME_SECONDS: u64 = 15 * 60',
        'StateDirectoryLock::acquire', 'Durability::Immediate',
        'RuntimeIpcRecipientImportExecutionCommand::Confirm',
        'memberships != dispatch.exact_membership_references()',
        'owner.sign(&record_signing_bytes',
        '.verify(&record_signing_bytes',
        'RecipientImportConfirmationLedgerError::NonceConflict',
        'RecipientImportConfirmationLedgerError::LedgerChanged',
        'RecipientImportConfirmationLedgerError::ConfirmationExpired',
        'contains_membership_snapshot(&self) -> bool',
        'grants_import_authority(&self) -> bool',
        'commit_with_fault_before_commit',
        'confirmation_planning_is_read_only_and_privacy_bounded',
        'exact_confirmation_commits_and_replays_idempotently',
        'reused_nonce_for_another_plan_fails_closed',
        'concurrent_stale_plan_cannot_fork_the_chain',
        'expiry_and_foreign_device_fail_closed',
        'injected_precommit_failure_is_retryable',
        'corrupt_head_fails_closed',
        'changed_exact_membership_references_fail_closed'
    ) 'recipient import confirmation ledger'

    $productionSource = ($Source -split '(?m)^#\[cfg\(test\)\]\s*$', 2)[0]
    foreach ($forbidden in @(
        'tokio::', 'iroh::', 'reqwest::', 'TcpListener', 'TcpStream',
        'apply_next(', 'apply_bounded(', '.consume(',
        'RuntimeIpcCommand::RecipientImport'
    )) {
        if ($productionSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import confirmation gained forbidden surface '$forbidden'"
        }
    }
    foreach ($appManifest in @($CliManifest, $DesktopManifest)) {
        if ($appManifest.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
            throw 'recipient import confirmation was wired into an active application'
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'evidence that the user explicitly confirmed one exact plan',
        'Planning is read-only',
        'no more than 900 seconds',
        'one append-only content-addressed chain',
        'same nonce with another plan fails closed',
        'does not claim detection of a whole-file rollback',
        'stores no membership snapshot',
        'No M1.2.32 apply method is called',
        'Public default remains `legacy-only`',
        'Aggregate accepted-tag readiness contains 161 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import confirmation RFC is missing '$needle'"
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
    $changed.milestone = 'M1.2.36'
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.confirmation_boundary.maximum_lifetime_seconds = 3600
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.confirmation_boundary.planning_creates_database = $true
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
    $changed.activation_boundary.history_import = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.targeted_ledger_tests = '7/8'
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $cliManifest $desktopManifest $rfc }

    $mutations += {
        Assert-Boundary $manifest ($crateManifest + "`niroh.workspace = true") $crateLib $source $cliManifest $desktopManifest $rfc
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import confirmation boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_confirmation_ledger_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_confirmation_ledger_boundary=verified'
Write-Output 'planning_read_only=true'
Write-Output 'mutation_command_dispatched=false'
Write-Output 'membership_snapshot_persisted=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
