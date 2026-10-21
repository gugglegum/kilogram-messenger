[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-MUTATION-ADAPTER.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_mutation_adapter.rs'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_authorization.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$libPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$cratePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0264-m1240-recipient-import-consumption-mutation-adapter.md'

function Copy-JsonObject { param([object]$Value) $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json }

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param([object]$Manifest, [string]$Source, [string]$Authorization,
          [string]$Intent, [string]$Lib, [string]$Crate,
          [string]$Cli, [string]$Desktop, [string]$Rfc)

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.40' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-mutation-adapter-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'consumption mutation identity or public scope changed'
    }
    foreach ($property in @(
        'dormant_library_api', 'commit_command_only',
        'exact_durable_authorization_required', 'current_owner_device_required',
        'authorization_expiry_enforced_before_new_consume',
        'completed_mutation_evidence_reopened', 'caller_owned_memberships_reloaded',
        'current_coordinator_required', 'active_complete_unconsumed_intent_required',
        'prepared_durable_before_consume', 'completed_receipt_durable_before_success',
        'only_exact_active_intent_cleared', 'membership_and_history_preserved',
        'completed_replay_without_loader_or_mutation',
        'post_consume_crash_reconciled_without_second_consume'
    )) {
        if (-not [bool]$Manifest.mutation_boundary.$property) {
            throw "consumption mutation property '$property' changed"
        }
    }
    if ([string]$Manifest.durability.separate_database -cne
            'history-recovery/recipient-import-consumption-mutations-v1.redb' -or
        [int]$Manifest.durability.maximum_mutations -ne 4096 -or
        [bool]$Manifest.durability.whole_database_rollback_witnessed) {
        throw 'consumption mutation durable bounds changed'
    }
    foreach ($property in @(
        'state_directory_lock_required', 'immediate_durability',
        'device_signed_prepared_and_completed_records', 'content_addressed_record_ids',
        'immutable_nonce_keyed_records', 'single_in_flight_mutation',
        'exact_replay_idempotent', 'nonce_rebinding_rejected',
        'prepared_completed_link_verified'
    )) {
        if (-not [bool]$Manifest.durability.$property) {
            throw "consumption mutation durability '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot', 'message_body_or_plaintext',
        'ciphertext_history_artifact', 'filesystem_path', 'transport_authority',
        'raw_runtime_token'
    )) {
        if ([bool]$Manifest.stored_content.$property) {
            throw "consumption mutation stores forbidden '$property'"
        }
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_server_dispatch',
        'signed_runtime_response_dispatch', 'cli_or_desktop_control',
        'background_worker_or_timer', 'network', 'history_deletion',
        'membership_deletion'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "consumption mutation gained forbidden authority '$property'"
        }
    }
    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.focused_mutation_tests -cne '5/5' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '166/166' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'consumption mutation verification claim changed'
    }

    Assert-ContainsAll $Source @(
        'recipient-import-consumption-mutations-v1.redb',
        'MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS: u64 = 4_096',
        'RecipientImportConsumptionMutationPhase::Prepared',
        'RecipientImportConsumptionMutationPhase::Completed',
        'StateDirectoryLock::acquire', 'Durability::Immediate',
        'load_by_consumption_nonce', 'exact_completed_mutation',
        'load_exact_memberships', 'RecipientRecoveryImportRoundIntentProgressState::Complete',
        'store.consume(', 'receipt_from_consumed', 'InjectedAfterConsume',
        'replayed_completed_receipt: true', 'reconciled_consumed_intent: true',
        'recipient_import_consumption_mutation_ledger_survives_restart_and_replays',
        'recipient_import_consumption_mutation_ledger_rejects_nonce_rebinding_and_wrong_owner'
    ) 'consumption mutation source'
    Assert-ContainsAll $Authorization @('pub fn load_by_consumption_nonce') 'authorization ledger'
    Assert-ContainsAll $Intent @(
        'pub fn final_coordinator_sequence(&self)',
        'pub fn completed_source_count(&self)',
        'active.remove(ACTIVE_KEY)'
    ) 'intent consumption store'
    Assert-ContainsAll $Lib @(
        'mod recipient_import_consumption_mutation_adapter;',
        'RecipientHistoryRecoveryImportConsumptionMutationAdapter'
    ) 'runtime integration library'
    Assert-ContainsAll $Crate @('name = "kilogram-peer-history-recovery-runtime"') 'runtime crate'
    foreach ($dependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($Crate.IndexOf($dependency, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption mutation crate gained '$dependency'"
        }
    }
    $successorDependency = 'kilogram-peer-history-recovery-runtime = { path = "../../crates/kilogram-peer-history-recovery-runtime" }'
    if ($Cli.IndexOf($successorDependency, [StringComparison]::Ordinal) -lt 0 -or
        [regex]::Matches($Cli, 'kilogram-peer-history-recovery-runtime\s*=').Count -ne 1 -or
        $Desktop.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
        throw 'consumption mutation adapter successor application boundary changed'
    }
    $production = ($Source -split '(?m)^#\[cfg\(test\)\]\s*$', 2)[0]
    foreach ($forbidden in @('tokio::', 'iroh::', 'reqwest::', 'TcpListener', 'TcpStream',
                              'RuntimeIpcCommand::RecipientImport')) {
        if ($production.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption mutation gained forbidden surface '$forbidden'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'consumes exactly that authorized active intent',
        'never deletes the intent record',
        'Device-signed `Prepared` and `Completed` records',
        'without invoking the membership loader',
        'without consuming again',
        'does not claim detection of a whole-database rollback',
        'No active `RuntimeIpcCommand`',
        'Aggregate accepted-tag readiness contains 166 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "consumption mutation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $sourcePath, $authorizationPath, $intentPath,
                     $libPath, $cratePath, $cliPath, $desktopPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$crate = Get-Content -LiteralPath $cratePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $source $authorization $intent $lib $crate $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.milestone = 'M1.2.41'
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.mutation_boundary.exact_durable_authorization_required = $false
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.mutation_boundary.prepared_durable_before_consume = $false
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.mutation_boundary.completed_receipt_durable_before_success = $false
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.mutation_boundary.membership_and_history_preserved = $false
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.mutation_boundary.completed_replay_without_loader_or_mutation = $false
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.durability.maximum_mutations = 0
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.durability.whole_database_rollback_witnessed = $true
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.network = $true
    $mutations += { Assert-Boundary $changed $source $authorization $intent $lib $crate $cli $desktop $rfc }
    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'mutation was accepted' } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=10/10'
}

Write-Output 'canonical_recipient_import_consumption_mutation_adapter_boundary=verified'
Write-Output 'changes_active_runtime_ipc=false'
Write-Output 'intent_consumption=explicit-exact-and-crash-safe'
Write-Output 'history_and_membership_deletion=false'
Write-Output 'network_executed=false'
