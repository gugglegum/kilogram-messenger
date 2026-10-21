[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-RUNTIME-SERVICE.json'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_runtime_service.rs'
$cachePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_session_cache.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0269-m1245-process-owned-recipient-import-consumption-service.md'

function Copy-JsonObject { param([object]$Value) $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json }
function Require-Text { param([string]$Text,[string[]]$Needles,[string]$Label)
    foreach ($needle in $Needles) {
        if ($Text.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) { throw "$Label is missing '$needle'" }
    }
}
function Assert-Boundary {
    param([object]$Manifest,[string]$Service,[string]$Cache,[string]$Cli,[string]$Rfc)
    if ([int]$Manifest.schema_version -ne 1 -or [string]$Manifest.milestone -cne 'M1.2.45' -or
        [string]$Manifest.status -cne 'process-owned-service-core-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.44') { throw 'runtime service identity changed' }
    if ([int]$Manifest.service.dispatcher_count -ne 1 -or
        [int]$Manifest.service.plan_session_cache_count -ne 1 -or
        [int]$Manifest.service.maximum_live_plan_sessions -ne 32 -or
        [string]$Manifest.service.plan_session_persistence -cne 'process-local-only' -or
        -not [bool]$Manifest.service.commit_requires_same_process_plan -or
        -not [bool]$Manifest.service.successful_commit_retains_stage_until_expiry -or
        -not [bool]$Manifest.service.restart_discards_stage -or
        [bool]$Manifest.service.silent_capacity_eviction) { throw 'runtime service custody changed' }
    if (-not [bool]$Manifest.context.live_authenticated_recovery_context_required -or
        [bool]$Manifest.context.authority_reconstructed_from_request -or
        -not [bool]$Manifest.context.descriptor_and_current_device_revalidated -or
        -not [bool]$Manifest.context.membership_reloaded_by_caller_owned_loader) {
        throw 'runtime service live-context boundary changed'
    }
    if ([bool]$Manifest.active_runtime_handler.service_context_attached -or
        [bool]$Manifest.active_runtime_handler.signed_success_reachable -or
        [string]$Manifest.active_runtime_handler.unconfigured_result -cne
            'recipient import-consumption runtime service is not configured') {
        throw 'active handler was enabled without its guided context ceremony'
    }
    foreach ($property in @('opens_network','adds_cli_control','adds_desktop_control','changes_peer_wire','changes_runtime_ipc_wire','changes_public_default')) {
        if ([bool]$Manifest.scope.$property) { throw "scope member '$property' became active" }
    }
    if ([string]$Manifest.scope.public_default -cne 'legacy-only') { throw 'public default changed' }
    Require-Text $Service @(
        'pub struct RecipientImportConsumptionRuntimeService',
        'dispatcher: RecipientHistoryRecoveryImportConsumptionResponseDispatcher,',
        'sessions: RecipientImportConsumptionPlanSessionCache,',
        '.exact_plan_replay(&request, now_unix_seconds)?',
        '.dispatch_commit(',
        'RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(',
        'pub fn opened_network(&self) -> bool {',
        'false'
    ) 'runtime service source'
    Require-Text $Cache @('pub fn exact_plan_replay(', 'entry.plan_request_digest == request_digest') 'Plan cache source'
    Require-Text $Cli @(
        'let mut recipient_import_consumption = RecipientImportConsumptionRuntimeActor::new(',
        'RuntimeIpcCommand::RecipientImportConsumption { request } => {',
        'recipient_import_consumption.dispatch(',
        'runtime_recipient_import_consumption_context=unattached'
    ) 'active runtime successor handler'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @('exact repeated request returns the original signed response byte for byte',
        'Successful Commit retains the opaque Plan stage until its bounded expiry',
        'still returns the explicit unconfigured error', 'legacy-only')) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw "RFC is missing '$needle'" }
    }
}

foreach ($path in @($manifestPath,$servicePath,$cachePath,$cliPath,$rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$service = Get-Content -LiteralPath $servicePath -Raw
$cache = Get-Content -LiteralPath $cachePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $service $cache $cli $rfc

if ($SelfTest) {
    $mutations = @()
    $changed=Copy-JsonObject $manifest; $changed.service.plan_session_cache_count=2; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changed=Copy-JsonObject $manifest; $changed.service.plan_session_persistence='durable'; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changed=Copy-JsonObject $manifest; $changed.service.silent_capacity_eviction=$true; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changed=Copy-JsonObject $manifest; $changed.context.authority_reconstructed_from_request=$true; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changed=Copy-JsonObject $manifest; $changed.active_runtime_handler.signed_success_reachable=$true; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changed=Copy-JsonObject $manifest; $changed.scope.opens_network=$true; $mutations += { Assert-Boundary $changed $service $cache $cli $rfc }
    $changedService=$service.Replace('.exact_plan_replay(&request, now_unix_seconds)?','.dispatch_plan_again()?'); $mutations += { Assert-Boundary $manifest $changedService $cache $cli $rfc }
    $changedCli=$cli.Replace('recipient_import_consumption.dispatch(','recipient_import_consumption.accept_without_dispatch('); $mutations += { Assert-Boundary $manifest $service $cache $changedCli $rfc }
    $rejected=0
    foreach ($mutation in $mutations) { try { & $mutation; throw 'mutation was accepted' } catch { if ($_.Exception.Message -eq 'mutation was accepted') { throw }; $rejected++ } }
    if ($rejected -ne 8) { throw "expected 8 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=8/8'
}

Write-Output 'canonical_recipient_import_consumption_runtime_service_boundary=verified'
Write-Output 'active_runtime_handler_configured=context-gated'
Write-Output 'network_executed=false'
