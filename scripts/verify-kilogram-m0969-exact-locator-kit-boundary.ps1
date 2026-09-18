[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$source = Join-Path $workspace 'scripts\m0969-exact'
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0969-exact-locator-kit.ps1'
$evidenceVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0969-exact-locator-evidence.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0092-clean-external-exact-locator-field-run.md'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$required = @(
    $generatorPath,
    $evidenceVerifierPath,
    $rfcPath,
    $cliPath,
    (Join-Path $source 'common.ps1'),
    (Join-Path $source '1\01_PREPARE_ALICE.ps1'),
    (Join-Path $source '1\02_SEND_ALICE.ps1'),
    (Join-Path $source '1\03_VERIFY.ps1'),
    (Join-Path $source '2\01_START_PROVIDERS.ps1'),
    (Join-Path $source '3\01_PREPARE_BOB.ps1'),
    (Join-Path $source '3\02_RECEIVE_BOB.ps1')
)
foreach ($path in $required) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M0.9.69 exact-locator kit source is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$common = Get-Content -LiteralPath (Join-Path $source 'common.ps1') -Raw
$alicePrepare = Get-Content -LiteralPath (Join-Path $source '1\01_PREPARE_ALICE.ps1') -Raw
$aliceSend = Get-Content -LiteralPath (Join-Path $source '1\02_SEND_ALICE.ps1') -Raw
$aliceVerify = Get-Content -LiteralPath (Join-Path $source '1\03_VERIFY.ps1') -Raw
$providers = Get-Content -LiteralPath (Join-Path $source '2\01_START_PROVIDERS.ps1') -Raw
$bobPrepare = Get-Content -LiteralPath (Join-Path $source '3\01_PREPARE_BOB.ps1') -Raw
$bobReceive = Get-Content -LiteralPath (Join-Path $source '3\02_RECEIVE_BOB.ps1') -Raw
$verifier = Get-Content -LiteralPath $evidenceVerifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw

foreach ($value in @(
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-ticket-store',
    "[ValidateRange(1, 64)] [int] `$CargoJobs = 2",
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false",
    'verify-kilogram-volunteer-replica-locator-boundary.ps1',
    'verify-kilogram-m0969-exact-locator-kit-boundary.ps1',
    "@('1', '2', '3')",
    'operator_launches=6',
    "'1/01_PREPARE_ALICE.ps1'",
    "'3/02_RECEIVE_BOB.ps1'",
    'kit_script_integrity=sha256-length',
    'README-RU.txt'
)) {
    if (-not $generator.Contains($value)) { throw "M0.9.69 generator is missing '$value'" }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip', 'runtime-from-profile --profile-file')) {
    if ($generator.Contains($forbidden)) { throw "M0.9.69 generator contains forbidden action '$forbidden'" }
}

foreach ($value in @(
    '$env:LOCALAPPDATA',
    'Get-M0969ProviderOfferSnapshot',
    'Publish-M0969ProviderOffers',
    'field_provider_snapshot=manifest-hash-consistent',
    '01-provider-offers-publication.json',
    'expires_at_unix_seconds',
    'Get-FileHash',
    'Start-M0969Process',
    '-WindowStyle Hidden',
    'Import-M0969Providers',
    "'--count', '2'",
    'Still working: observed $matched/$Count required events',
    'Wait-M0969FileHashChange'
)) {
    if (-not $common.Contains($value)) { throw "M0.9.69 common helper is missing '$value'" }
}

foreach ($value in @(
    'account-create',
    'conversation-create',
    'runtime-contact-add',
    'runtime-ipc-contact-add',
    '03-alice-live-contact-refresh.log',
    'Bob live convergence ticket',
    'runtime-mailbox-offer-import',
    'status=runtime-mailbox-capability-updated',
    'bob-capability-acked.marker',
    '127.0.0.1:8787'
)) {
    if (-not $alicePrepare.Contains($value)) { throw "M0.9.69 Alice preparation is missing '$value'" }
}
foreach ($value in @(
    'runtime-ipc-queue-message',
    'runtime_mailbox_replica_set_discovery=exact-authenticated',
    'runtime_mailbox_replica_set_resolved=2/2',
    'runtime_mailbox_replication_receipts=2/2',
    'runtime_mailbox_replication_provider_attempt_store_key=',
    'runtime_mailbox_replication_receipt_store_key=',
    'legacy-random-fallback',
    'alice_runtime_ipc_reachable=false'
)) {
    if (-not $aliceSend.Contains($value)) { throw "M0.9.69 Alice send is missing '$value'" }
}
foreach ($value in @(
    'verify-kilogram-m0969-exact-locator-evidence.ps1',
    'M0.9.69 CLEAN EXACT-LOCATOR FIELD TEST COMPLETED SUCCESSFULLY.'
)) {
    if (-not $aliceVerify.Contains($value)) { throw "M0.9.69 final verification is missing '$value'" }
}

