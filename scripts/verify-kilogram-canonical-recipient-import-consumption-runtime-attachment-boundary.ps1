[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-RUNTIME-ATTACHMENT.json'
$attachmentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_runtime_attachment.rs'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_runtime_service.rs'
$cachePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_session_cache.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0270-m1246-guided-recipient-import-consumption-runtime-attachment.md'

function Copy-JsonObject {
    param([object]$Value)
    $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Require-Text {
    param([string]$Text, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Text.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$Attachment, [string]$Service,
        [string]$Cache, [string]$Execution, [string]$Cli,
        [string]$CliCargo, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.46' -or
        [string]$Manifest.status -cne 'runtime-owned-guided-context-slot-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.45') {
        throw 'runtime attachment identity changed'
    }
    if ([string]$Manifest.actor.owner -cne 'one-runtime-process' -or
        [int]$Manifest.actor.service_count -ne 1 -or
        [int]$Manifest.actor.context_slot_count -ne 1 -or
        [int]$Manifest.actor.maximum_live_plan_sessions -ne 32 -or
        -not [bool]$Manifest.actor.created_once_per_runtime_process) {
        throw 'runtime actor ownership changed'
    }
    $attachmentBoundary = $Manifest.attachment
    if ([string]$attachmentBoundary.source -cne 'guided-authenticated-recovery-only' -or
        -not [bool]$attachmentBoundary.process_local -or
        [bool]$attachmentBoundary.durable -or
        [int]$attachmentBoundary.maximum_lifetime_seconds -ne 300 -or
        -not [bool]$attachmentBoundary.bound_state_directory -or
        -not [bool]$attachmentBoundary.bound_account -or
        -not [bool]$attachmentBoundary.bound_device -or
        -not [bool]$attachmentBoundary.bound_coordinator_plan_and_sequence -or
        -not [bool]$attachmentBoundary.bound_membership_digests -or
        [bool]$attachmentBoundary.ipc_can_attach -or
        [bool]$attachmentBoundary.environment_can_attach -or
        [bool]$attachmentBoundary.filesystem_scan_can_attach -or
        [bool]$attachmentBoundary.implicit_live_replacement -or
        -not [bool]$attachmentBoundary.exact_install_replay_allowed -or
        -not [bool]$attachmentBoundary.clear_discards_plan_sessions -or
        -not [bool]$attachmentBoundary.expiry_discards_plan_sessions -or
        -not [bool]$attachmentBoundary.restart_discards_context_and_plan_sessions) {
        throw 'guided context custody changed'
    }
    if (-not [bool]$Manifest.handler.delegates_registered_v34_envelope -or
        [string]$Manifest.handler.unattached_result -cne
            'recipient import-consumption runtime service is not configured' -or
        [bool]$Manifest.handler.production_startup_attached -or
        -not [bool]$Manifest.handler.signed_success_requires_attached_context -or
        -not [bool]$Manifest.handler.descriptor_account_and_device_revalidated -or
        -not [bool]$Manifest.handler.current_signer_device_revalidated -or
        -not [bool]$Manifest.handler.state_changed_only_for_new_consumption) {
        throw 'active handler attachment boundary changed'
    }
    foreach ($property in @(
        'adds_cli_control', 'adds_desktop_control', 'adds_timer',
        'opens_product_network', 'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.scope.$property) {
            throw "scope member '$property' became active"
        }
    }
    if ([string]$Manifest.scope.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.stable_path_test_count -ne 1 -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'runtime attachment verification scope changed'
    }

    Require-Text $Attachment @(
        'pub struct GuidedRecipientImportConsumptionContext',
        'fn from_authenticated_recovery(',
        'coordination.state_directory() != state_directory',
        'coordination.owner_account_id() != owner_account_id',
        'coordination.owner_device_id() != owner_device_id',
        'MAX_RECIPIENT_IMPORT_CONSUMPTION_CONTEXT_LIFETIME_SECONDS',
        'recipient_import_membership_set_digest(&references)?;',
        'pub struct RecipientImportConsumptionRuntimeContextSlot',
        'ContextAlreadyAttached',
        'RecipientImportConsumptionRuntimeContextInstallOutcome::ExactReplay',
        'pub struct RecipientImportConsumptionRuntimeActor',
        'service: RecipientImportConsumptionRuntimeService,',
        'context_slot: RecipientImportConsumptionRuntimeContextSlot,',
        'self.service.clear_plan_sessions();',
        'self.context_slot.purge_expired(now_unix_seconds)',
        'descriptor.account_id() != self.owner_account_id',
        'descriptor.device_id() != self.owner_device_id',
        'signer.device_id() != self.owner_device_id',
        'recipient import-consumption runtime service is not configured',
        'pub fn opened_network(&self) -> bool {',
        'false'
    ) 'runtime attachment source'
    if ($Attachment.IndexOf('pub fn from_authenticated_recovery(', [StringComparison]::Ordinal) -ge 0) {
        throw 'raw guided-context constructor became publicly caller-assemblable'
    }
    Require-Text $Service @(
        'pub fn dispatch_with_metadata<',
        'pub fn clear_plan_sessions(&mut self) -> usize',
        'let state_changed = matches!(',
        'RuntimeIpcRecipientImportConsumptionOutcome::Consumed(_)',
        'state_changed,'
    ) 'runtime service source'
    Require-Text $Cache @('pub fn clear(&mut self) -> usize', 'self.sessions.clear();') 'Plan cache source'
    Require-Text $Execution @(
        'recipient_import_consumption_runtime_actor_requires_and_clears_guided_context',
        'RecipientImportConsumptionRuntimeAttachmentError::ContextUnavailable',
        'RecipientImportConsumptionRuntimeContextInstallOutcome::Installed',
        'RecipientImportConsumptionRuntimeContextInstallOutcome::ExactReplay',
        'RecipientImportConsumptionRuntimeAttachmentError::ContextAlreadyAttached',
        'RecipientImportConsumptionRuntimeServiceError::Session(',
        'RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage'
    ) 'stable regression source'
    Require-Text $Cli @(
        'let mut recipient_import_consumption = RecipientImportConsumptionRuntimeActor::new(',
        'RuntimeIpcCommand::RecipientImportConsumption { request } => {',
        'recipient_import_consumption.dispatch(',
        'RuntimeIpcResponse::RecipientImportConsumption {',
        'runtime_recipient_import_consumption_service=process-owned',
        'runtime_recipient_import_consumption_context=unattached',
        'runtime_recipient_import_consumption_context_source=guided-recovery-only'
    ) 'active runtime source'
    if ($Cli.IndexOf('.install_guided_context(', [StringComparison]::Ordinal) -ge 0) {
        throw 'ordinary CLI runtime gained an ambient guided-context installer'
    }
    Require-Text $CliCargo @(
        'kilogram-peer-history-recovery-runtime = { path = "../../crates/kilogram-peer-history-recovery-runtime" }'
    ) 'CLI Cargo manifest'

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'one `RecipientImportConsumptionRuntimeActor` for its whole process lifetime',
        'The runtime deliberately starts with the slot unattached',
        'cannot be supplied by the outer runtime IPC payload, an environment variable or a filesystem scan',
        'Explicit clear, expiry and process restart destroy both the context and every opaque Plan stage',
        'public default remains `legacy-only`',
        'M1.2.47 may connect a producer'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0270 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $attachmentPath, $servicePath, $cachePath, $executionPath,
    $cliPath, $cliCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$attachment = Get-Content -LiteralPath $attachmentPath -Raw
$service = Get-Content -LiteralPath $servicePath -Raw
$cache = Get-Content -LiteralPath $cachePath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $attachment, $service, $cache, $execution, $cli, $cliCargo, $rfc)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.actor.service_count = 2
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.attachment.durable = $true
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.attachment.maximum_lifetime_seconds = 301
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.attachment.ipc_can_attach = $true
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.attachment.implicit_live_replacement = $true
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handler.production_startup_attached = $true
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.scope.opens_product_network = $true
    $mutations += { Assert-Boundary $changed $attachment $service $cache $execution $cli $cliCargo $rfc }
    $changedAttachment = $attachment.Replace('ContextAlreadyAttached', 'ContextImplicitlyReplaced')
    $mutations += { Assert-Boundary $manifest $changedAttachment $service $cache $execution $cli $cliCargo $rfc }
    $changedCache = $cache.Replace('self.sessions.clear();', 'self.sessions.pop();')
    $mutations += { Assert-Boundary $manifest $attachment $service $changedCache $execution $cli $cliCargo $rfc }
    $changedCli = $cli.Replace(
        'runtime_recipient_import_consumption_context=unattached',
        'runtime_recipient_import_consumption_context=unattached.install_guided_context('
    )
    $mutations += { Assert-Boundary $manifest $attachment $service $cache $execution $changedCli $cliCargo $rfc }
    $changedRfc = $rfc.Replace('`legacy-only`', '`enabled`')
    $mutations += { Assert-Boundary $manifest $attachment $service $cache $execution $cli $cliCargo $changedRfc }
    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            & $mutation
            throw 'mutation was accepted'
        } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 11) { throw "expected 11 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=11/11'
}

Write-Output 'canonical_recipient_import_consumption_runtime_attachment_boundary=verified'
Write-Output 'runtime_actor=one-process-owned-service-one-context-slot'
Write-Output 'production_context=unattached'
Write-Output 'context_source=guided-authenticated-recovery-only'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
