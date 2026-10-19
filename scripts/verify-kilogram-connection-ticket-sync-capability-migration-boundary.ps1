[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$migrationPath = Join-Path $workspace 'ACTIVE-CONNECTION-TICKET-SYNC-CAPABILITY.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$canonicalMigrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$offlinePath = Join-Path $workspace 'apps\kilogram-offline\src\lib.rs'
$capabilityCargoPath = Join-Path $workspace 'crates\kilogram-sync-capability\Cargo.toml'
$capabilityPath = Join-Path $workspace 'crates\kilogram-sync-capability\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0215-m1191-n-minus-one-production-interop.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactSequence {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualStrings = @($Actual | ForEach-Object { [string]$_ })
    if (($actualStrings -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-Boundary {
    param(
        [object]$Migration,
        [object]$TransportManifest,
        [object]$CanonicalMigration,
        [object]$Coverage,
        [object]$Compatibility,
        [string]$CliSource,
        [string]$OfflineSource,
        [string]$CapabilityCargo,
        [string]$CapabilitySource,
        [string]$Rfc
    )

    $legacyAlpn = 'kilogram/m0/sync/8'
    $historicalCanonicalAlpn = 'kilogram/m1/sync/cbor/1'
    $canonicalAlpn = 'kilogram/m1/sync/cbor/2'
    $ticketDomain = 'kilogram:connection-ticket-signature:v11\0'
    $capabilityDomain = 'kilogram:sync-transport-capability-signature:v2\0'

    if ([UInt64]$Migration.schema_version -ne 1 -or
        [string]$Migration.milestone -cne 'M1.1.91' -or
        [string]$Migration.status -cne 'active-m1-compatible-optional-extension' -or
        [string]$Migration.surface -cne 'connection-ticket' -or
        [int]$Migration.previous.ticket_version -ne 13 -or
        [string]$Migration.previous.signature_domain -cne
            'kilogram:connection-ticket-signature:v13\0' -or
        -not [bool]$Migration.previous.sync_transport_capability -or
        [bool]$Migration.previous.sync_transport_capability_optional -or
        [int]$Migration.current.ticket_version -ne 11 -or
        [string]$Migration.current.signature_domain -cne $ticketDomain -or
        -not [bool]$Migration.current.sync_transport_capability -or
        -not [bool]$Migration.current.sync_transport_capability_optional -or
        [string]$Migration.current.outer_signature_projection -cne
            'exact-m1.0.0-legacy-fields' -or
        [string]$Migration.current.extension_signature -cne
            'independent-device-signature-v2') {
        throw 'connection-ticket sync capability migration identity changed'
    }

    Assert-ExactSequence @($Migration.capability.active_protocols) @($legacyAlpn) `
        'active capability protocols'
    Assert-ExactSequence @($Migration.capability.candidate_protocols) @($canonicalAlpn) `
        'candidate capability protocols'
    Assert-ExactSequence @($Migration.capability.bindings) @(
        'listener-account-id',
        'listener-device-id',
        'authority-revision',
        'allowed-requester-account-id',
        'endpoint-id',
        'ticket-publication-channel-epoch',
        'prekey-directory-common-validity-window'
    ) 'capability bindings'
    if ([int]$Migration.capability.version -ne 2 -or
        [string]$Migration.capability.signature_domain -cne $capabilityDomain -or
        [bool]$Migration.capability.canonical_dial_enabled) {
        throw 'connection-ticket sync capability state changed'
    }

    if ([string]$Migration.migration.strategy -cne
            'accepted-m1-envelope-with-optional-independently-signed-extension' -or
        -not [bool]$Migration.migration.legacy_reader -or
        [bool]$Migration.migration.dual_reader -or
        -not [bool]$Migration.migration.optional_extension_dual_shape_reader -or
        [bool]$Migration.migration.superseded_v13_reader -or
        -not [bool]$Migration.migration.accepted_m1_reader_ignores_unknown_extension -or
        -not [bool]$Migration.migration.current_reader_accepts_missing_extension_as_legacy_only -or
        [bool]$Migration.migration.silent_downgrade -or
        [bool]$Migration.migration.extension_stripping_detectable_before_first_selection -or
        -not [bool]$Migration.migration.selected_session_downgrade_blocked -or
        -not [bool]$Migration.migration.listener_restart_reissues_current_ticket) {
        throw 'connection-ticket compatible extension migration policy changed'
    }
    if (-not [bool]$Migration.scope.changes_connection_ticket_bytes -or
        [bool]$Migration.scope.changes_active_sync_alpn -or
        [bool]$Migration.scope.registers_canonical_alpn -or
        [bool]$Migration.scope.dials_canonical_alpn -or
        [bool]$Migration.scope.changes_sync_frames -or
        -not [bool]$Migration.scope.network_executed) {
        throw 'connection-ticket migration scope changed'
    }

    foreach ($source in @($CliSource, $OfflineSource)) {
        foreach ($required in @(
            'const TICKET_VERSION: u8 = 11;',
            'kilogram:connection-ticket-signature:v11\0',
            'sync_transport_capability: Option<SignedSyncTransportCapability>',
            'SyncTransportCapabilityBinding'
        )) {
            if ($source.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
                throw "connection-ticket verifier is missing '$required'"
            }
        }
        if ($source.IndexOf('const TICKET_VERSION: u8 = 13;',
                [StringComparison]::Ordinal) -ge 0 -or
            $source.IndexOf('kilogram:connection-ticket-signature:v13\0',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'superseded incompatible connection-ticket envelope remains active'
        }
    }
    foreach ($required in @(
        'struct LegacyConnectionTicketContent',
        'let legacy = LegacyConnectionTicketContent {',
        'Missing extension means an N-1 legacy peer.'
    )) {
        if ($CliSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "online compatible ticket projection is missing '$required'"
        }
    }
    foreach ($required in @(
        'struct OfflineLegacyConnectionTicketContent',
        'let legacy = OfflineLegacyConnectionTicketContent {'
    )) {
        if ($OfflineSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "offline compatible ticket projection is missing '$required'"
        }
    }

    $cliOuterSignature = $CliSource.IndexOf(
        '.verify(&ticket_signing_bytes(&self.content)?, &self.signature)',
        [StringComparison]::Ordinal
    )
    $cliCapability = $CliSource.IndexOf(
        'verify_sync_transport_capability_binding(&self.content)?;',
        [StringComparison]::Ordinal
    )
    if ($cliOuterSignature -lt 0 -or $cliCapability -le $cliOuterSignature) {
        throw 'online verifier does not authenticate outer ticket before nested capability'
    }
    $offlineOuterSignature = $OfflineSource.IndexOf(
        '.verify(&ticket_signing_bytes(&self.content)?, &self.signature)',
        [StringComparison]::Ordinal
    )
    $offlineCapability = $OfflineSource.IndexOf(
        '.sync_transport_capability',
        $offlineOuterSignature,
        [StringComparison]::Ordinal
    )
    if ($offlineOuterSignature -lt 0 -or $offlineCapability -le $offlineOuterSignature -or
        $OfflineSource.IndexOf('.verify_binding(&SyncTransportCapabilityBinding {',
            $offlineCapability, [StringComparison]::Ordinal) -lt 0) {
        throw 'offline verifier does not authenticate outer ticket before nested capability'
    }

    if ($CapabilityCargo.IndexOf('kilogram-identity', [StringComparison]::Ordinal) -lt 0 -or
        $CapabilityCargo.IndexOf('iroh', [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
        $CapabilityCargo.IndexOf('tokio', [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
        $CapabilityCargo.IndexOf('reqwest', [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw 'shared sync capability crate dependency boundary changed'
    }
    foreach ($required in @(
        'pub const CANONICAL_SYNC_ALPN: &str = "kilogram/m1/sync/cbor/2";',
        'pub const LEGACY_SYNC_ALPN: &str = "kilogram/m0/sync/8";',
        'kilogram:sync-transport-capability-signature:v2\0',
        'pub const SYNC_TRANSPORT_CAPABILITY_VERSION: u8 = 2;',
        'pub struct SignedSyncTransportCapability',
        'pub struct SyncTransportCapabilityBinding',
        'issue_legacy_with_canonical_candidate',
        'SyncTransportCapabilityError::UnsafeProtocolState',
        'SyncTransportCapabilityError::BindingMismatch',
        'signed_capability_is_exact_bound_and_legacy_only',
        'previous_capability_and_candidate_generation_fail_closed',
        'changed_context_and_time_fail_closed'
    )) {
        if ($CapabilitySource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "shared sync capability source is missing '$required'"
        }
    }

    $compatibilitySurface = @($Compatibility.surfaces | Where-Object {
        [string]$_.id -ceq 'connection-ticket'
    })
    if ($compatibilitySurface.Count -ne 1) {
        throw 'connection-ticket compatibility surface is missing'
    }
    if ([string]$compatibilitySurface[0].compatibility -cne 'explicit-reader-set' -or
        [string]$compatibilitySurface[0].migration -cne 'new-version-explicit-migration') {
        throw 'connection-ticket compatibility policy changed'
    }
    Assert-ExactSequence @($compatibilitySurface[0].bindings | ForEach-Object {
        "$($_.path)|$($_.symbol)|$($_.rust_type)|$($_.value)"
    }) @(
        'apps/kilogram-cli/src/main.rs|TICKET_SIGNATURE_DOMAIN|bytes|kilogram:connection-ticket-signature:v11\0',
        'apps/kilogram-cli/src/main.rs|TICKET_VERSION|u8|11',
        'apps/kilogram-offline/src/lib.rs|TICKET_SIGNATURE_DOMAIN|bytes|kilogram:connection-ticket-signature:v11\0',
        'apps/kilogram-offline/src/lib.rs|TICKET_VERSION|u8|11',
        'crates/kilogram-sync-capability/src/lib.rs|CANONICAL_SYNC_ALPN|str|kilogram/m1/sync/cbor/2',
        'crates/kilogram-sync-capability/src/lib.rs|LEGACY_SYNC_ALPN|str|kilogram/m0/sync/8',
        'crates/kilogram-sync-capability/src/lib.rs|SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN|bytes|kilogram:sync-transport-capability-signature:v2\0',
        'crates/kilogram-sync-capability/src/lib.rs|SYNC_TRANSPORT_CAPABILITY_VERSION|u8|2'
    ) 'connection-ticket compatibility bindings'

    if ([string]$TransportManifest.milestone -cne 'M1.1.85' -or
        [string]$TransportManifest.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$TransportManifest.active.alpn -cne $legacyAlpn -or
        -not [bool]$TransportManifest.active.registered -or
        -not [bool]$TransportManifest.active.dialed -or
        [string]$TransportManifest.candidate.alpn -cne $historicalCanonicalAlpn -or
        -not [bool]$TransportManifest.candidate.allocated -or
        [bool]$TransportManifest.candidate.registered -or
        [bool]$TransportManifest.candidate.dialed -or
        [string]$TransportManifest.history_compatible_successor.alpn -cne
            $canonicalAlpn -or
        -not [bool]$TransportManifest.history_compatible_successor.authenticated_ticket_capability -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_runtime_state_machine -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_successor_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_mixed_version_and_downgrade_matrix -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_dual_reader_runtime_coordinator -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_deterministic_rollback_drill -or
        -not [bool]$TransportManifest.history_compatible_successor.coverage_guided_fuzz_harness -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [string]$TransportManifest.authenticated_capability.portable_artifact -cne
            'connection-ticket-v11-with-optional-capability-v2' -or
        [int]$TransportManifest.authenticated_capability.capability_version -ne 2 -or
        [string]$TransportManifest.authenticated_capability.signature_domain -cne
            $capabilityDomain -or
        (@($TransportManifest.authenticated_capability.active_protocols) -join ',') -cne
            $legacyAlpn -or
        (@($TransportManifest.authenticated_capability.candidate_protocols) -join ',') -cne
            $canonicalAlpn -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled -or
        -not [bool]$TransportManifest.scope.active_wire_changed -or
        [string]$TransportManifest.public_default -cne 'legacy-only' -or
        [bool]$TransportManifest.opt_in_dark_launch.registered_by_default -or
        [bool]$TransportManifest.opt_in_dark_launch.dialed_by_default -or
        -not [bool]$TransportManifest.scope.network_executed) {
        throw 'canonical transport remains-active boundary changed'
    }

    $capabilityRequirement = @($CanonicalMigration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'authenticated-capability-envelope'
    })
    if ([string]$CanonicalMigration.milestone -cne 'M1.1.85' -or
        [string]$CanonicalMigration.status -cne 'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$CanonicalMigration.current_state -cne 'legacy-only' -or
        [string]$CanonicalMigration.candidate_protocol_identifier -cne $canonicalAlpn -or
        [string]$CanonicalMigration.history_compatible_successor.authenticated_ticket_capability -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.dormant_signed_session_bootstrap -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.dormant_signed_session_successor_store -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -cne
            'verified' -or
        [string]$CanonicalMigration.history_compatible_successor.coverage_guided_fuzz_harness -cne
            'instrumented-sustained-verified' -or
        [bool]$CanonicalMigration.candidate_identifier_registered -or
        $capabilityRequirement.Count -ne 1 -or
        [string]$capabilityRequirement[0].status -cne 'verified') {
        throw 'canonical migration capability-envelope boundary changed'
    }

    $coverageSurface = @($Coverage.surfaces | Where-Object {
        [string]$_.id -ceq 'transport-sync'
    })
    if ([string]$Coverage.milestone -cne 'M1.1.85' -or
        $coverageSurface.Count -ne 1 -or
        [string]$coverageSurface[0].coverage -cne 'complete' -or
        (@($coverageSurface[0].covered) -join ',') -cne
            'parallel-cbor-alpn-identifier-allocation,history-compatible-cbor-v2-alpn-identifier-allocation,typed-durable-canonical-sync-session-checkpoint,authenticated-ticket-v11-optional-history-compatible-cbor-v2-capability,dormant-history-compatible-sync-v2-runtime-state-machine,dormant-atomic-typed-carrier-store,dormant-cross-format-membership-policy,dormant-bounded-resumable-legacy-import,dormant-atomic-legacy-import-checkpoint,dormant-signed-session-bootstrap,dormant-signed-session-successor-store,dormant-checkpoint-bound-runtime-frame-journal,dormant-atomic-runtime-transition-transaction,dormant-durable-client-transition-adapter,dormant-durable-responder-transition-adapter,dormant-mutual-authenticated-candidate-selection,dormant-mixed-version-and-downgrade-matrix,dormant-dual-reader-runtime-coordinator,dormant-deterministic-rollback-drill,bounded-canonical-sync-v2-decoder-mutation-corpus,coverage-guided-fuzz-harness,instrumented-fuzz-smoke,sustained-fuzz-campaign,fresh-hosted-and-field-evidence,device-signed-opt-in-activation-policy,production-runtime-dark-launch,two-host-resilience-rehearsal,aggregate-opt-in-dark-launch-acceptance,n-minus-one-production-executable-interop' -or
        @($coverageSurface[0].remaining).Count -ne 0) {
        throw 'canonical coverage did not record the authenticated capability envelope'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'outer ticket is now frozen at the exact M1.0.0 v11 projection',
        'optional, independently Device-signed extension',
        'old reader ignores the unknown JSON field',
        'absence means a legacy-only N-1 peer',
        'optional-extension stripping before the first selection is not proven detectable',
        'does not authorize public canonical rollout'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0215 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $migrationPath, $transportManifestPath, $canonicalMigrationPath, $coveragePath,
    $compatibilityPath, $cliPath, $offlinePath, $capabilityCargoPath,
    $capabilityPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "connection-ticket sync capability migration file is missing: $path"
    }
}

$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$canonicalMigration = Get-Content -LiteralPath $canonicalMigrationPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cli = Get-Content -LiteralPath $cliPath -Raw
$offline = Get-Content -LiteralPath $offlinePath -Raw
$capabilityCargo = Get-Content -LiteralPath $capabilityCargoPath -Raw
$capability = Get-Content -LiteralPath $capabilityPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Boundary $migration $transportManifest $canonicalMigration $coverage `
    $compatibility $cli $offline $capabilityCargo $capability $rfc

if ($SelfTest) {
    $forgedVersion = Copy-JsonObject $migration
    $forgedVersion.current.ticket_version = 13
    Assert-Rejected {
        Assert-Boundary $forgedVersion $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'forged_current_ticket_version'

    $enabledDial = Copy-JsonObject $migration
    $enabledDial.capability.canonical_dial_enabled = $true
    Assert-Rejected {
        Assert-Boundary $enabledDial $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'premature_canonical_dial'

    $downgradedCandidate = Copy-JsonObject $migration
    $downgradedCandidate.capability.candidate_protocols = @('kilogram/m1/sync/cbor/1')
    Assert-Rejected {
        Assert-Boundary $downgradedCandidate $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'history_incompatible_candidate'

    $droppedBinding = Copy-JsonObject $migration
    $droppedBinding.capability.bindings = @(
        $droppedBinding.capability.bindings | Where-Object { [string]$_ -cne 'endpoint-id' }
    )
    Assert-Rejected {
        Assert-Boundary $droppedBinding $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'dropped_endpoint_binding'

    $legacyReader = Copy-JsonObject $migration
    $legacyReader.migration.legacy_reader = $false
    Assert-Rejected {
        Assert-Boundary $legacyReader $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'accepted_m1_reader_removed'

    $dualShapeReader = Copy-JsonObject $migration
    $dualShapeReader.migration.optional_extension_dual_shape_reader = $false
    Assert-Rejected {
        Assert-Boundary $dualShapeReader $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'optional_extension_shape_reader_removed'

    $supersededReader = Copy-JsonObject $migration
    $supersededReader.migration.superseded_v13_reader = $true
    Assert-Rejected {
        Assert-Boundary $supersededReader $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'superseded_v13_reader_reintroduced'

    $prematureRegistration = Copy-JsonObject $transportManifest
    $prematureRegistration.history_compatible_successor.registered = $true
    Assert-Rejected {
        Assert-Boundary $migration $prematureRegistration $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'premature_canonical_registration'

    Write-Output 'connection_ticket_sync_capability_migration_self_test=passed'
}

Write-Output 'connection_ticket_sync_capability_migration_boundary=verified'
Write-Output 'connection_ticket_version=11'
Write-Output 'connection_ticket_legacy_reader=true'
Write-Output 'connection_ticket_version_dual_reader=false'
Write-Output 'connection_ticket_optional_extension_dual_shape_reader=true'
Write-Output 'connection_ticket_superseded_v13_reader=false'
Write-Output 'sync_transport_capability_optional=true'
Write-Output 'sync_transport_capability_version=2'
Write-Output 'active_sync_alpn=kilogram/m0/sync/8'
Write-Output 'candidate_sync_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'canonical_dial_enabled=false'
Write-Output 'active_sync_wire_changed=false'
Write-Output 'network_executed=true'
