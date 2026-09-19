[CmdletBinding()]
param(
    [string]$OutputDirectory = '.tmp\repro\kilogram-offline',
    [int]$CargoJobs = 0,
    [switch]$AllowDirty,
    [switch]$KeepBuildRoots
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
. (Join-Path $PSScriptRoot 'kilogram-reproducible-linker.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$linkerIdentity = $null
$nativeToolchainIdentity = $null
$cargoRegistrySourceRoot = $null
$nativeToolchainLockPath = Join-Path $workspace 'WINDOWS-NATIVE-LINK-INPUTS.lock'
Push-Location $workspace
try {
    $linkerIdentity = Get-KilogramBundledLldIdentity
    $nativeToolchainIdentity = Get-KilogramPinnedNativeToolchainIdentity `
        -LockPath $nativeToolchainLockPath
    $cargoRegistrySourceRoot = Get-KilogramCargoRegistrySourceRoot
}
finally {
    Pop-Location
}
$reproRoot = [System.IO.Path]::GetFullPath((Join-Path $workspace '.tmp\repro'))
$output = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
}
$reproPrefix = $reproRoot.TrimEnd('\') + '\'
if (-not $output.StartsWith($reproPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDirectory must remain below $reproRoot"
}
if (Test-Path -LiteralPath $output) {
    throw "OutputDirectory already exists: $output"
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Write-Utf8Lines([string]$Path, [string[]]$Lines) {
    [System.IO.File]::WriteAllLines($Path, $Lines, [System.Text.UTF8Encoding]::new($false))
}

function Assert-DisposablePath([string]$Path, [string]$ExpectedName) {
    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not $resolved.StartsWith($reproPrefix, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not ([System.IO.Path]::GetFileName($resolved).Equals($ExpectedName, [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "Refusing unexpected reproducibility work path: $resolved"
    }
}

function Copy-SourceTree([string]$Destination, [string[]]$RelativeFiles) {
    New-Item -ItemType Directory -Path $Destination | Out-Null
    foreach ($relativeFile in $RelativeFiles) {
        if ([string]::IsNullOrWhiteSpace($relativeFile) -or
            [System.IO.Path]::IsPathRooted($relativeFile) -or
            $relativeFile -match '(^|[\\/])\.\.([\\/]|$)') {
            throw "Git returned an unsafe source path: $relativeFile"
        }
        $source = [System.IO.Path]::GetFullPath((Join-Path $workspace $relativeFile))
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            continue
        }
        $destinationFile = [System.IO.Path]::GetFullPath((Join-Path $Destination $relativeFile))
        $destinationParent = Split-Path -Parent $destinationFile
        New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $destinationFile
    }
}

function New-SourceManifest([string]$SourceRoot, [string[]]$RelativeFiles, [string]$ManifestPath) {
    $lines = foreach ($relativeFile in $RelativeFiles) {
        $path = [System.IO.Path]::GetFullPath((Join-Path $SourceRoot $relativeFile))
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            "$(Get-Sha256 $path)  $($relativeFile.Replace('\', '/'))"
        }
    }
    Write-Utf8Lines $ManifestPath @($lines | Sort-Object)
}

function Invoke-CleanBuild(
    [string]$SourceRoot,
    [string]$TargetRoot,
    [string]$ArtifactName,
    [string]$NativeManifestName,
    [string]$LinkReproName
) {
    $oldTarget = $env:CARGO_TARGET_DIR
    $oldIncremental = $env:CARGO_INCREMENTAL
    $oldEpoch = $env:SOURCE_DATE_EPOCH
    $oldEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
    $linkRepro = [System.IO.Path]::GetFullPath((Join-Path $output $LinkReproName))
    $linkReproArgument = $linkRepro.Replace('\', '/')
    $nativeManifest = [System.IO.Path]::GetFullPath((Join-Path $output $NativeManifestName))
    try {
        try {
            $env:CARGO_TARGET_DIR = $TargetRoot
            $env:CARGO_INCREMENTAL = '0'
            $env:SOURCE_DATE_EPOCH = $sourceEpoch
            $unitSeparator = [char]0x1f
            $env:CARGO_ENCODED_RUSTFLAGS = @(
                New-KilogramReproducibleRustFlags `
                    -SourceRoot $SourceRoot `
                    -CargoRegistrySourceRoot $cargoRegistrySourceRoot `
                    -LinkerIdentity $linkerIdentity
            ) -join $unitSeparator
            Push-Location $SourceRoot
            try {
                $cargoArguments = @(
                    'rustc',
                    '--jobs',
                    [string]$cargoJobsResolved,
                    '--frozen',
                    '--release',
                    '--target',
                    'x86_64-pc-windows-msvc',
                    '--package',
                    'kilogram-offline',
                    '--bin',
                    'kilogram-offline',
                    '--'
                ) + @($nativeToolchainIdentity.rustc_arguments) + @(
                    '-C',
                    "link-arg=/reproduce:$linkReproArgument"
                )
                & cargo @cargoArguments
                if ($LASTEXITCODE -ne 0) {
                    throw "offline build failed in $SourceRoot"
                }
            }
            finally {
                Pop-Location
            }
        }
        finally {
            $env:CARGO_TARGET_DIR = $oldTarget
            $env:CARGO_INCREMENTAL = $oldIncremental
            $env:SOURCE_DATE_EPOCH = $oldEpoch
            $env:CARGO_ENCODED_RUSTFLAGS = $oldEncodedFlags
        }

        $built = Join-Path $TargetRoot 'x86_64-pc-windows-msvc\release\kilogram-offline.exe'
        if (-not (Test-Path -LiteralPath $built -PathType Leaf)) {
            throw "offline build artifact is missing: $built"
        }
        $nativeLinkInputs = Write-KilogramNativeLinkInputManifest `
            -ArchivePath $linkRepro `
            -ManifestPath $nativeManifest
        $nativeLinkInputs = Assert-KilogramNativeLinkInputManifestMatchesLock `
            -ManifestPath $nativeManifest `
            -LockPath (Join-Path $SourceRoot $nativeToolchainIdentity.lock_file)
        $artifact = Join-Path $output $ArtifactName
        Copy-Item -LiteralPath $built -Destination $artifact
        Normalize-KilogramPeReproducibilityMetadata -Path $artifact
        $pathRemap = Assert-KilogramPeCanonicalPathRemapping `
            -Path $artifact `
            -HostSourceRoot $SourceRoot `
            -CargoRegistrySourceRoot $cargoRegistrySourceRoot
        [PSCustomObject]@{
            file = $ArtifactName
            sha256 = Get-Sha256 $artifact
            bytes = (Get-Item -LiteralPath $artifact).Length
            native_link_inputs = $nativeLinkInputs
            path_remap = $pathRemap
        }
    }
    finally {
        if (Test-Path -LiteralPath $linkRepro -PathType Leaf) {
            $resolvedLinkRepro = [System.IO.Path]::GetFullPath($linkRepro)
            if (-not $resolvedLinkRepro.StartsWith(
                $output.TrimEnd('\') + '\',
                [System.StringComparison]::OrdinalIgnoreCase
            ) -or
                -not ([System.IO.Path]::GetFileName($resolvedLinkRepro).Equals(
                    $LinkReproName,
                    [System.StringComparison]::Ordinal
                ))) {
                throw "Refusing unexpected LLD link-reproduction cleanup path: $resolvedLinkRepro"
            }
            Remove-Item -LiteralPath $resolvedLinkRepro -Force
        }
    }
}

Push-Location $workspace
try {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-boundary.ps1') | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw 'offline dependency/code-generation boundary verification failed'
    }
    $dirty = @(& git status --porcelain --untracked-files=normal)
    if ($LASTEXITCODE -ne 0) {
        throw 'git status failed'
    }
    if ($dirty.Count -ne 0 -and -not $AllowDirty) {
        throw 'Refusing a reproducibility record from a dirty worktree; commit first or use -AllowDirty for development verification'
    }
    $revision = (& git rev-parse HEAD).Trim()
    $sourceEpoch = (& git show -s --format=%ct HEAD).Trim()
    $sourceFiles = @(& git ls-files --cached --others --exclude-standard | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0 -or $sourceFiles.Count -eq 0) {
        throw 'resolve Git source file list failed'
    }

    New-Item -ItemType Directory -Path $output -Force | Out-Null
    $sourceA = Join-Path $output 'source-a'
    $sourceB = Join-Path $output 'source-b'
    $targetA = Join-Path $output 'target-a'
    $targetB = Join-Path $output 'target-b'
    foreach ($entry in @(
        @{ Path = $sourceA; Name = 'source-a' },
        @{ Path = $sourceB; Name = 'source-b' },
        @{ Path = $targetA; Name = 'target-a' },
        @{ Path = $targetB; Name = 'target-b' }
    )) {
        Assert-DisposablePath $entry.Path $entry.Name
    }

    Copy-SourceTree $sourceA $sourceFiles
    Copy-SourceTree $sourceB $sourceFiles
    $manifestA = Join-Path $output 'SOURCE-MANIFEST-A.sha256'
    $manifestB = Join-Path $output 'SOURCE-MANIFEST-B.sha256'
    New-SourceManifest $sourceA $sourceFiles $manifestA
    New-SourceManifest $sourceB $sourceFiles $manifestB
    if ((Get-Sha256 $manifestA) -ne (Get-Sha256 $manifestB)) {
        throw 'The two clean source roots are not byte-identical.'
    }
    Move-Item -LiteralPath $manifestA -Destination (Join-Path $output 'SOURCE-MANIFEST.sha256')
    Remove-Item -LiteralPath $manifestB

    Copy-Item -LiteralPath (Join-Path $sourceA 'Cargo.lock') -Destination (Join-Path $output 'Cargo.lock')
    Copy-Item -LiteralPath (Join-Path $sourceA 'rust-toolchain.toml') -Destination (Join-Path $output 'rust-toolchain.toml')
    Copy-Item `
        -LiteralPath (Join-Path $sourceA $nativeToolchainIdentity.lock_file) `
        -Destination (Join-Path $output $nativeToolchainIdentity.lock_file)

    $buildA = Invoke-CleanBuild `
        $sourceA `
        $targetA `
        'kilogram-offline-build-a.exe' `
        'NATIVE-LINK-INPUTS-A.sha256' `
        'link-repro-a.tar'
    $buildB = Invoke-CleanBuild `
        $sourceB `
        $targetB `
        'kilogram-offline-build-b.exe' `
        'NATIVE-LINK-INPUTS-B.sha256' `
        'link-repro-b.tar'
    $linkerAfterBuild = Get-KilogramBundledLldIdentity
    if ([string]$linkerAfterBuild.sha256 -cne [string]$linkerIdentity.sha256 -or
        [int64]$linkerAfterBuild.bytes -ne [int64]$linkerIdentity.bytes -or
        -not ([string]$linkerAfterBuild.path).Equals(
            [string]$linkerIdentity.path,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'The bundled LLD linker changed during the two clean builds.'
    }
    if ([string]$buildA.native_link_inputs.sha256 -cne [string]$buildB.native_link_inputs.sha256 -or
        [int]$buildA.native_link_inputs.count -ne [int]$buildB.native_link_inputs.count) {
        throw 'The two clean builds did not consume identical native link inputs.'
    }
    $nativeManifestA = Join-Path $output 'NATIVE-LINK-INPUTS-A.sha256'
    $nativeManifestB = Join-Path $output 'NATIVE-LINK-INPUTS-B.sha256'
    $nativeManifest = Join-Path $output 'NATIVE-LINK-INPUTS.sha256'
    Move-Item -LiteralPath $nativeManifestA -Destination $nativeManifest
    Remove-Item -LiteralPath $nativeManifestB
    $nativeLinkInputs = Assert-KilogramNativeLinkInputManifest -Path $nativeManifest
    $equal = $buildA.sha256 -eq $buildB.sha256 -and $buildA.bytes -eq $buildB.bytes
    $buildARecord = [ordered]@{
        file = $buildA.file
        sha256 = $buildA.sha256
        bytes = $buildA.bytes
    }
    $buildBRecord = [ordered]@{
        file = $buildB.file
        sha256 = $buildB.sha256
        bytes = $buildB.bytes
    }

    Push-Location $sourceA
    try {
        $rustcVersion = ((& rustc -vV) -join "`n")
        $cargoVersion = ((& cargo -vV) -join "`n")
    }
    finally {
        Pop-Location
    }
    $record = [ordered]@{
        format_version = 6
        status = if ($equal) { 'reproducible' } else { 'divergent' }
        builder_scope = 'same-host-separate-clean-roots'
        build_root_count = 2
        dependency_mode = 'cargo-frozen'
        artifact_comparison = 'sha256-and-length'
        cargo_jobs = $cargoJobsResolved
        source_revision = "$revision$(if ($dirty.Count -ne 0) { '+dirty' } else { '' })"
        source_epoch = [int64]$sourceEpoch
        source_manifest_sha256 = Get-Sha256 (Join-Path $output 'SOURCE-MANIFEST.sha256')
        cargo_lock_sha256 = Get-Sha256 (Join-Path $output 'Cargo.lock')
        rust_toolchain_sha256 = Get-Sha256 (Join-Path $output 'rust-toolchain.toml')
        rustc = $rustcVersion
        cargo = $cargoVersion
        target = 'x86_64-pc-windows-msvc'
        incremental = $false
        path_remap = [ordered]@{
            mode = $buildA.path_remap.mode
            source_root = $buildA.path_remap.source_root
            cargo_registry_source_root = $buildA.path_remap.cargo_registry_source_root
            canonical_cargo_registry_source_present = $buildA.path_remap.canonical_cargo_registry_source_present
            raw_cargo_registry_source_absent = $buildA.path_remap.raw_cargo_registry_source_absent
        }
        blake3_codegen = 'pure-rust-intrinsics'
        pe_metadata_normalization = 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'
        linker = [ordered]@{
            mode = $linkerIdentity.mode
            source = $linkerIdentity.source
            file = $linkerIdentity.file
            flavor = $linkerIdentity.flavor
            sha256 = $linkerIdentity.sha256
            bytes = $linkerIdentity.bytes
            reproducibility_flag = $linkerIdentity.reproducibility_flag
        }
        native_toolchain = [ordered]@{
            mode = $nativeToolchainIdentity.mode
            selection = $nativeToolchainIdentity.selection
            lock_file = $nativeToolchainIdentity.lock_file
            lock_sha256 = $nativeToolchainIdentity.lock_sha256
            count = $nativeToolchainIdentity.count
            msvc_version = $nativeToolchainIdentity.msvc_version
            windows_sdk_version = $nativeToolchainIdentity.windows_sdk_version
            architecture = $nativeToolchainIdentity.architecture
            libraries_bundled = $nativeToolchainIdentity.libraries_bundled
        }
        native_link_inputs = [ordered]@{
            capture_mode = 'lld-link-reproduce-archive'
            manifest_format = 'sha256-bytes-logical-path-v1'
            file = $nativeLinkInputs.file
            sha256 = $nativeLinkInputs.sha256
            count = $nativeLinkInputs.count
            archive_retained = $false
        }
        network_surface_compiled = $false
        runtime_surface_compiled = $false
        build_a = $buildARecord
        build_b = $buildBRecord
    }
    $json = $record | ConvertTo-Json -Depth 6
    Write-Utf8Lines (Join-Path $output 'REPRODUCIBILITY.json') @($json)

    $checksumFiles = @(
        'kilogram-offline-build-a.exe',
        'kilogram-offline-build-b.exe',
        'SOURCE-MANIFEST.sha256',
        'Cargo.lock',
        'rust-toolchain.toml',
        'WINDOWS-NATIVE-LINK-INPUTS.lock',
        'NATIVE-LINK-INPUTS.sha256',
        'REPRODUCIBILITY.json'
    )
    $checksums = foreach ($name in $checksumFiles) {
        "$(Get-Sha256 (Join-Path $output $name))  $name"
    }
    Write-Utf8Lines (Join-Path $output 'SHA256SUMS') $checksums

    if (-not $equal) {
        throw "Offline builds diverged: $($buildA.sha256) != $($buildB.sha256)"
    }
    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-reproducibility-record.ps1') -RecordDirectory $output
    if ($LASTEXITCODE -ne 0) {
        throw 'independent reproducibility-record verification failed'
    }
    Write-Output "reproducibility_directory=$output"
    Write-Output "source_revision=$($record.source_revision)"
    Write-Output "artifact_sha256=$($buildA.sha256)"
    Write-Output "artifact_bytes=$($buildA.bytes)"
    Write-Output 'status=kilogram-offline-reproducible'
}
finally {
    if (-not $KeepBuildRoots -and (Test-Path -LiteralPath $output)) {
        foreach ($entry in @(
            @{ Path = (Join-Path $output 'source-a'); Name = 'source-a' },
            @{ Path = (Join-Path $output 'source-b'); Name = 'source-b' },
            @{ Path = (Join-Path $output 'target-a'); Name = 'target-a' },
            @{ Path = (Join-Path $output 'target-b'); Name = 'target-b' }
        )) {
            Assert-DisposablePath $entry.Path $entry.Name
            if (Test-Path -LiteralPath $entry.Path) {
                Remove-Item -LiteralPath $entry.Path -Recurse -Force
            }
        }
    }
    Pop-Location
}
