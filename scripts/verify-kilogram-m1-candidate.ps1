[CmdletBinding()]
param(
    [string] $RecordPath,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
if ([string]::IsNullOrWhiteSpace($RecordPath)) {
    $RecordPath = Join-Path $workspace 'M1-CANDIDATE.json'
}

$expectedFieldRevision = 'ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae'
$expectedIndependentRevision = '4e9054ab2ccc6a4c542fb37d486b70e53027dd08'
$expectedArtifactSha256 = 'd9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4'
$expectedFieldManifestSha256 = 'a67580165a166e22d6c52b52e93d29c562f1bb6289e2497e220fe3ff1e788a3d'
$expectedIndependentManifestSha256 = '4ada929ce28d5e152923f746d339d168c5620767f8270443e2a63baf4553e294'
$expectedFieldPaths = @(
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    'apps/kilogram-bootstrap',
    'apps/kilogram-cli',
    'apps/kilogram-ticket-store',
    'apps/kilogram-windows',
    'crates'
)
$expectedIndependentPaths = @(
    '.gitattributes',
    '.github/workflows/independent-offline-reproduction.yml',
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    'WINDOWS-NATIVE-LINK-INPUTS.lock',
    'apps/kilogram-offline',
    'crates',
    'scripts/cargo-resource-policy.ps1',
    'scripts/build-kilogram-offline-reproducible.ps1',
    'scripts/kilogram-reproducible-linker.ps1',
    'scripts/package-kilogram-offline.ps1',
    'scripts/verify-kilogram-independent-builder-boundary.ps1',
    'scripts/verify-kilogram-independent-builder.ps1',
    'scripts/verify-kilogram-offline-boundary.ps1',
    'scripts/verify-kilogram-offline-reproducibility-record.ps1'
)
$expectedResidualRisks = @(
    'volunteer-providers-not-operator-independent',
    'sybil-resistance-not-implemented',
    'access-correlation-not-hidden',
    'windows-only-field-evidence',
    'not-a-public-security-release'
)
$candidateControlPaths = @(
    'M1-CANDIDATE.json',
    'scripts/verify-kilogram-m1-candidate.ps1',
    'scripts/verify-kilogram-m1-candidate-boundary.ps1',
    'docs/RFC-0104-m1-candidate-evidence-composition.md'
)

function Assert-ExactProperties {
    param(
        [Parameter(Mandatory = $true)] [object] $Object,
        [Parameter(Mandatory = $true)] [string[]] $Expected,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    $actual = @($Object.PSObject.Properties.Name)
    [Array]::Sort($actual, [StringComparer]::Ordinal)
    $wanted = @($Expected)
    [Array]::Sort($wanted, [StringComparer]::Ordinal)
    if ($actual.Count -ne $wanted.Count) {
        throw "$Name has an unexpected property count"
    }
    for ($index = 0; $index -lt $wanted.Count; $index++) {
        if ([string]$actual[$index] -cne [string]$wanted[$index]) {
            throw "$Name properties do not match the fail-closed schema"
        }
    }
}

function Assert-ExactStringArray {
    param(
        [Parameter(Mandatory = $true)] [object[]] $Actual,
        [Parameter(Mandatory = $true)] [string[]] $Expected,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    $values = @($Actual | ForEach-Object { [string]$_ })
    if ($values.Count -ne $Expected.Count) {
        throw "$Name has an unexpected item count"
    }
    for ($index = 0; $index -lt $Expected.Count; $index++) {
        if ($values[$index] -cne $Expected[$index]) {
            throw "$Name differs at item $index"
        }
    }
}

function Assert-CandidateRecord {
    param([Parameter(Mandatory = $true)] [object] $Record)

    Assert-ExactProperties $Record @(
        'format_version', 'milestone', 'status', 'scope', 'field_evidence',
        'independent_reproduction', 'stage_boundary', 'residual_risks'
    ) 'candidate record'
    if ([int]$Record.format_version -ne 1 -or
        [string]$Record.milestone -cne 'M0.9.81' -or
        [string]$Record.status -cne 'candidate' -or
        [string]$Record.scope -cne 'windows-technical-proof-of-concept') {
        throw 'candidate record identity is invalid'
    }

    $field = $Record.field_evidence
    Assert-ExactProperties $field @(
        'milestone', 'run_id', 'source_revision', 'result', 'verifier', 'runtime_surface'
    ) 'field evidence'
    if ([string]$field.milestone -cne 'M0.9.76' -or
        [string]$field.run_id -cne '20260920-001614' -or
        [string]$field.source_revision -cne $expectedFieldRevision -or
        [string]$field.result -cne 'verified' -or
        [string]$field.verifier -cne 'scripts/verify-kilogram-m0976-service-free-v2-evidence.ps1') {
        throw 'accepted service-free field evidence identity is invalid'
    }
    $runtimeSurface = $field.runtime_surface
    Assert-ExactProperties $runtimeSurface @('mode', 'manifest_sha256', 'file_count', 'paths') 'field runtime surface'
    if ([string]$runtimeSurface.mode -cne 'git-ls-tree-sha256-v1' -or
        [string]$runtimeSurface.manifest_sha256 -cne $expectedFieldManifestSha256 -or
        [int]$runtimeSurface.file_count -ne 83) {
        throw 'accepted field runtime surface identity is invalid'
    }
    Assert-ExactStringArray @($runtimeSurface.paths) $expectedFieldPaths 'field runtime paths'

    $independent = $Record.independent_reproduction
    Assert-ExactProperties $independent @(
        'milestone', 'source_revision', 'repository', 'workflow', 'workflow_run_id',
        'attestation_id', 'result', 'artifact', 'protected_surface'
    ) 'independent reproduction'
    if ([string]$independent.milestone -cne 'M0.9.80' -or
        [string]$independent.source_revision -cne $expectedIndependentRevision -or
        [string]$independent.repository -cne 'gugglegum/kilogram-messenger' -or
        [string]$independent.workflow -cne '.github/workflows/independent-offline-reproduction.yml' -or
        [string]$independent.workflow_run_id -cne '35474774356' -or
        [string]$independent.attestation_id -cne '48691093' -or
        [string]$independent.result -cne 'verified') {
        throw 'accepted independent reproduction identity is invalid'
    }
    $artifact = $independent.artifact
    Assert-ExactProperties $artifact @('file', 'sha256', 'bytes') 'independent artifact'
    if ([string]$artifact.file -cne 'kilogram-offline.exe' -or
        [string]$artifact.sha256 -cne $expectedArtifactSha256 -or
        [int64]$artifact.bytes -ne 2432000) {
        throw 'accepted independent artifact identity is invalid'
    }
    $independentSurface = $independent.protected_surface
    Assert-ExactProperties $independentSurface @('mode', 'manifest_sha256', 'file_count', 'paths') 'independent protected surface'
    if ([string]$independentSurface.mode -cne 'git-ls-tree-sha256-v1' -or
        [string]$independentSurface.manifest_sha256 -cne $expectedIndependentManifestSha256 -or
        [int]$independentSurface.file_count -ne 63) {
        throw 'accepted independent protected surface identity is invalid'
    }
    Assert-ExactStringArray @($independentSurface.paths) $expectedIndependentPaths 'independent protected paths'

    $boundary = $Record.stage_boundary
    Assert-ExactProperties $boundary @(
        'network_retest_required', 'network_executed', 'release_build',
        'archive_created', 'background_service_created', 'git_tag_created'
    ) 'stage boundary'
    foreach ($name in @(
        'network_retest_required', 'network_executed', 'release_build',
        'archive_created', 'background_service_created', 'git_tag_created'
    )) {
        if ([bool]$boundary.$name) {
            throw "M0.9.81 stage boundary must keep $name false"
        }
    }
    Assert-ExactStringArray @($Record.residual_risks) $expectedResidualRisks 'residual risks'
}

function Get-Sha256Text {
    param([Parameter(Mandatory = $true)] [string] $Text)

    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Text)
        $hash = $sha.ComputeHash($bytes)
        return (($hash | ForEach-Object { $_.ToString('x2') }) -join '')
    }
    finally {
        $sha.Dispose()
    }
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)] [string[]] $Arguments,
        [int[]] $AllowedExitCodes = @(0)
    )

    $output = @(& git -C $workspace @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    if ($exitCode -notin $AllowedExitCodes) {
        throw "git failed ($exitCode): git $($Arguments -join ' ')`n$($output -join [Environment]::NewLine)"
    }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = @($output | ForEach-Object { [string]$_ }) }
}

