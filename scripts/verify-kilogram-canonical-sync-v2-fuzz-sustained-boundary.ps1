[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$sustainedPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json'
$campaignPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json'
$toolchainPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json'
$smokePath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-SMOKE.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$activeTransportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0203-m1176-sustained-fuzz-campaign.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-SustainedContract {
    param(
        [object]$Sustained,
        [object]$Campaign,
        [object]$Toolchain,
        [object]$Smoke,
        [object]$Migration,
        [object]$Transport,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Wire,
        [string]$ActiveTransport,
        [string]$Cli,
        [string]$Rfc
    )

    $campaignStatus = 'instrumented-sustained-and-coverage-verified-field-evidence-pending'
    $aggregateStatus = 'canonical-sync-v2-instrumented-sustained-verified-field-evidence-pending'
    $sourceRevision = 'eaf3d75bb16ce42c8368017a68c6f4183d15ab9f'

    if ([UInt64]$Sustained.schema_version -ne 1 -or
        [string]$Sustained.milestone -cne 'M1.1.76' -or
        [string]$Sustained.status -cne $campaignStatus -or
        [string]$Sustained.campaign_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json' -or
        [string]$Sustained.toolchain_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json' -or
        [string]$Sustained.smoke_evidence_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-SMOKE.json' -or
        [string]$Sustained.target -cne 'canonical_sync_v2_decoders') {
        throw 'sustained campaign identity changed'
    }
    if ([string]$Sustained.source.revision_before -cne $sourceRevision -or
        [string]$Sustained.source.revision_after -cne $sourceRevision -or
        -not [bool]$Sustained.source.worktree_clean_before -or
        -not [bool]$Sustained.source.worktree_clean_after) {
        throw 'sustained campaign source was not clean and unchanged'
    }
    if ([string]$Sustained.host.os -cne 'windows' -or
        [string]$Sustained.host.target_triple -cne 'x86_64-pc-windows-msvc' -or
        [string]$Sustained.toolchain.rustc_release -cne '1.101.0-nightly' -or
        [string]$Sustained.toolchain.rustc_commit_hash -cne
            '5d8fec0669a64607db9ccbf663e7c23125bf4748' -or
        [string]$Sustained.toolchain.rustc_commit_date -cne '2026-10-16' -or
        [string]$Sustained.toolchain.llvm_version -cne '23.1.4' -or
        [string]$Sustained.toolchain.cargo_fuzz_version -cne '0.13.2' -or
        [string]$Sustained.toolchain.sanitizer -cne 'address') {
        throw 'sustained campaign toolchain or host changed'
    }

    $expectedStartedAt = [DateTime]::Parse(
        '2026-10-17T12:02:20.6781480Z',
        [System.Globalization.CultureInfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::RoundtripKind
    )
    if ([string]$Sustained.run.mode -cne 'sustained' -or
        ([DateTime]$Sustained.run.started_at_utc).ToUniversalTime() -ne
            $expectedStartedAt.ToUniversalTime() -or
        [UInt64]$Sustained.run.requested_seconds -ne 28800 -or
        [UInt64]$Sustained.run.elapsed_seconds -lt 28800 -or
        [UInt64]$Sustained.run.executions -lt 1000000 -or
        [UInt64]$Sustained.run.initial_seed_count -ne 20 -or
        [UInt64]$Sustained.run.final_corpus_files -lt 20 -or
        [UInt64]$Sustained.run.artifact_files -ne 0 -or
        [UInt64]$Sustained.run.exit_code -ne 0 -or
        [UInt64]$Sustained.run.workers -ne 1 -or
        [UInt64]$Sustained.run.jobs -ne 1 -or
        -not [bool]$Sustained.run.coverage_generated -or
        -not [bool]$Sustained.run.sustained_gate_satisfied) {
        throw 'sustained result does not satisfy the closed campaign contract'
    }
    if ([UInt64]$Sustained.run.elapsed_seconds -ne 28801 -or
        [UInt64]$Sustained.run.executions -ne 469853771 -or
        [UInt64]$Sustained.run.final_corpus_files -ne 1043) {
        throw 'sustained campaign observation changed'
    }

    $expectedEvidence = @{
        record = @('.tmp/fuzz/evidence/20261017T120220Z-canonical_sync_v2_decoders.json', 1185,
            'c831227154c9ed630bd1e38246bcb7a05d322918ab5e3ad6987179f747d5b981')
        run_log = @('.tmp/fuzz/evidence/20261017T120220Z-canonical_sync_v2_decoders.log', 773230,
            'bcab77531605db17d1b27caf34c162907db1ddab49c010d4ddf2704555013747')
        coverage_log = @('.tmp/fuzz/evidence/20261017T120220Z-canonical_sync_v2_decoders-coverage.log', 592266,
            'd20d1b78c3dd2e7994b261dacf38f431d997d6e5a515156bc0ecfd1bd338a778')
        coverage_profile = @('fuzz/coverage/canonical_sync_v2_decoders/coverage.profdata', 230688,
            '34ca49d1a3d4b7ba5cc91969d823527f9370de41282761a0fb923fb87c710a92')
    }
    foreach ($name in @('record', 'run_log', 'coverage_log', 'coverage_profile')) {
        $actual = $Sustained.evidence.$name
        $expected = $expectedEvidence[$name]
        if ([string]$actual.path -cne [string]$expected[0] -or
            [UInt64]$actual.bytes -ne [UInt64]$expected[1] -or
            [string]$actual.sha256 -cne [string]$expected[2]) {
            throw "sustained evidence metadata changed: $name"
        }
    }
    if ([string]$Sustained.coverage_profile_validation.tool -cne 'llvm-profdata show' -or
        [string]$Sustained.coverage_profile_validation.instrumentation_level -cne 'Front-end' -or
        [UInt64]$Sustained.coverage_profile_validation.total_functions -ne 888 -or
        [UInt64]$Sustained.coverage_profile_validation.maximum_function_count -ne 204308962 -or
        [UInt64]$Sustained.coverage_profile_validation.maximum_internal_block_count -ne 10543038 -or
        [UInt64]$Sustained.coverage_profile_validation.total_blocks -ne 2010 -or
        [UInt64]$Sustained.coverage_profile_validation.total_count -ne 885327009) {
        throw 'sustained coverage profile validation changed'
    }
    if ([bool]$Sustained.activation.active_transport_authorized -or
        [bool]$Sustained.activation.candidate_identifier_registered -or
        [bool]$Sustained.activation.candidate_identifier_dialed -or
        [bool]$Sustained.activation.active_runtime_changed -or
        [string]$Sustained.next_gate -cne 'fresh-hosted-and-field-evidence') {
        throw 'sustained evidence activated production transport or skipped field evidence'
    }

    if ([string]$Campaign.milestone -cne 'M1.1.76' -or
        [string]$Campaign.status -cne $campaignStatus -or
        [string]$Campaign.engine.sustained_evidence_manifest -cne
            'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json' -or
        -not [bool]$Campaign.observed.harness_compiled_with_libfuzzer -or
        -not [bool]$Campaign.observed.smoke_campaign_completed -or
        -not [bool]$Campaign.observed.sustained_campaign_completed -or
        -not [bool]$Campaign.observed.coverage_report_generated -or
        [UInt64]$Campaign.observed.executions -ne 469853771 -or
        [UInt64]$Campaign.observed.wall_clock_seconds -ne 28801) {
        throw 'campaign manifest does not bind the accepted sustained result'
    }
    if ([UInt64]$Campaign.campaign_contract.minimum_wall_clock_seconds -ne 28800 -or
        [UInt64]$Campaign.campaign_contract.minimum_executions -ne 1000000 -or
        [UInt64]$Campaign.campaign_contract.workers -ne 1 -or
        [UInt64]$Campaign.campaign_contract.jobs -ne 1 -or
        -not [bool]$Campaign.campaign_contract.coverage_report_required) {
        throw 'sustained campaign contract was weakened'
    }
    Assert-ExactStrings @($Campaign.remaining_m11_gates) @(
        'fresh-hosted-and-field-evidence'
    ) 'remaining campaign gates'

    if ([string]$Toolchain.milestone -cne 'M1.1.74' -or
        [string]$Toolchain.status -cne 'pinned-instrumented-fuzz-toolchain-verified' -or
        [bool]$Toolchain.observed.smoke_campaign_completed -or
        [bool]$Toolchain.observed.sustained_campaign_completed) {
        throw 'historical toolchain build record changed or was rewritten'
    }
    if ([string]$Smoke.milestone -cne 'M1.1.75' -or
        [string]$Smoke.status -cne 'instrumented-smoke-and-coverage-verified-sustained-pending' -or
        -not [bool]$Smoke.run.smoke_gate_satisfied -or
        [bool]$Smoke.run.sustained_gate_satisfied) {
        throw 'historical smoke evidence changed or was rewritten'
    }

    if ([string]$Migration.milestone -cne 'M1.1.76' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.history_compatible_successor.coverage_guided_fuzz_harness -cne
            'instrumented-sustained-verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration sustained state changed'
    }
    $harnessRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'coverage-guided-fuzz-harness'
    })
    $sustainedRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'sustained-coverage-guided-fuzzing'
    })
    $fieldRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'fresh-hosted-and-field-evidence'
    })
    if ($harnessRequirement.Count -ne 1 -or
        [string]$harnessRequirement[0].status -cne 'instrumented-smoke-verified' -or
        $sustainedRequirement.Count -ne 1 -or
        [string]$sustainedRequirement[0].status -cne 'verified' -or
        $fieldRequirement.Count -ne 1 -or
        [string]$fieldRequirement[0].status -cne 'pending') {
        throw 'sustained and field-evidence gates were conflated'
    }
    if ([string]$Transport.milestone -cne 'M1.1.76' -or
        [string]$Transport.status -cne $aggregateStatus -or
        -not [bool]$Transport.history_compatible_successor.instrumented_fuzz_smoke -or
        -not [bool]$Transport.history_compatible_successor.sustained_fuzz_campaign -or
        [bool]$Transport.history_compatible_successor.registered -or
        [bool]$Transport.history_compatible_successor.dialed -or
        [bool]$Transport.scope.network_executed) {
        throw 'transport sustained state changed'
    }
    Assert-ExactStrings @($Transport.remaining) @(
        'fresh-hosted-and-field-evidence'
    ) 'transport remaining work'
    if ([string]$Compatibility.milestone -cne 'M1.1.76' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.instrumented_fuzz_smoke_verified -or
        -not [bool]$Compatibility.required_successor.sustained_fuzz_campaign_verified -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility sustained state changed'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    $requiredArtifacts = @(
        'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json',
        'scripts/verify-kilogram-canonical-sync-v2-fuzz-sustained-boundary.ps1',
        'docs/RFC-0203-m1176-sustained-fuzz-campaign.md'
    )
    if ([string]$Coverage.milestone -cne 'M1.1.76' -or
        [string]$Coverage.status -cne $aggregateStatus -or
        $surface.Count -ne 1 -or
        'sustained-fuzz-campaign' -cnotin @($surface[0].covered)) {
        throw 'canonical coverage sustained state changed'
    }
    foreach ($artifact in $requiredArtifacts) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing sustained artifact '$artifact'"
        }
    }
    Assert-ExactStrings @($surface[0].remaining) @(
        'fresh-hosted-and-field-evidence'
    ) 'coverage remaining work'

    foreach ($inactiveSource in @($Wire, $ActiveTransport, $Cli)) {
        if ($inactiveSource.Contains('CANONICAL-SYNC-V2-FUZZ-SUSTAINED') -or
            $inactiveSource.Contains('sustained_fuzz_campaign')) {
            throw 'sustained evidence leaked into active wire, transport, or CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    Assert-ContainsAll $normalizedRfc @(
        '28,801 seconds',
        '469,853,771',
        'generated failure artifacts: 0',
        'does not require ignored raw evidence to exist on every clone',
        'fresh hosted and physical-device evidence remains pending',
        'does not register or dial `kilogram/m1/sync/cbor/2`'
    ) 'sustained campaign RFC'
}

