[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0209-m1185-canonical-sync-v2-opt-in-dark-launch-acceptance.md'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) { throw "$Label changed" }
}

function Assert-Manifest {
    param([object]$Manifest)
    if ([UInt64]$Manifest.schema_version -ne 1 -or [string]$Manifest.milestone -cne 'M1.1.85' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$Manifest.current_state -cne 'legacy-only' -or
        -not [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.opt_in_dark_launch_manifest -cne
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json') {
        throw 'canonical migration header changed'
    }
    if ([string]$Manifest.current_encoding -cne 'postcard' -or
        [string]$Manifest.candidate_encoding -cne 'kilogram-deterministic-cbor-v2-typed-history' -or
        [string]$Manifest.candidate_protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [bool]$Manifest.candidate_identifier_registered) {
        throw 'canonical migration encoding or identifier changed'
    }
    if ([string]$Manifest.opt_in_dark_launch.authority -cne
            'device-signed-append-only-chain' -or
        [bool]$Manifest.opt_in_dark_launch.registered_by_default -or
        [bool]$Manifest.opt_in_dark_launch.dialed_by_default -or
        -not [bool]$Manifest.opt_in_dark_launch.registered_by_verified_opt_in -or
        -not [bool]$Manifest.opt_in_dark_launch.dialed_after_mutual_authenticated_selection -or
        [bool]$Manifest.opt_in_dark_launch.selected_failure_fallback_to_legacy -or
        [string]$Manifest.opt_in_dark_launch.two_host_resilience_rehearsal -cne 'verified') {
        throw 'canonical opt-in dark-launch boundary changed'
    }
    if ([string]$Manifest.history_compatible_successor.protocol_identifier -cne
            'kilogram/m1/sync/cbor/2' -or
        -not [bool]$Manifest.history_compatible_successor.identifier_allocated -or
        [string]$Manifest.history_compatible_successor.typed_event_identity -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.closed_event_carrier -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.closed_sync_frames -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.typed_session_checkpoint -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.authenticated_ticket_capability -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_runtime_state_machine -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_atomic_typed_store -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_cross_format_membership_policy -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_signed_session_bootstrap -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_signed_session_successor_store -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_atomic_runtime_transition_transaction -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_durable_client_transition_adapter -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_durable_responder_transition_adapter -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_mutual_authenticated_candidate_selection -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_mixed_version_and_downgrade_matrix -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_dual_reader_runtime_coordinator -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.dormant_deterministic_rollback_drill -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.bounded_sync_v2_decoder_mutation_corpus -cne
            'verified' -or
        [string]$Manifest.history_compatible_successor.coverage_guided_fuzz_harness -cne
            'instrumented-sustained-verified' -or
        [bool]$Manifest.history_compatible_successor.registered -or
        [bool]$Manifest.history_compatible_successor.dialed) {
        throw 'history-compatible successor boundary changed'
    }
    if ([string]$Manifest.selection.mechanism -cne
            'listener-capability-bound-requester-offer-and-listener-acceptance' -or
        [string]$Manifest.selection.capability_freshness -cne
            'ticket-v13-device-signed-authority-endpoint-requester-publication-epoch-and-prekey-validity-bound' -or
        [bool]$Manifest.selection.format_sniffing -or [bool]$Manifest.selection.parse_error_fallback -or
        [bool]$Manifest.selection.reuse_existing_identifier -or
        [string]$Manifest.selection.mutual_authenticated_candidate_selection -cne 'verified' -or
        [string]$Manifest.selection.mixed_version_and_downgrade_matrix -cne 'verified' -or
        [string]$Manifest.selection.dual_reader_runtime_coordinator -cne 'verified' -or
        [string]$Manifest.selection.rollback_drill -cne 'verified' -or
        [bool]$Manifest.selection.selection_authorizes_registration_or_dial) {
        throw 'unsafe canonical selection policy'
    }

    Assert-ExactStrings @($Manifest.states | ForEach-Object { $_.id }) @(
        'legacy-only', 'advertise-cbor-v2', 'dual-reader-prefer-legacy',
        'dual-reader-prefer-cbor', 'cbor-required'
    ) 'migration states'
    Assert-ExactStrings @($Manifest.forward_transitions | ForEach-Object { "$($_.from)->$($_.to)" }) @(
        'legacy-only->advertise-cbor-v2',
        'advertise-cbor-v2->dual-reader-prefer-legacy',
        'dual-reader-prefer-legacy->dual-reader-prefer-cbor',
        'dual-reader-prefer-cbor->cbor-required'
    ) 'forward transitions'
    if ([string]$Manifest.rollback.trigger -cne 'explicit-local-or-release-policy-only' -or
        [string]$Manifest.rollback.deterministic_drill -cne 'verified' -or
        [UInt64]$Manifest.rollback.snapshot_bytes -ne 107 -or
        -not [bool]$Manifest.rollback.explicit_one_step_only -or
        -not [bool]$Manifest.rollback.selected_or_durable_canonical_evidence_blocks) {
        throw 'rollback trigger changed'
    }
    Assert-ExactStrings @($Manifest.rollback.requires) @(
        'recorded-incident-or-test-reason', 'monotonic-migration-epoch',
        'no-parse-or-network-error-trigger'
    ) 'rollback requirements'
    Assert-ExactStrings @($Manifest.compatibility_matrix | ForEach-Object { "$($_.initiator)|$($_.responder)|$($_.result)" }) @(
        'legacy-only|legacy-only|postcard',
        'dual-reader|legacy-only|postcard',
        'legacy-only|dual-reader|postcard',
        'dual-reader|dual-reader-without-mutual-authenticated-capability|postcard',
        'dual-reader-with-authenticated-capability|dual-reader-with-authenticated-capability|kilogram-deterministic-cbor-v2-over-history-compatible-identifier'
    ) 'compatibility matrix'
    Assert-ExactStrings @($Manifest.fail_closed_conditions) @(
        'unknown-encoding-or-version', 'selected-cbor-decode-failure',
        'capability-authority-or-session-binding-mismatch',
        'selected-identifier-and-payload-format-mismatch',
        'detected-downgrade-or-migration-epoch-regression'
    ) 'fail-closed conditions'
    $requirements = @($Manifest.activation_requirements | ForEach-Object { "$($_.id)|$($_.status)" })
    Assert-ExactStrings $requirements @(
        'canonical-schema-coverage|verified',
        'parallel-protocol-identifier-allocation|verified',
        'authenticated-capability-envelope|verified',
        'authenticated-legacy-history-bridge|verified',
        'history-compatible-typed-event-carrier|verified',
        'history-compatible-canonical-sync-frames|verified',
        'history-compatible-typed-session-checkpoint|verified',
        'history-compatible-canonical-sync-generation|verified',
        'dormant-history-compatible-runtime-state-machine|verified',
        'dormant-atomic-typed-carrier-store|verified',
        'dormant-cross-format-membership-policy|verified',
        'dormant-bounded-resumable-legacy-import|verified',
        'dormant-atomic-legacy-import-checkpoint|verified',
        'dormant-signed-session-bootstrap|verified',
        'dormant-signed-session-successor-store|verified',
        'dormant-checkpoint-bound-runtime-frame-journal|verified',
        'dormant-atomic-runtime-transition-transaction|verified',
        'dormant-durable-client-transition-adapter|verified',
        'dormant-durable-responder-transition-adapter|verified',
        'mutual-authenticated-capability-selection|verified',
        'dual-reader-runtime-implementation|verified',
        'independent-crypto-conformance|verified',
        'bounded-decoder-mutation-regression|verified',
        'bounded-canonical-sync-v2-decoder-mutation-regression|verified',
        'coverage-guided-fuzz-harness|instrumented-smoke-verified',
        'sustained-coverage-guided-fuzzing|verified',
        'mixed-version-and-downgrade-tests|verified',
        'fresh-hosted-and-field-evidence|verified',
        'rollback-drill|verified',
        'device-signed-opt-in-activation-policy|verified',
        'production-runtime-dark-launch-integration|verified',
        'two-host-opt-in-resilience-rehearsal|verified',
        'aggregate-regression-and-formalization|verified'
    ) 'activation requirements'
}

foreach ($path in @($manifestPath, $rfcPath, $transportPath, $cliPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical migration file is missing: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
Assert-Manifest $manifest

$normalizedRfc = [regex]::Replace($rfc, '\s+', ' ')
foreach ($required in @(
    'verified opt-in dark launch',
    'public_default=legacy-only',
    'does not enable a public dual-reader default',
    'Neither timeout grants activation authority or permits a Postcard retry'
)) {
    if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "migration RFC is missing '$required'"
    }
}

$activeWire = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs') -Raw
if ($activeWire.IndexOf('kilogram-deterministic-cbor-v1', [StringComparison]::Ordinal) -ge 0 -or
    $activeWire.IndexOf('kilogram-deterministic-cbor-v2', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical encoding leaked into active protocol wire'
}
if ($transport.IndexOf('pub const CANONICAL_SYNC_ALPN_V1: &[u8] = b"kilogram/m1/sync/cbor/1";',
        [StringComparison]::Ordinal) -lt 0 -or
    $transport.IndexOf('pub const CANONICAL_SYNC_ALPN_V2: &[u8] = b"kilogram/m1/sync/cbor/2";',
        [StringComparison]::Ordinal) -lt 0 -or
    $cli.IndexOf('CANONICAL_SYNC_ALPN_V1', [StringComparison]::Ordinal) -ge 0) {
    throw 'canonical identifier allocation changed'
}
foreach ($required in @(
    'let mut runtime_alpns = Vec::new();',
    '.register_canonical_alpn()',
    'runtime_alpns.push(CANONICAL_SYNC_ALPN_V2.to_vec());',
    'let mut listener_alpns = Vec::new();',
    'listener_alpns.push(CANONICAL_SYNC_ALPN_V2.to_vec());',
    '.initiate_new_canonical_session()',
    'endpoint.connect(ticket.endpoint().clone(), CANONICAL_SYNC_ALPN_V2)'
)) {
    if ($cli.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "guarded canonical production path is missing '$required'"
    }
}

if ($SelfTest) {
    $fallback = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $fallback.selection.parse_error_fallback = $true
    Assert-Rejected { Assert-Manifest $fallback } 'parse_error_fallback'
    $identifier = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $identifier.candidate_protocol_identifier = 'kilogram/m1/sync/cbor/1'
    Assert-Rejected { Assert-Manifest $identifier } 'history_incompatible_candidate_identifier'
    $registered = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $registered.candidate_identifier_registered = $true
    Assert-Rejected { Assert-Manifest $registered } 'premature_identifier_registration'
    $reused = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $reused.candidate_protocol_identifier = 'kilogram/m0/sync/8'
    Assert-Rejected { Assert-Manifest $reused } 'reused_legacy_identifier'
    $transition = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $transition.forward_transitions[0].to = 'dual-reader-prefer-cbor'
    Assert-Rejected { Assert-Manifest $transition } 'skipped_migration_state'
    Write-Output 'canonical_migration_self_test=passed'
}

Write-Output 'canonical_migration_boundary=verified'
Write-Output 'migration_status=canonical-sync-v2-opt-in-dark-launch-verified'
Write-Output 'candidate_protocol_identifier=kilogram/m1/sync/cbor/2'
Write-Output 'history_compatible_successor_identifier=kilogram/m1/sync/cbor/2'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'format_sniffing=false'
Write-Output 'parse_error_fallback=false'
Write-Output 'public_default=legacy-only'
Write-Output 'opt_in_active_wire_changed=true'
Write-Output 'active_wire_changed=true'
Write-Output 'network_evidence_executed=true'
