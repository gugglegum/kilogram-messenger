[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$generatorPath = Join-Path $workspace 'scripts\new-kilogram-m0969-exact-locator-kit.ps1'
$wrapperPath = Join-Path $workspace 'scripts\new-kilogram-m0974-no-https-kit.ps1'
$commonPath = Join-Path $workspace 'scripts\m0969-exact\common.ps1'
$alicePreparePath = Join-Path $workspace 'scripts\m0969-exact\1\01_PREPARE_ALICE.ps1'
$aliceVerifyPath = Join-Path $workspace 'scripts\m0969-exact\1\03_VERIFY.ps1'
$baseVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0969-exact-locator-evidence.ps1'
$noHttpsVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-m0972-no-https-evidence.ps1'
$recoveryGatePath = Join-Path $workspace 'scripts\verify-kilogram-runtime-mailbox-replication-recovery.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0097-replication-ledger-crash-recovery.md'

foreach ($path in @(
    $generatorPath,
    $wrapperPath,
    $commonPath,
    $alicePreparePath,
    $aliceVerifyPath,
    $baseVerifierPath,
    $noHttpsVerifierPath,
    $recoveryGatePath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M0.9.74 no-HTTPS kit source is missing: $path"
    }
}

$generator = Get-Content -LiteralPath $generatorPath -Raw
$wrapper = Get-Content -LiteralPath $wrapperPath -Raw
$common = Get-Content -LiteralPath $commonPath -Raw
$alicePrepare = Get-Content -LiteralPath $alicePreparePath -Raw
$aliceVerify = Get-Content -LiteralPath $aliceVerifyPath -Raw
$baseVerifier = Get-Content -LiteralPath $baseVerifierPath -Raw
$noHttpsVerifier = Get-Content -LiteralPath $noHttpsVerifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    "-Milestone 'M0.9.74'",
    '[ValidateRange(1, 64)] [int] $CargoJobs = 2'
)) {
    if (-not $wrapper.Contains($required)) {
        throw "M0.9.74 generator wrapper is missing '$required'"
    }
}

foreach ($required in @(
    "[ValidateSet('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74', 'M0.9.76')] [string] `$Milestone = 'M0.9.69'",
    "'M0.9.74' { 'm0974-no-https' }",
    "'verify-kilogram-runtime-mailbox-replication-recovery.ps1'",
    "'verify-kilogram-m0974-no-https-kit-boundary.ps1'",
    'status=kilogram-m0974-no-https-kit-created',
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli',
    "profile = 'debug'",
    "archive = `$false",
    "network_executed = `$false"
)) {
    if (-not $generator.Contains($required)) {
        throw "M0.9.74 generator is missing '$required'"
    }
}
foreach ($forbidden in @('--release', 'Compress-Archive', '.zip')) {
    if ($generator.Contains($forbidden)) {
        throw "M0.9.74 generator contains forbidden action '$forbidden'"
    }
}

foreach ($required in @(
    "`$milestone -cnotin @('M0.9.69', 'M0.9.72', 'M0.9.73', 'M0.9.74', 'M0.9.76', 'M0.9.87')",
    "'M0.9.74' { 'M0974' }"
)) {
    if (-not $common.Contains($required)) {
        throw "M0.9.74 common helper is missing '$required'"
    }
}
if (-not $alicePrepare.Contains("'M0.9.74' { 'm0974' }")) {
    throw 'M0.9.74 Alice preparation does not use a distinct evidence label'
}
foreach ($required in @(
    "'M0.9.74' { 'm0974' }",
    '-LabelPrefix $labelPrefix',
    'CLEAN NO-HTTPS VOLUNTEER DELIVERY TEST COMPLETED SUCCESSFULLY.'
)) {
    if (-not $aliceVerify.Contains($required)) {
        throw "M0.9.74 final verification is missing '$required'"
    }
}
if (-not $baseVerifier.Contains("[ValidateSet('m0969', 'm0972', 'm0973', 'm0974', 'm0976', 'm0987')]")) {
    throw 'base exact-locator verifier does not accept the m0974 label'
}
foreach ($required in @(
    "[ValidateSet('m0972', 'm0973', 'm0974')] [string] `$LabelPrefix = 'm0972'",
    "[ValidateSet('M0.9.72', 'M0.9.73', 'M0.9.74')] [string] `$ExpectedMilestone = 'M0.9.72'",
    'm0974_no_https_evidence_self_test=verified',
    'runtime_mailbox_replication_recovery=verified',
    'm0974_no_https_kit_boundary=verified'
)) {
    if (-not $noHttpsVerifier.Contains($required)) {
        throw "M0.9.74 evidence verifier is missing '$required'"
    }
}
foreach ($required in @(
    'distinct `M0.9.74` milestone',
    '`m0974` evidence label',
    'does not delete, recreate, truncate',
    'operator flow remains the same six launches'
)) {
    if (-not $rfc.Contains($required)) {
        throw "RFC-0097 is missing '$required'"
    }
}

& (Join-Path $workspace 'scripts\verify-kilogram-m0973-no-https-kit-boundary.ps1') | Out-Null
& $recoveryGatePath | Out-Null

Write-Output 'm0974_no_https_kit_boundary=verified'
Write-Output 'inherits_m0973_no_https_contract=true'
Write-Output 'runtime_mailbox_replication_recovery=required'
Write-Output 'evidence_label=m0974'
Write-Output 'profile=debug'
Write-Output 'archive=false'
Write-Output 'operator_launches=6'
Write-Output 'generator_network_execution=false'
Write-Output 'new_executable=false'
