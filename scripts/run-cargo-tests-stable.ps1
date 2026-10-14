[CmdletBinding()]
param(
    [string[]]$Package = @(),
    [string]$Filter,
    [string]$Profile = 'verification',
    [int]$CargoJobs = 0,
    [string]$IsolatedTestsFile,
    [ValidateSet('AfterAggregate', 'BeforeAggregate')]
    [string]$IsolatedTestOrder = 'AfterAggregate',
    [switch]$NormalProcessPriority,
    [switch]$Run,
    [switch]$NoFailFast,
    [switch]$NoCapture
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs
if ($NormalProcessPriority) {
    try {
        [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass =
            [System.Diagnostics.ProcessPriorityClass]::Normal
        Write-Host 'cargo_process_priority_override=normal'
    }
    catch {
        throw "Could not set the explicitly requested normal process priority: $($_.Exception.Message)"
    }
}

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

$isolatedTests = @()
if (-not [string]::IsNullOrWhiteSpace($IsolatedTestsFile)) {
    $isolatedTestsPath = [System.IO.Path]::GetFullPath($IsolatedTestsFile)
    if (-not (Test-Path -LiteralPath $isolatedTestsPath -PathType Leaf)) {
        throw "Isolated test manifest does not exist: $isolatedTestsPath"
    }
    $isolatedManifest = Get-Content -LiteralPath $isolatedTestsPath -Raw | ConvertFrom-Json
    $manifestProperties = @($isolatedManifest.PSObject.Properties.Name | Sort-Object)
    if (($manifestProperties -join ',') -cne 'format_version,tests' -or
        [int]$isolatedManifest.format_version -ne 1) {
        throw 'Isolated test manifest must be exact format version 1.'
    }
    $seenIsolatedTests = @{}
    foreach ($entry in @($isolatedManifest.tests)) {
        $entryProperties = @($entry.PSObject.Properties.Name | Sort-Object)
        if (($entryProperties -join ',') -cne 'kind,name,package,target') {
            throw 'Each isolated test entry must contain only package, kind, target and name.'
        }
        foreach ($property in @('package', 'kind', 'target', 'name')) {
            if ([string]::IsNullOrWhiteSpace([string]$entry.$property)) {
                throw "Isolated test property '$property' must not be empty."
            }
        }
        $key = "$($entry.package)|$($entry.kind)|$($entry.target)|$($entry.name)"
        if ($seenIsolatedTests.ContainsKey($key)) {
            throw "Duplicate isolated test entry: $key"
        }
        $seenIsolatedTests[$key] = $true
        $matches = @($stableArtifacts | Where-Object {
            $_.Package -ceq [string]$entry.package -and
            $_.Kind -ceq [string]$entry.kind -and
            $_.Target -ceq [string]$entry.target
        })
        if ($matches.Count -ne 1) {
            throw "Isolated test entry must match exactly one stable executable: $key"
        }
        $isolatedTests += [PSCustomObject]@{
            Package = [string]$entry.package
            Kind = [string]$entry.kind
            Target = [string]$entry.target
            Name = [string]$entry.name
            Path = [string]$matches[0].Path
        }
    }
    if ($isolatedTests.Count -eq 0) {
        throw 'Isolated test manifest must contain at least one test.'
    }
    if (-not [string]::IsNullOrWhiteSpace($Filter)) {
        throw 'Filter and IsolatedTestsFile cannot be combined.'
    }
    foreach ($artifactPath in @($isolatedTests.Path | Sort-Object -Unique)) {
        Push-Location $workspaceRoot
        try {
            $listedTests = @(& $artifactPath '--list' '--format' 'terse')
            $listExitCode = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        if ($listExitCode -ne 0) {
            throw "Failed to list tests from isolated-test executable: $artifactPath"
        }
        foreach ($isolatedTest in @($isolatedTests | Where-Object { $_.Path -ceq $artifactPath })) {
            $expectedListLine = "$($isolatedTest.Name): test"
            if (@($listedTests | Where-Object { [string]$_ -ceq $expectedListLine }).Count -ne 1) {
                throw "Isolated test selector does not resolve to exactly one test: $($isolatedTest.Name)"
            }
        }
    }
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
$runIsolatedTests = {
    foreach ($isolatedTest in $isolatedTests) {
        Write-Host "running_isolated_test_executable=$($isolatedTest.Path)"
        Write-Host "running_isolated_test=$($isolatedTest.Name)"
        $isolatedArguments = @($isolatedTest.Name, '--exact')
        if ($NoCapture) {
            $isolatedArguments += '--nocapture'
        }
        $isolatedArguments += '--test-threads=1'
        Push-Location $workspaceRoot
        try {
            & $isolatedTest.Path @isolatedArguments
            $testExitCode = $LASTEXITCODE
        } finally {
            Pop-Location
        }

        if ($testExitCode -ne 0) {
            $failedArtifacts += "$($isolatedTest.Path) :: $($isolatedTest.Name)"
            if (-not $NoFailFast) {
                throw "Isolated stable test failed with exit code ${testExitCode}: $($isolatedTest.Name)"
            }
        }
    }
}

Write-Host "isolated_test_order=$IsolatedTestOrder"
if ($IsolatedTestOrder -ceq 'BeforeAggregate') {
    . $runIsolatedTests
}

foreach ($artifact in $stableArtifacts) {
    Write-Host "running_test_executable=$($artifact.Path)"
    $artifactArguments = @($testArguments)
    foreach ($isolatedTest in @($isolatedTests | Where-Object { $_.Path -ceq $artifact.Path })) {
        $artifactArguments += @('--skip', $isolatedTest.Name)
    }
    Push-Location $workspaceRoot
    try {
        & $artifact.Path @artifactArguments
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

if ($IsolatedTestOrder -ceq 'AfterAggregate') {
    . $runIsolatedTests
}

if ($failedArtifacts.Count -ne 0) {
    throw "One or more stable test executables failed: $($failedArtifacts -join ', ')"
}

Write-Host 'status=passed'
Write-Host "isolated_test_count=$($isolatedTests.Count)"
& powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'cargo-cache-maintenance.ps1')
if ($LASTEXITCODE -ne 0) {
    throw 'Cargo cache status check failed.'
}