$paths = @(
    $sustainedPath, $campaignPath, $toolchainPath, $smokePath, $migrationPath,
    $transportPath, $compatibilityPath, $coveragePath, $wirePath,
    $activeTransportPath, $cliPath, $rfcPath
)
foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "sustained campaign contract file is missing: $path"
    }
}

$sustained = Get-Content -LiteralPath $sustainedPath -Raw | ConvertFrom-Json
$campaign = Get-Content -LiteralPath $campaignPath -Raw | ConvertFrom-Json
$toolchain = Get-Content -LiteralPath $toolchainPath -Raw | ConvertFrom-Json
$smoke = Get-Content -LiteralPath $smokePath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transport = Get-Content -LiteralPath $transportPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$wire = Get-Content -LiteralPath $wirePath -Raw
$activeTransport = Get-Content -LiteralPath $activeTransportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

$arguments = @(
    $campaign, $toolchain, $smoke, $migration, $transport, $compatibility,
    $coverage, $wire, $activeTransport, $cli, $rfc
)
Assert-SustainedContract $sustained @arguments

if ($SelfTest) {
    $short = Copy-JsonObject $sustained
    $short.run.elapsed_seconds = 28799
    Assert-Rejected { Assert-SustainedContract $short @arguments } 'short_campaign'

    $artifact = Copy-JsonObject $sustained
    $artifact.run.artifact_files = 1
    Assert-Rejected { Assert-SustainedContract $artifact @arguments } 'generated_artifact'

    $sourceDirty = Copy-JsonObject $sustained
    $sourceDirty.source.worktree_clean_after = $false
    Assert-Rejected { Assert-SustainedContract $sourceDirty @arguments } 'dirty_source'

    $activated = Copy-JsonObject $sustained
    $activated.activation.candidate_identifier_dialed = $true
    Assert-Rejected { Assert-SustainedContract $activated @arguments } 'premature_activation'

    $aggregateDrift = Copy-JsonObject $migration
    $aggregateDrift.status = 'different-status'
    $driftArguments = @(
        $campaign, $toolchain, $smoke, $aggregateDrift, $transport,
        $compatibility, $coverage, $wire, $activeTransport, $cli, $rfc
    )
    Assert-Rejected {
        Assert-SustainedContract $sustained @driftArguments
    } 'aggregate_status_drift'

    Write-Output 'canonical_sync_v2_fuzz_sustained_self_test=passed'
}

$localEvidenceChecked = 0
foreach ($name in @('record', 'run_log', 'coverage_log', 'coverage_profile')) {
    $entry = $sustained.evidence.$name
    $relativePath = ([string]$entry.path).Replace('/', [System.IO.Path]::DirectorySeparatorChar)
    $path = Join-Path $workspace $relativePath
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ([UInt64]$item.Length -ne [UInt64]$entry.bytes -or
            $hash -cne [string]$entry.sha256) {
            throw "local ignored sustained evidence does not match the tracked digest: $name"
        }
        $localEvidenceChecked++
    }
}

Write-Output 'canonical_sync_v2_fuzz_sustained_boundary=verified'
Write-Output 'sustained_campaign_completed=true'
Write-Output 'sustained_executions=469853771'
Write-Output 'sustained_elapsed_seconds=28801'
Write-Output 'sustained_artifact_files=0'
Write-Output 'coverage_profile_validated=true'
Write-Output "local_ignored_evidence_checked=$localEvidenceChecked"
Write-Output 'fresh_hosted_and_field_evidence=pending'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'network_executed=false'
