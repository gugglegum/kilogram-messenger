[CmdletBinding()]
param(
    [string[]]$Package = @(),
    [string]$Filter,
    [string]$Profile = 'verification',
    [int]$CargoJobs = 0,
    [switch]$Run,
    [switch]$NoFailFast,
    [switch]$NoCapture
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs

$workspaceRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $workspaceRoot 'Cargo.toml'
$metadataOutput = & cargo metadata --manifest-path $manifestPath --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed with exit code $LASTEXITCODE"
}
$metadata = ($metadataOutput -join [Environment]::NewLine) | ConvertFrom-Json
$packageNamesById = @{}
foreach ($metadataPackage in $metadata.packages) {
    $packageNamesById[[string]$metadataPackage.id] = [string]$metadataPackage.name
}

$cargoArguments = @(
    'test'
    '--manifest-path', $manifestPath
    '--locked'
    '--jobs', $cargoJobsResolved
    '--profile', $Profile
    '--no-run'
    '--message-format=json-render-diagnostics'
)

if ($Package.Count -eq 0) {
    $cargoArguments += '--workspace'
} else {
    foreach ($packageName in $Package) {
        if ([string]::IsNullOrWhiteSpace($packageName)) {
            throw 'Package names must not be empty.'
        }
        $cargoArguments += @('--package', $packageName)
    }
}

Write-Host "Compiling Rust test harnesses with profile '$Profile' without executing them..."
Push-Location $workspaceRoot
try {
    $cargoOutput = @(& cargo @cargoArguments)
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test --no-run failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

$artifactsByExecutable = @{}
foreach ($line in $cargoOutput) {
    try {
        $message = $line | ConvertFrom-Json -ErrorAction Stop
    } catch {
        continue
    }

    if ($message.reason -ne 'compiler-artifact' -or
        -not $message.profile.test -or
        [string]::IsNullOrWhiteSpace([string]$message.executable)) {
        continue
    }

    $executable = [System.IO.Path]::GetFullPath([string]$message.executable)
    if ([System.IO.Path]::GetExtension($executable) -ne '.exe') {
        continue
    }
    $artifactsByExecutable[$executable] = $message
}

if ($artifactsByExecutable.Count -eq 0) {
    throw 'Cargo did not report any Windows test executables.'
}

$stableArtifacts = @()
foreach ($entry in $artifactsByExecutable.GetEnumerator()) {
    $sourcePath = $entry.Key
    $message = $entry.Value
    $packageId = [string]$message.package_id
    $packageName = [string]$message.target.name
    if ($packageNamesById.ContainsKey($packageId)) {
        $packageName = $packageNamesById[$packageId]
    }
    $targetName = [string]$message.target.name
    $targetKind = (@($message.target.kind) -join '-')
    $stableStem = "$packageName-$targetKind-$targetName-tests"
    $stableStem = $stableStem -replace '[^A-Za-z0-9._-]', '-'
    $stablePath = Join-Path ([System.IO.Path]::GetDirectoryName($sourcePath)) "$stableStem.exe"

    Copy-Item -LiteralPath $sourcePath -Destination $stablePath -Force
    $stableArtifacts += [PSCustomObject]@{
        Package = $packageName
        Kind = $targetKind
        Target = $targetName
        Path = $stablePath
    }
}

$stableArtifacts = @($stableArtifacts | Sort-Object Package, Kind, Target, Path -Unique)
foreach ($artifact in $stableArtifacts) {
    Write-Host "stable_test_executable=$($artifact.Path)"
}

if (-not $Run) {
    Write-Host "test_profile=$Profile"
    Write-Host 'status=compiled-only'
    Write-Host 'No test executable was started. Pass -Run explicitly when it is safe to run tests.'
    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'cargo-cache-maintenance.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw 'Cargo cache status check failed.'
    }
    exit 0
}

Write-Warning 'Executing stable-path test harnesses. A network-listening harness can request one initial Windows Firewall approval for this stable path.'
$testArguments = @()
if (-not [string]::IsNullOrWhiteSpace($Filter)) {
    $testArguments += $Filter
}
if ($NoCapture) {
    $testArguments += '--nocapture'
}
$testArguments += '--test-threads=1'

$failedArtifacts = @()
foreach ($artifact in $stableArtifacts) {
    Write-Host "running_test_executable=$($artifact.Path)"
    Push-Location $workspaceRoot
    try {
        & $artifact.Path @testArguments
        $testExitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }

    if ($testExitCode -ne 0) {
        $failedArtifacts += $artifact.Path
        if (-not $NoFailFast) {
            throw "Stable test executable failed with exit code ${testExitCode}: $($artifact.Path)"
        }
    }
}

if ($failedArtifacts.Count -ne 0) {
    throw "One or more stable test executables failed: $($failedArtifacts -join ', ')"
}

Write-Host 'status=passed'
& powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'cargo-cache-maintenance.ps1')
if ($LASTEXITCODE -ne 0) {
    throw 'Cargo cache status check failed.'
}