foreach ($value in @(
    'Get-M0969Run',
    "@('provider1', 'provider2')",
    'Publish-M0969ProviderOffers',
    'providers-ready.marker',
    'ready-before-mailbox-activation',
    'STOP-PROVIDERS.marker'
)) {
    if (-not $providers.Contains($value)) { throw "M0.9.69 provider runner is missing '$value'" }
}
if ($providers.Contains('alice-ready.marker') -or $providers.Contains('bob-ready.marker')) {
    throw 'M0.9.69 providers must start before Alice/Bob preparation completes'
}

foreach ($value in @(
    "'bob-before-activation'",
    "'02-bob-providers-before-activation.log'",
    'runtime-mailbox-offer-create',
    'mailbox_replica_set_discovery=exact-authenticated',
    'mailbox_replica_set_store_count=2',
    'runtime-ipc-contact-add',
    '03-bob-live-contact-refresh.log',
    'Alice live convergence ticket',
    'runtime_mailbox_capability_update_status=acknowledged',
    'alice-capability-applied.marker'
)) {
    if (-not $bobPrepare.Contains($value)) { throw "M0.9.69 Bob preparation is missing '$value'" }
}
$providerImportIndex = $bobPrepare.IndexOf("Import-M0969Providers 'bob-before-activation'")
$activationIndex = $bobPrepare.IndexOf("'runtime-mailbox-offer-create'")
if ($providerImportIndex -lt 0 -or $activationIndex -lt 0 -or $providerImportIndex -ge $activationIndex) {
    throw 'Bob provider import must precede mailbox activation in M0.9.69'
}
$aliceRefreshIndex = $alicePrepare.IndexOf("'runtime-ipc-contact-add'")
$aliceConvergenceIndex = $alicePrepare.IndexOf("'^status=runtime-mailbox-capability-updated$'")
$bobRefreshIndex = $bobPrepare.IndexOf("'runtime-ipc-contact-add'")
$bobConvergenceIndex = $bobPrepare.IndexOf("'^runtime_mailbox_capability_update_status=acknowledged$'")
if ($aliceRefreshIndex -lt 0 -or $aliceConvergenceIndex -lt 0 -or
    $aliceRefreshIndex -ge $aliceConvergenceIndex -or
    $bobRefreshIndex -lt 0 -or $bobConvergenceIndex -lt 0 -or
    $bobRefreshIndex -ge $bobConvergenceIndex) {
    throw 'fresh live peer tickets must be validated before M0.9.69 capability convergence'
}

foreach ($value in @(
    "'bob-before-receive'",
    'runtime_mailbox_replica_set_discovery=exact-authenticated',
    'runtime_mailbox_replica_set_resolved=2/2',
    'runtime_mailbox_replica_poll_store_key=',
    'runtime_mailbox_replica_source_store_key=',
    'runtime_mailbox_inbound_source=volunteer-iroh',
    'deleted-after-commit',
    'legacy-random-fallback',
    'observing for 20 seconds to reject replica redelivery'
)) {
    if (-not $bobReceive.Contains($value)) { throw "M0.9.69 Bob receive is missing '$value'" }
}

foreach ($value in @(
    'providers-before-capability',
    'replica_set_commitment_id',
    'provider_store_keys',
    'legacy-random-fallback is forbidden',
    'provider outside the committed set',
    'sender_resolution',
    'recipient_resolution',
    'https_compatibility_copy',
    'provider_substitution_rejected=true',
    'stale_endpoint_ticket_rejected=true'
)) {
    if (-not $verifier.Contains($value)) { throw "M0.9.69 evidence verifier is missing '$value'" }
}
foreach ($value in @(
    'RuntimeIpcContactAdd',
    'RuntimeIpcCommand::AddContact',
    'runtime_contact_update=live-ipc',
    'async fn runtime_ipc_contact_add'
)) {
    if (-not $cli.Contains($value)) { throw "kilogram-cli live contact IPC surface is missing '$value'" }
}
foreach ($value in @(
    'providers exist before mailbox activation',
    'same commitment',
    'fresh runtime tickets',
    'legacy-random-fallback',
    'HTTPS compatibility',
    'no ZIP',
    'debug'
)) {
    if (-not $rfc.Contains($value)) { throw "M0.9.69 RFC is missing '$value'" }
}

& $evidenceVerifierPath -SelfTest

Write-Output 'm0969_exact_locator_kit_boundary=verified'
Write-Output 'provider_activation_order=providers-before-mailbox-capability'
Write-Output 'capability_convergence=recipient-applied-owner-acknowledged'
Write-Output 'live_endpoint_refresh=validated-before-convergence'
Write-Output 'sender_provider_resolution=exact-2-of-2'
Write-Output 'recipient_provider_resolution=exact-2-of-2'
Write-Output 'legacy_random_fallback=forbidden'
Write-Output 'provider_substitution=forbidden'
Write-Output 'operator_launches=6'
Write-Output 'profile=debug'
Write-Output 'archive=false'
Write-Output 'generator_network_execution=false'
