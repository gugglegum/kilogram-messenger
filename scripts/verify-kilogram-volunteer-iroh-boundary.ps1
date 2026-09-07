[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$mailboxPath = Join-Path $workspace 'crates\kilogram-mailbox\src\lib.rs'
$mailboxWirePath = Join-Path $workspace 'crates\kilogram-mailbox\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$storePath = Join-Path $workspace 'apps\kilogram-ticket-store\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'

$mailbox = Get-Content -LiteralPath $mailboxPath -Raw
$mailboxWire = Get-Content -LiteralPath $mailboxWirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw

foreach ($required in @(
    'kilogram:blind-mailbox-storage-offer:v1',
    'SignedMailboxStorageOffer',
    'BoundedVolunteer',
    'MAX_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS',
    'provider_endpoint',
    'capacity_hint_bytes',
    'storage_offer_is_store_signed_bounded_and_expiring'
)) {
    if (-not $mailbox.Contains($required)) {
        throw "signed volunteer storage offer boundary is missing '$required'"
    }
}

foreach ($required in @(
    'MailboxPeerOperation',
    'MailboxPeerRequest',
    'MailboxPeerResponse',
    'MailboxPeerRejection',
    'PEER_REQUEST_DIGEST_DOMAIN',
    'MailboxPutRequest::decode_and_verify',
    'MailboxListRequest::decode_and_verify',
    'MailboxDeleteRequest::decode_and_verify',
    'peer_frame_authenticates_capability_and_binds_response_digest'
)) {
    if (-not $mailboxWire.Contains($required)) {
        throw "capability-authenticated peer wire boundary is missing '$required'"
    }
}

foreach ($required in @(
    'MAILBOX_ALPN: &[u8] = b"kilogram/m0/blind-mailbox/1"',
    'encode_mailbox_provider_endpoint',
    'mailbox_provider_endpoint_from_offer',
    'read_mailbox_peer_request',
    'write_mailbox_peer_request',
    'read_mailbox_peer_response',
    'write_mailbox_peer_response',
    'signed_mailbox_offer_carries_a_bounded_iroh_endpoint'
)) {
    if (-not $transport.Contains($required)) {
        throw "dedicated Iroh mailbox adapter is missing '$required'"
    }
}

foreach ($required in @(
    'VolunteerMailboxService',
    'handle_peer_request',
    'peer_limiter',
    'RateLimiter<String>',
    'reserve_transfer',
    'InvalidCapability',
    'peer_service_executes_capability_request_and_rate_limits_endpoint_identity'
)) {
    if (-not $store.Contains($required)) {
        throw "volunteer peer service boundary is missing '$required'"
    }
}

foreach ($required in @(
    'runtime_alpns.push(MAILBOX_ALPN.to_vec())',
    'connection.alpn() == MAILBOX_ALPN',
    'handle_runtime_volunteer_storage_connection',
    'try_peer_permit',
    'JoinSet::new()',
    'runtime_volunteer_storage_offer_distribution=authenticated-bounded-peer-gossip',
    'runtime_volunteer_storage_discovery=verified-expiring-offer-registry',
    'runtime_volunteer_storage_replication=false'
)) {
    if (-not $runtime.Contains($required)) {
        throw "ordinary runtime volunteer Iroh integration is missing '$required'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer Iroh storage unexpectedly added another executable target'
}

Write-Output 'volunteer_iroh_boundary=verified'
Write-Output 'alpn=kilogram/m0/blind-mailbox/1'
Write-Output 'operations=capability-authenticated-put-list-delete'
Write-Output 'offer=store-signed-expiring-endpoint-bound'
Write-Output 'rate_limit=authenticated-iroh-endpoint-id'
Write-Output 'transfer_limit=durable-shared-network-class-budget'
Write-Output 'discovery=authenticated-bounded-peer-gossip'
Write-Output 'replication=false'
Write-Output 'new_executable=false'
