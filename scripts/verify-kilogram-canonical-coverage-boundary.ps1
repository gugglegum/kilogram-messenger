[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$sourcePath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0209-m1185-canonical-sync-v2-opt-in-dark-launch-acceptance.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-Manifest {
    param([object]$Coverage, [object]$Source)
    if ([UInt64]$Coverage.schema_version -ne 1 -or [string]$Coverage.milestone -cne 'M1.1.85' -or
        [string]$Coverage.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        -not [bool]$Coverage.changes_active_wire -or
        [string]$Coverage.public_default -cne 'legacy-only' -or
        [string]$Coverage.opt_in_dark_launch_manifest -cne
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json' -or
        [string]$Coverage.source_manifest -cne 'PROTOCOL-COMPATIBILITY.json' -or
        [string]$Coverage.target_profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical coverage header changed'
    }
    $sourceIds = @($Source.surfaces | ForEach-Object { [string]$_.id })
    $coverageIds = @($Coverage.surfaces | ForEach-Object { [string]$_.id })
    if (($sourceIds -join "`n") -cne ($coverageIds -join "`n")) {
        throw 'canonical coverage does not exactly cover source registry'
    }
    if ($sourceIds.Count -ne 20 -or [int]$Coverage.summary.source_surfaces -ne 20 -or
        [int]$Coverage.summary.complete -ne 18 -or [int]$Coverage.summary.partial -ne 0 -or
        [int]$Coverage.summary.not_started -ne 0 -or [int]$Coverage.summary.not_targeted -ne 2 -or
        -not [bool]$Coverage.summary.activation_ready -or
        -not [bool]$Coverage.summary.opt_in_dark_launch_verified -or
        [bool]$Coverage.summary.public_default_activated) {
        throw 'canonical coverage summary changed'
    }

    $counts = @{}
    foreach ($name in @('complete', 'partial', 'not-started', 'not-targeted')) { $counts[$name] = 0 }
    foreach ($surface in $Coverage.surfaces) {
        $status = [string]$surface.coverage
        if (-not $counts.ContainsKey($status)) { throw "unknown coverage status '$status'" }
        $counts[$status]++
        $artifacts = @($surface.artifacts)
        $covered = @($surface.covered)
        $remaining = @($surface.remaining)
        if ($status -ceq 'complete' -and ($covered.Count -eq 0 -or $remaining.Count -ne 0 -or $artifacts.Count -eq 0)) {
            throw "invalid complete coverage for $($surface.id)"
        }
        if ($status -ceq 'partial' -and ($covered.Count -eq 0 -or $remaining.Count -eq 0 -or $artifacts.Count -eq 0)) {
            throw "invalid partial coverage for $($surface.id)"
        }
        if ($status -ceq 'not-started' -and ($covered.Count -ne 0 -or $remaining.Count -eq 0 -or $artifacts.Count -ne 0)) {
            throw "invalid not-started coverage for $($surface.id)"
        }
        if ($status -ceq 'not-targeted' -and ($remaining.Count -ne 0 -or [string]::IsNullOrWhiteSpace([string]$surface.reason))) {
            throw "invalid not-targeted coverage for $($surface.id)"
        }
        foreach ($artifact in $artifacts) {
            $relative = [string]$artifact
            if ([System.IO.Path]::IsPathRooted($relative) -or $relative.Contains('..')) {
                throw "invalid canonical artifact path '$relative'"
            }
            if (-not (Test-Path -LiteralPath (Join-Path $workspace $relative) -PathType Leaf)) {
                throw "missing canonical artifact '$relative'"
            }
        }
    }
    if ($counts['complete'] -ne 18 -or $counts['partial'] -ne 0 -or
        $counts['not-started'] -ne 0 -or $counts['not-targeted'] -ne 2) {
        throw 'computed canonical coverage counts changed'
    }
    $completeIds = @($Coverage.surfaces | Where-Object { $_.coverage -ceq 'complete' } | ForEach-Object { $_.id })
    if (($completeIds -join ',') -cne 'account-authority,account-recovery,authorized-event,blind-mailbox-capability,blind-mailbox-provider-gossip,blind-mailbox-transport,device-session-authorization,endpoint-announcement,history-recovery,history-rewrap,mailbox-provisioning,publication-conflict,ratchet-public-artifacts,runtime-ipc,runtime-mailbox-envelope,sync,ticket-publication-capability,transport-sync') {
        throw 'unexpected complete canonical surface'
    }
    $partialIds = @($Coverage.surfaces | Where-Object { $_.coverage -ceq 'partial' } | ForEach-Object { $_.id })
    if ($partialIds.Count -ne 0) {
        throw 'unexpected partial canonical surface'
    }
    $transportSurface = @($Coverage.surfaces | Where-Object { $_.id -ceq 'transport-sync' })
    if ($transportSurface.Count -ne 1) {
        throw 'transport-sync coverage surface is missing'
    }
    foreach ($requiredArtifact in @(
        'CANONICAL-SYNC-V2-N-MINUS-ONE-INTEROP.json',
        'scripts/verify-kilogram-canonical-sync-v2-n-minus-one-interop-boundary.ps1',
        'docs/RFC-0215-m1191-n-minus-one-production-interop.md'
    )) {
        if ($requiredArtifact -cnotin @($transportSurface[0].artifacts)) {
            throw "transport-sync coverage is missing '$requiredArtifact'"
        }
    }
    if ((@($Coverage.next_priority) -join ',') -cne
        'privacy-preserving-rollout-cohort-authority,durable-local-rollout-health-evidence,first-contact-ticket-extension-stripping-resolution,public-rollout-containment-rehearsal,canonical-native-history-materialization,legacy-sync-retirement-prerequisites') {
        throw 'canonical next-priority inventory changed'
    }
}

foreach ($path in @($coveragePath, $sourcePath, $migrationPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical coverage file is missing: $path" }
}
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Manifest $coverage $source

$schemaRequirement = @($migration.activation_requirements | Where-Object { $_.id -ceq 'canonical-schema-coverage' })
if ($schemaRequirement.Count -ne 1 -or [string]$schemaRequirement[0].status -cne 'verified') {
    throw 'migration did not verify canonical schema coverage'
}
$normalizedRfc = [regex]::Replace($rfc, '\s+', ' ')
foreach ($required in @(
    'verified opt-in dark launch',
    'public_default=legacy-only',
    'does not enable a public dual-reader default'
)) {
    if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "coverage RFC is missing '$required'"
    }
}

if ($SelfTest) {
    $removed = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.surfaces = @($removed.surfaces | Select-Object -Skip 1)
    Assert-Rejected { Assert-Manifest $removed $source } 'removed_source_surface'
    $reopened = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $reopened.surfaces[19].coverage = 'partial'
    Assert-Rejected { Assert-Manifest $reopened $source } 'completed_surface_reopened'
    $notReady = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $notReady.summary.activation_ready = $false
    Assert-Rejected { Assert-Manifest $notReady $source } 'completed_coverage_not_ready'
    $missingInteropEvidence = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missingTransportSurface = @($missingInteropEvidence.surfaces | Where-Object {
        [string]$_.id -ceq 'transport-sync'
    })[0]
    $missingTransportSurface.artifacts = @(
        $missingTransportSurface.artifacts |
            Where-Object { [string]$_ -cne 'CANONICAL-SYNC-V2-N-MINUS-ONE-INTEROP.json' }
    )
    Assert-Rejected {
        Assert-Manifest $missingInteropEvidence $source
    } 'n_minus_one_evidence_untracked'
    $stalePriority = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $stalePriority.next_priority[2] = 'n-minus-one-production-interoperability'
    Assert-Rejected { Assert-Manifest $stalePriority $source } 'completed_priority_reopened'
    Write-Output 'canonical_coverage_self_test=passed'
}

Write-Output 'canonical_coverage_boundary=verified'
Write-Output 'source_surfaces=20'
Write-Output 'complete_surfaces=18'
Write-Output 'partial_surfaces=0'
Write-Output 'not_started_surfaces=0'
Write-Output 'not_targeted_surfaces=2'
Write-Output 'activation_ready=true'
Write-Output 'opt_in_dark_launch_verified=true'
Write-Output 'public_default=legacy-only'
Write-Output 'active_wire_changed=true'
