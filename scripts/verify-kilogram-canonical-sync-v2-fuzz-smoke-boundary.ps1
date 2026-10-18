[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$smokePath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-SMOKE.json'
$campaignPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json'
$toolchainPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$runnerPath = Join-Path $PSScriptRoot 'invoke-kilogram-canonical-sync-v2-fuzz.ps1'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$activeTransportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0202-m1175-instrumented-fuzz-smoke.md'

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
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-SmokeContract {
    param(
        [object]$Smoke,
        [object]$Campaign,
        [object]$Toolchain,
        [object]$Migration,
        [object]$Transport,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Runner,
        [string]$Wire,
        [string]$ActiveTransport,
        [string]$Cli,
        [string]$Rfc
    )

    $aggregateStatus = 'canonical-sync-v2-ready-for-explicit-activation-decision'
    $smokeStatus = 'instrumented-smoke-and-coverage-verified-sustained-pending'
    $campaignStatus = 'instrumented-sustained-and-coverage-verified-field-evidence-pending'
    $sourceRevision = 'f9148093a7476e2d77c4a967b018c4b26a230225'

    if ([UInt64]$Smoke.schema_version -ne 1 -or
        [string]$Smoke.milestone -cne 'M1.1.75' -or
        [string]$Smoke.status -cne $smokeStatus -or
        [string]$Smoke.campaign_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json' -or
        [string]$Smoke.toolchain_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json' -or
        [string]$Smoke.target -cne 'canonical_sync_v2_decoders') {
        throw 'instrumented smoke identity changed'
    }
    if ([string]$Smoke.source.revision_before -cne $sourceRevision -or
        [string]$Smoke.source.revision_after -cne $sourceRevision -or
        -not [bool]$Smoke.source.worktree_clean_before -or
        -not [bool]$Smoke.source.worktree_clean_after) {
        throw 'instrumented smoke source was not clean and unchanged'
    }
    if ([string]$Smoke.host.os -cne 'windows' -or
        [string]$Smoke.host.target_triple -cne 'x86_64-pc-windows-msvc' -or
        [string]$Smoke.toolchain.rustc_release -cne '1.101.0-nightly' -or
        [string]$Smoke.toolchain.rustc_commit_hash -cne
            '5d8fec0669a64607db9ccbf663e7c23125bf4748' -or
        [string]$Smoke.toolchain.rustc_commit_date -cne '2026-10-16' -or
        [string]$Smoke.toolchain.llvm_version -cne '23.1.4' -or
        [string]$Smoke.toolchain.cargo_fuzz_version -cne '0.13.2' -or
        [string]$Smoke.toolchain.sanitizer -cne 'address') {
        throw 'instrumented smoke toolchain or host changed'
    }
    $expectedStartedAt = [DateTime]::Parse(
        '2026-10-17T11:00:00.1772545Z',
        [System.Globalization.CultureInfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::RoundtripKind
    )
    if ([string]$Smoke.run.mode -cne 'smoke' -or
        ([DateTime]$Smoke.run.started_at_utc).ToUniversalTime() -ne $expectedStartedAt.ToUniversalTime() -or
        [UInt64]$Smoke.run.requested_seconds -ne 300 -or
        [UInt64]$Smoke.run.elapsed_seconds -lt 300 -or
        [UInt64]$Smoke.run.executions -lt 1000000 -or
        [UInt64]$Smoke.run.initial_seed_count -ne 20 -or
        [UInt64]$Smoke.run.final_corpus_files -lt 20 -or
        [UInt64]$Smoke.run.artifact_files -ne 0 -or
        [UInt64]$Smoke.run.exit_code -ne 0 -or
        [UInt64]$Smoke.run.workers -ne 1 -or
        [UInt64]$Smoke.run.jobs -ne 1 -or
        -not [bool]$Smoke.run.coverage_generated -or
        -not [bool]$Smoke.run.smoke_gate_satisfied -or
        [bool]$Smoke.run.sustained_gate_satisfied) {
        throw 'instrumented smoke result does not satisfy the closed smoke contract'
    }
    if ([UInt64]$Smoke.run.elapsed_seconds -ne 301 -or
        [UInt64]$Smoke.run.executions -ne 5697050 -or
        [UInt64]$Smoke.run.final_corpus_files -ne 859) {
        throw 'instrumented smoke observation changed'
    }

    $expectedEvidence = @{
        record = @('.tmp/fuzz/evidence/20261017T110000Z-canonical_sync_v2_decoders.json', 1175,
            '60b5dbc862b914eef257c37749b478acce76f94379f6306432949c87f28dbf5b')
        run_log = @('.tmp/fuzz/evidence/20261017T110000Z-canonical_sync_v2_decoders.log', 566714,
            'd96ddd1c6f830c1f1a1b7fa36a61085694d8735d66ae0c54ba3db3bc3d248e50')
        coverage_log = @('.tmp/fuzz/evidence/20261017T110000Z-canonical_sync_v2_decoders-coverage.log', 498484,
            '61b97c3560359699742e3d6c946cad0e54d9ff1c5c9e032225dc150f580d6829')
        coverage_profile = @('fuzz/coverage/canonical_sync_v2_decoders/coverage.profdata', 226312,
            '16e801dbd95013ac16e4bc413723a65ef3d243b0552c5160f5013614d253a723')
    }
    foreach ($name in @('record', 'run_log', 'coverage_log', 'coverage_profile')) {
        $actual = $Smoke.evidence.$name
        $expected = $expectedEvidence[$name]
        if ([string]$actual.path -cne [string]$expected[0] -or
            [UInt64]$actual.bytes -ne [UInt64]$expected[1] -or
            [string]$actual.sha256 -cne [string]$expected[2]) {
            throw "instrumented smoke evidence metadata changed: $name"
        }
    }
    if ([string]$Smoke.coverage_profile_validation.tool -cne 'llvm-profdata show' -or
        [string]$Smoke.coverage_profile_validation.instrumentation_level -cne 'Front-end' -or
        [UInt64]$Smoke.coverage_profile_validation.total_functions -ne 871 -or
        [UInt64]$Smoke.coverage_profile_validation.maximum_function_count -ne 193715260 -or
        [UInt64]$Smoke.coverage_profile_validation.maximum_internal_block_count -ne 9996470 -or
        [UInt64]$Smoke.coverage_profile_validation.total_blocks -ne 1986 -or
        [UInt64]$Smoke.coverage_profile_validation.total_count -ne 838948912) {
        throw 'instrumented smoke coverage profile validation changed'
    }
    if ([bool]$Smoke.activation.active_transport_authorized -or
        [bool]$Smoke.activation.candidate_identifier_registered -or
        [bool]$Smoke.activation.candidate_identifier_dialed -or
        [bool]$Smoke.activation.active_runtime_changed -or
        [string]$Smoke.next_gate -cne 'sustained-instrumented-campaign') {
        throw 'instrumented smoke activated production transport or skipped the next gate'
    }

    if ([string]$Campaign.milestone -cne 'M1.1.76' -or
        [string]$Campaign.status -cne $campaignStatus -or
        [string]$Campaign.engine.smoke_evidence_manifest -cne
            'CANONICAL-SYNC-V2-FUZZ-SMOKE.json' -or
        [string]$Campaign.engine.sustained_evidence_manifest -cne
            'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json' -or
        -not [bool]$Campaign.observed.harness_compiled_with_libfuzzer -or
        -not [bool]$Campaign.observed.smoke_campaign_completed -or
        -not [bool]$Campaign.observed.sustained_campaign_completed -or
        -not [bool]$Campaign.observed.coverage_report_generated -or
        [UInt64]$Campaign.observed.executions -ne 469853771 -or
        [UInt64]$Campaign.observed.wall_clock_seconds -ne 28801) {
        throw 'campaign manifest does not preserve smoke and accepted sustained results'
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
        [string]$Toolchain.verified_source_revision -cne
            '6c3e0c7280c023a34146d8268e73679539b86adb' -or
        -not [bool]$Toolchain.observed.instrumented_target_built -or
        [bool]$Toolchain.observed.smoke_campaign_completed) {
        throw 'historical toolchain build record changed or was rewritten'
    }

    if ([string]$Migration.milestone -cne 'M1.1.80' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.history_compatible_successor.coverage_guided_fuzz_harness -cne
            'instrumented-sustained-verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration smoke state changed'
    }
    $smokeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'coverage-guided-fuzz-harness'
    })
    $sustainedRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'sustained-coverage-guided-fuzzing'
    })
    if ($smokeRequirement.Count -ne 1 -or
        [string]$smokeRequirement[0].status -cne 'instrumented-smoke-verified' -or
        $sustainedRequirement.Count -ne 1 -or
        [string]$sustainedRequirement[0].status -cne 'verified') {
        throw 'smoke and sustained campaign gates were conflated'
    }
    if ([string]$Transport.milestone -cne 'M1.1.80' -or
        [string]$Transport.status -cne $aggregateStatus -or
        -not [bool]$Transport.history_compatible_successor.coverage_guided_fuzz_harness -or
        -not [bool]$Transport.history_compatible_successor.instrumented_fuzz_smoke -or
        -not [bool]$Transport.history_compatible_successor.sustained_fuzz_campaign -or
        [bool]$Transport.history_compatible_successor.registered -or
        [bool]$Transport.history_compatible_successor.dialed -or
        -not [bool]$Transport.scope.network_executed) {
        throw 'transport smoke state changed'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.80' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.coverage_guided_fuzz_harness_integrated -or
        -not [bool]$Compatibility.required_successor.instrumented_fuzz_smoke_verified -or
        -not [bool]$Compatibility.required_successor.sustained_fuzz_campaign_verified -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility smoke state changed'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    $requiredArtifacts = @(
        'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json',
        'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json',
        'CANONICAL-SYNC-V2-FUZZ-SMOKE.json',
        'fuzz/Cargo.lock',
        'scripts/verify-kilogram-canonical-sync-v2-fuzz-smoke-boundary.ps1',
        'docs/RFC-0201-m1174-pinned-fuzz-toolchain.md',
        'docs/RFC-0202-m1175-instrumented-fuzz-smoke.md'
    )
    if ([string]$Coverage.milestone -cne 'M1.1.80' -or
        [string]$Coverage.status -cne $aggregateStatus -or
        $surface.Count -ne 1 -or
        'instrumented-fuzz-smoke' -cnotin @($surface[0].covered)) {
        throw 'canonical coverage smoke state changed'
    }
    foreach ($artifact in $requiredArtifacts) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing smoke artifact '$artifact'"
        }
    }
    Assert-ExactStrings @($surface[0].remaining) @() 'coverage remaining work'

    Assert-ContainsAll $Runner @(
        "[ValidateSet('Preflight', 'Smoke', 'Sustained')]",
        "'run', '--jobs', '1'",
        'initial_seed_count = [int]$manifest.seed_count',
        'artifact_files = $artifactFiles',
        'coverage_generated = $coverageGenerated',
        'sustained_gate_satisfied = $sustainedSatisfied',
        'Sustained mode requires a clean committed source worktree.'
    ) 'instrumented runner'
    foreach ($inactiveSource in @($Wire, $ActiveTransport, $Cli)) {
        if ($inactiveSource.Contains('CANONICAL-SYNC-V2-FUZZ-SMOKE') -or
            $inactiveSource.Contains('instrumented_fuzz_smoke')) {
            throw 'smoke evidence leaked into active wire, transport, or CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'first clean-source instrumented run',
        '5,697,050',
        'generated failure artifacts: 0',
        'does not require ignored raw evidence to exist on every clone',
        'at least 28,800 seconds',
        'does not register or dial `kilogram/m1/sync/cbor/2`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "instrumented smoke RFC is missing '$needle'"
        }
    }
}

