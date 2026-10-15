[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$sourcePath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0133-m117-canonical-coverage-audit.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-Manifest {
    param([object]$Coverage, [object]$Source)
    if ([UInt64]$Coverage.schema_version -ne 1 -or [string]$Coverage.milestone -cne 'M1.1.23' -or
        [string]$Coverage.status -cne 'coverage-audit-not-active' -or [bool]$Coverage.changes_active_wire -or
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
        [int]$Coverage.summary.complete -ne 13 -or [int]$Coverage.summary.partial -ne 1 -or
        [int]$Coverage.summary.not_started -ne 4 -or [int]$Coverage.summary.not_targeted -ne 2 -or
        [bool]$Coverage.summary.activation_ready) { throw 'canonical coverage summary changed' }

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
    if ($counts['complete'] -ne 13 -or $counts['partial'] -ne 1 -or
        $counts['not-started'] -ne 4 -or $counts['not-targeted'] -ne 2) {
        throw 'computed canonical coverage counts changed'
    }
    $completeIds = @($Coverage.surfaces | Where-Object { $_.coverage -ceq 'complete' } | ForEach-Object { $_.id })
    if (($completeIds -join ',') -cne 'account-authority,authorized-event,blind-mailbox-capability,blind-mailbox-provider-gossip,blind-mailbox-transport,device-session-authorization,history-recovery,mailbox-provisioning,publication-conflict,ratchet-public-artifacts,runtime-mailbox-envelope,sync,ticket-publication-capability') {
        throw 'unexpected complete canonical surface'
    }
    $partialIds = @($Coverage.surfaces | Where-Object { $_.coverage -ceq 'partial' } | ForEach-Object { $_.id })
    if (($partialIds -join ',') -cne 'endpoint-announcement') {
        throw 'unexpected partial canonical surface'
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
if ($schemaRequirement.Count -ne 1 -or [string]$schemaRequirement[0].status -cne 'pending') {
    throw 'migration no longer blocks on canonical schema coverage'
}
foreach ($required in @('four are complete, two are partial, twelve are not',
    'A partial surface is not activation', 'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "coverage RFC is missing '$required'"
    }
}

if ($SelfTest) {
    $removed = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.surfaces = @($removed.surfaces | Select-Object -Skip 1)
    Assert-Rejected { Assert-Manifest $removed $source } 'removed_source_surface'
    $promoted = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $promoted.surfaces[1].coverage = 'complete'
    Assert-Rejected { Assert-Manifest $promoted $source } 'incomplete_surface_promoted'
    $ready = $coverage | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $ready.summary.activation_ready = $true
    Assert-Rejected { Assert-Manifest $ready $source } 'premature_activation_ready'
    Write-Output 'canonical_coverage_self_test=passed'
}

Write-Output 'canonical_coverage_boundary=verified'
Write-Output 'source_surfaces=20'
Write-Output 'complete_surfaces=13'
Write-Output 'partial_surfaces=1'
Write-Output 'not_started_surfaces=4'
Write-Output 'not_targeted_surfaces=2'
Write-Output 'activation_ready=false'
Write-Output 'active_wire_changed=false'
