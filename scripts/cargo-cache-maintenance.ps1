[CmdletBinding()]
param(
    [switch]$VacuumAndWarm,
    [switch]$PruneVerification,
    [int]$CargoJobs = 0,
    [double]$WarningGiB = 40
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs

if ($VacuumAndWarm -and $PruneVerification) {
    throw 'Choose either -VacuumAndWarm or -PruneVerification, not both.'
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
    $measure = Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum
    $bytes = if ($null -eq $measure.Sum) { [int64]0 } else { [int64]$measure.Sum }
    [PSCustomObject]@{ Files = [int64]$measure.Count; Bytes = $bytes }
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
        Write-Warning "Cargo target exceeds $WarningGiB GiB. Keep the fast dev/release cache unless space is needed; use -PruneVerification for the disposable test profile or -VacuumAndWarm for an explicit full reset."
    }
    Write-Output "vacuum_and_warm=$($VacuumAndWarm.IsPresent)".ToLowerInvariant()
    Write-Output "verification_pruned=$($PruneVerification.IsPresent)".ToLowerInvariant()
}
finally {
    Pop-Location
}
