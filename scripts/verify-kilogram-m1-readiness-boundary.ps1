[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-m1-readiness.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0113-complete-m1-readiness-gate.md'
foreach ($path in @($verifierPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M1 readiness boundary file is missing: $path"
    }
}

$verifier = Get-Content -LiteralPath $verifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    "'verify-kilogram-m1-readiness-boundary.ps1' = 'm1_readiness_boundary=verified'",
    "Get-ChildItem -LiteralPath `$scriptsDirectory -Filter 'verify-*-boundary.ps1'",
    'M1 readiness boundary inventory must contain exactly 33 gates',
    'm1_candidate_status=verified',
    'cargo fmt --all -- --check',
    "-CargoCommand clippy",
    "-CargoJobs 2",
    "'--workspace', '--all-targets', '--all-features', '--locked'",
    "'--offline', '--', '-D', 'warnings'",
    'missing_success_marker',
    'changed_boundary_inventory',
    'm1_readiness_status=verified',
    'boundary_gates=',
    'test_executables_started=false',
    'network_executed=false',
    'release_build=false',
    'archive_created=false',
    'git_tag_created=false',
    'strict_independent_provider_field_acceptance=pending',
    'public_security_release=false'
)) {
    if ($verifier.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "M1 readiness verifier is missing '$required'"
    }
}

foreach ($forbidden in @(
    'Invoke-WebRequest',
    'Invoke-RestMethod',
    'Start-Process',
    'cargo test',
    '--release',
    'Compress-Archive',
    'git tag',
    'git push',
    'SkipGate',
    'AllowDirty'
)) {
    if ($verifier.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "M1 readiness verifier contains forbidden execution/bypass surface: $forbidden"
    }
}

foreach ($required in @(
    'closed inventory of every boundary contract',
    'does not execute Rust test binaries',
    'does not create a release build, archive or Git tag',
    'strict independent-provider field acceptance remains pending',
    'not a public security release'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0113 is missing '$required'"
    }
}

$selfTestOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $verifierPath -SelfTest 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'm1_readiness_self_test=passed' -notin @($selfTestOutput | ForEach-Object { [string]$_ })) {
    throw "M1 readiness verifier self-test failed.`n$($selfTestOutput -join [Environment]::NewLine)"
}

Write-Output 'm1_readiness_boundary=verified'
Write-Output 'boundary_inventory=closed-33'
Write-Output 'candidate_production_verifier=required'
Write-Output 'format_and_offline_strict_clippy=required'
Write-Output 'rust_test_executables_started=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
