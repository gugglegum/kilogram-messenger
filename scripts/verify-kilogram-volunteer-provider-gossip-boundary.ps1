[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$clientManifestPath = Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'

$clientManifest = Get-Content -LiteralPath $clientManifestPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$provider = Get-Content -LiteralPath $providerPath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw

foreach ($required in @(
    'MailboxProviderGossipFrame',
    'MAX_PROVIDER_GOSSIP_OFFERS',
    'MAX_PROVIDER_GOSSIP_HOPS',
    'MAX_PROVIDER_GOSSIP_AGE_SECONDS',
    'MAX_PROVIDER_GOSSIP_OFFER_BYTES',
    'MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS'
)) {
    if (-not $clientLib.Contains($required)) {
        throw "mailbox provider gossip API is missing '$required'"
    }
}

foreach ($required in @(
    'MAX_PROVIDER_GOSSIP_OFFERS: u8 = 8',
    'MAX_PROVIDER_GOSSIP_HOPS: u8 = 2',
    'MAX_PROVIDER_GOSSIP_AGE_SECONDS: u64 = 15 * 60',
    'MAX_PROVIDER_GOSSIP_OFFER_BYTES: usize = 2 * 1024',
    'MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS: u64 = 60',
    'mailbox-provider-gossip-hops-v1',
    'import_gossiped_offer',
    'select_for_gossip',
    'if observed_gossip_hops == 0',
    'offer.observed_gossip_hops < MAX_PROVIDER_GOSSIP_HOPS',
    'gossip_frame_is_short_lived_bounded_and_reply_bound',
    'gossip_hops_and_age_stop_amplification'
)) {
    if (-not $provider.Contains($required)) {
        throw "bounded mailbox provider gossip state is missing '$required'"
    }
}

foreach ($forbiddenDependency in @(
    'kilogram-identity',
    'kilogram-protocol',
    'iroh ='
)) {
    if ($clientManifest.Contains($forbiddenDependency)) {
        throw "mailbox provider gossip unexpectedly depends on '$forbiddenDependency'"
    }
}

foreach ($forbiddenType in @(
    'AccountId',
    'DeviceId',
    'ConversationId',
    'MailboxId',
    'MailboxWriteAuthorization',
    'MailboxReadAuthorization'
)) {
    if ($provider.Contains($forbiddenType)) {
        throw "provider gossip payload/state unexpectedly names social or capability type '$forbiddenType'"
    }
}

foreach ($required in @(
    'MAX_MAILBOX_PROVIDER_GOSSIP_WIRE_BYTES: usize = 24 * 1024',
    'MailboxProviderGossip(Vec<u8>)',
    'MailboxProviderGossipAcknowledged(Vec<u8>)',
    'MailboxProviderGossipRejected',
    'mailbox_provider_gossip_frames_are_bounded_and_round_trip'
)) {
    if (-not $protocol.Contains($required)) {
        throw "authenticated protocol gossip carrier is missing '$required'"
    }
}

foreach ($required in @(
    'RUNTIME_MAILBOX_PROVIDER_GOSSIP_INTERVAL: Duration = Duration::from_secs(5 * 60)',
    'RUNTIME_MAILBOX_PROVIDER_OFFER_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60)',
    'import_runtime_mailbox_provider_gossip',
    'attempt_runtime_mailbox_provider_gossip',
    'send_runtime_mailbox_provider_gossip',
    'last_mailbox_provider_gossip_check.elapsed()',
    'min_by_key(|contact| last_attempts.get(&contact.contact_id()).copied())',
    'Some(request_frame.frame_id()?)',
    'runtime_volunteer_storage_offer_distribution=authenticated-bounded-peer-gossip',
    'runtime_volunteer_storage_policy_refresh=automatic-five-minutes',
    'runtime_volunteer_storage_offer_gossip_eligible=',
    'runtime_mailbox_provider_gossip_payload_social_ids=false',
    'runtime_provider_gossip_is_reply_bound_and_imports_verified_endpoints'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime provider gossip integration is missing '$required'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'provider gossip unexpectedly added another executable target'
}

Write-Output 'volunteer_provider_gossip_boundary=verified'
Write-Output 'carrier=existing-device-authenticated-iroh-session'
Write-Output 'maximum_offers_per_exchange=8'
Write-Output 'maximum_hops=2'
Write-Output 'maximum_offer_age_seconds=900'
Write-Output 'maximum_offer_bytes=2048'
Write-Output 'maximum_frame_validity_seconds=60'
Write-Output 'maximum_frame_bytes=20480'
Write-Output 'maximum_exchange_frequency=one-per-five-minutes'
Write-Output 'global_directory=false'
Write-Output 'social_ids_in_payload=false'
Write-Output 'mailbox_capabilities_in_payload=false'
Write-Output 'automatic_gossip=true'
Write-Output 'replication=false'
Write-Output 'new_executable=false'
