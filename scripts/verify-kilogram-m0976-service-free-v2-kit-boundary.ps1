[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0969-exact-locator-kit.ps1'
$wrapperPath = Join-Path $workspace 'scripts\new-kilogram-m0976-service-free-v2-kit.ps1'
$commonPath = Join-Path $workspace 'scripts\m0969-exact\common.ps1'
$alicePreparePath = Join-Path $workspace 'scripts\m0969-exact\1\01_PREPARE_ALICE.ps1'
$aliceSendPath = Join-Path $workspace 'scripts\m0969-exact\1\02_SEND_ALICE.ps1'
$aliceVerifyPath = Join-Path $workspace 'scripts\m0969-exact\1\03_VERIFY.ps1'
$bobPreparePath = Join-Path $workspace 'scripts\m0969-exact\3\01_PREPARE_BOB.ps1'
$baseVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0969-exact-locator-evidence.ps1'
$verifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0976-service-free-v2-evidence.ps1'
$serviceFreeGatePath = Join-Path $workspace 'scripts\verify-kilogram-service-free-mailbox-capability-v2.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0099-service-free-v2-field-run.md'

foreach ($path in @(
    $generatorPath,
    $wrapperPath,
    $commonPath,
    $alicePreparePath,
    $aliceSendPath,
    $aliceVerifyPath,
    $bobPreparePath,
    $baseVerifierPath,
    $verifierPath,
    $serviceFreeGatePath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M0.9.76 service-free v2 kit source is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$wrapper = Get-Content -LiteralPath $wrapperPath -Raw
$common = Get-Content -LiteralPath $commonPath -Raw
$alicePrepare = Get-Content -LiteralPath $alicePreparePath -Raw
$aliceSend = Get-Content -LiteralPath $aliceSendPath -Raw
$aliceVerify = Get-Content -LiteralPath $aliceVerifyPath -Raw
$bobPrepare = Get-Content -LiteralPath $bobPreparePath -Raw
$baseVerifier = Get-Content -LiteralPath $baseVerifierPath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    "-Milestone 'M0.9.76'",
    '[ValidateRange(1, 64)] [int] $CargoJobs = 2'
)) {
    if (-not $wrapper.Contains($required)) {
        throw "M0.9.76 generator wrapper is missing '$required'"
    }
}

foreach ($required in @(
    "[ValidateSet('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74', 'M0.9.76')] [string] `$Milestone = 'M0.9.69'",
    "'M0.9.76' { 'm0976-service-free-v2' }",
    "'verify-kilogram-service-free-mailbox-capability-v2.ps1'",
    "'verify-kilogram-m0976-service-free-v2-kit-boundary.ps1'",
    "'verify-kilogram-m0976-service-free-v2-evidence.ps1'",
    "mailbox_capability_format = if (`$serviceFreeV2) { 'v2-exact-volunteer' }",
    'central_service_descriptor_present = (-not $serviceFreeV2)',
    'status=kilogram-m0976-service-free-v2-kit-created',
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false"
)) {
    if (-not $generator.Contains($required)) {
        throw "M0.9.76 generator is missing '$required'"
    }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip')) {
    if ($generator.Contains($forbidden)) {
        throw "M0.9.76 generator contains forbidden action '$forbidden'"
    }
}

foreach ($required in @(
    "`$milestone -cnotin @('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74', 'M0.9.76', 'M0.9.87')",
    "'M0.9.76' { 'M0976' }"
)) {
    if (-not $common.Contains($required)) {
        throw "M0.9.76 common helper is missing '$required'"
    }
}

foreach ($required in @(
    "'M0.9.76' { 'm0976' }",
    "'mailbox_capability_format=v2-exact-volunteer'",
    "'mailbox_service_descriptor_input=absent'",
    "`$manifest['central_service_descriptor_present'] = `$false",
    "`$manifest['mailbox_capability_format'] = 'v2-exact-volunteer'"
)) {
    if (-not $alicePrepare.Contains($required)) {
        throw "M0.9.76 Alice preparation is missing '$required'"
    }
}

foreach ($required in @(
    "'runtime-mailbox-exact-offer-create'",
    'if (-not $serviceFreeV2)',
    "'mailbox_capability_format=v2-exact-volunteer'",
    "'mailbox_service_descriptor=absent'",
    'mailbox_service_url|mailbox_store_key'
)) {
    if (-not $bobPrepare.Contains($required)) {
        throw "M0.9.76 Bob activation is missing '$required'"
    }
}

foreach ($required in @(
    "'^runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer$'",
    "'mailbox_capability_format=v2-exact-volunteer'",
    "'mailbox_service_descriptor=absent'",
    'if (-not $noHttpsCompatibility)',
    'M0.9.76 must not contain or start the HTTPS compatibility store.',
    'ALICE SERVICE-FREE V2 SEND COMPLETED'
)) {
    if (-not $aliceSend.Contains($required)) {
        throw "M0.9.76 Alice send is missing '$required'"
    }
}

foreach ($required in @(
    "'M0.9.76' { 'm0976' }",
    'verify-kilogram-m0976-service-free-v2-evidence.ps1',
    'M0.9.76 SERVICE-FREE V2 VOLUNTEER DELIVERY TEST COMPLETED SUCCESSFULLY.'
)) {
    if (-not $aliceVerify.Contains($required)) {
        throw "M0.9.76 final verification is missing '$required'"
    }
}

if (-not $baseVerifier.Contains("[ValidateSet('m0969', 'm0972', 'm0973', 'm0974', 'm0976', 'm0987')]")) {
    throw 'base exact-locator verifier does not accept the m0976 label'
}
foreach ($required in @(
    "-LabelPrefix 'm0976' -SuppressReport",
    'central_service_descriptor_present',
    "'compatibility_endpoint'",
    'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
    'central_service_tuple_rejected=true',
    'suppressed_legacy_copy_rejected=true',
    'm0976_service_free_v2_kit_boundary=verified'
)) {
    if (-not $verifier.Contains($required)) {
        throw "M0.9.76 evidence verifier is missing '$required'"
    }
}

foreach ($required in @(
    'distinct `M0.9.76` build milestone',
    '`m0976` conversation/message',
    '`runtime-mailbox-exact-offer-create`',
    'never contains a mailbox URL',
    'creates no ZIP or release build',
    'starts no network process'
)) {
    if (-not $rfc.Contains($required)) {
        throw "RFC-0099 is missing '$required'"
    }
}

& (Join-Path $workspace 'scripts\verify-kilogram-m0974-no-https-kit-boundary.ps1') | Out-Null
& $serviceFreeGatePath | Out-Null
& $verifierPath -SelfTest | Out-Null

Write-Output 'm0976_service_free_v2_kit_boundary=verified'
Write-Output 'inherits_m0974_lifecycle_contract=true'
Write-Output 'mailbox_capability_format=v2-exact-volunteer'
Write-Output 'central_service_descriptor_present=false'
Write-Output 'compatibility_endpoint_present=false'
Write-Output 'evidence_label=m0976'
Write-Output 'profile=debug'
Write-Output 'archive=false'
Write-Output 'operator_launches=6'
Write-Output 'generator_network_execution=false'
Write-Output 'new_executable=false'
