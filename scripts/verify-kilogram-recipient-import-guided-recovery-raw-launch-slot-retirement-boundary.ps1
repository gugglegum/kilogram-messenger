[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-RAW-LAUNCH-SLOT-RETIREMENT.json'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_service.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$runtimeLibraryPath = Join-Path $runtimeSourceDirectory 'lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0284-m1260-raw-launch-slot-public-surface-retirement.md'

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
        [object]$Manifest, [string]$Slot, [string]$Service,
        [string]$RuntimeLibrary, [string]$Execution,
        [string]$OtherRuntimeSources, [string]$Cli, [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.60' -or
        [string]$Manifest.status -cne 'raw-launch-slot-public-surface-retired' -or
        [string]$Manifest.predecessor -cne 'M1.2.59') {
        throw 'raw launch-slot retirement identity changed'
    }
    if ([string]$Manifest.raw_slot.type -cne 'RecipientImportGuidedRecoveryLaunchSlot') {
        throw 'raw launch-slot retirement type changed'
    }
    Require-TrueProperties $Manifest.raw_slot @(
        'crate_private', 'service_only_production_owner'
    ) 'raw launch slot'
    Require-FalseProperties $Manifest.raw_slot @(
        'public_reexport', 'production_constructible', 'cloneable',
        'serializable', 'debuggable', 'default_constructible', 'raw_accessor',
        'replacement_seam', 'restart_recovery'
    ) 'raw launch slot'
    Require-TrueProperties $Manifest.safe_vocabulary @(
        'public_service', 'public_state', 'public_approve_command',
        'public_cancel_command', 'public_typed_error', 'safe_preview_preserved',
        'host_commands_preserved'
    ) 'raw launch-slot safe vocabulary'
    Require-FalseProperties $Manifest.safe_vocabulary @(
        'exposes_raw_slot', 'exposes_host', 'exposes_issuer',
        'exposes_authorization', 'changes_semantics',
        'changes_wire_representation'
    ) 'raw launch-slot safe vocabulary'
    Require-TrueProperties $Manifest.test_boundary @(
        'copied_module_only', 'raw_reexport_crate_private',
        'predecessor_fixtures_retained'
    ) 'raw launch-slot test boundary'
    Require-FalseProperties $Manifest.test_boundary @(
        'production_api_reopened'
    ) 'raw launch-slot test boundary'
    Require-FalseProperties $Manifest.production @(
        'ordinary_startup_creates_service', 'ordinary_startup_creates_slot',
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    ) 'raw launch-slot production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'raw launch-slot retirement public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 24 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '186/186') {
        throw 'raw launch-slot retirement verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'raw_slot_not_publicly_nameable',
        'service_only_production_owner_verified',
        'safe_facade_regression_verified'
    ) 'raw launch-slot verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'raw launch-slot verification'

    Require-Text $Slot @(
        "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        'pub(crate) fn new_empty() -> Self',
        'pub(crate) fn stage_from_authenticated_assembly(',
        'pub enum RecipientImportGuidedRecoveryLaunchSlotState',
        'pub struct RecipientImportGuidedRecoveryLaunchSlotApprove',
        'pub struct RecipientImportGuidedRecoveryLaunchSlotCancel',
        'pub enum RecipientImportGuidedRecoveryLaunchSlotError'
    ) 'raw launch-slot source'
    foreach ($forbidden in @(
        "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        'pub fn new_empty()', 'pub fn stage_from_authenticated_assembly(',
        'use serde::', '#[derive(Serialize', '#[derive(Deserialize',
        'impl Default for RecipientImportGuidedRecoveryLaunchSlot'
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "raw launch-slot source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Slot,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )) { throw 'raw launch slot gained a derive attribute' }

    Require-Text $Service @(
        "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>",
        "slot: RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        'slot: RecipientImportGuidedRecoveryLaunchSlot::new_empty()',
        'pub fn state(&self) -> RecipientImportGuidedRecoveryLaunchSlotState',
        'pub fn preview(', 'pub fn approve(', 'pub fn cancel(',
        'pub fn host_prepare(', 'pub fn host_confirm(', 'pub fn host_apply(',
        'pub fn host_cancel(', 'pub fn host_refresh('
    ) 'raw launch-slot service owner'
    foreach ($forbidden in @(
        'pub slot:', 'pub fn slot(', 'pub fn slot_mut(', 'pub fn into_slot(',
        'pub fn replace_slot('
    )) {
        if ($Service.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "raw launch-slot service contains forbidden '$forbidden'"
        }
    }
    if (([regex]::Matches(
        $Service,
        "slot: RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )).Count -ne 1 -or ([regex]::Matches(
        $Service,
        'RecipientImportGuidedRecoveryLaunchSlot::new_empty\(\)'
    )).Count -ne 1) {
        throw 'launch service is not the exact one-slot production owner'
    }

    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_slot;',
        'pub(crate) use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;',
        'pub use recipient_import_guided_recovery_launch_slot::{',
        'RecipientImportGuidedRecoveryLaunchSlotApprove,',
        'RecipientImportGuidedRecoveryLaunchSlotCancel,',
        'RecipientImportGuidedRecoveryLaunchSlotError,',
        'RecipientImportGuidedRecoveryLaunchSlotState,'
    ) 'raw launch-slot runtime exports'
    if ([regex]::IsMatch(
        $RuntimeLibrary,
        '(?ms)pub use\s+recipient_import_guided_recovery_launch_slot::\{[^}]*\bRecipientImportGuidedRecoveryLaunchSlot\b'
    ) -or [regex]::IsMatch(
        $RuntimeLibrary,
        '(?m)^pub use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;'
    )) { throw 'runtime crate publicly re-exported the raw launch slot' }

    Require-Text $Execution @(
        '#[path = "../../kilogram-peer-history-recovery-runtime/src/recipient_import_guided_recovery_launch_slot.rs"]',
        'pub(crate) use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;',
        'pub use recipient_import_guided_recovery_launch_slot::{',
        'recipient_import_guided_recovery_launch_slot_owns_one_approved_host',
        'recipient_import_guided_recovery_launch_slot_rejects_replacement_and_clears_failures',
        'recipient_import_guided_recovery_launch_service_runs_one_safe_lifecycle',
        'recipient_import_guided_recovery_launch_service_preserves_and_consumes_custody'
    ) 'raw launch-slot copied-module tests'
    if ([regex]::IsMatch(
        $Execution,
        '(?m)^pub use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;'
    ) -or [regex]::IsMatch(
        $Execution,
        '(?ms)pub use\s+recipient_import_guided_recovery_launch_slot::\{[^}]*\bRecipientImportGuidedRecoveryLaunchSlot\b'
    )) { throw 'copied-module test boundary publicly re-exported the raw launch slot' }

    if ($OtherRuntimeSources.IndexOf(
        'RecipientImportGuidedRecoveryLaunchSlot',
        [StringComparison]::Ordinal
    ) -ge 0) { throw 'a second runtime production source owns or names the raw launch slot' }
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf(
            'RecipientImportGuidedRecoveryLaunchSlot',
            [StringComparison]::Ordinal
        ) -ge 0) { throw 'raw launch slot became reachable from an active product surface' }
    }
    Require-Text $Rfc @(
        'Raw launch-slot public surface retirement',
        'crate-private implementation detail',
        'only production owner',
        'copied-module test boundary',
        'No runtime IPC, CLI,',
        'public sync default remains'
    ) 'RFC-0284'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "mutation_$Label=rejected"; return }
    throw "mutation '$Label' was unexpectedly accepted"
}

