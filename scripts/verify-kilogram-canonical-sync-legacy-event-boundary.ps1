[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-LEGACY-EVENT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC-LEGACY-EVENT.cddl'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$syncSchemaPath = Join-Path $workspace 'CANONICAL-SYNC-SCHEMA.json'
$sessionSchemaPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_legacy_event_v1.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeWirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-legacy-event-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0178-m1152-canonical-sync-historical-signature-bridge.md'

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
        $Value -cnotmatch '^[0-9a-f]+$' -or
        $Value.Length % 2 -ne 0 -or
        ($Bytes -ge 0 -and $Value.Length -ne $Bytes * 2)) {
        throw "$Label is not canonical lowercase hexadecimal"
    }
}

function Assert-BridgeContract {
    param(
        [object]$Schema,
        [object]$Vectors,
        [object]$Compatibility,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$SyncSchema,
        [object]$SessionSchema,
        [object]$Coverage,
        [string]$Rust,
        [string]$Protocol,
        [string]$ActiveWire,
        [string]$Transport,
        [string]$Cli,
        [string]$Cddl,
        [string]$Rfc
    )
    $legacyAlpn = 'kilogram/m0/sync/8'
    $oldCanonicalAlpn = 'kilogram/m1/sync/cbor/1'

    if ([UInt64]$Schema.schema_version -ne 1 -or
        [string]$Schema.milestone -cne 'M1.1.52' -or
        [string]$Schema.status -cne 'candidate-prerequisite-not-active' -or
        [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.purpose -cne
            'preserve-and-reverify-exact-active-postcard-authorized-events-without-resigning') {
        throw 'canonical sync legacy-event bridge schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-SYNC-LEGACY-EVENT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-legacy-event-v1' -or
        [string]$Schema.source.encoding -cne 'postcard-v5-authorized-event' -or
        -not [bool]$Schema.source.signature_preimages_preserved -or
        [bool]$Schema.source.original_author_private_keys_required -or
        [string]$Schema.source.native_canonical_event_conversion -cne
            'forbidden-without-original-author-signatures') {
        throw 'canonical sync legacy-event source-preservation contract changed'
    }
    if ([string]$Schema.integration.current_sync_codec -cne 'postcard-v6' -or
        [bool]$Schema.integration.current_sync_changed -or
        [string]$Schema.integration.canonical_sync_v1_identifier -cne $oldCanonicalAlpn -or
        [bool]$Schema.integration.canonical_sync_v1_can_carry_bridge -or
        [string]$Schema.integration.required_next -cne
            'allocate-history-compatible-canonical-sync-generation-before-activation' -or
        [bool]$Schema.integration.registered -or [bool]$Schema.integration.dialed -or
        [int]$Schema.limits.maximum_bridge_bytes -ne 8388608 -or
        [int]$Schema.limits.maximum_active_authorized_event_postcard_bytes -ne 8388352) {
        throw 'canonical sync legacy-event activation or resource boundary changed'
    }

    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.52' -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync legacy-event vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.input.active_authorized_event_postcard_hex) -1 `
        'active AuthorizedEvent Postcard bytes'
    foreach ($field in @('conversation_id_hex', 'event_id_hex', 'author_account_id_hex',
        'author_device_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    Assert-LowerHex ([string]$Vectors.expected.bridge_cbor_hex) -1 'bridge CBOR'

    if ([UInt64]$Compatibility.schema_version -ne 1 -or
        [string]$Compatibility.milestone -cne 'M1.1.59' -or
        [string]$Compatibility.status -cne
            'dormant-cross-format-membership-policy-verified-not-active' -or
        [bool]$Compatibility.changes_active_wire -or
        [string]$Compatibility.active.alpn -cne $legacyAlpn -or
        -not [bool]$Compatibility.active.history_compatible -or
        -not [bool]$Compatibility.active.registered -or
        -not [bool]$Compatibility.active.dialed -or
        [string]$Compatibility.existing_canonical_candidate.alpn -cne $oldCanonicalAlpn -or
        [bool]$Compatibility.existing_canonical_candidate.installed_active_history_compatible -or
        [bool]$Compatibility.existing_canonical_candidate.registered -or
        [bool]$Compatibility.existing_canonical_candidate.dialed -or
        [string]$Compatibility.existing_canonical_candidate.activation_disposition -cne
            'blocked-before-activation') {
        throw 'canonical sync historical compatibility state changed'
    }
    if ([string]$Compatibility.bridge.schema -cne
            'CANONICAL-SYNC-LEGACY-EVENT-SCHEMA.json' -or
        [string]$Compatibility.bridge.rust_type -cne 'CanonicalSyncLegacyEventBridgeV1' -or
        -not [bool]$Compatibility.bridge.nested_active_signature_verification -or
        -not [bool]$Compatibility.bridge.exact_nested_reencoding_required -or
        [bool]$Compatibility.bridge.original_author_private_keys_required -or
        (@($Compatibility.bridge.metadata_bindings) -join ',') -cne
            'conversation-id,active-event-id,author-account-id,author-device-id' -or
        [string]$Compatibility.required_successor.explicit_protocol_identifier -cne
            'kilogram/m1/sync/cbor/2' -or
        -not [bool]$Compatibility.required_successor.identifier_allocated -or
        -not [bool]$Compatibility.required_successor.typed_event_key_integrated -or
        -not [bool]$Compatibility.required_successor.closed_event_carrier_integrated -or
        -not [bool]$Compatibility.required_successor.closed_sync_frames_integrated -or
        -not [bool]$Compatibility.required_successor.typed_session_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.ticket_capability_authenticated -or
        -not [bool]$Compatibility.required_successor.dormant_runtime_state_machine_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_typed_store_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_cross_format_membership_policy_integrated -or
        -not [bool]$Compatibility.required_successor.must_integrate_bridge_into_closed_frames -or
        -not [bool]$Compatibility.required_successor.must_authenticate_exact_identifier_in_connection_ticket -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-and-membership-policy-verified-not-active') {
        throw 'canonical sync legacy-event bridge or successor contract changed'
    }

    $bridgeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'authenticated-legacy-history-bridge'
    })
    $successorRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'history-compatible-canonical-sync-generation'
    })
    if ($bridgeRequirement.Count -ne 1 -or
        [string]$bridgeRequirement[0].status -cne 'verified' -or
        $successorRequirement.Count -ne 1 -or
        [string]$successorRequirement[0].status -cne 'verified' -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [bool]$Migration.candidate_identifier_registered) {
        throw 'canonical migration does not block on a history-compatible generation'
    }
    if ('typed-durable-canonical-sync-session-checkpoint' -cin
            @($TransportManifest.remaining) -or
        'authenticated-cbor-v2-ticket-capability' -cin
            @($TransportManifest.remaining) -or
        'legacy-import-checkpoint-and-capability-integration' -cnotin
            @($TransportManifest.remaining) -or
        [string]$TransportManifest.active.alpn -cne $legacyAlpn -or
        [bool]$TransportManifest.candidate.registered -or
        [bool]$TransportManifest.candidate.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled) {
        throw 'canonical transport did not preserve the historical compatibility blocker'
    }
    foreach ($candidateSchema in @($SyncSchema, $SessionSchema)) {
        if ('authenticated-legacy-history-bridge' -cnotin
                @($candidateSchema.activation.required_before_activation) -or
            'history-compatible-canonical-sync-generation' -cnotin
                @($candidateSchema.activation.required_before_activation)) {
            throw 'canonical sync schema omitted the historical-event activation blocker'
        }
    }
    if ([bool]$SyncSchema.historical_active_event_compatibility.bridge_integrated_into_this_closed_generation -or
        [bool]$SessionSchema.historical_active_event_compatibility.bridge_integrated_into_sync_frames) {
        throw 'legacy-event bridge was silently inserted into the closed canonical sync v1 generation'
    }

    $syncCoverage = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'sync' })
    $bridgeArtifacts = @(
        'CANONICAL-SYNC-LEGACY-EVENT.cddl',
        'CANONICAL-SYNC-LEGACY-EVENT-SCHEMA.json',
        'CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json',
        'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json',
        'crates/kilogram-protocol/src/canonical_sync_legacy_event_v1.rs',
        'scripts/verify-kilogram-canonical-sync-legacy-event-reference.mjs',
        'scripts/verify-kilogram-canonical-sync-legacy-event-boundary.ps1'
    )
    if ($syncCoverage.Count -ne 1 -or [string]$syncCoverage[0].coverage -cne 'complete') {
        throw 'canonical sync coverage surface is missing'
    }
    foreach ($artifact in $bridgeArtifacts) {
        if ($artifact -cnotin @($syncCoverage[0].artifacts)) {
            throw "canonical sync coverage is missing bridge artifact '$artifact'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'pub struct CanonicalSyncLegacyEventBridgeV1',
        'pub fn from_active_event(',
        'pub fn decode_and_verify(',
        'AuthorizedEvent::decode_and_verify_author',
        'self.active_authorized_event.encode()? != self.active_authorized_event_postcard',
        'ConversationMismatch',
        'EventIdMismatch',
        'AuthorAccountMismatch',
        'AuthorDeviceMismatch',
        'MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES',
        'MAX_ACTIVE_AUTHORIZED_EVENT_POSTCARD_BYTES'
    )) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync legacy-event Rust codec is missing '$required'"
        }
    }
    if ($Protocol.IndexOf('pub mod canonical_sync_legacy_event_v1;',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical sync legacy-event module is not exported'
    }
    foreach ($activeSource in @($ActiveWire, $Transport, $Cli)) {
        if ($activeSource.IndexOf('canonical_sync_legacy_event_v1',
                [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf('CanonicalSyncLegacyEventBridgeV1',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'inactive legacy-event bridge leaked into an active runtime path'
        }
    }
    foreach ($required in @('sync-legacy-event-bridge-v1',
        'active-authorized-event-postcard', 'kilogram-sync-legacy-event-v1')) {
        if ($Cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync legacy-event CDDL is missing '$required'"
        }
    }
    foreach ($required in @(
        'cannot honestly translate and re-sign existing history',
        'exact active Postcard `AuthorizedEvent` bytes',
        'does not mint a new signature',
        'does not silently alter it',
        'only registered and dialed synchronization ALPN',
        'changes no active wire bytes'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0178 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $schemaPath, $vectorsPath, $cddlPath, $compatibilityPath, $migrationPath,
    $transportManifestPath, $syncSchemaPath, $sessionSchemaPath, $coveragePath,
    $rustPath, $protocolPath, $activeWirePath, $transportPath, $cliPath,
    $referencePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync legacy-event bridge file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$syncSchema = Get-Content -LiteralPath $syncSchemaPath -Raw | ConvertFrom-Json
$sessionSchema = Get-Content -LiteralPath $sessionSchemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$activeWire = Get-Content -LiteralPath $activeWirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-BridgeContract $schema $vectors $compatibility $migration $transportManifest `
    $syncSchema $sessionSchema $coverage $rust $protocol $activeWire $transport `
    $cli $cddl $rfc

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_legacy_event_reference_self_test=passed' -notin
        @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync legacy-event independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $premature = Copy-JsonObject $compatibility
    $premature.existing_canonical_candidate.registered = $true
    Assert-Rejected {
        Assert-BridgeContract $schema $vectors $premature $migration $transportManifest `
            $syncSchema $sessionSchema $coverage $rust $protocol $activeWire $transport `
            $cli $cddl $rfc
    } 'premature_canonical_registration'

    $requiresKeys = Copy-JsonObject $schema
    $requiresKeys.source.original_author_private_keys_required = $true
    Assert-Rejected {
        Assert-BridgeContract $requiresKeys $vectors $compatibility $migration `
            $transportManifest $syncSchema $sessionSchema $coverage $rust $protocol `
            $activeWire $transport $cli $cddl $rfc
    } 'historical_resigning_required'

    $changedEvent = Copy-JsonObject $vectors
    $changedEvent.expected.event_id_hex = '00'
    Assert-Rejected {
        Assert-BridgeContract $schema $changedEvent $compatibility $migration `
            $transportManifest $syncSchema $sessionSchema $coverage $rust $protocol `
            $activeWire $transport $cli $cddl $rfc
    } 'invalid_event_id_shape'
    Write-Output 'canonical_sync_legacy_event_self_test=passed'
}

Write-Output 'canonical_sync_legacy_event_boundary=verified'
Write-Output 'exact_active_postcard_bytes=preserved-and-reverified'
Write-Output 'original_author_private_keys_required=false'
Write-Output 'canonical_sync_v1_history_compatible=false'
Write-Output 'history_compatible_successor_identifier=kilogram/m1/sync/cbor/2'
Write-Output 'active_sync_alpn=kilogram/m0/sync/8'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