function Assert-CommitAvailable {
    param([Parameter(Mandatory = $true)] [string] $Revision)

    if ($Revision -cnotmatch '^[0-9a-f]{40}$') {
        throw "candidate evidence revision is not a full lowercase Git commit: $Revision"
    }
    Invoke-Git @('cat-file', '-e', "$Revision`^{commit}") | Out-Null
}

function Assert-Ancestor {
    param(
        [Parameter(Mandatory = $true)] [string] $Revision,
        [Parameter(Mandatory = $true)] [string] $Head
    )

    $result = Invoke-Git @('merge-base', '--is-ancestor', $Revision, $Head) @(0, 1)
    if ($result.ExitCode -ne 0) {
        throw "accepted evidence revision is not an ancestor of the candidate: $Revision"
    }
}

function Get-GitSurfaceIdentity {
    param(
        [Parameter(Mandatory = $true)] [string] $Revision,
        [Parameter(Mandatory = $true)] [string[]] $Paths
    )

    $result = Invoke-Git (@('ls-tree', '-r', '--full-tree', $Revision, '--') + $Paths)
    $lines = @($result.Output)
    [Array]::Sort($lines, [StringComparer]::Ordinal)
    $canonical = ($lines -join "`n") + "`n"
    return [pscustomobject]@{
        Sha256 = Get-Sha256Text $canonical
        FileCount = $lines.Count
    }
}

