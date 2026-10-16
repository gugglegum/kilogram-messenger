[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-V2-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-V2-VECTORS.json'
$legacyVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-V2.cddl'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_history_v2.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$activeWirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-history-v2-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0179-m1153-typed-history-compatible-sync-event-carrier.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-LowerHex {
    param([string]$Value, [int]$Bytes = -1, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Value) -or
        $Value -cnotmatch '^[0-9a-f]+$' -or $Value.Length % 2 -ne 0 -or
        ($Bytes -ge 0 -and $Value.Length -ne $Bytes * 2)) {
        throw "$Label is not canonical lowercase hexadecimal"
    }
}

function Assert-Contract {
    param(
        [object]$Schema,
        [object]$Vectors,
        [object]$Compatibility,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Coverage,
        [string]$Rust,
        [string]$Protocol,
        [string]$Transport,
        [string]$ActiveWire,
        [string]$Cli,
        [string]$Cddl,
        [string]$Rfc
    )
    $successorAlpn = 'kilogram/m1/sync/cbor/2'
    if ([UInt64]$Schema.schema_version -ne 1 -or
        [string]$Schema.milestone -cne 'M1.1.53' -or
        [string]$Schema.status -cne 'typed-history-carrier-verified-frames-pending' -or
        [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.candidate_protocol_identifier -cne $successorAlpn -or
        -not [bool]$Schema.candidate_identifier_allocated -or
        [bool]$Schema.candidate_identifier_registered -or
        [bool]$Schema.candidate_identifier_dialed -or
        [string]$Schema.cddl.path -cne 'CANONICAL-SYNC-HISTORY-V2.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-history-v2' -or
        [int]$Schema.limits.maximum_encoded_object_bytes -ne 8388608) {
        throw 'canonical sync history-v2 schema or activation boundary changed'
    }
    if ([string]$Schema.event_namespaces.'1'.id -cne 'legacy-postcard-v6' -or
        [string]$Schema.event_namespaces.'1'.event_id_domain -cne
            "kilogram:event-id:v5`0" -or
        [string]$Schema.event_namespaces.'1'.payload -cne
            'sync-legacy-event-bridge-v1' -or
        [string]$Schema.event_namespaces.'2'.id -cne 'canonical-cbor-v1' -or
        [string]$Schema.event_namespaces.'2'.event_id_domain -cne
            "kilogram:event-id:cbor-v1`0" -or
        [string]$Schema.event_namespaces.'2'.payload -cne 'authorized-event-v1') {
        throw 'canonical sync history-v2 namespace registry changed'
    }
    if ((@($Schema.remaining_before_activation) -join ',') -cne
        'closed-history-compatible-sync-frames,typed-session-checkpoints,authenticated-ticket-capability-for-exact-cbor-v2-identifier,dual-reader-runtime-and-downgrade-tests') {
        throw 'canonical sync history-v2 remaining activation work changed'
    }

    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.53' -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync history-v2 vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.expected.legacy_key_cbor_hex) -1 'legacy key'
    Assert-LowerHex ([string]$Vectors.expected.canonical_key_cbor_hex) -1 'canonical key'
    Assert-LowerHex ([string]$Vectors.expected.legacy_carrier_cbor_hex) -1 'legacy carrier'
    Assert-LowerHex ([string]$Vectors.expected.canonical_carrier_cbor_hex) -1 `
        'canonical carrier'
    if ([string]$Vectors.expected.legacy_key_cbor_hex -cnotmatch '^8301015820[0-9a-f]{64}$' -or
        [string]$Vectors.expected.canonical_key_cbor_hex -cnotmatch '^8301025820[0-9a-f]{64}$' -or
        [string]$Vectors.expected.legacy_key_cbor_hex -ceq
            [string]$Vectors.expected.canonical_key_cbor_hex) {
        throw 'canonical sync history-v2 typed key namespace collapsed'
    }

    if ([string]$Compatibility.required_successor.explicit_protocol_identifier -cne
            $successorAlpn -or
        -not [bool]$Compatibility.required_successor.identifier_allocated -or
        -not [bool]$Compatibility.required_successor.typed_event_key_integrated -or
        -not [bool]$Compatibility.required_successor.closed_event_carrier_integrated -or
        -not [bool]$Compatibility.required_successor.closed_sync_frames_integrated -or
        -not [bool]$Compatibility.required_successor.typed_session_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.ticket_capability_authenticated -or
        -not [bool]$Compatibility.required_successor.dormant_runtime_state_machine_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_typed_store_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_cross_format_membership_policy_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_bounded_resumable_legacy_import_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_legacy_import_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_bootstrap_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_successor_store_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-checkpoint-session-bootstrap-successor-store-runtime-frame-journal-atomic-transition-and-durable-client-adapter-verified-not-active') {
        throw 'canonical sync history compatibility ledger changed'
    }
    if ([string]$Migration.history_compatible_successor.protocol_identifier -cne
            $successorAlpn -or
        -not [bool]$Migration.history_compatible_successor.identifier_allocated -or
        [string]$Migration.history_compatible_successor.typed_event_identity -cne 'verified' -or
        [string]$Migration.history_compatible_successor.closed_event_carrier -cne 'verified' -or
        [string]$Migration.history_compatible_successor.closed_sync_frames -cne 'verified' -or
        [string]$Migration.history_compatible_successor.typed_session_checkpoint -cne 'verified' -or
        [string]$Migration.history_compatible_successor.authenticated_ticket_capability -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_runtime_state_machine -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_typed_store -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_cross_format_membership_policy -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_bootstrap -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_successor_store -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed -or
        [string]$Migration.current_state -cne 'legacy-only') {
        throw 'canonical migration history-compatible successor changed'
    }
    $carrierRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'history-compatible-typed-event-carrier'
    })
    if ($carrierRequirement.Count -ne 1 -or
        [string]$carrierRequirement[0].status -cne 'verified') {
        throw 'canonical migration omitted verified typed event carrier'
    }
    if ([string]$TransportManifest.history_compatible_successor.alpn -cne $successorAlpn -or
        -not [bool]$TransportManifest.history_compatible_successor.allocated -or
        -not [bool]$TransportManifest.history_compatible_successor.closed_sync_frames -or
        -not [bool]$TransportManifest.history_compatible_successor.typed_session_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.authenticated_ticket_capability -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_runtime_state_machine -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_successor_store -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled) {
        throw 'canonical transport history-compatible successor changed'
    }

    $syncCoverage = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'sync' })
    $transportCoverage = @($Coverage.surfaces | Where-Object {
        [string]$_.id -ceq 'transport-sync'
    })
    foreach ($artifact in @(
        'CANONICAL-SYNC-HISTORY-V2.cddl',
        'CANONICAL-SYNC-HISTORY-V2-SCHEMA.json',
        'CANONICAL-SYNC-HISTORY-V2-VECTORS.json',
        'crates/kilogram-protocol/src/canonical_sync_history_v2.rs',
        'scripts/verify-kilogram-canonical-sync-history-v2-reference.mjs',
        'scripts/verify-kilogram-canonical-sync-history-v2-boundary.ps1'
    )) {
        if ($syncCoverage.Count -ne 1 -or $artifact -cnotin @($syncCoverage[0].artifacts)) {
            throw "canonical sync coverage is missing history-v2 artifact '$artifact'"
        }
    }
    if ($transportCoverage.Count -ne 1 -or
        'history-compatible-cbor-v2-alpn-identifier-allocation' -cnotin
            @($transportCoverage[0].covered)) {
        throw 'transport coverage omitted history-compatible cbor-v2 allocation'
    }

    foreach ($required in @(
        'pub enum CanonicalSyncEventNamespaceV2',
        'pub struct CanonicalSyncEventKeyV2',
        'pub enum CanonicalSyncEventCarrierV2',
        'LegacyPostcardV6(CanonicalSyncLegacyEventBridgeV1)',
        'CanonicalCborV1(CanonicalAuthorizedEventV1)',
        'CanonicalSyncLegacyEventBridgeV1::decode_and_verify',
        'decode_and_verify_authorized_event_v1',
        'UnknownEventNamespace',
        'MAX_CANONICAL_SYNC_HISTORY_V2_OBJECT_BYTES'
    )) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync history-v2 Rust codec is missing '$required'"
        }
    }
    if ($Protocol.IndexOf('pub mod canonical_sync_history_v2;',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical sync history-v2 module is not exported'
    }
    foreach ($required in @(
        'pub const CANONICAL_SYNC_ALPN_V2: &[u8] = b"kilogram/m1/sync/cbor/2";',
        'CanonicalCborV2',
        'Self::CanonicalCborV2 => CANONICAL_SYNC_ALPN_V2'
    )) {
        if ($Transport.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "transport is missing inactive cbor-v2 allocation '$required'"
        }
    }
    foreach ($activeSource in @($ActiveWire, $Cli)) {
        if ($activeSource.IndexOf('CANONICAL_SYNC_ALPN_V2',
                [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf($successorAlpn, [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf('CanonicalSyncEventCarrierV2',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'inactive history-compatible sync successor leaked into active runtime'
        }
    }
    foreach ($required in @(
        'sync-event-namespace-v2', 'sync-event-key-v2', 'sync-event-carrier-v2',
        'kilogram-sync-history-v2'
    )) {
        if ($Cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync history-v2 CDDL is missing '$required'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'bare 32-byte Event ID is ambiguous',
        'ordered pair',
        'closed carrier',
        '`kilogram/m1/sync/cbor/2`',
        'not registered or dialed',
        'does not change active wire bytes'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0179 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $schemaPath, $vectorsPath, $legacyVectorsPath, $eventVectorsPath, $cddlPath,
    $compatibilityPath, $migrationPath, $transportManifestPath, $coveragePath,
    $rustPath, $protocolPath, $transportPath, $activeWirePath, $cliPath,
    $referencePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync history-v2 file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$activeWire = Get-Content -LiteralPath $activeWirePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $schema $vectors $compatibility $migration $transportManifest `
    $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --legacy-vectors $legacyVectorsPath --event-vectors $eventVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_history_v2_reference_self_test=passed' -notin
        @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync history-v2 independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $registered = Copy-JsonObject $schema
    $registered.candidate_identifier_registered = $true
    Assert-Rejected {
        Assert-Contract $registered $vectors $compatibility $migration $transportManifest `
            $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc
    } 'premature_history_v2_registration'

    $namespace = Copy-JsonObject $schema
    $namespace.event_namespaces.'1'.id = 'canonical-cbor-v1'
    Assert-Rejected {
        Assert-Contract $namespace $vectors $compatibility $migration $transportManifest `
            $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc
    } 'changed_event_namespace'

    $collision = Copy-JsonObject $vectors
    $collision.expected.canonical_key_cbor_hex = $collision.expected.legacy_key_cbor_hex
    Assert-Rejected {
        Assert-Contract $schema $collision $compatibility $migration $transportManifest `
            $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc
    } 'collapsed_typed_event_key'
    Write-Output 'canonical_sync_history_v2_self_test=passed'
}

Write-Output 'canonical_sync_history_v2_boundary=verified'
Write-Output 'history_compatible_sync_alpn=kilogram/m1/sync/cbor/2'
Write-Output 'typed_event_namespaces=2'
Write-Output 'closed_event_carriers=2'
Write-Output 'closed_sync_frames=verified'
Write-Output 'authenticated_ticket_capability=verified'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
