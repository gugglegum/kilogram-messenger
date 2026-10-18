[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RUNTIME-OBSERVABILITY.json'
$policyPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$darkLaunchPath = Join-Path $workspace 'CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json'
$desktopActivationPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DESKTOP-ACTIVATION.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0211-m1187-canonical-sync-v2-runtime-observability.md'

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
        [object]$Policy,
        [object]$DarkLaunch,
        [object]$DesktopActivation,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Ipc,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.87' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-runtime-observability-verified' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'runtime observability identity or scope changed'
    }

    $ipcContract = $Manifest.runtime_ipc
    if ([string]$ipcContract.operation -cne 'read-only-status' -or
        [string]$ipcContract.command -cne 'CanonicalSyncV2RuntimeStatus' -or
        [string]$ipcContract.response -cne 'CanonicalSyncV2RuntimeStatus' -or
        [int]$ipcContract.version -ne 30 -or
        [string]$ipcContract.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v11\0' -or
        [int]$ipcContract.previous_version -ne 29 -or
        [string]$ipcContract.previous_descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v10\0' -or
        [string]$ipcContract.migration -cne
            'exact-version-hard-cut-for-co-installed-binaries' -or
        [bool]$ipcContract.dual_reader -or
        [bool]$ipcContract.mutation_or_hot_apply) {
        throw 'runtime observability IPC boundary changed'
    }

    $projection = $Manifest.policy_projection
    if (-not [bool]$projection.loaded_at_process_start -or
        -not [bool]$projection.configured_reloaded_under_state_lock -or
        -not [bool]$projection.exact_comparison -or
        -not [bool]$projection.restart_required_iff_different -or
        -not [bool]$projection.implicit_default_visible -or
        -not [bool]$projection.signed_chain_head_visible -or
        -not [bool]$projection.six_permissions_visible) {
        throw 'runtime policy projection changed'
    }

    $paths = $Manifest.sync_path_projection
    if ([string]$paths.storage -cne 'process-memory-only' -or
        [int]$paths.capacity -ne 32 -or
        [string]$paths.overflow -cne 'drop-oldest' -or
        [string]$paths.sequence -cne 'process-local-monotonic') {
        throw 'runtime sync path retention changed'
    }
    Assert-ExactStrings @($paths.directions) @('inbound', 'outbound') 'sync directions'
    Assert-ExactStrings @($paths.protocols) @('legacy-postcard-v6', 'canonical-cbor-v2') 'sync protocols'
    Assert-ExactStrings @($paths.outcomes) @('completed', 'paused', 'interrupted', 'failed') 'sync outcomes'
    Assert-ExactStrings @($paths.forbidden) @(
        'message-plaintext', 'ciphertext-payload', 'ip-address', 'relay-url',
        'filesystem-path', 'bearer-token', 'device-secret', 'account-root-secret',
        'mailbox-capability'
    ) 'forbidden observation data'

    if (-not [bool]$Manifest.desktop.loads_on_authenticated_connect -or
        -not [bool]$Manifest.desktop.refreshes_after_runtime_change_notification -or
        -not [bool]$Manifest.desktop.manual_refresh -or
        -not [bool]$Manifest.desktop.shows_loaded_and_configured_policy -or
        -not [bool]$Manifest.desktop.shows_restart_required -or
        [int]$Manifest.desktop.shows_at_most_recent_observations -ne 8 -or
        -not [bool]$Manifest.desktop.activation_authority_remains_offline_only) {
        throw 'desktop runtime observability projection changed'
    }

    if ([string]$Policy.status -cne 'canonical-sync-v2-activation-semantics-verified-not-active' -or
        [string]$DarkLaunch.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$DarkLaunch.public_default -cne 'legacy-only' -or
        [string]$DesktopActivation.status -cne 'canonical-sync-v2-desktop-offline-activation-verified' -or
        [string]$DesktopActivation.public_default -cne 'legacy-only') {
        throw 'runtime observability prerequisite linkage changed'
    }

    $activeCommands = @($Completion.active_postcard.commands)
    $activeResponses = @($Completion.active_postcard.responses)
    if ([int]$Completion.active_postcard.version -ne 30 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v11\0' -or
        $activeCommands.Count -ne 30 -or $activeResponses.Count -ne 29 -or
        [string]$activeCommands[-1].variant -cne 'CanonicalSyncV2RuntimeStatus' -or
        [string]$activeResponses[-1].variant -cne 'CanonicalSyncV2RuntimeStatus' -or
        'canonical-sync-v2-runtime-status' -cnotin @($Completion.canonical.commands) -or
        'canonical-sync-v2-runtime-status-result' -cnotin @($Completion.canonical.results)) {
        throw 'runtime observability completion inventory changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v30-with-json-descriptor-v11' -or
        @($Schema.covered_commands).Count -ne 35 -or
        @($Schema.covered_results).Count -ne 34 -or
        @($Schema.semantic_rules).Count -ne 180) {
        throw 'runtime observability canonical schema changed'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 30 -or
        [string](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v11\0') {
        throw 'runtime observability compatibility binding changed'
    }

    Assert-ContainsAll $Ipc @(
        'const IPC_VERSION: u8 = 30;',
        'kilogram:runtime-ipc-descriptor:v11\0',
        'pub const MAX_RUNTIME_IPC_SYNC_PATH_OBSERVATIONS: usize = 32;',
        'CanonicalSyncV2RuntimeStatus,',
        'CanonicalSyncV2RuntimeStatus(Box<RuntimeIpcCanonicalSyncV2RuntimeStatus>)',
        'pub struct RuntimeIpcCanonicalSyncV2RuntimeStatus',
        'version_29_request_is_rejected_before_actor_dispatch'
    ) 'runtime IPC source'
    Assert-ContainsAll $Cli @(
        'struct RuntimeSyncObservability',
        'if self.observations.len() == MAX_RUNTIME_IPC_SYNC_PATH_OBSERVATIONS',
        'self.observations.pop_front();',
        'fn collect_canonical_sync_v2_runtime_status(',
        'lock runtime state for canonical sync v2 observability',
        'configured_matches_loaded: configured == loaded',
        'RuntimeIpcCommand::CanonicalSyncV2RuntimeStatus',
        'runtime_sync_observability_is_bounded_and_reports_restart_divergence'
    ) 'runtime actor source'
    Assert-ContainsAll $Desktop @(
        'RefreshCanonicalRuntimeStatus',
        'RuntimeIpcCommand::CanonicalSyncV2RuntimeStatus',
        'Refresh loaded policy and sync paths',
        'status.restart_required',
        'restart required',
        '.iter().rev().take(8)'
    ) 'desktop observability source'

    $acceptance = $Manifest.acceptance
    if ([string]$acceptance.runtime_ipc_unit_tests -cne 'verified' -or
        [string]$acceptance.cli_observability_unit_test -cne 'verified' -or
        [string]$acceptance.desktop_unit_tests -cne 'verified' -or
        [string]$acceptance.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$acceptance.m1_readiness -cne '114/114' -or
        [string]$acceptance.rustfmt -cne 'verified' -or
        [string]$acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [bool]$acceptance.release_build -or [bool]$acceptance.archive_created -or
        [bool]$acceptance.git_tag_created -or [bool]$acceptance.automatic_github_build -or
        [bool]$acceptance.network_executed) {
        throw 'runtime observability acceptance evidence changed'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'public default remains `legacy-only`',
        'loaded when the process built its Iroh endpoint',
        'at most 32 rows',
        'does not infer canonical use from a configured policy',
        'exact IPC v30 / descriptor domain v11',
        'adds no activation mutation, hot apply, endpoint rebind'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "runtime observability RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $policyPath, $darkLaunchPath, $desktopActivationPath,
    $completionPath, $schemaPath, $compatibilityPath, $ipcPath, $cliPath,
    $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "runtime observability boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
$darkLaunch = Get-Content -LiteralPath $darkLaunchPath -Raw | ConvertFrom-Json
$desktopActivation = Get-Content -LiteralPath $desktopActivationPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $policy $darkLaunch $desktopActivation $completion $schema `
    $compatibility $ipc $cli $desktop $rfc

if ($SelfTest) {
    $mutation = Copy-JsonObject $manifest
    $mutation.runtime_ipc.mutation_or_hot_apply = $true
    Assert-Rejected {
        Assert-Contract $mutation $policy $darkLaunch $desktopActivation $completion $schema `
            $compatibility $ipc $cli $desktop $rfc
    } 'live_mutation'

    $unbounded = Copy-JsonObject $manifest
    $unbounded.sync_path_projection.capacity = 1024
    Assert-Rejected {
        Assert-Contract $unbounded $policy $darkLaunch $desktopActivation $completion $schema `
            $compatibility $ipc $cli $desktop $rfc
    } 'unbounded_observations'

    $publicDefault = Copy-JsonObject $manifest
    $publicDefault.public_default = 'dual-reader-prefer-canonical'
    Assert-Rejected {
        Assert-Contract $publicDefault $policy $darkLaunch $desktopActivation $completion $schema `
            $compatibility $ipc $cli $desktop $rfc
    } 'public_default_activation'

    $unboundedSource = $cli.Replace(
        'if self.observations.len() == MAX_RUNTIME_IPC_SYNC_PATH_OBSERVATIONS',
        'if false'
    )
    Assert-Rejected {
        Assert-Contract $manifest $policy $darkLaunch $desktopActivation $completion $schema `
            $compatibility $ipc $unboundedSource $desktop $rfc
    } 'removed_runtime_bound'

    Write-Output 'canonical_sync_v2_runtime_observability_self_test=passed'
}

Write-Output 'canonical_sync_v2_runtime_observability_boundary=verified'
Write-Output 'runtime_ipc=postcard-v30-json-descriptor-v11'
Write-Output 'observation_capacity=32'
Write-Output 'public_default=legacy-only'
Write-Output 'runtime_mutation=false'
