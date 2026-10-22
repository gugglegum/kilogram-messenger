[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-LAUNCH-SERVICE.json'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_service.rs'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0283-m1259-dormant-process-owned-guided-recovery-launch-service.md'

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

function Require-TrueProperties {
    param([object]$Object, [string[]]$Names, [string]$Label)
    foreach ($name in $Names) {
        if (-not [bool]$Object.$name) { throw "$Label invariant '$name' weakened" }
    }
}

function Require-FalseProperties {
    param([object]$Object, [string[]]$Names, [string]$Label)
    foreach ($name in $Names) {
        if ([bool]$Object.$name) { throw "$Label unsafe capability '$name' enabled" }
    }
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$Service, [string]$Slot,
        [string]$Controller, [string]$RuntimeLibrary, [string]$Execution,
        [string]$Cli, [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.59' -or
        [string]$Manifest.status -cne 'dormant-process-owned-guided-recovery-launch-service-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.58') {
        throw 'guided-recovery launch-service identity changed'
    }
    if ([string]$Manifest.service.type -cne 'RecipientImportGuidedRecoveryLaunchService') {
        throw 'guided-recovery launch-service type changed'
    }
    Require-TrueProperties $Manifest.service @(
        'process_owned', 'owns_exactly_one_launch_slot', 'slot_field_private',
        'constructor_crate_private', 'construction_dormant'
    ) 'launch service'
    Require-FalseProperties $Manifest.service @(
        'cloneable', 'serializable', 'debuggable', 'default_constructible',
        'public_constructor', 'raw_slot_accessor', 'slot_replacement',
        'arbitrary_second_slot', 'restart_recovers_service'
    ) 'launch service'
    Require-TrueProperties $Manifest.facade @(
        'accepts_opaque_assembly_by_value', 'routes_safe_state',
        'routes_safe_preview', 'routes_typed_approve',
        'routes_typed_pending_cancel', 'routes_typed_host_prepare',
        'routes_typed_host_confirm', 'routes_typed_host_apply',
        'routes_typed_host_cancel', 'routes_typed_host_refresh',
        'returns_existing_safe_projections', 'returns_typed_errors'
    ) 'launch-service facade'
    Require-FalseProperties $Manifest.facade @(
        'accepts_loose_authority_fragments', 'returns_slot', 'returns_host',
        'returns_issuer', 'returns_authorization',
        'returns_assembly_or_authority_fragments'
    ) 'launch-service facade'
    Require-TrueProperties $Manifest.custody @(
        'pending_replacement_rejected', 'installed_host_replacement_rejected',
        'invalid_staging_consumes_incoming_authority',
        'failed_approval_consumes_pending_authorization',
        'host_command_failure_retains_installed_host',
        'explicit_pending_cancel_consumes_authorization',
        'existing_custody_preserved_on_replacement_attempt'
    ) 'launch-service custody'
    Require-FalseProperties $Manifest.custody @(
        'failure_returns_authority', 'failure_silently_replaces_or_evicts_custody'
    ) 'launch-service custody'
    Require-FalseProperties $Manifest.production @(
        'ordinary_startup_creates_service', 'ordinary_startup_stages_assembly',
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire', 'changes_public_default'
    ) 'launch-service production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'launch-service public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 24 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '185/185') {
        throw 'launch-service verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'safe_lifecycle_verified', 'invalid_staging_rejected',
        'pending_replacement_rejected', 'installed_host_replacement_rejected',
        'failed_approval_consumed', 'host_retained_after_command_failure'
    ) 'launch-service verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'launch-service verification'

    Require-Text $Service @(
        "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>",
        "slot: RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "impl<'identity> RecipientImportGuidedRecoveryLaunchService<'identity>",
        'pub(crate) fn new_dormant() -> Self',
        'slot: RecipientImportGuidedRecoveryLaunchSlot::new_empty()',
        'pub(crate) fn stage_authenticated_assembly(',
        "assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        '.stage_from_authenticated_assembly(',
        'pub fn state(&self) -> RecipientImportGuidedRecoveryLaunchSlotState',
        'pub fn preview(', 'pub fn approve(', 'pub fn cancel(',
        'pub fn host_prepare(', 'pub fn host_confirm(', 'pub fn host_apply(',
        'pub fn host_cancel(', 'pub fn host_refresh(',
        'pub fn opened_network(&self) -> bool',
        'pub enum RecipientImportGuidedRecoveryLaunchServiceError',
        'Slot(Box<RecipientImportGuidedRecoveryLaunchSlotError>)'
    ) 'launch-service source'

    if ([regex]::IsMatch(
        $Service,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub struct RecipientImportGuidedRecoveryLaunchService<'identity>"
    )) { throw 'launch service gained a derive attribute' }
    foreach ($forbidden in @(
        'pub fn new_dormant()', 'pub fn stage_authenticated_assembly(',
        'pub slot:', 'pub fn slot(', 'pub fn slot_mut(', 'pub fn into_slot(',
        'pub fn replace_slot(', 'impl Default for RecipientImportGuidedRecoveryLaunchService',
        'use serde::', '#[derive(Serialize', '#[derive(Deserialize',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($Service.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "launch-service source contains forbidden '$forbidden'"
        }
    }
    $structStart = $Service.IndexOf(
        "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>",
        [StringComparison]::Ordinal
    )
    $implStart = $Service.IndexOf("impl<'identity>", $structStart, [StringComparison]::Ordinal)
    if ($structStart -lt 0 -or $implStart -lt 0) { throw 'launch-service struct is missing' }
    $serviceStruct = $Service.Substring($structStart, $implStart - $structStart)
    if (([regex]::Matches(
        $serviceStruct,
        "RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )).Count -ne 1) { throw 'launch service does not own exactly one slot field' }

    $stageStart = $Service.IndexOf(
        'pub(crate) fn stage_authenticated_assembly(',
        [StringComparison]::Ordinal
    )
    $stageBody = $Service.IndexOf('{', $stageStart, [StringComparison]::Ordinal)
    if ($stageStart -lt 0 -or $stageBody -lt 0) { throw 'launch-service staging method is missing' }
    $stageHeader = $Service.Substring($stageStart, $stageBody - $stageStart)
    foreach ($forbidden in @(
        'Path', 'AccountId', 'DeviceId', 'DeviceIdentity', 'RuntimeIpcDescriptor',
        'ExecutionRequest', 'CoordinationPlan', 'GuidedRecipientImportMembership',
        'AssemblyIssuer', 'LaunchAuthorization', 'RecoveryHost'
    )) {
        if ($stageHeader.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "launch-service staging signature exposed loose '$forbidden'"
        }
    }

    Require-Text $Slot @(
        'pub(crate) fn new_empty() -> Self',
        'pub(crate) fn stage_from_authenticated_assembly(',
        'fn installed_host_mut(', 'pub fn opened_network(&self) -> bool'
    ) 'launch-service slot predecessor'
    Require-Text $Controller @(
        "pub(crate) struct RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'pub(crate) fn into_issuer(self)'
    ) 'launch-service assembly predecessor'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_service;',
        'pub use recipient_import_guided_recovery_launch_service::{',
        'RecipientImportGuidedRecoveryLaunchService,',
        'RecipientImportGuidedRecoveryLaunchServiceError,',
        'pub(crate) use recipient_import_guided_session_controller::RecipientImportGuidedRecoveryAssemblyStagingInput;'
    ) 'launch-service runtime library'
    Require-Text $Execution @(
        'recipient_import_guided_recovery_launch_service_runs_one_safe_lifecycle',
        'recipient_import_guided_recovery_launch_service_preserves_and_consumes_custody',
        'RecipientImportGuidedRecoveryLaunchService::new_dormant()',
        'stage_authenticated_assembly(',
        'RecipientImportGuidedRecoveryLaunchServiceError::Slot(error)',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled'
    ) 'launch-service real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        foreach ($needle in @(
            'RecipientImportGuidedRecoveryLaunchService',
            'stage_authenticated_assembly',
            'RecipientImportGuidedRecoveryAssemblyStagingInput'
        )) {
            if ($surface.IndexOf($needle, [StringComparison]::Ordinal) -ge 0) {
                throw 'guided-recovery launch service became reachable from an active product surface'
            }
        }
    }
    Require-Text $Rfc @(
        'Dormant process-owned guided-recovery launch service',
        'owns exactly one', 'there is no accessor',
        'No failure returns authority', 'Dormant product boundary',
        'public sync default remains `legacy-only`'
    ) 'RFC-0283'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "mutation_$Label=rejected"; return }
    throw "mutation '$Label' was unexpectedly accepted"
}

