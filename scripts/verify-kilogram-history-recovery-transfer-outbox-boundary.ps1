[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-OUTBOX.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-IROH-SESSION.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\Cargo.toml'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\lib.rs'
$outboxPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\outbox.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0248-m1224-durable-history-transfer-outbox.md'

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

function Assert-OmitsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "$Label unexpectedly contains '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Predecessor,
        [object]$Completion,
        [object]$Compatibility,
        [string]$Cargo,
        [string]$Library,
        [string]$Outbox,
        [string]$CliCargo,
        [string]$Cli,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.24' -or
        [string]$Manifest.status -cne 'durable-history-transfer-outbox-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.23' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'history transfer outbox identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.23' -or
        [string]$Predecessor.status -cne 'authenticated-iroh-history-transfer-session-verified') {
        throw 'history transfer outbox predecessor changed'
    }

    foreach ($required in @(
        'canonical_exact_bytes_persisted', 'source_device_signature_reverified',
        'owner_account_bound', 'source_device_bound', 'recipient_device_bound',
        'source_endpoint_bound', 'recipient_endpoint_bound', 'session_nonce_bound',
        'transfer_id_bound', 'competing_request_for_transfer_rejected'
    )) {
        if (-not [bool]$Manifest.prepared_request.$required) {
            throw "prepared request property '$required' changed"
        }
    }
    if ([bool]$Manifest.prepared_request.new_nonce_on_restart) {
        throw 'restart can generate a new transfer nonce'
    }

    $store = $Manifest.store
    if ([string]$store.database -cne 'old-history-recovery-transfer-outbox-v1.redb' -or
        [int]$store.schema_version -ne 1 -or
        [int64]$store.max_intents -ne 16384 -or
        [bool]$store.whole_database_rollback_detected_without_external_witness) {
        throw 'history transfer outbox store boundary changed'
    }
    foreach ($required in @(
        'immediate_durability', 'atomic_request_and_head', 'atomic_receipt_commit',
        'owner_metadata_verified', 'orphan_and_binding_validation',
        'read_only_open_existing_no_create', 'injected_precommit_rollback_verified'
    )) {
        if (-not [bool]$store.$required) { throw "store property '$required' changed" }
    }

    foreach ($required in @(
        'exact_canonical_bytes_persisted', 'recipient_device_signature_reverified',
        'exact_request_and_transfer_bound', 'both_endpoints_bound',
        'exact_replay_idempotent', 'different_receipt_after_commit_rejected'
    )) {
        if (-not [bool]$Manifest.receipt.$required) { throw "receipt property '$required' changed" }
    }
    if ([bool]$Manifest.receipt.receipt_grants_import_authority) {
        throw 'transport receipt gained import authority'
    }

    if (@($Manifest.observation.states).Count -ne 3 -or
        'absent' -notin @($Manifest.observation.states) -or
        'pending' -notin @($Manifest.observation.states) -or
        'acknowledged' -notin @($Manifest.observation.states) -or
        -not [bool]$Manifest.observation.opaque_recipient_token) {
        throw 'history transfer outbox status boundary changed'
    }
    foreach ($forbidden in @(
        'message_bodies_visible', 'plaintext_visible', 'raw_identity_visible',
        'filesystem_paths_visible', 'network_coordinates_visible',
        'network_authority_granted', 'import_authority_granted'
    )) {
        if ([bool]$Manifest.observation.$forbidden) {
            throw "status non-claim '$forbidden' changed"
        }
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_scheduler_added', 'automatic_network_retry_added',
        'multi_source_scheduling_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "authority non-claim '$forbidden' changed"
        }
    }

    if ([int]$Manifest.verification.focused_outbox_tests -ne 4 -or
        [int]$Manifest.verification.combined_transport_outbox_tests -ne 8 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '150/150' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'history transfer outbox verification boundary changed'
    }
    foreach ($nonClaim in @(
        'active-runtime-history-transfer', 'background-transfer-worker',
        'automatic-network-resume', 'multi-source-recovery-scheduling',
        'runtime-ipc-history-recovery-authority', 'desktop-history-recovery-authority',
        'whole-database-rollback-detection-without-external-witness'
    )) {
        if ($nonClaim -notin @($Manifest.non_claims)) { throw "required non-claim '$nonClaim' is missing" }
    }

    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38) {
        throw 'active runtime IPC inventory changed'
    }
    $runtimeSurface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([bool]$Compatibility.changes_wire -or
        $runtimeSurface.Count -ne 1 -or
        [int](@($runtimeSurface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'compatibility inventory changed'
    }

    Assert-ContainsAll $Cargo @(
        'kilogram-peer-history-recovery-execution', 'iroh.workspace = true',
        'redb.workspace = true', 'tokio.workspace = true'
    ) 'history transfer Cargo manifest'
    Assert-OmitsAll ($CliCargo + "`n" + $DesktopCargo) @(
        'kilogram-peer-history-recovery-transport'
    ) 'active app manifests'
    Assert-OmitsAll $Cli @(
        'kilogram/m1/history-recovery/postcard/1',
        'HISTORY_RECOVERY_TRANSFER_ALPN'
    ) 'active runtime source'

    Assert-ContainsAll $Library @(
        'PreparedHistoryRecoveryTransferSession',
        'SignedHistoryRecoveryTransferSessionReceipt',
        'send_prepared_history_recovery_transfer',
        'prepared.encoded_request()',
        'SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify'
    ) 'prepared session boundary'
    Assert-ContainsAll $Outbox @(
        'old-history-recovery-transfer-outbox-v1.redb',
        'old-history-recovery-transfer-outbox-request-v1',
        'old-history-recovery-transfer-outbox-head-v1',
        'old-history-recovery-transfer-outbox-receipt-v1',
        'MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS: u64 = 16_384',
        'ReadOnlyDatabase::open(database_path)',
        '.set_durability(Durability::Immediate)',
        'existing.encoded_request() == prepared.encoded_request()',
        'TransferAlreadyPrepared', 'ReceiptConflict',
        'SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify',
        'fail_before_commit', 'InjectedBeforeCommit',
        'outbox_restart_preserves_exact_prepared_request',
        'outbox_acknowledgement_and_exact_replay_are_idempotent',
        'outbox_rejects_competing_intent_and_receipt',
        'outbox_uncommitted_transactions_leave_no_ambiguous_state'
    ) 'history transfer outbox implementation'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Before any network attempt',
        'persists its exact signed bytes',
        'Read-only `open_existing` never creates a database',
        'does not itself open a connection',
        'Aggregate accepted-tag readiness contains 150 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "history transfer outbox RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $cargoPath, $libraryPath, $outboxPath, $cliCargoPath, $cliPath,
    $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath $cargoPath -Raw
$library = Get-Content -LiteralPath $libraryPath -Raw
$outbox = Get-Content -LiteralPath $outboxPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $cargo, $library,
    $outbox, $cliCargo, $cli, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.prepared_request.new_nonce_on_restart = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.prepared_request.competing_request_for_transfer_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.store.max_intents = 16385
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.store.read_only_open_existing_no_create = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.receipt.different_receipt_after_commit_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.observation.raw_identity_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.authority.background_scheduler_added = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '149/149'
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $rfc }
    $changedOutbox = $outbox.Replace('TransferAlreadyPrepared', 'TransferCanBeReplaced')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $cargo $library $changedOutbox $cliCargo $cli $desktopCargo $rfc }
    $changedRfc = $rfc.Replace('does not itself open a connection', 'opens a connection itself')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $cargo $library $outbox $cliCargo $cli $desktopCargo $changedRfc }

    $rejected = 0
    $mutationIndex = 0
    foreach ($mutation in $mutations) {
        $mutationIndex++
        try { & $mutation; throw "negative mutation $mutationIndex unexpectedly passed" }
        catch {
            if ($_.Exception.Message -ceq "negative mutation $mutationIndex unexpectedly passed") { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_transfer_outbox_boundary=verified'
Write-Output 'milestone=M1.2.24'
Write-Output 'durable_states=absent,pending,acknowledged'
Write-Output 'active_runtime_registration=false'
Write-Output 'public_default=legacy-only'
