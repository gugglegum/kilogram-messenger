[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-CONTRACT.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\Cargo.toml'
$libPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$contractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\recipient_import_consumption_contract.rs'
$executionContractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\recipient_import_execution_contract.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0261-m1237-explicit-recipient-import-consumption-ceremony-contract.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param(
        [object]$Manifest,
        [string]$CrateManifest,
        [string]$Lib,
        [string]$Contract,
        [string]$ExecutionContract,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.37' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-contract-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_runtime_ipc_version -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import consumption contract identity or scope changed'
    }

    if ((@($Manifest.ceremony_boundary.stages) -join ',') -cne 'plan,commit' -or
        [int]$Manifest.ceremony_boundary.maximum_plan_lifetime_seconds -ne 300) {
        throw 'recipient import consumption ceremony stages or lifetime changed'
    }
    foreach ($property in @(
        'plan_is_read_only', 'commit_is_explicit', 'explicit_approval_required',
        'exact_confirmation_phrase_required', 'fresh_consumption_nonce_required',
        'consumption_nonce_separate_from_mutation_nonce', 'content_addressed_plan',
        'caller_ordered_membership_references_required', 'descriptor_bound_request',
        'current_descriptor_device_signed_response'
    )) {
        if (-not [bool]$Manifest.ceremony_boundary.$property) {
            throw "recipient import consumption ceremony property '$property' changed"
        }
    }
    if ([bool]$Manifest.ceremony_boundary.automatic_consumption) {
        throw 'recipient import consumption became automatic'
    }

    foreach ($property in @(
        'complete_mutation_receipt_required', 'completed_source_count_must_equal_total',
        'completed_mutation_record_id_required', 'completed_mutation_receipt_digest_required',
        'membership_set_digest_bound',
        'intent_execution_confirmation_and_mutation_ids_bound',
        'final_coordinator_id_and_sequence_bound', 'imported_event_count_bound',
        'incomplete_receipt_rejected', 'changed_evidence_rejected'
    )) {
        if (-not [bool]$Manifest.completed_evidence_boundary.$property) {
            throw "recipient import completed evidence property '$property' changed"
        }
    }

    foreach ($property in @(
        'request_digest_bound', 'descriptor_and_current_device_bound',
        'plan_response_cannot_claim_consumed', 'commit_response_must_match_exact_plan',
        'consumed_at_must_be_within_plan_lifetime', 'active_intent_cleared_required',
        'already_consumed_is_exact_replay_only', 'stage_skip_or_rewind_rejected',
        'noncanonical_or_tampered_encoding_rejected'
    )) {
        if (-not [bool]$Manifest.response_boundary.$property) {
            throw "recipient import consumption response property '$property' changed"
        }
    }
    if (-not [bool]$Manifest.privacy_boundary.membership_references_are_opaque) {
        throw 'recipient import consumption membership references lost their opaque boundary'
    }

    foreach ($property in @(
        'membership_snapshot', 'message_body_or_plaintext',
        'ciphertext_history_artifact', 'filesystem_path', 'transport_authority',
        'raw_runtime_token'
    )) {
        if ([bool]$Manifest.privacy_boundary.$property) {
            throw "recipient import consumption privacy boundary leaked '$property'"
        }
    }

    if (-not [bool]$Manifest.activation_boundary.dormant_contract_only) {
        throw 'recipient import consumption contract is no longer dormant'
    }
    foreach ($property in @(
        'active_runtime_command_variant', 'runtime_server_dispatch',
        'consumption_adapter', 'durable_consumption_plan_ledger',
        'cli_or_desktop_control', 'background_worker_or_timer', 'network'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "recipient import consumption activation escaped through '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.focused_contract_tests -cne '8/8' -or
        [string]$verification.runtime_ipc_regression -cne '43/43' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '163/163' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'recipient import consumption verification claim changed'
    }

    Assert-ContainsAll $CrateManifest @(
        'blake3.workspace = true', 'postcard.workspace = true',
        'serde.workspace = true', 'kilogram-identity ='
    ) 'runtime IPC crate manifest'
    Assert-ContainsAll $Lib @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15',
        'mod recipient_import_consumption_contract;',
        'pub use recipient_import_consumption_contract::*;'
    ) 'runtime IPC library'
    Assert-ContainsAll $ExecutionContract @(
        'EXECUTION_RECEIPT_DIGEST_DOMAIN',
        'pub fn receipt_digest(&self)',
        'pub fn recipient_import_membership_set_digest(',
        'encode runtime IPC recipient import execution receipt digest'
    ) 'recipient import execution receipt digest'
    Assert-ContainsAll $Contract @(
        'RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION: u8 = 1',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_REQUEST_BYTES: usize = 8 * 1024',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_RESPONSE_BYTES: usize = 8 * 1024',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS: u64 = 300',
        'pub struct RuntimeIpcRecipientImportCompletedMutationEvidence',
        'from_completed_receipt(',
        'receipt.complete',
        'pub membership_set_digest: RuntimeIpcRecipientImportArtifactId',
        'recipient_import_membership_set_digest(self.memberships())?',
        'pub fn memberships(&self) -> &[RuntimeIpcRecipientImportMembershipReference]',
        'pub enum RuntimeIpcRecipientImportConsumptionCommand',
        'Plan {', 'Commit {',
        'RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed',
        'pub struct RuntimeIpcRecipientImportConsumptionPlanSummary',
        'pub fn expired(&self',
        'recipient_import_consumption_confirmation_phrase(',
        '"CONSUME {} {} {} {}"',
        'pub enum RuntimeIpcRecipientImportConsumptionOutcome',
        'Consumed(', 'AlreadyConsumed(',
        'pub struct SignedRuntimeIpcRecipientImportConsumptionResponse',
        'request_digest: request.request_digest()?',
        'runtime_descriptor_binding: descriptor_binding(descriptor)?',
        'current_device_binding: device_binding(descriptor.device_id().as_bytes())',
        'explicit_commit_required: true',
        'automatic_consumption: false',
        'verify runtime IPC recipient import consumption response signature',
        'outcome skips or rewinds its stage',
        'plan_and_commit_requests_round_trip_canonically',
        'evidence_requires_complete_exact_mutation_receipt',
        'plan_exact_binds_evidence_nonce_and_short_expiry',
        'implicit_or_mismatched_commit_fails_closed',
        'descriptor_substitution_fails_before_future_adapter',
        'signed_plan_response_exact_binds_request_descriptor_and_device',
        'commit_response_consumes_only_the_exact_explicit_plan',
        'noncanonical_oversized_and_tampered_encodings_fail_closed'
    ) 'recipient import consumption contract'

    foreach ($forbidden in @(
        'tokio::net', 'reqwest::', 'iroh::', 'std::net::Tcp', 'PathBuf',
        'std::path', 'remove_file(', 'remove_dir_all(', 'StateDirectoryLock',
        'RecipientRecoveryImportRoundIntentStore', '.consume('
    )) {
        if ($Contract.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import consumption contract gained forbidden active surface '$forbidden'"
        }
    }
    if ([regex]::IsMatch($Contract, 'pub\s+[A-Za-z0-9_]+\s*:\s*(AccountId|DeviceId)')) {
        throw 'recipient import consumption public projection exposed a raw identity identifier'
    }
    $commandEnum = [regex]::Match(
        $Lib,
        'pub enum RuntimeIpcCommand \{(?s:.*?)\r?\n\}'
    )
    if (-not $commandEnum.Success -or
        [regex]::Matches($commandEnum.Value, 'RecipientImportConsumption\s*\{').Count -ne 1 -or
        $commandEnum.Value.IndexOf('RecipientImportConsumption {', [StringComparison]::Ordinal) -lt 0 -or
        $commandEnum.Value.IndexOf('request: Vec<u8>', [StringComparison]::Ordinal) -lt 0) {
        throw 'explicit M1.2.44 recipient import consumption successor envelope changed'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'different authorities: finishing a bounded import does not authorize clearing the active intent',
        'not a variant of the active `RuntimeIpcCommand`',
        'evidence only from a complete M1.2.36 mutation receipt',
        'same ordered opaque membership references used by execution',
        'requested lifetime no longer than 300 seconds',
        '`CONSUME xxxx xxxx xxxx xxxx`',
        '`explicit_commit_required=true` and `automatic_consumption=false`',
        'No adapter, durable consumption-plan ledger, active runtime registration',
        'complete runtime IPC regression passes 43/43',
        'accepted-tag readiness contains 163 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import consumption RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $crateManifestPath, $libPath, $contractPath,
    $executionContractPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$contract = Get-Content -LiteralPath $contractPath -Raw
$executionContract = Get-Content -LiteralPath $executionContractPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $crateManifest $lib $contract $executionContract $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.milestone = 'M1.2.38'
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.ceremony_boundary.stages = @('commit')
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.ceremony_boundary.explicit_approval_required = $false
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.ceremony_boundary.maximum_plan_lifetime_seconds = 301
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.completed_evidence_boundary.complete_mutation_receipt_required = $false
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_boundary.message_body_or_plaintext = $true
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.activation_boundary.consumption_adapter = $true
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.focused_contract_tests = '7/8'
    $mutations += { Assert-Boundary $changed $crateManifest $lib $contract $executionContract $rfc }

    $mutations += {
        Assert-Boundary $manifest $crateManifest $lib ($contract + "`ntokio::net") $executionContract $rfc
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import consumption boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_consumption_contract_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_consumption_contract_boundary=verified'
Write-Output 'active_runtime_command_variant_added=false'
Write-Output 'consumption_adapter_added=false'
Write-Output 'automatic_consumption=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
