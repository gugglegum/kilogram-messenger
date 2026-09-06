[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RecordDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$directory = [System.IO.Path]::GetFullPath($RecordDirectory)
if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
    throw "Reproducibility record directory does not exist: $directory"
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-Sha256([string]$Value, [string]$Field) {
    if ($Value -cnotmatch '^[0-9a-f]{64}$') {
        throw "Reproducibility record field is not a lowercase SHA-256: $Field"
    }
}

function Resolve-RecordFile([string]$Name) {
    if ([string]::IsNullOrWhiteSpace($Name) -or
        [System.IO.Path]::IsPathRooted($Name) -or
        $Name.Contains('..') -or
        $Name.Contains('/') -or
        $Name.Contains('\')) {
        throw "Reproducibility record contains an unsafe file name: $Name"
    }
    $path = [System.IO.Path]::GetFullPath((Join-Path $directory $Name))
    if (-not ([System.IO.Directory]::GetParent($path).FullName.Equals($directory, [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "Reproducibility record file escapes its directory: $Name"
    }
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Reproducibility record file is missing: $path"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Reproducibility record file must not be a reparse point: $path"
    }
    $path
}

$recordPath = Resolve-RecordFile 'REPRODUCIBILITY.json'
$recordItem = Get-Item -LiteralPath $recordPath
if ($recordItem.Length -le 0 -or $recordItem.Length -gt 64KB) {
    throw 'Reproducibility record size is invalid.'
}
$record = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
if ($record.format_version -ne 1 -or $record.status -ne 'reproducible') {
    throw 'Reproducibility record version or status is invalid.'
}
if ($record.builder_scope -ne 'same-host-separate-clean-roots' -or
    $record.build_root_count -ne 2 -or
    $record.dependency_mode -ne 'cargo-frozen' -or
    $record.artifact_comparison -ne 'sha256-and-length' -or
    [int]$record.cargo_jobs -le 0 -or
    [int]$record.cargo_jobs -gt 256 -or
    $record.target -ne 'x86_64-pc-windows-msvc' -or
    $record.incremental -ne $false -or
    $record.path_remap -ne '<BUILD_ROOT>=Z:/kilogram-source' -or
    $record.linker_reproducibility_flag -ne '/Brepro' -or
    $record.network_surface_compiled -ne $false -or
    $record.runtime_surface_compiled -ne $false) {
    throw 'Reproducibility record build boundary is invalid.'
}
if ([string]$record.source_revision -notmatch '^[0-9a-f]{40}(\+dirty)?$' -or
    [int64]$record.source_epoch -le 0 -or
    [string]::IsNullOrWhiteSpace([string]$record.rustc) -or
    [string]::IsNullOrWhiteSpace([string]$record.cargo)) {
    throw 'Reproducibility record source or toolchain identity is invalid.'
}
foreach ($hashField in @(
    @{ Value = [string]$record.source_manifest_sha256; Name = 'source_manifest_sha256' },
    @{ Value = [string]$record.cargo_lock_sha256; Name = 'cargo_lock_sha256' },
    @{ Value = [string]$record.rust_toolchain_sha256; Name = 'rust_toolchain_sha256' },
    @{ Value = [string]$record.build_a.sha256; Name = 'build_a.sha256' },
    @{ Value = [string]$record.build_b.sha256; Name = 'build_b.sha256' }
)) {
    Assert-Sha256 $hashField.Value $hashField.Name
}
if ($record.build_a.file -ne 'kilogram-offline-build-a.exe' -or
    $record.build_b.file -ne 'kilogram-offline-build-b.exe') {
    throw 'Reproducibility record artifact names are invalid.'
}

$manifestPath = Resolve-RecordFile 'SOURCE-MANIFEST.sha256'
$lockPath = Resolve-RecordFile 'Cargo.lock'
$toolchainPath = Resolve-RecordFile 'rust-toolchain.toml'
if ((Get-Sha256 $manifestPath) -ne $record.source_manifest_sha256 -or
    (Get-Sha256 $lockPath) -ne $record.cargo_lock_sha256 -or
    (Get-Sha256 $toolchainPath) -ne $record.rust_toolchain_sha256) {
    throw 'Reproducibility record input hash mismatch.'
}

$buildAPath = Resolve-RecordFile $record.build_a.file
$buildBPath = Resolve-RecordFile $record.build_b.file
$buildAHash = Get-Sha256 $buildAPath
$buildBHash = Get-Sha256 $buildBPath
$buildALength = (Get-Item -LiteralPath $buildAPath).Length
$buildBLength = (Get-Item -LiteralPath $buildBPath).Length
if ($buildAHash -ne $record.build_a.sha256 -or
    $buildBHash -ne $record.build_b.sha256 -or
    $buildALength -ne $record.build_a.bytes -or
    $buildBLength -ne $record.build_b.bytes -or
    $buildAHash -ne $buildBHash -or
    $buildALength -ne $buildBLength) {
    throw 'The two recorded offline builds are not byte-identical.'
}

Write-Output 'reproducibility_record=verified'
Write-Output "source_revision=$($record.source_revision)"
Write-Output "source_manifest_sha256=$($record.source_manifest_sha256)"
Write-Output "artifact_sha256=$buildAHash"
Write-Output "artifact_bytes=$buildALength"
Write-Output "target=$($record.target)"
Write-Output 'network_surface_compiled=false'
Write-Output 'runtime_surface_compiled=false'
