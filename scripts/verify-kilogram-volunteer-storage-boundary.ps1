[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$runtimeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimeManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$storePath = Join-Path $workspace 'apps\kilogram-ticket-store\src\lib.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'

$runtimeIpc = Get-Content -LiteralPath $runtimeIpcPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$runtimeManifest = Get-Content -LiteralPath $runtimeManifestPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw

foreach ($required in @(
    'DEFAULT_VOLUNTEER_STORAGE_TOTAL_BYTES: u64 = 200 * 1024 * 1024',
    'DEFAULT_VOLUNTEER_STORAGE_ETHERNET_TRANSFER_BYTES: u64 = 500 * 1024 * 1024',
    'DEFAULT_VOLUNTEER_STORAGE_WIFI_TRANSFER_BYTES: u64 = 500 * 1024 * 1024',
    'DEFAULT_VOLUNTEER_STORAGE_MOBILE_TRANSFER_BYTES: u64 = 0',
    'DEFAULT_VOLUNTEER_STORAGE_UNKNOWN_NETWORK_TRANSFER_BYTES: u64 = 0',
    'max_total_storage_bytes',
    'transfer_bytes_per_30_days'
)) {
    if (-not $runtimeIpc.Contains($required)) {
        throw "runtime volunteer policy is missing '$required'"
    }
}

foreach ($required in @(
    'start_runtime_volunteer_storage',
    'StoreServiceMode::MailboxOnly',
    'Ipv4Addr::LOCALHOST',
    'runtime_volunteer_storage=disabled-by-user',
    'runtime_volunteer_storage=paused-by-network-policy',
    'runtime_volunteer_storage_ingress=dedicated-iroh-alpn-plus-loopback',
    'runtime_volunteer_storage_offer_distribution=authenticated-bounded-peer-gossip',
    'runtime_volunteer_storage_discovery=verified-expiring-offer-registry',
    'runtime_volunteer_storage_replication=false',
    'server.shutdown().await?'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime volunteer storage integration is missing '$required'"
    }
}

if (-not $runtimeManifest.Contains('kilogram-ticket-store = { path = "../kilogram-ticket-store" }')) {
    throw 'ordinary runtime does not embed the bounded blind mailbox store'
}
if (Select-String -LiteralPath $runtimeManifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer storage unexpectedly added another executable target'
}

foreach ($required in @(
    'StoreServiceMode::MailboxOnly',
    'transfer_accounting_scope',
    'max_transfer_bytes_per_30_days',
    'reserve_transfer_bytes',
    'TRANSFER_WINDOW_SECONDS',
    'Durability::Immediate',
    'volunteer transfer budget exhausted',
    'mailbox_only_mode_fails_closed_for_ticket_publication_routes',
    'transfer_budget_is_durable_and_resets_after_thirty_day_window'
)) {
    if (-not $store.Contains($required)) {
        throw "blind store boundary is missing '$required'"
    }
}

foreach ($required in @(
    'volunteer_storage_enabled: true',
    'Help store encrypted offline messages for other users',
    'Storage limit MiB',
    'Ethernet / 30 days MiB',
    'Wi-Fi / 30 days MiB',
    'Mobile / 30 days MiB',
    'Unknown network / 30 days MiB'
)) {
    if (-not $desktop.Contains($required)) {
        throw "desktop volunteer storage controls are missing '$required'"
    }
}

Write-Output 'volunteer_storage_boundary=verified'
Write-Output 'default_enabled=true'
Write-Output 'default_storage_mib=200'
Write-Output 'default_ethernet_transfer_mib_per_30_days=500'
Write-Output 'default_wifi_transfer_mib_per_30_days=500'
Write-Output 'default_mobile_transfer_mib_per_30_days=0'
Write-Output 'transfer_accounting=durable-application-payload'
Write-Output 'runtime_scope=embedded-mailbox-only'
Write-Output 'current_ingress=iroh-dedicated-alpn-plus-loopback'
Write-Output 'peer_discovery=authenticated-bounded-peer-gossip'
Write-Output 'new_executable=false'
