[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-RESPONSE-DISPATCHER.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_response_dispatcher.rs'
$libPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$cratePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0265-m1241-recipient-import-consumption-response-dispatcher.md'

function Copy-JsonObject { param([object]$Value) $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json }
function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param([object]$Manifest, [string]$Source, [string]$Lib, [string]$Crate,
          [string]$Cli, [string]$Desktop, [string]$Rfc)
    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.41' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-response-dispatcher-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'consumption response dispatcher identity or public scope changed'
    }
    foreach ($property in @(
        'dormant_library_api', 'canonical_request_revalidated_before_dispatch',
        'descriptor_account_and_device_bound', 'current_device_signer_required',
        'plan_stage_read_only', 'plan_stage_creates_no_durable_state',
        'commit_requires_opaque_exact_plan_stage', 'stage_skipping_rejected',
        'explicit_commit_required', 'durable_authorization_before_mutation',
        'durable_mutation_receipt_before_signed_success',
        'signed_response_exact_request_digest_bound',
        'signed_response_exact_descriptor_bound', 'signed_response_current_device_bound',
        'exact_replay_without_second_consume_or_reload'
    )) {
        if (-not [bool]$Manifest.dispatcher_boundary.$property) {
            throw "consumption response dispatcher property '$property' changed"
        }
    }
    if ([bool]$Manifest.dispatcher_boundary.unsigned_outcome_exposed_by_dispatcher) {
        throw 'consumption response dispatcher exposes an unsigned outcome'
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_socket_handler', 'cli_or_desktop_control',
        'background_worker_or_timer', 'network', 'automatic_consumption',
        'history_or_membership_deletion'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "consumption response dispatcher gained forbidden authority '$property'"
        }
    }
    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.focused_signed_fixture_tests -cne '2/2' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '167/167' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'consumption response dispatcher verification claim changed'
    }

    Assert-ContainsAll $Source @(
        'pub struct RecipientHistoryRecoveryImportConsumptionResponseDispatcher',
        'RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate',
        'RuntimeIpcRecipientImportConsumptionCommand::Plan',
        'RuntimeIpcRecipientImportConsumptionCommand::Commit',
        'RecipientHistoryRecoveryImportConsumptionReadAdapter::dispatch',
        'RecipientImportConsumptionAuthorizationLedger::new',
        'authorizations.commit(',
        'RecipientHistoryRecoveryImportConsumptionMutationAdapter::new',
        'SignedRuntimeIpcRecipientImportConsumptionResponse::issue',
        'SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify',
        'authorization_committed_before_mutation: true',
        'mutation_receipt_persisted_before_response',
        'registered_active_runtime_command(&self) -> bool'
    ) 'consumption response dispatcher source'
    Assert-ContainsAll $Lib @(
        'mod recipient_import_consumption_response_dispatcher;',
        'RecipientHistoryRecoveryImportConsumptionResponseDispatcher',
        'SignedRuntimeIpcRecipientImportConsumptionResponse'
    ) 'runtime integration library'
    Assert-ContainsAll $Crate @('name = "kilogram-peer-history-recovery-runtime"') 'runtime crate'
    foreach ($dependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($Crate.IndexOf($dependency, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption response dispatcher crate gained '$dependency'"
        }
    }
    foreach ($app in @($Cli, $Desktop)) {
        if ($app.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
            throw 'consumption response dispatcher was wired into an active application'
        }
    }
    foreach ($forbidden in @('tokio::', 'iroh::', 'reqwest::', 'TcpListener', 'TcpStream',
                              'RuntimeIpcCommand::RecipientImport')) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "consumption response dispatcher gained forbidden surface '$forbidden'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'returns only an existing M1.2.37 Device-signed response',
        'Planning creates no authorization or mutation database',
        'requires the opaque result of an earlier successful dispatcher Plan stage',
        'durably commits the exact explicit M1.2.39 authorization',
        'only after the mutation adapter has durably stored its completed consumption receipt',
        'does not call the membership loader or consume the intent a second time',
        'no active runtime command',
        'contains 167 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "consumption response dispatcher RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $sourcePath, $libPath, $cratePath, $cliPath, $desktopPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$crate = Get-Content -LiteralPath $cratePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $source $lib $crate $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.milestone = 'M1.2.42'
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.changes_public_default = $true
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.dispatcher_boundary.plan_stage_creates_no_durable_state = $false
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.dispatcher_boundary.commit_requires_opaque_exact_plan_stage = $false
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.dispatcher_boundary.durable_authorization_before_mutation = $false
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.dispatcher_boundary.durable_mutation_receipt_before_signed_success = $false
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.dispatcher_boundary.unsigned_outcome_exposed_by_dispatcher = $true
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.automatic_consumption = $true
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.network = $true
    $mutations += { Assert-Boundary $changed $source $lib $crate $cli $desktop $rfc }
    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'mutation was accepted' } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=10/10'
}

Write-Output 'canonical_recipient_import_consumption_response_dispatcher_boundary=verified'
Write-Output 'changes_active_runtime_ipc=false'
Write-Output 'plan_state=ephemeral'
Write-Output 'commit_response=signed-after-durable-receipt'
Write-Output 'network_executed=false'
