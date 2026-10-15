[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$officialPath = Join-Path $workspace 'BLAKE3-REFERENCE-VECTORS.json'
$referencePath = Join-Path $PSScriptRoot 'lib\blake3-reference.mjs'
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-blake3-conformance.mjs'
$testPath = Join-Path $workspace 'crates\kilogram-protocol\tests\canonical_decoder_mutations.rs'
$ratchetReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ratchet-reference.mjs'
$recoveryReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-reference.mjs'
$eventReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-event-reference.mjs'
$prekeySelectionReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-prekey-selection-reference.mjs'
$syncReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-reference.mjs'
$syncSessionReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-session-reference.mjs'
$recoveryExchangeReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-exchange-reference.mjs'
$mailboxReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-reference.mjs'
$mailboxTransportReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-transport-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0131-m115-independent-digests-and-decoder-mutations.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}
function Assert-Manifest {
    param([object]$Manifest)
    if ([UInt64]$Manifest.schema_version -ne 1 -or [string]$Manifest.milestone -cne 'M1.1.5' -or
        [string]$Manifest.status -cne 'bounded-regression-corpus' -or [bool]$Manifest.active_runtime_changed) {
        throw 'decoder mutation manifest header changed'
    }
    if ([int]$Manifest.seed -ne 1262836037 -or [int]$Manifest.minimum_mutations -ne 500 -or
        [int]$Manifest.expected_mutations -ne 7045 -or
        [int]$Manifest.maximum_mutations -ne 8000) { throw 'decoder mutation bounds changed' }
    $expectedFixtures = @(
        'signed-device-certificate-v1|CANONICAL-AUTHORITY-VECTORS.json|/expected/certificate/signed_cbor_hex|decode_and_verify_signed_device_certificate_v1',
        'signed-device-session-authorization-v1|CANONICAL-AUTHORITY-VECTORS.json|/expected/session_authorization/signed_cbor_hex|decode_and_verify_signed_device_session_authorization_v1',
        'signed-prekey-pool-v1|CANONICAL-RATCHET-VECTORS.json|/expected/prekey_pool/signed_cbor_hex|decode_and_verify_signed_prekey_pool_v1',
        'account-prekey-directory-v1|CANONICAL-RATCHET-VECTORS.json|/expected/account_prekey_directory_cbor_hex|decode_and_verify_account_prekey_directory_v1',
        'signed-history-recovery-checkpoint-v1|CANONICAL-RECOVERY-VECTORS.json|/expected/successor_checkpoint/signed_cbor_hex|decode_and_verify_signed_history_recovery_checkpoint_v1'
        'signed-history-recovery-link-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/link/signed_cbor_hex|decode_and_verify_signed_history_recovery_link_v1'
        'signed-history-recovery-plan-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/plan/signed_cbor_hex|decode_and_verify_signed_history_recovery_plan_v1'
        'signed-history-recovery-response-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/page_response/signed_cbor_hex|decode_and_verify_signed_history_recovery_response_v1'
        'signed-account-device-list-v1|CANONICAL-ACCOUNT-VECTORS.json|/expected/account_device_list/signed_cbor_hex|decode_and_verify_signed_account_device_list_v1'
        'signed-conversation-membership-v1|CANONICAL-ACCOUNT-VECTORS.json|/expected/conversation_membership/signed_cbor_hex|decode_and_verify_signed_conversation_membership_v1'
        'signed-device-link-authorization-v1|CANONICAL-DEVICE-LINK-VECTORS.json|/expected/authorization/signed_cbor_hex|decode_and_verify_signed_device_link_authorization_v1'
        'signed-acknowledgement-event-v1|CANONICAL-EVENT-VECTORS.json|/expected/acknowledgement_event/signed_cbor_hex|decode_and_verify_signed_event_v1'
        'authorized-text-event-v1|CANONICAL-EVENT-VECTORS.json|/expected/authorized_text_event_cbor_hex|decode_and_verify_authorized_event_v1'
        'prekey-selection-transcript-v1|CANONICAL-PREKEY-SELECTION-VECTORS.json|/expected/transcript_cbor_hex|decode_and_verify_prekey_selection_transcript_v1'
        'signed-sync-diff-v1|CANONICAL-SYNC-VECTORS.json|/expected/diff/signed_cbor_hex|decode_and_verify_signed_sync_diff_v1'
        'sync-client-frame-v1|CANONICAL-SYNC-VECTORS.json|/expected/client_frames/inventory_cbor_hex|decode_and_verify_sync_client_frame_v1'
        'sync-server-frame-v1|CANONICAL-SYNC-VECTORS.json|/expected/server_frames/diff_cbor_hex|decode_and_verify_sync_server_frame_v1'
        'signed-sync-session-checkpoint-v1|CANONICAL-SYNC-SESSION-VECTORS.json|/expected/checkpoints/3/signed_cbor_hex|decode_and_verify_signed_sync_session_checkpoint_v1'
        'signed-mailbox-write-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/write_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_write_authorization_v1'
        'signed-mailbox-read-list-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/read_list_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_read_authorization_v1'
        'signed-mailbox-read-delete-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/read_delete_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_read_authorization_v1'
        'signed-mailbox-storage-offer-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/storage_offer/signed_cbor_hex|decode_and_verify_signed_mailbox_storage_offer_v1'
        'signed-mailbox-stored-receipt-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/stored_receipt/signed_cbor_hex|decode_and_verify_signed_mailbox_stored_receipt_v1'
        'signed-mailbox-delete-receipt-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/delete_receipt/signed_cbor_hex|decode_and_verify_signed_mailbox_delete_receipt_v1'
        'mailbox-peer-put-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/put/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
        'mailbox-peer-list-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/list/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
        'mailbox-peer-delete-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/delete/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
    )
    $actualFixtures = @($Manifest.fixtures | ForEach-Object { "$($_.id)|$($_.vector_file)|$($_.json_pointer)|$($_.decoder)" })
    if (($actualFixtures -join "`n") -cne ($expectedFixtures -join "`n")) { throw 'decoder fixture inventory changed' }
    $expectedClasses = @(
        'bounded-strict-prefix-truncation-grid', 'bounded-low-bit-flip-grid', 'trailing-byte',
        'non-minimal-top-level-version', 'indefinite-top-level-array',
        'reserved-top-level-additional-information', 'oversized-declared-byte-string'
    )
    if ((@($Manifest.mutation_classes) -join "`n") -cne ($expectedClasses -join "`n")) {
        throw 'decoder mutation classes changed'
    }
    if ([int]$Manifest.resource_bounds.maximum_fixture_bytes -ne 524288 -or
        [int]$Manifest.resource_bounds.maximum_positions_per_grid_per_fixture -ne 128 -or
        [int]$Manifest.resource_bounds.maximum_total_mutations -ne 8000 -or
        [bool]$Manifest.resource_bounds.network_allowed -or
        [bool]$Manifest.resource_bounds.randomness_allowed) { throw 'decoder resource bounds changed' }
}

