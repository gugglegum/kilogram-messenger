[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$provisioningPath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$replicationPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\replication.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0091-authenticated-volunteer-replica-set-locator.md'

foreach ($path in @(
    $provisioningPath,
    $clientLibPath,
    $providerPath,
    $replicationPath,
    $runtimePath,
    $runtimeManifestPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "volunteer replica-set locator boundary file is missing: $path"
    }
}

$provisioning = Get-Content -LiteralPath $provisioningPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$provider = Get-Content -LiteralPath $providerPath -Raw
$replication = Get-Content -LiteralPath $replicationPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$runtimeManifest = Get-Content -LiteralPath $runtimeManifestPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'MailboxReplicaSetCommitment',
    'ActivateWithReplicaSet',
    'activate_with_replica_set',
    'replica_set_commitment_id',
    'MIN_MAILBOX_REPLICA_SET_STORES: usize = 2',
    'MAX_MAILBOX_REPLICA_SET_STORES: usize = 8',
    'capability_update_authenticates_canonical_replica_set'
)) {
    if (-not $provisioning.Contains($required)) {
        throw "authenticated mailbox replica-set contract is missing '$required'"
    }
}

$commitmentStart = $provisioning.IndexOf('pub struct MailboxReplicaSetCommitment')
$commitmentEnd = $provisioning.IndexOf('enum MailboxCapabilityUpdateAction', $commitmentStart)
if ($commitmentStart -lt 0 -or $commitmentEnd -le $commitmentStart) {
    throw 'mailbox replica-set commitment boundary cannot be isolated'
}
$commitmentContract = $provisioning.Substring($commitmentStart, $commitmentEnd - $commitmentStart)
foreach ($forbidden in @('AccountId', 'DeviceId', 'ConversationId', 'MailboxId', 'write_secret')) {
    if ($commitmentContract.Contains($forbidden)) {
        throw "replica-set commitment unexpectedly contains '$forbidden'"
    }
}

foreach ($required in @(
    'active_offers_for_store_keys',
    'active_offers_for_store_keys_read_only',
    'indexed lookups instead of sampling',
    'exact_replica_set_lookup_is_indexed_bounded_and_expiry_aware'
)) {
    if (-not $provider.Contains($required)) {
        throw "exact provider lookup is missing '$required'"
    }
}

foreach ($required in @(
    'MailboxReplicaSetLocator',
    'mailbox-replica-set-locators-v1',
    'pub fn ensure_replica_set_locator(',
    'mailbox replica-set locator replay conflicts',
    'removed_replica_set_locators',
    'Durability::Immediate'
)) {
    if (-not $replication.Contains($required) -and -not $clientLib.Contains($required)) {
        throw "durable replica-set locator ledger is missing '$required'"
    }
}

foreach ($forbidden in @('AccountId', 'DeviceId', 'ConversationId')) {
    if ($replication.Contains($forbidden)) {
        throw "replica-set locator ledger unexpectedly stores social identifier '$forbidden'"
    }
}

foreach ($required in @(
    'RUNTIME_MAILBOX_REPLICA_LOCATOR_PROVIDERS: u8 = DEFAULT_REPLICATION_TARGETS',
    'peer_mailbox_replica_set_locator',
    'local_mailbox_replica_set_locator',
    'ensure_replica_set_locator(',
    'active_offers_for_store_keys_read_only(',
    'runtime_mailbox_replica_set_discovery=exact-authenticated',
    'runtime_mailbox_replica_set_discovery=legacy-random-fallback'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime authenticated replica-set integration is missing '$required'"
    }
}

foreach ($required in @(
    'Device-signed mailbox capability',
    'indexed lookup',
    'Account ID, Device ID, conversation ID',
    'global directory',
    'legacy-random-fallback',
    'HTTPS mailbox upload remains a compatibility copy'
)) {
    if (-not $rfc.Contains($required)) {
        throw "replica-set locator RFC is missing '$required'"
    }
}

if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'replica-set locator unexpectedly added another executable target'
}

Write-Output 'volunteer_replica_set_locator_boundary=verified'
Write-Output 'locator_authority=recipient-device-signed-capability-update'
Write-Output 'locator_store_limit=3-runtime-8-protocol'
Write-Output 'provider_resolution=exact-indexed-store-key'
Write-Output 'legacy_capability_mode=bounded-random-fallback'
Write-Output 'social_ids_exposed_to_provider=false'
Write-Output 'global_directory=false'
Write-Output 'new_executable=false'
