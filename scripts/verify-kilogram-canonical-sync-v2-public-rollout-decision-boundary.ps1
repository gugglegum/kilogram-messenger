[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$decisionPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PUBLIC-ROLLOUT-DECISION.json'
$policyPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$authorityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'
$optInPath = Join-Path $workspace 'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json'
$networkPath = Join-Path $workspace 'CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DECODER-MUTATIONS.json'
$fuzzPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json'
$observabilityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RUNTIME-OBSERVABILITY.json'
$restartPath = Join-Path $workspace 'CANONICAL-SYNC-V2-GUIDED-RESTART.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0213-m1189-canonical-sync-v2-public-rollout-decision.md'

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
        [object]$Decision,
        [object]$Policy,
        [object]$Authority,
        [object]$OptIn,
        [object]$Network,
        [object]$Mutations,
        [object]$Fuzz,
        [object]$Observability,
        [object]$Restart,
        [object]$Coverage,
        [string]$Rfc
    )

    if ([UInt64]$Decision.schema_version -ne 1 -or
        [string]$Decision.milestone -cne 'M1.1.89' -or
        [string]$Decision.status -cne
            'canonical-sync-v2-public-rollout-decision-ready-held' -or
        [bool]$Decision.changes_peer_network_wire -or
        [bool]$Decision.changes_sync_wire -or
        [bool]$Decision.changes_runtime_ipc -or
        [bool]$Decision.changes_public_default -or
        [string]$Decision.public_default -cne 'legacy-only') {
        throw 'public rollout decision header changed'
    }

    $verdict = $Decision.decision
    if ([string]$verdict.scope -cne
            'automatic-public-canonical-sync-v2-rollout' -or
        [string]$verdict.verdict -cne 'hold' -or
        [bool]$verdict.eligible -or
        [string]$verdict.current_state -cne 'legacy-only' -or
        [string]$verdict.first_automatic_state_when_eligible -cne
            'advertise-cbor-v2' -or
        [string]$verdict.first_session_accepting_state -cne
            'dual-reader-prefer-legacy' -or
        [string]$verdict.first_session_initiating_state -cne
            'dual-reader-prefer-canonical' -or
        [bool]$verdict.canonical_required_authorized -or
        [bool]$verdict.runtime_consumes_this_manifest -or
        -not [bool]$verdict.human_release_decision_required_for_each_phase -or
        -not [bool]$verdict.missing_unknown_or_stale_gate_means_hold -or
        [bool]$verdict.weighted_score_or_override_allowed) {
        throw 'public rollout verdict changed'
    }

    $expectedEvidence = @(
        @('activation-policy-and-authority',
            'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json',
            'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'),
        @('production-opt-in-dark-launch',
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json'),
        @('two-host-production-resilience',
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json',
            'CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json'),
        @('bounded-and-sustained-decoder-testing',
            'CANONICAL-SYNC-V2-DECODER-MUTATIONS.json',
            'CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json'),
        @('desktop-exact-restart-activation',
            'CANONICAL-SYNC-V2-RUNTIME-OBSERVABILITY.json',
            'CANONICAL-SYNC-V2-GUIDED-RESTART.json')
    )
    $actualEvidence = @($Decision.accepted_evidence)
    if ($actualEvidence.Count -ne $expectedEvidence.Count) {
        throw 'accepted rollout evidence count changed'
    }
    for ($index = 0; $index -lt $expectedEvidence.Count; $index++) {
        if ([string]$actualEvidence[$index].id -cne $expectedEvidence[$index][0] -or
            [string]$actualEvidence[$index].status -cne 'verified') {
            throw "accepted rollout evidence $index changed"
        }
        Assert-ExactStrings @($actualEvidence[$index].artifacts) `
            @($expectedEvidence[$index] | Select-Object -Skip 1) `
            "accepted rollout evidence artifacts $index"
    }

    $expectedGates = @(
        @('privacy-preserving-cohort-authority', 'advertise-canary'),
        @('durable-local-rollout-health-evidence', 'advertise-canary'),
        @('n-minus-one-production-executable-interop', 'advertise-canary'),
        @('public-rollout-containment-drill', 'advertise-canary'),
        @('independent-security-review', 'accept-canary')
    )
    $actualGates = @($Decision.mandatory_gates)
    if ($actualGates.Count -ne $expectedGates.Count) {
        throw 'mandatory rollout gate count changed'
    }
    for ($index = 0; $index -lt $expectedGates.Count; $index++) {
        if ([string]$actualGates[$index].id -cne $expectedGates[$index][0] -or
            [string]$actualGates[$index].status -cne 'pending' -or
            [string]$actualGates[$index].required_before_phase -cne
                $expectedGates[$index][1]) {
            throw "mandatory rollout gate $index changed"
        }
    }
    $pendingGates = @($actualGates | Where-Object {
        [string]$_.status -ceq 'pending'
    } | ForEach-Object { [string]$_.id })
    Assert-ExactStrings @($Decision.blocking_gates) $pendingGates `
        'blocking rollout gates'
    if ($pendingGates.Count -eq 0 -or [bool]$verdict.eligible -or
        [string]$verdict.verdict -cne 'hold') {
        throw 'pending rollout gates do not force hold'
    }

    $cohort = $Decision.cohort_contract
    if ([string]$cohort.assignment_seed -cne
            'local-installation-random-secret' -or
        [string]$cohort.assignment_function -cne
            'sha256(seed || signed-release-policy-id)-mod-10000' -or
        [string]$cohort.assignment_unit -cne
            'installation-not-account-or-device' -or
        [bool]$cohort.seed_exported -or
        [bool]$cohort.seed_synced_between_devices -or
        [bool]$cohort.seed_visible_to_peers -or
        [bool]$cohort.account_root_or_device_id_used -or
        [bool]$cohort.contact_conversation_ip_or_relay_used -or
        [bool]$cohort.phase_changes_while_runtime_is_running -or
        [bool]$cohort.cohort_expands_automatically) {
        throw 'privacy-preserving cohort contract changed'
    }

    $expectedPhases = @(
        @('advertise-canary', 'advertise-cbor-v2', 100, 168, $false, $false,
            1000, 1000, 0),
        @('accept-canary', 'dual-reader-prefer-legacy', 100, 336, $true, $false,
            1000, 1000, 250),
        @('initiate-canary', 'dual-reader-prefer-canonical', 500, 336, $true,
            $true, 2000, 5000, 1000),
        @('expanded-dual-reader', 'dual-reader-prefer-canonical', 2500, 672,
            $true, $true, 10000, 50000, 10000),
        @('broad-dual-reader-default', 'dual-reader-prefer-canonical', 10000,
            672, $true, $true, 0, 0, 0)
    )
    $phases = @($Decision.rollout_phases)
    if ($phases.Count -ne $expectedPhases.Count) {
        throw 'rollout phase count changed'
    }
    for ($index = 0; $index -lt $expectedPhases.Count; $index++) {
        $phase = $phases[$index]
        $expected = $expectedPhases[$index]
        if ([string]$phase.id -cne $expected[0] -or
            [string]$phase.state -cne $expected[1] -or
            [UInt64]$phase.maximum_cohort_basis_points -ne [UInt64]$expected[2] -or
            [UInt64]$phase.minimum_soak_hours -ne [UInt64]$expected[3] -or
            [bool]$phase.accepts_new_canonical_sessions -ne [bool]$expected[4] -or
            [bool]$phase.initiates_new_canonical_sessions -ne [bool]$expected[5] -or
            [UInt64]$phase.minimum_runtime_starts -ne [UInt64]$expected[6] -or
            [UInt64]$phase.minimum_sync_attempts -ne [UInt64]$expected[7] -or
            [UInt64]$phase.minimum_canonical_sessions -ne [UInt64]$expected[8] -or
            -not [bool]$phase.separate_promotion_decision) {
            throw "rollout phase $index changed"
        }
    }

    $promotion = $Decision.promotion_contract
    if ([bool]$promotion.phases_may_be_skipped -or
        -not [bool]$promotion.broad_default_requires_new_decision_manifest -or
        [bool]$promotion.missing_denominator_is_zero_failures -or
        [UInt64]$promotion.minimum_canonical_completion_basis_points -ne 9950 -or
        [UInt64]$promotion.maximum_failure_delta_from_legacy_baseline_basis_points -ne 50 -or
        [UInt64]$promotion.maximum_runtime_start_failure_delta_basis_points -ne 25) {
        throw 'rollout promotion contract changed'
    }
    Assert-ExactStrings @($promotion.zero_tolerance_events) @(
        'history-loss-or-duplication',
        'selected-canonical-legacy-fallback',
        'authority-or-device-binding-mismatch',
        'authenticated-downgrade-acceptance',
        'unrecoverable-state-corruption'
    ) 'zero-tolerance rollout events'

    $health = $Decision.health_evidence_privacy
    if ([string]$health.storage -cne 'bounded-local-aggregate-only' -or
        [string]$health.export -cne 'explicit-user-or-test-operator-action-only' -or
        [bool]$health.message_plaintext_or_ciphertext -or
        [bool]$health.account_root_or_device_identifier -or
        [bool]$health.peer_contact_or_conversation_identifier -or
        [bool]$health.ip_relay_url_or_filesystem_path -or
        [bool]$health.per_message_or_per_peer_timeline -or
        [bool]$health.missing_or_nonexported_evidence_counts_as_success) {
        throw 'rollout health-evidence privacy changed'
    }

    $rollback = $Decision.containment_and_rollback
    if (-not [bool]$rollback.new_enrollment_stops_first -or
        -not [bool]$rollback.new_canonical_accept_and_initiate_disabled -or
        -not [bool]$rollback.canonical_reader_retained_for_selected_resume -or
        [bool]$rollback.durable_canonical_evidence_deleted -or
        [bool]$rollback.selected_canonical_failure_falls_back_to_legacy -or
        [bool]$rollback.forced_state_rewind_after_canonical_evidence -or
        -not [bool]$rollback.one_step_rollback_before_canonical_evidence_only -or
        -not [bool]$rollback.fresh_runtime_exact_verification_required -or
        -not [bool]$rollback.rollback_or_containment_requires_signed_release_or_explicit_local_authority) {
        throw 'rollout containment and rollback contract changed'
    }

    if ([string]$Decision.acceptance.decision_boundary_self_test_and_negative_mutations -cne
            'verified' -or
        [string]$Decision.acceptance.m1_readiness -cne '116/116' -or
        [string]$Decision.acceptance.rustfmt -cne 'verified' -or
        [string]$Decision.acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [bool]$Decision.acceptance.release_build -or
        [bool]$Decision.acceptance.archive_created -or
        [bool]$Decision.acceptance.git_tag_created -or
        [bool]$Decision.acceptance.automatic_github_build -or
        [bool]$Decision.acceptance.network_executed) {
        throw 'rollout decision acceptance changed'
    }
    Assert-ExactStrings @($Decision.next) @(
        'privacy-preserving-rollout-cohort-authority',
        'durable-local-rollout-health-evidence',
        'n-minus-one-production-interoperability',
        'public-rollout-containment-rehearsal'
    ) 'rollout decision next steps'
    Assert-ExactStrings @($Decision.non_claims) @(
        'public-dual-reader-default-enabled',
        'automatic-cohort-enrollment-implemented',
        'rollout-health-telemetry-implemented',
        'canonical-native-history-materialization',
        'legacy-retirement',
        'independent-security-review-completed'
    ) 'rollout decision non-claims'

    if ([string]$Policy.status -cne
            'canonical-sync-v2-activation-semantics-verified-not-active' -or
        [string]$Policy.public_default -cne 'legacy-only' -or
        [string]$Authority.status -cne
            'canonical-sync-v2-durable-local-activation-authority-verified' -or
        [string]$Authority.public_default -cne 'legacy-only' -or
        [string]$OptIn.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$OptIn.public_default -cne 'legacy-only' -or
        -not [bool]$OptIn.field_evidence.distinct_physical_hosts -or
        [string]$Network.status -cne
            'canonical-sync-v2-ready-for-explicit-activation-decision' -or
        [UInt64]$Mutations.expected_mutations -ne 2863 -or
        [UInt64]$Fuzz.run.executions -ne 469853771 -or
        -not [bool]$Fuzz.run.coverage_generated -or
        [string]$Observability.status -cne
            'canonical-sync-v2-runtime-observability-verified' -or
        [string]$Observability.public_default -cne 'legacy-only' -or
        [string]$Restart.status -cne
            'canonical-sync-v2-guided-restart-verified' -or
        [string]$Restart.public_default -cne 'legacy-only') {
        throw 'accepted rollout prerequisite evidence changed'
    }

    if ([string]$Coverage.public_default -cne 'legacy-only' -or
        (@($Coverage.next_priority) -join ',') -cne
            'privacy-preserving-rollout-cohort-authority,durable-local-rollout-health-evidence,first-contact-ticket-extension-stripping-resolution,public-rollout-containment-rehearsal,canonical-native-history-materialization,legacy-sync-retirement-prerequisites') {
        throw 'canonical coverage rollout priority changed'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'decision is `hold`',
        '`eligible=false`',
        '`public_default=legacy-only`',
        'There is no weighted score, override',
        'cannot be skipped',
        'Containment is not erasure',
        'Runtime does not consume it'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "public rollout decision RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $decisionPath, $policyPath, $authorityPath, $optInPath, $networkPath,
    $mutationsPath, $fuzzPath, $observabilityPath, $restartPath, $coveragePath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "public rollout decision file is missing: $path"
    }
}

$decision = Get-Content -LiteralPath $decisionPath -Raw | ConvertFrom-Json
$policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
$authority = Get-Content -LiteralPath $authorityPath -Raw | ConvertFrom-Json
$optIn = Get-Content -LiteralPath $optInPath -Raw | ConvertFrom-Json
$network = Get-Content -LiteralPath $networkPath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$fuzz = Get-Content -LiteralPath $fuzzPath -Raw | ConvertFrom-Json
$observability = Get-Content -LiteralPath $observabilityPath -Raw | ConvertFrom-Json
$restart = Get-Content -LiteralPath $restartPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $decision $policy $authority $optIn $network $mutations $fuzz `
    $observability $restart $coverage $rfc

if ($SelfTest) {
    $activatedDefault = Copy-JsonObject $decision
    $activatedDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $activatedDefault $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'public_default_activated_without_decision'

    $falseEligibility = Copy-JsonObject $decision
    $falseEligibility.decision.eligible = $true
    Assert-Rejected {
        Assert-Contract $falseEligibility $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'pending_gates_marked_eligible'

    $hiddenBlocker = Copy-JsonObject $decision
    $hiddenBlocker.blocking_gates = @($hiddenBlocker.blocking_gates | Select-Object -Skip 1)
    Assert-Rejected {
        Assert-Contract $hiddenBlocker $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'pending_gate_hidden_from_blockers'

    $stableIdentifier = Copy-JsonObject $decision
    $stableIdentifier.cohort_contract.account_root_or_device_id_used = $true
    Assert-Rejected {
        Assert-Contract $stableIdentifier $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'stable_identity_used_for_cohort'

    $automaticExpansion = Copy-JsonObject $decision
    $automaticExpansion.cohort_contract.cohort_expands_automatically = $true
    Assert-Rejected {
        Assert-Contract $automaticExpansion $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'automatic_cohort_expansion'

    $phaseSkip = Copy-JsonObject $decision
    $phaseSkip.promotion_contract.phases_may_be_skipped = $true
    Assert-Rejected {
        Assert-Contract $phaseSkip $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'rollout_phase_skip'

    $fallback = Copy-JsonObject $decision
    $fallback.containment_and_rollback.selected_canonical_failure_falls_back_to_legacy = $true
    Assert-Rejected {
        Assert-Contract $fallback $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'selected_canonical_legacy_fallback'

    $broadWithoutDecision = Copy-JsonObject $decision
    $broadWithoutDecision.promotion_contract.broad_default_requires_new_decision_manifest = $false
    Assert-Rejected {
        Assert-Contract $broadWithoutDecision $policy $authority $optIn $network `
            $mutations $fuzz $observability $restart $coverage $rfc
    } 'broad_default_without_new_decision'

    Write-Output 'canonical_sync_v2_public_rollout_decision_self_test=passed'
}

Write-Output 'canonical_sync_v2_public_rollout_decision_boundary=verified'
Write-Output 'decision=hold'
Write-Output 'eligible=false'
Write-Output 'public_default=legacy-only'
Write-Output 'blocking_gates=5'
Write-Output 'network_executed=false'
