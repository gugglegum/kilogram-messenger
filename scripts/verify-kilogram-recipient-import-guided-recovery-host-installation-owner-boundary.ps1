[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-HOST-INSTALLATION-OWNER.json'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0276-m1252-authenticated-one-shot-host-installation-owner.md'

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
        [object]$Manifest, [string]$HostSource, [string]$Controller,
        [string]$RuntimeLibrary, [string]$Execution, [string]$Cli,
        [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.52' -or
        [string]$Manifest.status -cne 'authenticated-one-shot-host-installation-owner-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.51') {
        throw 'host-installation owner identity changed'
    }

    foreach ($property in @(
        'contains_signed_plan', 'contains_exact_coordinator',
        'contains_ordered_loaded_memberships', 'binds_exact_local_signer',
        'opaque_host_installation_type'
    )) {
        if (-not [bool]$Manifest.assembly.$property) {
            throw "host-installation assembly invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'public_constructor', 'cloneable', 'serializable', 'debuggable',
        'ambient_filesystem_discovery'
    )) {
        if ([bool]$Manifest.assembly.$property) {
            throw "host-installation assembly unsafe capability '$property' enabled"
        }
    }

    if ([string]$Manifest.owner.type -cne 'RecipientImportGuidedRecoveryHostInstallationOwner') {
        throw 'host-installation owner type changed'
    }
    foreach ($property in @(
        'process_local', 'accepts_only_full_assembly', 'install_consumes_self',
        'assembly_extracted_before_fallible_host_work',
        'returns_host_only_on_success'
    )) {
        if (-not [bool]$Manifest.owner.$property) {
            throw "host-installation owner invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'public_constructor', 'accepts_loose_state_path', 'accepts_loose_signer',
        'accepts_loose_descriptor', 'accepts_loose_coordinator',
        'accepts_loose_membership', 'cloneable', 'serializable', 'debuggable',
        'returns_reusable_grant', 'replayable_after_success',
        'replayable_after_failure', 'restart_recovers_owner', 'opens_network'
    )) {
        if ([bool]$Manifest.owner.$property) {
            throw "host-installation owner unsafe capability '$property' enabled"
        }
    }

    foreach ($property in @(
        'host_constructor_private', 'host_install_method_private',
        'only_installer_is_owner', 'creates_m1250_handoff_internally',
        'repeats_canonical_preflight', 'public_local_ux_remains_on_host'
    )) {
        if (-not [bool]$Manifest.host_installation.$property) {
            throw "host-installation invariant '$property' weakened"
        }
    }
    if ([bool]$Manifest.host_installation.replacement_surface_exposed -or
        ($Manifest.host_installation.public_local_ux_commands -join ',') -cne
            'prepare,confirm,apply,cancel,refresh') {
        throw 'host installation or local UX surface changed'
    }

    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_owner', 'ordinary_startup_creates_host',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "host-installation production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 10 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.readiness_gates -cne '178/178' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        -not [bool]$Manifest.verification.full_assembly_install_tested -or
        -not [bool]$Manifest.verification.exact_signer_binding_tested -or
        -not [bool]$Manifest.verification.invalid_install_failure_tested -or
        -not [bool]$Manifest.verification.owner_consumed_by_install -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'host-installation verification/product boundary changed'
    }

    Require-Text $Controller @(
        'pub struct AuthenticatedGuidedRecipientImportSession',
        'pub(crate) fn into_host_installation(', 'self,',
        'signer: &DeviceIdentity', 'self.verify_controller_binding(',
        'signer.device_id()',
        "pub(crate) struct AuthenticatedGuidedRecipientImportHostInstallation<'identity>",
        'authority: AuthenticatedGuidedRecipientImportSession',
        "signer: &'identity DeviceIdentity", 'pub(crate) fn into_parts(',
        'self.authority.state_directory.clone()',
        'self.authority.owner_account_id'
    ) 'authenticated host-installation assembly'
    foreach ($forbidden in @(
        'pub fn into_host_installation(',
        "pub struct AuthenticatedGuidedRecipientImportHostInstallation<'identity>"
    )) {
        if ($Controller.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "authenticated host-installation assembly contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Controller,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct AuthenticatedGuidedRecipientImportHostInstallation"
    )) {
        throw 'authenticated host-installation assembly gained a derive attribute'
    }

    Require-Text $HostSource @(
        "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        "installation: AuthenticatedGuidedRecipientImportHostInstallation<'identity>",
        'pub(crate) fn from_authenticated_recovery(',
        "installation: AuthenticatedGuidedRecipientImportHostInstallation<'identity>",
        'pub fn install(', 'self,', 'self.installation.into_parts()',
        'RecipientImportGuidedRecoveryHost::new(',
        'host.install_authenticated_handoff(', 'Ok(host)',
        "impl<'identity> RecipientImportGuidedRecoveryHost<'identity>",
        'fn new(', 'fn install_authenticated_handoff(',
        'AuthenticatedGuidedRecipientImportSessionStartHandoff::from_authenticated_recovery(',
        'pub fn prepare(', 'pub fn confirm(', 'pub fn apply(',
        'pub fn cancel(', 'pub fn refresh('
    ) 'host-installation owner source'
    foreach ($forbidden in @(
        'pub fn install_authenticated_handoff(',
        'pub(crate) fn install_authenticated_handoff(',
        'pub fn into_parts(', 'use serde::', 'TcpListener', 'UdpSocket',
        'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($HostSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "host-installation owner source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $HostSource,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub struct RecipientImportGuidedRecoveryHostInstallationOwner"
    )) {
        throw 'host-installation owner gained a derive attribute'
    }
    if (-not [regex]::IsMatch(
        $HostSource,
        '(?s)impl<''identity> RecipientImportGuidedRecoveryHostInstallationOwner<''identity> \{.*?pub fn install\(\s*self,'
    )) {
        throw 'host-installation owner install no longer consumes self'
    }
    if ([regex]::IsMatch(
        $HostSource,
        "(?s)impl<'identity> RecipientImportGuidedRecoveryHost<'identity> \{\s+pub(?:\(crate\))? fn new\("
    )) {
        throw 'guided-recovery host constructor became externally callable'
    }
    $partsOffset = $HostSource.IndexOf('self.installation.into_parts()', [StringComparison]::Ordinal)
    $hostOffset = $HostSource.IndexOf('RecipientImportGuidedRecoveryHost::new(', [StringComparison]::Ordinal)
    if ($partsOffset -lt 0 -or $hostOffset -lt 0 -or $partsOffset -gt $hostOffset) {
        throw 'owner no longer extracts the one-shot assembly before fallible host work'
    }
    if ([regex]::Matches($HostSource, 'install_authenticated_handoff\(').Count -ne 2) {
        throw 'authenticated host installation is no longer limited to one owner call and one definition'
    }

    Require-Text $RuntimeLibrary @(
        'RecipientImportGuidedRecoveryHostInstallationOwner',
        'RecipientImportGuidedRecoveryHostPrepare',
        'RecipientImportGuidedRecoveryHostCancel'
    ) 'runtime installation-owner export'
    if ($RuntimeLibrary.IndexOf(
        'AuthenticatedGuidedRecipientImportHostInstallation',
        [StringComparison]::Ordinal
    ) -ge 0) {
        throw 'opaque host-installation assembly was publicly re-exported'
    }
    Require-Text $Execution @(
        'recipient_import_guided_recovery_host_installation_owner_accepts_only_full_assembly',
        'recipient_import_guided_recovery_host_installation_owner_is_one_shot_and_fail_closed',
        '.into_host_installation(&foreign_signer)',
        'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch',
        'owner.install(0, 100)', 'owner.install(17_100, 0)'
    ) 'host-installation owner fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf(
            'RecipientImportGuidedRecoveryHostInstallationOwner',
            [StringComparison]::Ordinal
        ) -ge 0) {
            throw 'ordinary CLI/desktop startup registered the host-installation owner'
        }
    }

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'complete canonical recovery assembly',
        'host-installation value is crate-private and opaque',
        'public `install` method consumes `self`',
        'assembly is destructured before host construction',
        'host constructor and authenticated install method are now private',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0276 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $hostPath, $controllerPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$hostSource = Get-Content -LiteralPath $hostPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.assembly.public_constructor = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.assembly.cloneable = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.accepts_loose_state_path = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.accepts_loose_membership = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.install_consumes_self = $false
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.assembly_extracted_before_fallible_host_work = $false
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.returns_reusable_grant = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.owner.restart_recovers_owner = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.host_installation.host_constructor_private = $false
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.host_installation.only_installer_is_owner = $false
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.registered_cli_command = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.ordinary_startup_creates_owner = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'auto'
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.network_executed = $true
    $mutations += ({ Assert-Boundary $changed $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedHost = [regex]::Replace(
        $hostSource,
        "(?s)(impl<'identity> RecipientImportGuidedRecoveryHost<'identity> \{\s+)fn new\(",
        '${1}pub fn new(',
        1
    )
    $mutations += ({ Assert-Boundary $manifest $changedHost $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedHost = $hostSource.Replace(
        'fn install_authenticated_handoff(',
        'pub(crate) fn install_authenticated_handoff('
    )
    $mutations += ({ Assert-Boundary $manifest $changedHost $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedHost = $hostSource.Replace(
        "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>"
    )
    $mutations += ({ Assert-Boundary $manifest $changedHost $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedHost = [regex]::Replace(
        $hostSource,
        '(?s)(pub fn install\(\s+)self,',
        '${1}&mut self,',
        1
    )
    $mutations += ({ Assert-Boundary $manifest $changedHost $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = $controller.Replace(
        'pub(crate) fn into_host_installation(',
        'pub fn into_host_installation('
    )
    $mutations += ({ Assert-Boundary $manifest $hostSource $changedController $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = $controller.Replace(
        "pub(crate) struct AuthenticatedGuidedRecipientImportHostInstallation<'identity>",
        "pub struct AuthenticatedGuidedRecipientImportHostInstallation<'identity>"
    )
    $mutations += ({ Assert-Boundary $manifest $hostSource $changedController $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedCli = $cli + "`r`nRecipientImportGuidedRecoveryHostInstallationOwner::from_authenticated_recovery"
    $mutations += ({ Assert-Boundary $manifest $hostSource $controller $runtimeLibrary $execution $changedCli $desktop $rfc }.GetNewClosure())

    $rejected = 0
    $mutationIndex = 0
    foreach ($mutation in $mutations) {
        $mutationIndex++
        try {
            & $mutation
            Write-Output "unexpectedly_accepted_mutation=$mutationIndex"
        }
        catch {
            $rejected++
        }
    }
    if ($rejected -ne $mutations.Count) {
        throw "host-installation owner boundary rejected $rejected/$($mutations.Count) negative mutations"
    }
    Write-Output "negative_mutations=$rejected/$($mutations.Count)"
}

Assert-Boundary $manifest $hostSource $controller $runtimeLibrary $execution $cli $desktop $rfc
Write-Output 'recipient_import_guided_recovery_host_installation_owner_boundary=verified'
Write-Output 'owner_installation=one-shot-full-assembly-only'
Write-Output 'host_constructor=private'
Write-Output 'host_installation=private-owner-only'
Write-Output 'ordinary_startup=no-owner-no-host'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
