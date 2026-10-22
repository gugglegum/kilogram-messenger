[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-HOST-COMMAND-ROUTER.json'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$uxPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_local_ux.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0280-m1256-opaque-installed-host-command-router.md'

foreach ($path in @(
    $manifestPath, $slotPath, $hostPath, $uxPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "host-command-router boundary file is missing: $path"
    }
}

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

function Require-FalseProperties {
    param([object]$Value, [string[]]$Properties, [string]$Label)
    foreach ($property in $Properties) {
        if ([bool]$Value.$property) {
            throw "$Label unsafe capability '$property' became true"
        }
    }
}

function Require-TrueProperties {
    param([object]$Value, [string[]]$Properties, [string]$Label)
    foreach ($property in $Properties) {
        if (-not [bool]$Value.$property) {
            throw "$Label invariant '$property' became false"
        }
    }
}

function Assert-RoutedMethod {
    param(
        [string]$Slot,
        [string]$Method,
        [string]$CommandType,
        [string]$HostCall
    )
    $escapedMethod = [regex]::Escape($Method)
    $match = [regex]::Match(
        $Slot,
        "(?s)pub fn $escapedMethod\(.*?command: $([regex]::Escape($CommandType)).*?\) -> Result<(?<return>.*?)>\s*\{(?<body>.*?)\r?\n    \}"
    )
    if (-not $match.Success) {
        throw "host-command-router method '$Method' is missing or changed"
    }
    $returnType = $match.Groups['return'].Value
    if ($returnType.IndexOf('RecipientImportGuidedSessionLocalUxView', [StringComparison]::Ordinal) -lt 0 -or
        $returnType.IndexOf('RecipientImportGuidedRecoveryLaunchSlotError', [StringComparison]::Ordinal) -lt 0) {
        throw "host-command-router method '$Method' no longer returns the safe UX view"
    }
    foreach ($forbidden in @(
        "RecipientImportGuidedRecoveryHost<'", 'InstallationOwner',
        'LaunchAuthorization', 'AssemblyIssuer', 'SessionStartHandoff'
    )) {
        if ($returnType.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "host-command-router method '$Method' return type exposed '$forbidden'"
        }
    }
    $body = $match.Groups['body'].Value
    Require-Text $body @('self.installed_host_mut()?', $HostCall) "host-command-router method '$Method'"
}

function Assert-Boundary {
    param(
        [object]$Manifest,
        [string]$Slot,
        [string]$HostSource,
        [string]$Ux,
        [string]$RuntimeLibrary,
        [string]$Execution,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.56' -or
        [string]$Manifest.status -cne 'opaque-installed-host-command-router-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.55') {
        throw 'host-command-router identity changed'
    }
    if ([string]$Manifest.router.owner_type -cne 'RecipientImportGuidedRecoveryLaunchSlot') {
        throw 'host-command-router owner type changed'
    }
    Require-TrueProperties $Manifest.router @(
        'installed_host_remains_process_owned', 'typed_prepare', 'typed_confirm',
        'typed_apply', 'typed_cancel', 'typed_refresh', 'requires_host_installed',
        'rejects_empty_before_host_work',
        'rejects_pending_authorization_before_host_work',
        'returns_existing_safe_ux_view_only'
    ) 'host-command-router'
    Require-FalseProperties $Manifest.router @(
        'returns_raw_host', 'returns_installation_owner',
        'returns_authorization_or_issuer', 'returns_session_authority_or_handoff',
        'returns_identity_or_membership'
    ) 'host-command-router'
    Require-TrueProperties $Manifest.failure @(
        'host_errors_remain_typed', 'installed_host_retained_after_routed_error'
    ) 'host-command-router failure'
    Require-FalseProperties $Manifest.failure @(
        'replacement_after_routed_error', 'host_extraction_after_routed_error',
        'host_restart_recovery', 'slot_cloneable', 'slot_serializable'
    ) 'host-command-router failure'
    Require-FalseProperties $Manifest.production @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_slot', 'ordinary_startup_routes_host_commands',
        'changes_public_default'
    ) 'host-command-router production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'host-command-router public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 18 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '16/16' -or
        [string]$Manifest.verification.readiness_gates -cne '182/182') {
        throw 'host-command-router verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'full_host_lifecycle_routed', 'all_commands_reject_empty',
        'all_commands_reject_pending_authorization',
        'host_retained_after_routed_error', 'installed_host_replacement_rejected'
    ) 'host-command-router verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'host-command-router verification'

    Require-Text $Slot @(
        "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>",
        'pub fn host_prepare(', 'pub fn host_confirm(', 'pub fn host_apply(',
        'pub fn host_cancel(', 'pub fn host_refresh(',
        'fn installed_host_mut(',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable',
        'HostUnavailable,'
    ) 'host-command-router slot source'
    if (([regex]::Matches($Slot, 'self\.installed_host_mut\(\)\?')).Count -ne 5) {
        throw 'host-command-router does not route exactly five commands through the common gate'
    }
    Assert-RoutedMethod $Slot 'host_prepare' 'RecipientImportGuidedRecoveryHostPrepare' '.prepare(command, now_unix_seconds)'
    Assert-RoutedMethod $Slot 'host_confirm' 'RecipientImportGuidedSessionLocalUxConfirm' '.confirm(command, now_unix_seconds)'
    Assert-RoutedMethod $Slot 'host_apply' 'RecipientImportGuidedSessionLocalUxApply' '.apply(command, now_unix_seconds)'
    Assert-RoutedMethod $Slot 'host_cancel' 'RecipientImportGuidedRecoveryHostCancel' '.cancel(command, now_unix_seconds)'
    Assert-RoutedMethod $Slot 'host_refresh' 'RecipientImportGuidedSessionLocalUxRefresh' '.refresh(command, now_unix_seconds)'

    $gate = [regex]::Match($Slot, '(?s)fn installed_host_mut\(.*?\r?\n    \}')
    if (-not $gate.Success) {
        throw 'host-command-router common installed-host gate is missing'
    }
    Require-Text $gate.Value @(
        'if self.pending_authorization.is_some()',
        'return Err(RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable);',
        'self.installed_host', '.as_mut()',
        '.ok_or(RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable)'
    ) 'host-command-router common gate'
    $pendingOffset = $gate.Value.IndexOf('if self.pending_authorization.is_some()', [StringComparison]::Ordinal)
    $hostOffset = $gate.Value.IndexOf('self.installed_host', [StringComparison]::Ordinal)
    if ($pendingOffset -lt 0 -or $hostOffset -lt 0 -or $pendingOffset -gt $hostOffset) {
        throw 'host-command-router can borrow host before rejecting pending authorization'
    }
    foreach ($forbidden in @(
        'pub fn installed_host_mut(', 'pub fn host(&', 'pub fn installed_host(',
        'pub fn take_host(', 'pub fn into_host(', 'use serde::',
        '#[derive(Serialize', '#[derive(Deserialize',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "host-command-router slot source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Slot,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )) {
        throw 'host-command-router slot became derive-enabled'
    }

    Require-Text $HostSource @(
        'pub fn prepare(', 'pub fn confirm(', 'pub fn apply(',
        'pub fn cancel(', 'pub fn refresh(',
        'RecipientImportGuidedSessionLocalUxView'
    ) 'existing guided recovery host'
    Require-Text $Ux @(
        'pub struct RecipientImportGuidedSessionLocalUxView',
        'contains no state path, Account/Device identifier, descriptor',
        'pub fn phase(&self)', 'pub fn session_id(&self)'
    ) 'existing guided recovery safe UX view'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_slot;',
        'RecipientImportGuidedRecoveryLaunchSlot',
        'RecipientImportGuidedRecoveryLaunchSlotError'
    ) 'runtime host-command-router export'
    Require-Text $Execution @(
        'recipient_import_guided_recovery_launch_slot_router_runs_one_full_host_lifecycle',
        'recipient_import_guided_recovery_launch_slot_router_rejects_preinstall_and_retains_host',
        'unavailable(slot.host_prepare', 'unavailable(slot.host_confirm',
        'unavailable(slot.host_apply', 'unavailable(slot.host_cancel',
        'unavailable(slot.host_refresh',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled'
    ) 'host-command-router real-storage fixtures'
    foreach ($productSource in @($Cli, $Desktop)) {
        if ($productSource.IndexOf('RecipientImportGuidedRecoveryLaunchSlot', [StringComparison]::Ordinal) -ge 0 -or
            $productSource.IndexOf('host_prepare(', [StringComparison]::Ordinal) -ge 0) {
            throw 'host-command-router became reachable from an active product surface'
        }
    }
    Require-Text $Rfc @(
        'Route commands, never the host',
        'Installed-host state is mandatory',
        'Failure keeps custody opaque',
        'Dormant product boundary',
        'public sync default remains `legacy-only`'
    ) 'RFC-0280'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try {
        & $Action
    }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$slot = Get-Content -LiteralPath $slotPath -Raw
