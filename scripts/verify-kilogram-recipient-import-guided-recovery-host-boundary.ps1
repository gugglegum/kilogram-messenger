[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-HOST.json'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$producerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_start_producer.rs'
$uxPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_local_ux.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0275-m1251-dormant-process-owned-guided-recovery-host.md'

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
        [object]$Manifest, [string]$HostSource, [string]$Producer,
        [string]$Ux, [string]$RuntimeLibrary, [string]$Execution,
        [string]$Cli, [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.51' -or
        [string]$Manifest.status -cne 'dormant-process-owned-guided-recovery-host-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.50') {
        throw 'guided-recovery host identity changed'
    }
    foreach ($property in @(
        'process_local', 'owns_controller', 'owns_pending_handoff'
    )) {
        if (-not [bool]$Manifest.host.$property) {
            throw "guided-recovery host invariant '$property' weakened"
        }
    }
    if ([int]$Manifest.host.maximum_pending_handoffs -ne 1 -or
        [int]$Manifest.host.maximum_active_sessions -ne 1 -or
        [bool]$Manifest.host.cloneable -or
        [bool]$Manifest.host.serializable -or
        [bool]$Manifest.host.debuggable -or
        [bool]$Manifest.host.restart_recovers_pending_authority -or
        [bool]$Manifest.host.restart_recovers_active_authority -or
        [bool]$Manifest.host.opens_network) {
        throw 'guided-recovery host ownership/cardinality changed'
    }
    foreach ($property in @(
        'crate_private', 'accepts_only_authenticated_session_authority',
        'creates_sealed_m1250_handoff_internally',
        'preflights_state_account_device_and_plan', 'second_pending_rejected',
        'replacement_rejected', 'active_session_replacement_rejected',
        'foreign_binding_rejected', 'existing_custody_preserved_on_rejection'
    )) {
        if (-not [bool]$Manifest.installation.$property) {
            throw "guided-recovery installation invariant '$property' weakened"
        }
    }
    if ([bool]$Manifest.installation.incoming_authority_retained_on_rejection) {
        throw 'rejected incoming authority became reusable'
    }
    if (($Manifest.public_local_ux_surface.typed_commands -join ',') -cne
        'prepare,confirm,apply,cancel,refresh') {
        throw 'guided-recovery public command surface changed'
    }
    if (-not [bool]$Manifest.public_local_ux_surface.returns_m1249_safe_view) {
        throw 'guided-recovery host no longer returns the safe M1.2.49 view'
    }
    foreach ($property in @(
        'returns_handoff', 'returns_start_command', 'returns_controller',
        'returns_authority_fragments', 'accepts_state_path_after_construction',
        'accepts_signer_after_construction', 'accepts_descriptor',
        'accepts_coordinator', 'accepts_membership'
    )) {
        if ([bool]$Manifest.public_local_ux_surface.$property) {
            throw "guided-recovery public unsafe capability '$property' enabled"
        }
    }
    foreach ($property in @(
        'takes_pending_before_fallible_work', 'uses_m1250_one_shot_producer',
        'consumes_start_command_inside_host', 'failure_clears_pending_authority',
        'success_clears_pending_authority'
    )) {
        if (-not [bool]$Manifest.prepare.$property) {
            throw "guided-recovery prepare invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'explicit_pending_cancel', 'exact_active_cancel', 'pending_expiry',
        'active_expiry', 'failed_prepare', 'process_restart'
    )) {
        if (-not [bool]$Manifest.custody_cleanup.$property) {
            throw "guided-recovery custody cleanup '$property' weakened"
        }
    }
    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "guided-recovery production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 8 -or
        [string]$Manifest.verification.readiness_gates -cne '177/177' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'guided-recovery verification/product boundary changed'
    }

    Require-Text $HostSource @(
        "pub(crate) struct RecipientImportGuidedRecoveryHost<'identity>",
        "controller: RecipientImportGuidedSessionController<'identity>",
        'pending_handoff: Option<AuthenticatedGuidedRecipientImportSessionStartHandoff>',
        'fn install_authenticated_handoff(',
        'self.controller.preflight_local_ux_start(',
        'AuthenticatedGuidedRecipientImportSessionStartHandoff::from_authenticated_recovery(',
        'pub fn prepare(', '.pending_handoff', '.take()',
        'RecipientImportGuidedSessionStartProducer::new(&mut self.controller).produce(',
        'RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)',
        '.start(start, now_unix_seconds)',
        'pub fn confirm(', 'pub fn apply(', 'pub fn cancel(', 'pub fn refresh(',
        'RecipientImportGuidedRecoveryHostCancelTarget::PendingHandoff',
        'RecipientImportGuidedRecoveryHostCancelTarget::ActiveSession(session_id)',
        'pub fn opened_network(&self) -> bool', 'false'
    ) 'guided-recovery host source'
    foreach ($forbidden in @(
        "pub struct RecipientImportGuidedRecoveryHost<'identity>",
        'pub fn install_authenticated_handoff(',
        'pub(crate) fn install_authenticated_handoff(', 'use serde::', 'TcpListener',
        'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($HostSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "guided-recovery host contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $HostSource,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct RecipientImportGuidedRecoveryHost<'identity>[ \t]*\{"
    )) {
        throw 'guided-recovery host gained a derive attribute'
    }
    $takeOffset = $HostSource.IndexOf('.pending_handoff', [StringComparison]::Ordinal)
    $takeOffset = $HostSource.IndexOf('.take()', $takeOffset, [StringComparison]::Ordinal)
    $produceOffset = $HostSource.IndexOf(
        'RecipientImportGuidedSessionStartProducer::new(&mut self.controller).produce(',
        [StringComparison]::Ordinal
    )
    if ($takeOffset -lt 0 -or $produceOffset -lt 0 -or $takeOffset -gt $produceOffset) {
        throw 'guided-recovery prepare no longer takes pending authority before producer work'
    }
    $publicCommands = [regex]::Matches(
        $HostSource,
        '(?ms)pub fn (prepare|confirm|apply|cancel|refresh)\(.*?\)\s*->.*?\{'
    ) | ForEach-Object { $_.Value }
    $publicCommandText = $publicCommands -join "`n"
    foreach ($forbiddenType in @(
        'AuthenticatedGuidedRecipientImportSession',
        'AuthenticatedGuidedRecipientImportSessionStartHandoff',
        'RecipientImportGuidedSessionLocalUxStart',
        'RecipientImportGuidedSessionController',
        'DeviceIdentity', 'RuntimeIpcDescriptor',
        'RecipientRecoveryCoordinationPlan', 'GuidedRecipientImportMembership'
    )) {
        if ($publicCommandText.IndexOf($forbiddenType, [StringComparison]::Ordinal) -ge 0) {
            throw "guided-recovery public command leaks '$forbiddenType'"
        }
    }

    Require-Text $Producer @(
        'pub struct AuthenticatedGuidedRecipientImportSessionStartHandoff',
        'pub struct RecipientImportGuidedSessionStartProducer',
        '.authority', '.take()'
    ) 'M1.2.50 producer predecessor'
    Require-Text $Ux @(
        'pub struct RecipientImportGuidedSessionLocalUxView',
        'pub struct RecipientImportGuidedSessionLocalUxConfirm',
        'pub struct RecipientImportGuidedSessionLocalUxApply',
        'pub struct RecipientImportGuidedSessionLocalUxRefresh'
    ) 'M1.2.49 safe UX predecessor'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_host;',
        'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;',
        'pub use recipient_import_guided_recovery_host::{',
        'RecipientImportGuidedRecoveryHostPrepare',
        'RecipientImportGuidedRecoveryHostCancel'
    ) 'runtime guided-recovery host export'
    if ([regex]::IsMatch(
        $RuntimeLibrary,
        '(?m)^pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'
    ) -or [regex]::IsMatch(
        $RuntimeLibrary,
        '(?ms)pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHost\b'
    )) {
        throw 'runtime crate publicly re-exported the raw guided-recovery host'
    }
    Require-Text $Execution @(
        '#[path = "../../kilogram-peer-history-recovery-runtime/src/recipient_import_guided_recovery_host.rs"]',
        'pub(crate) use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;',
        'pub use recipient_import_guided_recovery_host::{',
        'recipient_import_guided_recovery_host_owns_one_full_local_ux_ceremony',
        'recipient_import_guided_recovery_host_fails_closed_and_drops_ephemeral_custody',
        'recipient_import_guided_recovery_host_installation_owner_accepts_only_full_assembly',
        'recipient_import_guided_recovery_host_installation_owner_is_one_shot_and_fail_closed',
        'SessionOutlivesHandoff', 'OwnerDeviceMismatch',
        'RecipientImportGuidedRecoveryHostCancel::pending_handoff()',
        'drop(host)'
    ) 'guided-recovery host fixtures'
    if ([regex]::IsMatch(
        $Execution,
        '(?m)^pub use recipient_import_guided_recovery_host::RecipientImportGuidedRecoveryHost;'
    ) -or [regex]::IsMatch(
        $Execution,
        '(?ms)pub use\s+recipient_import_guided_recovery_host::\{[^}]*\bRecipientImportGuidedRecoveryHost\b'
    )) {
        throw 'copied-module test boundary publicly re-exported the raw guided-recovery host'
    }
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf('RecipientImportGuidedRecoveryHost', [StringComparison]::Ordinal) -ge 0) {
            throw 'ordinary CLI/desktop startup registered guided-recovery host'
        }
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'owns the M1.2.48 controller and at most one pending M1.2.50 hand-off',
        'pending and active authority are intentionally process-local',
        'A second pending authority, replacement attempt, replay while active, or foreign binding is rejected',
        'public host surface consists only of typed `prepare`, `confirm`, `apply`, `cancel`, and `refresh` commands',
        'takes the pending hand-off out of the host before invoking the M1.2.50 one-shot producer',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0275 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $hostPath, $producerPath, $uxPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$hostSource = Get-Content -LiteralPath $hostPath -Raw
