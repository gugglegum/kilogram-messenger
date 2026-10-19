[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DESKTOP-ACTIVATION.json'
$policyPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$authorityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'
$darkLaunchPath = Join-Path $workspace 'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json'
$activationPath = Join-Path $workspace 'apps\kilogram-windows\src\activation.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0210-m1186-desktop-canonical-sync-v2-activation.md'

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
    param([string]$Value, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Value.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Policy,
        [object]$Authority,
        [object]$DarkLaunch,
        [string]$Activation,
        [string]$Desktop,
        [string]$Ipc,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.86' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-desktop-offline-activation-verified' -or
        [bool]$Manifest.changes_active_wire -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.existing_opt_in_wire_available) {
        throw 'desktop activation header changed'
    }

    if ([string]$Manifest.authority.source -cne
            'existing-cli-device-signed-append-only-chain' -or
        [bool]$Manifest.authority.desktop_holds_device_secret -or
        [bool]$Manifest.authority.desktop_holds_account_root_secret -or
        [string]$Manifest.authority.actor -cne 'explicit-local' -or
        -not [bool]$Manifest.authority.runtime_must_be_stopped -or
        [string]$Manifest.authority.execution -cne
            'offline-cli-helper-under-existing-state-lock' -or
        [bool]$Manifest.authority.live_runtime_ipc_extended -or
        [UInt64]$Manifest.authority.ipc_version -ne 29 -or
        -not [bool]$Manifest.authority.fresh_runtime_required_after_apply) {
        throw 'desktop activation authority boundary changed'
    }

    Assert-ExactStrings @($Manifest.ceremony.operations) @(
        'refresh-verified-status',
        'plan-exact-transition-without-write',
        'apply-exact-signed-successor'
    ) 'desktop activation operations'
    Assert-ExactStrings @($Manifest.ceremony.states) @(
        'legacy-only',
        'advertise-cbor-v2',
        'dual-reader-prefer-legacy',
        'dual-reader-prefer-canonical',
        'canonical-required'
    ) 'desktop activation states'
    if ([UInt64]$Manifest.ceremony.reason_min_utf8_bytes -ne 1 -or
        [UInt64]$Manifest.ceremony.reason_max_utf8_bytes -ne 256 -or
        [bool]$Manifest.ceremony.reason_surrounding_whitespace_allowed -or
        [bool]$Manifest.ceremony.reason_control_characters_allowed -or
        [bool]$Manifest.ceremony.plan_writes_state -or
        -not [bool]$Manifest.ceremony.apply_requires_expected_record_id -or
        [string]$Manifest.ceremony.confirmation -cne
            'APPLY <first-12-lowercase-hex-of-candidate-record-id>' -or
        -not [bool]$Manifest.ceremony.draft_change_invalidates_plan -or
        -not [bool]$Manifest.ceremony.post_apply_status_head_must_equal_applied_record_id -or
        -not [bool]$Manifest.ceremony.helper_rights_must_equal_policy_matrix -or
        -not [bool]$Manifest.ceremony.emergency_containment_requires_existing_evidence -or
        -not [bool]$Manifest.ceremony.emergency_containment_preserves_exact_resume) {
        throw 'desktop activation ceremony changed'
    }

    Assert-ExactStrings @($Manifest.fail_closed) @(
        'runtime-running-or-connected',
        'state-lock-unavailable',
        'missing-or-malformed-helper-field',
        'noncanonical-identifier',
        'inconsistent-policy-rights',
        'stale-device-head-epoch-or-evidence',
        'changed-draft-after-plan',
        'wrong-confirmation-phrase',
        'unexpected-applied-record-id',
        'post-apply-head-mismatch'
    ) 'desktop fail-closed cases'

    if ([string]$Manifest.accepted_prerequisites.activation_policy -cne
            'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json' -or
        [string]$Manifest.accepted_prerequisites.activation_authority -cne
            'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json' -or
        [string]$Manifest.accepted_prerequisites.opt_in_dark_launch -cne
            'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json' -or
        [string]$Policy.status -cne
            'canonical-sync-v2-activation-semantics-verified-not-active' -or
        [string]$Authority.status -cne
            'canonical-sync-v2-durable-local-activation-authority-verified' -or
        [string]$DarkLaunch.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$DarkLaunch.public_default -cne 'legacy-only') {
        throw 'desktop activation prerequisite linkage changed'
    }

    $acceptance = $Manifest.acceptance
    if ([string]$acceptance.desktop_unit_tests -cne 'verified' -or
        [string]$acceptance.offline_cli_lifecycle_smoke -cne 'verified' -or
        [string]$acceptance.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$acceptance.m1_readiness -cne '113/113' -or
        [string]$acceptance.rustfmt -cne 'verified' -or
        [string]$acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [bool]$acceptance.release_build -or
        [bool]$acceptance.archive_created -or
        [bool]$acceptance.git_tag_created -or
        [bool]$acceptance.automatic_github_build -or
        [bool]$acceptance.network_executed) {
        throw 'desktop activation acceptance evidence changed'
    }

    Assert-ContainsAll $Activation @(
        'pub(crate) const MAX_ACTIVATION_REASON_BYTES: usize = 256;',
        'fn validate_policy(',
        'permissions == ActivationPermissions::expected(migration_state, disposition)',
        'plan.current_record_id == status.record_id',
        'plan.migration_epoch == status.migration_epoch + 1',
        'plan.canonical_evidence_id == status.canonical_evidence_id',
        'format!("APPLY {}", &self.record_id[..12])',
        'self.invalidate_plan();',
        'Public default remains legacy-only.',
        'takes effect after a fresh runtime start'
    ) 'desktop activation module'
    foreach ($forbidden in @(
        'RuntimeIpcCommand', 'DeviceSecret', 'AccountRootSecret', 'SigningKey',
        'SecretKey', 'std::env', 'set_var('
    )) {
        if ($Activation.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "desktop activation module contains forbidden authority surface '$forbidden'"
        }
    }

    Assert-ContainsAll $Desktop @(
        '"canonical-sync-policy-status"',
        '"canonical-sync-policy-plan"',
        '"canonical-sync-policy-apply"',
        '("--actor", std::ffi::OsStr::new("explicit-local"))',
        '"--expected-record-id"',
        'self.runtime_process.is_none()',
        'self.model.connection == ConnectionState::Disconnected',
        '"Stop and disconnect the runtime before using activation authority"',
        'status.record_id.as_deref() == Some(applied_record_id.as_str())',
        'Starting a fresh runtime for exact verification.'
    ) 'desktop integration'
    Assert-ContainsAll $Ipc @('const IPC_VERSION: u8 = 31;') 'runtime IPC source'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'public default remains `legacy-only`',
        'Why activation is deliberately offline',
        'Runtime IPC stays at version 29',
        'Any draft change clears both the plan and confirmation',
        'does not enable a public dual-reader default'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "desktop activation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $policyPath, $authorityPath, $darkLaunchPath,
    $activationPath, $desktopPath, $ipcPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "desktop activation boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
$authority = Get-Content -LiteralPath $authorityPath -Raw | ConvertFrom-Json
$darkLaunch = Get-Content -LiteralPath $darkLaunchPath -Raw | ConvertFrom-Json
$activation = Get-Content -LiteralPath $activationPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $policy $authority $darkLaunch $activation $desktop $ipc $rfc

if ($SelfTest) {
    $hotApply = Copy-JsonObject $manifest
    $hotApply.authority.runtime_must_be_stopped = $false
    Assert-Rejected {
        Assert-Contract $hotApply $policy $authority $darkLaunch $activation $desktop $ipc $rfc
    } 'live_runtime_apply'

    $toggle = Copy-JsonObject $manifest
    $toggle.ceremony.apply_requires_expected_record_id = $false
    Assert-Rejected {
        Assert-Contract $toggle $policy $authority $darkLaunch $activation $desktop $ipc $rfc
    } 'unchecked_ui_toggle'

    $publicDefault = Copy-JsonObject $manifest
    $publicDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $publicDefault $policy $authority $darkLaunch $activation $desktop $ipc $rfc
    } 'public_default_activation'

    $noRightsCheck = $activation.Replace(
        'permissions == ActivationPermissions::expected(migration_state, disposition)',
        'true'
    )
    Assert-Rejected {
        Assert-Contract $manifest $policy $authority $darkLaunch $noRightsCheck $desktop $ipc $rfc
    } 'removed_rights_matrix_check'

    $noExpectedId = $desktop.Replace('"--expected-record-id"', '"--unchecked-record-id"')
    Assert-Rejected {
        Assert-Contract $manifest $policy $authority $darkLaunch $activation $noExpectedId $ipc $rfc
    } 'removed_exact_record_binding'

    Write-Output 'canonical_sync_v2_desktop_activation_self_test=passed'
}

Write-Output 'canonical_sync_v2_desktop_activation_boundary=verified'
Write-Output 'status=canonical-sync-v2-desktop-offline-activation-verified'
Write-Output 'public_default=legacy-only'
Write-Output 'runtime_ipc_extended=false'
Write-Output 'release_build=false'
Write-Output 'network_executed=false'
