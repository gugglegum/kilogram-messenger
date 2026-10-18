[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$cargoPath = Join-Path $root 'apps\kilogram-cli\Cargo.toml'
$evidencePath = Join-Path $root 'apps\kilogram-cli\src\bin\kilogram-sync-v2-evidence.rs'
$productionPath = Join-Path $root 'apps\kilogram-cli\src\main.rs'
$transportPath = Join-Path $root 'crates\kilogram-transport-iroh\src\lib.rs'
$workflowPath = Join-Path $root '.github\workflows\canonical-sync-v2-evidence.yml'
$runnerPath = Join-Path $root 'scripts\invoke-kilogram-sync-v2-hosted-evidence.ps1'
$verifierPath = Join-Path $root 'scripts\verify-kilogram-sync-v2-network-evidence.ps1'

foreach ($path in @(
    $cargoPath, $evidencePath, $productionPath, $transportPath,
    $workflowPath, $runnerPath, $verifierPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "sync v2 evidence boundary input is missing: $path"
    }
}

$cargo = (Get-Content -LiteralPath $cargoPath -Raw).Replace("`r`n", "`n")
$evidence = (Get-Content -LiteralPath $evidencePath -Raw).Replace("`r`n", "`n")
$production = (Get-Content -LiteralPath $productionPath -Raw).Replace("`r`n", "`n")
$transport = (Get-Content -LiteralPath $transportPath -Raw).Replace("`r`n", "`n")
$workflow = (Get-Content -LiteralPath $workflowPath -Raw).Replace("`r`n", "`n")
$runner = (Get-Content -LiteralPath $runnerPath -Raw).Replace("`r`n", "`n")
$verifier = (Get-Content -LiteralPath $verifierPath -Raw).Replace("`r`n", "`n")

foreach ($required in @(
    '[features]',
    'default = []',
    'sync-v2-evidence = ["dep:ed25519-dalek"]',
    'name = "kilogram-sync-v2-evidence"',
    'path = "src/bin/kilogram-sync-v2-evidence.rs"',
    'required-features = ["sync-v2-evidence"]',
    'ed25519-dalek = { workspace = true, optional = true }'
)) {
    if (-not $cargo.Contains($required)) {
        throw "sync v2 evidence Cargo boundary is missing: $required"
    }
}

foreach ($required in @(
    '#![forbid(unsafe_code)]',
    'Feature-gated evidence harness for dormant canonical sync v2',
    'CANONICAL_SYNC_ALPN_V2',
    'verify_durable_mutual_canonical_candidate_selection',
    'relay_url: Option<RelayUrl>',
    'fallback_attempts: 0',
    'production_default_changed=false',
    'selected canonical decode failed; endpoint registered no legacy ALPN'
)) {
    if (-not $evidence.Contains($required)) {
        throw "sync v2 evidence source boundary is missing: $required"
    }
}

foreach ($forbidden in @(
    'kilogram-sync-v2-evidence',
    'sync-v2-evidence',
    'CANONICAL_SYNC_ALPN_V2'
)) {
    if ($production.Contains($forbidden)) {
        throw "production CLI unexpectedly references evidence-only surface: $forbidden"
    }
}

if (-not $transport.Contains('pub const ALPN: &[u8] = b"kilogram/m0/sync/8";') -or
    -not $transport.Contains('pub const CANONICAL_SYNC_ALPN_V2: &[u8] = b"kilogram/m1/sync/cbor/2";')) {
    throw 'sync transport identifiers are not exact'
}

if ([regex]::IsMatch($production, '(?m)^\s*\.alpns\([^\r\n]*CANONICAL_SYNC_ALPN_V2')) {
    throw 'production CLI registers the dormant canonical v2 ALPN'
}

foreach ($required in @(
    "on:`n  workflow_dispatch:",
    '--features sync-v2-evidence',
    '--bin kilogram-sync-v2-evidence',
    'invoke-kilogram-sync-v2-hosted-evidence.ps1',
    '.tmp/canonical-sync-v2-upload/manifest.json'
)) {
    if (-not $workflow.Contains($required)) {
        throw "hosted sync v2 workflow boundary is missing: $required"
    }
}
foreach ($forbidden in @("`n  push:", "`n  pull_request:", '--release')) {
    if ($workflow.Contains($forbidden)) {
        throw "hosted sync v2 workflow contains forbidden automatic/release surface: $forbidden"
    }
}
if (-not $runner.Contains("name = 'kilogram-cli.exe'") -or
    -not $runner.Contains("evidence_kind = 'hosted-windows'") -or
    -not $runner.Contains("distinct_physical_hosts = `$false")) {
    throw 'hosted runner does not preserve the stable-name and topology boundary'
}
foreach ($required in @(
    "'local_device_secret'", "'endpoint_secret'", "'requester_authorization'",
    "'listener_authorization'", "'evidence-role.json'",
    "evidence_kind = 'physical-windows'", "legacy_fallback_after_selection"
)) {
    if (-not $verifier.Contains($required)) {
        throw "network evidence verifier boundary is missing: $required"
    }
}

Write-Output 'sync_v2_evidence_boundary=verified'
Write-Output 'evidence_feature=sync-v2-evidence'
Write-Output 'evidence_binary=kilogram-sync-v2-evidence'
Write-Output 'candidate_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'active_alpn=kilogram/m0/sync/8'
Write-Output 'production_default_changed=false'
Write-Output 'hosted_workflow=manual-only'