function Assert-ProtectedSurface {
    param(
        [Parameter(Mandatory = $true)] [string] $Name,
        [Parameter(Mandatory = $true)] [string] $BaselineRevision,
        [Parameter(Mandatory = $true)] [string] $Head,
        [Parameter(Mandatory = $true)] [string[]] $Paths,
        [Parameter(Mandatory = $true)] [string] $ExpectedSha256,
        [Parameter(Mandatory = $true)] [int] $ExpectedFileCount
    )

    $baseline = Get-GitSurfaceIdentity $BaselineRevision $Paths
    if ($baseline.Sha256 -cne $ExpectedSha256 -or $baseline.FileCount -ne $ExpectedFileCount) {
        throw "$Name baseline manifest does not match the committed M1 candidate record"
    }
    $current = Get-GitSurfaceIdentity $Head $Paths
    if ($current.Sha256 -cne $baseline.Sha256 -or $current.FileCount -ne $baseline.FileCount) {
        $changed = Invoke-Git (@('diff', '--name-only', $BaselineRevision, $Head, '--') + $Paths)
        throw "$Name changed after accepted evidence:`n$($changed.Output -join [Environment]::NewLine)"
    }
    $dirty = Invoke-Git (@('status', '--porcelain', '--untracked-files=all', '--') + $Paths)
    if ($dirty.Output.Count -ne 0) {
        throw "$Name has uncommitted or untracked changes:`n$($dirty.Output -join [Environment]::NewLine)"
    }
}

function Assert-CandidateControlCommitted {
    $tracked = Invoke-Git (@('ls-files', '--error-unmatch', '--') + $candidateControlPaths)
    if ($tracked.Output.Count -ne $candidateControlPaths.Count) {
        throw 'M1 candidate control surface is not completely tracked'
    }
    $dirty = Invoke-Git (@('status', '--porcelain', '--untracked-files=all', '--') + $candidateControlPaths)
    if ($dirty.Output.Count -ne 0) {
        throw "M1 candidate control surface is not committed cleanly:`n$($dirty.Output -join [Environment]::NewLine)"
    }
}

function Assert-Rejected {
    param(
        [Parameter(Mandatory = $true)] [scriptblock] $Action,
        [Parameter(Mandatory = $true)] [string] $Name
    )

    $rejected = $false
    try {
        & $Action
    }
    catch {
        $rejected = $true
    }
    if (-not $rejected) {
        throw "M1 candidate self-test accepted $Name"
    }
    Write-Output "$Name=rejected"
}

$resolvedRecord = [IO.Path]::GetFullPath($RecordPath)
if (-not (Test-Path -LiteralPath $resolvedRecord -PathType Leaf)) {
    throw "M1 candidate record is missing: $resolvedRecord"
}
$recordJson = Get-Content -LiteralPath $resolvedRecord -Raw
$record = $recordJson | ConvertFrom-Json
Assert-CandidateRecord $record

