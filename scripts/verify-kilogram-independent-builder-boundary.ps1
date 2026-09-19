[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workflowPath = Join-Path $workspace '.github\workflows\independent-offline-reproduction.yml'
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-independent-builder.ps1'
$localBuilderPath = Join-Path $PSScriptRoot 'build-kilogram-offline-reproducible.ps1'
$linkerHelperPath = Join-Path $PSScriptRoot 'kilogram-reproducible-linker.ps1'
$offlineBoundaryPath = Join-Path $PSScriptRoot 'verify-kilogram-offline-boundary.ps1'
$nativeLockPath = Join-Path $workspace 'WINDOWS-NATIVE-LINK-INPUTS.lock'
foreach ($path in @($workflowPath, $verifierPath, $localBuilderPath, $linkerHelperPath, $offlineBoundaryPath, $nativeLockPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Independent-builder boundary file is missing: $path"
    }
}

$workflow = Get-Content -LiteralPath $workflowPath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$localBuilder = Get-Content -LiteralPath $localBuilderPath -Raw
$linkerHelper = Get-Content -LiteralPath $linkerHelperPath -Raw
$offlineBoundary = Get-Content -LiteralPath $offlineBoundaryPath -Raw
$nativeLock = @(Get-Content -LiteralPath $nativeLockPath)
if ($nativeLock.Count -ne 10 -or
    @($nativeLock | Where-Object { $_ -cmatch 'msvc/14\.44\.35207/' }).Count -ne 2 -or
    @($nativeLock | Where-Object { $_ -cmatch 'windows-sdk/10\.0\.19041\.0/' }).Count -ne 8) {
    throw 'Pinned native link-input lock does not retain the exact M0.9.79 version boundary.'
}

foreach ($required in @(
    'workflow_dispatch:',
    'source_revision:',
    'expected_local_sha256:',
    'runs-on: windows-2022',
    'contents: read',
    'id-token: write',
    'attestations: write',
    'artifact-metadata: write',
    'persist-credentials: false',
    'cargo fetch --locked --target x86_64-pc-windows-msvc',
    'verify-kilogram-offline-boundary.ps1',
    "blake3_codegen = 'pure-rust-intrinsics'",
    "'rustc', '--jobs', '2', '--frozen', '--release'",
    '@($nativeToolchain.rustc_arguments)',
    'kilogram-reproducible-linker.ps1',
    'Get-KilogramBundledLldIdentity',
    'New-KilogramReproducibleRustFlags',
    'Get-KilogramPinnedNativeToolchainIdentity',
    'Assert-KilogramNativeLinkInputManifestMatchesLock',
    'Normalize-KilogramPeReproducibilityMetadata -Path $artifact',
    'format_version = 5',
    'Write-KilogramNativeLinkInputManifest',
    "manifest_format = 'sha256-bytes-logical-path-v1'",
    'NATIVE-LINK-INPUTS.sha256',
    'WINDOWS-NATIVE-LINK-INPUTS.lock',
    'Remove-Item -LiteralPath $linkRepro -Force',
    "pe_metadata_normalization = 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'",
    'sha256 = $linker.sha256',
    'The bundled LLD linker changed during the independent build.',
    'actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10',
    'actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6',
    "if: steps.reproduce.outputs.matches_expected == 'true'",
    'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a',
    'compression-level: 0',
    'Fail closed on byte divergence'
)) {
    if ($workflow.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Independent-builder workflow boundary is missing: $required"
    }
}
foreach ($forbidden in @(
    "`n  push:",
    "`n  pull_request:",
    "`n  schedule:",
    "`n  release:",
    'Compress-Archive',
    'package-kilogram-offline.ps1',
    'runs-on: self-hosted',
    'runs-on: windows-2025',
    'runs-on: windows-latest',
    'persist-credentials: true',
    'linker-features=+lld',
    'independent-builder/link-repro.tar'
)) {
    if ($workflow.IndexOf($forbidden, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "Independent-builder workflow contains forbidden automatic/package boundary: $forbidden"
    }
}
foreach ($actionUse in [regex]::Matches($workflow, '(?m)^\s*uses:\s+([^\s#]+)')) {
    if ($actionUse.Groups[1].Value -cnotmatch '@[0-9a-f]{40}$') {
        throw "Every third-party workflow action must be pinned to an exact commit: $($actionUse.Groups[1].Value)"
    }
}

foreach ($required in @(
    'format_version -ne 5',
    "builder_scope -ne 'github-hosted-windows-independent'",
    "linker.mode -ne 'rust-toolchain-bundled-lld'",
    "pe_metadata_normalization -ne 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'",
    "blake3_codegen -ne 'pure-rust-intrinsics'",
    'builderRecord.linker.sha256 -cne [string]$localRecord.linker.sha256',
    'Independent builder did not consume the exact locally recorded native link inputs.',
    'Independent builder did not use the exact locally recorded native toolchain lock.',
    'non_normalized_pe=rejected',
    'checksum_bearing_pe=rejected',
    'authenticode_bearing_pe=rejected',
    'mismatched_linker=rejected',
    'mismatched_native_link_inputs=rejected',
    'tampered_native_toolchain_lock=rejected',
    'mismatched_native_toolchain_lock=rejected',
    'malformed_native_link_manifest=rejected',
    "workflow.event -ne 'workflow_dispatch'",
    "workflow.name -ne 'Independent offline reproduction'",
    "runner.environment -ne 'github-hosted'",
    "attestation verify",
    '--signer-workflow',
    '--source-digest',
    '--deny-self-hosted-runners',
    "artifact.file -ne 'kilogram-offline.exe'",
    'verify-kilogram-offline-reproducibility-record.ps1',
    'SelfTest does not accept external evidence parameters.'
)) {
    if ($verifier.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Independent-builder verifier boundary is missing: $required"
    }
}
if ($verifier.IndexOf('SkipAttestationForProduction', [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
    throw 'Production independent-builder verification must not expose an attestation bypass.'
}

foreach ($required in @(
    'kilogram-reproducible-linker.ps1',
    'Get-KilogramBundledLldIdentity',
    'New-KilogramReproducibleRustFlags',
    'Get-KilogramPinnedNativeToolchainIdentity',
    'Assert-KilogramNativeLinkInputManifestMatchesLock',
    'Write-KilogramNativeLinkInputManifest',
    '@($nativeToolchainIdentity.rustc_arguments)',
    'Normalize-KilogramPeReproducibilityMetadata -Path $artifact',
    'verify-kilogram-offline-boundary.ps1',
    'format_version = 5',
    "manifest_format = 'sha256-bytes-logical-path-v1'",
    'NATIVE-LINK-INPUTS.sha256',
    'WINDOWS-NATIVE-LINK-INPUTS.lock',
    "pe_metadata_normalization = 'coff-and-debug-timestamps-plus-codeview-guid-zeroed-v1'",
    "blake3_codegen = 'pure-rust-intrinsics'",
    'sha256 = $linkerIdentity.sha256',
    'The bundled LLD linker changed during the two clean builds.'
)) {
    if ($localBuilder.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Local reproducible builder boundary is missing: $required"
    }
}

foreach ($required in @(
    'rustc --print sysroot',
    'rust-lld.exe',
    "mode = 'rust-toolchain-bundled-lld'",
    "source = 'rustc-sysroot-target-bin'",
    "flavor = 'lld-link'",
    "reproducibility_flag = '/Brepro'",
    'linker-flavor=lld-link',
    'link-arg=/Brepro',
    'function Normalize-KilogramPeReproducibilityMetadata',
    'function Assert-KilogramPeReproducibilityMetadataNormalized',
    'function Write-KilogramNativeLinkInputManifest',
    'function Assert-KilogramNativeLinkInputManifest',
    'function Get-KilogramPinnedNativeToolchainIdentity',
    'function Assert-KilogramNativeLinkInputManifestMatchesLock',
    "mode = 'repository-hash-locked-installed-libraries'",
    "selection = 'explicit-final-rustc-native-search-paths'",
    'LLD consumed an unclassified native library',
    'Refusing to normalize a PE image with a non-zero checksum',
    'Refusing to normalize an Authenticode-bearing PE image',
    'Get-FileHash -LiteralPath $path -Algorithm SHA256'
)) {
    if ($linkerHelper.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Bundled LLD helper boundary is missing: $required"
    }
}
if ($linkerHelper.IndexOf('linker-features=+lld', [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
    throw 'Bundled LLD helper must not use the unstable linker-features flag.'
}

foreach ($required in @(
    'blake3 pure feature',
    'blake3 feature "pure"$',
    'blake3_codegen=pure-rust-intrinsics',
    'linked_blake3_native_objects=false',
    '--offline --locked',
    '--target x86_64-pc-windows-msvc'
)) {
    if ($offlineBoundary.IndexOf($required, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Offline pure-Rust code-generation boundary is missing: $required"
    }
}

Write-Output 'independent_builder_boundary=verified'
Write-Output 'trigger=workflow-dispatch-only'
Write-Output 'builder=github-hosted-windows'
Write-Output 'artifact=kilogram-offline.exe'
Write-Output 'linker=rust-toolchain-bundled-lld'
Write-Output 'blake3_codegen=pure-rust-intrinsics'
Write-Output 'attestation=required-for-production-verification'
Write-Output 'automatic_release=false'
Write-Output 'local_zip=false'
