[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$cargoPath = Join-Path $root 'apps\kilogram-cli\Cargo.toml'
$evidencePath = Join-Path $root 'apps\kilogram-cli\src\bin\kilogram-sync-v2-evidence.rs'
$productionPath = Join-Path $root 'apps\kilogram-cli\src\main.rs'
$transportPath = Join-Path $root 'crates\kilogram-transport-iroh\src\lib.rs'
$workflowPath = Join-Path $root '.github\workflows\canonical-sync-v2-evidence.yml'
$runnerPath = Join-Path $root 'scripts\invoke-kilogram-sync-v2-hosted-evidence.ps1'
$verifierPath = Join-Path $root 'scripts\verify-kilogram-sync-v2-network-evidence.ps1'
$recordPath = Join-Path $root 'CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json'
$migrationPath = Join-Path $root 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $root 'CANONICAL-TRANSPORT-SYNC.json'
$coveragePath = Join-Path $root 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $root 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactProperties {
    param([object]$Value, [string[]]$Expected, [string]$Label)
    $actual = @($Value.PSObject.Properties.Name | Sort-Object)
    $wanted = @($Expected | Sort-Object)
    if (($actual -join "`n") -cne ($wanted -join "`n")) { throw "$Label properties changed" }
}

function Assert-NetworkEvidenceRecord {
    param([object]$Record)
    Assert-ExactProperties $Record @(
        'schema_version', 'milestone', 'status', 'source_revision', 'protocol',
        'matrix', 'hosted', 'physical', 'acceptance', 'production_boundary',
        'residual_risks'
    ) 'network evidence record'
    if ([UInt64]$Record.schema_version -ne 1 -or
        [string]$Record.milestone -cne 'M1.1.80' -or
        [string]$Record.status -cne 'canonical-sync-v2-ready-for-explicit-activation-decision' -or
        [string]$Record.source_revision -cne 'b1f23c3719a0842979f047d874141953ae93ccac') {
        throw 'network evidence identity changed'
    }
    if ([string]$Record.protocol.active -cne 'kilogram/m0/sync/8' -or
        [string]$Record.protocol.evidence_candidate -cne 'kilogram/m1/sync/cbor/2' -or
        (@($Record.matrix) -join ',') -cne
            'canonical-v2-v2-direct,canonical-v2-restart-resume,canonical-v2-v2-relay,mixed-version-pre-selection-legacy,post-selection-corruption-fail-closed,explicit-one-step-rollback') {
        throw 'network evidence protocol or matrix changed'
    }
    Assert-ExactProperties $Record.hosted @(
        'evidence_kind', 'workflow', 'workflow_run_id', 'collected_at_utc',
        'manifest_sha256', 'manifest_bytes', 'binary_sha256', 'binary_bytes',
        'target', 'profile', 'feature', 'topology',
        'same_binary_within_topology', 'result'
    ) 'hosted evidence'
    if ([string]$Record.hosted.evidence_kind -cne 'hosted-windows' -or
        [string]$Record.hosted.workflow -cne '.github/workflows/canonical-sync-v2-evidence.yml' -or
        [UInt64]$Record.hosted.workflow_run_id -ne 39116855502 -or
        [string]$Record.hosted.collected_at_utc -cne '2026-10-18T09:56:31.9633548Z' -or
        [string]$Record.hosted.manifest_sha256 -cne 'd6382354c0327bb1aa900e7dc33536b467401f95edade1e9113f33e89e21e535' -or
        [UInt64]$Record.hosted.manifest_bytes -ne 29618 -or
        [string]$Record.hosted.binary_sha256 -cne '6a0add9561ce59e4c9b2d36a0ab3f81ab2fc5287df25905579f94c81ec031f9c' -or
        [UInt64]$Record.hosted.binary_bytes -ne 39045120 -or
        [string]$Record.hosted.target -cne 'x86_64-pc-windows-msvc' -or
        [string]$Record.hosted.profile -cne 'debug' -or
        [string]$Record.hosted.feature -cne 'sync-v2-evidence' -or
        [string]$Record.hosted.topology -cne 'single-host-two-process' -or
        -not [bool]$Record.hosted.same_binary_within_topology -or
        [string]$Record.hosted.result -cne 'verified') {
        throw 'hosted evidence record changed'
    }
    Assert-ExactProperties $Record.physical @(
        'evidence_kind', 'run_id', 'collected_at_utc', 'manifest_sha256',
        'manifest_bytes', 'binary_sha256', 'binary_bytes', 'target', 'profile',
        'feature', 'topology', 'same_binary_within_topology', 'result'
    ) 'physical evidence'
    if ([string]$Record.physical.evidence_kind -cne 'physical-windows' -or
        [string]$Record.physical.run_id -cne 'm1180-field-20261018T093610Z' -or
        [string]$Record.physical.collected_at_utc -cne '2026-10-18T09:38:46.4698415Z' -or
        [string]$Record.physical.manifest_sha256 -cne '3b87c95c9298fead83ab6cf07d2c37d59e1d8e3336db1d85eb6841a8c9831b37' -or
        [UInt64]$Record.physical.manifest_bytes -ne 29646 -or
        [string]$Record.physical.binary_sha256 -cne 'e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625' -or
        [UInt64]$Record.physical.binary_bytes -ne 39043072 -or
        [string]$Record.physical.target -cne 'x86_64-pc-windows-msvc' -or
        [string]$Record.physical.profile -cne 'debug' -or
        [string]$Record.physical.feature -cne 'sync-v2-evidence' -or
        [string]$Record.physical.topology -cne 'two-distinct-physical-hosts' -or
        -not [bool]$Record.physical.same_binary_within_topology -or
        [string]$Record.physical.result -cne 'verified') {
        throw 'physical evidence record changed'
    }
    if (-not [bool]$Record.acceptance.same_source_revision -or
        -not [bool]$Record.acceptance.hosted_and_physical_topologies_distinct -or
        (@(
            [string]$Record.acceptance.direct,
            [string]$Record.acceptance.restart_resume,
            [string]$Record.acceptance.relay,
            [string]$Record.acceptance.mixed_version,
            [string]$Record.acceptance.post_selection_fail_closed,
            [string]$Record.acceptance.rollback
        ) -join ',') -cne 'verified,verified,verified,verified,verified,verified') {
        throw 'network evidence acceptance changed'
    }
    if ([bool]$Record.production_boundary.candidate_registered_by_default -or
        [bool]$Record.production_boundary.candidate_dialed_by_default -or
        [bool]$Record.production_boundary.legacy_fallback_after_selection -or
        [bool]$Record.production_boundary.changes_active_wire -or
        -not [bool]$Record.production_boundary.evidence_only) {
        throw 'network evidence activated a production boundary'
    }
    if ((@($Record.residual_risks) -join ',') -cne
        'windows-only-network-evidence,independent-security-audit-not-performed,default-activation-requires-separate-explicit-decision') {
        throw 'network evidence residual risks changed'
    }
}

foreach ($path in @(
    $cargoPath, $evidencePath, $productionPath, $transportPath,
    $workflowPath, $runnerPath, $verifierPath, $recordPath, $migrationPath,
    $transportManifestPath, $coveragePath, $compatibilityPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "sync v2 evidence boundary input is missing: $path"
    }
}

$cargo = (Get-Content -LiteralPath $cargoPath -Raw).Replace("`r`n", "`n")
$evidence = (Get-Content -LiteralPath $evidencePath -Raw).Replace("`r`n", "`n")
$production = (Get-Content -LiteralPath $productionPath -Raw).Replace("`r`n", "`n")
$transport = (Get-Content -LiteralPath $transportPath -Raw).Replace("`r`n", "`n")
$workflow = (Get-Content -LiteralPath $workflowPath -Raw).Replace("`r`n", "`n")
$runner = (Get-Content -LiteralPath $runnerPath -Raw).Replace("`r`n", "`n")
$verifier = (Get-Content -LiteralPath $verifierPath -Raw).Replace("`r`n", "`n")
$record = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json

Assert-NetworkEvidenceRecord $record
foreach ($aggregate in @($migration, $transportManifest, $coverage, $compatibility)) {
    if ([string]$aggregate.milestone -cne 'M1.1.85' -or
        [string]$aggregate.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$aggregate.network_evidence_manifest -cne 'CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json' -or
        [string]$aggregate.opt_in_dark_launch_manifest -cne
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json' -or
        -not [bool]$aggregate.changes_active_wire -or
        [string]$aggregate.public_default -cne 'legacy-only') {
        throw 'aggregate canonical evidence linkage changed'
    }
}
$evidenceRequirement = @($migration.activation_requirements | Where-Object {
    [string]$_.id -ceq 'fresh-hosted-and-field-evidence'
})
$transportCoverage = @($coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
if ($evidenceRequirement.Count -ne 1 -or [string]$evidenceRequirement[0].status -cne 'verified' -or
    @($transportManifest.remaining).Count -ne 0 -or
    -not [bool]$transportManifest.scope.network_executed -or
    -not [bool]$transportManifest.scope.active_wire_changed -or
    [bool]$transportManifest.history_compatible_successor.registered -or
    [bool]$transportManifest.history_compatible_successor.dialed -or
    [bool]$transportManifest.authenticated_capability.canonical_dial_enabled -or
    $transportCoverage.Count -ne 1 -or [string]$transportCoverage[0].coverage -cne 'complete' -or
    @($transportCoverage[0].remaining).Count -ne 0 -or
    'fresh-hosted-and-field-evidence' -cnotin @($transportCoverage[0].covered) -or
    -not [bool]$coverage.summary.activation_ready -or
    -not [bool]$compatibility.scope.network_executed -or
    -not [bool]$compatibility.changes_active_wire -or
    [bool]$compatibility.existing_canonical_candidate.registered -or
    [bool]$compatibility.existing_canonical_candidate.dialed) {
    throw 'aggregate canonical evidence acceptance is incomplete or activated prematurely'
}

foreach ($required in @(
    '[features]',
    'default = []',
    'sync-v2-evidence = ["dep:ed25519-dalek"]',
    'name = "kilogram-sync-v2-evidence"',
    'path = "src/bin/kilogram-sync-v2-evidence.rs"',
    'required-features = ["sync-v2-evidence"]',
    'ed25519-dalek = { workspace = true, optional = true }'
)) {
    if (-not $cargo.Contains($required)) {
        throw "sync v2 evidence Cargo boundary is missing: $required"
    }
}

if ($SelfTest) {
    $changedRevision = Copy-JsonObject $record
    $changedRevision.source_revision = '0000000000000000000000000000000000000000'
    Assert-Rejected { Assert-NetworkEvidenceRecord $changedRevision } 'changed_evidence_revision'
    $activated = Copy-JsonObject $record
    $activated.production_boundary.candidate_registered_by_default = $true
    Assert-Rejected { Assert-NetworkEvidenceRecord $activated } 'premature_default_registration'
    $collapsedTopology = Copy-JsonObject $record
    $collapsedTopology.physical.topology = 'single-host-two-process'
    Assert-Rejected { Assert-NetworkEvidenceRecord $collapsedTopology } 'collapsed_physical_topology'
    Write-Output 'sync_v2_evidence_self_test=passed'
}

foreach ($required in @(
    '#![forbid(unsafe_code)]',
    'Feature-gated evidence harness for dormant canonical sync v2',
    'CANONICAL_SYNC_ALPN_V2',
    'verify_durable_mutual_canonical_candidate_selection',
    'relay_url: Option<RelayUrl>',
    'fallback_attempts: 0',
    'production_default_changed=false',
    'selected canonical decode failed; endpoint registered no legacy ALPN'
)) {
    if (-not $evidence.Contains($required)) {
        throw "sync v2 evidence source boundary is missing: $required"
    }
}

foreach ($forbidden in @(
    'kilogram-sync-v2-evidence',
    'sync-v2-evidence'
)) {
    if ($production.Contains($forbidden)) {
        throw "production CLI unexpectedly references evidence-only surface: $forbidden"
    }
}

if (-not $transport.Contains('pub const ALPN: &[u8] = b"kilogram/m0/sync/8";') -or
    -not $transport.Contains('pub const CANONICAL_SYNC_ALPN_V2: &[u8] = b"kilogram/m1/sync/cbor/2";')) {
    throw 'sync transport identifiers are not exact'
}

foreach ($required in @(
    'let mut runtime_alpns = Vec::new();',
    '.register_canonical_alpn()',
    'runtime_alpns.push(CANONICAL_SYNC_ALPN_V2.to_vec());',
    '.initiate_new_canonical_session()',
    'endpoint.connect(ticket.endpoint().clone(), CANONICAL_SYNC_ALPN_V2)'
)) {
    if (-not $production.Contains($required)) {
        throw "production CLI is missing guarded opt-in marker: $required"
    }
}

foreach ($required in @(
    "on:`n  workflow_dispatch:",
    '--features sync-v2-evidence',
    '--bin kilogram-sync-v2-evidence',
    'invoke-kilogram-sync-v2-hosted-evidence.ps1',
    '.tmp/canonical-sync-v2-upload/manifest.json'
)) {
    if (-not $workflow.Contains($required)) {
        throw "hosted sync v2 workflow boundary is missing: $required"
    }
}
foreach ($forbidden in @("`n  push:", "`n  pull_request:", '--release')) {
    if ($workflow.Contains($forbidden)) {
        throw "hosted sync v2 workflow contains forbidden automatic/release surface: $forbidden"
    }
}
if (-not $runner.Contains("name = 'kilogram-cli.exe'") -or
    -not $runner.Contains("evidence_kind = 'hosted-windows'") -or
    -not $runner.Contains("distinct_physical_hosts = `$false") -or
    -not $runner.Contains("sync_v2_hosted_prepare_lifecycle_self_test=verified") -or
    -not $runner.Contains('if (Test-Path -LiteralPath $directory)') -or
    -not $runner.Contains('(Join-Path $output "$Name-prepare")')) {
    throw 'hosted runner does not preserve the stable-name and topology boundary'
}
if ($runner.Contains("New-Item -ItemType Directory -Force -Path `$directory") -or
    $runner.Contains("(Join-Path `$directory 'prepare')") -or
    $runner.Contains('if ($LASTEXITCODE -ne 0)')) {
    throw 'hosted runner retains a forbidden prepare or PowerShell exit-code boundary'
}
foreach ($required in @(
    "'local_device_secret'", "'endpoint_secret'", "'requester_authorization'",
    "'listener_authorization'", "'evidence-role.json'",
    "evidence_kind = 'physical-windows'", "legacy_fallback_after_selection"
)) {
    if (-not $verifier.Contains($required)) {
        throw "network evidence verifier boundary is missing: $required"
    }
}

Write-Output 'sync_v2_evidence_boundary=verified'
Write-Output 'evidence_feature=sync-v2-evidence'
Write-Output 'evidence_binary=kilogram-sync-v2-evidence'
Write-Output 'candidate_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'active_alpn=kilogram/m0/sync/8'
Write-Output 'production_default_changed=false'
Write-Output 'hosted_workflow=manual-only'
Write-Output 'evidence_source_revision=b1f23c3719a0842979f047d874141953ae93ccac'
Write-Output 'hosted_workflow_run_id=39116855502'
Write-Output 'network_evidence_status=ready-for-explicit-activation-decision'
