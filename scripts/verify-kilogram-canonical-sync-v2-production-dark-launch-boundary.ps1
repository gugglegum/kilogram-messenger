[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PRODUCTION-DARK-LAUNCH.json'
$policyPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$authorityPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-AUTHORITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\canonical_sync_v2_runtime.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$capabilityPath = Join-Path $workspace 'crates\kilogram-sync-capability\src\lib.rs'
$rehearsalPath = Join-Path $workspace 'scripts\invoke-kilogram-canonical-sync-v2-dark-launch-local.ps1'
$rfcPath = Join-Path $workspace 'docs\RFC-0207-m1183-production-runtime-canonical-sync-v2-dark-launch.md'

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
    if (($values -join "`n") -cne ($Expected -join "`n")) { throw "$Label changed" }
}

function Assert-ContainsAll {
    param([string]$Text, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if (-not $Text.Contains($needle)) { throw "$Label is missing '$needle'" }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Policy,
        [object]$Authority,
        [object]$Migration,
        [string]$Cli,
        [string]$Runtime,
        [string]$Wire,
        [string]$Capability,
        [string]$Rehearsal,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.83' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-production-runtime-opt-in-integrated' -or
        -not [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.legacy_protocol_identifier -cne 'kilogram/m0/sync/8' -or
        [string]$Manifest.canonical_protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'production dark-launch manifest header changed'
    }

    if ([string]$Manifest.authority.source -cne 'verified-device-signed-activation-chain' -or
        [string]$Manifest.authority.missing_authority -cne 'implicit-legacy-only' -or
        [string]$Manifest.authority.malformed_or_stale_authority -cne 'fail-closed' -or
        [bool]$Manifest.authority.environment_or_convenience_flag_authority) {
        throw 'production dark-launch authority boundary changed'
    }
    Assert-ExactStrings @($Manifest.endpoint_policy.default_registered_alpns) @(
        'kilogram/m0/sync/8'
    ) 'default registered ALPNs'
    Assert-ExactStrings @($Manifest.endpoint_policy.opt_in_registered_alpns) @(
        'kilogram/m0/sync/8',
        'kilogram/m1/sync/cbor/2'
    ) 'opt-in registered ALPNs'
    Assert-ExactStrings @($Manifest.endpoint_policy.default_ticket_candidate_protocols) @() `
        'default ticket candidates'
    Assert-ExactStrings @($Manifest.endpoint_policy.opt_in_ticket_candidate_protocols) @(
        'kilogram/m1/sync/cbor/2'
    ) 'opt-in ticket candidates'
    if (-not [bool]$Manifest.endpoint_policy.publication_requires_registered_reader) {
        throw 'capability publication no longer requires a registered reader'
    }

    foreach ($property in @(
        'stable_session_binding', 'both_account_authority_snapshots_persisted',
        'both_device_certificates_persisted',
        'selection_persisted_before_canonical_dial',
        'selection_persisted_before_acceptance_response'
    )) {
        if (-not [bool]$Manifest.selection.$property) {
            throw "selection contract '$property' is not verified"
        }
    }
    if ([string]$Manifest.selection.selection_epoch_source -cne
        'listener-ticket-publication-channel-epoch-plus-one') {
        throw 'cross-Device selection epoch source changed'
    }
    if (-not [bool]$Manifest.canonical_runtime.shared_production_adapter -or
        -not [bool]$Manifest.canonical_runtime.typed_carrier_store -or
        -not [bool]$Manifest.canonical_runtime.checkpoint_bound_frame_journal -or
        [bool]$Manifest.canonical_runtime.selected_error_fallback_to_legacy -or
        -not [bool]$Manifest.canonical_runtime.selected_resume_requires_exact_persisted_selection) {
        throw 'production canonical runtime boundary changed'
    }

    foreach ($property in @(
        'policy_state', 'migration_epoch', 'containment', 'registered_alpn',
        'published_capability', 'selected_alpn', 'selection_evidence_id'
    )) {
        if (-not [bool]$Manifest.telemetry.$property) {
            throw "stable telemetry '$property' disappeared"
        }
    }
    if ([bool]$Manifest.telemetry.message_contents_or_secrets) {
        throw 'production dark-launch telemetry includes plaintext or secrets'
    }
    if ([string]$Manifest.verification.cli_unit_tests -cne '85/85' -or
        [string]$Manifest.verification.local_default_default -cne
            'legacy-postcard-v6-converged' -or
        [string]$Manifest.verification.local_optin_default -cne
            'legacy-postcard-v6-converged' -or
        [string]$Manifest.verification.local_optin_optin -cne
            'canonical-cbor-v2-converged' -or
        -not [bool]$Manifest.verification.local_histories_equal -or
        -not [bool]$Manifest.verification.local_selection_evidence_equal -or
        [bool]$Manifest.verification.local_fallback_after_selection) {
        throw 'local production dark-launch verification changed'
    }

    if ([string]$Policy.public_default -cne 'legacy-only' -or
        [bool]$Policy.activation.active_runtime_integrated -or
        [string]$Authority.public_default -cne 'legacy-only' -or
        [bool]$Authority.activation.active_runtime_integrated -or
        [string]$Migration.current_state -cne 'legacy-only') {
        throw 'historical prerequisite or public default was rewritten'
    }

    Assert-ContainsAll $Cli @(
        'CanonicalSyncV2RuntimePolicy',
        'load_canonical_sync_v2_runtime_policy(',
        'runtime_alpns.push(CANONICAL_SYNC_ALPN_V2.to_vec())',
        'listener_alpns.push(CANONICAL_SYNC_ALPN_V2.to_vec())',
        'publish_canonical_capability()',
        'SyncTransportCandidateOffer(offer_bytes)',
        'persist_canonical_sync_v2_selection(',
        'canonical_sync_v2_selection_epoch(',
        'endpoint.connect(ticket.endpoint().clone(), CANONICAL_SYNC_ALPN_V2)',
        'let outcome = drive_canonical_sync_v2_client(',
        'serve_canonical_sync_v2_connection(connection, &server, stop_after_diff)',
        'sync_transport_fallback_after_selection=false'
    ) 'production CLI integration'
    Assert-ContainsAll $Runtime @(
        'pub async fn drive_canonical_sync_v2_client',
        'pub async fn serve_canonical_sync_v2_connection'
    ) 'shared canonical Iroh adapter'
    Assert-ContainsAll $Wire @(
        'SyncTransportCandidateOffer(Vec<u8>)',
        'SyncTransportCandidateAccepted(Vec<u8>)',
        'SyncTransportCandidateRejected',
        'MAX_SYNC_TRANSPORT_SELECTION_WIRE_BYTES'
    ) 'bounded selection wire'
    Assert-ContainsAll $Capability @(
        'pub fn issue_legacy_only(',
        'pub fn issue_legacy_with_canonical_candidate(',
        'content.candidate_protocols.is_empty()'
    ) 'default-safe capability'
    Assert-ContainsAll $Rehearsal @(
        "'default-default'",
        "'optin-default'",
        "'optin-optin'",
        "'kilogram/m0/sync/8'",
        "'kilogram/m1/sync/cbor/2'",
        "'sync_transport_fallback_after_selection' 'false'",
        'histories_equal = $true'
    ) 'local dark-launch rehearsal'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'A new or existing state directory without the Device-signed activation chain',
        'cannot precede reader registration',
        'persists before returning acceptance',
        'independent local administrative histories share a clock',
        'never turns a selected canonical failure into a Postcard retry',
        'public default activation, canonical-native history and legacy retirement'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "production dark-launch RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $policyPath, $authorityPath, $migrationPath, $cliPath,
    $runtimePath, $wirePath, $capabilityPath, $rehearsalPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "production dark-launch file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$policy = Get-Content -LiteralPath $policyPath -Raw | ConvertFrom-Json
$authority = Get-Content -LiteralPath $authorityPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$cli = Get-Content -LiteralPath $cliPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$capability = Get-Content -LiteralPath $capabilityPath -Raw
$rehearsal = Get-Content -LiteralPath $rehearsalPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $policy $authority $migration $cli $runtime $wire `
    $capability $rehearsal $rfc

if ($SelfTest) {
    $unsafeDefault = Copy-JsonObject $manifest
    $unsafeDefault.endpoint_policy.default_registered_alpns += 'kilogram/m1/sync/cbor/2'
    Assert-Rejected {
        Assert-Contract $unsafeDefault $policy $authority $migration $cli $runtime `
            $wire $capability $rehearsal $rfc
    } 'canonical_alpn_in_public_default'

    $fallback = Copy-JsonObject $manifest
    $fallback.canonical_runtime.selected_error_fallback_to_legacy = $true
    Assert-Rejected {
        Assert-Contract $fallback $policy $authority $migration $cli $runtime `
            $wire $capability $rehearsal $rfc
    } 'legacy_fallback_after_selection'

    $unpersisted = Copy-JsonObject $manifest
    $unpersisted.selection.selection_persisted_before_canonical_dial = $false
    Assert-Rejected {
        Assert-Contract $unpersisted $policy $authority $migration $cli $runtime `
            $wire $capability $rehearsal $rfc
    } 'dial_before_durable_selection'

    $secretTelemetry = Copy-JsonObject $manifest
    $secretTelemetry.telemetry.message_contents_or_secrets = $true
    Assert-Rejected {
        Assert-Contract $secretTelemetry $policy $authority $migration $cli $runtime `
            $wire $capability $rehearsal $rfc
    } 'secret_transport_telemetry'

    Write-Output 'canonical_sync_v2_production_dark_launch_self_test=passed'
}

Write-Output 'canonical_sync_v2_production_dark_launch_boundary=verified'
Write-Output 'public_default=legacy-only'
Write-Output 'default_registered_alpn=kilogram/m0/sync/8'
Write-Output 'opt_in_canonical_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'selection_evidence=mutually-signed-and-durable'
Write-Output 'selected_legacy_fallback=false'
