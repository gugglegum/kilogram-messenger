[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DECODER-MUTATIONS.json'
$responderVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json'
$rollbackPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLBACK-DRILL.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$testPath = Join-Path $workspace 'crates\kilogram-protocol\tests\canonical_sync_v2_decoder_mutations.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0199-m1172-canonical-sync-v2-decoder-mutations.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-LowerHex {
    param([string]$Value, [int]$Bytes, [string]$Label)
    if ($Value.Length -ne (2 * $Bytes) -or $Value -cnotmatch '^[0-9a-f]+$') {
        throw "$Label is not exact lowercase $Bytes-byte hexadecimal"
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$ResponderVectors,
        [object]$Rollback,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$TestSource,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.72' -or
        [string]$Manifest.status -cne 'bounded-canonical-sync-v2-regression-corpus' -or
        [bool]$Manifest.active_runtime_changed -or
        [bool]$Manifest.coverage_guided -or
        [bool]$Manifest.compiler_instrumentation -or
        [UInt64]$Manifest.fixture_count -ne 20 -or
        [UInt64]$Manifest.minimum_mutations -ne 2500 -or
        [UInt64]$Manifest.expected_mutations -ne 2863 -or
        [UInt64]$Manifest.maximum_mutations -ne 4000) {
        throw 'sync-v2 mutation manifest header changed'
    }
    if ([UInt64]$Manifest.resource_bounds.maximum_fixture_bytes -ne 8388608 -or
        [UInt64]$Manifest.resource_bounds.maximum_positions_per_grid_per_fixture -ne 128 -or
        [UInt64]$Manifest.resource_bounds.maximum_total_mutations -ne 4000 -or
        [bool]$Manifest.resource_bounds.network_allowed -or
        [bool]$Manifest.resource_bounds.randomness_allowed) {
        throw 'sync-v2 mutation resource boundary changed'
    }
    Assert-ExactStrings @($Manifest.mutation_classes) @(
        'bounded-strict-prefix-truncation-grid',
        'authenticated-bounded-low-bit-flip-grid',
        'trailing-byte',
        'non-minimal-top-level-version',
        'indefinite-top-level-array',
        'reserved-top-level-additional-information',
        'oversized-declared-byte-string'
    ) 'sync-v2 mutation classes'
    Assert-ExactStrings @($Manifest.remaining_m1_1_gates) @(
        'sustained-coverage-guided-fuzzing', 'fresh-hosted-and-field-evidence'
    ) 'sync-v2 remaining gates'

    $fixtureIds = @($Manifest.fixtures | ForEach-Object { [string]$_.id })
    Assert-ExactStrings $fixtureIds @(
        'legacy-event-key-v2', 'canonical-event-key-v2',
        'legacy-event-carrier-v2', 'canonical-event-carrier-v2',
        'signed-sync-inventory-v2', 'sync-event-batch-v2',
        'signed-sync-diff-v2', 'sync-complete-v2', 'sync-pause-v2',
        'sync-paused-v2', 'sync-rejected-v2',
        'sync-client-inventory-frame-v2', 'sync-client-events-frame-v2',
        'sync-client-pause-frame-v2', 'sync-server-diff-frame-v2',
        'sync-server-complete-frame-v2', 'sync-server-rejected-frame-v2',
        'sync-server-paused-frame-v2', 'signed-sync-session-checkpoint-v2',
        'signed-sync-responder-checkpoint-v2'
    ) 'sync-v2 fixture inventory'
    $authenticated = @($Manifest.fixtures | Where-Object {
        [string]$_.mutation_policy -ceq 'authenticated-bit-flips'
    } | ForEach-Object { [string]$_.id })
    Assert-ExactStrings $authenticated @(
        'legacy-event-carrier-v2', 'canonical-event-carrier-v2',
        'signed-sync-inventory-v2', 'signed-sync-diff-v2',
        'sync-client-inventory-frame-v2', 'sync-server-diff-frame-v2',
        'signed-sync-session-checkpoint-v2', 'signed-sync-responder-checkpoint-v2'
    ) 'authenticated bit-flip allowlist'
    foreach ($fixture in @($Manifest.fixtures)) {
        if ([string]$fixture.mutation_policy -cnotin @(
            'structural-only', 'authenticated-bit-flips'
        )) {
            throw "unknown mutation policy on $([string]$fixture.id)"
        }
    }

    if ([UInt64]$ResponderVectors.schema_version -ne 1 -or
        [string]$ResponderVectors.milestone -cne 'M1.1.72' -or
        [string]$ResponderVectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$ResponderVectors.activation_status -cne 'not-active' -or
        [string]$ResponderVectors.expected.protocol_identifier_utf8 -cne
            'kilogram/m1/sync/cbor/2') {
        throw 'responder checkpoint vector header changed'
    }
    Assert-LowerHex ([string]$ResponderVectors.input.responder_device_secret_key_hex) 32 `
        'responder secret seed'
    Assert-LowerHex ([string]$ResponderVectors.expected.responder_device_id_hex) 32 `
        'responder Device ID'
    Assert-LowerHex ([string]$ResponderVectors.expected.signature_hex) 64 `
        'responder signature'
    Assert-LowerHex ([string]$ResponderVectors.expected.signed_cbor_hex) 238 `
        'signed responder checkpoint'
    Assert-LowerHex ([string]$ResponderVectors.expected.checkpoint_id_hex) 32 `
        'responder checkpoint ID'
    if ([string]$ResponderVectors.expected.signature_hex -cne
            '21dae402a3a47421558a6ec8d55c6732462d60d25e0f8c363e784dfe5398c7e07619cdc57083ddd9bbbe3c492b117b59a20f316ab03f2524b1dfb138a0e36c0c' -or
        [string]$ResponderVectors.expected.signature_domain_hex -cne
            '6b696c6f6772616d3a73796e632d726573706f6e6465722d636865636b706f696e742d7369676e61747572653a63626f722d763200' -or
        [string]$ResponderVectors.expected.checkpoint_id_domain_hex -cne
            '6b696c6f6772616d3a73796e632d726573706f6e6465722d636865636b706f696e742d69643a63626f722d763200') {
        throw 'responder checkpoint domains changed'
    }

    foreach ($needle in @(
        'const EXPECTED_MUTATIONS: usize = 2_863;',
        'const MAXIMUM_MUTATIONS: usize = 4_000;',
        'CanonicalSyncEventKeyV2::decode(bytes)',
        'CanonicalSyncEventCarrierV2::decode_and_verify(bytes)',
        'decode_and_verify_signed_sync_inventory_v2(bytes)',
        'decode_and_verify_sync_event_batch_v2(bytes)',
        'decode_and_verify_signed_sync_diff_v2(bytes)',
        'decode_and_verify_sync_client_frame_v2(bytes)',
        'decode_and_verify_sync_server_frame_v2(bytes)',
        'decode_and_verify_signed_sync_session_checkpoint_v2(bytes)',
        'decode_and_verify_signed_sync_responder_checkpoint_v2(bytes)',
        'canonical_sync_v2_decoder_mutations={mutations}'
    )) {
        if (-not $TestSource.Contains($needle)) {
            throw "sync-v2 mutation source marker missing: $needle"
        }
    }

    $aggregateStatus = 'canonical-sync-v2-opt-in-dark-launch-verified'
    if ([string]$Rollback.milestone -cne 'M1.1.71' -or
        [string]$Rollback.status -cne
            'dormant-deterministic-rollback-drill-verified-not-active' -or
        [bool]$Rollback.activation.active_transport_authorized) {
        throw 'rollback drill prerequisite changed'
    }
    if ([string]$Migration.milestone -cne 'M1.1.85' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.history_compatible_successor.bounded_sync_v2_decoder_mutation_corpus -cne
            'verified' -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial) {
        throw 'canonical migration mutation state changed'
    }
    $mutationRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'bounded-canonical-sync-v2-decoder-mutation-regression'
    })
    if ($mutationRequirement.Count -ne 1 -or
        [string]$mutationRequirement[0].status -cne 'verified') {
        throw 'sync-v2 mutation activation requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.85' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.bounded_sync_v2_decoder_mutation_corpus -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport manifest mutation state changed'
    }
    Assert-ExactStrings @($TransportManifest.remaining) @() 'transport remaining work'
    if ([string]$Compatibility.milestone -cne 'M1.1.85' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.bounded_sync_v2_decoder_mutation_corpus_integrated -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility mutation state changed'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.85' -or $surface.Count -ne 1 -or
        'CANONICAL-SYNC-V2-DECODER-MUTATIONS.json' -cnotin @($surface[0].artifacts) -or
        'CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json' -cnotin @($surface[0].artifacts) -or
        'crates/kilogram-protocol/tests/canonical_sync_v2_decoder_mutations.rs' -cnotin
            @($surface[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-decoder-mutations-boundary.ps1' -cnotin
            @($surface[0].artifacts) -or
        'docs/RFC-0199-m1172-canonical-sync-v2-decoder-mutations.md' -cnotin
            @($surface[0].artifacts) -or
        'bounded-canonical-sync-v2-decoder-mutation-corpus' -cnotin @($surface[0].covered)) {
        throw 'coverage mutation state changed'
    }
    Assert-ExactStrings @($surface[0].remaining) @() 'coverage remaining work'

    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('CANONICAL-SYNC-V2-DECODER-MUTATIONS') -or
            $inactiveSource.Contains('bounded_canonical_sync_v2_decoder_mutation_corpus')) {
            throw 'test-only mutation corpus leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'separate, reproducible corpus for every public sync v2 decoder',
        'exact corpus is 2,863 mutations across 20 fixtures',
        'applies only to authenticated envelopes and carriers',
        'has no compiler instrumentation and is not coverage-guided fuzzing',
        'does not close the sustained coverage-guided fuzzing gate',
        'does not register or dial `kilogram/m1/sync/cbor/2`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "sync-v2 mutation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $responderVectorsPath, $rollbackPath, $migrationPath,
    $transportManifestPath, $compatibilityPath, $coveragePath, $testPath,
    $wirePath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "sync-v2 mutation file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$responderVectors = Get-Content -LiteralPath $responderVectorsPath -Raw | ConvertFrom-Json
$rollback = Get-Content -LiteralPath $rollbackPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$testSource = Get-Content -LiteralPath $testPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $responderVectors $rollback $migration $transportManifest `
    $compatibility $coverage $testSource $wire $transport $cli $rfc

if ($SelfTest) {
    $coverageClaim = Copy-JsonObject $manifest
    $coverageClaim.coverage_guided = $true
    Assert-Rejected {
        Assert-Contract $coverageClaim $responderVectors $rollback $migration `
            $transportManifest $compatibility $coverage $testSource $wire $transport $cli $rfc
    } 'false_coverage_guided_claim'

    $count = Copy-JsonObject $manifest
    $count.expected_mutations = 2862
    Assert-Rejected {
        Assert-Contract $count $responderVectors $rollback $migration $transportManifest `
            $compatibility $coverage $testSource $wire $transport $cli $rfc
    } 'mutation_count_drift'

    $policy = Copy-JsonObject $manifest
    $policy.fixtures[0].mutation_policy = 'authenticated-bit-flips'
    Assert-Rejected {
        Assert-Contract $policy $responderVectors $rollback $migration $transportManifest `
            $compatibility $coverage $testSource $wire $transport $cli $rfc
    } 'unsigned_semantic_bit_flip_claim'

    $signature = Copy-JsonObject $responderVectors
    $signature.expected.signature_hex = ('00' +
        ([string]$signature.expected.signature_hex).Substring(2))
    Assert-Rejected {
        Assert-Contract $manifest $signature $rollback $migration $transportManifest `
            $compatibility $coverage $testSource $wire $transport $cli $rfc
    } 'responder_vector_signature_drift'

    $activated = Copy-JsonObject $transportManifest
    $activated.history_compatible_successor.registered = $true
    Assert-Rejected {
        Assert-Contract $manifest $responderVectors $rollback $migration $activated `
            $compatibility $coverage $testSource $wire $transport $cli $rfc
    } 'premature_transport_activation'

    Write-Output 'canonical_sync_v2_decoder_mutations_self_test=passed'
}

Write-Output 'canonical_sync_v2_decoder_mutations_boundary=verified'
Write-Output 'fixture_count=20'
Write-Output 'deterministic_mutations=2863'
Write-Output 'coverage_guided=false'
Write-Output 'compiler_instrumentation=false'
Write-Output 'sustained_coverage_guided_fuzzing=pending'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
