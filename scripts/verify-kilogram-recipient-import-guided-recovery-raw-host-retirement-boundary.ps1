[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-RAW-HOST-RETIREMENT.json'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_service.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$runtimeLibraryPath = Join-Path $runtimeSourceDirectory 'lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0285-m1261-raw-installed-host-public-surface-retirement.md'

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

function Contains-RawHostName {
    param([string]$Text)
    [regex]::IsMatch($Text, '\bRecipientImportGuidedRecoveryHost\b')
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$HostSource, [string]$Slot, [string]$Service,
        [string]$RuntimeLibrary, [string]$Execution,
        [string]$OtherRuntimeSources, [string]$Cli, [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.61' -or
        [string]$Manifest.status -cne 'raw-installed-host-public-surface-retired' -or
        [string]$Manifest.predecessor -cne 'M1.2.60') {
        throw 'raw installed-host retirement identity changed'
    }
    if ([string]$Manifest.raw_host.type -cne 'RecipientImportGuidedRecoveryHost') {
        throw 'raw installed-host retirement type changed'
    }
    Require-TrueProperties $Manifest.raw_host @(
        'crate_private', 'slot_only_production_owner',
        'installation_method_crate_private'
    ) 'raw installed host'
    Require-FalseProperties $Manifest.raw_host @(
        'public_reexport', 'production_constructible', 'cloneable',
        'serializable', 'debuggable', 'default_constructible', 'raw_accessor',
        'extraction_seam', 'replacement_seam', 'restart_recovery'
    ) 'raw installed host'
    Require-TrueProperties $Manifest.safe_vocabulary @(
        'public_prepare_command', 'public_cancel_command', 'public_typed_error',
        'public_service', 'safe_local_ux_view',
        'service_host_routing_preserved',
        'transitional_installation_owner_public'
    ) 'raw installed-host safe vocabulary'
    Require-FalseProperties $Manifest.safe_vocabulary @(
        'exposes_raw_host', 'exposes_controller', 'exposes_handoff',
        'changes_semantics', 'changes_wire_representation'
    ) 'raw installed-host safe vocabulary'
    Require-TrueProperties $Manifest.test_boundary @(
        'copied_module_only', 'raw_reexport_crate_private',
        'predecessor_fixtures_retained'
    ) 'raw installed-host test boundary'
    Require-FalseProperties $Manifest.test_boundary @(
        'production_api_reopened'
    ) 'raw installed-host test boundary'
    Require-FalseProperties $Manifest.production @(
        'ordinary_startup_creates_service', 'ordinary_startup_creates_host',
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    ) 'raw installed-host production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'raw installed-host retirement public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 24 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '187/187') {
        throw 'raw installed-host retirement verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'raw_host_not_publicly_nameable',
        'slot_only_production_owner_verified',
        'safe_facade_regression_verified'
    ) 'raw installed-host verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'raw installed-host verification'

    Require-Text $HostSource @(
        "pub(crate) struct RecipientImportGuidedRecoveryHost<'identity>",
        'pub struct RecipientImportGuidedRecoveryHostPrepare',
        'pub struct RecipientImportGuidedRecoveryHostCancel',
        'pub enum RecipientImportGuidedRecoveryHostError',
        "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        'pub(crate) fn install(', 'self,',
        'RecipientImportGuidedRecoveryHost::new(',
        'pub fn prepare(', 'pub fn confirm(', 'pub fn apply(',
        'pub fn cancel(', 'pub fn refresh('
    ) 'raw installed-host source'
    foreach ($forbidden in @(
        "pub struct RecipientImportGuidedRecoveryHost<'identity>",
        'pub fn install(', 'use serde::',
        'impl Default for RecipientImportGuidedRecoveryHost'
    )) {
        if ($HostSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "raw installed-host source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $HostSource,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct RecipientImportGuidedRecoveryHost<'identity>"
    )) { throw 'raw installed host gained a derive attribute' }
    if ([regex]::IsMatch(
        $HostSource,
        "(?s)impl<'identity> RecipientImportGuidedRecoveryHost<'identity> \{\s+pub(?:\(crate\))? fn new\("
    )) { throw 'raw installed-host constructor became externally callable' }

    Require-Text $Slot @(
        "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>",
        'self.installed_host = Some(host);',
        'fn installed_host_mut(',
        "&mut RecipientImportGuidedRecoveryHost<'identity>",
        'self.installed_host', '.as_mut()',
        'pub fn host_prepare(', 'pub fn host_confirm(', 'pub fn host_apply(',
        'pub fn host_cancel(', 'pub fn host_refresh('
    ) 'raw installed-host slot owner'
    foreach ($forbidden in @(
        'pub installed_host:', 'pub fn installed_host(',
        'pub fn installed_host_mut(', 'pub fn host(', 'pub fn host_mut(',
        'pub fn into_host(', 'pub fn take_host(', 'pub fn replace_host('
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "raw installed-host slot contains forbidden '$forbidden'"
        }
    }
    if (([regex]::Matches(
        $Slot,
        "installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>"
    )).Count -ne 1) {
        throw 'launch slot is not the exact one-host production owner'
    }

    Require-Text $Service @(
        "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>",
        'pub fn host_prepare(', 'pub fn host_confirm(', 'pub fn host_apply(',
        'pub fn host_cancel(', 'pub fn host_refresh(',
        'RecipientImportGuidedSessionLocalUxView',
        'RecipientImportGuidedRecoveryLaunchServiceError'
    ) 'raw installed-host safe service'
    if (Contains-RawHostName $Service) {
        throw 'launch service named the raw installed host'
    }

    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_host;',
        'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;',
        'pub use recipient_import_guided_recovery_host::{',
        'RecipientImportGuidedRecoveryHostCancel,',
        'RecipientImportGuidedRecoveryHostError,',
        'RecipientImportGuidedRecoveryHostInstallationOwner,',
        'RecipientImportGuidedRecoveryHostPrepare,'
    ) 'raw installed-host runtime exports'
    if ([regex]::IsMatch(
        $RuntimeLibrary,
        '(?m)^pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'
    ) -or [regex]::IsMatch(
        $RuntimeLibrary,
        '(?ms)pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHost\b'
    )) { throw 'runtime crate publicly re-exported the raw installed host' }

    Require-Text $Execution @(
        '#[path = "../../kilogram-peer-history-recovery-runtime/src/recipient_import_guided_recovery_host.rs"]',
        'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;',
        'pub use recipient_import_guided_recovery_host::{',
        'recipient_import_guided_recovery_host_owns_one_full_local_ux_ceremony',
        'recipient_import_guided_recovery_host_installation_owner_accepts_only_full_assembly',
        'recipient_import_guided_recovery_launch_slot_owns_one_approved_host',
        'recipient_import_guided_recovery_launch_service_runs_one_safe_lifecycle'
    ) 'raw installed-host copied-module tests'
    if ([regex]::IsMatch(
        $Execution,
        '(?m)^pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'
    ) -or [regex]::IsMatch(
        $Execution,
        '(?ms)pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHost\b'
    )) { throw 'copied-module test boundary publicly re-exported the raw installed host' }

    if (Contains-RawHostName $OtherRuntimeSources) {
        throw 'a second runtime production source owns or names the raw installed host'
    }
    foreach ($surface in @($Cli, $Desktop)) {
        if (Contains-RawHostName $surface) {
            throw 'raw installed host became reachable from an active product surface'
        }
    }
    Require-Text $Rfc @(
        'Raw installed-host public surface retirement',
        'crate-private implementation detail',
        'only production module',
        'copied-module test boundary',
        'No runtime IPC, CLI,',
        'public sync default remains'
    ) 'RFC-0285'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "mutation_$Label=rejected"; return }
    throw "mutation '$Label' was unexpectedly accepted"
}

