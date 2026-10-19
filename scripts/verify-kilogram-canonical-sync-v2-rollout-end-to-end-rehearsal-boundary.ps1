[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-REHEARSAL.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0229-m1205-rollout-end-to-end-rehearsal.md'
$portabilityRfcPath = Join-Path $workspace 'docs\RFC-0222-m1198-exact-binary-two-host-portability.md'
$rehearsalPath = Join-Path $workspace 'crates\kilogram-rollout\src\end_to_end_rehearsal.rs'
$healthPath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_health_store.rs'
$policyStorePath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_policy_store.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$fieldPath = Join-Path $workspace 'scripts\invoke-kilogram-rollout-two-host-portability.ps1'
$evidencePath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json'

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
        [string]$Rfc,
        [string]$PortabilityRfc,
        [string]$Rehearsal,
        [string]$Health,
        [string]$PolicyStore,
        [string]$Cli,
        [string]$Field,
        [object]$Evidence
    )
    if ([string]$Manifest.milestone -cne 'M1.2.5' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-rollout-end-to-end-two-host-verified' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.control_plane.offline_root_anchor -or
        -not [bool]$Manifest.control_plane.operational_key_rotation -or
        -not [bool]$Manifest.control_plane.complete_policy_chain_required -or
        -not [bool]$Manifest.control_plane.trusted_health_rotation_aware -or
        -not [bool]$Manifest.control_plane.explicit_device_approval -or
        -not [bool]$Manifest.control_plane.emergency_containment -or
        -not [bool]$Manifest.control_plane.explicit_device_recovery -or
        [bool]$Manifest.control_plane.automatic_enrollment -or
        [bool]$Manifest.control_plane.network_triggered_apply -or
        -not [bool]$Manifest.crash_boundary.record_before_high_water_repair -or
        -not [bool]$Manifest.crash_boundary.exact_device_signed_artifact_required -or
        -not [bool]$Manifest.crash_boundary.current_policy_health_and_predecessor_rechecked -or
        -not [bool]$Manifest.crash_boundary.altered_candidate_rejected -or
        [string]$Manifest.acceptance.rollout_unit_tests -cne '56/56' -or
        [string]$Manifest.acceptance.named_end_to_end_rehearsal -cne 'verified-local-and-remote' -or
        [string]$Manifest.acceptance.cli_interrupted_apply_repair -cne 'verified-local' -or
        [string]$Manifest.acceptance.two_host_exact_binary_rehearsal -cne 'verified' -or
        [string]$Manifest.acceptance.two_host_evidence -cne 'CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json' -or
        [string]$Manifest.acceptance.implementation_revision -cne '2f005c124a8333af4cd7a09bd36dee93cb1111a8' -or
        [string]$Manifest.acceptance.boundary_self_test -cne 'verified' -or
        [string]$Manifest.acceptance.rustfmt -cne 'verified' -or
        [string]$Manifest.acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [string]$Manifest.acceptance.stable_full_workspace_regression -cne 'verified' -or
        [string]$Manifest.acceptance.aggregate_readiness -cne '131/131' -or
        [string]$Manifest.acceptance.legacy_decoder_mutations -cne '31851/31851' -or
        [string]$Manifest.acceptance.canonical_sync_v2_decoder_mutations -cne '2863/2863' -or
        [bool]$Manifest.acceptance.release_build -or
        [bool]$Manifest.acceptance.archive_created -or
        [bool]$Manifest.acceptance.git_tag_created) {
        throw 'Rollout end-to-end security boundary changed'
    }
    if ([int]$Evidence.schema_version -ne 1 -or
        [string]$Evidence.milestone -cne 'M1.2.5' -or
        [string]$Evidence.status -cne 'canonical-sync-v2-rollout-end-to-end-two-host-verified' -or
        [string]$Evidence.run_id -cne 'm1205-rollout-20261019T071251Z' -or
        [string]$Evidence.source_revision -cne '2f005c124a8333af4cd7a09bd36dee93cb1111a8' -or
        [string]$Evidence.public_default -cne 'legacy-only' -or
        [bool]$Evidence.changes_peer_network_wire -or
        [bool]$Evidence.changes_sync_wire -or
        [bool]$Evidence.changes_runtime_ipc -or
        [bool]$Evidence.changes_public_default -or
        [string]$Evidence.build.rollout_test_harness_profile -cne 'verification' -or
        [string]$Evidence.build.cli_profile -cne 'debug' -or
        [string]$Evidence.build.target -cne 'x86_64-pc-windows-msvc' -or
        [bool]$Evidence.build.release_build -or
        [bool]$Evidence.build.archive_created -or
        [string]$Evidence.artifacts.rollout_test_harness.stable_name -cne 'kilogram-rollout-tests.exe' -or
        [string]$Evidence.artifacts.rollout_test_harness.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        [int64]$Evidence.artifacts.rollout_test_harness.bytes -le 0 -or
        [string]$Evidence.artifacts.cli.stable_name -cne 'kilogram-cli.exe' -or
        [string]$Evidence.artifacts.cli.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        [int64]$Evidence.artifacts.cli.bytes -le 0 -or
        @($Evidence.hosts).Count -ne 2 -or
        -not [bool]$Evidence.transport.ssh_control_channel_only -or
        [bool]$Evidence.transport.p2p_network_executed -or
        [bool]$Evidence.transport.listening_socket_created -or
        [bool]$Evidence.transport.firewall_prompt_expected -or
        [bool]$Evidence.privacy.raw_logs_tracked -or
        [bool]$Evidence.privacy.state_tracked -or
        [bool]$Evidence.privacy.account_or_device_ids_tracked -or
        [bool]$Evidence.privacy.secrets_tracked) {
        throw 'Two-host rollout evidence boundary changed'
    }
    $expectedLabels = @('local-windows-host', 'remote-windows-host')
    for ($index = 0; $index -lt $expectedLabels.Count; $index++) {
        $hostEvidence = @($Evidence.hosts)[$index]
        if ([string]$hostEvidence.label -cne $expectedLabels[$index] -or
            [string]$hostEvidence.harness_tests -cne '56/56' -or
            [string]$hostEvidence.end_to_end_rehearsal -cne 'verified' -or
            [string]$hostEvidence.absent_store_observer -cne 'hold-without-create' -or
            [string]$hostEvidence.operator_observer -cne 'hold-without-create') {
            throw "Two-host rollout evidence for $($expectedLabels[$index]) changed"
        }
    }
    foreach ($name in @('foreign_root', 'stale', 'truncated', 'fork')) {
        if ([string]$Manifest.negative_matrix.$name -cne 'rejected') {
            throw "Rollout negative matrix no longer rejects $name"
        }
    }
    Assert-ContainsAll $Rehearsal @(
        'canonical_sync_v2_rollout_end_to_end_rehearsal',
        'trusted_policy_chain=5/5',
        'device_approval_transitions=3/3',
        'explicit_recovery=verified',
        'negative_stale_truncated_fork=3/3',
        'default_public_activation=legacy-only'
    ) 'Rollout end-to-end rehearsal'
    Assert-ContainsAll $Health @(
        'pub fn synchronize_bundle_chain(',
        'validate_bundle_chain(bundles, self.trust_root)',
        'full_verified_chain_bootstraps_and_resumes_idempotently'
    ) 'Rotation-aware trusted health store'
    Assert-ContainsAll $PolicyStore @('pub fn bundle_chain(&self)') 'Trusted policy store'
    Assert-ContainsAll $Cli @(
        'load complete verified trusted-policy chain',
        'synchronize rollout health with complete trusted-policy chain',
        'fn inspect_interrupted_rollout_activation(',
        'fn repair_interrupted_rollout_approval(',
        'fn repair_interrupted_rollout_recovery(',
        '"atomic-repair"',
        'interrupted_rollout_approval_repairs_only_the_exact_device_signed_candidate'
    ) 'Rollout CLI'
    Assert-ContainsAll $Field @(
        '[string] $WorkingDirectory',
        "'end_to_end_rehearsal::canonical_sync_v2_rollout_end_to_end_rehearsal'",
        'rollout_test_harness_profile = ''verification''',
        'cli_profile = ''debug''',
        'p2p_network_executed = $false',
        'raw_logs_tracked = $false',
        "status = 'canonical-sync-v2-rollout-end-to-end-two-host-verified'"
    ) 'Two-host rollout harness'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'complete verified trusted-policy bundle chain',
        'same exact artifact may repair only that missing high-water',
        'public default remains `legacy-only`',
        'not an independent security review',
        'First-contact capability stripping remains unresolved'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Rollout end-to-end RFC is missing '$needle'"
        }
    }
    $normalizedPortability = [regex]::Replace($PortabilityRfc, '\s+', ' ')
    foreach ($needle in @(
        'M1.1.98 is an evidence milestone',
        'passed 56/56 rollout tests',
        'SSH connection was only a test-control and artifact-transfer channel',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedPortability.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Exact-binary portability RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $rfcPath, $portabilityRfcPath, $rehearsalPath, $healthPath, $policyStorePath, $cliPath, $fieldPath, $evidencePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$portabilityRfc = Get-Content -LiteralPath $portabilityRfcPath -Raw
$rehearsal = Get-Content -LiteralPath $rehearsalPath -Raw
$health = Get-Content -LiteralPath $healthPath -Raw
$policyStore = Get-Content -LiteralPath $policyStorePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$field = Get-Content -LiteralPath $fieldPath -Raw
$evidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
Assert-Contract $manifest $rfc $portabilityRfc $rehearsal $health $policyStore $cli $field $evidence

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.control_plane.automatic_enrollment = $true
    try { Assert-Contract $mutated $rfc $portabilityRfc $rehearsal $health $policyStore $cli $field $evidence } catch {
        Write-Output 'automatic_enrollment=rejected'
        Write-Output 'canonical_sync_v2_rollout_end_to_end_rehearsal_self_test=passed'
        exit 0
    }
    throw 'End-to-end rehearsal silently enabled automatic enrollment'
}

Write-Output 'trusted_policy_chain=complete-root-verified'
Write-Output 'device_apply_repair=exact-artifact-only'
Write-Output 'two_host_exact_binary_rehearsal=verified'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_rollout_end_to_end_rehearsal_boundary=verified'
