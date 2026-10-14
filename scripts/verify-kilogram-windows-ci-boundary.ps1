[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workflowPath = Join-Path $workspace '.github\workflows\windows-ci.yml'
$rfcPath = Join-Path $workspace 'docs\RFC-0114-continuous-windows-readiness-and-tests.md'
foreach ($path in @($workflowPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Windows CI boundary file is missing: $path"
    }
}

$workflow = Get-Content -LiteralPath $workflowPath -Raw
$workflowLines = @($workflow -split '\r?\n')
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($requiredLine in @('  push:', '  pull_request:', '  workflow_dispatch:')) {
    if ($workflowLines -cnotcontains $requiredLine) {
        throw "Windows CI workflow is missing exact line '$requiredLine'"
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
    'CARGO_NET_OFFLINE: "true"',
    '-File .\scripts\verify-kilogram-m1-readiness.ps1',
    '-File .\scripts\run-cargo-tests-stable.ps1',
    '-CargoJobs 2',
    '-Run',
    '-NoFailFast'
)) {
    if ($workflow.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Windows CI workflow is missing '$required'"
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
    'every push and pull request to master',
    'complete Git history',
    'read-only repository permission',
    'does not create a release build, archive, attestation or tag',
    'stable-path Rust test harnesses',
    'GitHub-hosted Windows runner is not physical-device field evidence'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0114 is missing '$required'"
    }
}

Write-Output 'windows_ci_boundary=verified'
Write-Output 'events=push-master,pull-request-master,manual'
Write-Output 'permissions=contents-read'
Write-Output 'checkout_history=complete'
Write-Output 'readiness_gate=required'
Write-Output 'stable_rust_tests=required'
Write-Output 'cargo_jobs=2'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'attestation_created=false'
Write-Output 'git_tag_created=false'