$hostSource = Get-Content -LiteralPath $hostPath -Raw
$ux = Get-Content -LiteralPath $uxPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Boundary $manifest $slot $hostSource $ux $runtimeLibrary $execution $cli $desktop $rfc

$manifestMutations = @(
    [PSCustomObject]@{ Label = 'requires_host_installed'; Mutate = { param($m) $m.router.requires_host_installed = $false } },
    [PSCustomObject]@{ Label = 'rejects_empty'; Mutate = { param($m) $m.router.rejects_empty_before_host_work = $false } },
    [PSCustomObject]@{ Label = 'rejects_pending'; Mutate = { param($m) $m.router.rejects_pending_authorization_before_host_work = $false } },
    [PSCustomObject]@{ Label = 'safe_view_only'; Mutate = { param($m) $m.router.returns_existing_safe_ux_view_only = $false } },
    [PSCustomObject]@{ Label = 'returns_raw_host'; Mutate = { param($m) $m.router.returns_raw_host = $true } },
    [PSCustomObject]@{ Label = 'retains_host_after_error'; Mutate = { param($m) $m.failure.installed_host_retained_after_routed_error = $false } },
    [PSCustomObject]@{ Label = 'replacement_after_error'; Mutate = { param($m) $m.failure.replacement_after_routed_error = $true } },
    [PSCustomObject]@{ Label = 'restart_recovers_host'; Mutate = { param($m) $m.failure.host_restart_recovery = $true } },
    [PSCustomObject]@{ Label = 'runtime_ipc_registration'; Mutate = { param($m) $m.production.registered_runtime_ipc_command = $true } }
)
$rejected = 0
foreach ($mutation in $manifestMutations) {
    $copy = Copy-JsonObject $manifest
    $mutator = $mutation.Mutate
    & $mutator $copy
    Assert-Rejected {
        Assert-Boundary $copy $slot $hostSource $ux $runtimeLibrary $execution $cli $desktop $rfc
    } "mutation_$($mutation.Label)"
    $rejected++
}