$paths = @(
    $smokePath, $campaignPath, $toolchainPath, $migrationPath, $transportPath,
    $compatibilityPath, $coveragePath, $runnerPath, $wirePath,
    $activeTransportPath, $cliPath, $rfcPath
)
foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "instrumented smoke contract file is missing: $path"
    }
}

$smoke = Get-Content -LiteralPath $smokePath -Raw | ConvertFrom-Json
$campaign = Get-Content -LiteralPath $campaignPath -Raw | ConvertFrom-Json
$toolchain = Get-Content -LiteralPath $toolchainPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transport = Get-Content -LiteralPath $transportPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$runner = Get-Content -LiteralPath $runnerPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$activeTransport = Get-Content -LiteralPath $activeTransportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

$arguments = @(
    $campaign, $toolchain, $migration, $transport, $compatibility, $coverage,
    $runner, $wire, $activeTransport, $cli, $rfc
)
Assert-SmokeContract $smoke @arguments

if ($SelfTest) {
    $sustained = Copy-JsonObject $smoke
    $sustained.run.sustained_gate_satisfied = $true
    Assert-Rejected { Assert-SmokeContract $sustained @arguments } 'false_sustained_completion'

    $artifact = Copy-JsonObject $smoke
    $artifact.run.artifact_files = 1
    Assert-Rejected { Assert-SmokeContract $artifact @arguments } 'generated_artifact'

    $coverageMissing = Copy-JsonObject $smoke
    $coverageMissing.run.coverage_generated = $false
    Assert-Rejected { Assert-SmokeContract $coverageMissing @arguments } 'missing_coverage_profile'

    $sourceDrift = Copy-JsonObject $smoke
    $sourceDrift.source.revision_after = 'different-source-revision'
    Assert-Rejected { Assert-SmokeContract $sourceDrift @arguments } 'source_revision_drift'

    $activated = Copy-JsonObject $smoke
    $activated.activation.active_transport_authorized = $true
    Assert-Rejected { Assert-SmokeContract $activated @arguments } 'premature_transport_activation'

    Write-Output 'canonical_sync_v2_fuzz_smoke_self_test=passed'
}

$localEvidenceChecked = 0
foreach ($name in @('record', 'run_log', 'coverage_log')) {
    $entry = $smoke.evidence.$name
    $relativePath = ([string]$entry.path).Replace('/', [System.IO.Path]::DirectorySeparatorChar)
    $path = Join-Path $workspace $relativePath
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ([UInt64]$item.Length -ne [UInt64]$entry.bytes -or
            $hash -cne [string]$entry.sha256) {
            throw "local ignored smoke evidence does not match the tracked digest: $name"
        }
        $localEvidenceChecked++
    }
}

Write-Output 'canonical_sync_v2_fuzz_smoke_boundary=verified'
Write-Output 'smoke_campaign_completed=true'
Write-Output 'smoke_executions=5697050'
Write-Output 'smoke_elapsed_seconds=301'
Write-Output 'smoke_artifact_files=0'
Write-Output 'coverage_profile_validated=true'
Write-Output "local_ignored_evidence_checked=$localEvidenceChecked"
Write-Output 'sustained_coverage_guided_fuzzing=verified'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
