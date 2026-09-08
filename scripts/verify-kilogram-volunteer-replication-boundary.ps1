[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$clientManifestPath = Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$replicationPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\replication.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeMailboxPath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_mailbox.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0087-resumable-volunteer-mailbox-replication.md'

foreach ($path in @(
    $clientManifestPath,
    $clientLibPath,
    $replicationPath,
    $runtimePath,
    $runtimeMailboxPath,
    $runtimeManifestPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "volunteer replication boundary file is missing: $path"
    }
}

$clientManifest = Get-Content -LiteralPath $clientManifestPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$replication = Get-Content -LiteralPath $replicationPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$runtimeMailbox = Get-Content -LiteralPath $runtimeMailboxPath -Raw

foreach ($required in @(
    'MailboxReplicationLedger',
    'MailboxReplicationLedgerConfig',
    'DEFAULT_REPLICATION_TARGETS',
    'DEFAULT_REQUIRED_REPLICA_RECEIPTS',
    'DEFAULT_REPLICATION_RETRY_SECONDS'
)) {
    if (-not $clientLib.Contains($required)) {
        throw "mailbox replication public API is missing '$required'"
    }
}

foreach ($required in @(
    'mailbox-replication-ledger.redb',
    'mailbox-replication-plans-v1',
    'mailbox-replication-receipts-v1',
    'mailbox-replication-attempts-v1',
    'DEFAULT_REPLICATION_TARGETS: u8 = 3',
    'DEFAULT_REQUIRED_REPLICA_RECEIPTS: u8 = 2',
    'DEFAULT_REPLICATION_RETRY_SECONDS: u64 = 60',
    'request: MailboxPutRequest',
    'selection_salt: [u8; 32]',
    'dispatch_binding: [u8; 32]',
    'transport_identity: [u8; 32]',
    'Durability::Immediate',
    'current.transport_identity != transport_identity',
    'MailboxPutResponse::decode_and_verify',
    'plan_and_transport_distinct_receipts_survive_restart'
)) {
    if (-not $replication.Contains($required)) {
        throw "durable volunteer replication ledger is missing '$required'"
    }
}

foreach ($forbidden in @('AccountId', 'DeviceId', 'ConversationId')) {
    if ($replication.Contains($forbidden)) {
        throw "volunteer replication ledger unexpectedly stores social identifier '$forbidden'"
    }
}

foreach ($required in @(
    'DISPATCH_REPLICATION_BINDING_DOMAIN',
    'pub fn replication_binding(&self)'
)) {
    if (-not $runtimeMailbox.Contains($required)) {
        throw "signed outbox dispatch binding is missing '$required'"
    }
}

foreach ($required in @(
    'attempt_runtime_volunteer_mailbox_replication',
    'random_mailbox_replication_selection_salt',
    'mailbox_provider_endpoint_from_offer',
    'endpoint.connect(provider_endpoint, MAILBOX_ALPN)',
    'MailboxPeerRequest::put(request)',
    'write_mailbox_peer_request',
    'read_mailbox_peer_response',
    'offer.transport_identity() != &own_transport_identity',
    'retained_transport_identities.contains',
    'offer.max_record_bytes() >= request.envelope().len() as u64',
    'ledger.mark_attempt(&plan, now, DEFAULT_REPLICATION_RETRY_SECONDS)',
    'replication_ledger.next_due(now, DEFAULT_REPLICATION_RETRY_SECONDS)',
    'runtime_mailbox_replication_http_delivery_compatibility=true',
    'runtime_volunteer_storage_replication=sender-three-target-two-receipt',
    'runtime_volunteer_storage_replica_retrieval=https-compatible-pending-iroh-read',
    'match attempt_runtime_mailbox_fallback(endpoint, state_directory, &prepared).await'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime volunteer replication integration is missing '$required'"
    }
}

foreach ($forbiddenDependency in @('kilogram-identity', 'kilogram-protocol', 'iroh =')) {
    if ($clientManifest.Contains($forbiddenDependency)) {
        throw "mailbox replication ledger unexpectedly depends on '$forbiddenDependency'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer replication unexpectedly added another executable target'
}

Write-Output 'volunteer_replication_boundary=verified'
Write-Output 'selection_salt=durable-random-per-signed-dispatch'
Write-Output 'default_targets=3'
Write-Output 'required_independent_receipts=2'
Write-Output 'retry=durable-60-second-cooldown'
Write-Output 'carrier=existing-blind-mailbox-iroh-alpn'
Write-Output 'direct_delivery_preferred=true'
Write-Output 'https_delivery_compatibility=true'
Write-Output 'iroh_replica_retrieval=pending'
Write-Output 'new_executable=false'
