[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-INSTALLATION-OWNER-RETIREMENT.json'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_authorization.rs'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$servicePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_service.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$runtimeLibraryPath = Join-Path $runtimeSourceDirectory 'lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0286-m1262-host-installation-owner-public-surface-retirement.md'

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

function Contains-OwnerName {
    param([string]$Text)
    $Text.IndexOf('RecipientImportGuidedRecoveryHostInstallationOwner', [StringComparison]::Ordinal) -ge 0
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$HostSource, [string]$Controller,
        [string]$Authorization, [string]$Slot, [string]$Service,
        [string]$RuntimeLibrary, [string]$Execution,
        [string]$OtherRuntimeSources, [string]$Cli, [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.62' -or
        [string]$Manifest.status -cne 'host-installation-owner-public-surface-retired' -or
        [string]$Manifest.predecessor -cne 'M1.2.61') {
        throw 'installation-owner retirement identity changed'
    }
    if ([string]$Manifest.installation_owner.type -cne 'RecipientImportGuidedRecoveryHostInstallationOwner') {
        throw 'installation-owner retirement type changed'
    }
    Require-TrueProperties $Manifest.installation_owner @(
        'crate_private', 'constructed_only_from_authenticated_recovery',
        'consumed_only_by_launch_slot_path', 'install_consumes_self'
    ) 'installation owner'
    Require-FalseProperties $Manifest.installation_owner @(
        'public_reexport', 'publicly_nameable', 'cloneable', 'serializable',
        'debuggable', 'default_constructible', 'raw_accessor',
        'extraction_seam', 'replacement_seam', 'restart_recovery'
    ) 'installation owner'
    Require-TrueProperties $Manifest.returning_transitions @(
        'issuer_issue_crate_private', 'authorization_approve_crate_private',
        'owner_install_crate_private', 'all_consume_self'
    ) 'owner-returning transitions'
    Require-FalseProperties $Manifest.returning_transitions @(
        'public_transition_returns_owner', 'failure_returns_reusable_authority'
    ) 'owner-returning transitions'
    Require-TrueProperties $Manifest.safe_vocabulary @(
        'public_launch_service', 'public_safe_preview', 'public_safe_state',
        'public_typed_launch_commands', 'public_typed_host_commands',
        'public_typed_errors', 'public_safe_local_ux_view',
        'service_routing_preserved'
    ) 'installation-owner safe vocabulary'
    Require-FalseProperties $Manifest.safe_vocabulary @(
        'exposes_installation_owner', 'exposes_raw_host', 'exposes_controller',
        'changes_semantics', 'changes_wire_representation'
    ) 'installation-owner safe vocabulary'
    Require-TrueProperties $Manifest.test_boundary @(
        'copied_module_only', 'owner_reexport_crate_private',
        'predecessor_fixtures_retained'
    ) 'installation-owner test boundary'
    Require-FalseProperties $Manifest.test_boundary @('production_api_reopened') 'installation-owner test boundary'
    Require-FalseProperties $Manifest.production @(
        'ordinary_startup_creates_service', 'ordinary_startup_creates_owner',
        'ordinary_startup_creates_host', 'registered_runtime_ipc_command',
        'registered_cli_command', 'registered_desktop_control',
        'registered_timer', 'registered_background_start',
        'adds_filesystem_discovery', 'changes_peer_wire',
        'changes_runtime_ipc_wire', 'changes_public_default'
    ) 'installation-owner production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'installation-owner retirement public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 24 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '188/188') {
        throw 'installation-owner retirement verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'owner_not_publicly_nameable', 'returning_transitions_not_public',
        'safe_facade_regression_verified'
    ) 'installation-owner verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'installation-owner verification'

    Require-Text $HostSource @(
        "pub(crate) struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        "installation: AuthenticatedGuidedRecipientImportHostInstallation<'identity>",
        'pub(crate) fn from_authenticated_recovery(',
        'pub(crate) fn install(', 'self,',
        'self.installation.into_parts()', 'Ok(host)'
    ) 'installation-owner source'
    foreach ($forbidden in @(
        "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        'pub fn install(', 'use serde::',
        'impl Default for RecipientImportGuidedRecoveryHostInstallationOwner'
    )) {
        if ($HostSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "installation-owner source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $HostSource,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct RecipientImportGuidedRecoveryHostInstallationOwner"
    )) { throw 'installation owner gained a derive attribute' }
    if (-not [regex]::IsMatch(
        $HostSource,
        '(?s)impl<''identity> RecipientImportGuidedRecoveryHostInstallationOwner<''identity> \{.*?pub\(crate\) fn install\(\s*self,'
    )) { throw 'installation-owner install no longer consumes self' }

    Require-Text $Controller @(
        "pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        'pub(crate) fn issue(', 'self,',
        "RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        'RecipientImportGuidedRecoveryHostInstallationOwner::from_authenticated_recovery('
    ) 'installation-owner issuer transition'
    if ($Controller.IndexOf('pub fn issue(', [StringComparison]::Ordinal) -ge 0) {
        throw 'assembly issuer publicly returns the installation owner'
    }
    if (-not [regex]::IsMatch(
        $Controller,
        '(?s)pub struct RecipientImportGuidedRecoveryAssemblyIssuer<''identity>.*?pub\(crate\) fn issue\(\s*self,'
    )) { throw 'assembly issuer issue no longer consumes self' }

    Require-Text $Authorization @(
        "pub struct RecipientImportGuidedRecoveryLaunchAuthorization<'identity>",
        'pub fn preview(&self)', 'pub(crate) fn approve(', 'self,',
        "RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        'issuer.issue().map_err(Into::into)', 'pub fn cancel(self) {}'
    ) 'installation-owner authorization transition'
    if ($Authorization.IndexOf('pub fn approve(', [StringComparison]::Ordinal) -ge 0) {
        throw 'launch authorization publicly returns the installation owner'
    }
    if (-not [regex]::IsMatch(
        $Authorization,
        '(?s)pub struct RecipientImportGuidedRecoveryLaunchAuthorization<''identity>.*?pub\(crate\) fn approve\(\s*self,'
    )) { throw 'launch authorization approval no longer consumes self' }

    Require-Text $Slot @(
        "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        'let owner = authorization.approve(command.approval, now_unix_seconds)?;',
        'let host = owner.install(now_unix_seconds, command.host_handoff_lifetime_seconds)?;',
        'self.installed_host = Some(host);',
        'Ok(RecipientImportGuidedRecoveryLaunchSlotState::HostInstalled)'
    ) 'installation-owner slot path'
    if (Contains-OwnerName $Slot) {
        throw 'launch slot named or exposed the inferred installation-owner type'
    }

    Require-Text $Service @(
        "pub struct RecipientImportGuidedRecoveryLaunchService<'identity>",
        'pub fn state(&self)', 'pub fn preview(', 'pub fn approve(',
        'pub fn cancel(', 'pub fn host_prepare(', 'pub fn host_confirm(',
        'pub fn host_apply(', 'pub fn host_cancel(', 'pub fn host_refresh('
    ) 'installation-owner safe service'
    if (Contains-OwnerName $Service) {
        throw 'launch service exposed the installation owner'
    }

    Require-Text $RuntimeLibrary @(
        'pub(crate) use recipient_import_guided_recovery_host::{',
        'RecipientImportGuidedRecoveryHost, RecipientImportGuidedRecoveryHostInstallationOwner,',
        'pub use recipient_import_guided_recovery_host::{',
        'RecipientImportGuidedRecoveryHostCancel, RecipientImportGuidedRecoveryHostError,',
        'RecipientImportGuidedRecoveryHostPrepare,'
    ) 'installation-owner runtime exports'
    if ([regex]::IsMatch(
        $RuntimeLibrary,
        '(?ms)^pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHostInstallationOwner\b'
    ) -or [regex]::IsMatch(
        $RuntimeLibrary,
        '(?m)^pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHostInstallationOwner;'
    )) { throw 'runtime crate publicly re-exported the installation owner' }

    Require-Text $Execution @(
        '#[path = "../../kilogram-peer-history-recovery-runtime/src/recipient_import_guided_recovery_host.rs"]',
        'pub(crate) use recipient_import_guided_recovery_host::{',
        'RecipientImportGuidedRecoveryHost, RecipientImportGuidedRecoveryHostInstallationOwner,',
        'recipient_import_guided_recovery_host_installation_owner_accepts_only_full_assembly',
        'recipient_import_guided_recovery_host_installation_owner_is_one_shot_and_fail_closed',
        'recipient_import_guided_recovery_launch_authorization_requires_explicit_exact_approval',
        'recipient_import_guided_recovery_launch_slot_owns_one_approved_host',
        'recipient_import_guided_recovery_launch_service_runs_one_safe_lifecycle'
    ) 'installation-owner copied-module tests'
    if ([regex]::IsMatch(
        $Execution,
        '(?ms)^pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHostInstallationOwner\b'
    )) { throw 'copied-module test boundary publicly re-exported the installation owner' }

    if (Contains-OwnerName $OtherRuntimeSources) {
        throw 'a second runtime source owns or names the installation owner'
    }
    foreach ($surface in @($Cli, $Desktop)) {
        if (Contains-OwnerName $surface) {
            throw 'installation owner became reachable from an active product surface'
        }
    }
    Require-Text $Rfc @(
        'Host installation-owner public surface retirement',
        'crate-private implementation detail',
        'Every transition that returns the owner is equally narrow',
        'crate-private launch slot remains the only production',
        'No runtime IPC, CLI, desktop',
        'default remains `legacy-only`'
    ) 'RFC-0286'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "mutation_$Label=rejected"; return }
    throw "mutation '$Label' was unexpectedly accepted"
}

