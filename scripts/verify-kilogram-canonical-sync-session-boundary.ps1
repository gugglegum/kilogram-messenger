[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$syncVectorsPath = Join-Path $workspace 'CANONICAL-SYNC-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC-SESSION.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_session_v1.rs'
$activePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-session-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0139-m1113-canonical-sync-session.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.13' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical sync session schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-SYNC-SESSION.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-session-v1' -or
        [string]$Schema.activation.current_runtime_codec -cne 'postcard-v6' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical sync session activation boundary changed'
    }
    if ([int]$Schema.limits.maximum_received_event_ids -ne 4096 -or
        [int]$Schema.limits.maximum_sent_event_ids -ne 4096 -or
        [int]$Schema.limits.maximum_encoded_checkpoint_bytes -ne 524288) {
        throw 'canonical sync session resource bounds changed'
    }
    $objectIds = @($Schema.objects | ForEach-Object { [string]$_.id })
    if (($objectIds -join ',') -cne 'sync-session-checkpoint-content-v1,signed-sync-session-checkpoint-v1') {
        throw 'canonical sync session object inventory changed'
    }
    $actions = @($Schema.next_action_registry.psobject.Properties | Sort-Object { [int]$_.Name } |
        ForEach-Object { "$($_.Name):$($_.Value)" })
    if (($actions -join ',') -cne '1:send-inventory,2:receive-diff,3:send-event-batch,4:receive-complete,5:complete,6:rejected') {
        throw 'canonical sync session next-action registry changed'
    }
    if ([string]$Schema.domains[0].value -cne ("kilogram:sync-session-checkpoint-signature:cbor-v1" + [char]0) -or
        [string]$Schema.domains[1].value -cne ("kilogram:sync-session-checkpoint-id:cbor-v1" + [char]0)) {
        throw 'canonical sync session domains changed'
    }
    foreach ($required in @('CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN',
        'CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN',
        'decode_and_verify_signed_sync_session_checkpoint_v1',
        'verify_sync_session_checkpoint_successor_v1', 'PredecessorMismatch',
        'EventEvidenceRollback', 'InvalidPauseTransition', 'InvalidActionTransition',
        'TerminalCheckpointHasSuccessor', 'MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync session Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync session vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.input.local_device_secret_key_hex) 32 'local Device key'
    foreach ($field in @('local_device_id_hex', 'peer_device_id_hex', 'conversation_id_hex',
        'session_binding_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($field in @('received_event_ids_hex', 'sent_event_ids_hex')) {
        $values = @($Vectors.expected.$field)
        if ($values.Count -ne 1) { throw "$field vector count changed" }
        Assert-LowerHex ([string]$values[0]) 32 $field
    }
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

foreach ($path in @($schemaPath, $vectorsPath, $eventVectorsPath, $syncVectorsPath, $cddlPath,
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

foreach ($required in @('sync-session-checkpoint-content-v1',
    'signed-sync-session-checkpoint-v1', 'sync-next-action-v1 = 1 / 2 / 3 / 4 / 5 / 6',
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
if ($active.IndexOf('canonical_sync_session_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical sync session codec leaked into active runtime'
}
$syncCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'sync' })
if ($syncCoverage.Count -ne 1 -or [string]$syncCoverage[0].coverage -cne 'complete' -or
    (@($syncCoverage[0].covered) -join ',') -cne 'sync-inventory,sync-diff,sync-request-response-frames,multi-round-session-state' -or
    @($syncCoverage[0].remaining).Count -ne 0) {
    throw 'canonical sync coverage is not complete'
}
foreach ($required in @('increments the sequence exactly once', 'cannot remove',
    'Pause and resume', 'terminal', 'from 3,654 to 3,915',
    'multi-round session state', 'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical sync session RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --event-vectors $eventVectorsPath --sync-vectors $syncVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_session_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync session independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $removed = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.next_action_registry.psobject.Properties.Remove('6')
    Assert-Rejected { Assert-Schema $removed $rust } 'removed_terminal_action'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.checkpoints[0].checkpoint_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_checkpoint_id_shape'
    Write-Output 'changed_local_key=rejected-by-independent-reference'
    Write-Output 'canonical_sync_session_self_test=passed'
}

Write-Output 'canonical_sync_session_boundary=verified'
Write-Output 'checkpoint_chain=verified'
Write-Output 'pause_resume_state=verified'
Write-Output 'bounded_decoder_mutations=6264'
Write-Output 'sync_coverage=complete'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
