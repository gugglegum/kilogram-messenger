[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-GUIDED-PRODUCER.json'
$attachmentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_runtime_attachment.rs'
$mutationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_mutation_adapter.rs'
$coordinatorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\coordinator.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0271-m1247-authenticated-guided-context-producer.md'

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
        [object]$Manifest, [string]$Attachment, [string]$Mutation,
        [string]$Coordinator, [string]$Execution, [string]$Cli, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.47' -or
        [string]$Manifest.status -cne 'authenticated-guided-context-producer-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.46') {
        throw 'guided producer identity changed'
    }
    $producer = $Manifest.producer
    if ([string]$producer.source -cne 'successful-complete-recipient-import-mutation-dispatch' -or
        -not [bool]$producer.process_local -or [bool]$producer.serializable -or
        [bool]$producer.public_constructor -or [bool]$producer.caller_assembled_context_allowed -or
        [bool]$producer.raw_context_constructor_public -or
        -not [bool]$producer.requires_complete_receipt -or
        -not [bool]$producer.requires_durable_completed_mutation -or
        -not [bool]$producer.requires_canonical_coordinator_refresh -or
        -not [bool]$producer.requires_exact_membership_reload -or
        [bool]$producer.rejected_dispatch_produces_handoff -or
        [bool]$producer.partial_dispatch_produces_handoff -or
        [bool]$producer.completed_replay_without_live_reload_produces_handoff) {
        throw 'guided producer custody changed'
    }
    foreach ($property in @(
        'exact_state_directory', 'exact_account', 'exact_device',
        'exact_final_coordinator_plan_and_sequence', 'exact_ordered_membership_set_digest',
        'exact_per_membership_digest', 'actor_rechecks_state_account_and_device_before_install'
    )) {
        if (-not [bool]$Manifest.binding.$property) {
            throw "guided producer binding '$property' weakened"
        }
    }
    if ([int]$Manifest.binding.maximum_context_lifetime_seconds -ne 300 -or
        -not [bool]$Manifest.loopback.canonical_nested_request_bytes -or
        -not [bool]$Manifest.loopback.signed_response_verified -or
        -not [bool]$Manifest.loopback.byte_exact_plan_replay -or
        -not [bool]$Manifest.loopback.explicit_commit -or
        [string]$Manifest.loopback.lost_response_replay_outcome -cne 'already-consumed' -or
        -not [bool]$Manifest.loopback.explicit_detach_clears_plan_stage -or
        [bool]$Manifest.loopback.post_detach_second_consume) {
        throw 'guided producer loopback proof changed'
    }
    foreach ($property in @(
        'startup_attached', 'ambient_installer', 'adds_cli_control', 'adds_desktop_control',
        'adds_environment_control', 'adds_filesystem_scan', 'opens_product_network',
        'changes_peer_wire', 'changes_runtime_ipc_wire', 'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "guided producer scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.stable_path_test_count -ne 2 -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'guided producer verification scope changed'
    }

    Require-Text $Attachment @(
        'pub struct AuthenticatedGuidedRecipientImportCompletion',
        'pub(crate) fn from_completed_mutation(',
        'pub struct GuidedRecipientImportConsumptionContextProducer',
        'pub fn produce(',
        'pub(crate) fn from_authenticated_recovery(',
        'pub fn install_authenticated_guided_recovery(',
        'completion.verify_runtime_binding(',
        'RecipientImportConsumptionRuntimeAttachmentError::StateDirectoryMismatch',
        'RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch',
        'RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch',
        'RecipientImportConsumptionRuntimeAttachmentError::CoordinatorChanged',
        'MembershipSetDigestMismatch'
    ) 'guided producer source'
    foreach ($forbidden in @(
        'pub fn from_completed_mutation(',
        'pub fn from_authenticated_recovery(',
        'impl Serialize for AuthenticatedGuidedRecipientImportCompletion',
        'impl Deserialize for AuthenticatedGuidedRecipientImportCompletion'
    )) {
        if ($Attachment.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "guided producer source contains forbidden '$forbidden'"
        }
    }
    Require-Text $Mutation @(
        'authenticated_guided_completion: Option<AuthenticatedGuidedRecipientImportCompletion>',
        'pub fn into_authenticated_guided_completion(',
        'if receipt.complete {',
        'RecipientHistoryRecoveryCoordinator::refresh_after_authenticated_import(',
        'AuthenticatedGuidedRecipientImportCompletion::from_completed_mutation(',
        'authenticated_guided_completion: None'
    ) 'guided mutation hand-off'
    Require-Text $Coordinator @(
        'pub fn refresh_after_authenticated_import(',
        '&previous.approved_sources'
    ) 'canonical coordinator refresh'
    Require-Text $Execution @(
        'recipient_import_consumption_authenticated_producer_drives_loopback_exactly_once',
        'successful guided mutation did not emit its in-process hand-off',
        'OwnerAccountMismatch', 'OwnerDeviceMismatch', 'StateDirectoryMismatch',
        'RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed(_)',
        'actor.clear_guided_context(&context_id)?'
    ) 'stable guided producer regression'
    if ($Cli.IndexOf('.install_authenticated_guided_recovery(', [StringComparison]::Ordinal) -ge 0) {
        throw 'ordinary runtime startup gained an ambient guided producer installer'
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'the only producer of an `AuthenticatedGuidedRecipientImportCompletion`',
        'no public constructor and no serialization implementation',
        'The raw context constructor is crate-private',
        'lost-response replay returns durable `AlreadyConsumed` without a second consume',
        'Ordinary runtime startup remains unattached',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0271 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $attachmentPath, $mutationPath, $coordinatorPath,
    $executionPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$attachment = Get-Content -LiteralPath $attachmentPath -Raw
$mutation = Get-Content -LiteralPath $mutationPath -Raw
$coordinator = Get-Content -LiteralPath $coordinatorPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $attachment, $mutation, $coordinator, $execution, $cli, $rfc)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.producer.serializable = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.producer.public_constructor = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.producer.partial_dispatch_produces_handoff = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.binding.exact_state_directory = $false
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.loopback.post_detach_second_consume = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.startup_attached = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.opens_product_network = $true
    $mutations += { Assert-Boundary $changed $attachment $mutation $coordinator $execution $cli $rfc }
    $changedAttachment = $attachment.Replace('pub(crate) fn from_completed_mutation(', 'pub fn from_completed_mutation(')
    $mutations += { Assert-Boundary $manifest $changedAttachment $mutation $coordinator $execution $cli $rfc }
    $changedMutation = $mutation.Replace('if receipt.complete {', 'if true {')
    $mutations += { Assert-Boundary $manifest $attachment $changedMutation $coordinator $execution $cli $rfc }
    $changedCoordinator = $coordinator.Replace('&previous.approved_sources', '&[]')
    $mutations += { Assert-Boundary $manifest $attachment $mutation $changedCoordinator $execution $cli $rfc }
    $changedCli = $cli.Replace('runtime_recipient_import_consumption_context=unattached', 'runtime_recipient_import_consumption_context=unattached.install_authenticated_guided_recovery(')
    $mutations += { Assert-Boundary $manifest $attachment $mutation $coordinator $execution $changedCli $rfc }
    $changedRfc = $rfc.Replace('`legacy-only`', '`enabled`')
    $mutations += { Assert-Boundary $manifest $attachment $mutation $coordinator $execution $cli $changedRfc }
    $rejected = 0
    foreach ($case in $mutations) {
        try {
            & $case
            throw 'mutation was accepted'
        } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 12) { throw "expected 12 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=12/12'
}

Write-Output 'canonical_recipient_import_consumption_guided_producer_boundary=verified'
Write-Output 'handoff_source=fresh-successful-complete-guided-mutation'
Write-Output 'handoff_transport=process-local-non-serializable'
Write-Output 'production_context=unattached'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