$producer = Get-Content -LiteralPath $producerPath -Raw
$ux = Get-Content -LiteralPath $uxPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.host.cloneable = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.host.maximum_pending_handoffs = 2
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.host.restart_recovers_pending_authority = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.installation.crate_private = $false
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.installation.existing_custody_preserved_on_rejection = $false
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.installation.incoming_authority_retained_on_rejection = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.public_local_ux_surface.returns_handoff = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.public_local_ux_surface.returns_start_command = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.public_local_ux_surface.accepts_signer_after_construction = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.prepare.takes_pending_before_fallible_work = $false
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.prepare.uses_m1250_one_shot_producer = $false
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.custody_cleanup.pending_expiry = $false
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.registered_desktop_control = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'auto'
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.verification.network_executed = $true
    $mutations += { Assert-Boundary $changed $hostSource $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedHost = $hostSource.Replace('fn install_authenticated_handoff(', 'pub fn install_authenticated_handoff(')
    $mutations += { Assert-Boundary $manifest $changedHost $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedHost = $hostSource.Replace(
        "pub(crate) struct RecipientImportGuidedRecoveryHost<'identity>",
        "#[derive(Clone)]`r`npub(crate) struct RecipientImportGuidedRecoveryHost<'identity>"
    )
    $mutations += { Assert-Boundary $manifest $changedHost $producer $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedCli = $cli + "`r`nRecipientImportGuidedRecoveryHost::new"
    $mutations += { Assert-Boundary $manifest $hostSource $producer $ux $runtimeLibrary $execution $changedCli $desktop $rfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            & $mutation
        }
        catch {
            $rejected++
        }
    }
    if ($rejected -ne $mutations.Count) {
        throw "guided-recovery host boundary rejected $rejected/$($mutations.Count) negative mutations"
    }
    Write-Output "negative_mutations=$rejected/$($mutations.Count)"
}

Write-Output 'recipient_import_guided_recovery_host_boundary=verified'
Write-Output 'maximum_pending_handoffs=1'
Write-Output 'maximum_active_sessions=1'
Write-Output 'public_commands=prepare,confirm,apply,cancel,refresh'
Write-Output 'restart_custody=empty'
Write-Output 'ordinary_startup=no-host'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
