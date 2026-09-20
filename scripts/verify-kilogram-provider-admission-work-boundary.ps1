[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$mailboxPath = Join-Path $workspace 'crates\kilogram-mailbox\src\lib.rs'
$storePath = Join-Path $workspace 'crates\kilogram-mailbox\src\store.rs'
$clientPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0105-sybil-costed-provider-admission.md'

foreach ($path in @(
    $mailboxPath,
    $storePath,
    $clientPath,
    $clientLibPath,
    $runtimePath,
    $ipcPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "provider admission-work boundary file is missing: $path"
    }
}

$mailbox = Get-Content -LiteralPath $mailboxPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$client = Get-Content -LiteralPath $clientPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'STORAGE_OFFER_ADMISSION_WORK_DOMAIN',
    'DEFAULT_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS: u8 = 18',
    'MAX_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS: u8 = 20',
    'MAX_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_ATTEMPTS: u32 = 1 << 24',
    'storage_offer_with_admission_work',
    'storage_offer_admission_work_hasher',
    'increment_admission_nonce',
    'verify_admission_work',
    'storage_offer_admission_work_is_bounded_bound_and_cheap_to_verify'
)) {
    if (-not $mailbox.Contains($required)) {
        throw "mailbox provider admission work is missing '$required'"
    }
}

foreach ($required in @(
    'storage_offer_with_admission_work',
    'DEFAULT_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS',
    'create admission-work-qualified mailbox storage offer'
)) {
    if (-not $store.Contains($required)) {
        throw "blind mailbox store does not create qualified offers: '$required'"
    }
}

foreach ($required in @(
    'DEFAULT_PROVIDER_ADMISSION_WORK_BITS',
    'select_admission_qualified',
    'select_admission_qualified_from_active_offers',
    'offer.admission_work_bits() >= u16::from(DEFAULT_PROVIDER_ADMISSION_WORK_BITS)',
    'admission_qualified_selection_and_gossip_exclude_cheap_identities'
)) {
    if (-not $client.Contains($required)) {
        throw "provider selection admission boundary is missing '$required'"
    }
}
if (-not $clientLib.Contains('DEFAULT_PROVIDER_ADMISSION_WORK_BITS')) {
    throw 'mailbox client does not export the provider admission-work policy'
}

foreach ($required in @(
    'select_admission_qualified_from_active_offers',
    '.select_admission_qualified(selection_salt, requested, now)',
    'provider_selection_policy=admission-work-plus-transport-distinct',
    'runtime_volunteer_storage_offer_admission_work_bits=',
    'runtime_volunteer_storage_selection=admission-work-plus-transport-distinct',
    'runtime_volunteer_storage_minimum_admission_work_bits=',
    'admission-unqualified runtime test offer'
)) {
    if (-not $runtime.Contains($required)) {
        throw "runtime provider admission integration is missing '$required'"
    }
}

if (-not $ipc.Contains('const IPC_VERSION: u8 = 26;')) {
    throw 'provider admission work unexpectedly changed authenticated IPC v26'
}

foreach ($required in @(
    'does not prove that two providers have',
    'different operators, networks or physical failure domains',
    'does not make Kilogram Sybil-proof',
    'existing exact replica-set commitments remain',
    'network-free and debug-only'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0105 is missing '$required'"
    }
}

foreach ($manifest in @(
    (Join-Path $workspace 'crates\kilogram-mailbox\Cargo.toml'),
    (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml')
)) {
    if (Select-String -LiteralPath $manifest -Pattern '^\s*\[\[bin\]\]\s*$') {
        throw "provider admission work unexpectedly added an executable target: $manifest"
    }
}

Write-Output 'provider_admission_work_boundary=verified'
Write-Output 'minimum_admission_work_bits=18'
Write-Output 'maximum_generation_work_bits=20'
Write-Output 'selection=admission-work-plus-transport-distinct'
Write-Output 'legacy_exact_commitment_resolution=preserved'
Write-Output 'operator_independence=false'
Write-Output 'sybil_resistance=cost-floor-not-proof'
Write-Output 'ipc_version=26'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'new_executable=false'