if ($SelfTest) {
    $badField = $recordJson | ConvertFrom-Json
    $badField.field_evidence.source_revision = '0000000000000000000000000000000000000000'
    Assert-Rejected { Assert-CandidateRecord $badField } 'tampered_field_revision'

    $badArtifact = $recordJson | ConvertFrom-Json
    $badArtifact.independent_reproduction.artifact.sha256 = '0000000000000000000000000000000000000000000000000000000000000000'
    Assert-Rejected { Assert-CandidateRecord $badArtifact } 'tampered_artifact_sha256'

    $badPath = $recordJson | ConvertFrom-Json
    $badPath.field_evidence.runtime_surface.paths[7] = 'crates/kilogram-protocol'
    Assert-Rejected { Assert-CandidateRecord $badPath } 'narrowed_runtime_surface'

    $badRisk = $recordJson | ConvertFrom-Json
    $badRisk.residual_risks = @($badRisk.residual_risks | Select-Object -First 4)
    Assert-Rejected { Assert-CandidateRecord $badRisk } 'omitted_residual_risk'

    Write-Output 'm1_candidate_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'release_build=false'
    Write-Output 'archive_created=false'
    exit 0
}

$head = (Invoke-Git @('rev-parse', 'HEAD')).Output[0].Trim()
Assert-CandidateControlCommitted
Assert-CommitAvailable $expectedFieldRevision
Assert-CommitAvailable $expectedIndependentRevision
Assert-Ancestor $expectedFieldRevision $head
Assert-Ancestor $expectedIndependentRevision $head
Assert-ProtectedSurface `
    -Name 'field-tested runtime/protocol surface' `
    -BaselineRevision $expectedFieldRevision `
    -Head $head `
    -Paths $expectedFieldPaths `
    -ExpectedSha256 $expectedFieldManifestSha256 `
    -ExpectedFileCount 83
Assert-ProtectedSurface `
    -Name 'independently reproduced offline artifact surface' `
    -BaselineRevision $expectedIndependentRevision `
    -Head $head `
    -Paths $expectedIndependentPaths `
    -ExpectedSha256 $expectedIndependentManifestSha256 `
    -ExpectedFileCount 63

$fieldRfc = Get-Content -LiteralPath (Join-Path $workspace 'docs\RFC-0099-service-free-v2-field-run.md') -Raw
foreach ($required in @('20260920-001614', $expectedFieldRevision, 'result=verified')) {
    if ($fieldRfc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "accepted field RFC is missing '$required'"
    }
}
$reproductionRfc = Get-Content -LiteralPath (Join-Path $workspace 'docs\RFC-0103-canonical-cargo-registry-path-remapping.md') -Raw
foreach ($required in @('35474774356', '48691093', $expectedArtifactSha256)) {
    if ($reproductionRfc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "accepted reproduction RFC is missing '$required'"
    }
}

foreach ($gate in @(
    [pscustomobject]@{ File = 'verify-kilogram-m0976-service-free-v2-kit-boundary.ps1'; Marker = 'm0976_service_free_v2_kit_boundary=verified' },
    [pscustomobject]@{ File = 'verify-kilogram-independent-builder-boundary.ps1'; Marker = 'independent_builder_boundary=verified' },
    [pscustomobject]@{ File = 'verify-kilogram-m1-acceptance-kit-boundary.ps1'; Marker = 'm1_acceptance_kit_boundary=verified' }
)) {
    $output = @(& (Join-Path $PSScriptRoot $gate.File) 2>&1)
    if ($gate.Marker -notin @($output | ForEach-Object { [string]$_ })) {
        throw "M1 candidate prerequisite gate failed: $($gate.File)`n$($output -join [Environment]::NewLine)"
    }
}

Write-Output 'm1_candidate_record=verified'
Write-Output "candidate_revision=$head"
Write-Output "field_evidence_revision=$expectedFieldRevision"
Write-Output 'field_evidence_result=verified'
Write-Output 'runtime_protocol_surface=unchanged'
Write-Output "independent_reproduction_revision=$expectedIndependentRevision"
Write-Output "independent_artifact_sha256=$expectedArtifactSha256"
Write-Output 'independent_attestation=48691093'
Write-Output 'independent_artifact_surface=unchanged'
Write-Output 'network_retest_required=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'm1_candidate_status=verified'
