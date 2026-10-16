[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-V2-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-V2-VECTORS.json'
$historyVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-V2-VECTORS.json'
$syncVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-V2-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-V2.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_session_v2.rs'
$activePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-session-v2-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0181-m1155-typed-canonical-sync-v2-session-checkpoint.md'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-LowerHex {
    param([string]$Value, [int]$Bytes = -1, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -cnotmatch '^[0-9a-f]+$' -or
        $Value.Length % 2 -ne 0 -or ($Bytes -ge 0 -and $Value.Length -ne $Bytes * 2)) {
        throw "$Label is not canonical lowercase hexadecimal"
    }
}

function Assert-Schema {
    param([object]$Schema, [string]$Rust)
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.55' -or
        [string]$Schema.status -cne 'typed-durable-checkpoint-verified-capability-pending' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical sync session schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-SYNC-SESSION-V2.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-session-v2' -or
        [string]$Schema.activation.current_runtime_codec -cne 'postcard-v6' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.allocated_protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [bool]$Schema.activation.registered_or_dialed -or
        [bool]$Schema.activation.advertised_by_authenticated_ticket -or
        'authenticated-sync-v2-ticket-capability' -cnotin
            @($Schema.activation.required_before_activation)) {
        throw 'canonical sync session activation boundary changed'
    }
    if ([string]$Schema.history_compatibility.event_key_schema -cne
            'CANONICAL-SYNC-HISTORY-V2-SCHEMA.json' -or
        [string]$Schema.history_compatibility.event_identity -cne 'namespace-and-event-id' -or
        [bool]$Schema.history_compatibility.bare_event_ids_allowed -or
        [bool]$Schema.history_compatibility.format_sniffing_or_fallback) {
        throw 'canonical sync session typed history boundary changed'
    }
    if ([int]$Schema.limits.maximum_received_event_keys -ne 4096 -or
        [int]$Schema.limits.maximum_sent_event_keys -ne 4096 -or
        [int]$Schema.limits.maximum_encoded_checkpoint_bytes -ne 524288) {
        throw 'canonical sync session resource bounds changed'
    }
    $objectIds = @($Schema.objects | ForEach-Object { [string]$_.id })
    if (($objectIds -join ',') -cne 'sync-session-checkpoint-content-v2,signed-sync-session-checkpoint-v2') {
        throw 'canonical sync session object inventory changed'
    }
    $actions = @($Schema.next_action_registry.psobject.Properties | Sort-Object { [int]$_.Name } |
        ForEach-Object { "$($_.Name):$($_.Value)" })
    if (($actions -join ',') -cne '1:send-inventory,2:receive-diff,3:send-event-batch,4:receive-complete,5:complete,6:rejected') {
        throw 'canonical sync session next-action registry changed'
    }
    if ([string]$Schema.domains[0].value -cne ("kilogram:sync-session-checkpoint-signature:cbor-v2" + [char]0) -or
        [string]$Schema.domains[1].value -cne ("kilogram:sync-session-checkpoint-id:cbor-v2" + [char]0)) {
        throw 'canonical sync session domains changed'
    }
    foreach ($required in @('CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2',
        'CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN_V2',
        'CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN_V2',
        'decode_and_verify_signed_sync_session_checkpoint_v2',
        'verify_sync_session_checkpoint_successor_v2', 'PredecessorMismatch',
        'EventEvidenceRollback', 'WrongProtocolIdentifier', 'NonCanonicalEventKeyOrder',
        'InvalidPauseTransition', 'InvalidActionTransition',
        'TerminalCheckpointHasSuccessor', 'MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync session Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or [string]$Vectors.milestone -cne 'M1.1.55' -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync session vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.input.local_device_secret_key_hex) 32 'local Device key'
    foreach ($field in @('local_device_id_hex', 'peer_device_id_hex', 'conversation_id_hex',
        'session_binding_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    if ([string]$Vectors.expected.protocol_identifier_utf8 -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync session protocol identifier changed'
    }
    Assert-LowerHex ([string]$Vectors.expected.protocol_identifier_hex) 23 'protocol identifier'
    $received = @($Vectors.expected.received_event_keys_cbor_hex)
    $sent = @($Vectors.expected.sent_event_keys_cbor_hex)
    if ($received.Count -ne 1 -or $sent.Count -ne 0) {
        throw 'typed Event-key vector counts changed'
    }
    Assert-LowerHex ([string]$received[0]) -1 'received typed Event key'
    $checkpoints = @($Vectors.expected.checkpoints)
    if ($checkpoints.Count -ne 4) { throw 'canonical sync session checkpoint vector count changed' }
    foreach ($checkpoint in $checkpoints) {
        foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex',
            'signed_cbor_hex', 'checkpoint_id_domain_hex')) {
            Assert-LowerHex ([string]$checkpoint.$field) -1 "checkpoint $field"
        }
        Assert-LowerHex ([string]$checkpoint.signature_hex) 64 'checkpoint signature'
        Assert-LowerHex ([string]$checkpoint.checkpoint_id_hex) 32 'checkpoint ID'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $historyVectorsPath, $syncVectorsPath, $cddlPath,
    $rustPath, $activePath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync session file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('sync-session-checkpoint-content-v2',
    'signed-sync-session-checkpoint-v2', 'sync-next-action-v2 = 1 / 2 / 3 / 4 / 5 / 6',
    'protocol-identifier-v2 = bytes .size 23', 'sync-event-key-v2 = bytes',
    'canonical-bool = 0 / 1', 'optional-checkpoint-id = [] / [id-32]')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical sync session CDDL is missing '$required'"
    }
}
foreach ($required in @('const SYNC_VERSION: u8 = 6;', 'postcard::to_allocvec(self)',
    'postcard::from_bytes(bytes)', 'kilogram:sync-session:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active Postcard sync session boundary changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_sync_session_v2', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical sync session codec leaked into active runtime'
}
$syncCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'sync' })
if ($syncCoverage.Count -ne 1 -or [string]$syncCoverage[0].coverage -cne 'complete' -or
    (@($syncCoverage[0].covered) -join ',') -cne 'sync-inventory,sync-diff,sync-request-response-frames,multi-round-session-state,typed-cross-generation-event-identity,closed-verified-legacy-and-canonical-event-carrier,history-compatible-typed-sync-v2-frames,typed-durable-sync-v2-checkpoint' -or
    @($syncCoverage[0].remaining).Count -ne 0) {
    throw 'canonical sync coverage is not complete'
}
foreach ($required in @('exact protocol identifier', 'typed Event key', 'cannot remove',
    'Pause and resume', 'terminal', 'namespace collision',
    'Postcard v6 remains the only active sync protocol')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical sync session RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --history-vectors $historyVectorsPath --sync-vectors $syncVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_session_v2_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync session independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $wrongProtocol = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $wrongProtocol.activation.allocated_protocol_identifier = 'kilogram/m1/sync/cbor/1'
    Assert-Rejected { Assert-Schema $wrongProtocol $rust } 'wrong_protocol_identifier'
    $removed = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.next_action_registry.psobject.Properties.Remove('6')
    Assert-Rejected { Assert-Schema $removed $rust } 'removed_terminal_action'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.checkpoints[0].checkpoint_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_checkpoint_id_shape'
    Write-Output 'changed_local_key=rejected-by-independent-reference'
    Write-Output 'canonical_sync_session_v2_self_test=passed'
}

Write-Output 'canonical_sync_session_v2_boundary=verified'
Write-Output 'checkpoint_chain=verified'
Write-Output 'pause_resume_state=verified'
Write-Output 'typed_event_key_namespaces=verified'
Write-Output 'protocol_identifier_binding=verified'
Write-Output 'sync_coverage=complete'
Write-Output 'allocated_protocol_identifier=kilogram/m1/sync/cbor/2'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
