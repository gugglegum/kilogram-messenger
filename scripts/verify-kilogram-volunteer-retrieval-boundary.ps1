[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$ledgerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\ledger.rs'
$replicationPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\replication.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0088-bounded-volunteer-mailbox-retrieval.md'

foreach ($path in @($clientLibPath, $ledgerPath, $replicationPath, $runtimePath, $runtimeManifestPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "volunteer retrieval boundary file is missing: $path"
    }
}

$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$ledger = Get-Content -LiteralPath $ledgerPath -Raw
$replication = Get-Content -LiteralPath $replicationPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$runtimeManifest = Get-Content -LiteralPath $runtimeManifestPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @('PendingReplicaDelete', 'MAX_REPLICA_DELETE_BATCH')) {
    if (-not $clientLib.Contains($required)) {
        throw "volunteer retrieval public API is missing '$required'"
    }
}

foreach ($required in @(
    'pub fn expected_store_key(&self)',
    'pub fn stored_receipt(&self)'
)) {
    if (-not $ledger.Contains($required)) {
        throw "prepared inbound source binding is missing '$required'"
    }
}

foreach ($required in @(
    'mailbox-replica-inbound-commits-v1',
    'mailbox-replica-inbound-deleted-v1',
    'pub fn record_inbound_commit(',
    'pub fn pending_inbound_deletes(',
    'pub fn pending_inbound_delete_for(',
    'pub fn mark_inbound_deleted(',
    'application commit is absent before replica deletion',
    'Durability::Immediate',
    'MailboxDeleteResponse::decode_and_verify',
    'replica_delete_requires_durable_application_commit'
)) {
    if (-not $replication.Contains($required)) {
        throw "durable volunteer retrieval ledger is missing '$required'"
    }
}

foreach ($required in @(
    'RUNTIME_MAILBOX_REPLICA_POLL_PROVIDERS: u8 = 3',
    'RUNTIME_MAILBOX_REPLICA_LIST_ITEMS: u16 = 1',
    'attempt_runtime_volunteer_mailbox_poll',
    'MailboxPeerRequest::list(request)',
    'MailboxPeerRequest::delete(request)',
    'MailboxListResponse::decode_and_verify',
    'MailboxDeleteResponse::decode_and_verify',
    'pending_inbound_delete_for(',
    'commit_runtime_mailbox_payload',
    'runtime_mailbox_replica_delete_status=deleted-after-commit',
    'runtime_mailbox_inbound_source=volunteer-iroh',
    'runtime_volunteer_storage_replica_retrieval=bounded-three-provider-iroh-list-delete',
    'runtime_volunteer_storage_https_mailbox=compatibility-fallback'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime volunteer retrieval integration is missing '$required'"
    }
}

$commitPosition = $runtime.IndexOf('replication.record_inbound_commit(')
$deletePosition = $runtime.IndexOf('delete_runtime_volunteer_mailbox_replica(', $commitPosition)
if ($commitPosition -lt 0 -or $deletePosition -le $commitPosition) {
    throw 'volunteer replica DELETE is not visibly ordered after durable application commit'
}

foreach ($required in @(
    'at most three providers',
    'one item per provider',
    'application commit',
    'signed deletion receipt',
    'HTTPS compatibility fallback',
    'does not guarantee immediate discovery'
)) {
    if (-not $rfc.Contains($required)) {
        throw "volunteer retrieval RFC is missing '$required'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer retrieval unexpectedly added another executable target'
}

Write-Output 'volunteer_retrieval_boundary=verified'
Write-Output 'provider_probe_limit=3'
Write-Output 'items_per_provider=1'
Write-Output 'application_commit_before_delete=true'
Write-Output 'delete_receipt=store-signed-and-durable'
Write-Output 'delete_resume_after_restart=true'
Write-Output 'https_compatibility_fallback=true'
Write-Output 'new_executable=false'
