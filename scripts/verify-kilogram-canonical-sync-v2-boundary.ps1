[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-V2-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-V2-VECTORS.json'
$historyVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-V2-VECTORS.json'
$legacyVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC-V2.cddl'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_v2.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$activeWirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-v2-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0180-m1154-history-compatible-canonical-sync-v2-frames.md'

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
        [string]$Schema.milestone -cne 'M1.1.54' -or
        [string]$Schema.status -cne
            'history-compatible-frames-verified-session-and-capability-pending' -or
        [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-SYNC-V2.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-v2' -or
        [string]$Schema.activation.current_runtime_codec -cne 'postcard-v6' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.allocated_protocol_identifier -cne $successorAlpn -or
        [bool]$Schema.activation.registered_or_dialed -or
        [bool]$Schema.activation.advertised_by_authenticated_ticket) {
        throw 'canonical sync v2 schema or inactive activation boundary changed'
    }
    if ([int]$Schema.limits.maximum_inventory_event_keys -ne 4096 -or
        [int]$Schema.limits.maximum_events_per_batch -ne 64 -or
        [int]$Schema.limits.maximum_encoded_object_bytes -ne 8388608 -or
        [string]$Schema.domains[0].value -cne
            ("kilogram:sync-inventory-signature:cbor-v2" + [char]0) -or
        [string]$Schema.domains[1].value -cne
            ("kilogram:sync-diff-signature:cbor-v2" + [char]0)) {
        throw 'canonical sync v2 bounds or signature domains changed'
    }
    $objectIds = @($Schema.objects | ForEach-Object { [string]$_.id })
    $expectedObjectIds = @(
        'signed-sync-inventory-v2', 'sync-event-batch-v2', 'signed-sync-diff-v2',
        'sync-complete-v2', 'sync-pause-v2', 'sync-paused-v2', 'sync-rejected-v2',
        'sync-client-frame-v2', 'sync-server-frame-v2'
    )
    if (($objectIds -join ',') -cne ($expectedObjectIds -join ',')) {
        throw 'canonical sync v2 closed object inventory changed'
    }
    if ([string]$Schema.variant_registry.client.'1' -cne 'signed-sync-inventory-v2' -or
        [string]$Schema.variant_registry.client.'2' -cne 'sync-event-batch-v2' -or
        [string]$Schema.variant_registry.client.'3' -cne 'sync-pause-v2' -or
        [string]$Schema.variant_registry.server.'1' -cne 'signed-sync-diff-v2' -or
        [string]$Schema.variant_registry.server.'2' -cne 'sync-complete-v2' -or
        [string]$Schema.variant_registry.server.'3' -cne 'sync-rejected-v2' -or
        [string]$Schema.variant_registry.server.'4' -cne 'sync-paused-v2' -or
        [string]$Schema.variant_registry.rejection_reason.'1' -cne
            'requester-not-allowed') {
        throw 'canonical sync v2 closed variant registry changed'
    }

    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.54' -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync v2 vector header changed'
    }
    foreach ($entry in @(
        @([string]$Vectors.expected.inventory.signed_cbor_hex, 'signed inventory'),
        @([string]$Vectors.expected.diff.signed_cbor_hex, 'signed diff'),
        @([string]$Vectors.expected.complete.cbor_hex, 'complete'),
        @([string]$Vectors.expected.client_frames.inventory_cbor_hex, 'inventory frame'),
        @([string]$Vectors.expected.server_frames.diff_cbor_hex, 'diff frame')
    )) {
        Assert-LowerHex $entry[0] -1 $entry[1]
    }
    if ([int]$Vectors.expected.event_batch.encoded_length -ne 650 -or
        [string]$Vectors.expected.event_batch.nested_source -cne
            'CANONICAL-SYNC-HISTORY-V2-VECTORS.json#/expected/legacy_carrier_cbor_hex' -or
        [int]$Vectors.expected.client_frames.events.encoded_length -ne 656) {
        throw 'canonical sync v2 nested carrier vector boundary changed'
    }

    if ([string]$Compatibility.milestone -cne 'M1.1.63' -or
        [string]$Compatibility.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.closed_sync_frames_integrated -or
        -not [bool]$Compatibility.required_successor.typed_session_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.ticket_capability_authenticated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_typed_store_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_cross_format_membership_policy_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_bounded_resumable_legacy_import_integrated) {
        throw 'canonical sync compatibility frame ledger changed'
    }
    if (-not [bool]$Compatibility.required_successor.dormant_atomic_legacy_import_checkpoint_integrated) {
        throw 'canonical sync compatibility import checkpoint ledger changed'
    }
    if (-not [bool]$Compatibility.required_successor.dormant_signed_session_bootstrap_integrated) {
        throw 'canonical sync compatibility signed bootstrap ledger changed'
    }
    if ([string]$Migration.milestone -cne 'M1.1.63' -or
        [string]$Migration.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        [string]$Migration.history_compatible_successor.closed_sync_frames -cne 'verified' -or
        [string]$Migration.history_compatible_successor.typed_session_checkpoint -cne 'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration sync v2 frame ledger changed'
    }
    $frameRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'history-compatible-canonical-sync-frames'
    })
    if ($frameRequirement.Count -ne 1 -or
        [string]$frameRequirement[0].status -cne 'verified') {
        throw 'canonical migration omitted verified history-compatible frames'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.63' -or
        [string]$TransportManifest.history_compatible_successor.alpn -cne $successorAlpn -or
        -not [bool]$TransportManifest.history_compatible_successor.closed_sync_frames -or
        -not [bool]$TransportManifest.history_compatible_successor.typed_session_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.authenticated_ticket_capability -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled) {
        throw 'canonical transport sync v2 frame boundary changed'
    }

    $syncCoverage = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'sync' })
    foreach ($artifact in @(
        'CANONICAL-SYNC-V2.cddl', 'CANONICAL-SYNC-V2-SCHEMA.json',
        'CANONICAL-SYNC-V2-VECTORS.json',
        'crates/kilogram-protocol/src/canonical_sync_v2.rs',
        'scripts/verify-kilogram-canonical-sync-v2-reference.mjs',
        'scripts/verify-kilogram-canonical-sync-v2-boundary.ps1'
    )) {
        if ($syncCoverage.Count -ne 1 -or $artifact -cnotin @($syncCoverage[0].artifacts)) {
            throw "canonical sync coverage is missing v2 artifact '$artifact'"
        }
    }
    if ('history-compatible-typed-sync-v2-frames' -cnotin @($syncCoverage[0].covered)) {
        throw 'canonical sync coverage omitted typed v2 frames'
    }

    foreach ($required in @(
        'pub struct CanonicalSyncInventoryContentV2',
        'pub struct CanonicalSyncDiffContentV2',
        'pub enum CanonicalSyncClientFrameV2',
        'pub enum CanonicalSyncServerFrameV2',
        'decode_and_verify_signed_sync_inventory_v2',
        'decode_and_verify_signed_sync_diff_v2',
        'decode_and_verify_sync_client_frame_v2',
        'decode_and_verify_sync_server_frame_v2',
        'verify_sync_frame_session_v2',
        'NonCanonicalEventKeyOrder', 'NonCanonicalEventOrder',
        'EventConversationMismatch', 'InvalidBoolean',
        'MAX_CANONICAL_SYNC_V2_OBJECT_BYTES'
    )) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 Rust codec is missing '$required'"
        }
    }
    if ($Protocol.IndexOf('pub mod canonical_sync_v2;', [StringComparison]::Ordinal) -lt 0 -or
        $Transport.IndexOf('CANONICAL_SYNC_ALPN_V2', [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical sync v2 module or allocated ALPN is missing'
    }
    foreach ($activeSource in @($ActiveWire, $Cli)) {
        if ($activeSource.IndexOf('CANONICAL_SYNC_ALPN_V2', [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf('CanonicalSyncClientFrameV2', [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf($successorAlpn, [StringComparison]::Ordinal) -ge 0) {
            throw 'inactive canonical sync v2 frames leaked into active runtime'
        }
    }
    foreach ($required in @(
        'sync-inventory-content-v2', 'sync-event-batch-v2', 'sync-diff-content-v2',
        'sync-complete-v2', 'sync-client-frame-v2', 'sync-server-frame-v2',
        'kilogram-sync-v2'
    )) {
        if ($Cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 CDDL is missing '$required'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'complete closed frame codec', 'typed Event identity', 'does not use format sniffing',
        'not registered or dialed', 'does not change active wire bytes',
        'typed durable canonical sync session checkpoint'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0180 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $schemaPath, $vectorsPath, $historyVectorsPath, $legacyVectorsPath,
    $eventVectorsPath, $cddlPath, $compatibilityPath, $migrationPath,
    $transportManifestPath, $coveragePath, $rustPath, $protocolPath,
    $transportPath, $activeWirePath, $cliPath, $referencePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 file is missing: $path"
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

$referenceArguments = @(
    $referencePath, '--schema', $schemaPath, '--vectors', $vectorsPath,
    '--history-vectors', $historyVectorsPath, '--legacy-vectors', $legacyVectorsPath,
    '--event-vectors', $eventVectorsPath, '--self-test'
)
$referenceOutput = @(& node @referenceArguments 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_v2_reference_self_test=passed' -notin
        @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync v2 independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activated = Copy-JsonObject $schema
    $activated.activation.registered_or_dialed = $true
    Assert-Rejected {
        Assert-Contract $activated $vectors $compatibility $migration $transportManifest `
            $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc
    } 'premature_sync_v2_activation'

    $variant = Copy-JsonObject $schema
    $variant.variant_registry.client.'1' = 'sync-event-batch-v2'
    Assert-Rejected {
        Assert-Contract $variant $vectors $compatibility $migration $transportManifest `
            $coverage $rust $protocol $transport $activeWire $cli $cddl $rfc
    } 'changed_sync_v2_variant_registry'
    Write-Output 'canonical_sync_v2_self_test=passed'
}

Write-Output 'canonical_sync_v2_boundary=verified'
Write-Output 'history_compatible_sync_frames=verified'
Write-Output 'typed_session_checkpoint=verified'
Write-Output 'authenticated_ticket_capability=verified'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
