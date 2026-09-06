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
    if ($builderRecord.format_version -ne 1 -or
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
        $builderRecord.path_remap -ne '<BUILD_ROOT>=Z:/kilogram-source' -or
        $builderRecord.linker_reproducibility_flag -ne '/Brepro' -or
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
    if ($builderRecord.artifact.file -ne 'kilogram-offline.exe') {
        throw 'Independent builder record contains an unexpected artifact name.'
    }

    foreach ($hashField in @(
        @{ Value = [string]$localRecord.build_a.sha256; Name = 'local.build_a.sha256' },
        @{ Value = [string]$localRecord.build_b.sha256; Name = 'local.build_b.sha256' },
        @{ Value = [string]$builderRecord.expected_local_sha256; Name = 'expected_local_sha256' },
        @{ Value = [string]$builderRecord.artifact.sha256; Name = 'artifact.sha256' }
    )) {
        Assert-Sha256 $hashField.Value $hashField.Name
    }

    $actualHash = Get-Sha256 $artifact
    $actualBytes = (Get-Item -LiteralPath $artifact).Length
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
        $gh = Get-Command gh -ErrorAction SilentlyContinue
        if ($null -eq $gh) {
            throw 'GitHub CLI (gh) is required to verify signed provenance.'
        }
        $workflowIdentity = "$ExpectedRepository/.github/workflows/independent-offline-reproduction.yml"
        foreach ($subject in @($artifact, $builderRecordPathResolved)) {
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
    Write-Output "attestation_verified=$(((-not $SkipAttestation)).ToString().ToLowerInvariant())"
}

function Invoke-SelfTest {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ("kilogram-independent-builder-" + [Guid]::NewGuid().ToString('N'))
    $local = Join-Path $root 'local'
    $independent = Join-Path $root 'independent'
    New-Item -ItemType Directory -Path $local, $independent | Out-Null
    try {
        $revision = '1234567890abcdef1234567890abcdef12345678'
        $artifactBytes = [System.Text.Encoding]::UTF8.GetBytes('kilogram independent builder self-test')
        $buildA = Join-Path $local 'kilogram-offline-build-a.exe'
        $buildB = Join-Path $local 'kilogram-offline-build-b.exe'
        $artifact = Join-Path $independent 'kilogram-offline.exe'
        [System.IO.File]::WriteAllBytes($buildA, $artifactBytes)
        [System.IO.File]::WriteAllBytes($buildB, $artifactBytes)
        [System.IO.File]::WriteAllBytes($artifact, $artifactBytes)
        [System.IO.File]::WriteAllText((Join-Path $local 'SOURCE-MANIFEST.sha256'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText((Join-Path $local 'Cargo.lock'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        [System.IO.File]::WriteAllText((Join-Path $local 'rust-toolchain.toml'), 'self-test', [System.Text.UTF8Encoding]::new($false))
        $hash = Get-Sha256 $artifact
        $length = (Get-Item -LiteralPath $artifact).Length
        $localRecord = [ordered]@{
            format_version = 1
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
            path_remap = '<BUILD_ROOT>=Z:/kilogram-source'
            linker_reproducibility_flag = '/Brepro'
            network_surface_compiled = $false
            runtime_surface_compiled = $false
            build_a = [ordered]@{ file = 'kilogram-offline-build-a.exe'; sha256 = $hash; bytes = $length }
            build_b = [ordered]@{ file = 'kilogram-offline-build-b.exe'; sha256 = $hash; bytes = $length }
        }
        [System.IO.File]::WriteAllText((Join-Path $local 'REPRODUCIBILITY.json'), ($localRecord | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

        $builderRecord = [ordered]@{
            format_version = 1
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
            path_remap = '<BUILD_ROOT>=Z:/kilogram-source'
            linker_reproducibility_flag = '/Brepro'
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
