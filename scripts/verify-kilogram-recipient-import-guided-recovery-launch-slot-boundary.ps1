[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-LAUNCH-SLOT.json'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_authorization.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0279-m1255-process-owned-pending-guided-recovery-launch-slot.md'

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
        [object]$Manifest, [string]$Slot, [string]$Authorization,
        [string]$RuntimeLibrary, [string]$Execution, [string]$Cli,
        [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.55' -or
        [string]$Manifest.status -cne 'process-owned-pending-launch-slot-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.54') {
        throw 'launch-slot identity changed'
    }
    if ([string]$Manifest.slot.type -cne 'RecipientImportGuidedRecoveryLaunchSlot' -or
        [int]$Manifest.slot.maximum_pending_authorizations -ne 1 -or
        [int]$Manifest.slot.maximum_installed_hosts -ne 1) {
        throw 'launch-slot type or cardinality changed'
    }
    foreach ($property in @(
        'process_owned', 'crate_private_empty_constructor', 'crate_private_install',
        'install_accepts_complete_authorization_only',
        'install_consumes_incoming_authorization', 'install_checks_clock_and_expiry',
        'rejects_pending_replacement', 'rejects_installed_host_replacement'
    )) {
        if (-not [bool]$Manifest.slot.$property) {
            throw "launch-slot invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'silent_replacement', 'silent_eviction', 'cloneable', 'serializable',
        'debuggable', 'restart_recovers_pending_authorization',
        'restart_recovers_installed_host'
    )) {
        if ([bool]$Manifest.slot.$property) {
            throw "launch-slot unsafe capability '$property' enabled"
        }
    }
    foreach ($property in @(
        'typed_command', 'takes_pending_before_fallible_work',
        'uses_m1254_exact_phrase', 'installs_exactly_one_host',
        'returns_safe_state_only', 'mismatch_clears_pending',
        'expiry_clears_pending', 'clock_regression_clears_pending',
        'issuer_failure_clears_pending', 'host_install_failure_clears_pending'
    )) {
        if (-not [bool]$Manifest.approval.$property) {
            throw "launch-slot approval invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'returns_authorization', 'returns_installation_owner', 'returns_host',
        'returns_session_or_handoff'
    )) {
        if ([bool]$Manifest.approval.$property) {
            throw "launch-slot approval exposed '$property'"
        }
    }
    foreach ($property in @(
        'typed_command', 'takes_and_destroys_pending', 'returns_safe_state_only',
        'cannot_cancel_installed_host'
    )) {
        if (-not [bool]$Manifest.cancel.$property) {
            throw "launch-slot cancel invariant '$property' weakened"
        }
    }
    if ([bool]$Manifest.cancel.replayable) {
        throw 'launch-slot cancellation became replayable'
    }
    foreach ($property in @('borrows_m1254_safe_preview_only', 'contains_state_only')) {
        if (-not [bool]$Manifest.preview.$property) {
            throw "launch-slot preview invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'contains_identity_or_authority', 'exposes_issuer', 'exposes_authorization',
        'exposes_installation_owner', 'exposes_host', 'exposes_session_or_handoff'
    )) {
        if ([bool]$Manifest.preview.$property) {
            throw "launch-slot preview leaked '$property'"
        }
    }
    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_slot', 'ordinary_startup_installs_authorization',
        'ordinary_startup_approves_authorization', 'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "launch-slot production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 16 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.readiness_gates -cne '181/181' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        -not [bool]$Manifest.verification.success_installs_one_host -or
        -not [bool]$Manifest.verification.clock_regression_rejected -or
        -not [bool]$Manifest.verification.expired_install_rejected -or
        -not [bool]$Manifest.verification.pending_replacement_rejected -or
        -not [bool]$Manifest.verification.installed_host_replacement_rejected -or
        -not [bool]$Manifest.verification.cancel_clears_pending -or
        -not [bool]$Manifest.verification.mismatch_clears_pending -or
        -not [bool]$Manifest.verification.expiry_clears_pending -or
        -not [bool]$Manifest.verification.issuer_failure_clears_pending -or
        -not [bool]$Manifest.verification.host_install_failure_clears_pending -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'launch-slot verification/product boundary changed'
    }

    Require-Text $Slot @(
        "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "pending_authorization: Option<RecipientImportGuidedRecoveryLaunchAuthorization<'identity>>",
        "installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>",
        'pub(crate) fn new_empty() -> Self', 'pub(crate) fn install(',
        "authorization: RecipientImportGuidedRecoveryLaunchAuthorization<'identity>",
        'if self.pending_authorization.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'if self.installed_host.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled',
        'authorization.validate_pending_at(now_unix_seconds)?;',
        'self.pending_authorization = Some(authorization);',
        'pub fn state(&self) -> RecipientImportGuidedRecoveryLaunchSlotState',
        'pub fn preview(', 'pub fn approve(', 'pub fn cancel(',
        'command: RecipientImportGuidedRecoveryLaunchSlotApprove',
        '_command: RecipientImportGuidedRecoveryLaunchSlotCancel',
        'let authorization = self', '.pending_authorization', '.take()',
        'let owner = authorization.approve(command.approval, now_unix_seconds)?;',
        'let host = owner.install(now_unix_seconds, command.host_handoff_lifetime_seconds)?;',
        'self.installed_host = Some(host);',
        'Ok(RecipientImportGuidedRecoveryLaunchSlotState::HostInstalled)',
        'authorization.cancel();',
        'Ok(RecipientImportGuidedRecoveryLaunchSlotState::Empty)',
        'pub enum RecipientImportGuidedRecoveryLaunchSlotState',
        'pub struct RecipientImportGuidedRecoveryLaunchSlotApprove',
        'pub struct RecipientImportGuidedRecoveryLaunchSlotCancel'
    ) 'launch-slot source'
    foreach ($forbidden in @(
        'pub fn new_empty()', 'pub fn install(', 'use serde::',
        '#[derive(Serialize', '#[derive(Deserialize', 'impl Serialize',
        'impl Deserialize', 'TcpListener', 'UdpSocket', 'iroh::Endpoint',
        'std::fs::read_dir'
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "launch-slot source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Slot,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )) {
        throw 'launch slot gained a derive attribute'
    }
    $validateOffset = $Slot.IndexOf(
        'authorization.validate_pending_at(now_unix_seconds)?;',
        [StringComparison]::Ordinal
    )
    $retainOffset = $Slot.IndexOf(
        'self.pending_authorization = Some(authorization);',
        [StringComparison]::Ordinal
    )
    if ($validateOffset -lt 0 -or $retainOffset -lt 0 -or $validateOffset -gt $retainOffset) {
        throw 'launch slot retains authorization before validation'
    }
    $takeOffset = $Slot.IndexOf('let authorization = self', [StringComparison]::Ordinal)
    $approvalOffset = $Slot.IndexOf(
        'let owner = authorization.approve(command.approval, now_unix_seconds)?;',
        [StringComparison]::Ordinal
    )
    if ($takeOffset -lt 0 -or $approvalOffset -lt 0 -or $takeOffset -gt $approvalOffset) {
        throw 'launch slot no longer takes pending custody before fallible approval'
    }
    $approveHeader = [regex]::Match(
        $Slot,
        '(?s)pub fn approve\(.*?\) -> Result<(?<result>.*?)\> \{'
    )
    if (-not $approveHeader.Success -or
        $approveHeader.Groups['result'].Value.IndexOf(
            'RecipientImportGuidedRecoveryLaunchSlotState',
            [StringComparison]::Ordinal
        ) -lt 0) {
        throw 'launch slot approve no longer returns only safe state'
    }
    foreach ($forbidden in @('InstallationOwner', "RecoveryHost<'", 'SessionStartHandoff')) {
        if ($approveHeader.Groups['result'].Value.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "launch slot approve return type exposed '$forbidden'"
        }
    }

    Require-Text $Authorization @(
        'pub(crate) fn validate_pending_at(',
        'if now_unix_seconds < self.issued_at_unix_seconds',
        'if now_unix_seconds >= self.preview.expires_at_unix_seconds',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired'
    ) 'launch authorization slot admission'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_slot;',
        'RecipientImportGuidedRecoveryLaunchSlot',
        'RecipientImportGuidedRecoveryLaunchSlotApprove',
        'RecipientImportGuidedRecoveryLaunchSlotCancel',
        'RecipientImportGuidedRecoveryLaunchSlotState'
    ) 'runtime launch-slot export'
    Require-Text $Execution @(
        'recipient_import_guided_recovery_launch_slot_owns_one_approved_host',
        'recipient_import_guided_recovery_launch_slot_rejects_replacement_and_clears_failures',
        'RecipientImportGuidedRecoveryLaunchSlot::new_empty()',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled',
        'RecipientImportGuidedRecoveryLaunchSlotCancel::pending()',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ConfirmationMismatch',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired',
        'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch',
        'RecipientImportGuidedSessionControllerError::InvalidLifetime'
    ) 'launch-slot real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf(
            'RecipientImportGuidedRecoveryLaunchSlot',
            [StringComparison]::Ordinal
        ) -ge 0) {
            throw 'ordinary CLI/desktop startup registered launch slot'
        }
    }

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'owns at most one complete M1.2.54 authorization',
        'at most one installed M1.2.51 host',
        'no silent replacement, eviction',
        'Approve takes the pending authorization out of the slot before',
        'leave the slot empty and no reusable grant',
        'Typed cancel likewise takes and destroys the pending authorization',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0279 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $slotPath, $authorizationPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$slot = Get-Content -LiteralPath $slotPath -Raw
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.slot.maximum_pending_authorizations = 2
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.slot.silent_replacement = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.slot.cloneable = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.approval.takes_pending_before_fallible_work = $false
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.approval.returns_host = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.cancel.replayable = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.preview.exposes_authorization = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.registered_cli_command = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.ordinary_startup_creates_slot = $true
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'auto'
    $mutations += ({ Assert-Boundary $changed $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedSlot = $slot.Replace('pub(crate) fn new_empty()', 'pub fn new_empty()')
    $mutations += ({ Assert-Boundary $manifest $changedSlot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedSlot = $slot.Replace('pub(crate) fn install(', 'pub fn install(')
    $mutations += ({ Assert-Boundary $manifest $changedSlot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedSlot = $slot.Replace(
        "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )
    $mutations += ({ Assert-Boundary $manifest $changedSlot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedSlot = $slot.Replace(
        'let authorization = self',
        'let authorization = unsafe_reusable_authorization(self)'
    )
    $mutations += ({ Assert-Boundary $manifest $changedSlot $authorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedAuthorization = $authorization.Replace(
        'pub(crate) fn validate_pending_at(',
        'pub fn validate_pending_at('
    )
    $mutations += ({ Assert-Boundary $manifest $slot $changedAuthorization $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedCli = $cli + "`r`nRecipientImportGuidedRecoveryLaunchSlot::new_empty()"
    $mutations += ({ Assert-Boundary $manifest $slot $authorization $runtimeLibrary $execution $changedCli $desktop $rfc }.GetNewClosure())

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
        throw "launch-slot boundary rejected $rejected/$($mutations.Count) negative mutations"
    }
    Write-Output "negative_mutations=$rejected/$($mutations.Count)"
}

Assert-Boundary $manifest $slot $authorization $runtimeLibrary $execution $cli $desktop $rfc
Write-Output 'recipient_import_guided_recovery_launch_slot_boundary=verified'
Write-Output 'slot=one-pending-authorization-one-installed-host'
Write-Output 'replacement=forbidden-no-eviction'
Write-Output 'approval=take-before-fallible-install-safe-state-only'
Write-Output 'failure=clears-pending-custody'
Write-Output 'ordinary_startup=no-slot-no-install-no-approval'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
