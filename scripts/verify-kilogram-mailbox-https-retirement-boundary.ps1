[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ledgerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\ledger.rs'
$replicationPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\replication.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0094-exact-mailbox-https-copy-retirement.md'

foreach ($path in @($runtimePath, $ledgerPath, $replicationPath, $manifestPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "HTTPS compatibility retirement boundary file is missing: $path"
    }
}

$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ledger = Get-Content -LiteralPath $ledgerPath -Raw
$replication = Get-Content -LiteralPath $replicationPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'RuntimeMailboxHttpsCompatibilityCopy',
    'RetainedLegacyCapability',
    'RetainedIncompleteExactReplication',
    'RetainedCompatibilityOnlyPayload',
    'SuppressedExactVolunteerDurability',
    'runtime_mailbox_https_compatibility_copy=',
    'runtime_mailbox_http_put=not-attempted',
    'runtime_mailbox_http_put=attempted',
    'runtime_mailbox_exact_completion_status=failed',
    'runtime_mailbox_delivery_durability=exact-volunteer-replication',
    'runtime_mailbox_delivery_durability=https-compatibility',
    'mark_outbound_replicated(',
    'https_compatibility_copy_is_suppressed_only_after_exact_volunteer_durability'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime HTTPS compatibility retirement boundary is missing '$required'"
    }
}

$uploadStart = $runtime.IndexOf('async fn upload_runtime_mailbox_request(')
$uploadEnd = $runtime.IndexOf('async fn prepare_orphan_runtime_mailbox_dispatch(', $uploadStart)
if ($uploadStart -lt 0 -or $uploadEnd -le $uploadStart) {
    throw 'could not isolate runtime mailbox upload implementation'
}
$upload = $runtime.Substring($uploadStart, $uploadEnd - $uploadStart)
$replicationAttempt = $upload.IndexOf('attempt_runtime_volunteer_mailbox_replication(')
$replicationCommit = $upload.IndexOf('mark_outbound_replicated(')
$suppressionEvidence = $upload.IndexOf('"runtime_mailbox_https_compatibility_copy={}"', $replicationCommit)
$httpPut = $upload.IndexOf('MailboxHttpClient::new(')
if ($replicationAttempt -lt 0 -or $replicationCommit -le $replicationAttempt -or
    $suppressionEvidence -le $replicationCommit -or $httpPut -le $suppressionEvidence) {
    throw 'exact volunteer durability is not proven and committed before the HTTPS decision'
}
foreach ($required in @(
    'status.plan.dispatch_binding() == &dispatch_binding',
    'durable_locator.commitment_id() == &expected_locator.commitment_id',
    'status.is_satisfied()',
    'if !exact_replication_attempted'
)) {
    if (-not $upload.Contains($required)) {
        throw "exact volunteer suppression guard is missing '$required'"
    }
}
if ($upload -notmatch 'durable_locator\.store_keys\(\)\s*==\s*expected_locator\.store_keys\.as_slice\(\)') {
    throw 'exact volunteer suppression guard does not bind the durable and authenticated store-key sets'
}

foreach ($required in @(
    'REPLICATED_OUTBOUND_RECORD_PREFIX',
    'ReplicatedOutboundCommit',
    'expected_service_store_key',
    'replica_set_commitment_id',
    'replica_set_store_keys',
    'required_receipts',
    'transport_identities.insert(evidence.transport_identity)',
    'evidence.receipt.verify(request.envelope())?',
    'MailboxOutboundState::Replicated',
    'exact_volunteer_receipts_atomically_replace_pending_https_upload',
    'two receipts from one transport must not suppress HTTPS'
)) {
    if (-not $ledger.Contains($required)) {
        throw "durable exact volunteer commit boundary is missing '$required'"
    }
}
if (-not $ledger.Contains('Self::Https(StoredOutboundReceipt::decode(bytes)?)')) {
    throw 'legacy HTTPS stored-receipt decoding is no longer backward compatible'
}

$commitStart = $ledger.IndexOf('pub fn mark_outbound_replicated(')
$commitEnd = $ledger.IndexOf('pub fn prepare_inbound(', $commitStart)
if ($commitStart -lt 0 -or $commitEnd -le $commitStart) {
    throw 'could not isolate exact volunteer durable commit implementation'
}
$commit = $ledger.Substring($commitStart, $commitEnd - $commitStart)
$insert = $commit.IndexOf('.insert(key.as_slice(), encoded.as_slice())?')
$remove = $commit.IndexOf('.remove(key.as_slice())?')
$durableCommit = $commit.IndexOf('commit exact volunteer replicated outbound mailbox item')
if ($insert -lt 0 -or $remove -le $insert -or $durableCommit -le $remove) {
    throw 'pending-to-replicated transition is not one ordered durable Redb transaction'
}

foreach ($required in @(
    'pub fn qualifying_receipts(&self)',
    'locator.store_keys()',
    '.binary_search(&receipt.store_key())',
    'self.qualifying_receipt_count() >= usize::from(self.plan.required_receipts())',
    'legacy_only_status.qualifying_receipt_count(), 0'
)) {
    if (-not $replication.Contains($required)) {
        throw "exact locator receipt qualification is missing '$required'"
    }
}

foreach ($required in @(
    'two transport-distinct signed volunteer receipts',
    'does not call HTTPS',
    'legacy capability',
    'incomplete exact replication',
    'reverse acknowledgement',
    'compatibility copy retroactively',
    'no new executable',
    'no IPC schema change'
)) {
    if (-not $rfc.Contains($required)) {
        throw "HTTPS compatibility retirement RFC is missing '$required'"
    }
}

if (Select-String -LiteralPath $manifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'HTTPS compatibility retirement unexpectedly added another executable target'
}

Write-Output 'mailbox_https_retirement_boundary=verified'
Write-Output 'suppression=exact-authenticated-plus-two-transport-distinct-signed-receipts'
Write-Output 'legacy_https_compatibility=retained'
Write-Output 'incomplete_exact_https_compatibility=retained'
Write-Output 'reverse_ack_https_compatibility=retained'
Write-Output 'retroactive_https_delete=false'
Write-Output 'new_executable=false'
Write-Output 'ipc_schema_change=false'
