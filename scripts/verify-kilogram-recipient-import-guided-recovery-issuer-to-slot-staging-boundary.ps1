[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-ISSUER-TO-SLOT-STAGING.json'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_authorization.rs'
$issuerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0281-m1257-atomic-issuer-to-slot-launch-staging.md'

foreach ($path in @(
    $manifestPath, $slotPath, $authorizationPath, $issuerPath,
    $runtimeLibraryPath, $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "issuer-to-slot staging boundary file is missing: $path"
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

function Require-TrueProperties {
    param([object]$Value, [string[]]$Properties, [string]$Label)
    foreach ($property in $Properties) {
        if (-not [bool]$Value.$property) {
            throw "$Label invariant '$property' became false"
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

function Assert-Boundary {
    param(
        [object]$Manifest,
        [string]$Slot,
        [string]$Authorization,
        [string]$Issuer,
        [string]$RuntimeLibrary,
        [string]$Execution,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.57' -or
        [string]$Manifest.status -cne 'atomic-issuer-to-slot-staging-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.56') {
        throw 'issuer-to-slot staging identity changed'
    }
    if ([string]$Manifest.staging.owner_type -cne 'RecipientImportGuidedRecoveryLaunchSlot' -or
        [string]$Manifest.staging.input_type -cne 'RecipientImportGuidedRecoveryAssemblyIssuer') {
        throw 'issuer-to-slot staging types changed'
    }
    Require-TrueProperties $Manifest.staging @(
        'crate_private_transition', 'accepts_issuer_by_value',
        'constructs_authorization_inside_slot', 'validates_before_retaining',
        'returns_safe_state_only'
    ) 'issuer-to-slot staging'
    Require-FalseProperties $Manifest.staging @(
        'accepts_prebuilt_authorization', 'returns_issuer', 'returns_authorization',
        'returns_installation_owner_or_host',
        'returns_session_handoff_identity_or_membership'
    ) 'issuer-to-slot staging'
    Require-TrueProperties $Manifest.failure @(
        'invalid_lifetime_consumes_incoming_issuer',
        'clock_regression_consumes_incoming_grant',
        'expiry_consumes_incoming_grant',
        'pending_replacement_consumes_incoming_issuer',
        'installed_host_replacement_consumes_incoming_issuer',
        'pending_custody_retained_after_rejected_replacement',
        'installed_host_custody_retained_after_rejected_replacement'
    ) 'issuer-to-slot failure'
    Require-FalseProperties $Manifest.failure @(
        'returns_rejected_authority', 'silently_replaces_or_evicts_custody',
        'slot_cloneable', 'slot_serializable', 'restart_recovers_authority'
    ) 'issuer-to-slot failure'
    Require-FalseProperties $Manifest.production @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_slot', 'ordinary_startup_stages_issuer',
        'changes_public_default'
    ) 'issuer-to-slot production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'issuer-to-slot public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 20 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '183/183') {
        throw 'issuer-to-slot verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'atomic_success_verified', 'invalid_lifetime_rejected',
        'clock_regression_rejected', 'expired_admission_rejected',
        'pending_replacement_rejected', 'installed_host_replacement_rejected',
        'existing_custody_preserved'
    ) 'issuer-to-slot verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'issuer-to-slot verification'

    Require-Text $Slot @(
        "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        "pending_authorization: Option<RecipientImportGuidedRecoveryLaunchAuthorization<'identity>>",
        "installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>",
        'pub(crate) fn stage_from_authenticated_assembly(',
        "assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'issued_at_unix_seconds: u64', 'lifetime_seconds: u64',
        'if self.pending_authorization.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'if self.installed_host.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled',
        'let issuer = assembly.into_issuer();',
        'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer(',
        'authorization.validate_pending_at(now_unix_seconds)?;',
        'self.pending_authorization = Some(authorization);',
        'Ok(RecipientImportGuidedRecoveryLaunchSlotState::PendingAuthorization)'
    ) 'issuer-to-slot source'
    foreach ($forbidden in @(
        'pub fn stage_from_authenticated_assembly(',
        'pub(crate) fn stage_from_authenticated_issuer(', 'pub(crate) fn install(',
        'pub fn install(', 'use serde::', '#[derive(Serialize',
        '#[derive(Deserialize', 'TcpListener', 'UdpSocket', 'iroh::Endpoint',
        'std::fs::read_dir'
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "issuer-to-slot source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Slot,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub\(crate\) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"
    )) {
        throw 'issuer-to-slot launch slot gained a derive attribute'
    }
    if (([regex]::Matches(
        $Slot,
        'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer\('
    )).Count -ne 1) {
        throw 'issuer-to-slot authorization is not constructed exactly once inside the slot'
    }
    $stageStart = $Slot.IndexOf('pub(crate) fn stage_from_authenticated_assembly(', [StringComparison]::Ordinal)
    $stageEnd = $Slot.IndexOf('pub fn state(&self)', $stageStart, [StringComparison]::Ordinal)
    if ($stageStart -lt 0 -or $stageEnd -lt 0) {
        throw 'issuer-to-slot staging method boundary is missing'
    }
    $stage = $Slot.Substring($stageStart, $stageEnd - $stageStart)
    foreach ($forbidden in @(
        'RecipientImportGuidedRecoveryLaunchAuthorization,',
        "RecipientImportGuidedRecoveryHost<'", 'InstallationOwner',
        'SessionStartHandoff'
    )) {
        $headerEnd = $stage.IndexOf('{', [StringComparison]::Ordinal)
        if ($headerEnd -gt 0 -and
            $stage.Substring(0, $headerEnd).IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "issuer-to-slot signature exposed '$forbidden'"
        }
    }
    $pendingCheck = $stage.IndexOf('if self.pending_authorization.is_some()', [StringComparison]::Ordinal)
    $hostCheck = $stage.IndexOf('if self.installed_host.is_some()', [StringComparison]::Ordinal)
    $issue = $stage.IndexOf('let issuer = assembly.into_issuer();', [StringComparison]::Ordinal)
    $construct = $stage.IndexOf('RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer(', [StringComparison]::Ordinal)
    $validate = $stage.IndexOf('authorization.validate_pending_at(now_unix_seconds)?;', [StringComparison]::Ordinal)
    $retain = $stage.IndexOf('self.pending_authorization = Some(authorization);', [StringComparison]::Ordinal)
    if ($pendingCheck -lt 0 -or $hostCheck -lt 0 -or $issue -lt 0 -or $construct -lt 0 -or
        $validate -lt 0 -or $retain -lt 0 -or
        $pendingCheck -gt $issue -or $hostCheck -gt $issue -or $issue -gt $construct -or
        $construct -gt $validate -or $validate -gt $retain) {
        throw 'issuer-to-slot ownership checks, construction, validation, and retention are out of order'
    }

    Require-Text $Authorization @(
        'pub(crate) fn from_authenticated_issuer(',
        'pub(crate) fn validate_pending_at(',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired'
    ) 'issuer-to-slot authorization'
    Require-Text $Issuer @(
        "pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        'pub(crate) fn launch_binding(', 'pub fn issue(', 'self,'
    ) 'issuer-to-slot issuer'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_slot;',
        'RecipientImportGuidedRecoveryLaunchSlot',
        'RecipientImportGuidedRecoveryAssemblyIssuer',
        'RecipientImportGuidedRecoveryAssemblyStagingInput'
    ) 'issuer-to-slot runtime library'
    Require-Text $Execution @(
        'recipient_import_guided_recovery_issuer_to_slot_staging_is_atomic_and_safe',
        'recipient_import_guided_recovery_issuer_to_slot_staging_consumes_failures_and_preserves_custody',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled'
    ) 'issuer-to-slot real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf('RecipientImportGuidedRecoveryLaunchSlot', [StringComparison]::Ordinal) -ge 0 -or
            $surface.IndexOf('stage_from_authenticated_assembly', [StringComparison]::Ordinal) -ge 0 -or
            $surface.IndexOf('stage_from_authenticated_issuer', [StringComparison]::Ordinal) -ge 0) {
            throw 'issuer-to-slot staging became reachable from an active product surface'
        }
    }
    Require-Text $Rfc @(
        'Atomic issuer-to-slot launch staging',
        'constructs the M1.2.54 authorization inside the empty slot',
        'No rejection returns authority',
        'Dormant product boundary',
        'public sync default remains `legacy-only`'
    ) 'RFC-0281'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$slot = Get-Content -LiteralPath $slotPath -Raw
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
$issuer = Get-Content -LiteralPath $issuerPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$runtimeSource = @(
    Get-ChildItem -LiteralPath $runtimeSourceDirectory -Filter '*.rs' -File |
        Sort-Object FullName |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"

if (([regex]::Matches(
    $runtimeSource,
    'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer\('
)).Count -ne 1) {
    throw 'ready launch authorization is constructed outside the sole slot staging transition'
}

Assert-Boundary $manifest $slot $authorization $issuer $runtimeLibrary $execution $cli $desktop $rfc

$manifestMutations = @(
    [PSCustomObject]@{ Label = 'crate_private'; Mutate = { param($m) $m.staging.crate_private_transition = $false } },
    [PSCustomObject]@{ Label = 'internal_construction'; Mutate = { param($m) $m.staging.constructs_authorization_inside_slot = $false } },
    [PSCustomObject]@{ Label = 'prebuilt_authorization'; Mutate = { param($m) $m.staging.accepts_prebuilt_authorization = $true } },
    [PSCustomObject]@{ Label = 'returns_authorization'; Mutate = { param($m) $m.staging.returns_authorization = $true } },
    [PSCustomObject]@{ Label = 'invalid_lifetime_replay'; Mutate = { param($m) $m.failure.invalid_lifetime_consumes_incoming_issuer = $false } },
    [PSCustomObject]@{ Label = 'pending_custody'; Mutate = { param($m) $m.failure.pending_custody_retained_after_rejected_replacement = $false } },
    [PSCustomObject]@{ Label = 'silent_replacement'; Mutate = { param($m) $m.failure.silently_replaces_or_evicts_custody = $true } },
    [PSCustomObject]@{ Label = 'runtime_ipc'; Mutate = { param($m) $m.production.registered_runtime_ipc_command = $true } },
    [PSCustomObject]@{ Label = 'ordinary_startup'; Mutate = { param($m) $m.production.ordinary_startup_stages_issuer = $true } },
    [PSCustomObject]@{ Label = 'public_default'; Mutate = { param($m) $m.production.public_default = 'auto' } }
)
$rejected = 0
foreach ($mutation in $manifestMutations) {
    $copy = Copy-JsonObject $manifest
    & $mutation.Mutate $copy
    Assert-Rejected {
        Assert-Boundary $copy $slot $authorization $issuer $runtimeLibrary $execution $cli $desktop $rfc
    } "mutation_$($mutation.Label)"
    $rejected++
}

$sourceMutations = @(
    [PSCustomObject]@{ Label = 'public_stage'; Target = 'slot'; From = 'pub(crate) fn stage_from_authenticated_assembly('; To = 'pub fn stage_from_authenticated_assembly(' },
    [PSCustomObject]@{ Label = 'constructor_removed'; Target = 'slot'; From = 'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer('; To = 'removed_internal_authorization_constructor(' },
    [PSCustomObject]@{ Label = 'validation_removed'; Target = 'slot'; From = 'authorization.validate_pending_at(now_unix_seconds)?;'; To = 'removed_pending_validation();' },
    [PSCustomObject]@{ Label = 'prebuilt_install_added'; Target = 'slot_append'; From = ''; To = "`r`npub(crate) fn install() {}`r`n" },
    [PSCustomObject]@{ Label = 'clone_added'; Target = 'slot'; From = "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>"; To = "#[derive(Clone)]`r`npub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>" },
    [PSCustomObject]@{ Label = 'success_fixture_removed'; Target = 'execution'; From = 'recipient_import_guided_recovery_issuer_to_slot_staging_is_atomic_and_safe'; To = 'removed_atomic_staging_fixture' },
    [PSCustomObject]@{ Label = 'failure_fixture_removed'; Target = 'execution'; From = 'recipient_import_guided_recovery_issuer_to_slot_staging_consumes_failures_and_preserves_custody'; To = 'removed_staging_failure_fixture' },
    [PSCustomObject]@{ Label = 'cli_activation'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryLaunchSlot::stage_from_authenticated_assembly`r`n" }
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
        Assert-Boundary $manifest $mutatedSlot $authorization $issuer $runtimeLibrary $mutatedExecution $mutatedCli $desktop $rfc
    } "mutation_$($mutation.Label)"
    $rejected++
}

if ($rejected -ne 18) {
    throw "issuer-to-slot staging boundary rejected $rejected/18 negative mutations"
}
if ($SelfTest) {
    Write-Output 'recipient_import_guided_recovery_issuer_to_slot_staging_self_test=passed'
}
Write-Output 'recipient_import_guided_recovery_issuer_to_slot_staging_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'staging=issuer-by-value-authorization-created-inside-slot'
Write-Output 'failure=incoming-authority-consumed-existing-custody-preserved'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
