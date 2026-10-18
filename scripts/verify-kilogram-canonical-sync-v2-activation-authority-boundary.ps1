[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'
$policyPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$authorityPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_activation.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$statePath = Join-Path $workspace 'crates\kilogram-state\src\lib.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0206-m1182-durable-canonical-sync-v2-activation-authority.md'

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

function Assert-ContainsAll {
    param([string]$Text, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if (-not $Text.Contains($needle)) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Policy,
        [object]$Migration,
        [string]$Authority,
        [string]$Cli,
        [string]$State,
        [string]$Transport,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.82' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-durable-local-activation-authority-verified' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'activation authority manifest header changed'
    }

    $authorityContract = $Manifest.authority
    if ([string]$authorityContract.source -cne 'device-signed-append-only-chain' -or
        [UInt64]$authorityContract.record_version -ne 1 -or
        [string]$authorityContract.record_magic -cne 'KILOGRAM-CSAP-V1' -or
        [string]$authorityContract.signature -cne 'ed25519-device' -or
        -not [bool]$authorityContract.device_bound -or
        [UInt64]$authorityContract.first_epoch -ne 1 -or
        [UInt64]$authorityContract.epoch_step -ne 1 -or
        -not [bool]$authorityContract.exact_previous_record_binding -or
        [UInt64]$authorityContract.reason_min_bytes -ne 1 -or
        [UInt64]$authorityContract.reason_max_bytes -ne 256 -or
        -not [bool]$authorityContract.reason_trimmed_control_free -or
        [bool]$authorityContract.environment_variable_authority -or
        [bool]$authorityContract.hidden_permissive_flag_authority) {
        throw 'activation authority record contract changed'
    }
    Assert-ExactStrings @($authorityContract.actors) @(
        'explicit-local',
        'release-policy'
    ) 'activation actors'

    $highWater = $Manifest.high_water
    if ([UInt64]$highWater.version -ne 1 -or
        [string]$highWater.magic -cne 'KILOGRAM-CSHW-V1' -or
        [string]$highWater.signature -cne 'ed25519-device' -or
        -not [bool]$highWater.atomic_replace -or
        -not [bool]$highWater.exact_epoch_and_head_binding -or
        [bool]$highWater.chain_without_high_water_activates_canonical -or
        [bool]$highWater.high_water_without_chain_activates_canonical -or
        [bool]$highWater.mismatched_high_water_activates_canonical -or
        -not [bool]$highWater.interrupted_tail_repair_requires_exact_planned_record_id) {
        throw 'activation high-water contract changed'
    }

    $transitions = $Manifest.transitions
    if ([string]$transitions.forward -cne 'exactly-one-state' -or
        [string]$transitions.pre_evidence_rollback -cne 'exactly-one-state' -or
        [string]$transitions.post_evidence_rollback -cne 'forbidden' -or
        [string]$transitions.containment -cne 'same-state-with-exact-evidence' -or
        [string]$transitions.exit_containment -cne 'same-state-preserve-exact-evidence' -or
        [string]$transitions.first_explicit_state -cne 'advertise-cbor-v2' -or
        [string]$transitions.epoch_skip -cne 'rejected' -or
        [string]$transitions.fork -cne 'rejected' -or
        [string]$transitions.replay -cne 'rejected' -or
        [string]$transitions.wrong_device -cne 'rejected' -or
        [string]$transitions.canonical_evidence_change_or_removal -cne 'rejected') {
        throw 'activation transition contract changed'
    }

    if ([string]$Manifest.commands.status.name -cne 'canonical-sync-policy-status' -or
        [bool]$Manifest.commands.status.writes_state -or
        [string]$Manifest.commands.status.missing_chain_and_high_water -cne
            'implicit-legacy-only' -or
        [string]$Manifest.commands.plan.name -cne 'canonical-sync-policy-plan' -or
        [bool]$Manifest.commands.plan.writes_state -or
        -not [bool]$Manifest.commands.plan.prints_exact_candidate_record_id -or
        [string]$Manifest.commands.apply.name -cne 'canonical-sync-policy-apply' -or
        -not [bool]$Manifest.commands.apply.requires_exact_candidate_record_id -or
        -not [bool]$Manifest.commands.apply.append_only_record -or
        -not [bool]$Manifest.commands.apply.vault_primary_transaction -or
        -not [bool]$Manifest.commands.apply.publishes_signed_high_water_after_record_commit) {
        throw 'activation command contract changed'
    }

    foreach ($property in @(
        'malformed_record', 'invalid_signature', 'stale_expected_record_id',
        'forked_chain', 'epoch_gap', 'wrong_device',
        'truncated_tail_against_retained_high_water'
    )) {
        if ([string]$Manifest.failure_policy.$property -cne 'fail-closed') {
            throw "activation failure policy '$property' changed"
        }
    }
    if ([string]$Manifest.failure_policy.missing_authority -cne 'implicit-legacy-only') {
        throw 'missing activation authority no longer maps to the safe default'
    }

    if (-not [bool]$Manifest.anti_rollback_scope.detects_record_tamper_fork_gap_and_retained_high_water_tail_rollback -or
        [bool]$Manifest.anti_rollback_scope.detects_coordinated_whole_directory_and_witness_rollback -or
        -not [bool]$Manifest.anti_rollback_scope.external_or_hardware_witness_required_for_coordinated_rollback) {
        throw 'activation anti-rollback scope overclaims or regressed'
    }

    foreach ($property in @(
        'restart', 'interrupted_record_before_high_water', 'replay', 'epoch_skip',
        'fork', 'wrong_device', 'filesystem_cli_lifecycle',
        'vault_primary_cli_lifecycle'
    )) {
        if (-not [bool]$Manifest.verification.$property) {
            throw "activation verification '$property' is not closed"
        }
    }
    if ([bool]$Manifest.verification.network_executed) {
        throw 'activation authority stage unexpectedly claims network evidence'
    }

    if (-not [bool]$Policy.activation.semantic_policy_integrated -or
        -not [bool]$Policy.activation.durable_local_authority_integrated -or
        [bool]$Policy.activation.active_runtime_integrated -or
        -not [bool]$Manifest.activation.semantic_policy_integrated -or
        -not [bool]$Manifest.activation.durable_local_authority_integrated -or
        [bool]$Manifest.activation.active_runtime_integrated -or
        [bool]$Manifest.activation.candidate_endpoint_registered_by_default -or
        [bool]$Manifest.activation.candidate_dialed_by_default) {
        throw 'activation integration stage boundary changed'
    }
    Assert-ExactStrings @($Manifest.next) @(
        'production-runtime-dark-launch-integration',
        'local-and-two-host-activation-rehearsal',
        'aggregate-regression-and-formalization'
    ) 'activation authority next work'

    if ([string]$Migration.current_state -cne 'legacy-only' -or
        -not [bool]$Migration.changes_active_wire -or
        [string]$Migration.public_default -cne 'legacy-only' -or
        [bool]$Migration.opt_in_dark_launch.registered_by_default -or
        [bool]$Migration.opt_in_dark_launch.dialed_by_default -or
        -not [bool]$Migration.opt_in_dark_launch.registered_by_verified_opt_in -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'public canonical migration changed during authority-only work'
    }

    Assert-ContainsAll $Authority @(
        'pub enum CanonicalSyncV2ActivationActor',
        'pub struct SignedCanonicalSyncV2ActivationRecord',
        'pub struct SignedCanonicalSyncV2ActivationHighWater',
        'MAX_CANONICAL_SYNC_V2_ACTIVATION_REASON_BYTES: usize = 256',
        'pub fn verify_canonical_sync_v2_activation_chain(',
        'pub fn verify_canonical_sync_v2_activation_authority(',
        'HighWaterMissing',
        'HighWaterMismatch',
        'PreviousRecordMismatch',
        'RollbackAfterCanonicalEvidence',
        'signed_chain_round_trips_and_survives_restart',
        'authority_rejects_interrupted_tail_until_exact_high_water_is_published',
        'chain_rejects_replay_epoch_skip_fork_and_wrong_device',
        'high_water_rejects_truncated_or_replaced_chain_head'
    ) 'activation authority source'
    if ($Authority.Contains('std::env') -or $Authority.Contains('var_os(')) {
        throw 'activation authority source gained environment-variable authority'
    }

    Assert-ContainsAll $Cli @(
        'CanonicalSyncPolicyStatus',
        'CanonicalSyncPolicyPlan',
        'CanonicalSyncPolicyApply',
        'expected_record_id: String',
        'RUNTIME_CANONICAL_SYNC_V2_ACTIVATION_DIRECTORY',
        'persist_runtime_record(&state_dir, &relative_path, &encoded, transaction)',
        'publish_canonical_sync_v2_activation_high_water',
        'activation plan changed; refusing stale or altered apply',
        'status=canonical-sync-v2-activation-authority-verified',
        'status=canonical-sync-v2-activation-applied',
        'status=canonical-sync-v2-activation-repaired'
    ) 'activation CLI source'
    Assert-ContainsAll $State @(
        '"runtime",',
        'Some(Component::Normal(first)) if first == "runtime" => StateRecordKind::Runtime'
    ) 'state append-only/vault source'

    if ($Transport.Contains('SignedCanonicalSyncV2ActivationRecord') -or
        $Transport.Contains('verify_canonical_sync_v2_activation_authority')) {
        throw 'durable authority leaked into active Iroh transport prematurely'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'still exactly `legacy-only`, epoch zero, normal disposition',
        'environment variable, a startup convenience flag, a malformed file',
        'requires the operator to provide that exact ID',
        'crash after the record commit but before high-water publication',
        'coordinated rollback of the complete state directory',
        'does not add `/cbor/2` to an ordinary endpoint'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "activation authority RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $policyPath, $migrationPath, $authorityPath, $cliPath,
    $statePath, $transportPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "activation authority file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$authority = Get-Content -LiteralPath $authorityPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$state = Get-Content -LiteralPath $statePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $policy $migration $authority $cli $state $transport $rfc

if ($SelfTest) {
    $unsigned = Copy-JsonObject $manifest
    $unsigned.authority.signature = 'none'
    Assert-Rejected {
        Assert-Contract $unsigned $policy $migration $authority $cli $state $transport $rfc
    } 'unsigned_activation_record'

    $skip = Copy-JsonObject $manifest
    $skip.authority.epoch_step = 2
    Assert-Rejected {
        Assert-Contract $skip $policy $migration $authority $cli $state $transport $rfc
    } 'epoch_skip'

    $permissive = Copy-JsonObject $manifest
    $permissive.high_water.chain_without_high_water_activates_canonical = $true
    Assert-Rejected {
        Assert-Contract $permissive $policy $migration $authority $cli $state $transport $rfc
    } 'missing_high_water_activation'

    $overclaim = Copy-JsonObject $manifest
    $overclaim.anti_rollback_scope.detects_coordinated_whole_directory_and_witness_rollback = $true
    Assert-Rejected {
        Assert-Contract $overclaim $policy $migration $authority $cli $state $transport $rfc
    } 'coordinated_rollback_overclaim'

    $active = Copy-JsonObject $manifest
    $active.activation.active_runtime_integrated = $true
    Assert-Rejected {
        Assert-Contract $active $policy $migration $authority $cli $state $transport $rfc
    } 'premature_runtime_activation'

    Assert-Rejected {
        Assert-Contract $manifest $policy $migration ($authority + ' std::env') `
            $cli $state $transport $rfc
    } 'environment_variable_authority'

    Write-Output 'canonical_sync_v2_activation_authority_self_test=passed'
}

Write-Output 'canonical_sync_v2_activation_authority_boundary=verified'
Write-Output 'authority_source=device-signed-append-only-chain'
Write-Output 'missing_authority=implicit-legacy-only'
Write-Output 'malformed_or_stale_authority=fail-closed'
Write-Output 'coordinated_whole_directory_rollback_detected=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