foreach ($path in @(
    $manifestPath, $hostPath, $controllerPath, $authorizationPath, $slotPath,
    $servicePath, $runtimeLibraryPath, $executionPath, $cliPath, $desktopPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$hostSource = Get-Content -LiteralPath $hostPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
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
            $_.FullName -notin @(
                $hostPath, $controllerPath, $authorizationPath, $runtimeLibraryPath
            )
        } |
        Sort-Object FullName |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"

Assert-Boundary $manifest $hostSource $controller $authorization $slot $service `
    $runtimeLibrary $execution $otherRuntimeSources $cli $desktop $rfc

if ($SelfTest) {
    $rejected = 0
    $manifestMutations = @(
        @{ Section = 'installation_owner'; Property = 'crate_private'; Value = $false },
        @{ Section = 'installation_owner'; Property = 'public_reexport'; Value = $true },
        @{ Section = 'installation_owner'; Property = 'consumed_only_by_launch_slot_path'; Value = $false },
        @{ Section = 'returning_transitions'; Property = 'issuer_issue_crate_private'; Value = $false },
        @{ Section = 'returning_transitions'; Property = 'public_transition_returns_owner'; Value = $true },
        @{ Section = 'safe_vocabulary'; Property = 'public_launch_service'; Value = $false },
        @{ Section = 'test_boundary'; Property = 'owner_reexport_crate_private'; Value = $false },
        @{ Section = 'production'; Property = 'registered_runtime_ipc_command'; Value = $true },
        @{ Section = 'production'; Property = 'public_default'; Value = 'auto' },
        @{ Section = 'verification'; Property = 'network_executed'; Value = $true }
    )
    foreach ($mutation in $manifestMutations) {
        $changed = Copy-JsonObject $manifest
        $changed.($mutation.Section).($mutation.Property) = $mutation.Value
        Assert-Rejected {
            Assert-Boundary $changed $hostSource $controller $authorization $slot `
                $service $runtimeLibrary $execution $otherRuntimeSources $cli `
                $desktop $rfc
        } "$($mutation.Section)_$($mutation.Property)"
        $rejected++
    }

    $sourceMutations = @(
        @{ Label = 'public_owner_struct'; Target = 'host'; From = "pub(crate) struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>"; To = "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>" },
        @{ Label = 'public_owner_install'; Target = 'host'; From = '    pub(crate) fn install('; To = '    pub fn install(' },
        @{ Label = 'public_issuer_transition'; Target = 'controller'; From = '    pub(crate) fn issue('; To = '    pub fn issue(' },
        @{ Label = 'public_authorization_transition'; Target = 'authorization'; From = '    pub(crate) fn approve('; To = '    pub fn approve(' },
        @{ Label = 'public_runtime_reexport'; Target = 'runtime'; From = 'pub(crate) use recipient_import_guided_recovery_host::{'; To = 'pub use recipient_import_guided_recovery_host::{' },
        @{ Label = 'public_test_reexport'; Target = 'execution'; From = 'pub(crate) use recipient_import_guided_recovery_host::{'; To = 'pub use recipient_import_guided_recovery_host::{' },
        @{ Label = 'second_runtime_owner'; Target = 'other_append'; From = ''; To = "`r`nstruct AlternateOwner(RecipientImportGuidedRecoveryHostInstallationOwner<'static>);`r`n" },
        @{ Label = 'cli_owner'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryHostInstallationOwner::from_authenticated_recovery`r`n" }
    )
    foreach ($mutation in $sourceMutations) {
        $changedHost = $hostSource
        $changedController = $controller
        $changedAuthorization = $authorization
        $changedRuntimeLibrary = $runtimeLibrary
        $changedExecution = $execution
        $changedOtherRuntimeSources = $otherRuntimeSources
        $changedCli = $cli
        switch ($mutation.Target) {
            'host' { $changedHost = $hostSource.Replace($mutation.From, $mutation.To) }
            'controller' { $changedController = $controller.Replace($mutation.From, $mutation.To) }
            'authorization' { $changedAuthorization = $authorization.Replace($mutation.From, $mutation.To) }
            'runtime' { $changedRuntimeLibrary = $runtimeLibrary.Replace($mutation.From, $mutation.To) }
            'execution' { $changedExecution = $execution.Replace($mutation.From, $mutation.To) }
            'other_append' { $changedOtherRuntimeSources = $otherRuntimeSources + $mutation.To }
            'cli_append' { $changedCli = $cli + $mutation.To }
        }
        Assert-Rejected {
            Assert-Boundary $manifest $changedHost $changedController `
                $changedAuthorization $slot $service $changedRuntimeLibrary `
                $changedExecution $changedOtherRuntimeSources $changedCli $desktop $rfc
        } $mutation.Label
        $rejected++
    }
    if ($rejected -ne 18) {
        throw "installation-owner retirement boundary rejected $rejected/18 mutations"
    }
    Write-Output 'recipient_import_guided_recovery_installation_owner_retirement_self_test=passed'
}

Write-Output 'recipient_import_guided_recovery_installation_owner_retirement_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'installation_owner_visibility=crate-private'
Write-Output 'owner_returning_transitions=crate-private'
Write-Output 'production_owner_path=launch-slot-only'
Write-Output 'safe_service_facade=public'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
