[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$source = Join-Path $workspace 'scripts\m0969-exact'
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0969-exact-locator-kit.ps1'
$wrapperPath = Join-Path $workspace 'scripts\new-kilogram-m0972-no-https-kit.ps1'
$baseVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0969-exact-locator-evidence.ps1'
$verifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0972-no-https-evidence.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0095-clean-no-https-volunteer-field-run.md'
$required = @(
    $generatorPath,
    $wrapperPath,
    $baseVerifierPath,
    $verifierPath,
    $rfcPath,
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
        throw "M0.9.72 no-HTTPS kit source is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$wrapper = Get-Content -LiteralPath $wrapperPath -Raw
$common = Get-Content -LiteralPath (Join-Path $source 'common.ps1') -Raw
$alicePrepare = Get-Content -LiteralPath (Join-Path $source '1\01_PREPARE_ALICE.ps1') -Raw
$aliceSend = Get-Content -LiteralPath (Join-Path $source '1\02_SEND_ALICE.ps1') -Raw
$aliceVerify = Get-Content -LiteralPath (Join-Path $source '1\03_VERIFY.ps1') -Raw
$baseVerifier = Get-Content -LiteralPath $baseVerifierPath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($value in @(
    "-Milestone 'M0.9.72'",
    "[ValidateRange(1, 64)] [int] `$CargoJobs = 2"
)) {
    if (-not $wrapper.Contains($value)) { throw "M0.9.72 generator wrapper is missing '$value'" }
}

foreach ($value in @(
    "[ValidateSet('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74')] [string] `$Milestone = 'M0.9.69'",
    "`$noHttpsCompatibility = `$Milestone -cne 'M0.9.69'",
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    "if (-not `$noHttpsCompatibility)",
    "`$artifactNames += 'kilogram-ticket-store.exe'",
    "'verify-kilogram-mailbox-https-retirement-boundary.ps1'",
    "'verify-kilogram-m0972-no-https-kit-boundary.ps1'",
    "'verify-kilogram-m0972-no-https-evidence.ps1'",
    "milestone = `$Milestone",
    "https_fixture_included = (-not `$noHttpsCompatibility)",
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false",
    'status=kilogram-m0972-no-https-kit-created'
)) {
    if (-not $generator.Contains($value)) { throw "M0.9.72 generator is missing '$value'" }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip')) {
    if ($generator.Contains($forbidden)) { throw "M0.9.72 generator contains forbidden action '$forbidden'" }
}
$noHttpsBuild = $generator.IndexOf('if ($noHttpsCompatibility)')
$cliOnlyBuild = $generator.IndexOf(
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    $noHttpsBuild
)
$legacyBuild = $generator.IndexOf(
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-ticket-store',
    $cliOnlyBuild
)
if ($noHttpsBuild -lt 0 -or $cliOnlyBuild -le $noHttpsBuild -or $legacyBuild -le $cliOnlyBuild) {
    throw 'M0.9.72 CLI-only build is not isolated from the legacy compatibility build'
}

foreach ($value in @(
    "`$script:M0972CompatibilityStoreUrl = 'http://127.0.0.1:18787'",
    "`$script:M0972CompatibilityStoreKey = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'",
    'Test-M0972CompatibilityEndpointReachable',
    'Assert-M0972HttpsFixtureAbsent',
    "`$milestone -cnotin @('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74')",
    "`$milestone -cne 'M0.9.69'",
    "'M0972'"
)) {
    if (-not $common.Contains($value)) { throw "M0.9.72 common helper is missing '$value'" }
}

foreach ($value in @(
    "'M0.9.72' { 'm0972' }",
    '00-https-fixture-absence.log',
    'field_phase=before-identity-and-mailbox-activation',
    'https_fixture_binary_present=false',
    'https_fixture_process_started=false',
    'compatibility_endpoint_reachable=false',
    'evidence_milestone = [string]$build.milestone',
    'https_fixture_present_at_start = (-not $noHttpsCompatibility)'
)) {
    if (-not $alicePrepare.Contains($value)) { throw "M0.9.72 Alice preparation is missing '$value'" }
}
$preparationGuard = $alicePrepare.IndexOf('if (-not $noHttpsCompatibility)')
$preparationStoreStart = $alicePrepare.IndexOf(
    'Start-M0969Process $script:StorePath',
    $preparationGuard
)
if ($preparationGuard -lt 0 -or $preparationStoreStart -le $preparationGuard) {
    throw 'compatibility-store bootstrap is not guarded out of M0.9.72 preparation'
}

foreach ($value in @(
    '04-https-fixture-absence.log',
    'field_phase=immediately-before-send',
    'runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability',
    'runtime_mailbox_http_put=not-attempted',
    'runtime_mailbox_delivery_durability=exact-volunteer-replication',
    'runtime_mailbox_http_put=attempted',
    'runtime_mailbox_delivery_durability=https-compatibility',
    'runtime_mailbox_exact_completion_status=failed',
    'Assert-M0972HttpsFixtureAbsent'
)) {
    if (-not $aliceSend.Contains($value)) { throw "M0.9.72 Alice send is missing '$value'" }
}
$sendGuard = $aliceSend.IndexOf('if ($noHttpsCompatibility)')
$legacyStoreStart = $aliceSend.IndexOf('Start-M0969Process $script:StorePath', $sendGuard)
if ($sendGuard -lt 0 -or $legacyStoreStart -le $sendGuard) {
    throw 'M0.9.72 send does not isolate the legacy compatibility-store start'
}

foreach ($value in @(
    'verify-kilogram-m0972-no-https-evidence.ps1',
    '-LabelPrefix $labelPrefix',
    'CLEAN NO-HTTPS VOLUNTEER DELIVERY TEST COMPLETED SUCCESSFULLY.'
)) {
    if (-not $aliceVerify.Contains($value)) { throw "M0.9.72 final verification is missing '$value'" }
}

foreach ($value in @(
    "[ValidateSet('m0969', 'm0972', 'm0973', 'm0974')] [string] `$LabelPrefix = 'm0969'",
    '[switch] $SuppressReport',
    '$ExpectedLabelPrefix'
)) {
    if (-not $baseVerifier.Contains($value)) { throw "shared exact-locator verifier is missing '$value'" }
}
foreach ($value in @(
    "[ValidateSet('m0972', 'm0973', 'm0974')] [string] `$LabelPrefix = 'm0972'",
    "[ValidateSet('M0.9.72', 'M0.9.73', 'M0.9.74')] [string] `$ExpectedMilestone = 'M0.9.72'",
    'runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability',
    'runtime_mailbox_http_put=not-attempted',
    'runtime_mailbox_delivery_durability=exact-volunteer-replication',
    'runtime_mailbox_http_put=attempted',
    'compatibility_endpoint_reachable=true',
    '-LabelPrefix $LabelPrefix -SuppressReport',
    'm0972_no_https_evidence_self_test=verified'
)) {
    if (-not $verifier.Contains($value)) { throw "M0.9.72 evidence verifier is missing '$value'" }
}

foreach ($value in @(
    'absent from the beginning',
    'no `kilogram-ticket-store.exe` artifact',
    'runtime_mailbox_http_put=not-attempted',
    'two qualifying signed',
    "Alice's runtime stops",
    'no release build or ZIP'
)) {
    if (-not $rfc.Contains($value)) { throw "M0.9.72 RFC is missing '$value'" }
}

& $baseVerifierPath -SelfTest
& $verifierPath -SelfTest

Write-Output 'm0972_no_https_kit_boundary=verified'
Write-Output 'https_fixture_included=false'
Write-Output 'compatibility_endpoint=unreachable-loopback'
Write-Output 'sender_http_put=not-attempted-required'
Write-Output 'delivery_durability=exact-volunteer-replication'
Write-Output 'recipient_commit_delete=exact-2-of-2'
Write-Output 'operator_launches=6'
Write-Output 'profile=debug'
Write-Output 'archive=false'
Write-Output 'generator_network_execution=false'
Write-Output 'new_executable=false'
