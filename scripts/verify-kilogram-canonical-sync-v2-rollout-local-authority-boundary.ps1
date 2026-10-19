[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-LOCAL-AUTHORITY.json'
$decisionPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PUBLIC-ROLLOUT-DECISION.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\lib.rs'
$cratePath = Join-Path $workspace 'crates\kilogram-rollout\Cargo.toml'
$workspaceCargoPath = Join-Path $workspace 'Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0214-m1190-rollout-local-authority.md'

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

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$PriorDecision,
        [string]$Source,
        [string]$Crate,
        [string]$WorkspaceCargo,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.90' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-rollout-local-authority-verified-dormant' -or
        [string]$Manifest.implementation -cne 'crates/kilogram-rollout' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.runtime_rollout_integration_enabled -or
        [bool]$Manifest.automatic_enrollment_enabled -or
        [bool]$Manifest.automatic_health_upload_enabled) {
        throw 'rollout local authority scope changed'
    }

    $prerequisites = @($Manifest.engineering_prerequisites)
    if ($prerequisites.Count -ne 2) {
        throw 'rollout engineering prerequisite count changed'
    }
    Assert-ExactStrings @($prerequisites | ForEach-Object { [string]$_.id }) @(
        'privacy-preserving-cohort-authority',
        'durable-local-rollout-health-evidence'
    ) 'rollout engineering prerequisites'
    foreach ($prerequisite in $prerequisites) {
        if ([string]$prerequisite.status -cne
                'implemented-and-locally-verified-dormant') {
            throw 'rollout engineering prerequisite status changed'
        }
    }

    $policy = $Manifest.release_policy
    if ([string]$policy.signature -cne
            'ed25519-with-explicitly-pinned-release-public-key' -or
        [string]$policy.encoding -cne 'bounded-canonical-postcard-v1' -or
        [string]$policy.policy_id -cne
            'sha256(domain || canonical-signed-policy-bytes)' -or
        [string]$policy.policy_chain -cne
            'epoch-plus-one-and-exact-previous-policy-id' -or
        [string]$policy.genesis -cne 'epoch-one-legacy-only-hold' -or
        [bool]$policy.phase_skip_allowed -or
        [bool]$policy.phase_rewind_allowed -or
        -not [bool]$policy.same_phase_signed_successor_allowed -or
        [string]$policy.containment -cne
            'same-or-next-phase-with-zero-enrollment' -or
        [bool]$policy.broad_default_representable -or
        [bool]$policy.production_release_key_embedded -or
        [bool]$policy.policy_distribution_implemented) {
        throw 'signed release-policy contract changed'
    }

    $cohort = $Manifest.cohort
    if ([string]$cohort.unit -cne 'local-installation-not-account-or-device' -or
        [string]$cohort.assignment -cne
            'sha256(seed || signed-release-policy-id)-first-u64-be-mod-10000' -or
        [UInt64]$cohort.buckets -ne 10000 -or
        [UInt64]$cohort.maximum_phase_basis_points.'legacy-only-hold' -ne 0 -or
        [UInt64]$cohort.maximum_phase_basis_points.'advertise-canary' -ne 100 -or
        [UInt64]$cohort.maximum_phase_basis_points.'accept-canary' -ne 100 -or
        [UInt64]$cohort.maximum_phase_basis_points.'initiate-canary' -ne 500 -or
        [UInt64]$cohort.maximum_phase_basis_points.'expanded-dual-reader' -ne 2500 -or
        [bool]$cohort.seed_returned_by_public_api -or
        [bool]$cohort.seed_in_health_export -or
        [bool]$cohort.bucket_in_health_export -or
        [bool]$cohort.account_or_device_id_input -or
        [bool]$cohort.contact_conversation_ip_relay_or_path_input -or
        -not [bool]$cohort.reinstallation_may_reassign -or
        [bool]$cohort.cross_device_assignment_shared) {
        throw 'installation cohort privacy contract changed'
    }

    $store = $Manifest.durable_local_store
    if ([string]$store.file -cne 'rollout-local-v1.redb' -or
        [string]$store.durability -cne 'redb-immediate-transaction' -or
        -not [bool]$store.release_authority_pinned -or
        -not [bool]$store.policy_head_monotonic -or
        -not [bool]$store.policy_replay_idempotent -or
        [bool]$store.database_symlink_allowed -or
        [string]$store.health_record_mac -cne
            'keyed-blake3-with-installation-seed' -or
        [bool]$store.health_record_mac_is_remote_attestation -or
        [bool]$store.whole_database_rollback_detectable_without_external_witness) {
        throw 'durable rollout local-store contract changed'
    }

    $health = $Manifest.health_aggregate
    if ([UInt64]$health.maximum_policy_records -ne 16 -or
        -not [bool]$health.only_current_policy_accepts_events -or
        [string]$health.counter_overflow -cne 'fail-closed' -or
        [bool]$health.per_peer_or_per_message_records -or
        [bool]$health.timestamps -or
        [bool]$health.message_plaintext_or_ciphertext -or
        [bool]$health.account_device_peer_contact_or_conversation_id -or
        [bool]$health.ip_relay_url_or_filesystem_path -or
        [bool]$health.cohort_seed_or_bucket -or
        [string]$health.explicit_export_marker -cne
            'user-or-test-operator-requested' -or
        [string]$health.export_format -cne 'bounded-json-aggregate' -or
        [bool]$health.automatic_export_or_upload) {
        throw 'bounded local health contract changed'
    }
    Assert-ExactStrings @($health.zero_tolerance_counters) @(
        'history-loss-or-duplication',
        'selected-canonical-legacy-fallback',
        'authority-or-device-binding-mismatch',
        'authenticated-downgrade-acceptance',
        'unrecoverable-state-corruption'
    ) 'rollout zero-tolerance counters'

    $activation = $Manifest.activation_boundary
    if ([bool]$activation.cohort_selection_grants_runtime_activation -or
        [bool]$activation.release_policy_directly_writes_device_activation_chain -or
        [bool]$activation.runtime_depends_on_kilogram_rollout -or
        -not [bool]$activation.fresh_runtime_device_authority_ceremony_still_required -or
        [string]$activation.missing_policy -cne 'legacy-only-no-selection' -or
        [string]$activation.invalid_stale_or_tampered_policy -cne 'fail-closed' -or
        [string]$activation.missing_or_tampered_health -cne
            'fail-closed-not-success') {
        throw 'rollout activation boundary changed'
    }

    $acceptance = $Manifest.acceptance
    if ([string]$acceptance.rollout_unit_tests -cne '12/12' -or
        [string]$acceptance.exact_sha256_assignment_vector -cne 'verified' -or
        [string]$acceptance.signature_tamper_and_wrong_authority_rejection -cne
            'verified' -or
        [string]$acceptance.ordered_policy_chain_and_containment -cne 'verified' -or
        [string]$acceptance.restart_seed_and_policy_stability -cne 'verified' -or
        [string]$acceptance.independent_installation_seeds -cne 'verified' -or
        [string]$acceptance.authenticated_durable_health_and_privacy_export -cne
            'verified' -or
        [string]$acceptance.hard_health_history_bound -cne 'verified' -or
        [string]$acceptance.boundary_self_test_and_negative_mutations -cne
            'verified' -or
        [string]$acceptance.m1_readiness -cne '117/117' -or
        [string]$acceptance.rustfmt -cne 'verified' -or
        [string]$acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [bool]$acceptance.release_build -or
        [bool]$acceptance.archive_created -or
        [bool]$acceptance.git_tag_created -or
        [bool]$acceptance.automatic_github_build -or
        [bool]$acceptance.network_executed) {
        throw 'rollout local authority acceptance changed'
    }

    Assert-ExactStrings @($Manifest.remaining_public_rollout_gates) @(
        'n-minus-one-production-executable-interop',
        'public-rollout-containment-drill',
        'independent-security-review-before-accept-canary'
    ) 'remaining public rollout gates'
    Assert-ExactStrings @($Manifest.next) @(
        'n-minus-one-production-executable-interop-matrix',
        'dormant-controller-containment-rehearsal',
        'production-release-key-and-policy-delivery-decision'
    ) 'rollout local authority next steps'

    if ([string]$PriorDecision.status -cne
            'canonical-sync-v2-public-rollout-decision-ready-held' -or
        [string]$PriorDecision.decision.verdict -cne 'hold' -or
        [bool]$PriorDecision.decision.eligible -or
        [string]$PriorDecision.public_default -cne 'legacy-only') {
        throw 'prior public rollout hold was weakened'
    }

    Assert-ContainsAll $Source @(
        'pub const RUNTIME_ROLLOUT_INTEGRATION_ENABLED: bool = false;',
        'pub const MAX_DURABLE_HEALTH_AGGREGATES: u64 = 16;',
        'b"kilogram:rollout:release-policy:v1\0"',
        'b"kilogram:rollout:release-policy-id:v1\0"',
        'hasher.update(seed);',
        'hasher.update(policy_id);',
        'u64::from_be_bytes',
        'RolloutPhase::LegacyOnlyHold',
        'RolloutDisposition::Containment',
        'runtime_activation_authorized: false',
        'Durability::Immediate',
        'blake3::keyed_hash(seed, &bytes)',
        'ExplicitHealthExport::UserOrTestOperatorRequested',
        'fn installation_seed_for_test',
        '#[cfg(test)]'
    ) 'rollout local authority implementation'
    foreach ($forbidden in @(
        'use kilogram_identity',
        'use kilogram_transport',
        'use iroh',
        'use reqwest',
        'pub fn installation_seed('
    )) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "rollout implementation contains forbidden surface '$forbidden'"
        }
    }
    Assert-ContainsAll $Crate @(
        'name = "kilogram-rollout"',
        'getrandom.workspace = true',
        'redb.workspace = true',
        'sha2.workspace = true'
    ) 'rollout crate manifest'
    foreach ($forbiddenDependency in @('iroh.workspace', 'reqwest.workspace', 'tokio.workspace')) {
        if ($Crate.IndexOf($forbiddenDependency, [StringComparison]::Ordinal) -ge 0) {
            throw "rollout crate gained network dependency '$forbiddenDependency'"
        }
    }
    if ($WorkspaceCargo.IndexOf('"crates/kilogram-rollout"',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'rollout crate is absent from the workspace'
    }

    $rolloutConsumers = @(Get-ChildItem -LiteralPath (Join-Path $workspace 'apps'),
        (Join-Path $workspace 'crates') -Recurse -Filter 'Cargo.toml' |
        Where-Object { $_.FullName -cne $cratePath } |
        Where-Object {
            (Get-Content -LiteralPath $_.FullName -Raw).IndexOf(
                'kilogram-rollout', [StringComparison]::Ordinal) -ge 0
        })
    $expectedReadOnlyObserver = [IO.Path]::GetFullPath(
        (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'))
    if ($rolloutConsumers.Count -ne 1 -or
        [IO.Path]::GetFullPath($rolloutConsumers[0].FullName) -cne
            $expectedReadOnlyObserver) {
        throw 'kilogram-rollout has a consumer beyond the bounded CLI observer'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'implementation is dormant and public default remains `legacy-only`',
        'cannot enroll a user or change active wire behaviour',
        '`runtime_activation_authorized=false`',
        'cannot skip or rewind',
        'It contains no seed, bucket, timestamp, local path or user identifier',
        'There is no uploader, endpoint or telemetry service',
        'coordinated rollback of the whole database needs an external or hardware witness'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "rollout local authority RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $decisionPath, $sourcePath, $cratePath, $workspaceCargoPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "rollout local authority file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$priorDecision = Get-Content -LiteralPath $decisionPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$crate = Get-Content -LiteralPath $cratePath -Raw
$workspaceCargo = Get-Content -LiteralPath $workspaceCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $priorDecision $source $crate $workspaceCargo $rfc

if ($SelfTest) {
    $publicDefault = Copy-JsonObject $manifest
    $publicDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $publicDefault $priorDecision $source $crate $workspaceCargo $rfc
    } 'public_default_activated'

    $runtimeIntegration = Copy-JsonObject $manifest
    $runtimeIntegration.runtime_rollout_integration_enabled = $true
    Assert-Rejected {
        Assert-Contract $runtimeIntegration $priorDecision $source $crate $workspaceCargo $rfc
    } 'runtime_integration_enabled'

    $stableIdentity = Copy-JsonObject $manifest
    $stableIdentity.cohort.account_or_device_id_input = $true
    Assert-Rejected {
        Assert-Contract $stableIdentity $priorDecision $source $crate $workspaceCargo $rfc
    } 'stable_identity_cohort_input'

    $seedExport = Copy-JsonObject $manifest
    $seedExport.cohort.seed_in_health_export = $true
    Assert-Rejected {
        Assert-Contract $seedExport $priorDecision $source $crate $workspaceCargo $rfc
    } 'seed_exported'

    $automaticUpload = Copy-JsonObject $manifest
    $automaticUpload.health_aggregate.automatic_export_or_upload = $true
    Assert-Rejected {
        Assert-Contract $automaticUpload $priorDecision $source $crate $workspaceCargo $rfc
    } 'automatic_health_upload'

    $unbounded = Copy-JsonObject $manifest
    $unbounded.health_aggregate.maximum_policy_records = 0
    Assert-Rejected {
        Assert-Contract $unbounded $priorDecision $source $crate $workspaceCargo $rfc
    } 'unbounded_health_history'

    $activation = Copy-JsonObject $manifest
    $activation.activation_boundary.cohort_selection_grants_runtime_activation = $true
    Assert-Rejected {
        Assert-Contract $activation $priorDecision $source $crate $workspaceCargo $rfc
    } 'cohort_grants_activation'

    $broadDefault = Copy-JsonObject $manifest
    $broadDefault.release_policy.broad_default_representable = $true
    Assert-Rejected {
        Assert-Contract $broadDefault $priorDecision $source $crate $workspaceCargo $rfc
    } 'broad_default_representable'

    $rollbackOverclaim = Copy-JsonObject $manifest
    $rollbackOverclaim.durable_local_store.whole_database_rollback_detectable_without_external_witness = $true
    Assert-Rejected {
        Assert-Contract $rollbackOverclaim $priorDecision $source $crate $workspaceCargo $rfc
    } 'whole_database_rollback_overclaim'

    Write-Output 'canonical_sync_v2_rollout_local_authority_self_test=passed'
}

Write-Output 'canonical_sync_v2_rollout_local_authority_boundary=verified'
Write-Output 'public_default=legacy-only'
Write-Output 'runtime_rollout_integration_enabled=false'
Write-Output 'automatic_health_upload_enabled=false'
Write-Output 'remaining_public_rollout_gates=3'
Write-Output 'network_executed=false'
