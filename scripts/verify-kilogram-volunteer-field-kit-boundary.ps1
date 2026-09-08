[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0967-volunteer-field-kit.ps1'
$providerPath = Join-Path $workspace 'scripts\new-kilogram-volunteer-field-provider.ps1'
$runtimePath = Join-Path $workspace 'scripts\invoke-kilogram-volunteer-field-runtime.ps1'
$offerPath = Join-Path $workspace 'scripts\export-kilogram-volunteer-field-offer.ps1'
$importPath = Join-Path $workspace 'scripts\import-kilogram-volunteer-field-providers.ps1'
$queuePath = Join-Path $workspace 'scripts\queue-kilogram-volunteer-field-message.ps1'
$offlinePath = Join-Path $workspace 'scripts\confirm-kilogram-volunteer-field-sender-offline.ps1'
$historyPath = Join-Path $workspace 'scripts\capture-kilogram-volunteer-field-history.ps1'
$evidencePath = Join-Path $workspace 'scripts\verify-kilogram-volunteer-field-evidence.ps1'
$guidePath = Join-Path $workspace 'docs\M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$paths = @($generatorPath, $providerPath, $runtimePath, $offerPath, $importPath, $queuePath, $offlinePath, $historyPath, $evidencePath, $guidePath, $manifestPath)
foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "volunteer field-kit boundary file is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$provider = Get-Content -LiteralPath $providerPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$offer = Get-Content -LiteralPath $offerPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$queue = Get-Content -LiteralPath $queuePath -Raw
$offline = Get-Content -LiteralPath $offlinePath -Raw
$history = Get-Content -LiteralPath $historyPath -Raw
$evidence = Get-Content -LiteralPath $evidencePath -Raw
$guide = Get-Content -LiteralPath $guidePath -Raw
$manifest = Get-Content -LiteralPath $manifestPath -Raw

foreach ($required in @(
    'git status --porcelain --untracked-files=normal',
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false",
    "'bin/kilogram-cli.exe'",
    'verify-kilogram-volunteer-field-evidence.ps1',
    'M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md'
)) {
    if (-not $generator.Contains($required)) { throw "field-kit generator is missing '$required'" }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip', 'Start-Process', 'runtime-from-profile --profile-file')) {
    if ($generator.Contains($forbidden)) { throw "field-kit generator has forbidden build/network action '$forbidden'" }
}

foreach ($required in @('account-create', 'device-enroll', 'account-device-list', 'runtime-profile-create', 'provider_secrets_copied_to_shared_directory=false')) {
    if (-not $provider.Contains($required)) { throw "private provider bootstrap is missing '$required'" }
}
foreach ($required in @('Tee-Object -LiteralPath $logPath', 'runtime-from-profile', 'already exists and will not be overwritten')) {
    if (-not $runtime.Contains($required)) { throw "field runtime capture is missing '$required'" }
}
foreach ($required in @('runtime_volunteer_storage_offer=', 'runtime_volunteer_storage=serving', 'status=runtime-listening')) {
    if (-not $offer.Contains($required)) { throw "provider offer export is missing '$required'" }
}
foreach ($required in @('runtime-ipc-volunteer-provider-import', 'runtime-ipc-volunteer-provider-select', "@('provider1', 'provider2')")) {
    if (-not $import.Contains($required)) { throw "provider import evidence is missing '$required'" }
}
foreach ($required in @('runtime-ipc-queue-message', 'message_marker', 'bob_account_id')) {
    if (-not $queue.Contains($required)) { throw "field queue action is missing '$required'" }
}
foreach ($required in @('runtime_mailbox_replication_receipts=2/2', 'runtime_mailbox_replication_status=satisfied', 'runtime-ipc-ping', 'alice_runtime_ipc_reachable=false')) {
    if (-not $offline.Contains($required)) { throw "sender-offline boundary is missing '$required'" }
}
foreach ($required in @('history --state-dir', 'conversation_label', '05-bob-history.log')) {
    if (-not $history.Contains($required)) { throw "restart history capture is missing '$required'" }
}
foreach ($required in @(
    'runtime_mailbox_replication_receipts=2/2',
    'runtime_mailbox_inbound_source=volunteer-iroh',
    'runtime_mailbox_replica_delete_status=deleted-after-commit',
    'Alice runtime stopped before Bob receive',
    "operator_independence = 'not-proven-when-providers-share-a-host'",
    'volunteer_field_evidence_self_test=verified'
)) {
    if (-not $evidence.Contains($required)) { throw "fail-closed field verifier is missing '$required'" }
}
foreach ($required in @('15 минут', 'не', 'Account Root', 'две', 'HTTPS mailbox', 'не доказывает')) {
    if (-not $guide.Contains($required)) { throw "Russian field guide is missing '$required'" }
}

if (Select-String -LiteralPath $manifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'volunteer field kit unexpectedly added another executable target'
}

Write-Output 'volunteer_field_kit_boundary=verified'
Write-Output 'build=clean-head-stable-name-debug'
Write-Output 'archive=false'
Write-Output 'generator_network_execution=false'
Write-Output 'provider_profiles=two-private-independent-test-identities'
Write-Output 'shared_secrets=false'
Write-Output 'sender_offline_probe=true'
Write-Output 'evidence=two-put-receipts-two-iroh-commits-two-signed-deletes-restart-history'
Write-Output 'operator_independence=explicitly-not-proven-on-one-host'
Write-Output 'new_executable=false'
