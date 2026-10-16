[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$syncSchemaPath = Join-Path $workspace 'CANONICAL-SYNC-SCHEMA.json'
$sessionSchemaPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0176-m1150-canonical-sync-alpn-allocation.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-Boundary {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$SyncSchema,
        [object]$SessionSchema,
        [object]$Coverage,
        [object]$Compatibility,
        [string]$TransportSource,
        [string]$CliSource,
        [string]$Rfc
    )

    $legacyAlpn = 'kilogram/m0/sync/8'
    $canonicalAlpn = 'kilogram/m1/sync/cbor/1'
    $historyCompatibleAlpn = 'kilogram/m1/sync/cbor/2'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.68' -or
        [string]$Manifest.status -cne 'dormant-mutual-authenticated-candidate-selection-verified-not-active' -or
        [string]$Manifest.surface -cne 'transport-sync' -or
        [bool]$Manifest.changes_active_wire) {
        throw 'canonical transport sync manifest header changed'
    }
    if ([string]$Manifest.active.encoding -cne 'postcard-v6' -or
        [string]$Manifest.active.alpn -cne $legacyAlpn -or
        -not [bool]$Manifest.active.registered -or
        -not [bool]$Manifest.active.dialed) {
        throw 'active synchronization transport changed'
    }
    if ([string]$Manifest.candidate.encoding -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Manifest.candidate.alpn -cne $canonicalAlpn -or
        -not [bool]$Manifest.candidate.allocated -or
        [bool]$Manifest.candidate.registered -or
        [bool]$Manifest.candidate.dialed -or
        [bool]$Manifest.candidate.format_sniffing -or
        [bool]$Manifest.candidate.parse_error_fallback -or
        [string]$Manifest.candidate.alpn -ceq [string]$Manifest.active.alpn) {
        throw 'canonical synchronization candidate activation boundary changed'
    }
    if ([string]$Manifest.history_compatible_successor.encoding -cne
            'kilogram-deterministic-cbor-v2-typed-history' -or
        [string]$Manifest.history_compatible_successor.alpn -cne $historyCompatibleAlpn -or
        -not [bool]$Manifest.history_compatible_successor.allocated -or
        -not [bool]$Manifest.history_compatible_successor.typed_event_identity -or
        -not [bool]$Manifest.history_compatible_successor.closed_event_carrier -or
        -not [bool]$Manifest.history_compatible_successor.closed_sync_frames -or
        -not [bool]$Manifest.history_compatible_successor.typed_session_checkpoint -or
        -not [bool]$Manifest.history_compatible_successor.authenticated_ticket_capability -or
        -not [bool]$Manifest.history_compatible_successor.dormant_runtime_state_machine -or
        -not [bool]$Manifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$Manifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$Manifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$Manifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$Manifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        -not [bool]$Manifest.history_compatible_successor.dormant_signed_session_successor_store -or
        -not [bool]$Manifest.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -or
        [bool]$Manifest.history_compatible_successor.registered -or
        [bool]$Manifest.history_compatible_successor.dialed -or
        [bool]$Manifest.history_compatible_successor.format_sniffing -or
        [bool]$Manifest.history_compatible_successor.parse_error_fallback) {
        throw 'history-compatible synchronization successor boundary changed'
    }
    if ([string]$Manifest.selection.current_state -cne 'legacy-only' -or
        -not [bool]$Manifest.selection.listener_capability_required_before_dial -or
        [string]$Manifest.selection.listener_capability_authentication -cne
            'ticket-v13-device-signed-and-bound-to-authority-endpoint-requester-publication-epoch-and-prekey-validity' -or
        [string]$Manifest.selection.requester_capability_authentication -cne
            'canonical-device-session-authorization-before-sync-frames' -or
        [bool]$Manifest.selection.fallback_after_selected_canonical_failure) {
        throw 'canonical synchronization selection policy changed'
    }
    $expectedRemaining = @(
        'dual-reader-runtime-implementation',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )
    if ([string]$Manifest.authenticated_capability.portable_artifact -cne
            'connection-ticket-v13' -or
        [int]$Manifest.authenticated_capability.capability_version -ne 2 -or
        [string]$Manifest.authenticated_capability.signature_domain -cne
            'kilogram:sync-transport-capability-signature:v2\0' -or
        (@($Manifest.authenticated_capability.active_protocols) -join ',') -cne $legacyAlpn -or
        (@($Manifest.authenticated_capability.candidate_protocols) -join ',') -cne $historyCompatibleAlpn -or
        [bool]$Manifest.authenticated_capability.canonical_dial_enabled -or
        (@($Manifest.authenticated_capability.bindings) -join ',') -cne
            'listener-account-id,listener-device-id,authority-revision,allowed-requester-account-id,endpoint-id,ticket-publication-channel-epoch,prekey-directory-common-validity-window' -or
        (@($Manifest.remaining) -join ',') -cne ($expectedRemaining -join ',') -or
        [bool]$Manifest.scope.active_wire_changed -or
        [bool]$Manifest.scope.network_executed) {
        throw 'canonical synchronization remaining-work boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.68' -or
        [string]$Migration.status -cne 'dormant-mutual-authenticated-candidate-selection-verified-not-active' -or
        [string]$Migration.candidate_protocol_identifier -cne $historyCompatibleAlpn -or
        [string]$Migration.history_compatible_successor.protocol_identifier -cne
            $historyCompatibleAlpn -or
        [bool]$Migration.candidate_identifier_registered -or
        [string]$Migration.current_state -cne 'legacy-only') {
        throw 'canonical migration does not preserve allocated inactive identifier'
    }
    $allocation = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'parallel-protocol-identifier-allocation'
    })
    if ($allocation.Count -ne 1 -or [string]$allocation[0].status -cne 'verified') {
        throw 'canonical migration did not verify parallel identifier allocation'
    }
    $capabilityEnvelope = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'authenticated-capability-envelope'
    })
    if ($capabilityEnvelope.Count -ne 1 -or
        [string]$capabilityEnvelope[0].status -cne 'verified') {
        throw 'canonical migration did not verify authenticated capability envelope'
    }
    foreach ($schema in @($SyncSchema, $SessionSchema)) {
        if ([string]$schema.activation.candidate_protocol_identifier -cne $canonicalAlpn -or
            [bool]$schema.activation.current_runtime_changed -or
            'new-explicit-protocol-version-or-parallel-alpn' -cin
                @($schema.activation.required_before_activation)) {
            throw 'canonical sync schema does not consume exact allocated identifier'
        }
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.68' -or
        $surface.Count -ne 1 -or
        [string]$surface[0].coverage -cne 'partial' -or
        (@($surface[0].covered) -join ',') -cne
            'parallel-cbor-alpn-identifier-allocation,history-compatible-cbor-v2-alpn-identifier-allocation,typed-durable-canonical-sync-session-checkpoint,authenticated-ticket-v13-history-compatible-cbor-v2-capability,dormant-history-compatible-sync-v2-runtime-state-machine,dormant-atomic-typed-carrier-store,dormant-cross-format-membership-policy,dormant-bounded-resumable-legacy-import,dormant-atomic-legacy-import-checkpoint,dormant-signed-session-bootstrap,dormant-signed-session-successor-store,dormant-checkpoint-bound-runtime-frame-journal,dormant-atomic-runtime-transition-transaction,dormant-durable-client-transition-adapter,dormant-durable-responder-transition-adapter,dormant-mutual-authenticated-candidate-selection' -or
        (@($surface[0].remaining) -join ',') -cne
            'dual-reader-runtime-implementation,mixed-version-and-downgrade-tests,rollback-drill,sustained-fuzzing,fresh-hosted-and-field-evidence' -or
        [int]$Coverage.summary.complete -ne 17 -or
        [int]$Coverage.summary.partial -ne 1 -or
        [int]$Coverage.summary.not_started -ne 0 -or
        [int]$Coverage.summary.not_targeted -ne 2) {
        throw 'canonical transport sync coverage changed'
    }

    $compatibilitySurface = @($Compatibility.surfaces | Where-Object {
        [string]$_.id -ceq 'transport-sync'
    })
    $activeAlpnBinding = @($compatibilitySurface[0].bindings | Where-Object {
        [string]$_.symbol -ceq 'ALPN'
    })
    if ($compatibilitySurface.Count -ne 1 -or $activeAlpnBinding.Count -ne 1 -or
        [string]$activeAlpnBinding[0].value -cne $legacyAlpn) {
        throw 'active protocol compatibility ALPN changed'
    }

    foreach ($required in @(
        'pub const ALPN: &[u8] = b"kilogram/m0/sync/8";',
        'pub const CANONICAL_SYNC_ALPN_V1: &[u8] = b"kilogram/m1/sync/cbor/1";',
        'pub const CANONICAL_SYNC_ALPN_V2: &[u8] = b"kilogram/m1/sync/cbor/2";',
        'pub enum SyncWireProtocol',
        'Self::LegacyPostcardV6 => ALPN',
        'Self::CanonicalCborV1 => CANONICAL_SYNC_ALPN_V1',
        'Self::CanonicalCborV2 => CANONICAL_SYNC_ALPN_V2',
        'bail!("unsupported Kilogram synchronization ALPN")',
        'synchronization_alpns_are_exact_distinct_and_fail_closed'
    )) {
        if ($TransportSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "transport source is missing '$required'"
        }
    }
    if ($CliSource.IndexOf('CANONICAL_SYNC_ALPN_V1', [StringComparison]::Ordinal) -ge 0 -or
        $CliSource.IndexOf('CANONICAL_SYNC_ALPN_V2', [StringComparison]::Ordinal) -ge 0 -or
        $CliSource.IndexOf($canonicalAlpn, [StringComparison]::Ordinal) -ge 0 -or
        $CliSource.IndexOf($historyCompatibleAlpn, [StringComparison]::Ordinal) -ge 0) {
        throw 'inactive canonical ALPN leaked into active CLI registration or dial path'
    }
    if ($CliSource.IndexOf('.alpns(vec![ALPN.to_vec()])', [StringComparison]::Ordinal) -lt 0 -or
        $CliSource.IndexOf('let mut runtime_alpns = vec![ALPN.to_vec()];',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'active legacy synchronization ALPN registration is missing'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'identifier allocated, not active',
        'active ALPN remains `kilogram/m0/sync/8`',
        'Format sniffing and parse-error fallback are forbidden',
        'absent from every active endpoint ALPN list and every dial path',
        'must not silently retry',
        'Allocation alone is not dual-reader support and is not activation'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0176 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $syncSchemaPath, $sessionSchemaPath,
    $coveragePath, $compatibilityPath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical transport sync ALPN file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$syncSchema = Get-Content -LiteralPath $syncSchemaPath -Raw | ConvertFrom-Json
$sessionSchema = Get-Content -LiteralPath $sessionSchemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Boundary $manifest $migration $syncSchema $sessionSchema $coverage `
    $compatibility $transport $cli $rfc

if ($SelfTest) {
    $changedIdentifier = Copy-JsonObject $manifest
    $changedIdentifier.candidate.alpn = 'kilogram/m1/sync/cbor/2'
    Assert-Rejected {
        Assert-Boundary $changedIdentifier $migration $syncSchema $sessionSchema $coverage `
            $compatibility $transport $cli $rfc
    } 'changed_candidate_identifier'

    $prematureRegistration = Copy-JsonObject $manifest
    $prematureRegistration.history_compatible_successor.registered = $true
    Assert-Rejected {
        Assert-Boundary $prematureRegistration $migration $syncSchema $sessionSchema $coverage `
            $compatibility $transport $cli $rfc
    } 'premature_candidate_registration'

    $reusedLegacy = Copy-JsonObject $manifest
    $reusedLegacy.candidate.alpn = $reusedLegacy.active.alpn
    Assert-Rejected {
        Assert-Boundary $reusedLegacy $migration $syncSchema $sessionSchema $coverage `
            $compatibility $transport $cli $rfc
    } 'reused_legacy_identifier'

    $leakedCli = $cli + "`nCANONICAL_SYNC_ALPN_V1`n"
    Assert-Rejected {
        Assert-Boundary $manifest $migration $syncSchema $sessionSchema $coverage `
            $compatibility $transport $leakedCli $rfc
    } 'candidate_leaked_into_active_cli'
    Write-Output 'canonical_transport_sync_alpn_self_test=passed'
}

Write-Output 'canonical_transport_sync_alpn_boundary=verified'
Write-Output 'active_sync_alpn=kilogram/m0/sync/8'
Write-Output 'candidate_sync_alpn=kilogram/m1/sync/cbor/1'
Write-Output 'history_compatible_sync_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'candidate_identifier_allocated=true'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_wire_changed=false'
Write-Output 'network_executed=false'
