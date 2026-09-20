[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0108-local-provider-path-domain-provenance.md'

foreach ($path in @($transportPath, $providerPath, $clientLibPath, $runtimePath, $protocolPath, $ipcPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "provider path-domain boundary file is missing: $path"
    }
}

$transport = Get-Content -LiteralPath $transportPath -Raw
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
    'SelectedPathLocalDomainKind',
    'SelectedPathLocalDomainTag(<redacted>)',
    'SELECTED_PATH_LOCAL_DOMAIN_TAG_DOMAIN',
    'blake3::Hasher::new_keyed(local_subkey)',
    'address.ip()',
    'url.origin().ascii_serialization()',
    'local_path_domain_tags_strip_direct_ports_and_relay_paths',
    'local_path_domain_tags_are_installation_scoped_and_redacted'
)) {
    if ($transport.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "typed selected-path domain derivation is missing '$required'"
    }
}

foreach ($required in @(
    'mailbox-provider-verified-path-domains-v1',
    'MailboxProviderLocalPathDomainKind',
    'MailboxProviderLocalPathDomainTag(<redacted>)',
    'MailboxProviderPathDomainOutcome',
    'VerifiedProviderPathDomainRecord',
    'record_verified_path_domain',
    'offer_id(&active_offer.encoded_offer) == expected_offer_id',
    'set_durability(Durability::Immediate)',
    'verified_path_domains_are_local_exact_offer_bound_and_replaceable',
    'TableDoesNotExist'
)) {
    if ($provider.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "provider path-domain registry is missing '$required'"
    }
}
foreach ($required in @(
    'MailboxProviderLocalPathDomainKind',
    'MailboxProviderLocalPathDomainTag',
    'MailboxProviderPathDomainOutcome'
)) {
    if ($clientLib.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "mailbox client does not export '$required'"
    }
}

$exchange = Get-SourceBlock -Source $runtime `
    -Start 'async fn exchange_runtime_volunteer_mailbox_request(' `
    -End 'async fn list_runtime_volunteer_mailbox_replica('
$authenticatedResponse = $exchange.IndexOf('read_mailbox_peer_response(', [StringComparison]::Ordinal)
$pathSnapshot = $exchange.IndexOf('selected_path_diagnostics(&connection, Duration::ZERO)', [StringComparison]::Ordinal)
if ($authenticatedResponse -lt 0 -or $pathSnapshot -le $authenticatedResponse) {
    throw 'selected path snapshot is not ordered after the authenticated peer response'
}
if ($exchange.Contains('record_runtime_mailbox_provider_path_domain')) {
    throw 'untyped peer exchange persists path evidence before operation-specific verification'
}

$put = Get-SourceBlock -Source $runtime `
    -Start 'async fn put_runtime_volunteer_mailbox_replica(' `
    -End 'async fn exchange_runtime_volunteer_mailbox_request('
$list = Get-SourceBlock -Source $runtime `
    -Start 'async fn list_runtime_volunteer_mailbox_replica(' `
    -End 'async fn delete_runtime_volunteer_mailbox_replica('
$delete = Get-SourceBlock -Source $runtime `
    -Start 'async fn delete_runtime_volunteer_mailbox_replica(' `
    -End 'async fn attempt_runtime_volunteer_mailbox_replication('
foreach ($pair in @(
    @($put, 'MailboxPutResponse::decode_and_verify'),
    @($list, 'MailboxListResponse::decode_and_verify'),
    @($delete, 'MailboxDeleteResponse::decode_and_verify')
)) {
    if ($pair[0].IndexOf($pair[1], [StringComparison]::Ordinal) -lt 0 -or
        -not $pair[0].Contains('VerifiedVolunteerMailboxExchange')) {
        throw "operation-specific provider verification is missing '$($pair[1])'"
    }
}

foreach ($required in @(
    'MAILBOX_PROVIDER_LOCAL_PATH_DOMAIN_KEY_CONTEXT',
    'MAILBOX_PROVIDER_LOCAL_PATH_DOMAIN_EPOCH_CONTEXT',
    'record_runtime_mailbox_provider_path_domain',
    'load_device_state(state_directory, false)',
    'blake3::derive_key(',
    'selected_path.local_domain_tag(&local_path_domain_key)',
    'MailboxProviderLocalPathDomainTag::from_parts(derivation_epoch, tag.into_bytes())',
    'record_verified_path_domain(',
    'runtime_mailbox_provider_path_domain_status=unavailable'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime provider path-domain integration is missing '$required'"
    }
}
if ($runtime -match '(?m)(println|eprintln)![^\r\n]*(domain_tag|derivation_epoch|local_path_domain_key)') {
    throw 'runtime logs expose provider path-domain tag or derivation material'
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
        'PathDomain',
        'path_domain',
        'domain_tag',
        'derivation_epoch',
        'remote_ip',
        'relay_origin'
    )) {
        if ($block.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "provider wire or IPC projection exposes local path provenance '$forbidden'"
        }
    }
}
if (-not $ipc.Contains('const IPC_VERSION: u8 = 26;')) {
    throw 'local provider path provenance unexpectedly changed authenticated IPC v26'
}

foreach ($required in @(
    'collects provenance only',
    'exact selected remote IPv4 or IPv6 address',
    'canonical URL origin',
    'not proof of independence',
    'after operation-specific verification succeeds',
    'derivation epoch prevents comparisons',
    'not encoded into provider',
    'does not consume the new evidence for',
    'network-free and debug-only'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0108 is missing '$required'"
    }
}

foreach ($manifest in @(
    (Join-Path $workspace 'crates\kilogram-transport-iroh\Cargo.toml'),
    (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml')
)) {
    if (Select-String -LiteralPath $manifest -Pattern '^\s*\[\[bin\]\]\s*$') {
        throw "provider path-domain provenance unexpectedly added an executable: $manifest"
    }
}

Write-Output 'provider_path_domain_boundary=verified'
Write-Output 'source=verified-successful-selected-path'
Write-Output 'direct_domain=exact-remote-ip-without-port'
Write-Output 'relay_domain=canonical-origin'
Write-Output 'tag_scope=local-device-derivation-epoch'
Write-Output 'raw_ip_persisted=false'
Write-Output 'relay_url_persisted=false'
Write-Output 'path_tags_transmitted=false'
Write-Output 'path_tags_in_ipc=false'
Write-Output 'm0985_selection_policy_changed=false'
Write-Output 'current_selection_consumes_positive_colocation=true'
Write-Output 'operator_independence_proven=false'
Write-Output 'ipc_version=26'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'new_executable=false'
