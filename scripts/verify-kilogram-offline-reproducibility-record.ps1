[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RecordDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'kilogram-reproducible-linker.ps1')

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
if ($record.format_version -ne 5 -or $record.status -ne 'reproducible') {
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
    $record.blake3_codegen -ne 'pure-rust-intrinsics' -or
    $record.pe_metadata_normalization -ne 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1' -or
    $record.linker.mode -ne 'rust-toolchain-bundled-lld' -or
    $record.linker.source -ne 'rustc-sysroot-target-bin' -or
    $record.linker.file -ne 'rust-lld.exe' -or
    $record.linker.flavor -ne 'lld-link' -or
    [int64]$record.linker.bytes -le 0 -or
    $record.linker.reproducibility_flag -ne '/Brepro' -or
    $record.native_toolchain.mode -ne 'repository-hash-locked-installed-libraries' -or
    $record.native_toolchain.selection -ne 'explicit-final-rustc-native-search-paths' -or
    $record.native_toolchain.lock_file -ne 'WINDOWS-NATIVE-LINK-INPUTS.lock' -or
    [int]$record.native_toolchain.count -ne 10 -or
    $record.native_toolchain.msvc_version -ne '14.44.35207' -or
    $record.native_toolchain.windows_sdk_version -ne '10.0.19041.0' -or
    $record.native_toolchain.architecture -ne 'x64' -or
    $record.native_toolchain.libraries_bundled -ne $false -or
    $record.native_link_inputs.capture_mode -ne 'lld-link-reproduce-archive' -or
    $record.native_link_inputs.manifest_format -ne 'sha256-bytes-logical-path-v1' -or
    $record.native_link_inputs.file -ne 'NATIVE-LINK-INPUTS.sha256' -or
    [int]$record.native_link_inputs.count -le 0 -or
    [int]$record.native_link_inputs.count -gt 256 -or
    $record.native_link_inputs.archive_retained -ne $false -or
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
    @{ Value = [string]$record.linker.sha256; Name = 'linker.sha256' },
    @{ Value = [string]$record.native_toolchain.lock_sha256; Name = 'native_toolchain.lock_sha256' },
    @{ Value = [string]$record.native_link_inputs.sha256; Name = 'native_link_inputs.sha256' },
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
$nativeToolchainLockPath = Resolve-RecordFile 'WINDOWS-NATIVE-LINK-INPUTS.lock'
$nativeLinkManifestPath = Resolve-RecordFile 'NATIVE-LINK-INPUTS.sha256'
if ((Get-Sha256 $manifestPath) -ne $record.source_manifest_sha256 -or
    (Get-Sha256 $lockPath) -ne $record.cargo_lock_sha256 -or
    (Get-Sha256 $toolchainPath) -ne $record.rust_toolchain_sha256 -or
    (Get-Sha256 $nativeLinkManifestPath) -ne $record.native_link_inputs.sha256) {
    throw 'Reproducibility record input hash mismatch.'
}
$nativeLinkInputs = Assert-KilogramNativeLinkInputManifest -Path $nativeLinkManifestPath
$nativeToolchainLock = Assert-KilogramNativeLinkInputManifest -Path $nativeToolchainLockPath
$canonicalNativeToolchainLockHash = Get-KilogramCanonicalNativeLinkInputSha256 `
    -Lines @(Get-Content -LiteralPath $nativeToolchainLockPath)
if ([string]$nativeLinkInputs.file -cne [string]$record.native_link_inputs.file -or
    [string]$nativeLinkInputs.sha256 -cne [string]$record.native_link_inputs.sha256 -or
    [int]$nativeLinkInputs.count -ne [int]$record.native_link_inputs.count) {
    throw 'Reproducibility record native link-input manifest mismatch.'
}
if ([int]$nativeToolchainLock.count -ne [int]$record.native_toolchain.count -or
    [string]$canonicalNativeToolchainLockHash -cne [string]$record.native_toolchain.lock_sha256) {
    throw 'Reproducibility record pinned native-toolchain lock mismatch.'
}
$null = Assert-KilogramNativeLinkInputManifestMatchesLock `
    -ManifestPath $nativeLinkManifestPath `
    -LockPath $nativeToolchainLockPath

$buildAPath = Resolve-RecordFile $record.build_a.file
$buildBPath = Resolve-RecordFile $record.build_b.file
$buildAHash = Get-Sha256 $buildAPath
$buildBHash = Get-Sha256 $buildBPath
Assert-KilogramPeReproducibilityMetadataNormalized -Path $buildAPath
Assert-KilogramPeReproducibilityMetadataNormalized -Path $buildBPath
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
Write-Output "blake3_codegen=$($record.blake3_codegen)"
Write-Output "linker_sha256=$($record.linker.sha256)"
Write-Output "native_toolchain_lock_sha256=$($record.native_toolchain.lock_sha256)"
Write-Output "native_toolchain_msvc=$($record.native_toolchain.msvc_version)"
Write-Output "native_toolchain_windows_sdk=$($record.native_toolchain.windows_sdk_version)"
Write-Output "native_link_inputs_sha256=$($record.native_link_inputs.sha256)"
Write-Output "native_link_inputs_count=$($record.native_link_inputs.count)"
Write-Output "pe_metadata_normalization=$($record.pe_metadata_normalization)"
Write-Output 'network_surface_compiled=false'
Write-Output 'runtime_surface_compiled=false'
