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
$rfcPath = Join-Path $workspace 'docs\RFC-0177-m1151-authenticated-sync-transport-capability.md'

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
    $canonicalAlpn = 'kilogram/m1/sync/cbor/1'
    $ticketDomain = 'kilogram:connection-ticket-signature:v12\0'
    $capabilityDomain = 'kilogram:sync-transport-capability-signature:v1\0'

    if ([UInt64]$Migration.schema_version -ne 1 -or
        [string]$Migration.milestone -cne 'M1.1.51' -or
        [string]$Migration.status -cne 'active-portable-artifact-migration' -or
        [string]$Migration.surface -cne 'connection-ticket' -or
        [int]$Migration.previous.ticket_version -ne 11 -or
        [string]$Migration.previous.signature_domain -cne
            'kilogram:connection-ticket-signature:v11\0' -or
        [bool]$Migration.previous.sync_transport_capability -or
        [int]$Migration.current.ticket_version -ne 12 -or
        [string]$Migration.current.signature_domain -cne $ticketDomain -or
        -not [bool]$Migration.current.sync_transport_capability) {
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
    if ([int]$Migration.capability.version -ne 1 -or
        [string]$Migration.capability.signature_domain -cne $capabilityDomain -or
        [bool]$Migration.capability.canonical_dial_enabled) {
        throw 'connection-ticket sync capability state changed'
    }

    if ([string]$Migration.migration.strategy -cne
            'exact-version-hard-cut-for-experimental-portable-ticket' -or
        [bool]$Migration.migration.legacy_reader -or
        [bool]$Migration.migration.dual_reader -or
        [bool]$Migration.migration.silent_downgrade -or
        -not [bool]$Migration.migration.listener_restart_reissues_current_ticket) {
        throw 'connection-ticket hard-cut migration policy changed'
    }
    if (-not [bool]$Migration.scope.changes_connection_ticket_bytes -or
        [bool]$Migration.scope.changes_active_sync_alpn -or
        [bool]$Migration.scope.registers_canonical_alpn -or
        [bool]$Migration.scope.dials_canonical_alpn -or
        [bool]$Migration.scope.changes_sync_frames -or
        [bool]$Migration.scope.network_executed) {
        throw 'connection-ticket migration scope changed'
    }

    foreach ($source in @($CliSource, $OfflineSource)) {
        foreach ($required in @(
            'const TICKET_VERSION: u8 = 12;',
            'kilogram:connection-ticket-signature:v12\0',
            'sync_transport_capability: SignedSyncTransportCapability',
            'SyncTransportCapabilityBinding'
        )) {
            if ($source.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
                throw "connection-ticket verifier is missing '$required'"
            }
        }
        if ($source.IndexOf('const TICKET_VERSION: u8 = 11;',
                [StringComparison]::Ordinal) -ge 0 -or
            $source.IndexOf('kilogram:connection-ticket-signature:v11\0',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'legacy connection-ticket reader remains active'
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
        'pub const CANONICAL_SYNC_ALPN: &str = "kilogram/m1/sync/cbor/1";',
        'pub const LEGACY_SYNC_ALPN: &str = "kilogram/m0/sync/8";',
        'kilogram:sync-transport-capability-signature:v1\0',
        'pub const SYNC_TRANSPORT_CAPABILITY_VERSION: u8 = 1;',
        'pub struct SignedSyncTransportCapability',
        'pub struct SyncTransportCapabilityBinding',
        'issue_legacy_with_canonical_candidate',
        'SyncTransportCapabilityError::UnsafeProtocolState',
        'SyncTransportCapabilityError::BindingMismatch',
        'signed_capability_is_exact_bound_and_legacy_only',
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
    Assert-ExactSequence @($compatibilitySurface[0].bindings | ForEach-Object {
        "$($_.path)|$($_.symbol)|$($_.rust_type)|$($_.value)"
    }) @(
        'apps/kilogram-cli/src/main.rs|TICKET_SIGNATURE_DOMAIN|bytes|kilogram:connection-ticket-signature:v12\0',
        'apps/kilogram-cli/src/main.rs|TICKET_VERSION|u8|12',
        'apps/kilogram-offline/src/lib.rs|TICKET_SIGNATURE_DOMAIN|bytes|kilogram:connection-ticket-signature:v12\0',
        'apps/kilogram-offline/src/lib.rs|TICKET_VERSION|u8|12',
        'crates/kilogram-sync-capability/src/lib.rs|CANONICAL_SYNC_ALPN|str|kilogram/m1/sync/cbor/1',
        'crates/kilogram-sync-capability/src/lib.rs|LEGACY_SYNC_ALPN|str|kilogram/m0/sync/8',
        'crates/kilogram-sync-capability/src/lib.rs|SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN|bytes|kilogram:sync-transport-capability-signature:v1\0',
        'crates/kilogram-sync-capability/src/lib.rs|SYNC_TRANSPORT_CAPABILITY_VERSION|u8|1'
    ) 'connection-ticket compatibility bindings'

    if ([string]$TransportManifest.milestone -cne 'M1.1.54' -or
        [string]$TransportManifest.status -cne 'history-compatible-successor-frames-verified-not-active' -or
        [string]$TransportManifest.active.alpn -cne $legacyAlpn -or
        -not [bool]$TransportManifest.active.registered -or
        -not [bool]$TransportManifest.active.dialed -or
        [string]$TransportManifest.candidate.alpn -cne $canonicalAlpn -or
        -not [bool]$TransportManifest.candidate.allocated -or
        [bool]$TransportManifest.candidate.registered -or
        [bool]$TransportManifest.candidate.dialed -or
        [string]$TransportManifest.history_compatible_successor.alpn -cne
            'kilogram/m1/sync/cbor/2' -or
        [bool]$TransportManifest.history_compatible_successor.authenticated_ticket_capability -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled -or
        [bool]$TransportManifest.scope.active_wire_changed -or
        [bool]$TransportManifest.scope.network_executed) {
        throw 'canonical transport remains-active boundary changed'
    }

    $capabilityRequirement = @($CanonicalMigration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'authenticated-capability-envelope'
    })
    if ([string]$CanonicalMigration.milestone -cne 'M1.1.54' -or
        [string]$CanonicalMigration.status -cne 'history-compatible-successor-frames-verified-not-active' -or
        [string]$CanonicalMigration.current_state -cne 'legacy-only' -or
        [bool]$CanonicalMigration.candidate_identifier_registered -or
        $capabilityRequirement.Count -ne 1 -or
        [string]$capabilityRequirement[0].status -cne 'verified') {
        throw 'canonical migration capability-envelope boundary changed'
    }

    $coverageSurface = @($Coverage.surfaces | Where-Object {
        [string]$_.id -ceq 'transport-sync'
    })
    if ([string]$Coverage.milestone -cne 'M1.1.54' -or
        $coverageSurface.Count -ne 1 -or
        [string]$coverageSurface[0].coverage -cne 'partial' -or
        (@($coverageSurface[0].covered) -join ',') -cne
            'parallel-cbor-alpn-identifier-allocation,authenticated-ticket-v12-transport-capability-envelope,history-compatible-cbor-v2-alpn-identifier-allocation' -or
        (@($coverageSurface[0].remaining) -join ',') -cne
            'typed-durable-canonical-sync-session-checkpoint,authenticated-cbor-v2-capability,active-dual-reader-and-canonical-capability-activation') {
        throw 'canonical coverage did not record the authenticated capability envelope'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'Connection ticket v12 contains a separately Device-signed synchronization transport capability',
        'hard cut from v11 to v12',
        'there is no legacy reader or silent downgrade',
        'first authenticate the outer ticket',
        'canonical ALPN remains unregistered and is never dialed',
        'Format sniffing, parse-error fallback and retrying a failed canonical operation over legacy remain forbidden'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0177 is missing '$required'"
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
    $forgedVersion.current.ticket_version = 11
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

    $droppedBinding = Copy-JsonObject $migration
    $droppedBinding.capability.bindings = @(
        $droppedBinding.capability.bindings | Where-Object { [string]$_ -cne 'endpoint-id' }
    )
    Assert-Rejected {
        Assert-Boundary $droppedBinding $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'dropped_endpoint_binding'

    $dualReader = Copy-JsonObject $migration
    $dualReader.migration.dual_reader = $true
    Assert-Rejected {
        Assert-Boundary $dualReader $transportManifest $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'undeclared_dual_reader'

    $prematureRegistration = Copy-JsonObject $transportManifest
    $prematureRegistration.candidate.registered = $true
    Assert-Rejected {
        Assert-Boundary $migration $prematureRegistration $canonicalMigration $coverage `
            $compatibility $cli $offline $capabilityCargo $capability $rfc
    } 'premature_canonical_registration'

    Write-Output 'connection_ticket_sync_capability_migration_self_test=passed'
}

Write-Output 'connection_ticket_sync_capability_migration_boundary=verified'
Write-Output 'connection_ticket_version=12'
Write-Output 'connection_ticket_legacy_reader=false'
Write-Output 'sync_transport_capability_version=1'
Write-Output 'active_sync_alpn=kilogram/m0/sync/8'
Write-Output 'candidate_sync_alpn=kilogram/m1/sync/cbor/1'
Write-Output 'canonical_dial_enabled=false'
Write-Output 'active_sync_wire_changed=false'
Write-Output 'network_executed=false'
