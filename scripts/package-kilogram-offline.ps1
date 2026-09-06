param(
    [string]$OutputDirectory = '.tmp\dist\kilogram-offline-windows-x86_64',
    [string]$ReproducibilityRecordDirectory,
    [int]$CargoJobs = 0,
    [switch]$AllowDirty
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs

$workspace = Split-Path -Parent $PSScriptRoot
$output = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
}
$distRoot = [System.IO.Path]::GetFullPath((Join-Path $workspace '.tmp\dist'))
$distPrefix = $distRoot.TrimEnd('\') + '\'
if (-not $output.StartsWith($distPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDirectory must remain below $distRoot"
}
if (Test-Path -LiteralPath $output) {
    throw "OutputDirectory already exists: $output"
}

Push-Location $workspace
try {
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) {
        throw 'git status failed'
    }
    if ($dirty -and -not $AllowDirty) {
        throw 'Refusing a release package from a dirty worktree; commit first or use -AllowDirty for a development-only package'
    }

    $revision = (& git rev-parse HEAD).Trim()
    $reproducibilityVerified = $false
    $reproducibilityRecord = $null
    $sourceBinary = $null
    if (-not [string]::IsNullOrWhiteSpace($ReproducibilityRecordDirectory)) {
        $recordDirectory = if ([System.IO.Path]::IsPathRooted($ReproducibilityRecordDirectory)) {
            [System.IO.Path]::GetFullPath($ReproducibilityRecordDirectory)
        }
        else {
            [System.IO.Path]::GetFullPath((Join-Path $workspace $ReproducibilityRecordDirectory))
        }
        & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-reproducibility-record.ps1') -RecordDirectory $recordDirectory
        if ($LASTEXITCODE -ne 0) {
            throw 'offline reproducibility record verification failed'
        }
        $recordPath = Join-Path $recordDirectory 'REPRODUCIBILITY.json'
        $reproducibilityRecord = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
        if ($dirty) {
            throw 'A reproducibility-backed package requires a clean worktree.'
        }
        if ($reproducibilityRecord.source_revision -ne $revision) {
            throw "Reproducibility record source revision does not match HEAD: $($reproducibilityRecord.source_revision) != $revision"
        }
        $artifactName = [string]$reproducibilityRecord.build_a.file
        if ([System.IO.Path]::IsPathRooted($artifactName) -or
            $artifactName.Contains('..') -or
            $artifactName.Contains('/') -or
            $artifactName.Contains('\')) {
            throw 'Reproducibility record contains an unsafe artifact name.'
        }
        $sourceBinary = Join-Path $recordDirectory $artifactName
        $reproducibilityVerified = $true
    }
    elseif (-not $AllowDirty) {
        throw 'A clean release package requires -ReproducibilityRecordDirectory. Use -AllowDirty only for an explicitly unverified development package.'
    }

    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-publication-conflict-boundary.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw 'publication-conflict dependency boundary verification failed'
    }
    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-boundary.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw 'offline dependency boundary verification failed'
    }
    if (-not $reproducibilityVerified) {
        & cargo build --jobs $cargoJobsResolved --locked --release -p kilogram-offline
        if ($LASTEXITCODE -ne 0) {
            throw 'kilogram-offline release build failed'
        }
        $sourceBinary = Join-Path $workspace 'target\release\kilogram-offline.exe'
    }

    New-Item -ItemType Directory -Path $output | Out-Null
    $binary = Join-Path $output 'kilogram-offline.exe'
    $readme = Join-Path $output 'README.txt'
    Copy-Item -LiteralPath $sourceBinary -Destination $binary
    Copy-Item -LiteralPath (Join-Path $workspace 'apps\kilogram-offline\PACKAGE-README.txt') -Destination $readme

    $dirtySuffix = if ($dirty) { '+dirty' } else { '' }
    $rustcVersion = (& rustc --version).Trim()
    $cargoVersion = (& cargo --version).Trim()
    $buildInfo = @(
        'format_version=2',
        "source_revision=$revision$dirtySuffix",
        "rustc=$rustcVersion",
        "cargo=$cargoVersion",
        "cargo_jobs=$cargoJobsResolved",
        'target=x86_64-pc-windows-msvc',
        'network_surface_compiled=false',
        'runtime_surface_compiled=false',
        "reproducibility_verified=$($reproducibilityVerified.ToString().ToLowerInvariant())"
    )
    if ($reproducibilityVerified) {
        $recordDestination = Join-Path $output 'REPRODUCIBILITY.json'
        $manifestDestination = Join-Path $output 'SOURCE-MANIFEST.sha256'
        $lockDestination = Join-Path $output 'Cargo.lock'
        $toolchainDestination = Join-Path $output 'rust-toolchain.toml'
        Copy-Item -LiteralPath (Join-Path $recordDirectory 'REPRODUCIBILITY.json') -Destination $recordDestination
        Copy-Item -LiteralPath (Join-Path $recordDirectory 'SOURCE-MANIFEST.sha256') -Destination $manifestDestination
        Copy-Item -LiteralPath (Join-Path $recordDirectory 'Cargo.lock') -Destination $lockDestination
        Copy-Item -LiteralPath (Join-Path $recordDirectory 'rust-toolchain.toml') -Destination $toolchainDestination
        $buildInfo += "reproducibility_record_sha256=$((Get-FileHash -Algorithm SHA256 -LiteralPath $recordDestination).Hash.ToLowerInvariant())"
        $buildInfo += "source_manifest_sha256=$($reproducibilityRecord.source_manifest_sha256)"
        $buildInfo += "reproduced_artifact_sha256=$($reproducibilityRecord.build_a.sha256)"
    }
    [System.IO.File]::WriteAllLines((Join-Path $output 'BUILD-INFO.txt'), $buildInfo, [System.Text.UTF8Encoding]::new($false))

    $payloadNames = @('kilogram-offline.exe', 'README.txt', 'BUILD-INFO.txt')
    if ($reproducibilityVerified) {
        $payloadNames += @('REPRODUCIBILITY.json', 'SOURCE-MANIFEST.sha256', 'Cargo.lock', 'rust-toolchain.toml')
    }
    $checksumLines = foreach ($name in $payloadNames) {
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output $name)).Hash.ToLowerInvariant()
        "$hash  $name"
    }
    [System.IO.File]::WriteAllLines((Join-Path $output 'SHA256SUMS'), $checksumLines, [System.Text.UTF8Encoding]::new($false))

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zipPath = "$output.zip"
    if (Test-Path -LiteralPath $zipPath) {
        throw "Package archive already exists: $zipPath"
    }
    $archive = [System.IO.Compression.ZipFile]::Open($zipPath, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        $fixedTime = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
        foreach ($name in @($payloadNames + 'SHA256SUMS')) {
            $entry = $archive.CreateEntry($name, [System.IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = $fixedTime
            $sourceStream = [System.IO.File]::OpenRead((Join-Path $output $name))
            $targetStream = $entry.Open()
            try {
                $sourceStream.CopyTo($targetStream)
            }
            finally {
                $targetStream.Dispose()
                $sourceStream.Dispose()
            }
        }
    }
    finally {
        $archive.Dispose()
    }

    $archiveHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLowerInvariant()
    Write-Output "package_directory=$output"
    Write-Output "package_archive=$zipPath"
    Write-Output "package_sha256=$archiveHash"
    Write-Output "source_revision=$revision$dirtySuffix"
    Write-Output 'status=kilogram-offline-packaged'
}
finally {
    Pop-Location
}
