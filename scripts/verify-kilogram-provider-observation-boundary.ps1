[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0106-local-authenticated-provider-observation-provenance.md'

foreach ($path in @($providerPath, $clientLibPath, $runtimePath, $protocolPath, $ipcPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "provider observation boundary file is missing: $path"
    }
}

$provider = Get-Content -LiteralPath $providerPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

function Get-SourceBlock {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $Start,
        [Parameter(Mandatory)] [string] $End
    )
    $startIndex = $Source.IndexOf($Start, [StringComparison]::Ordinal)
    $endIndex = if ($startIndex -ge 0) {
        $Source.IndexOf($End, $startIndex + $Start.Length, [StringComparison]::Ordinal)
    }
    else {
        -1
    }
    if ($startIndex -lt 0 -or $endIndex -le $startIndex) {
        throw "could not isolate source block '$Start'"
    }
    $Source.Substring($startIndex, $endIndex - $startIndex)
}

foreach ($required in @(
    'mailbox-provider-authenticated-observations-v1',
    'MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER: u8 = 8',
    'MailboxProviderLocalObserverTag',
    'MailboxProviderObservationOutcome',
    'AuthenticatedProviderObservationRecord',
    'authenticated_observation_count',
    'authenticated_observations_are_local_bounded_deduplicated_and_offer_bound',
    'TableDoesNotExist'
)) {
    if ($provider.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "provider observation registry is missing '$required'"
    }
}
foreach ($required in @(
    'MailboxProviderLocalObserverTag',
    'MailboxProviderObservationOutcome',
    'MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER'
)) {
    if ($clientLib.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "mailbox client does not export '$required'"
    }
}

foreach ($required in @(
    'MAILBOX_PROVIDER_LOCAL_OBSERVER_KEY_CONTEXT',
    'runtime_mailbox_provider_local_observer_tag',
    'blake3::derive_key(',
    'blake3::keyed_hash(',
    'accept_device_authorization(',
    'authorize_with_listener(',
    'runtime_mailbox_provider_observation_scope=local-only',
    'runtime_mailbox_provider_observation_identifiers_transmitted=false',
    'runtime_mailbox_provider_observation_ipc_exposed=false',
    'runtime_provider_gossip_is_reply_bound_and_imports_verified_endpoints'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime provider observation integration is missing '$required'"
    }
}
if ($runtime -match '(?m)println!\([^\r\n]*(observer_tag|authenticated_observer)') {
    throw 'runtime logs expose a local authenticated provider observer tag'
}

$gossipEntry = Get-SourceBlock -Source $provider `
    -Start 'pub struct MailboxProviderGossipEntry {' -End 'impl MailboxProviderGossipEntry {'
$gossipFrame = Get-SourceBlock -Source $provider `
    -Start 'pub struct MailboxProviderGossipFrame {' -End 'impl MailboxProviderGossipFrame {'
$protocolRequest = Get-SourceBlock -Source $protocol `
    -Start 'pub enum ClientRequest {' -End 'impl ClientRequest {'
$ipcProvider = Get-SourceBlock -Source $ipc `
    -Start 'pub struct RuntimeIpcVolunteerStorageProvider {' `
    -End 'pub struct RuntimeIpcVolunteerStorageOfferImport {'
foreach ($block in @($gossipEntry, $gossipFrame, $protocolRequest, $ipcProvider)) {
    foreach ($forbidden in @(
        'MailboxProviderLocalObserverTag',
        'observer_tag',
        'authenticated_observation',
        'AccountId',
        'DeviceId'
    )) {
        if ($block.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "provider wire or IPC projection exposes forbidden local provenance '$forbidden'"
        }
    }
}
if ($ipc.IndexOf('const IPC_VERSION: u8 = 26;', [StringComparison]::Ordinal) -lt 0) {
    throw 'local provider observations unexpectedly changed authenticated IPC v26'
}

foreach ($required in @(
    'not a transferable endorsement',
    'At most eight distinct authenticated observations',
    'never encoded in a storage offer or provider gossip frame',
    'never returned through authenticated runtime IPC',
    'does not change replica-set ranking',
    'network-free and debug-only'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0106 is missing '$required'"
    }
}

foreach ($manifest in @(
    (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml')
)) {
    if (Select-String -LiteralPath $manifest -Pattern '^\s*\[\[bin\]\]\s*$') {
        throw "provider observation provenance unexpectedly added an executable target: $manifest"
    }
}

Write-Output 'provider_observation_boundary=verified'
Write-Output 'scope=local-device-pseudonymous'
Write-Output 'source=authenticated-device-session'
Write-Output 'maximum_observations_per_exact_offer=8'
Write-Output 'deduplication=exact-offer-plus-local-observer-tag'
Write-Output 'replacement_provenance_transfer=false'
Write-Output 'gossip_payload_social_ids=false'
Write-Output 'observer_tags_transmitted=false'
Write-Output 'observer_tags_in_ipc=false'
Write-Output 'm0983_selection_policy_changed=false'
Write-Output 'current_selection_consumes_binary_presence=true'
Write-Output 'observation_count_rank_weight=false'
Write-Output 'operator_independence=false'
Write-Output 'ipc_version=26'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'new_executable=false'
