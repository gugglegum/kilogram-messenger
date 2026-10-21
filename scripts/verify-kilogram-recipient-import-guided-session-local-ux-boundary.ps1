[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-SESSION-LOCAL-UX.json'
$uxPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_local_ux.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0273-m1249-dormant-local-ux-guided-session-adapter.md'

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

function Get-StructBody {
    param([string]$Text, [string]$Name)
    $start = $Text.IndexOf("pub struct $Name", [StringComparison]::Ordinal)
    if ($start -lt 0) { throw "missing struct $Name" }
    $bodyStart = $Text.IndexOf('{', $start)
    $bodyEnd = $Text.IndexOf("}`r`n`r`nimpl $Name", $bodyStart, [StringComparison]::Ordinal)
    if ($bodyEnd -lt 0) {
        $bodyEnd = $Text.IndexOf("}`n`nimpl $Name", $bodyStart, [StringComparison]::Ordinal)
    }
    if ($bodyStart -lt 0 -or $bodyEnd -lt 0) { throw "cannot isolate struct $Name" }
    $Text.Substring($bodyStart, $bodyEnd - $bodyStart)
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$Ux, [string]$Controller,
        [string]$RuntimeLibrary, [string]$Execution, [string]$Cli,
        [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.49' -or
        [string]$Manifest.status -cne 'dormant-local-ux-guided-session-adapter-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.48') {
        throw 'local UX guided-session identity changed'
    }
    if (-not [bool]$Manifest.adapter.process_local -or
        -not [bool]$Manifest.adapter.borrows_runtime_owned_controller -or
        [bool]$Manifest.adapter.accepts_state_path -or
        [bool]$Manifest.adapter.accepts_signer -or
        [bool]$Manifest.adapter.accepts_coordinator -or
        [bool]$Manifest.adapter.accepts_membership_snapshot -or
        -not [bool]$Manifest.adapter.generates_request_ids_internally -or
        -not [bool]$Manifest.adapter.derives_domain_separated_transition_nonces -or
        [bool]$Manifest.adapter.opens_network) {
        throw 'local UX adapter authority boundary changed'
    }
    if (-not [bool]$Manifest.start_command.contains_authenticated_process_local_authority -or
        [bool]$Manifest.start_command.public_constructor -or
        [bool]$Manifest.start_command.serializable -or
        [bool]$Manifest.start_command.cloneable -or
        [bool]$Manifest.start_command.debuggable -or
        [bool]$Manifest.start_command.accepts_serialized_authority_fragments) {
        throw 'local UX start-command custody changed'
    }
    $expectedViewFields = @(
        'phase', 'session-id', 'completed-source-count', 'total-source-count',
        'planned-step-count', 'imported-event-count', 'confirmation-phrase',
        'deadline', 'complete'
    )
    if ((@($Manifest.view.fields) -join ',') -cne ($expectedViewFields -join ',') -or
        [bool]$Manifest.view.serializable) {
        throw 'local UX safe view shape changed'
    }
    foreach ($property in @(
        'exposes_state_path', 'exposes_account_id', 'exposes_device_id',
        'exposes_descriptor', 'exposes_coordinator',
        'exposes_membership_reference', 'exposes_membership_snapshot',
        'exposes_signing_material', 'exposes_request_id', 'exposes_nonce',
        'exposes_authenticated_start_token'
    )) {
        if ([bool]$Manifest.view.$property) {
            throw "local UX view leaks '$property'"
        }
    }
    foreach ($property in @(
        'typed_start', 'typed_confirm', 'typed_apply', 'typed_cancel',
        'typed_refresh', 'exact_session_id_preflight', 'exact_phase_preflight',
        'stale_command_preserves_current_session',
        'lost_response_recovery_uses_refresh_only'
    )) {
        if (-not [bool]$Manifest.commands.$property) {
            throw "local UX command invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'double_submit_repeats_mutation', 'refresh_performs_import',
        'refresh_performs_consumption'
    )) {
        if ([bool]$Manifest.commands.$property) {
            throw "local UX command invariant '$property' became unsafe"
        }
    }
    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_scan',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "local UX production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [int]$Manifest.verification.combined_stable_path_test_count -ne 4 -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'local UX verification/product boundary changed'
    }

    Require-Text $Ux @(
        'pub struct RecipientImportGuidedSessionLocalUxStart',
        'pub(crate) fn from_authenticated_recovery(',
        'pub struct RecipientImportGuidedSessionLocalUxConfirm',
        'pub struct RecipientImportGuidedSessionLocalUxApply',
        'pub struct RecipientImportGuidedSessionLocalUxCancel',
        'pub struct RecipientImportGuidedSessionLocalUxRefresh',
        'pub struct RecipientImportGuidedSessionLocalUxView',
        "controller: &'controller mut RecipientImportGuidedSessionController<'identity>",
        'pub fn start(', 'pub fn confirm(', 'pub fn apply(', 'pub fn cancel(',
        'pub fn refresh(', 'RuntimeIpcRequestId::generate()',
        'CONFIRMATION_NONCE_DOMAIN', 'MUTATION_NONCE_DOMAIN',
        'self.require_exact_session(session_id)?',
        'self.require_exact_phase(',
        'RecipientImportGuidedSessionPhase::Planned',
        'RecipientImportGuidedSessionPhase::Confirmed',
        'self.controller.purge_expired(now_unix_seconds)?',
        'project_snapshot(self.controller.local_ux_snapshot()?)',
        'pub fn opened_network(&self) -> bool',
        'false'
    ) 'local UX adapter source'
    foreach ($forbidden in @(
        'pub fn from_authenticated_recovery(',
        'impl Serialize for RecipientImportGuidedSessionLocalUxStart',
        'impl Deserialize for RecipientImportGuidedSessionLocalUxStart',
        'impl Serialize for RecipientImportGuidedSessionLocalUxView',
        'impl Deserialize for RecipientImportGuidedSessionLocalUxView',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($Ux.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "local UX source contains forbidden '$forbidden'"
        }
    }
    $startOffset = $Ux.IndexOf('pub struct RecipientImportGuidedSessionLocalUxStart', [StringComparison]::Ordinal)
    $startPrefixStart = [Math]::Max(0, $startOffset - 100)
    $startPrefix = $Ux.Substring($startPrefixStart, $startOffset - $startPrefixStart)
    if ($startPrefix.IndexOf('#[derive', [StringComparison]::Ordinal) -ge 0) {
        throw 'RecipientImportGuidedSessionLocalUxStart gained a derive attribute'
    }
    $viewOffset = $Ux.IndexOf('pub struct RecipientImportGuidedSessionLocalUxView', [StringComparison]::Ordinal)
    $viewPrefixStart = [Math]::Max(0, $viewOffset - 100)
    $viewPrefix = $Ux.Substring($viewPrefixStart, $viewOffset - $viewPrefixStart)
    if ($viewPrefix.IndexOf('Serialize', [StringComparison]::Ordinal) -ge 0 -or
        $viewPrefix.IndexOf('Deserialize', [StringComparison]::Ordinal) -ge 0) {
        throw 'RecipientImportGuidedSessionLocalUxView became serializable'
    }
    $viewBody = Get-StructBody $Ux 'RecipientImportGuidedSessionLocalUxView'
    foreach ($forbiddenField in @(
        'state_directory', 'AccountId', 'DeviceId', 'RuntimeIpcDescriptor',
        'coordinator', 'membership', 'signer', 'request_id', 'nonce',
        'AuthenticatedGuidedRecipientImportSession'
    )) {
        if ($viewBody.IndexOf($forbiddenField, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "local UX view contains forbidden field '$forbiddenField'"
        }
    }
    Require-Text $Controller @(
        'pub(crate) enum RecipientImportGuidedSessionLocalSnapshot',
        'pub(crate) fn local_ux_snapshot(',
        'receipt: Box<RuntimeIpcRecipientImportExecutionReceiptSummary>'
    ) 'guided controller safe projection'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_session_local_ux;',
        'RecipientImportGuidedSessionLocalUxAdapter',
        'RecipientImportGuidedSessionLocalUxView'
    ) 'runtime local UX export'
    Require-Text $Execution @(
        'recipient_import_guided_session_local_ux_projects_and_recovers_each_phase',
        'recipient_import_guided_session_local_ux_fails_closed_on_stale_and_double_submit',
        'assert_eq!(recovered_plan, planned)',
        'assert_eq!(recovered_confirmation, confirmed)',
        'assert_eq!(recovered_attachment, attached)'
    ) 'local UX stable fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf('RecipientImportGuidedSessionLocalUxAdapter', [StringComparison]::Ordinal) -ge 0 -or
            $surface.IndexOf('RecipientImportGuidedSessionLocalUxStart', [StringComparison]::Ordinal) -ge 0) {
            throw 'ordinary CLI/desktop startup registered local UX guided recovery'
        }
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'no public constructor, and no Clone, Debug, Serialize, or Deserialize implementation',
        'A double confirm or double apply is rejected before durable work and preserves the already reached phase',
        'must refresh instead of replaying the command',
        'Process restart constructs a new controller and therefore also returns Idle',
        'Ordinary startup still creates neither start command nor session',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0273 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $uxPath, $controllerPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$ux = Get-Content -LiteralPath $uxPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $ux $controller $runtimeLibrary $execution $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.adapter.accepts_state_path = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.start_command.public_constructor = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.view.serializable = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.view.exposes_account_id = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.commands.double_submit_repeats_mutation = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.commands.refresh_performs_import = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.registered_desktop_control = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.adapter.opens_network = $true
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'enabled'
    $mutations += { Assert-Boundary $changed $ux $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changedUx = $ux.Replace('pub(crate) fn from_authenticated_recovery(', 'pub fn from_authenticated_recovery(')
    $mutations += { Assert-Boundary $manifest $changedUx $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changedUx = $ux.Replace(
        'pub struct RecipientImportGuidedSessionLocalUxStart',
        "#[derive(Serialize)]`r`npub struct RecipientImportGuidedSessionLocalUxStart"
    )
    $mutations += { Assert-Boundary $manifest $changedUx $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changedUx = $ux.Replace(
        'phase: RecipientImportGuidedSessionLocalUxPhase,',
        "state_directory: PathBuf,`r`n    phase: RecipientImportGuidedSessionLocalUxPhase,"
    )
    $mutations += { Assert-Boundary $manifest $changedUx $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changedUx = $ux.Replace('self.require_exact_phase(', 'self.skip_exact_phase(')
    $mutations += { Assert-Boundary $manifest $changedUx $controller $runtimeLibrary $execution $cli $desktop $rfc }
    $changedCli = $cli + "`r`nRecipientImportGuidedSessionLocalUxAdapter::new"
    $mutations += { Assert-Boundary $manifest $ux $controller $runtimeLibrary $execution $changedCli $desktop $rfc }
    $changedRfc = $rfc.Replace('`legacy-only`', '`enabled`')
    $mutations += { Assert-Boundary $manifest $ux $controller $runtimeLibrary $execution $cli $desktop $changedRfc }
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
    if ($rejected -ne 15) { throw "expected 15 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=15/15'
}

Write-Output 'recipient_import_guided_session_local_ux_boundary=verified'
Write-Output 'safe_view_fields=9'
Write-Output 'typed_commands=start,confirm,apply,cancel,refresh'
Write-Output 'lost_response_recovery=refresh-only'
Write-Output 'ordinary_startup=idle-unattached'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
