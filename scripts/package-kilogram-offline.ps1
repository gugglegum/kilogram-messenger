param(
    [string]$OutputDirectory = '.tmp\dist\kilogram-offline-windows-x86_64',
    [switch]$AllowDirty
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
$output = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
}
$distRoot = [System.IO.Path]::GetFullPath((Join-Path $workspace '.tmp\dist'))
if (-not $output.StartsWith($distRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
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

    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-boundary.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw 'offline dependency boundary verification failed'
    }
    & cargo build --locked --release -p kilogram-offline
    if ($LASTEXITCODE -ne 0) {
        throw 'kilogram-offline release build failed'
    }

    New-Item -ItemType Directory -Path $output | Out-Null
    $binary = Join-Path $output 'kilogram-offline.exe'
    $readme = Join-Path $output 'README.txt'
    Copy-Item -LiteralPath (Join-Path $workspace 'target\release\kilogram-offline.exe') -Destination $binary
    Copy-Item -LiteralPath (Join-Path $workspace 'apps\kilogram-offline\PACKAGE-README.txt') -Destination $readme

    $revision = (& git rev-parse HEAD).Trim()
    $dirtySuffix = if ($dirty) { '+dirty' } else { '' }
    $rustcVersion = (& rustc --version).Trim()
    $cargoVersion = (& cargo --version).Trim()
    $buildInfo = @(
        'format_version=1',
        "source_revision=$revision$dirtySuffix",
        "rustc=$rustcVersion",
        "cargo=$cargoVersion",
        'target=x86_64-pc-windows-msvc',
        'network_surface_compiled=false',
        'runtime_surface_compiled=false'
    )
    [System.IO.File]::WriteAllLines((Join-Path $output 'BUILD-INFO.txt'), $buildInfo, [System.Text.UTF8Encoding]::new($false))

    $payloadNames = @('kilogram-offline.exe', 'README.txt', 'BUILD-INFO.txt')
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
        foreach ($name in @('kilogram-offline.exe', 'README.txt', 'BUILD-INFO.txt', 'SHA256SUMS')) {
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
