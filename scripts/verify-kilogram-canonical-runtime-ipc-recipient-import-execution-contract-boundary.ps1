[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-RECIPIENT-IMPORT-EXECUTION-CONTRACT.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\Cargo.toml'
$libPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$contractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\recipient_import_execution_contract.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0257-m1233-dormant-cross-process-recipient-import-execution-contract.md'

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

function Assert-Contract {
    param(
        [object]$Manifest,
        [string]$CrateManifest,
        [string]$Lib,
        [string]$Contract,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.33' -or
        [string]$Manifest.status -cne 'canonical-runtime-ipc-recipient-import-execution-contract-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import IPC contract identity or scope changed'
    }

    $contractBoundary = $Manifest.contract
    if ([int]$contractBoundary.version -ne 1 -or
        [int]$contractBoundary.maximum_request_bytes -ne 8192 -or
        [int]$contractBoundary.maximum_signed_response_bytes -ne 16384 -or
        [int]$contractBoundary.maximum_membership_references -ne 8 -or
        [int]$contractBoundary.default_execution_steps -ne 1 -or
        [string]$contractBoundary.canonical_codec -cne 'postcard-reencode-exact' -or
        [int]$contractBoundary.active_runtime_ipc_version_retained -ne 33 -or
        [int]$contractBoundary.active_runtime_descriptor_domain_retained -ne 14 -or
        [bool]$contractBoundary.active_runtime_command_variant_added -or
        [bool]$contractBoundary.active_adapter_added -or
        [bool]$contractBoundary.cli_command_added -or
        [bool]$contractBoundary.desktop_control_added) {
        throw 'recipient import IPC codec or dormant boundary changed'
    }

    if ((@($Manifest.stages) -join ',') -cne 'plan,status,confirm,apply-next,apply-bounded') {
        throw 'recipient import IPC stage vocabulary changed'
    }
    foreach ($property in @(
        'plan_is_read_only', 'status_is_read_only',
        'explicit_confirmation_required', 'exact_confirmation_phrase_required',
        'mutation_nonce_required', 'confirmation_id_required_for_apply'
    )) {
        if (-not [bool]$Manifest.stage_boundary.$property) {
            throw "recipient import IPC stage property '$property' changed"
        }
    }
    foreach ($property in @(
        'implicit_apply_from_active_intent_allowed',
        'automatic_intent_consume_allowed', 'stage_skip_allowed'
    )) {
        if ([bool]$Manifest.stage_boundary.$property) {
            throw "recipient import IPC unsafe stage property '$property' changed"
        }
    }

    foreach ($property in @(
        'caller_ordered_authenticated_references_only',
        'strictly_increasing_unique_ordinals', 'exact_membership_set_digest',
        'reload_before_execution_required', 'caller_authorized_context_required'
    )) {
        if (-not [bool]$Manifest.membership_authority.$property) {
            throw "membership authority property '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot_transported', 'single_database_membership_authority',
        'automatic_source_discovery'
    )) {
        if ([bool]$Manifest.membership_authority.$property) {
            throw "membership authority escaped through '$property'"
        }
    }

    foreach ($property in @(
        'request_id', 'runtime_descriptor', 'current_descriptor_device',
        'intent_id', 'execution_plan_id', 'coordinator_plan_id_and_sequence',
        'ordered_membership_set_digest', 'confirmation_id_and_nonce',
        'mutation_nonce', 'bounded_step_count', 'domain_separated_device_signature'
    )) {
        if (-not [bool]$Manifest.exact_binding.$property) {
            throw "recipient import exact binding '$property' changed"
        }
    }

    foreach ($property in @('opaque_content_addressed_ids', 'closed_counts_and_states')) {
        if (-not [bool]$Manifest.privacy_projection.$property) {
            throw "recipient import privacy property '$property' changed"
        }
    }
    foreach ($property in @(
        'raw_account_id_exposed', 'raw_device_id_exposed',
        'root_or_device_secret_exposed', 'filesystem_path_exposed',
        'transport_ticket_or_endpoint_exposed', 'membership_snapshot_exposed',
        'message_body_or_plaintext_exposed', 'ciphertext_history_artifact_exposed'
    )) {
        if ([bool]$Manifest.privacy_projection.$property) {
            throw "recipient import IPC privacy projection leaked '$property'"
        }
    }

    foreach ($property in @(
        'canonical_reencode_required', 'response_exact_request_digest_bound',
        'reordered_memberships_rejected', 'duplicate_memberships_rejected',
        'substituted_memberships_rejected', 'stale_coordinator_rejected_by_future_adapter',
        'mutation_replay_requires_durable_deduplication_by_future_adapter',
        'partial_decode_rejected'
    )) {
        if (-not [bool]$Manifest.replay_boundary.$property) {
            throw "recipient import replay property '$property' changed"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.targeted_contract_tests -cne '8/8' -or
        [string]$verification.full_runtime_ipc_tests -cne '35/35' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '159/159' -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'recipient import IPC verification claim changed'
    }

    Assert-ContainsAll $CrateManifest @(
        'blake3.workspace = true', 'postcard.workspace = true', 'serde.workspace = true'
    ) 'runtime IPC crate manifest'
    Assert-ContainsAll $Lib @(
        'const IPC_VERSION: u8 = 33;',
        'kilogram:runtime-ipc-descriptor:v14',
        'mod recipient_import_execution_contract;',
        'pub use recipient_import_execution_contract::*;'
    ) 'runtime IPC library'
    Assert-ContainsAll $Contract @(
        'RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION: u8 = 1',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_REQUEST_BYTES: usize = 8 * 1024',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_RESPONSE_BYTES: usize = 16 * 1024',
        'MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES: usize = 8',
        'pub enum RuntimeIpcRecipientImportExecutionCommand',
        'Plan {', 'Status {', 'Confirm {', 'ApplyNext {', 'ApplyBounded {',
        'RuntimeIpcRecipientImportExplicitApproval::UserConfirmed',
        'confirmation phrase does not match the execution plan ID',
        'membership references are reordered or duplicated',
        'pub struct SignedRuntimeIpcRecipientImportExecutionResponse',
        'request_digest: request.request_digest()?',
        'runtime_descriptor_binding: descriptor_binding(descriptor)?',
        'current_device_binding: device_binding(descriptor.content.device_id.as_bytes())',
        'membership_reload_required: true',
        'explicit_execution_required: true',
        'intent_consumed: false',
        'verify runtime IPC recipient import execution response signature',
        'response skips or rewinds the requested stage',
        'all_five_request_stages_round_trip_canonically',
        'reordered_duplicated_or_oversized_membership_references_fail_closed',
        'implicit_or_mismatched_confirmation_fails_closed',
        'descriptor_substitution_rejects_before_adapter_dispatch',
        'signed_response_exact_binds_request_descriptor_and_device',
        'response_stage_skip_and_binding_substitution_fail_closed',
        'mutation_nonce_confirmation_and_step_budget_are_exact_bound',
        'confirmation_is_separate_and_cannot_consume_the_intent'
    ) 'recipient import IPC contract'

    foreach ($forbidden in @(
        'tokio::net', 'reqwest::', 'iroh::', 'std::net::Tcp',
        'RuntimeIpcCommand::RecipientImport', 'RuntimeIpcResponse::RecipientImport',
        'PathBuf', 'std::path',
        'remove_file(', 'remove_dir_all(', 'consume_active('
    )) {
        if ($Contract.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import IPC contract gained forbidden active surface '$forbidden'"
        }
    }
    if ([regex]::IsMatch($Contract, 'pub\s+[A-Za-z0-9_]+\s*:\s*(AccountId|DeviceId)')) {
        throw 'recipient import IPC public projection exposed a raw identity identifier'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'not a variant of the active `RuntimeIpcCommand`',
        'request vocabulary keeps five operations distinct',
        'An active intent alone is never execution authority',
        'future adapter must reload every referenced membership',
        'durably deduplicate that pair before invoking M1.2.32',
        'does not expose raw Account IDs, raw Device IDs',
        'Execution never consumes the M1.2.31 intent',
        'Aggregate accepted-tag readiness contains 159 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import IPC RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $crateManifestPath, $libPath, $contractPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$contract = Get-Content -LiteralPath $contractPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $crateManifest $lib $contract $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.contract.version = 2
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.stages = @('plan', 'status', 'apply-next')
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.stage_boundary.implicit_apply_from_active_intent_allowed = $true
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.membership_authority.reload_before_execution_required = $false
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.membership_authority.membership_snapshot_transported = $true
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_projection.raw_device_id_exposed = $true
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.exact_binding.mutation_nonce = $false
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.targeted_contract_tests = '7/8'
    $mutations += { Assert-Contract $changed $crateManifest $lib $contract $rfc }

    $mutations += {
        Assert-Contract $manifest $crateManifest $lib ($contract + "`nRuntimeIpcCommand::RecipientImport") $rfc
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            & $mutation
        }
        catch {
            $rejected++
        }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import IPC boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_runtime_ipc_recipient_import_execution_contract_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_runtime_ipc_recipient_import_execution_contract_boundary=verified'
Write-Output 'active_runtime_command_variant_added=false'
Write-Output 'active_adapter_added=false'
Write-Output 'membership_snapshot_transported=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
