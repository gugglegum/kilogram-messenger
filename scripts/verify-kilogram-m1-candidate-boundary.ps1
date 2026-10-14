[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$recordPath = Join-Path $workspace 'M1-CANDIDATE.json'
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-m1-candidate.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0116-fresh-field-evidence-and-candidate-refresh.md'
foreach ($path in @($recordPath, $verifierPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "M1 candidate boundary file is missing: $path"
    }
}

$recordRaw = Get-Content -LiteralPath $recordPath -Raw
$record = $recordRaw | ConvertFrom-Json
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    '"milestone": "M0.9.93"',
    '"scope": "windows-technical-proof-of-concept"',
    '"run_id": "20261014-104506"',
    '"source_revision": "d33dd0950484665ee397d161e37ce74743ee8f93"',
    '"result": "verified-reduced-two-host"',
    '"topology_profile": "two-host-reduced"',
    '"independent_provider_field_acceptance": false',
    '"workflow_run_id": "38507114425"',
    '"attestation_id": "55683374"',
    '"sha256": "d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4"',
    '"network_retest_required": false',
    '"release_build": false',
    '"archive_created": false',
    '"git_tag_created": false',
    '"not-a-public-security-release"'
)) {
    if ($recordRaw.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "M1 candidate record is missing '$required'"
    }
}

foreach ($required in @(
    'git-ls-tree-sha256-v1',
    'merge-base',
    '--is-ancestor',
    '--untracked-files=all',
    'Assert-CandidateControlCommitted',
    'M1 candidate control surface is not committed cleanly',
    'field-tested runtime/protocol surface',
    'independently reproduced offline artifact surface',
    'tampered_field_revision',
    'tampered_artifact_sha256',
    'narrowed_runtime_surface',
    'escalated_independent_provider_claim',
    'omitted_residual_risk',
    'verify-kilogram-m0987-independent-provider-kit-boundary.ps1',
    'verify-kilogram-independent-builder-boundary.ps1',
    'verify-kilogram-m1-acceptance-kit-boundary.ps1',
    'm1_candidate_status=verified',
    'network_retest_required=false',
    'network_executed=false',
    'release_build=false',
    'archive_created=false',
    'git_tag_created=false'
)) {
    if ($verifier.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "M1 candidate verifier is missing '$required'"
    }
}
foreach ($forbidden in @(
    'Invoke-WebRequest',
    'Invoke-RestMethod',
    'Start-Process',
    'cargo build',
    'cargo test',
    '--release',
    'Compress-Archive',
    'SkipGit',
    'SkipField',
    'SkipAttestation',
    'AllowDirty'
)) {
    if ($verifier.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "M1 candidate verifier contains forbidden bypass/build/network surface: $forbidden"
    }
}

if ([int]$record.format_version -ne 2 -or
    [string]$record.status -cne 'candidate' -or
    [bool]$record.stage_boundary.network_executed -or
    [bool]$record.stage_boundary.release_build -or
    [bool]$record.stage_boundary.archive_created -or
    [bool]$record.stage_boundary.background_service_created -or
    [bool]$record.stage_boundary.git_tag_created) {
    throw 'M1 candidate record stage boundary is invalid'
}

foreach ($required in @(
    'does not claim a public security release',
    'does not require another field network run',
    'exact runtime/protocol Git surface',
    'strict independent-provider field acceptance remains pending',
    'Sybil resistance remains incomplete'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0116 is missing '$required'"
    }
}

$selfTestOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $verifierPath -SelfTest 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'm1_candidate_self_test=passed' -notin @($selfTestOutput | ForEach-Object { [string]$_ })) {
    throw "M1 candidate verifier self-test failed.`n$($selfTestOutput -join [Environment]::NewLine)"
}

Write-Output 'm1_candidate_boundary=verified'
Write-Output 'evidence_composition=field-plus-independent-reproduction'
Write-Output 'field_evidence=verified-reduced-two-host'
Write-Output 'strict_independent_provider_field_acceptance=pending'
Write-Output 'runtime_protocol_drift=fail-closed'
Write-Output 'network_retest_required=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'background_service_created=false'
Write-Output 'git_tag_created=false'