foreach ($path in @($manifestPath, $officialPath, $referencePath, $verifierPath, $testPath, $ratchetReferencePath, $recoveryReferencePath, $eventReferencePath, $prekeySelectionReferencePath, $syncReferencePath, $syncSessionReferencePath, $recoveryExchangeReferencePath, $mailboxReferencePath, $mailboxTransportReferencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical conformance file is missing: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$official = Get-Content -LiteralPath $officialPath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$test = Get-Content -LiteralPath $testPath -Raw
$ratchetReference = Get-Content -LiteralPath $ratchetReferencePath -Raw
$recoveryReference = Get-Content -LiteralPath $recoveryReferencePath -Raw
$eventReference = Get-Content -LiteralPath $eventReferencePath -Raw
$prekeySelectionReference = Get-Content -LiteralPath $prekeySelectionReferencePath -Raw
$syncSessionReference = Get-Content -LiteralPath $syncSessionReferencePath -Raw
$recoveryExchangeReference = Get-Content -LiteralPath $recoveryExchangeReferencePath -Raw
$mailboxReference = Get-Content -LiteralPath $mailboxReferencePath -Raw
$mailboxTransportReference = Get-Content -LiteralPath $mailboxTransportReferencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Manifest $manifest

if ([UInt64]$official.schema_version -ne 1 -or
    [string]$official.source -cne 'https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json' -or
    [string]$official.input_pattern -cne 'byte_at_index_modulo_251' -or
    [int]$official.output_bytes_checked -ne 32) { throw 'official BLAKE3 vector metadata changed' }
$expectedLengths = @(0, 1, 1023, 1024, 1025, 2048)
$actualLengths = @($official.cases | ForEach-Object { [int]$_.input_len })
if (($actualLengths -join ',') -cne ($expectedLengths -join ',')) { throw 'official BLAKE3 case inventory changed' }
foreach ($case in $official.cases) {
    if ([string]$case.hash_hex -cnotmatch '^[0-9a-f]{64}$') { throw "invalid BLAKE3 hash for length $($case.input_len)" }
}
foreach ($required in @('const CHUNK_LEN = 1024', 'function compress(', 'function chunkOutput(',
    'function parentOutput(', 'export function blake3Hash(')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "BLAKE3 reference is missing '$required'" }
}
foreach ($forbidden in @('node:crypto', 'https:', 'fetch(', 'Invoke-WebRequest', 'npm', 'require(')) {
    if ($reference.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "BLAKE3 reference contains forbidden dependency/network surface '$forbidden'"
    }
}
foreach ($required in @('official_vectors_checked=', 'kilogram_digest_vectors_checked=5',
    'CANONICAL-WIRE-VECTORS.json', 'CANONICAL-RATCHET-VECTORS.json', 'CANONICAL-RECOVERY-VECTORS.json')) {
    if ($verifier.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "digest verifier is missing '$required'" }
}
foreach ($fixture in $manifest.fixtures) {
    if ($test.IndexOf([string]$fixture.decoder, [StringComparison]::Ordinal) -lt 0) {
        throw "mutation test is missing decoder '$($fixture.decoder)'"
    }
}
foreach ($required in @('MAXIMUM_GRID_POSITIONS: usize = 128', 'MINIMUM_MUTATIONS: usize = 500',
    'EXPECTED_MUTATIONS: usize = 7_045', 'MAXIMUM_MUTATIONS: usize = 8_000',
    'bounded_positions(', 'canonical_decoder_mutations=')) {
    if ($test.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "mutation test is missing '$required'" }
}
foreach ($referenceSource in @($ratchetReference, $recoveryReference, $eventReference, $prekeySelectionReference, $syncSessionReference, $recoveryExchangeReference, $mailboxReference, $mailboxTransportReference)) {
    if ($referenceSource.IndexOf('independent_blake3=verified', [StringComparison]::Ordinal) -lt 0 -or
        $referenceSource.IndexOf('blake3Hash(', [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical reference does not independently verify BLAKE3'
    }
}
foreach ($required in @('not a substitute for', 'deterministic, offline and bounded', 'does not activate canonical CBOR')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw "RFC is missing '$required'" }
}

$output = @(& node $verifierPath $workspace 2>&1)
if ($LASTEXITCODE -ne 0 -or 'blake3_reference_conformance=verified' -notin @($output | ForEach-Object { [string]$_ })) {
    throw "independent BLAKE3 conformance failed`n$($output -join [Environment]::NewLine)"
}
if ($SelfTest) {
    $changed = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.maximum_mutations = 8001
    Assert-Rejected { Assert-Manifest $changed } 'changed_mutation_bound'
    $removed = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.fixtures = @($removed.fixtures | Select-Object -Skip 1)
    Assert-Rejected { Assert-Manifest $removed } 'removed_decoder_fixture'
    Write-Output 'canonical_conformance_self_test=passed'
}
Write-Output 'canonical_conformance_boundary=verified'
Write-Output 'independent_blake3=verified'
Write-Output 'official_blake3_vectors=6/6'
Write-Output 'bounded_decoder_mutations=7045'
Write-Output 'coverage_guided_fuzzing=pending-before-activation'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