$sourceMutations = @(
    [PSCustomObject]@{ Label = 'common_gate_removed'; Target = 'slot'; From = 'fn installed_host_mut('; To = 'fn removed_installed_host_mut(' },
    [PSCustomObject]@{ Label = 'prepare_route_removed'; Target = 'slot'; From = 'pub fn host_prepare('; To = 'pub fn removed_host_prepare(' },
    [PSCustomObject]@{ Label = 'safe_return_removed'; Target = 'slot'; From = 'RecipientImportGuidedSessionLocalUxView'; To = "RecipientImportGuidedRecoveryHost<'identity>" },
    [PSCustomObject]@{ Label = 'host_extraction_added'; Target = 'slot_append'; From = ''; To = "`r`npub fn take_host() {}`r`n" },
    [PSCustomObject]@{ Label = 'clone_added'; Target = 'slot'; From = "pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"; To = "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryLaunchSlot<'identity>" },
    [PSCustomObject]@{ Label = 'fixture_removed'; Target = 'execution'; From = 'recipient_import_guided_recovery_launch_slot_router_runs_one_full_host_lifecycle'; To = 'removed_router_lifecycle_fixture' },
    [PSCustomObject]@{ Label = 'cli_activation_added'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryLaunchSlot`r`n" }
)
foreach ($mutation in $sourceMutations) {
    $mutatedSlot = $slot
    $mutatedExecution = $execution
    $mutatedCli = $cli
    switch ($mutation.Target) {
        'slot' {
            $mutatedSlot = $slot.Replace([string]$mutation.From, [string]$mutation.To)
            if ($mutatedSlot -ceq $slot) { throw "mutation source was not found: $($mutation.Label)" }
        }
        'slot_append' { $mutatedSlot = $slot + [string]$mutation.To }
        'execution' {
            $mutatedExecution = $execution.Replace([string]$mutation.From, [string]$mutation.To)
            if ($mutatedExecution -ceq $execution) { throw "mutation source was not found: $($mutation.Label)" }
        }
        'cli_append' { $mutatedCli = $cli + [string]$mutation.To }
        default { throw "unknown mutation target: $($mutation.Target)" }
    }
    Assert-Rejected {
        Assert-Boundary $manifest $mutatedSlot $hostSource $ux $runtimeLibrary $mutatedExecution $mutatedCli $desktop $rfc
    } "mutation_$($mutation.Label)"
    $rejected++
}

if ($rejected -ne 16) {
    throw "host-command-router boundary rejected $rejected/16 negative mutations"
}
if ($SelfTest) {
    Write-Output 'recipient_import_guided_recovery_host_command_router_self_test=passed'
}
Write-Output 'recipient_import_guided_recovery_host_command_router_boundary=verified'
Write-Output 'negative_mutations_rejected=16/16'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
