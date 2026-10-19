[CmdletBinding()]
param(
    [switch]$SelfTest,
    [switch]$VacuumAndWarm,
    [switch]$PruneVerification,
    [switch]$PruneDebugIncremental,
    [int]$CargoJobs = 0,
    [double]$WarningGiB = 40
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs

if ($VacuumAndWarm -and ($PruneVerification -or $PruneDebugIncremental)) {
    throw 'VacuumAndWarm cannot be combined with a targeted prune.'
}
if ($WarningGiB -le 0) {
    throw 'WarningGiB must be positive.'
}

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$target = [System.IO.Path]::GetFullPath((Join-Path $workspace 'target'))
$expectedTarget = [System.IO.Path]::GetFullPath((Join-Path $workspace 'target'))
if (-not $target.Equals($expectedTarget, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not ([System.IO.Directory]::GetParent($target).FullName.Equals($workspace, [System.StringComparison]::OrdinalIgnoreCase))) {
    throw "Refusing unexpected Cargo target path: $target"
}

function Get-TreeMeasure([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return [PSCustomObject]@{ Files = 0; Bytes = [int64]0 }
    }
    $measure = @(
        Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum
    )
    if ($measure.Count -eq 0) {
        return [PSCustomObject]@{ Files = 0; Bytes = [int64]0 }
    }
    if ($measure.Count -ne 1) {
        throw "Unexpected tree measurement result count for '$Path': $($measure.Count)"
    }
    $summary = $measure[0]
    $sumProperty = $summary.PSObject.Properties['Sum']
    $bytes = if ($null -eq $sumProperty -or $null -eq $sumProperty.Value) {
        [int64]0
    }
    else {
        [int64]$sumProperty.Value
    }
    [PSCustomObject]@{ Files = [int64]$summary.Count; Bytes = $bytes }
}

function Remove-VerifiedDirectory([string]$Path, [string]$ExpectedName) {
    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not ([System.IO.Directory]::GetParent($resolved).FullName.Equals($target, [System.StringComparison]::OrdinalIgnoreCase)) -or
        -not ([System.IO.Path]::GetFileName($resolved).Equals($ExpectedName, [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "Refusing unexpected cache path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

function Remove-VerifiedDebugIncrementalDirectory {
    $expectedParent = [System.IO.Path]::GetFullPath((Join-Path $target 'debug'))
    $resolved = [System.IO.Path]::GetFullPath((Join-Path $expectedParent 'incremental'))
    $parent = [System.IO.Directory]::GetParent($resolved)
    if ($null -eq $parent -or
        -not $parent.FullName.Equals($expectedParent, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not ([System.IO.Path]::GetFileName($resolved).Equals('incremental', [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "Refusing unexpected debug incremental cache path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

if ($SelfTest) {
    if ($VacuumAndWarm -or $PruneVerification -or $PruneDebugIncremental) {
        throw 'SelfTest cannot be combined with cache mutation switches.'
    }
    $selfTestRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        'kilogram-cargo-cache-self-test-' + [Guid]::NewGuid().ToString('N')
    )
    $missingPath = Join-Path $selfTestRoot 'missing'
    $emptyPath = Join-Path $selfTestRoot 'empty'
    try {
        $missing = Get-TreeMeasure $missingPath
        if ($missing.Files -ne 0 -or $missing.Bytes -ne 0) {
            throw 'Missing-tree measurement was not zero.'
        }

        [void](New-Item -ItemType Directory -Path $emptyPath -Force)
        $empty = Get-TreeMeasure $emptyPath
        if ($empty.Files -ne 0 -or $empty.Bytes -ne 0) {
            throw 'Empty-tree measurement was not zero.'
        }

        [System.IO.File]::WriteAllBytes((Join-Path $emptyPath 'three-bytes.bin'), [byte[]](1, 2, 3))
        $nonEmpty = Get-TreeMeasure $emptyPath
        if ($nonEmpty.Files -ne 1 -or $nonEmpty.Bytes -ne 3) {
            throw 'Non-empty tree measurement did not preserve exact file and byte counts.'
        }
    }
    finally {
        if ([System.IO.Directory]::Exists($selfTestRoot)) {
            [System.IO.Directory]::Delete($selfTestRoot, $true)
        }
    }
    Write-Output 'cargo_cache_measurement_self_test=passed'
    Write-Output 'missing_tree=zero'
    Write-Output 'empty_tree=zero'
    Write-Output 'non_empty_tree=exact'
    exit 0
}

Push-Location $workspace
try {
    if ($VacuumAndWarm) {
        if (Test-Path -LiteralPath $target) {
            Remove-Item -LiteralPath $target -Recurse -Force
        }
        & cargo check --jobs $cargoJobsResolved --workspace --all-targets --locked
        if ($LASTEXITCODE -ne 0) {
            throw 'cargo check failed while warming the development cache'
        }
        & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -CargoJobs $cargoJobsResolved
        if ($LASTEXITCODE -ne 0) {
            throw 'stable test-harness compilation failed while warming the verification cache'
        }
        & cargo build --jobs $cargoJobsResolved --workspace --release --locked
        if ($LASTEXITCODE -ne 0) {
            throw 'release build failed while warming the release cache'
        }
    }
    elseif ($PruneVerification) {
        Remove-VerifiedDirectory (Join-Path $target 'verification') 'verification'
    }
    if ($PruneDebugIncremental) {
        Remove-VerifiedDebugIncrementalDirectory
    }

    $rows = foreach ($entry in @(
        @{ Name = 'debug-incremental'; Path = (Join-Path $target 'debug\incremental') },
        @{ Name = 'debug-total'; Path = (Join-Path $target 'debug') },
        @{ Name = 'verification'; Path = (Join-Path $target 'verification') },
        @{ Name = 'release'; Path = (Join-Path $target 'release') }
    )) {
        $measure = Get-TreeMeasure $entry.Path
        [PSCustomObject]@{
            Name = $entry.Name
            Files = $measure.Files
            GiB = [math]::Round($measure.Bytes / 1GB, 2)
        }
    }
    $total = Get-TreeMeasure $target
    $rows | Format-Table -AutoSize | Out-Host
    Write-Output "target_path=$target"
    Write-Output "target_files=$($total.Files)"
    Write-Output "target_gib=$([math]::Round($total.Bytes / 1GB, 2))"
    Write-Output "warning_gib=$WarningGiB"
    $aboveWarning = $total.Bytes -gt ($WarningGiB * 1GB)
    Write-Output "cache_above_warning=$aboveWarning".ToLowerInvariant()
    if ($aboveWarning) {
        Write-Warning "Cargo target exceeds $WarningGiB GiB. Use -PruneDebugIncremental to remove only reproducible incremental generations, -PruneVerification for the disposable test profile, or -VacuumAndWarm for an explicit full reset."
    }
    Write-Output "vacuum_and_warm=$($VacuumAndWarm.IsPresent)".ToLowerInvariant()
    Write-Output "verification_pruned=$($PruneVerification.IsPresent)".ToLowerInvariant()
    Write-Output "debug_incremental_pruned=$($PruneDebugIncremental.IsPresent)".ToLowerInvariant()
}
finally {
    Pop-Location
}
