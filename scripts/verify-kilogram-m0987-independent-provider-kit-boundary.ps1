[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0987-independent-provider-kit.ps1'
$commonPath = Join-Path $workspace 'scripts\m0969-exact\common.ps1'
$alicePreparePath = Join-Path $workspace 'scripts\m0969-exact\1\01_PREPARE_ALICE.ps1'
$aliceVerifyPath = Join-Path $workspace 'scripts\m0969-exact\1\03_VERIFY.ps1'
$bobPreparePath = Join-Path $workspace 'scripts\m0969-exact\3\01_PREPARE_BOB.ps1'
$provider1Path = Join-Path $workspace 'scripts\m0987-independent\2\01_START_PROVIDER1.ps1'
$provider2Path = Join-Path $workspace 'scripts\m0987-independent\3\01_START_PROVIDER2.ps1'
$baseVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0969-exact-locator-evidence.ps1'
$verifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0987-independent-provider-evidence.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0110-independent-provider-field-contract.md'
$guidePath = Join-Path $workspace 'docs\M0.9.87-INDEPENDENT-PROVIDER-FIELD-TEST-RU.md'
$twoHostGuidePath = Join-Path $workspace 'docs\M0.9.87-TWO-HOST-REDUCED-TEST-RU.md'

foreach ($path in @(
    $generatorPath, $commonPath, $alicePreparePath, $aliceVerifyPath, $bobPreparePath,
    $provider1Path, $provider2Path, $baseVerifierPath, $verifierPath, $rfcPath, $guidePath,
    $twoHostGuidePath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M0.9.87 independent-provider source is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$common = Get-Content -LiteralPath $commonPath -Raw
$alicePrepare = Get-Content -LiteralPath $alicePreparePath -Raw
$aliceVerify = Get-Content -LiteralPath $aliceVerifyPath -Raw
$bobPrepare = Get-Content -LiteralPath $bobPreparePath -Raw
$provider1 = Get-Content -LiteralPath $provider1Path -Raw
$provider2 = Get-Content -LiteralPath $provider2Path -Raw
$baseVerifier = Get-Content -LiteralPath $baseVerifierPath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$guide = Get-Content -LiteralPath $guidePath -Raw
$twoHostGuide = Get-Content -LiteralPath $twoHostGuidePath -Raw

foreach ($required in @(
    "milestone = 'M0.9.87'",
    "foreach (`$directory in @('1', '2', '3', '4'))",
    "'2/01_START_PROVIDER1.ps1'",
    "'3/01_START_PROVIDER2.ps1'",
    "'4/01_PREPARE_BOB.ps1'",
    "'4/02_RECEIVE_BOB.ps1'",
    '[switch] $TwoHostReduced',
    "'two-host-reduced'",
    'minimum_physical_hosts = $minimumHosts',
    'ideal_physical_hosts = $idealHosts',
    'field_topology_mode = $topologyMode',
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false",
    "https_fixture_included = `$false",
    "central_service_descriptor_present = `$false",
    'provider_independence_claim=$claimBoundary',
    'independent_provider_field_acceptance=false',
    'status=kilogram-m0987-two-host-reduced-kit-created',
    'status=kilogram-m0987-independent-provider-kit-created'
)) {
    if (-not $generator.Contains($required)) { throw "M0.9.87 generator is missing '$required'" }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip', 'kilogram-ticket-store.exe')) {
    if ($generator.Contains($forbidden)) { throw "M0.9.87 generator contains forbidden '$forbidden'" }
}

foreach ($required in @(
    "`$milestone -cnotin @('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74', 'M0.9.76', 'M0.9.87')",
    "'M0.9.87' { 'M0987' }",
    '$env:LOCALAPPDATA',
    'function Start-M0987IndependentProvider',
    'function Publish-M0987ProviderPairIfReady',
    'function Get-M0987ClaimBoundary',
    'MachineGuid',
    "'kilogram/m0987/machine/v1'",
    "'kilogram/m0987/operator/v1'",
    "'kilogram/m0987/network/v1'",
    'controlled-self-attestation-not-protocol-proof',
    'two-host-reduced-not-independent-field-proof',
    "@('machine')",
    'private_state_shared = $false',
    '01-$ProviderName-publication.json',
    '01-$ProviderName-attestation.json',
    '01-provider-offers-publication.json',
    'self-attested $field domains are not distinct',
    'STOP-PROVIDERS.marker',
    'providers-stopped.marker',
    'Publish-M0969File $localLog',
    'Still waiting for the other provider stop marker'
)) {
    if (-not $common.Contains($required)) { throw "M0.9.87 common helper is missing '$required'" }
}
if ($common.Contains('machine_guid =') -or $common.Contains('operator_label =') -or
    $common.Contains('network_label =')) {
    throw 'M0.9.87 must not serialize raw machine, operator, or network labels.'
}
$stopIndex = $common.IndexOf('Stop-M0969Process $runtime', $common.IndexOf('function Start-M0987IndependentProvider'))
$publishIndex = $common.IndexOf('Publish-M0969File $localLog', $stopIndex)
if ($stopIndex -lt 0 -or $publishIndex -le $stopIndex) {
    throw 'M0.9.87 must close provider runtime logs before publishing them to the synchronized folder.'
}

foreach ($entry in @(@($provider1, "Start-M0987IndependentProvider 'provider1'"), @($provider2, "Start-M0987IndependentProvider 'provider2'"))) {
    foreach ($required in @('[string] $OperatorLabel', '[string] $NetworkLabel', $entry[1])) {
        if (-not $entry[0].Contains($required)) { throw "M0.9.87 provider wrapper is missing '$required'" }
    }
}
foreach ($required in @(
    "'M0.9.87' { 'm0987' }", "@('M0.9.76', 'M0.9.87')", "`$manifest['field_topology_mode']"
)) {
    if (-not $alicePrepare.Contains($required)) { throw "M0.9.87 Alice preparation is missing '$required'" }
}
if (-not $bobPrepare.Contains("@('M0.9.76', 'M0.9.87')")) {
    throw 'M0.9.87 Bob preparation does not use service-free capability v2.'
}
foreach ($required in @(
    "'M0.9.87' { 'm0987' }",
    'verify-kilogram-m0987-independent-provider-evidence.ps1',
    'M0.9.87 INDEPENDENT-PROVIDER SERVICE-FREE FIELD TEST COMPLETED SUCCESSFULLY.',
    'M0.9.87 TWO-HOST REDUCED SERVICE-FREE TEST COMPLETED SUCCESSFULLY.',
    '-TopologyMode ([string]$build.field_topology_mode)'
)) {
    if (-not $aliceVerify.Contains($required)) { throw "M0.9.87 final verification is missing '$required'" }
}
if (-not $baseVerifier.Contains("'m0976', 'm0987'")) {
    throw 'Base exact-locator verifier does not accept the m0987 evidence label.'
}
foreach ($required in @(
    "-LabelPrefix 'm0987' -SuppressReport",
    'Assert-M0987ExactProperties',
    'machine_pseudonym',
    'operator_claim_digest',
    'network_claim_digest',
    'controlled-self-attestation-not-protocol-proof',
    'two-host-reduced-not-independent-field-proof',
    'same_machine_claim_rejected=true',
    'same_operator_claim_rejected=true',
    'same_network_claim_rejected=true',
    'two_host_same_operator_allowed=true',
    'two_host_same_network_allowed=true',
    'two_host_same_machine_rejected=true',
    'independent_provider_field_acceptance',
    'raw_claim_field_rejected=true',
    'm0987_independent_provider_kit_boundary=verified'
)) {
    if (-not $verifier.Contains($required)) { throw "M0.9.87 evidence verifier is missing '$required'" }
}
foreach ($required in @(
    'minimum of three physical hosts',
    'controlled self-attestation',
    'a protocol proof of operator',
    'closed final logs',
    'no release build, ZIP, HTTPS mailbox service'
)) {
    if (-not $rfc.Contains($required)) { throw "RFC-0110 is missing '$required'" }
}
foreach ($required in @(
    'M0.9.87',
    '`01_START_PROVIDER1.ps1`',
    '`01_START_PROVIDER2.ps1`',
    '`01_PREPARE_BOB.ps1`',
    'run-scoped SHA-256 digests'
)) {
    if (-not $guide.Contains($required)) { throw "M0.9.87 Russian field guide is missing '$required'" }
}
foreach ($required in @(
    'M0.9.87 two-host reduced',
    'Alice + Provider 1',
    'Provider 2 + Bob',
    'independent-provider field acceptance',
    '`03_VERIFY.ps1`'
)) {
    if (-not $twoHostGuide.Contains($required)) { throw "M0.9.87 two-host guide is missing '$required'" }
}

foreach ($path in @($generatorPath, $commonPath, $alicePreparePath, $aliceVerifyPath, $bobPreparePath, $provider1Path, $provider2Path, $verifierPath)) {
    $tokens = $null
    $errors = $null
    [Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -ne 0) { throw "PowerShell source does not parse cleanly: $path" }
}

& (Join-Path $workspace 'scripts\verify-kilogram-m0976-service-free-v2-kit-boundary.ps1') | Out-Null
& $verifierPath -SelfTest | Out-Null

Write-Output 'm0987_independent_provider_kit_boundary=verified'
Write-Output 'm0987_two_host_reduced_profile_boundary=verified'
Write-Output 'field_folders=alice,provider1,provider2,bob'
Write-Output 'minimum_physical_hosts=3'
Write-Output 'ideal_physical_hosts=4'
Write-Output 'provider_machine_claims=distinct-required'
Write-Output 'provider_operator_claims=distinct-required'
Write-Output 'provider_network_claims=distinct-required'
Write-Output 'claim_strength=controlled-self-attestation-not-protocol-proof'
Write-Output 'shared_open_runtime_logs=false'
Write-Output 'private_state_shared=false'
Write-Output 'profile=debug'
Write-Output 'archive=false'
Write-Output 'generator_network_execution=false'
Write-Output 'new_executable=false'
