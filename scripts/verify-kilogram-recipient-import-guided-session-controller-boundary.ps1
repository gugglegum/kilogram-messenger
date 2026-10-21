[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-SESSION-CONTROLLER.json'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$attachmentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_runtime_attachment.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0272-m1248-runtime-owned-explicit-guided-session-controller.md'

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

function Require-MinimumCount {
    param([string]$Text, [string]$Needle, [int]$Minimum, [string]$Label)
    $count = ([regex]::Matches($Text, [regex]::Escape($Needle))).Count
    if ($count -lt $Minimum) {
        throw "$Label contains $count occurrences of '$Needle'; expected at least $Minimum"
    }
}

function Assert-Boundary {
    param(
        [object]$Manifest, [string]$Controller, [string]$Attachment,
        [string]$RuntimeLibrary, [string]$Execution, [string]$Cli, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.48' -or
        [string]$Manifest.status -cne 'runtime-owned-explicit-guided-session-controller-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.47') {
        throw 'guided session controller identity changed'
    }
    if (-not [bool]$Manifest.controller.process_owned -or
        [int]$Manifest.controller.maximum_active_sessions -ne 1 -or
        [string]$Manifest.controller.ordinary_startup_phase -cne 'idle' -or
        [int]$Manifest.controller.maximum_lifetime_seconds -ne 300 -or
        -not [bool]$Manifest.controller.owns_consumption_actor -or
        -not [bool]$Manifest.controller.owns_signer_reference -or
        [bool]$Manifest.controller.opens_network) {
        throw 'guided session controller ownership changed'
    }
    $expectedPhases = @('idle', 'planned', 'confirmed', 'attached')
    if ((@($Manifest.controller.phases) -join ',') -cne ($expectedPhases -join ',')) {
        throw 'guided session controller phases changed'
    }
    $authority = $Manifest.start_authority
    if ([string]$authority.type -cne 'authenticated-process-local-token' -or
        [bool]$authority.public_constructor -or [bool]$authority.serializable -or
        [bool]$authority.cloneable -or [bool]$authority.debuggable -or
        [bool]$authority.accepts_path_during_begin -or
        [bool]$authority.accepts_coordinator_during_begin -or
        [bool]$authority.accepts_membership_snapshot_during_begin -or
        [bool]$authority.accepts_secret_during_session_operations -or
        -not [bool]$authority.exact_state_account_device_binding -or
        -not [bool]$authority.exact_descriptor_binding -or
        -not [bool]$authority.exact_plan_and_membership_binding) {
        throw 'guided session start authority changed'
    }
    foreach ($property in @(
        'plan_is_read_only', 'confirmation_is_device_signed_and_durable',
        'mutation_is_bounded_and_durable', 'authenticated_completion_is_required',
        'attachment_occurs_once', 'consumption_delegates_to_existing_v34_actor',
        'exact_plan_replay_is_byte_stable'
    )) {
        if (-not [bool]$Manifest.lifecycle.$property) {
            throw "guided session lifecycle '$property' weakened"
        }
    }
    if ([string]$Manifest.lifecycle.lost_commit_response_replays_as -cne 'already-consumed') {
        throw 'guided session replay outcome changed'
    }
    foreach ($property in @(
        'cancel_drops_ephemeral_session', 'expiry_drops_ephemeral_session',
        'failed_confirmation_drops_ephemeral_session',
        'failed_mutation_drops_ephemeral_session', 'attached_expiry_detaches_actor',
        'detach_clears_consumption_plan_cache'
    )) {
        if (-not [bool]$Manifest.cleanup.$property) {
            throw "guided session cleanup '$property' weakened"
        }
    }
    if ([bool]$Manifest.cleanup.process_restart_restores_session -or
        [bool]$Manifest.cleanup.durable_ledgers_create_ambient_authority) {
        throw 'guided session restart acquired ambient authority'
    }
    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer', 'adds_filesystem_scan',
        'changes_peer_wire', 'changes_runtime_ipc_wire', 'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "guided session production scope '$property' became active"
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
        throw 'guided session verification scope changed'
    }

    Require-Text $Controller @(
        'pub struct AuthenticatedGuidedRecipientImportSession',
        'fn from_authenticated_recovery(',
        '#[cfg(test)]',
        'pub(crate) fn authenticated_guided_recipient_import_session_fixture(',
        'pub enum RecipientImportGuidedSessionPhase',
        'Idle,', 'Planned,', 'Confirmed,', 'Attached,',
        'pub struct RecipientImportGuidedSessionController',
        'active: Option<ActiveGuidedRecipientImportSession>',
        'pub fn begin(', 'pub fn confirm(', 'pub fn apply(',
        'pub fn dispatch_consumption(', 'pub fn cancel(', 'pub fn purge_expired(',
        'RecipientHistoryRecoveryImportReadAdapter::dispatch(',
        'self.confirmations.commit(&plan, now_unix_seconds)?',
        'self.mutations.dispatch(',
        '.into_authenticated_guided_completion()',
        'self.actor.install_authenticated_guided_recovery(',
        'self.actor.clear_guided_context(&attachment_id)?',
        'MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS: u64 = 5 * 60'
    ) 'guided session controller source'
    Require-MinimumCount $Controller '.active' 10 'guided session active-slot ownership'
    Require-MinimumCount $Controller '.take()' 4 'guided session destructive transitions'
    foreach ($forbidden in @(
        'pub fn from_authenticated_recovery(',
        'pub(crate) fn from_authenticated_recovery(',
        'impl Serialize for AuthenticatedGuidedRecipientImportSession',
        'impl Deserialize for AuthenticatedGuidedRecipientImportSession',
        'derive(Clone, Debug, Serialize)',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint'
    )) {
        if ($Controller.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "guided session controller contains forbidden '$forbidden'"
        }
    }
    $tokenOffset = $Controller.IndexOf(
        'pub struct AuthenticatedGuidedRecipientImportSession',
        [StringComparison]::Ordinal
    )
    $tokenPrefixStart = [Math]::Max(0, $tokenOffset - 120)
    $tokenPrefix = $Controller.Substring($tokenPrefixStart, $tokenOffset - $tokenPrefixStart)
    if ($tokenPrefix.IndexOf('#[derive', [StringComparison]::Ordinal) -ge 0) {
        throw 'authenticated guided session token gained a derive attribute'
    }
    Require-Text $Attachment @(
        'pub struct AuthenticatedGuidedRecipientImportCompletion',
        'pub fn install_authenticated_guided_recovery('
    ) 'guided attachment predecessor'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_session_controller;',
        'AuthenticatedGuidedRecipientImportSession',
        'RecipientImportGuidedSessionController'
    ) 'runtime library export'
    Require-Text $Execution @(
        'recipient_import_guided_session_controller_owns_full_explicit_lifecycle',
        'recipient_import_guided_session_controller_fails_closed_across_phases',
        'RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed(_)',
        'foreign-guided-controller-state',
        'RecipientImportGuidedSessionControllerError::OwnerAccountMismatch',
        'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch'
    ) 'guided session real fixtures'
    if ($Cli.IndexOf('RecipientImportGuidedSessionController', [StringComparison]::Ordinal) -ge 0 -or
        $Cli.IndexOf('AuthenticatedGuidedRecipientImportSession', [StringComparison]::Ordinal) -ge 0) {
        throw 'ordinary CLI/runtime startup registered the guided session controller'
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Its explicit phases are `Idle`, `Planned`, `Confirmed`, and `Attached`',
        'no public constructor, and no Clone, Debug, Serialize or Deserialize implementation',
        'leaves the controller idle and the actor unattached',
        'A new controller after process restart is idle',
        'Ordinary startup has no token producer and stays idle/unattached',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0272 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $controllerPath, $attachmentPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$controller = Get-Content -LiteralPath $controllerPath -Raw
$attachment = Get-Content -LiteralPath $attachmentPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $controller, $attachment, $runtimeLibrary, $execution, $cli, $rfc)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.controller.maximum_active_sessions = 2
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.start_authority.public_constructor = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.start_authority.serializable = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.start_authority.accepts_path_during_begin = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.cleanup.failed_confirmation_drops_ephemeral_session = $false
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.cleanup.failed_mutation_drops_ephemeral_session = $false
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.cleanup.process_restart_restores_session = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.registered_runtime_ipc_command = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.registered_cli_command = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changed = Copy-JsonObject $manifest; $changed.controller.opens_network = $true
    $mutations += { Assert-Boundary $changed $controller $attachment $runtimeLibrary $execution $cli $rfc }
    $changedController = $controller.Replace(
        '    fn from_authenticated_recovery(',
        '    pub(crate) fn from_authenticated_recovery('
    )
    $mutations += { Assert-Boundary $manifest $changedController $attachment $runtimeLibrary $execution $cli $rfc }
    $changedController = $controller.Replace(
        'pub struct AuthenticatedGuidedRecipientImportSession',
        "#[derive(Serialize)]`r`npub struct AuthenticatedGuidedRecipientImportSession"
    )
    $mutations += { Assert-Boundary $manifest $changedController $attachment $runtimeLibrary $execution $cli $rfc }
    $changedCli = $cli + "`r`nRecipientImportGuidedSessionController::new"
    $mutations += { Assert-Boundary $manifest $controller $attachment $runtimeLibrary $execution $changedCli $rfc }
    $changedRfc = $rfc.Replace('`legacy-only`', '`enabled`')
    $mutations += { Assert-Boundary $manifest $controller $attachment $runtimeLibrary $execution $cli $changedRfc }
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
    if ($rejected -ne 14) { throw "expected 14 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=14/14'
}

Write-Output 'recipient_import_guided_session_controller_boundary=verified'
Write-Output 'controller_phases=idle,planned,confirmed,attached'
Write-Output 'maximum_active_sessions=1'
Write-Output 'ordinary_startup=idle-unattached'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
