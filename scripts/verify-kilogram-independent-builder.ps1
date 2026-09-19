[CmdletBinding()]
param(
    [string]$ArtifactPath,
    [string]$BuilderRecordPath,
    [string]$LocalRecordDirectory,
    [string]$Repository,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'kilogram-reproducible-linker.ps1')

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-Sha256([string]$Value, [string]$Field) {
    if ($Value -cnotmatch '^[0-9a-f]{64}$') {
        throw "Independent-builder field is not a lowercase SHA-256: $Field"
    }
}

function Resolve-PlainFile([string]$Path, [string]$Description) {
    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "$Description path is required."
    }
    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "$Description is missing: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "$Description must not be a reparse point: $resolved"
    }
    $resolved
}

function Read-BoundedJson([string]$Path, [string]$Description) {
    $item = Get-Item -LiteralPath $Path
    if ($item.Length -le 0 -or $item.Length -gt 64KB) {
        throw "$Description size is invalid."
    }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Assert-IndependentEvidence {
    param(
        [string]$IndependentArtifact,
        [string]$IndependentRecord,
        [string]$SameHostDirectory,
        [string]$ExpectedRepository,
        [switch]$SkipAttestation
    )

    if ($ExpectedRepository -cnotmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') {
        throw 'Repository must use the OWNER/REPOSITORY form.'
    }
    $artifact = Resolve-PlainFile $IndependentArtifact 'Independent artifact'
    if (-not ([System.IO.Path]::GetFileName($artifact).Equals('kilogram-offline.exe', [System.StringComparison]::Ordinal))) {
        throw 'Independent artifact must retain the stable name kilogram-offline.exe.'
    }
    $builderRecordPathResolved = Resolve-PlainFile $IndependentRecord 'Independent builder record'
    $localDirectory = [System.IO.Path]::GetFullPath($SameHostDirectory)
    if (-not (Test-Path -LiteralPath $localDirectory -PathType Container)) {
        throw "Local reproducibility record directory is missing: $localDirectory"
    }

    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-kilogram-offline-reproducibility-record.ps1') -RecordDirectory $localDirectory | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw 'M0.9.50 local reproducibility record verification failed.'
    }

    $localRecordPath = Resolve-PlainFile (Join-Path $localDirectory 'REPRODUCIBILITY.json') 'Local reproducibility record'
    $localRecord = Read-BoundedJson $localRecordPath 'Local reproducibility record'
    $builderRecord = Read-BoundedJson $builderRecordPathResolved 'Independent builder record'

    if ($localRecord.source_revision -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Local reproducibility record must identify a clean exact Git commit.'
    }
    if ($builderRecord.format_version -ne 6 -or
        $builderRecord.status -ne 'matched' -or
        $builderRecord.builder_scope -ne 'github-hosted-windows-independent' -or
        $builderRecord.repository -cne $ExpectedRepository -or
        $builderRecord.source_revision -cne $localRecord.source_revision -or
        [int64]$builderRecord.source_epoch -ne [int64]$localRecord.source_epoch -or
        $builderRecord.matches_expected_local_sha256 -ne $true -or
        $builderRecord.dependency_mode -ne 'cargo-fetch-locked-then-frozen' -or
        $builderRecord.target -ne 'x86_64-pc-windows-msvc' -or
        [int]$builderRecord.cargo_jobs -ne 2 -or
        $builderRecord.incremental -ne $false -or
        $builderRecord.path_remap.mode -ne 'rustc-dual-prefix-remap-with-pe-leak-check-v1' -or
        $builderRecord.path_remap.source_root -ne '<BUILD_ROOT>=Z:/kilogram-source' -or
        $builderRecord.path_remap.cargo_registry_source_root -ne '<CARGO_REGISTRY_SOURCE_ROOT>=Z:/cargo-registry-src' -or
        $builderRecord.path_remap.canonical_cargo_registry_source_present -ne $true -or
        $builderRecord.path_remap.raw_cargo_registry_source_absent -ne $true -or
        $builderRecord.blake3_codegen -ne 'pure-rust-intrinsics' -or
        $builderRecord.pe_metadata_normalization -ne 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1' -or
        $builderRecord.linker.mode -ne 'rust-toolchain-bundled-lld' -or
        $builderRecord.linker.source -ne 'rustc-sysroot-target-bin' -or
        $builderRecord.linker.file -ne 'rust-lld.exe' -or
        $builderRecord.linker.flavor -ne 'lld-link' -or
        [int64]$builderRecord.linker.bytes -le 0 -or
        $builderRecord.linker.reproducibility_flag -ne '/Brepro' -or
        $builderRecord.native_toolchain.mode -ne 'repository-hash-locked-installed-libraries' -or
        $builderRecord.native_toolchain.selection -ne 'explicit-final-rustc-native-search-paths' -or
        $builderRecord.native_toolchain.lock_file -ne 'WINDOWS-NATIVE-LINK-INPUTS.lock' -or
        [int]$builderRecord.native_toolchain.count -ne 10 -or
        $builderRecord.native_toolchain.msvc_version -ne '14.44.35207' -or
        $builderRecord.native_toolchain.windows_sdk_version -ne '10.0.19041.0' -or
        $builderRecord.native_toolchain.architecture -ne 'x64' -or
        $builderRecord.native_toolchain.libraries_bundled -ne $false -or
        $builderRecord.native_link_inputs.capture_mode -ne 'lld-link-reproduce-archive' -or
        $builderRecord.native_link_inputs.manifest_format -ne 'sha256-bytes-logical-path-v1' -or
        $builderRecord.native_link_inputs.file -ne 'NATIVE-LINK-INPUTS.sha256' -or
        [int]$builderRecord.native_link_inputs.count -le 0 -or
        [int]$builderRecord.native_link_inputs.count -gt 256 -or
        $builderRecord.native_link_inputs.archive_retained -ne $false -or
        $builderRecord.network_surface_compiled -ne $false -or
        $builderRecord.runtime_surface_compiled -ne $false -or
        $builderRecord.workflow.event -ne 'workflow_dispatch' -or
        $builderRecord.workflow.name -ne 'Independent offline reproduction' -or
        $builderRecord.runner.os -ne 'Windows' -or
        $builderRecord.runner.environment -ne 'github-hosted') {
        throw 'Independent builder identity or build boundary is invalid.'
    }
    if ([int64]$builderRecord.source_epoch -le 0 -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.rustc) -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.cargo) -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.runner.image_os) -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.runner.image_version) -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.workflow.run_id) -or
        [string]::IsNullOrWhiteSpace([string]$builderRecord.workflow.run_attempt)) {
        throw 'Independent builder runtime identity is incomplete.'
    }
    if ([string]$builderRecord.rustc -cne [string]$localRecord.rustc -or
        [string]$builderRecord.cargo -cne [string]$localRecord.cargo) {
        throw 'Independent builder did not use the locally recorded pinned Rust/Cargo toolchain.'
    }
    if ([string]$builderRecord.path_remap.mode -cne [string]$localRecord.path_remap.mode -or
        [string]$builderRecord.path_remap.source_root -cne [string]$localRecord.path_remap.source_root -or
        [string]$builderRecord.path_remap.cargo_registry_source_root -cne [string]$localRecord.path_remap.cargo_registry_source_root -or
        $builderRecord.path_remap.canonical_cargo_registry_source_present -ne $localRecord.path_remap.canonical_cargo_registry_source_present -or
        $builderRecord.path_remap.raw_cargo_registry_source_absent -ne $localRecord.path_remap.raw_cargo_registry_source_absent) {
        throw 'Independent builder did not use the exact locally recorded path-remapping boundary.'
    }
    if ([string]$builderRecord.linker.sha256 -cne [string]$localRecord.linker.sha256 -or
        [int64]$builderRecord.linker.bytes -ne [int64]$localRecord.linker.bytes -or
        [string]$builderRecord.linker.mode -cne [string]$localRecord.linker.mode -or
        [string]$builderRecord.linker.source -cne [string]$localRecord.linker.source -or
        [string]$builderRecord.linker.file -cne [string]$localRecord.linker.file -or
        [string]$builderRecord.linker.flavor -cne [string]$localRecord.linker.flavor -or
        [string]$builderRecord.linker.reproducibility_flag -cne [string]$localRecord.linker.reproducibility_flag -or
        [string]$builderRecord.pe_metadata_normalization -cne [string]$localRecord.pe_metadata_normalization) {
        throw 'Independent builder did not use the exact locally recorded bundled LLD linker.'
    }
    if ([string]$builderRecord.native_toolchain.mode -cne [string]$localRecord.native_toolchain.mode -or
        [string]$builderRecord.native_toolchain.selection -cne [string]$localRecord.native_toolchain.selection -or
        [string]$builderRecord.native_toolchain.lock_file -cne [string]$localRecord.native_toolchain.lock_file -or
        [string]$builderRecord.native_toolchain.lock_sha256 -cne [string]$localRecord.native_toolchain.lock_sha256 -or
        [int]$builderRecord.native_toolchain.count -ne [int]$localRecord.native_toolchain.count -or
        [string]$builderRecord.native_toolchain.msvc_version -cne [string]$localRecord.native_toolchain.msvc_version -or
        [string]$builderRecord.native_toolchain.windows_sdk_version -cne [string]$localRecord.native_toolchain.windows_sdk_version -or
        [string]$builderRecord.native_toolchain.architecture -cne [string]$localRecord.native_toolchain.architecture -or
        $builderRecord.native_toolchain.libraries_bundled -ne $localRecord.native_toolchain.libraries_bundled) {
        throw 'Independent builder did not use the exact locally recorded native toolchain lock.'
    }
    if ($localRecord.native_link_inputs.capture_mode -ne 'lld-link-reproduce-archive' -or
        $localRecord.native_link_inputs.manifest_format -ne 'sha256-bytes-logical-path-v1' -or
        $localRecord.native_link_inputs.file -ne 'NATIVE-LINK-INPUTS.sha256' -or
        $localRecord.native_link_inputs.archive_retained -ne $false) {
        throw 'Local native link-input identity is invalid.'
    }
    if ($builderRecord.artifact.file -ne 'kilogram-offline.exe') {
        throw 'Independent builder record contains an unexpected artifact name.'
    }

    $localNativeManifest = Resolve-PlainFile `
        (Join-Path $localDirectory ([string]$localRecord.native_link_inputs.file)) `
        'Local native link-input manifest'
    $independentDirectory = Split-Path -Parent $builderRecordPathResolved
    $independentNativeManifest = Resolve-PlainFile `
        (Join-Path $independentDirectory ([string]$builderRecord.native_link_inputs.file)) `
        'Independent native link-input manifest'
    $localNativeLock = Resolve-PlainFile `
        (Join-Path $localDirectory ([string]$localRecord.native_toolchain.lock_file)) `
        'Local pinned native-toolchain lock'
    $independentNativeLock = Resolve-PlainFile `
        (Join-Path $independentDirectory ([string]$builderRecord.native_toolchain.lock_file)) `
        'Independent pinned native-toolchain lock'
    $localNativeIdentity = Assert-KilogramNativeLinkInputManifest -Path $localNativeManifest
    $independentNativeIdentity = Assert-KilogramNativeLinkInputManifest -Path $independentNativeManifest
    $localNativeLockIdentity = Assert-KilogramNativeLinkInputManifest -Path $localNativeLock
    $independentNativeLockIdentity = Assert-KilogramNativeLinkInputManifest -Path $independentNativeLock

    foreach ($hashField in @(
        @{ Value = [string]$localRecord.build_a.sha256; Name = 'local.build_a.sha256' },
        @{ Value = [string]$localRecord.build_b.sha256; Name = 'local.build_b.sha256' },
        @{ Value = [string]$localRecord.linker.sha256; Name = 'local.linker.sha256' },
        @{ Value = [string]$localRecord.native_toolchain.lock_sha256; Name = 'local.native_toolchain.lock_sha256' },
        @{ Value = [string]$localRecord.native_link_inputs.sha256; Name = 'local.native_link_inputs.sha256' },
        @{ Value = [string]$builderRecord.linker.sha256; Name = 'builder.linker.sha256' },
        @{ Value = [string]$builderRecord.native_toolchain.lock_sha256; Name = 'builder.native_toolchain.lock_sha256' },
        @{ Value = [string]$builderRecord.native_link_inputs.sha256; Name = 'builder.native_link_inputs.sha256' },
        @{ Value = [string]$builderRecord.expected_local_sha256; Name = 'expected_local_sha256' },
        @{ Value = [string]$builderRecord.artifact.sha256; Name = 'artifact.sha256' }
    )) {
        Assert-Sha256 $hashField.Value $hashField.Name
    }
    if ([string]$localNativeIdentity.sha256 -cne [string]$localRecord.native_link_inputs.sha256 -or
        [int]$localNativeIdentity.count -ne [int]$localRecord.native_link_inputs.count -or
        [string]$independentNativeIdentity.sha256 -cne [string]$builderRecord.native_link_inputs.sha256 -or
        [int]$independentNativeIdentity.count -ne [int]$builderRecord.native_link_inputs.count -or
        [string]$independentNativeIdentity.sha256 -cne [string]$localNativeIdentity.sha256 -or
        [int]$independentNativeIdentity.count -ne [int]$localNativeIdentity.count) {
        throw 'Independent builder did not consume the exact locally recorded native link inputs.'
    }
    $localLockCanonicalHash = Get-KilogramCanonicalNativeLinkInputSha256 `
        -Lines @(Get-Content -LiteralPath $localNativeLock)
    $independentLockCanonicalHash = Get-KilogramCanonicalNativeLinkInputSha256 `
        -Lines @(Get-Content -LiteralPath $independentNativeLock)
    if ([int]$localNativeLockIdentity.count -ne [int]$localRecord.native_toolchain.count -or
        [int]$independentNativeLockIdentity.count -ne [int]$builderRecord.native_toolchain.count -or
        [string]$localLockCanonicalHash -cne [string]$localRecord.native_toolchain.lock_sha256 -or
        [string]$independentLockCanonicalHash -cne [string]$builderRecord.native_toolchain.lock_sha256 -or
        [string]$localLockCanonicalHash -cne [string]$independentLockCanonicalHash) {
        throw 'Independent builder native toolchain lock is not identical to the local lock.'
    }
    $null = Assert-KilogramNativeLinkInputManifestMatchesLock `
        -ManifestPath $localNativeManifest `
        -LockPath $localNativeLock
    $null = Assert-KilogramNativeLinkInputManifestMatchesLock `
        -ManifestPath $independentNativeManifest `
        -LockPath $independentNativeLock

    $actualHash = Get-Sha256 $artifact
    $actualBytes = (Get-Item -LiteralPath $artifact).Length
    $null = Assert-KilogramPeCanonicalPathRemapping -Path $artifact
    if ($localRecord.build_a.sha256 -cne $localRecord.build_b.sha256 -or
        $builderRecord.expected_local_sha256 -cne $localRecord.build_a.sha256 -or
        $builderRecord.artifact.sha256 -cne $localRecord.build_a.sha256 -or
        $actualHash -cne $localRecord.build_a.sha256 -or
        [int64]$builderRecord.artifact.bytes -ne $actualBytes -or
        [int64]$localRecord.build_a.bytes -ne $actualBytes -or
        [int64]$localRecord.build_b.bytes -ne $actualBytes) {
        throw 'Independent artifact is not byte-identical to both local clean-root builds.'
    }

    if (-not $SkipAttestation) {
        Assert-KilogramPeReproducibilityMetadataNormalized -Path $artifact
        $gh = Get-Command gh -ErrorAction SilentlyContinue
        if ($null -eq $gh) {
            throw 'GitHub CLI (gh) is required to verify signed provenance.'
        }
        $workflowIdentity = "$ExpectedRepository/.github/workflows/independent-offline-reproduction.yml"
        foreach ($subject in @($artifact, $builderRecordPathResolved, $independentNativeLock, $independentNativeManifest)) {
            & $gh.Source attestation verify $subject --repo $ExpectedRepository --signer-workflow $workflowIdentity --source-digest $localRecord.source_revision --deny-self-hosted-runners
            if ($LASTEXITCODE -ne 0) {
                throw "GitHub artifact attestation verification failed: $subject"
            }
        }
    }

    Write-Output 'independent_builder_record=verified'
    Write-Output "repository=$ExpectedRepository"
    Write-Output "source_revision=$($localRecord.source_revision)"
    Write-Output "artifact_sha256=$actualHash"
    Write-Output "artifact_bytes=$actualBytes"
    Write-Output "blake3_codegen=$($builderRecord.blake3_codegen)"
    Write-Output "linker_sha256=$($builderRecord.linker.sha256)"
    Write-Output "native_toolchain_lock_sha256=$($builderRecord.native_toolchain.lock_sha256)"
    Write-Output "native_toolchain_msvc=$($builderRecord.native_toolchain.msvc_version)"
    Write-Output "native_toolchain_windows_sdk=$($builderRecord.native_toolchain.windows_sdk_version)"
    Write-Output "native_link_inputs_sha256=$($builderRecord.native_link_inputs.sha256)"
    Write-Output "native_link_inputs_count=$($builderRecord.native_link_inputs.count)"
    Write-Output "pe_metadata_normalization=$($builderRecord.pe_metadata_normalization)"
    Write-Output "attestation_verified=$(((-not $SkipAttestation)).ToString().ToLowerInvariant())"
}

