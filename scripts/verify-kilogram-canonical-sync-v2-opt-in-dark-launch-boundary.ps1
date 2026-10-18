[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$historyCompatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$productionPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PRODUCTION-DARK-LAUNCH.json'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\canonical_sync_v2_runtime.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0209-m1185-canonical-sync-v2-opt-in-dark-launch-acceptance.md'

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

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$Transport,
        [object]$Coverage,
        [object]$HistoryCompatibility,
        [object]$Production,
        [string]$Runtime,
        [string]$Rfc
    )

    $status = 'canonical-sync-v2-opt-in-dark-launch-verified'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.85' -or
        [string]$Manifest.status -cne $status -or
        -not [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.legacy_protocol_identifier -cne 'kilogram/m0/sync/8' -or
        [string]$Manifest.canonical_protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'opt-in dark-launch acceptance header changed'
    }

    Assert-ExactStrings @($Manifest.activation_scope.default_registered_alpns) @(
        'kilogram/m0/sync/8'
    ) 'default registered ALPNs'
    Assert-ExactStrings @($Manifest.activation_scope.opt_in_registered_alpns) @(
        'kilogram/m0/sync/8', 'kilogram/m1/sync/cbor/2'
    ) 'opt-in registered ALPNs'
    if ([string]$Manifest.activation_scope.authority -cne
            'verified-device-signed-append-only-chain' -or
        [string]$Manifest.activation_scope.missing_authority -cne
            'implicit-legacy-only' -or
        [string]$Manifest.activation_scope.malformed_or_stale_authority -cne
            'fail-closed' -or
        -not [bool]$Manifest.activation_scope.new_canonical_session_requires_mutual_authenticated_selection -or
        [bool]$Manifest.activation_scope.selected_canonical_failure_fallback_to_legacy -or
        -not [bool]$Manifest.activation_scope.containment_preserves_exact_selected_resume) {
        throw 'opt-in activation boundary changed'
    }

    if ([UInt64]$Manifest.runtime_bounds.stream_io_timeout_seconds -ne 20 -or
        [UInt64]$Manifest.runtime_bounds.responder_preparation_and_first_response_timeout_seconds -ne 120 -or
        -not [bool]$Manifest.runtime_bounds.response_timeout_is_distinct_from_stream_io_timeout -or
        [bool]$Manifest.runtime_bounds.timeout_grants_authority -or
        [bool]$Manifest.runtime_bounds.timeout_permits_legacy_fallback_after_selection) {
        throw 'canonical runtime timeout boundary changed'
    }

    $field = $Manifest.field_evidence
    if ([string]$field.status -cne
            'canonical-sync-v2-two-host-activation-rehearsal-verified' -or
        [string]$field.run_id -cne 'm1184-exact-20261018T165652Z' -or
        [string]$field.source_revision -cne
            '7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c' -or
        [string]$field.binary_sha256 -cne
            'be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532' -or
        [UInt64]$field.binary_bytes -ne 59981824 -or
        -not [bool]$field.distinct_physical_hosts -or
        -not [bool]$field.same_binary_on_both_hosts -or
        [string]$field.transport -cne 'iroh-direct-lan') {
        throw 'two-host opt-in field evidence changed'
    }
    foreach ($property in @(
        'default_default_legacy_converged', 'opt_in_default_legacy_converged',
        'opt_in_opt_in_canonical_converged', 'restart_exact_selection_resumed',
        'containment_exact_selection_resumed',
        'containment_new_conversation_legacy', 'selected_loss_failed_closed',
        'pre_selection_one_step_rollback', 'successful_histories_equal'
    )) {
        if (-not [bool]$field.$property) {
            throw "field evidence '$property' is not verified"
        }
    }
    if ([bool]$field.selected_loss_legacy_retry) {
        throw 'selected canonical loss permits legacy retry'
    }

    if ([bool]$Manifest.decoder_regression.decoder_or_frame_codec_changed_since_accepted_corpora -or
        [bool]$Manifest.decoder_regression.legacy_mutation_corpus_rerun_required -or
        [bool]$Manifest.decoder_regression.canonical_sync_v2_mutation_corpus_rerun_required -or
        [UInt64]$Manifest.decoder_regression.accepted_legacy_mutations -ne 31851 -or
        [UInt64]$Manifest.decoder_regression.accepted_canonical_sync_v2_mutations -ne 2863 -or
        [bool]$Manifest.decoder_regression.sustained_fuzz_campaign_rerun_required) {
        throw 'unchanged decoder regression disposition changed'
    }

    if ([string]$Manifest.acceptance.changed_boundary_self_tests -cne 'verified' -or
        [string]$Manifest.acceptance.negative_mutations -cne 'verified' -or
        [string]$Manifest.acceptance.m1_readiness -cne '112/112' -or
        [string]$Manifest.acceptance.rustfmt -cne 'verified' -or
        [string]$Manifest.acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [string]$Manifest.acceptance.stable_rust_regression_except_unchanged_decoder_corpora -cne 'verified' -or
        [string]$Manifest.acceptance.stable_runner_profile -cne 'test' -or
        [UInt64]$Manifest.acceptance.stable_runner_cargo_jobs -ne 2 -or
        [string]$Manifest.acceptance.stable_runner_isolated_tests -cne '1/1' -or
        [string]$Manifest.acceptance.stable_runner_status -cne 'passed' -or
        [bool]$Manifest.acceptance.release_build -or
        [bool]$Manifest.acceptance.archive_created -or
        [bool]$Manifest.acceptance.git_tag_created -or
        [bool]$Manifest.acceptance.automatic_github_build) {
        throw 'aggregate acceptance evidence changed'
    }
    Assert-ExactStrings @($Manifest.acceptance.intentionally_skipped_unchanged_tests) @(
        'bounded_golden_decoder_mutation_corpus_fails_closed',
        'bounded_canonical_sync_v2_decoder_mutation_corpus_fails_closed'
    ) 'intentionally skipped unchanged tests'

    foreach ($ledger in @($Migration, $Transport, $Coverage, $HistoryCompatibility)) {
        if ([string]$ledger.milestone -cne 'M1.1.85' -or
            [string]$ledger.status -cne $status -or
            -not [bool]$ledger.changes_active_wire -or
            [string]$ledger.opt_in_dark_launch_manifest -cne
                'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json') {
            throw 'aggregate canonical ledger linkage changed'
        }
    }
    if ([string]$Migration.current_state -cne 'legacy-only' -or
        [string]$Migration.public_default -cne 'legacy-only' -or
        [string]$Transport.public_default -cne 'legacy-only' -or
        [string]$Coverage.public_default -cne 'legacy-only' -or
        [string]$HistoryCompatibility.public_default -cne 'legacy-only') {
        throw 'public default is no longer legacy-only'
    }

    if ([string]$Production.status -cne
            'canonical-sync-v2-production-runtime-opt-in-integrated' -or
        [string]$Production.public_default -cne 'legacy-only' -or
        [bool]$Production.canonical_runtime.selected_error_fallback_to_legacy) {
        throw 'production dark-launch prerequisite changed'
    }
    foreach ($needle in @(
        'CANONICAL_SYNC_V2_STREAM_TIMEOUT: Duration = Duration::from_secs(20)',
        'CANONICAL_SYNC_V2_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2 * 60)',
        'CANONICAL_SYNC_V2_RESPONSE_TIMEOUT > CANONICAL_SYNC_V2_STREAM_TIMEOUT'
    )) {
        if (-not $Runtime.Contains($needle)) {
            throw "runtime timeout source is missing '$needle'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'verified opt-in dark launch',
        'narrower than public activation',
        'public_default=legacy-only',
        'does not enable a public dual-reader default'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "opt-in acceptance RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportPath, $coveragePath,
    $historyCompatibilityPath, $productionPath, $runtimePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "opt-in dark-launch acceptance file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transport = Get-Content -LiteralPath $transportPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$historyCompatibility = Get-Content -LiteralPath $historyCompatibilityPath -Raw | ConvertFrom-Json
$production = Get-Content -LiteralPath $productionPath -Raw | ConvertFrom-Json
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transport $coverage $historyCompatibility `
    $production $runtime $rfc

if ($SelfTest) {
    $publicDefault = Copy-JsonObject $manifest
    $publicDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $publicDefault $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'public_default_activation'

    $defaultAlpn = Copy-JsonObject $manifest
    $defaultAlpn.activation_scope.default_registered_alpns += 'kilogram/m1/sync/cbor/2'
    Assert-Rejected {
        Assert-Contract $defaultAlpn $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'canonical_alpn_in_default_endpoint'

    $fallback = Copy-JsonObject $manifest
    $fallback.activation_scope.selected_canonical_failure_fallback_to_legacy = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'selected_canonical_legacy_fallback'

    $singleHost = Copy-JsonObject $manifest
    $singleHost.field_evidence.distinct_physical_hosts = $false
    Assert-Rejected {
        Assert-Contract $singleHost $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'single_host_substituted_for_field_evidence'

    $unboundedPreparation = Copy-JsonObject $manifest
    $unboundedPreparation.runtime_bounds.responder_preparation_and_first_response_timeout_seconds = 0
    Assert-Rejected {
        Assert-Contract $unboundedPreparation $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'unbounded_or_missing_response_deadline'

    $unverifiedRegression = Copy-JsonObject $manifest
    $unverifiedRegression.acceptance.stable_runner_status = 'required'
    Assert-Rejected {
        Assert-Contract $unverifiedRegression $migration $transport $coverage `
            $historyCompatibility $production $runtime $rfc
    } 'unverified_aggregate_regression'

    Write-Output 'canonical_sync_v2_opt_in_dark_launch_self_test=passed'
}

Write-Output 'canonical_sync_v2_opt_in_dark_launch_boundary=verified'
Write-Output 'status=canonical-sync-v2-opt-in-dark-launch-verified'
Write-Output 'public_default=legacy-only'
Write-Output 'changes_active_wire=true'
Write-Output 'selected_legacy_fallback=false'