foreach ($path in @(
    $manifestPath, $hostPath, $slotPath, $servicePath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$hostSource = Get-Content -LiteralPath $hostPath -Raw
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
            $_.FullName -notin @($hostPath, $slotPath, $runtimeLibraryPath)
        } |
        Sort-Object FullName |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"

Assert-Boundary $manifest $hostSource $slot $service $runtimeLibrary $execution `
    $otherRuntimeSources $cli $desktop $rfc

if ($SelfTest) {
    $rejected = 0
    $manifestMutations = @(
        @{ Section = 'raw_host'; Property = 'crate_private'; Value = $false },
        @{ Section = 'raw_host'; Property = 'public_reexport'; Value = $true },
        @{ Section = 'raw_host'; Property = 'slot_only_production_owner'; Value = $false },
        @{ Section = 'safe_vocabulary'; Property = 'public_prepare_command'; Value = $false },
        @{ Section = 'test_boundary'; Property = 'copied_module_only'; Value = $false },
        @{ Section = 'raw_host'; Property = 'cloneable'; Value = $true },
        @{ Section = 'raw_host'; Property = 'restart_recovery'; Value = $true },
        @{ Section = 'production'; Property = 'registered_runtime_ipc_command'; Value = $true },
        @{ Section = 'production'; Property = 'public_default'; Value = 'auto' },
        @{ Section = 'verification'; Property = 'network_executed'; Value = $true }
    )
    foreach ($mutation in $manifestMutations) {
        $changed = Copy-JsonObject $manifest
        $changed.($mutation.Section).($mutation.Property) = $mutation.Value
        Assert-Rejected {
            Assert-Boundary $changed $hostSource $slot $service $runtimeLibrary `
                $execution $otherRuntimeSources $cli $desktop $rfc
        } "$($mutation.Section)_$($mutation.Property)"
        $rejected++
    }

    $sourceMutations = @(
        @{ Label = 'public_raw_struct'; Target = 'host'; From = "pub(crate) struct RecipientImportGuidedRecoveryHost<'identity>"; To = "pub struct RecipientImportGuidedRecoveryHost<'identity>" },
        @{ Label = 'public_runtime_reexport'; Target = 'runtime'; From = 'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'; To = 'pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;' },
        @{ Label = 'public_installation_method'; Target = 'host'; From = '    pub(crate) fn install('; To = '    pub fn install(' },
        @{ Label = 'public_slot_field'; Target = 'slot'; From = '    installed_host: Option<RecipientImportGuidedRecoveryHost'; To = '    pub installed_host: Option<RecipientImportGuidedRecoveryHost' },
        @{ Label = 'raw_slot_accessor'; Target = 'slot_append'; From = ''; To = "`r`npub fn installed_host(&self) -> &RecipientImportGuidedRecoveryHost<'identity> { todo!() }`r`n" },
        @{ Label = 'second_runtime_owner'; Target = 'other_append'; From = ''; To = "`r`nstruct AlternateHostOwner<'identity> { host: RecipientImportGuidedRecoveryHost<'identity> }`r`n" },
        @{ Label = 'public_test_reexport'; Target = 'execution'; From = 'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'; To = 'pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;' },
        @{ Label = 'cli_raw_host'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryHost::new`r`n" }
    )
    foreach ($mutation in $sourceMutations) {
        $changedHost = $hostSource
        $changedSlot = $slot
        $changedRuntimeLibrary = $runtimeLibrary
        $changedExecution = $execution
        $changedOtherRuntimeSources = $otherRuntimeSources
        $changedCli = $cli
        switch ($mutation.Target) {
            'host' { $changedHost = $hostSource.Replace($mutation.From, $mutation.To) }
            'slot' { $changedSlot = $slot.Replace($mutation.From, $mutation.To) }
            'slot_append' { $changedSlot = $slot + $mutation.To }
            'runtime' { $changedRuntimeLibrary = $runtimeLibrary.Replace($mutation.From, $mutation.To) }
            'execution' { $changedExecution = $execution.Replace($mutation.From, $mutation.To) }
            'other_append' { $changedOtherRuntimeSources = $otherRuntimeSources + $mutation.To }
            'cli_append' { $changedCli = $cli + $mutation.To }
        }
        Assert-Rejected {
            Assert-Boundary $manifest $changedHost $changedSlot $service `
                $changedRuntimeLibrary $changedExecution $changedOtherRuntimeSources `
                $changedCli $desktop $rfc
        } $mutation.Label
        $rejected++
    }
    if ($rejected -ne 18) {
        throw "raw installed-host retirement boundary rejected $rejected/18 mutations"
    }
    Write-Output 'recipient_import_guided_recovery_raw_host_retirement_self_test=passed'
}

Write-Output 'recipient_import_guided_recovery_raw_host_retirement_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'raw_host_visibility=crate-private'
Write-Output 'production_owner=launch-slot-only'
Write-Output 'safe_vocabulary=public'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