foreach ($path in @(
    $manifestPath, $slotPath, $servicePath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$slot = Get-Content -LiteralPath $slotPath -Raw
$service = Get-Content -LiteralPath $servicePath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$otherRuntimeSources = @(
    Get-ChildItem -LiteralPath $runtimeSourceDirectory -Filter '*.rs' -File |
        Where-Object {
            $_.FullName -notin @($slotPath, $servicePath, $runtimeLibraryPath)
        } |
        Sort-Object FullName |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"

Assert-Boundary $manifest $slot $service $runtimeLibrary $execution `
    $otherRuntimeSources $cli $desktop $rfc

if ($SelfTest) {
    $rejected = 0
    $manifestMutations = @(
        @{ Section = 'raw_slot'; Property = 'crate_private'; Value = $false },
        @{ Section = 'raw_slot'; Property = 'public_reexport'; Value = $true },
        @{ Section = 'raw_slot'; Property = 'service_only_production_owner'; Value = $false },
        @{ Section = 'safe_vocabulary'; Property = 'public_approve_command'; Value = $false },
        @{ Section = 'test_boundary'; Property = 'copied_module_only'; Value = $false },
        @{ Section = 'raw_slot'; Property = 'cloneable'; Value = $true },
        @{ Section = 'raw_slot'; Property = 'restart_recovery'; Value = $true },
        @{ Section = 'production'; Property = 'registered_runtime_ipc_command'; Value = $true },
        @{ Section = 'production'; Property = 'public_default'; Value = 'auto' },
        @{ Section = 'verification'; Property = 'network_executed'; Value = $true }
    )
    foreach ($mutation in $manifestMutations) {
        $changed = Copy-JsonObject $manifest
        $changed.($mutation.Section).($mutation.Property) = $mutation.Value
        Assert-Rejected {
            Assert-Boundary $changed $slot $service $runtimeLibrary $execution `
                $otherRuntimeSources $cli $desktop $rfc
        } "$($mutation.Section)_$($mutation.Property)"
        $rejected++
    }

    $sourceMutations = @(
        @{ Label = 'public_raw_struct'; Target = 'slot'; From = "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"; To = "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>" },
        @{ Label = 'public_runtime_reexport'; Target = 'runtime'; From = 'pub(crate) use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;'; To = 'pub use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;' },
        @{ Label = 'public_service_field'; Target = 'service'; From = '    slot: RecipientImportGuidedRecoveryLaunchSlot'; To = '    pub slot: RecipientImportGuidedRecoveryLaunchSlot' },
        @{ Label = 'raw_service_accessor'; Target = 'service_append'; From = ''; To = "`r`npub fn slot(&self) -> &RecipientImportGuidedRecoveryLaunchSlot<'identity> { &self.slot }`r`n" },
        @{ Label = 'second_runtime_owner'; Target = 'other_append'; From = ''; To = "`r`nstruct AlternateOwner<'identity> { slot: RecipientImportGuidedRecoveryLaunchSlot<'identity> }`r`n" },
        @{ Label = 'public_test_reexport'; Target = 'execution'; From = 'pub(crate) use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;'; To = 'pub use recipient_import_guided_recovery_launch_slot::RecipientImportGuidedRecoveryLaunchSlot;' },
        @{ Label = 'private_safe_state'; Target = 'slot'; From = 'pub enum RecipientImportGuidedRecoveryLaunchSlotState'; To = 'pub(crate) enum RecipientImportGuidedRecoveryLaunchSlotState' },
        @{ Label = 'cli_raw_slot'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryLaunchSlot::new_empty()`r`n" }
    )
    foreach ($mutation in $sourceMutations) {
        $changedSlot = $slot
        $changedService = $service
        $changedRuntimeLibrary = $runtimeLibrary
        $changedExecution = $execution
        $changedOtherRuntimeSources = $otherRuntimeSources
        $changedCli = $cli
        switch ($mutation.Target) {
            'slot' { $changedSlot = $slot.Replace($mutation.From, $mutation.To) }
            'service' { $changedService = $service.Replace($mutation.From, $mutation.To) }
            'service_append' { $changedService = $service + $mutation.To }
            'runtime' { $changedRuntimeLibrary = $runtimeLibrary.Replace($mutation.From, $mutation.To) }
            'execution' { $changedExecution = $execution.Replace($mutation.From, $mutation.To) }
            'other_append' { $changedOtherRuntimeSources = $otherRuntimeSources + $mutation.To }
            'cli_append' { $changedCli = $cli + $mutation.To }
        }
        Assert-Rejected {
            Assert-Boundary $manifest $changedSlot $changedService `
                $changedRuntimeLibrary $changedExecution $changedOtherRuntimeSources `
                $changedCli $desktop $rfc
        } $mutation.Label
        $rejected++
    }
    if ($rejected -ne 18) {
        throw "raw launch-slot retirement boundary rejected $rejected/18 mutations"
    }
    Write-Output 'recipient_import_guided_recovery_raw_launch_slot_retirement_self_test=passed'
}

Write-Output 'recipient_import_guided_recovery_raw_launch_slot_retirement_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'raw_slot_visibility=crate-private'
Write-Output 'production_owner=launch-service-only'
Write-Output 'safe_vocabulary=public'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
