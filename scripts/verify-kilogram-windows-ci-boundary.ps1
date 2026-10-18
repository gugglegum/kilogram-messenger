[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workflowPath = Join-Path $workspace '.github\workflows\windows-ci.yml'
$rfcPath = Join-Path $workspace 'docs\RFC-0114-continuous-windows-readiness-and-tests.md'
$postM1RfcPath = Join-Path $workspace 'docs\RFC-0124-post-m1-immutable-acceptance-and-continuous-ci.md'
$isolatedTestsPath = Join-Path $workspace 'scripts\windows-ci-isolated-tests.json'
$runnerPath = Join-Path $workspace 'scripts\run-cargo-tests-stable.ps1'
$cacheMaintenancePath = Join-Path $workspace 'scripts\cargo-cache-maintenance.ps1'
foreach ($path in @(
    $workflowPath,
    $rfcPath,
    $postM1RfcPath,
    $isolatedTestsPath,
    $runnerPath,
    $cacheMaintenancePath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Windows CI boundary file is missing: $path"
    }
}

$workflow = Get-Content -LiteralPath $workflowPath -Raw
$workflowLines = @($workflow -split '\r?\n')
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$postM1Rfc = Get-Content -LiteralPath $postM1RfcPath -Raw
$runner = Get-Content -LiteralPath $runnerPath -Raw
$cacheMaintenance = Get-Content -LiteralPath $cacheMaintenancePath -Raw

foreach ($requiredLine in @('  workflow_dispatch:')) {
    if ($workflowLines -cnotcontains $requiredLine) {
        throw "Windows CI workflow is missing exact line '$requiredLine'"
    }
}

foreach ($forbiddenLine in @('  push:', '  pull_request:')) {
    if ($workflowLines -ccontains $forbiddenLine) {
        throw "Windows CI workflow must remain manual-only; found exact line '$forbiddenLine'"
    }
}

foreach ($required in @(
    'name: Windows CI',
    'permissions:',
    '  contents: read',
    'cancel-in-progress: true',
    'runs-on: windows-2022',
    'timeout-minutes: 45',
    'shell: pwsh',
    'actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10',
    'persist-credentials: false',
    'fetch-depth: 0',
    'cargo fetch --locked --target x86_64-pc-windows-msvc',
    'id: fetch',
    'CARGO_NET_OFFLINE: "true"',
    '-File .\scripts\verify-kilogram-m1-readiness.ps1',
    '-AcceptedTag M1.0.0',
    '-File .\scripts\run-cargo-tests-stable.ps1',
    "if: `${{ always() && steps.fetch.outcome == 'success' }}",
    'timeout-minutes: 38',
    '-Profile test',
    '-CargoJobs 2',
    '-IsolatedTestsFile .\scripts\windows-ci-isolated-tests.json',
    '-IsolatedTestOrder BeforeAggregate',
    '-NormalProcessPriority',
    '-Run',
    '-NoFailFast'
)) {
    if ($workflow.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Windows CI workflow is missing '$required'"
    }
}

foreach ($required in @(
    '[switch]$SelfTest',
    '$measure = @(',
    'if ($measure.Count -eq 0)',
    '$summary.PSObject.Properties[''Sum'']',
    'cargo_cache_measurement_self_test=passed',
    'missing_tree=zero',
    'empty_tree=zero',
    'non_empty_tree=exact'
)) {
    if ($cacheMaintenance.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Cargo cache maintenance is missing empty-tree contract '$required'"
    }
}

foreach ($required in @(
    'immutable annotated',
    '`M1.0.0` tag',
    'does not refresh, rewrite or reclassify',
    'current boundaries, formatting',
    'strict Clippy and Rust tests',
    '`M1.0.1` is a governance snapshot',
    'does not create or move a tag'
)) {
    if ($postM1Rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0124 is missing '$required'"
    }
}

$cacheSelfTestOutput = @(
    & powershell -NoProfile -ExecutionPolicy Bypass -File $cacheMaintenancePath -SelfTest 2>&1
)
if ($LASTEXITCODE -ne 0 -or
    'cargo_cache_measurement_self_test=passed' -notin @($cacheSelfTestOutput | ForEach-Object { [string]$_ })) {
    throw "Cargo cache empty-tree self-test failed.`n$($cacheSelfTestOutput -join [Environment]::NewLine)"
}

$isolatedManifest = Get-Content -LiteralPath $isolatedTestsPath -Raw | ConvertFrom-Json
$isolatedProperties = @($isolatedManifest.PSObject.Properties.Name | Sort-Object)
if (($isolatedProperties -join ',') -cne 'format_version,tests' -or
    [int]$isolatedManifest.format_version -ne 1) {
    throw 'Windows CI isolated test manifest must be exact format version 1'
}
$isolatedNames = @($isolatedManifest.tests | ForEach-Object { [string]$_.name } | Sort-Object)
$expectedIsolatedNames = @(
    'tests::live_runtime_applies_revocation_republishes_ticket_and_retires_ratchet'
) | Sort-Object
if (($isolatedNames -join "`n") -cne ($expectedIsolatedNames -join "`n")) {
    throw 'Windows CI isolated test manifest does not contain the exact bounded test set'
}
foreach ($entry in @($isolatedManifest.tests)) {
    $entryProperties = @($entry.PSObject.Properties.Name | Sort-Object)
    if (($entryProperties -join ',') -cne 'kind,name,package,target' -or
        [string]$entry.package -cne 'kilogram-cli' -or
        [string]$entry.kind -cne 'bin' -or
        [string]$entry.target -cne 'kilogram-cli') {
        throw 'Windows CI isolated test selector is not exact'
    }
}

foreach ($required in @(
    '[string]$IsolatedTestsFile',
    "[ValidateSet('AfterAggregate', 'BeforeAggregate')]",
    '[string]$IsolatedTestOrder = ''AfterAggregate''',
    '[switch]$NormalProcessPriority',
    'cargo_process_priority_override=normal',
    'Filter and IsolatedTestsFile cannot be combined.',
    'Duplicate isolated test entry:',
    "'--list' '--format' 'terse'",
    'does not resolve to exactly one test:',
    "@('--skip', `$isolatedTest.Name)",
    "@(`$isolatedTest.Name, '--exact')",
    'running_isolated_test=',
    'isolated_test_order=',
    "`$IsolatedTestOrder -ceq 'BeforeAggregate'",
    "`$IsolatedTestOrder -ceq 'AfterAggregate'",
    'isolated_test_count='
)) {
    if ($runner.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Stable test runner is missing isolated-process contract '$required'"
    }
}

foreach ($forbidden in @(
    'pull_request_target',
    'contents: write',
    'id-token: write',
    'attestations: write',
    'actions/upload-artifact',
    'continue-on-error',
    '--release',
    'Compress-Archive',
    'git tag',
    'git push',
    'gh release'
)) {
    if ($workflow.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "Windows CI workflow contains forbidden privilege/release/bypass surface: $forbidden"
    }
}

$actionUses = @(
    [regex]::Matches($workflow, '(?m)^\s*uses:\s*([^\s#]+)') |
        ForEach-Object { $_.Groups[1].Value }
)
if ($actionUses.Count -ne 1 -or
    $actionUses[0] -cnotmatch '^actions/checkout@[0-9a-f]{40}$') {
    throw 'Windows CI actions must be a closed set pinned to full commit SHA'
}

foreach ($required in @(
    'only by an explicit manual dispatch',
    'does not run automatically on push or pull request',
    'complete Git history',
    'read-only repository permission',
    'does not create a release build, archive, attestation or tag',
    'stable-path Rust test harnesses',
    'fresh process',
    'normal process priority',
    'before the aggregate harnesses',
    'empty Cargo profile directory',
    'test step still runs when readiness fails',
    'failed readiness step continues to fail the job',
    'GitHub-hosted Windows runner is not physical-device field evidence'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0114 is missing '$required'"
    }
}

Write-Output 'windows_ci_boundary=verified'
Write-Output 'events=manual-only'
Write-Output 'permissions=contents-read'
Write-Output 'checkout_history=complete'
Write-Output 'readiness_gate=required'
Write-Output 'readiness_reference=accepted-M1.0.0'
Write-Output 'stable_rust_tests=required'
Write-Output 'cargo_jobs=2'
Write-Output 'test_profile=test'
Write-Output 'isolated_tests=1'
Write-Output 'isolated_test_order=before-aggregate'
Write-Output 'test_process_priority=normal'
Write-Output 'empty_cache_profile_measurement=verified'
Write-Output 'tests_after_readiness_failure=required-if-fetch-succeeded'
Write-Output 'test_step_timeout_minutes=38'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'attestation_created=false'
Write-Output 'git_tag_created=false'
