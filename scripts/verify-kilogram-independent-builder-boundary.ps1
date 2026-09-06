[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workflowPath = Join-Path $workspace '.github\workflows\independent-offline-reproduction.yml'
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-independent-builder.ps1'
foreach ($path in @($workflowPath, $verifierPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Independent-builder boundary file is missing: $path"
    }
}

$workflow = Get-Content -LiteralPath $workflowPath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw

foreach ($required in @(
    'workflow_dispatch:',
    'source_revision:',
    'expected_local_sha256:',
    'runs-on: windows-2025',
    'contents: read',
    'id-token: write',
    'attestations: write',
    'artifact-metadata: write',
    'persist-credentials: false',
    'cargo fetch --locked --target x86_64-pc-windows-msvc',
    'cargo build --jobs 2 --frozen --release --target x86_64-pc-windows-msvc --package kilogram-offline',
    'actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10',
    'actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6',
    "if: steps.reproduce.outputs.matches_expected == 'true'",
    'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a',
    'compression-level: 0',
    'Fail closed on byte divergence'
)) {
    if ($workflow.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Independent-builder workflow boundary is missing: $required"
    }
}
foreach ($forbidden in @(
    "`n  push:",
    "`n  pull_request:",
    "`n  schedule:",
    "`n  release:",
    'Compress-Archive',
    'package-kilogram-offline.ps1',
    'runs-on: self-hosted',
    'persist-credentials: true'
)) {
    if ($workflow.IndexOf($forbidden, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "Independent-builder workflow contains forbidden automatic/package boundary: $forbidden"
    }
}
foreach ($actionUse in [regex]::Matches($workflow, '(?m)^\s*uses:\s+([^\s#]+)')) {
    if ($actionUse.Groups[1].Value -cnotmatch '@[0-9a-f]{40}$') {
        throw "Every third-party workflow action must be pinned to an exact commit: $($actionUse.Groups[1].Value)"
    }
}

foreach ($required in @(
    "builder_scope -ne 'github-hosted-windows-independent'",
    "workflow.event -ne 'workflow_dispatch'",
    "workflow.name -ne 'Independent offline reproduction'",
    "runner.environment -ne 'github-hosted'",
    "attestation verify",
    '--signer-workflow',
    '--source-digest',
    '--deny-self-hosted-runners',
    "artifact.file -ne 'kilogram-offline.exe'",
    'verify-kilogram-offline-reproducibility-record.ps1',
    'SelfTest does not accept external evidence parameters.'
)) {
    if ($verifier.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Independent-builder verifier boundary is missing: $required"
    }
}
if ($verifier.IndexOf('SkipAttestationForProduction', [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
    throw 'Production independent-builder verification must not expose an attestation bypass.'
}

Write-Output 'independent_builder_boundary=verified'
Write-Output 'trigger=workflow-dispatch-only'
Write-Output 'builder=github-hosted-windows'
Write-Output 'artifact=kilogram-offline.exe'
Write-Output 'attestation=required-for-production-verification'
Write-Output 'automatic_release=false'
Write-Output 'local_zip=false'
