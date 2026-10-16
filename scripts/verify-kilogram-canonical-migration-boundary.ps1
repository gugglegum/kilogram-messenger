[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0132-m116-canonical-dual-reader-migration.md'
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
    if ([UInt64]$Manifest.schema_version -ne 1 -or [string]$Manifest.milestone -cne 'M1.1.55' -or
        [string]$Manifest.status -cne 'history-compatible-successor-session-verified-not-active' -or
        [string]$Manifest.current_state -cne 'legacy-only' -or [bool]$Manifest.changes_active_wire) {
        throw 'canonical migration header changed'
    }
    if ([string]$Manifest.current_encoding -cne 'postcard' -or
        [string]$Manifest.candidate_encoding -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Manifest.candidate_protocol_identifier -cne 'kilogram/m1/sync/cbor/1' -or
        [bool]$Manifest.candidate_identifier_registered) {
        throw 'canonical migration encoding or identifier changed'
    }
    if ([string]$Manifest.history_compatible_successor.protocol_identifier -cne
            'kilogram/m1/sync/cbor/2' -or
        -not [bool]$Manifest.history_compatible_successor.identifier_allocated -or
        [string]$Manifest.history_compatible_successor.typed_event_identity -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.closed_event_carrier -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.closed_sync_frames -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.typed_session_checkpoint -cne 'verified' -or
        [string]$Manifest.history_compatible_successor.authenticated_ticket_capability -cne
            'pending' -or
        [bool]$Manifest.history_compatible_successor.registered -or
        [bool]$Manifest.history_compatible_successor.dialed) {
        throw 'history-compatible successor boundary changed'
    }
    if ([string]$Manifest.selection.mechanism -cne 'authenticated-capability-bound-explicit-identifier' -or
        [string]$Manifest.selection.capability_freshness -cne
            'ticket-v12-device-signed-authority-endpoint-requester-publication-epoch-and-prekey-validity-bound' -or
        [bool]$Manifest.selection.format_sniffing -or [bool]$Manifest.selection.parse_error_fallback -or
        [bool]$Manifest.selection.reuse_existing_identifier) { throw 'unsafe canonical selection policy' }

    Assert-ExactStrings @($Manifest.states | ForEach-Object { $_.id }) @(
        'legacy-only', 'advertise-cbor-v1', 'dual-reader-prefer-legacy',
        'dual-reader-prefer-cbor', 'cbor-required'
    ) 'migration states'
    Assert-ExactStrings @($Manifest.forward_transitions | ForEach-Object { "$($_.from)->$($_.to)" }) @(
        'legacy-only->advertise-cbor-v1',
        'advertise-cbor-v1->dual-reader-prefer-legacy',
        'dual-reader-prefer-legacy->dual-reader-prefer-cbor',
        'dual-reader-prefer-cbor->cbor-required'
    ) 'forward transitions'
    if ([string]$Manifest.rollback.trigger -cne 'explicit-local-or-release-policy-only') {
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
        'dual-reader-with-authenticated-capability|dual-reader-with-authenticated-capability|kilogram-deterministic-cbor-v1-over-new-identifier'
    ) 'compatibility matrix'
    Assert-ExactStrings @($Manifest.fail_closed_conditions) @(
        'unknown-encoding-or-version', 'selected-cbor-decode-failure',
        'capability-authority-or-session-binding-mismatch',
        'selected-identifier-and-payload-format-mismatch',
        'detected-downgrade-or-migration-epoch-regression'
    ) 'fail-closed conditions'
    $requirements = @($Manifest.activation_requirements | ForEach-Object { "$($_.id)|$($_.status)" })
    Assert-ExactStrings $requirements @(
        'canonical-schema-coverage|pending',
        'parallel-protocol-identifier-allocation|verified',
        'authenticated-capability-envelope|verified',
        'authenticated-legacy-history-bridge|verified',
        'history-compatible-typed-event-carrier|verified',
        'history-compatible-canonical-sync-frames|verified',
        'history-compatible-typed-session-checkpoint|verified',
        'history-compatible-canonical-sync-generation|pending',
        'dual-reader-runtime-implementation|pending',
        'independent-crypto-conformance|verified',
        'bounded-decoder-mutation-regression|verified',
        'sustained-coverage-guided-fuzzing|pending',
        'mixed-version-and-downgrade-tests|pending',
        'fresh-hosted-and-field-evidence|pending',
        'rollback-drill|pending'
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

foreach ($required in @(
    'Format sniffing is forbidden', 'fails closed', 'must never be reused',
    'never trigger rollback',
    'Activation remains blocked', 'changes no active wire bytes'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "migration RFC is missing '$required'"
    }
}

$activeWire = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs') -Raw
if ($activeWire.IndexOf('kilogram-deterministic-cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical encoding leaked into active protocol wire'
}
if ($transport.IndexOf('pub const CANONICAL_SYNC_ALPN_V1: &[u8] = b"kilogram/m1/sync/cbor/1";',
        [StringComparison]::Ordinal) -lt 0 -or
    $cli.IndexOf('CANONICAL_SYNC_ALPN_V1', [StringComparison]::Ordinal) -ge 0) {
    throw 'allocated canonical identifier is missing or active prematurely'
}

if ($SelfTest) {
    $fallback = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $fallback.selection.parse_error_fallback = $true
    Assert-Rejected { Assert-Manifest $fallback } 'parse_error_fallback'
    $identifier = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $identifier.candidate_protocol_identifier = 'kilogram/m1/sync/cbor/2'
    Assert-Rejected { Assert-Manifest $identifier } 'changed_candidate_identifier'
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
Write-Output 'migration_status=history-compatible-successor-session-verified-not-active'
Write-Output 'candidate_protocol_identifier=kilogram/m1/sync/cbor/1'
Write-Output 'history_compatible_successor_identifier=kilogram/m1/sync/cbor/2'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'format_sniffing=false'
Write-Output 'parse_error_fallback=false'
Write-Output 'active_wire_changed=false'
Write-Output 'network_executed=false'