foreach ($path in @(
    $manifestPath, $servicePath, $slotPath, $controllerPath,
    $runtimeLibraryPath, $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$service = Get-Content -LiteralPath $servicePath -Raw
$slot = Get-Content -LiteralPath $slotPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Boundary $manifest $service $slot $controller $runtimeLibrary $execution $cli $desktop $rfc

if ($SelfTest) {
    $rejected = 0
    $manifestMutations = @(
        @{ Section = 'service'; Property = 'owns_exactly_one_launch_slot'; Value = $false },
        @{ Section = 'service'; Property = 'public_constructor'; Value = $true },
        @{ Section = 'service'; Property = 'cloneable'; Value = $true },
        @{ Section = 'facade'; Property = 'accepts_opaque_assembly_by_value'; Value = $false },
        @{ Section = 'facade'; Property = 'accepts_loose_authority_fragments'; Value = $true },
        @{ Section = 'facade'; Property = 'returns_slot'; Value = $true },
        @{ Section = 'custody'; Property = 'failed_approval_consumes_pending_authorization'; Value = $false },
        @{ Section = 'custody'; Property = 'failure_returns_authority'; Value = $true },
        @{ Section = 'production'; Property = 'registered_runtime_ipc_command'; Value = $true },
        @{ Section = 'production'; Property = 'public_default'; Value = 'auto' }
    )
    foreach ($mutation in $manifestMutations) {
        $changed = Copy-JsonObject $manifest
        $changed.($mutation.Section).($mutation.Property) = $mutation.Value
        Assert-Rejected {
            Assert-Boundary $changed $service $slot $controller $runtimeLibrary $execution $cli $desktop $rfc
        } "$($mutation.Section)_$($mutation.Property)"
        $rejected++
    }

    $sourceMutations = @(
        @{ Label = 'public_constructor'; Target = 'service'; From = 'pub(crate) fn new_dormant()'; To = 'pub fn new_dormant()' },
        @{ Label = 'public_stage'; Target = 'service'; From = 'pub(crate) fn stage_authenticated_assembly('; To = 'pub fn stage_authenticated_assembly(' },
        @{ Label = 'public_slot_field'; Target = 'service'; From = '    slot: RecipientImportGuidedRecoveryLaunchSlot'; To = '    pub slot: RecipientImportGuidedRecoveryLaunchSlot' },
        @{ Label = 'slot_accessor'; Target = 'service_append'; From = ''; To = "`r`npub fn slot(&self) -> &RecipientImportGuidedRecoveryLaunchSlot<'identity> { &self.slot }`r`n" },
        @{ Label = 'cloneable_service'; Target = 'service'; From = "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>"; To = "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryLaunchService<'identity>" },
        @{ Label = 'loose_staging'; Target = 'service'; From = "assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>"; To = 'state_directory: PathBuf' },
        @{ Label = 'success_fixture_removed'; Target = 'execution'; From = 'recipient_import_guided_recovery_launch_service_runs_one_safe_lifecycle'; To = 'removed_launch_service_lifecycle_fixture' },
        @{ Label = 'cli_activation'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryLaunchService::new_dormant()`r`n" }
    )
    foreach ($mutation in $sourceMutations) {
        $changedService = $service
        $changedExecution = $execution
        $changedCli = $cli
        switch ($mutation.Target) {
            'service' { $changedService = $service.Replace($mutation.From, $mutation.To) }
            'service_append' { $changedService = $service + $mutation.To }
            'execution' { $changedExecution = $execution.Replace($mutation.From, $mutation.To) }
            'cli_append' { $changedCli = $cli + $mutation.To }
        }
        Assert-Rejected {
            Assert-Boundary $manifest $changedService $slot $controller $runtimeLibrary $changedExecution $changedCli $desktop $rfc
        } $mutation.Label
        $rejected++
    }
    if ($rejected -ne 18) { throw "launch-service boundary rejected $rejected/18 mutations" }
    Write-Output 'recipient_import_guided_recovery_launch_service_self_test=passed'
}

Write-Output 'recipient_import_guided_recovery_launch_service_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'service=one-process-owned-slot-no-raw-access'
Write-Output 'assembly_staging=opaque-by-value-crate-private'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
