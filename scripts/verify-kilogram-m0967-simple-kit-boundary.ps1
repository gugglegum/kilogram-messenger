[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$source = Join-Path $workspace 'scripts\m0967-simple'
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0967-simple-kit.ps1'
$required = @(
    $generatorPath,
    (Join-Path $source 'common.ps1'),
    (Join-Path $source '1\01_PREPARE_ALICE.ps1'),
    (Join-Path $source '1\02_SEND_ALICE.ps1'),
    (Join-Path $source '1\03_VERIFY.ps1'),
    (Join-Path $source '2\01_START_PROVIDERS.ps1'),
    (Join-Path $source '2\02_RESTART_PROVIDERS_AFTER_FIX.ps1'),
    (Join-Path $source '3\01_PREPARE_BOB.ps1'),
    (Join-Path $source '3\02_RECEIVE_BOB.ps1'),
    (Join-Path $source '3\03_RETRY_BOB_AFTER_OFFER_SYNC_FIX.ps1')
)
foreach ($path in $required) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "simple kit source is missing: $path" }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$common = Get-Content -LiteralPath (Join-Path $source 'common.ps1') -Raw
$alicePrepare = Get-Content -LiteralPath (Join-Path $source '1\01_PREPARE_ALICE.ps1') -Raw
$aliceSend = Get-Content -LiteralPath (Join-Path $source '1\02_SEND_ALICE.ps1') -Raw
$providers = Get-Content -LiteralPath (Join-Path $source '2\01_START_PROVIDERS.ps1') -Raw
$providerRestart = Get-Content -LiteralPath (Join-Path $source '2\02_RESTART_PROVIDERS_AFTER_FIX.ps1') -Raw
$bobPrepare = Get-Content -LiteralPath (Join-Path $source '3\01_PREPARE_BOB.ps1') -Raw
$bobReceive = Get-Content -LiteralPath (Join-Path $source '3\02_RECEIVE_BOB.ps1') -Raw
$bobRetry = Get-Content -LiteralPath (Join-Path $source '3\03_RETRY_BOB_AFTER_OFFER_SYNC_FIX.ps1') -Raw

foreach ($value in @(
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-ticket-store',
    "profile = 'debug'", "archive = `$false", "network_executed = `$false",
    "@('1', '2', '3')", 'README-RU.txt', '02_RESTART_PROVIDERS_AFTER_FIX.ps1',
    'resumes an exact incomplete durable queue item'
)) {
    if (-not $generator.Contains($value)) { throw "simple kit generator is missing '$value'" }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip', 'runtime-from-profile --profile-file')) {
    if ($generator.Contains($forbidden)) { throw "simple kit generator contains forbidden action '$forbidden'" }
}
foreach ($value in @(
    '$script:KitRoot = [IO.Path]::GetFullPath($PSScriptRoot)', '$env:LOCALAPPDATA',
    'Wait-M0967File', 'Wait-M0967IpcReady', 'Move-M0967FailedAttemptAside',
    'Update-M0967ProviderOfferFiles', 'Wait-M0967ProviderOfferFile',
    'Start-M0967Process', 'Import-M0967Providers'
)) {
    if (-not $common.Contains($value)) { throw "simple kit common helper is missing '$value'" }
}
foreach ($value in @('account-create', 'conversation-create', 'runtime-contact-add', 'runtime-mailbox-offer-import', '127.0.0.1:8787')) {
    if (-not $alicePrepare.Contains($value)) { throw "Alice preparation is missing '$value'" }
}
foreach ($value in @(
    'runtime-ipc-queue-message', 'runtime_mailbox_replication_receipts=2/2',
    'alice_runtime_ipc_reachable=false', "'pre-queue'", "'post-queue'",
    'RESUMING THE EXISTING DURABLE ALICE QUEUE ITEM', 'runtime-message-queued',
    "Get-M0967ExactValue `$queueLines 'runtime_queue_id' '[0-9a-f]{64}'",
    'FINALIZING THE ALREADY SATISFIED ALICE SEND', '$replicationAlreadySatisfied',
    "`$ErrorActionPreference = 'SilentlyContinue'"
)) {
    if (-not $aliceSend.Contains($value)) { throw "Alice send is missing '$value'" }
}
foreach ($value in @("@('provider1', 'provider2')", 'runtime-profile-create', 'STOP-PROVIDERS.marker')) {
    if (-not $providers.Contains($value)) { throw "provider runner is missing '$value'" }
}
foreach ($value in @(
    'runtime-profile.json', 'old providers stopped marker', "'pre-wire-fix'",
    'FIXED PROVIDERS ARE READY'
)) {
    if (-not $providerRestart.Contains($value)) { throw "provider restart is missing '$value'" }
}
foreach ($value in @('account-create', 'conversation-membership-install', 'runtime-contact-add', 'runtime-mailbox-offer-create')) {
    if (-not $bobPrepare.Contains($value)) { throw "Bob preparation is missing '$value'" }
}
foreach ($value in @(
    'runtime_mailbox_inbound_source=volunteer-iroh', 'deleted-after-commit',
    '05-restart-bob.log', 'history', "'pre-inbound'",
    'RETRYING BOB BEFORE THE FIRST INBOUND COMMIT'
)) {
    if (-not $bobReceive.Contains($value)) { throw "Bob receive is missing '$value'" }
}
foreach ($value in @('Wait-M0967ProviderOfferFile', 'RETRYING BOB BEFORE THE FIRST INBOUND COMMIT')) {
    if (-not $bobRetry.Contains($value)) { throw "Bob recovery wrapper is missing '$value'" }
}

Write-Output 'm0967_simple_kit_boundary=verified'
Write-Output 'operator_launches=alice-prepare-bob-prepare-providers-alice-send-bob-receive-verify'
Write-Output 'shared_secrets=false'
Write-Output 'private_state=localappdata-only'
Write-Output 'compatibility_store=alice-loopback-only'
Write-Output 'volunteer_providers=two-independent-identities-one-host'
Write-Output 'archive=false'
Write-Output 'release=false'
Write-Output 'generator_network_execution=false'
