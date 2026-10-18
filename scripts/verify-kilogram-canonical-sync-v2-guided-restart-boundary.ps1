[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-GUIDED-RESTART.json'
$desktopActivationPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DESKTOP-ACTIVATION.json'
$runtimeObservabilityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RUNTIME-OBSERVABILITY.json'
$activationAuthorityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'
$activationSourcePath = Join-Path $workspace 'apps\kilogram-windows\src\activation.rs'
$desktopSourcePath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0212-m1188-canonical-sync-v2-guided-restart.md'

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
    $actualStrings = @($Actual | ForEach-Object { [string]$_ })
    if ($actualStrings.Count -ne $Expected.Count) {
        throw "$Label count changed: expected $($Expected.Count), got $($actualStrings.Count)"
    }
    for ($index = 0; $index -lt $Expected.Count; $index++) {
        if ($actualStrings[$index] -cne $Expected[$index]) {
            throw "$Label changed at index ${index}: expected '$($Expected[$index])', got '$($actualStrings[$index])'"
        }
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
        [object]$DesktopActivation,
        [object]$RuntimeObservability,
        [object]$ActivationAuthority,
        [string]$ActivationSource,
        [string]$DesktopSource,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.88' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-guided-restart-verified' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'guided restart identity or scope changed'
    }

    $workflow = $Manifest.workflow
    if ([string]$workflow.entry -cne 'explicit-local-desktop-action' -or
        -not [bool]$workflow.requires_desktop_owned_runtime -or
        -not [bool]$workflow.ordinary_apply_also_requires_fresh_start_verification -or
        [bool]$workflow.hot_apply -or [bool]$workflow.unchecked_toggle) {
        throw 'guided restart workflow authority changed'
    }
    Assert-ExactStrings @($workflow.phases) @(
        'stop-requested',
        'actual-child-exit-observed',
        'offline-authority-inspected',
        'exact-successor-planned',
        'exact-successor-applied',
        'fresh-child-spawned',
        'authenticated-runtime-connected',
        'exact-loaded-policy-verified'
    ) 'guided restart phases'

    $fresh = $Manifest.fresh_process_proof
    if (-not [bool]$fresh.old_desktop_child_exit_observed_before_offline_inspection -or
        -not [bool]$fresh.state_lock_required_by_existing_authority_helper -or
        -not [bool]$fresh.new_desktop_child_spawned_only_after_exact_apply -or
        -not [bool]$fresh.authenticated_connect_required -or
        [bool]$fresh.process_nonce_added) {
        throw 'fresh process proof changed'
    }

    $binding = $Manifest.exact_post_restart_binding
    if (-not [bool]$binding.device_id -or
        [string]$binding.policy_source -cne 'signed-device-chain' -or
        -not [bool]$binding.record_id -or
        -not [bool]$binding.migration_epoch -or
        -not [bool]$binding.migration_state -or
        -not [bool]$binding.disposition -or
        -not [bool]$binding.canonical_evidence_id -or
        -not [bool]$binding.six_permissions -or
        -not [bool]$binding.loaded_equals_configured -or
        -not [bool]$binding.configured_matches_loaded -or
        [bool]$binding.restart_required) {
        throw 'exact post-restart binding changed'
    }

    $failure = $Manifest.failure_handling
    if ([string]$failure.old_runtime_stop_timeout -cne
            'terminate-owned-child-and-fail-closed' -or
        [string]$failure.unexpected_identity_or_policy -cne 'terminate-fresh-desktop-child' -or
        [string]$failure.startup_exit_or_timeout -cne 'failed-closed' -or
        [bool]$failure.normal_conversation_refresh_before_verification -or
        [bool]$failure.durable_apply_rolled_back_implicitly -or
        -not [bool]$failure.exact_expectation_retained_for_retry -or
        [bool]$failure.retry_appends_authority_record -or
        -not [bool]$failure.profile_load_or_save_resets_workflow) {
        throw 'guided restart failure behavior changed'
    }

    if ([string]$DesktopActivation.status -cne
            'canonical-sync-v2-desktop-offline-activation-verified' -or
        [string]$RuntimeObservability.status -cne
            'canonical-sync-v2-runtime-observability-verified' -or
        [int]$RuntimeObservability.runtime_ipc.version -ne 30 -or
        [string]$RuntimeObservability.runtime_ipc.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v11\0' -or
        [string]$ActivationAuthority.status -cne
            'canonical-sync-v2-durable-local-activation-authority-verified') {
        throw 'guided restart prerequisite linkage changed'
    }

    Assert-ContainsAll $ActivationSource @(
        'pub(crate) struct ActivationRestartExpectation',
        'status.loaded == status.configured',
        'status.loaded == self.policy',
        'connected_device_id == self.device_id',
        'RuntimeIpcCanonicalSyncV2PolicySource::SignedDeviceChain',
        'GuidedRestartState::StartingFresh',
        'expectation: Some(expectation)',
        'Retry fresh start and exact verification',
        'guided_restart_accepts_only_the_exact_fresh_runtime_policy',
        'guided_restart_orders_stop_inspect_apply_start_verify_and_preserves_retry'
    ) 'desktop activation state machine'

    Assert-ContainsAll $DesktopSource @(
        'StopAndInspectCanonicalActivation',
        '.mark_runtime_exited_for_guided_stop();',
        'self.advance_guided_canonical_activation();',
        'self.enforce_guided_stop_deadline();',
        'Old runtime did not exit within',
        '.verify_fresh_runtime(device_id, canonical_runtime_status)',
        'self.abort_guided_fresh_runtime',
        'self.start_runtime();',
        'self.start_conversations();',
        'Guided activation completed: the fresh authenticated runtime loaded the exact applied signed policy.'
    ) 'desktop guided lifecycle'
    $verificationIndex = $DesktopSource.IndexOf(
        '.verify_fresh_runtime(device_id, canonical_runtime_status)',
        [StringComparison]::Ordinal
    )
    $conversationIndex = $DesktopSource.IndexOf(
        'self.start_conversations();',
        [StringComparison]::Ordinal
    )
    if ($verificationIndex -lt 0 -or $conversationIndex -lt 0 -or
        $verificationIndex -ge $conversationIndex) {
        throw 'normal conversation refresh is not ordered after exact verification'
    }

    $acceptance = $Manifest.acceptance
    if ([string]$acceptance.desktop_unit_tests -cne '28/28' -or
        [string]$acceptance.exact_positive_and_negative_restart_tests -cne 'verified' -or
        [string]$acceptance.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$acceptance.m1_readiness -cne '115/115' -or
        [string]$acceptance.rustfmt -cne 'verified' -or
        [string]$acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [bool]$acceptance.release_build -or [bool]$acceptance.archive_created -or
        [bool]$acceptance.git_tag_created -or [bool]$acceptance.automatic_github_build -or
        [bool]$acceptance.network_executed) {
        throw 'guided restart acceptance evidence changed'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'public default remains `legacy-only`',
        'actual desktop-owned process exit',
        'stops the ceremony fail closed before offline inspection',
        'Normal conversation subscription and refresh begin only after exact verification',
        'Retry does not append another authority record',
        'reuses authenticated read-only IPC v30 / descriptor domain v11',
        'does not make either decision implicitly'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "guided restart RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $desktopActivationPath, $runtimeObservabilityPath,
    $activationAuthorityPath, $activationSourcePath, $desktopSourcePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "guided restart boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$desktopActivation = Get-Content -LiteralPath $desktopActivationPath -Raw | ConvertFrom-Json
$runtimeObservability = Get-Content -LiteralPath $runtimeObservabilityPath -Raw | ConvertFrom-Json
$activationAuthority = Get-Content -LiteralPath $activationAuthorityPath -Raw | ConvertFrom-Json
$activationSource = Get-Content -LiteralPath $activationSourcePath -Raw
$desktopSource = Get-Content -LiteralPath $desktopSourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $desktopActivation $runtimeObservability $activationAuthority `
    $activationSource $desktopSource $rfc

if ($SelfTest) {
    $hotApply = Copy-JsonObject $manifest
    $hotApply.workflow.hot_apply = $true
    Assert-Rejected {
        Assert-Contract $hotApply $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $desktopSource $rfc
    } 'hot_apply'

    $publicDefault = Copy-JsonObject $manifest
    $publicDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $publicDefault $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $desktopSource $rfc
    } 'public_default_activation'

    $missingExit = Copy-JsonObject $manifest
    $missingExit.workflow.phases = @($missingExit.workflow.phases | Where-Object {
        [string]$_ -cne 'actual-child-exit-observed'
    })
    Assert-Rejected {
        Assert-Contract $missingExit $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $desktopSource $rfc
    } 'missing_actual_exit'

    $weakBinding = Copy-JsonObject $manifest
    $weakBinding.exact_post_restart_binding.record_id = $false
    Assert-Rejected {
        Assert-Contract $weakBinding $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $desktopSource $rfc
    } 'weak_record_binding'

    $unsafeRetry = Copy-JsonObject $manifest
    $unsafeRetry.failure_handling.retry_appends_authority_record = $true
    Assert-Rejected {
        Assert-Contract $unsafeRetry $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $desktopSource $rfc
    } 'retry_appends_record'

    $noAbort = $desktopSource.Replace('self.abort_guided_fresh_runtime', 'removed_abort')
    Assert-Rejected {
        Assert-Contract $manifest $desktopActivation $runtimeObservability $activationAuthority `
            $activationSource $noAbort $rfc
    } 'removed_fail_closed_termination'

    Write-Output 'canonical_sync_v2_guided_restart_self_test=passed'
}

Write-Output 'canonical_sync_v2_guided_restart_boundary=verified'
Write-Output 'public_default=legacy-only'
Write-Output 'runtime_ipc=unchanged-v30-domain-v11'
Write-Output 'post_restart_binding=exact-signed-device-policy'
Write-Output 'hot_apply=false'
Write-Output 'network_executed=false'