function Invoke-SelfTest {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ("kilogram-independent-builder-" + [Guid]::NewGuid().ToString('N'))
    $local = Join-Path $root 'local'
    $independent = Join-Path $root 'independent'
    New-Item -ItemType Directory -Path $local, $independent | Out-Null
    try {
        $revision = '1234567890abcdef1234567890abcdef12345678'
        $artifactBytes = [byte[]]::new(1024)
        $artifactBytes[0] = 0x4d
        $artifactBytes[1] = 0x5a
        [System.BitConverter]::GetBytes([uint32]0x80).CopyTo($artifactBytes, 0x3c)
        $artifactBytes[0x80] = 0x50
        $artifactBytes[0x81] = 0x45
        [System.BitConverter]::GetBytes([uint32]0x11223344).CopyTo($artifactBytes, 0x88)
        [System.BitConverter]::GetBytes([uint16]1).CopyTo($artifactBytes, 0x86)
        [System.BitConverter]::GetBytes([uint16]240).CopyTo($artifactBytes, 0x94)
        [System.BitConverter]::GetBytes([uint16]0x20b).CopyTo($artifactBytes, 0x98)
        [System.BitConverter]::GetBytes([uint32]16).CopyTo($artifactBytes, 0x104)
        [System.BitConverter]::GetBytes([uint32]0x1000).CopyTo($artifactBytes, 0x138)
        [System.BitConverter]::GetBytes([uint32]56).CopyTo($artifactBytes, 0x13c)
        [System.Text.Encoding]::ASCII.GetBytes('.rdata').CopyTo($artifactBytes, 0x188)
        [System.BitConverter]::GetBytes([uint32]0x200).CopyTo($artifactBytes, 0x190)
        [System.BitConverter]::GetBytes([uint32]0x1000).CopyTo($artifactBytes, 0x194)
        [System.BitConverter]::GetBytes([uint32]0x200).CopyTo($artifactBytes, 0x198)
        [System.BitConverter]::GetBytes([uint32]0x200).CopyTo($artifactBytes, 0x19c)
        [System.BitConverter]::GetBytes([uint32]0x55667788).CopyTo($artifactBytes, 0x204)
        [System.BitConverter]::GetBytes([uint32]2).CopyTo($artifactBytes, 0x20c)
        [System.BitConverter]::GetBytes([uint32]32).CopyTo($artifactBytes, 0x210)
        [System.BitConverter]::GetBytes([uint32]0x1038).CopyTo($artifactBytes, 0x214)
        [System.BitConverter]::GetBytes([uint32]0x238).CopyTo($artifactBytes, 0x218)
        [System.BitConverter]::GetBytes([uint32]0x33445566).CopyTo($artifactBytes, 0x220)
        [System.BitConverter]::GetBytes([uint32]16).CopyTo($artifactBytes, 0x228)
        [System.Text.Encoding]::ASCII.GetBytes('RSDS').CopyTo($artifactBytes, 0x238)
        for ($index = 0; $index -lt 16; $index++) {
            $artifactBytes[0x23c + $index] = [byte]($index + 1)
        }
        [System.BitConverter]::GetBytes([uint32]1).CopyTo($artifactBytes, 0x24c)
        [System.Text.Encoding]::ASCII.GetBytes("self.pdb`0").CopyTo($artifactBytes, 0x250)
        [System.Text.Encoding]::ASCII.GetBytes("Z:/cargo-registry-src/index.crates.io-self-test`0").CopyTo($artifactBytes, 0x300)
        $buildA = Join-Path $local 'kilogram-offline-build-a.exe'
        $buildB = Join-Path $local 'kilogram-offline-build-b.exe'
        $artifact = Join-Path $independent 'kilogram-offline.exe'
        [System.IO.File]::WriteAllBytes($buildA, $artifactBytes)
        [System.IO.File]::WriteAllBytes($buildB, $artifactBytes)
        [System.IO.File]::WriteAllBytes($artifact, $artifactBytes)
        $nonNormalizedRejected = $false
        try {
            Assert-KilogramPeReproducibilityMetadataNormalized -Path $artifact
        }
        catch {
            $nonNormalizedRejected = $true
        }
        if (-not $nonNormalizedRejected) {
            throw 'Self-test verifier accepted non-normalized PE metadata.'
        }
        $checksumFixture = Join-Path $independent 'checksum-bearing.exe'
        $checksumBytes = [byte[]]$artifactBytes.Clone()
        [System.BitConverter]::GetBytes([uint32]1).CopyTo($checksumBytes, 0xd8)
        [System.IO.File]::WriteAllBytes($checksumFixture, $checksumBytes)
        $checksumRejected = $false
        try {
            Normalize-KilogramPeReproducibilityMetadata -Path $checksumFixture
        }
        catch {
            $checksumRejected = $true
        }
        if (-not $checksumRejected) {
            throw 'Self-test normalizer accepted a PE image with a non-zero checksum.'
        }
        $signedFixture = Join-Path $independent 'authenticode-bearing.exe'
        $signedBytes = [byte[]]$artifactBytes.Clone()
        [System.BitConverter]::GetBytes([uint32]0x300).CopyTo($signedBytes, 0x128)
        [System.BitConverter]::GetBytes([uint32]8).CopyTo($signedBytes, 0x12c)
        [System.IO.File]::WriteAllBytes($signedFixture, $signedBytes)
        $authenticodeRejected = $false
        try {
            Normalize-KilogramPeReproducibilityMetadata -Path $signedFixture
        }
        catch {
            $authenticodeRejected = $true
        }
        if (-not $authenticodeRejected) {
            throw 'Self-test normalizer accepted an Authenticode-bearing PE image.'
        }
        $rawCargoPathFixture = Join-Path $independent 'host-cargo-registry-path-bearing.exe'
        $rawCargoPathBytes = [byte[]]$artifactBytes.Clone()
        [System.Text.Encoding]::ASCII.GetBytes("C:\Users\runneradmin\.cargo\registry\src\index.crates.io-self-test`0").CopyTo($rawCargoPathBytes, 0x340)
        [System.IO.File]::WriteAllBytes($rawCargoPathFixture, $rawCargoPathBytes)
        $rawCargoPathRejected = $false
        try {
            Assert-KilogramPeCanonicalPathRemapping -Path $rawCargoPathFixture | Out-Null
        }
        catch {
            $rawCargoPathRejected = $true
        }
        if (-not $rawCargoPathRejected) {
            throw 'Self-test path verifier accepted a host Cargo registry source path.'
        }
        Normalize-KilogramPeReproducibilityMetadata -Path $buildA
        Normalize-KilogramPeReproducibilityMetadata -Path $buildB
        Normalize-KilogramPeReproducibilityMetadata -Path $artifact
        [System.IO.File]::WriteAllText((Join-Path $local 'SOURCE-MANIFEST.sha256'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText((Join-Path $local 'Cargo.lock'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText((Join-Path $local 'rust-toolchain.toml'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        $hash = Get-Sha256 $artifact
        $length = (Get-Item -LiteralPath $artifact).Length
        $nativeManifestLines = @(
            "$hash  1  msvc/14.44.35207/lib/x64/msvcrt.lib",
            "$hash  1  msvc/14.44.35207/lib/x64/vcruntime.lib",
            "$hash  1  windows-sdk/10.0.19041.0/ucrt/x64/ucrt.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/advapi32.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/bcrypt.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/dbghelp.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/kernel32.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/ntdll.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/userenv.lib",
            "$hash  1  windows-sdk/10.0.19041.0/um/x64/ws2_32.lib"
        )
        $localNativeManifest = Join-Path $local 'NATIVE-LINK-INPUTS.sha256'
        $independentNativeManifest = Join-Path $independent 'NATIVE-LINK-INPUTS.sha256'
        $localNativeLock = Join-Path $local 'WINDOWS-NATIVE-LINK-INPUTS.lock'
        $independentNativeLock = Join-Path $independent 'WINDOWS-NATIVE-LINK-INPUTS.lock'
        $canonicalNativeText = ($nativeManifestLines -join "`r`n") + "`r`n"
        foreach ($path in @($localNativeManifest, $independentNativeManifest, $localNativeLock, $independentNativeLock)) {
            [System.IO.File]::WriteAllText($path, $canonicalNativeText, [System.Text.UTF8Encoding]::new($false))
        }
        $nativeIdentity = Assert-KilogramNativeLinkInputManifest -Path $localNativeManifest
        $nativeLockHash = Get-KilogramCanonicalNativeLinkInputSha256 -Lines $nativeManifestLines
        $invalidNativeManifest = Join-Path $independent 'INVALID-NATIVE-LINK-INPUTS.sha256'
        [System.IO.File]::WriteAllLines(
            $invalidNativeManifest,
            @("$hash  1  unclassified/unknown.lib"),
            [System.Text.UTF8Encoding]::new($false)
        )
        $invalidNativeManifestRejected = $false
        try {
            Assert-KilogramNativeLinkInputManifest -Path $invalidNativeManifest | Out-Null
        }
        catch {
            $invalidNativeManifestRejected = $true
        }
        if (-not $invalidNativeManifestRejected) {
            throw 'Self-test accepted a malformed native link-input manifest.'
        }
        $localRecord = [ordered]@{
            format_version = 6
            status = 'reproducible'
            builder_scope = 'same-host-separate-clean-roots'
            build_root_count = 2
            dependency_mode = 'cargo-frozen'
            artifact_comparison = 'sha256-and-length'
            cargo_jobs = 1
            source_revision = $revision
            source_epoch = 1
            source_manifest_sha256 = Get-Sha256 (Join-Path $local 'SOURCE-MANIFEST.sha256')
            cargo_lock_sha256 = Get-Sha256 (Join-Path $local 'Cargo.lock')
            rust_toolchain_sha256 = Get-Sha256 (Join-Path $local 'rust-toolchain.toml')
            rustc = 'rustc self-test'
            cargo = 'cargo self-test'
            target = 'x86_64-pc-windows-msvc'
            incremental = $false
            path_remap = [ordered]@{
                mode = 'rustc-dual-prefix-remap-with-pe-leak-check-v1'
                source_root = '<BUILD_ROOT>=Z:/kilogram-source'
                cargo_registry_source_root = '<CARGO_REGISTRY_SOURCE_ROOT>=Z:/cargo-registry-src'
                canonical_cargo_registry_source_present = $true
                raw_cargo_registry_source_absent = $true
            }
            blake3_codegen = 'pure-rust-intrinsics'
            pe_metadata_normalization = 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'
            linker = [ordered]@{
                mode = 'rust-toolchain-bundled-lld'
                source = 'rustc-sysroot-target-bin'
                file = 'rust-lld.exe'
                flavor = 'lld-link'
                sha256 = $hash
                bytes = 1
                reproducibility_flag = '/Brepro'
            }
            native_toolchain = [ordered]@{
                mode = 'repository-hash-locked-installed-libraries'
                selection = 'explicit-final-rustc-native-search-paths'
                lock_file = 'WINDOWS-NATIVE-LINK-INPUTS.lock'
                lock_sha256 = $nativeLockHash
                count = $nativeIdentity.count
                msvc_version = '14.44.35207'
                windows_sdk_version = '10.0.19041.0'
                architecture = 'x64'
                libraries_bundled = $false
            }
            native_link_inputs = [ordered]@{
                capture_mode = 'lld-link-reproduce-archive'
                manifest_format = 'sha256-bytes-logical-path-v1'
                file = 'NATIVE-LINK-INPUTS.sha256'
                sha256 = $nativeIdentity.sha256
                count = $nativeIdentity.count
                archive_retained = $false
            }
            network_surface_compiled = $false
            runtime_surface_compiled = $false
            build_a = [ordered]@{ file = 'kilogram-offline-build-a.exe'; sha256 = $hash; bytes = $length }
            build_b = [ordered]@{ file = 'kilogram-offline-build-b.exe'; sha256 = $hash; bytes = $length }
        }
        [System.IO.File]::WriteAllText((Join-Path $local 'REPRODUCIBILITY.json'), ($localRecord | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

        $builderRecord = [ordered]@{
            format_version = 6
            status = 'matched'
            builder_scope = 'github-hosted-windows-independent'
            repository = 'gugglegum/kilogram-messenger'
            source_revision = $revision
            source_epoch = 1
            expected_local_sha256 = $hash
            matches_expected_local_sha256 = $true
            dependency_mode = 'cargo-fetch-locked-then-frozen'
            target = 'x86_64-pc-windows-msvc'
            cargo_jobs = 2
            incremental = $false
            path_remap = [ordered]@{
                mode = 'rustc-dual-prefix-remap-with-pe-leak-check-v1'
                source_root = '<BUILD_ROOT>=Z:/kilogram-source'
                cargo_registry_source_root = '<CARGO_REGISTRY_SOURCE_ROOT>=Z:/cargo-registry-src'
                canonical_cargo_registry_source_present = $true
                raw_cargo_registry_source_absent = $true
            }
            blake3_codegen = 'pure-rust-intrinsics'
            pe_metadata_normalization = 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'
            linker = [ordered]@{
                mode = 'rust-toolchain-bundled-lld'
                source = 'rustc-sysroot-target-bin'
                file = 'rust-lld.exe'
                flavor = 'lld-link'
                sha256 = $hash
                bytes = 1
                reproducibility_flag = '/Brepro'
            }
            native_toolchain = [ordered]@{
                mode = 'repository-hash-locked-installed-libraries'
                selection = 'explicit-final-rustc-native-search-paths'
                lock_file = 'WINDOWS-NATIVE-LINK-INPUTS.lock'
                lock_sha256 = $nativeLockHash
                count = $nativeIdentity.count
                msvc_version = '14.44.35207'
                windows_sdk_version = '10.0.19041.0'
                architecture = 'x64'
                libraries_bundled = $false
            }
            native_link_inputs = [ordered]@{
                capture_mode = 'lld-link-reproduce-archive'
                manifest_format = 'sha256-bytes-logical-path-v1'
                file = 'NATIVE-LINK-INPUTS.sha256'
                sha256 = $nativeIdentity.sha256
                count = $nativeIdentity.count
                archive_retained = $false
            }
            network_surface_compiled = $false
            runtime_surface_compiled = $false
            rustc = 'rustc self-test'
            cargo = 'cargo self-test'
            runner = [ordered]@{ os = 'Windows'; arch = 'X64'; environment = 'github-hosted'; name = 'self-test'; image_os = 'win25'; image_version = 'self-test' }
            workflow = [ordered]@{ event = 'workflow_dispatch'; name = 'Independent offline reproduction'; ref = 'refs/heads/master'; workflow_ref = 'gugglegum/kilogram-messenger/.github/workflows/independent-offline-reproduction.yml@refs/heads/master'; run_id = '1'; run_attempt = '1' }
            artifact = [ordered]@{ file = 'kilogram-offline.exe'; sha256 = $hash; bytes = $length }
        }
        $builderRecordPath = Join-Path $independent 'INDEPENDENT-BUILDER.json'
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))

        Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        $builderRecord.linker.sha256 = '0000000000000000000000000000000000000000000000000000000000000000'
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        $linkerRejected = $false
        try {
            Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        }
        catch {
            $linkerRejected = $true
        }
        if (-not $linkerRejected) {
            throw 'Self-test verifier accepted a mismatched independent linker.'
        }
        $builderRecord.linker.sha256 = $hash
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        $builderRecord.native_link_inputs.sha256 = '0000000000000000000000000000000000000000000000000000000000000000'
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        $nativeInputsRejected = $false
        try {
            Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        }
        catch {
            $nativeInputsRejected = $true
        }
        if (-not $nativeInputsRejected) {
            throw 'Self-test verifier accepted mismatched native link inputs.'
        }
        $builderRecord.native_link_inputs.sha256 = $nativeIdentity.sha256
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        $tamperedNativeLines = @($nativeManifestLines)
        $tamperedNativeLines[0] = ('0' * 64) + $tamperedNativeLines[0].Substring(64)
        [System.IO.File]::WriteAllText(
            $independentNativeLock,
            (($tamperedNativeLines -join "`r`n") + "`r`n"),
            [System.Text.UTF8Encoding]::new($false)
        )
        $tamperedNativeToolchainLockRejected = $false
        try {
            Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        }
        catch {
            $tamperedNativeToolchainLockRejected = $true
        }
        if (-not $tamperedNativeToolchainLockRejected) {
            throw 'Self-test verifier accepted a tampered native toolchain lock file.'
        }
        [System.IO.File]::WriteAllText($independentNativeLock, $canonicalNativeText, [System.Text.UTF8Encoding]::new($false))
        $builderRecord.native_toolchain.lock_sha256 = '0000000000000000000000000000000000000000000000000000000000000000'
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        $nativeToolchainLockRejected = $false
        try {
            Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        }
        catch {
            $nativeToolchainLockRejected = $true
        }
        if (-not $nativeToolchainLockRejected) {
            throw 'Self-test verifier accepted a mismatched native toolchain lock.'
        }
        $builderRecord.native_toolchain.lock_sha256 = $nativeLockHash
        [System.IO.File]::WriteAllText($builderRecordPath, ($builderRecord | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
        Add-Content -LiteralPath $artifact -Value 'tamper'
        $rejected = $false
        try {
            Assert-IndependentEvidence -IndependentArtifact $artifact -IndependentRecord $builderRecordPath -SameHostDirectory $local -ExpectedRepository 'gugglegum/kilogram-messenger' -SkipAttestation | Out-Null
        }
        catch {
            $rejected = $true
        }
        if (-not $rejected) {
            throw 'Self-test verifier accepted a tampered independent artifact.'
        }
        Write-Output 'independent_builder_self_test=passed'
        Write-Output 'non_normalized_pe=rejected'
        Write-Output 'checksum_bearing_pe=rejected'
        Write-Output 'authenticode_bearing_pe=rejected'
        Write-Output 'host_cargo_registry_path=rejected'
        Write-Output 'mismatched_linker=rejected'
        Write-Output 'tampered_native_toolchain_lock=rejected'
        Write-Output 'mismatched_native_toolchain_lock=rejected'
        Write-Output 'mismatched_native_link_inputs=rejected'
        Write-Output 'malformed_native_link_manifest=rejected'
        Write-Output 'tampered_artifact=rejected'
    }
    finally {
        $resolvedRoot = [System.IO.Path]::GetFullPath($root)
        $tempPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if ($resolvedRoot.StartsWith($tempPrefix, [System.StringComparison]::OrdinalIgnoreCase) -and
            [System.IO.Path]::GetFileName($resolvedRoot).StartsWith('kilogram-independent-builder-', [System.StringComparison]::Ordinal)) {
            Remove-Item -LiteralPath $resolvedRoot -Recurse -Force
        }
    }
}

if ($SelfTest) {
    if (-not [string]::IsNullOrWhiteSpace($ArtifactPath) -or
        -not [string]::IsNullOrWhiteSpace($BuilderRecordPath) -or
        -not [string]::IsNullOrWhiteSpace($LocalRecordDirectory) -or
        -not [string]::IsNullOrWhiteSpace($Repository)) {
        throw 'SelfTest does not accept external evidence parameters.'
    }
    Invoke-SelfTest
    exit 0
}

Assert-IndependentEvidence -IndependentArtifact $ArtifactPath -IndependentRecord $BuilderRecordPath -SameHostDirectory $LocalRecordDirectory -ExpectedRepository $Repository
