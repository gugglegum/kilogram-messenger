[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-m1-readiness.ps1'
$tagVerifierPath = Join-Path $PSScriptRoot 'verify-kilogram-m1-tag-acceptance.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0113-complete-m1-readiness-gate.md'
foreach ($path in @($verifierPath, $tagVerifierPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M1 readiness boundary file is missing: $path"
    }
}

$verifier = Get-Content -LiteralPath $verifierPath -Raw
$tagVerifier = Get-Content -LiteralPath $tagVerifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    "'verify-kilogram-m1-readiness-boundary.ps1' = 'm1_readiness_boundary=verified'",
    "'verify-kilogram-canonical-authority-boundary.ps1' = 'canonical_authority_boundary=verified'",
    "'verify-kilogram-canonical-account-boundary.ps1' = 'canonical_account_boundary=verified'",
    "'verify-kilogram-canonical-device-link-boundary.ps1' = 'canonical_device_link_boundary=verified'",
    "'verify-kilogram-canonical-event-boundary.ps1' = 'canonical_event_boundary=verified'",
    "'verify-kilogram-canonical-prekey-selection-boundary.ps1' = 'canonical_prekey_selection_boundary=verified'",
    "'verify-kilogram-canonical-conformance-boundary.ps1' = 'canonical_conformance_boundary=verified'",
    "'verify-kilogram-canonical-coverage-boundary.ps1' = 'canonical_coverage_boundary=verified'",
    "'verify-kilogram-canonical-migration-boundary.ps1' = 'canonical_migration_boundary=verified'",
    "'verify-kilogram-canonical-ratchet-boundary.ps1' = 'canonical_ratchet_boundary=verified'",
    "'verify-kilogram-canonical-recovery-boundary.ps1' = 'canonical_recovery_boundary=verified'",
    "'verify-kilogram-canonical-recovery-exchange-boundary.ps1' = 'canonical_recovery_exchange_boundary=verified'",
    "'verify-kilogram-canonical-sync-boundary.ps1' = 'canonical_sync_boundary=verified'",
    "'verify-kilogram-canonical-sync-session-boundary.ps1' = 'canonical_sync_session_boundary=verified'",
    "'verify-kilogram-canonical-wire-boundary.ps1' = 'canonical_wire_boundary=verified'",
    "'verify-kilogram-protocol-compatibility-boundary.ps1' = 'protocol_compatibility_boundary=verified'",
    "Get-ChildItem -LiteralPath `$scriptsDirectory -Filter 'verify-*-boundary.ps1'",
    'M1 readiness boundary inventory must contain exactly 48 gates',
    '[string]$AcceptedTag',
    "'verify-kilogram-m1-tag-acceptance.ps1'",
    'm1_candidate_status=verified',
    'm1_tag_acceptance_status=verified',
    'accepted_m1_tag_status=verified',
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

foreach ($required in @(
    "`$expectedTag = 'M1.0.0'",
    "`$expectedTagObject = '15e74f7a786070c247d4d1a0f023adde8b250922'",
    "`$expectedCommit = '60b20b364c8df2324a0a287b4bcb6c056ae12c39'",
    "`$expectedTree = '6e3183e6a4cf754a4aa3f7788c5e8be4e70cf9f3'",
    "`$expectedCandidateBlob = 'c24389d673006deb28b09c66e4accfcc00e600f7'",
    "`$expectedAcceptanceRfcBlob = '38b791db2a96437e6fe10debd2375210fd2c22fe'",
    "`$expectedDecisionBlob = 'bae5c73ff79bf4477e7d030268cca532f83ccaed'",
    "@('cat-file', '-t', `$tagRef)",
    "@('merge-base', '--is-ancestor', `$expectedCommit, 'HEAD')",
    'm1_tag_acceptance_self_test=passed',
    'm1_tag_acceptance_status=verified',
    'historical_candidate_reclassified=false',
    'working_tree_mutated=false'
)) {
    if ($tagVerifier.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "M1 tag acceptance verifier is missing '$required'"
    }
}

foreach ($forbidden in @(
    'Invoke-WebRequest',
    'Invoke-RestMethod',
    'Start-Process',
    'cargo ',
    'worktree',
    "@('checkout'",
    "@('reset'",
    "@('switch'",
    "@('clean'",
    'Remove-Item',
    "@('push'",
    "@('fetch'",
    'git push',
    'git fetch'
)) {
    if ($tagVerifier.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "M1 tag acceptance verifier contains forbidden mutation/network surface: $forbidden"
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

$tagSelfTestOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $tagVerifierPath -SelfTest 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'm1_tag_acceptance_self_test=passed' -notin @($tagSelfTestOutput | ForEach-Object { [string]$_ })) {
    throw "M1 tag acceptance verifier self-test failed.`n$($tagSelfTestOutput -join [Environment]::NewLine)"
}

Write-Output 'm1_readiness_boundary=verified'
Write-Output 'boundary_inventory=closed-48'
Write-Output 'candidate_or_accepted_tag_verifier=required'
Write-Output 'accepted_m1_tag=M1.0.0'
Write-Output 'format_and_offline_strict_clippy=required'
Write-Output 'rust_test_executables_started=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
