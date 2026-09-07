[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$clientManifestPath = Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$runtimeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'

$clientManifest = Get-Content -LiteralPath $clientManifestPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$provider = Get-Content -LiteralPath $providerPath -Raw
$runtimeIpc = Get-Content -LiteralPath $runtimeIpcPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw

foreach ($required in @(
    'MailboxProviderRegistry',
    'MailboxProviderRegistryConfig',
    'MailboxProviderImportOutcome',
    'MAX_PROVIDER_SELECTION'
)) {
    if (-not $clientLib.Contains($required)) {
        throw "mailbox client provider API is missing '$required'"
    }
}

foreach ($required in @(
    'mailbox-provider-registry.redb',
    'DEFAULT_MAX_PROVIDER_OFFERS: u64 = 256',
    'MAX_PROVIDER_SELECTION: u8 = 8',
    'SignedMailboxStorageOffer::decode_and_verify',
    'canonical == encoded_offer',
    'record.issued_at_unix_seconds > current.issued_at_unix_seconds',
    'Durability::Immediate',
    'SELECTION_DOMAIN',
    'selected_identities.insert(*offer.transport_identity())',
    'registry_is_bounded_monotonic_and_prunes_expired_offers',
    'deterministic_selection_deduplicates_transport_identities',
    'import_rejects_tamper_expiry_and_noncanonical_identity_replay'
)) {
    if (-not $provider.Contains($required)) {
        throw "bounded volunteer provider registry is missing '$required'"
    }
}

foreach ($forbiddenDependency in @(
    'kilogram-identity',
    'kilogram-protocol',
    'iroh ='
)) {
    if ($clientManifest.Contains($forbiddenDependency)) {
        throw "mailbox provider registry unexpectedly depends on '$forbiddenDependency'"
    }
}

foreach ($required in @(
    'const IPC_VERSION: u8 = 24',
    'ImportVolunteerStorageOffer',
    'SelectVolunteerStorageProviders',
    'RuntimeIpcVolunteerStorageProvider',
    'RuntimeIpcVolunteerStorageOfferImport',
    'RuntimeIpcVolunteerStorageProviderSet',
    'volunteer_provider_ipc_is_bounded_and_capability_free'
)) {
    if (-not $runtimeIpc.Contains($required)) {
        throw "authenticated runtime provider IPC is missing '$required'"
    }
}

foreach ($required in @(
    'MAILBOX_PROVIDER_TRANSPORT_IDENTITY_DOMAIN',
    'mailbox_provider_endpoint_from_offer',
    'endpoint.id.to_string().as_bytes()',
    'runtime_mailbox_provider_registry',
    'import_runtime_volunteer_storage_offer',
    'select_runtime_volunteer_storage_providers',
    'with_locked_state(state_directory',
    'runtime_volunteer_storage_discovery=verified-expiring-offer-registry',
    'runtime_volunteer_storage_selection=deterministic-transport-distinct',
    'runtime_provider_import_and_selection_are_capability_free_and_deterministic'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime volunteer provider integration is missing '$required'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer provider selection unexpectedly added another executable target'
}

Write-Output 'volunteer_provider_selection_boundary=verified'
Write-Output 'registry=durable-bounded-signed-offers'
Write-Output 'default_registry_capacity=256'
Write-Output 'maximum_selection=8'
Write-Output 'selection=deterministic-transport-distinct'
Write-Output 'capability_linkage=false'
Write-Output 'automatic_gossip=false'
Write-Output 'replication=false'
Write-Output 'new_executable=false'
