[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-ASSEMBLY-TO-SLOT-STAGING.json'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$slotPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_slot.rs'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_authorization.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0282-m1258-atomic-authenticated-assembly-to-slot-staging.md'

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
        [object]$Manifest, [string]$Controller, [string]$Slot,
        [string]$Authorization, [string]$RuntimeLibrary,
        [string]$OtherRuntimeSources, [string]$Execution,
        [string]$Cli, [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.58' -or
        [string]$Manifest.status -cne 'atomic-authenticated-assembly-to-slot-staging-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.57') {
        throw 'assembly-to-slot identity changed'
    }
    if ([string]$Manifest.staging_input.type -cne 'RecipientImportGuidedRecoveryAssemblyStagingInput') {
        throw 'assembly-to-slot input type changed'
    }
    Require-TrueProperties $Manifest.staging_input @(
        'process_local', 'crate_private_type', 'crate_private_constructor',
        'contains_exact_authenticated_assembly', 'opaque_fields'
    ) 'assembly-to-slot input'
    Require-FalseProperties $Manifest.staging_input @(
        'cloneable', 'serializable', 'debuggable', 'public_constructor',
        'returns_authority_fragments', 'restart_recovers_input'
    ) 'assembly-to-slot input'
    Require-TrueProperties $Manifest.transition @(
        'accepts_staging_input_by_value', 'constructs_issuer_inside_slot',
        'constructs_authorization_inside_slot', 'validates_before_retaining',
        'returns_safe_state_only'
    ) 'assembly-to-slot transition'
    Require-FalseProperties $Manifest.transition @(
        'production_caller_constructs_ready_issuer', 'accepts_loose_authority_fragments',
        'returns_staging_input', 'returns_issuer', 'returns_authorization',
        'returns_owner_host_session_handoff_identity_or_membership'
    ) 'assembly-to-slot transition'
    Require-TrueProperties $Manifest.failure @(
        'invalid_lifetime_consumes_input_issuer_and_grant',
        'clock_regression_consumes_input_issuer_and_grant',
        'expiry_consumes_input_issuer_and_grant',
        'pending_replacement_consumes_incoming_input',
        'installed_host_replacement_consumes_incoming_input',
        'existing_custody_preserved'
    ) 'assembly-to-slot failure'
    Require-FalseProperties $Manifest.failure @(
        'returns_rejected_authority', 'silently_replaces_or_evicts_custody'
    ) 'assembly-to-slot failure'
    Require-TrueProperties $Manifest.predecessor_compatibility @(
        'issuer_fixture_explicitly_cfg_test', 'issuer_still_one_shot',
        'authorization_still_created_once_inside_slot'
    ) 'assembly-to-slot predecessor'
    Require-FalseProperties $Manifest.predecessor_compatibility @(
        'issuer_constructor_available_to_production_callers', 'slot_cloneable',
        'slot_serializable', 'restart_recovers_authority'
    ) 'assembly-to-slot predecessor'
    Require-FalseProperties $Manifest.production @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_slot', 'ordinary_startup_stages_assembly',
        'changes_public_default'
    ) 'assembly-to-slot production'
    if ([string]$Manifest.production.public_default -cne 'legacy-only') {
        throw 'assembly-to-slot public default changed'
    }
    if ([int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 22 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.negative_mutations_rejected -cne '18/18' -or
        [string]$Manifest.verification.readiness_gates -cne '184/184') {
        throw 'assembly-to-slot verification evidence changed'
    }
    Require-TrueProperties $Manifest.verification @(
        'format_verified', 'offline_strict_clippy_verified',
        'atomic_success_verified', 'invalid_lifetime_rejected',
        'pending_replacement_rejected', 'installed_host_replacement_rejected',
        'existing_custody_preserved'
    ) 'assembly-to-slot verification'
    Require-FalseProperties $Manifest.verification @(
        'network_executed', 'second_host_required', 'release_build',
        'archive_created', 'git_tag_created', 'automatic_github_build'
    ) 'assembly-to-slot verification'

    Require-Text $Controller @(
        "pub(crate) struct RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'state_directory: PathBuf', 'owner_account_id: AccountId',
        'owner_device_id: DeviceId', "signer: &'identity DeviceIdentity",
        'descriptor: RuntimeIpcDescriptor',
        'plan_request: RuntimeIpcRecipientImportExecutionRequest',
        'coordination: RecipientRecoveryCoordinationPlan',
        'memberships: Vec<GuidedRecipientImportMembership>',
        "impl<'identity> RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'pub(crate) fn from_authenticated_plan_assembly(',
        'pub(crate) fn into_issuer(self)',
        'RecipientImportGuidedRecoveryAssemblyIssuer { input: self }',
        '#[cfg(test)]',
        'pub(crate) fn from_authenticated_plan_assembly_fixture(',
        "pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        "input: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'pub fn issue(', 'self,'
    ) 'assembly-to-slot controller source'
    foreach ($forbidden in @(
        'pub struct RecipientImportGuidedRecoveryAssemblyStagingInput',
        'pub fn from_authenticated_plan_assembly(',
        'RecipientImportGuidedRecoveryAssemblyIssuer::from_authenticated_plan_assembly(',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint',
        'std::fs::read_dir'
    )) {
        if ($Controller.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "assembly-to-slot controller contains forbidden '$forbidden'"
        }
    }
    foreach ($type in @(
        'RecipientImportGuidedRecoveryAssemblyStagingInput',
        'RecipientImportGuidedRecoveryAssemblyIssuer'
    )) {
        if ([regex]::IsMatch(
            $Controller,
            "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*(?:pub(?:\(crate\))? )?struct $type"
        )) { throw "$type gained a derive attribute" }
    }
    if (-not [regex]::IsMatch(
        $Controller,
        '(?s)#\[cfg\(test\)\].{0,240}pub\(crate\) fn from_authenticated_plan_assembly_fixture\('
    )) { throw 'caller-owned issuer fixture is not explicitly test-only' }
    if (([regex]::Matches($Controller, '\.into_issuer\(\)')).Count -ne 1) {
        throw 'controller has an unexpected non-slot issuer conversion'
    }

    Require-Text $Slot @(
        "pub(crate) struct RecipientImportGuidedRecoveryLaunchSlot<'identity>",
        'pub(crate) fn stage_from_authenticated_assembly(',
        "assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'if self.pending_authorization.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled',
        'if self.installed_host.is_some()',
        'RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled',
        'let issuer = assembly.into_issuer();',
        'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer(',
        'authorization.validate_pending_at(now_unix_seconds)?;',
        'self.pending_authorization = Some(authorization);',
        'Ok(RecipientImportGuidedRecoveryLaunchSlotState::PendingAuthorization)'
    ) 'assembly-to-slot slot source'
    foreach ($forbidden in @(
        'pub fn stage_from_authenticated_assembly(',
        'pub(crate) fn stage_from_authenticated_issuer(',
        'pub fn stage_from_authenticated_issuer(',
        'use serde::', '#[derive(Serialize', '#[derive(Deserialize',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($Slot.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "assembly-to-slot slot contains forbidden '$forbidden'"
        }
    }
    if (([regex]::Matches($Slot, '\.into_issuer\(\)')).Count -ne 1 -or
        ([regex]::Matches(
            $Slot,
            'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer\('
        )).Count -ne 1) {
        throw 'assembly-to-slot conversion is not unique inside the slot'
    }
    $stageStart = $Slot.IndexOf(
        'pub(crate) fn stage_from_authenticated_assembly(',
        [StringComparison]::Ordinal
    )
    $stageEnd = $Slot.IndexOf('pub fn state(&self)', $stageStart, [StringComparison]::Ordinal)
    if ($stageStart -lt 0 -or $stageEnd -lt 0) { throw 'assembly-to-slot method is missing' }
    $stage = $Slot.Substring($stageStart, $stageEnd - $stageStart)
    $pending = $stage.IndexOf('if self.pending_authorization.is_some()', [StringComparison]::Ordinal)
    $hostCheck = $stage.IndexOf('if self.installed_host.is_some()', [StringComparison]::Ordinal)
    $issue = $stage.IndexOf('let issuer = assembly.into_issuer();', [StringComparison]::Ordinal)
    $authorize = $stage.IndexOf(
        'RecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer(',
        [StringComparison]::Ordinal
    )
    $validate = $stage.IndexOf('authorization.validate_pending_at(now_unix_seconds)?;', [StringComparison]::Ordinal)
    $retain = $stage.IndexOf('self.pending_authorization = Some(authorization);', [StringComparison]::Ordinal)
    if ($pending -lt 0 -or $hostCheck -lt 0 -or $issue -lt 0 -or $authorize -lt 0 -or
        $validate -lt 0 -or $retain -lt 0 -or $pending -gt $issue -or
        $hostCheck -gt $issue -or $issue -gt $authorize -or $authorize -gt $validate -or
        $validate -gt $retain) {
        throw 'assembly-to-slot ownership, conversion, validation, and retention are out of order'
    }
    $header = $stage.Substring(0, $stage.IndexOf('{', [StringComparison]::Ordinal))
    foreach ($forbidden in @(
        'RecipientImportGuidedRecoveryAssemblyIssuer',
        'RecipientImportGuidedRecoveryLaunchAuthorization,',
        'InstallationOwner', "RecoveryHost<'", 'SessionStartHandoff'
    )) {
        if ($header.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "assembly-to-slot signature exposed '$forbidden'"
        }
    }

    Require-Text $Authorization @(
        'pub(crate) fn from_authenticated_issuer(',
        'pub(crate) fn validate_pending_at('
    ) 'assembly-to-slot authorization predecessor'
    Require-Text $RuntimeLibrary @(
        'pub(crate) use recipient_import_guided_session_controller::RecipientImportGuidedRecoveryAssemblyStagingInput;',
        'RecipientImportGuidedRecoveryLaunchSlot',
        'RecipientImportGuidedRecoveryAssemblyIssuer'
    ) 'assembly-to-slot runtime library'
    if ($OtherRuntimeSources.IndexOf('.into_issuer()', [StringComparison]::Ordinal) -ge 0) {
        throw 'assembly-to-slot issuer conversion escaped its controller fixture and slot'
    }
    Require-Text $Execution @(
        'recipient_import_guided_recovery_assembly_to_slot_staging_is_atomic_and_safe',
        'recipient_import_guided_recovery_assembly_to_slot_staging_consumes_failures_and_preserves_custody',
        'guided_recovery_assembly_staging_input(',
        'stage_from_authenticated_assembly(',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime',
        'RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled'
    ) 'assembly-to-slot real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        foreach ($needle in @(
            'RecipientImportGuidedRecoveryAssemblyStagingInput',
            'stage_from_authenticated_assembly',
            'RecipientImportGuidedRecoveryLaunchSlot'
        )) {
            if ($surface.IndexOf($needle, [StringComparison]::Ordinal) -ge 0) {
                throw 'assembly-to-slot staging became reachable from an active product surface'
            }
        }
    }
    Require-Text $Rfc @(
        'Atomic authenticated-assembly-to-slot staging',
        'never owns a ready issuer or authorization',
        'failure returns authority',
        'Dormant product boundary',
        'public sync default remains `legacy-only`'
    ) 'RFC-0282'
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "mutation_$Label=rejected"; return }
    throw "mutation '$Label' was unexpectedly accepted"
}

foreach ($path in @(
    $manifestPath, $controllerPath, $slotPath, $authorizationPath,
    $runtimeLibraryPath, $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$controller = Get-Content -LiteralPath $controllerPath -Raw
$slot = Get-Content -LiteralPath $slotPath -Raw
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$otherRuntimeSources = @(
    Get-ChildItem -LiteralPath $runtimeSourceDirectory -Filter '*.rs' -File |
        Where-Object { $_.FullName -cne $controllerPath -and $_.FullName -cne $slotPath } |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Boundary $manifest $controller $slot $authorization $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc

if ($SelfTest) {
    $rejected = 0
    $manifestMutations = @(
        @{ Section = 'staging_input'; Property = 'crate_private_type'; Value = $false },
        @{ Section = 'staging_input'; Property = 'cloneable'; Value = $true },
        @{ Section = 'transition'; Property = 'accepts_staging_input_by_value'; Value = $false },
        @{ Section = 'transition'; Property = 'constructs_issuer_inside_slot'; Value = $false },
        @{ Section = 'transition'; Property = 'production_caller_constructs_ready_issuer'; Value = $true },
        @{ Section = 'failure'; Property = 'invalid_lifetime_consumes_input_issuer_and_grant'; Value = $false },
        @{ Section = 'failure'; Property = 'silently_replaces_or_evicts_custody'; Value = $true },
        @{ Section = 'production'; Property = 'registered_cli_command'; Value = $true },
        @{ Section = 'production'; Property = 'public_default'; Value = 'auto' },
        @{ Section = 'verification'; Property = 'network_executed'; Value = $true }
    )
    foreach ($mutation in $manifestMutations) {
        $changed = Copy-JsonObject $manifest
        $changed.($mutation.Section).($mutation.Property) = $mutation.Value
        Assert-Rejected {
            Assert-Boundary $changed $controller $slot $authorization $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc
        } "$($mutation.Section)_$($mutation.Property)"
        $rejected++
    }

    $sourceMutations = @(
        @{ Label = 'public_input'; Target = 'controller'; From = 'pub(crate) struct RecipientImportGuidedRecoveryAssemblyStagingInput'; To = 'pub struct RecipientImportGuidedRecoveryAssemblyStagingInput' },
        @{ Label = 'public_constructor'; Target = 'controller'; From = 'pub(crate) fn from_authenticated_plan_assembly('; To = 'pub fn from_authenticated_plan_assembly(' },
        @{ Label = 'issuer_conversion_removed'; Target = 'slot'; From = 'let issuer = assembly.into_issuer();'; To = 'let issuer = removed_issuer_conversion();' },
        @{ Label = 'public_stage'; Target = 'slot'; From = 'pub(crate) fn stage_from_authenticated_assembly('; To = 'pub fn stage_from_authenticated_assembly(' },
        @{ Label = 'issuer_input_reopened'; Target = 'slot'; From = "assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>"; To = "issuer: RecipientImportGuidedRecoveryAssemblyIssuer<'identity>" },
        @{ Label = 'validation_removed'; Target = 'slot'; From = 'authorization.validate_pending_at(now_unix_seconds)?;'; To = 'removed_pending_validation();' },
        @{ Label = 'success_fixture_removed'; Target = 'execution'; From = 'recipient_import_guided_recovery_assembly_to_slot_staging_is_atomic_and_safe'; To = 'removed_atomic_assembly_fixture' },
        @{ Label = 'cli_activation'; Target = 'cli_append'; From = ''; To = "`r`nRecipientImportGuidedRecoveryLaunchSlot::stage_from_authenticated_assembly`r`n" }
    )
    foreach ($mutation in $sourceMutations) {
        $changedController = $controller
        $changedSlot = $slot
        $changedExecution = $execution
        $changedCli = $cli
        switch ($mutation.Target) {
            'controller' { $changedController = $controller.Replace($mutation.From, $mutation.To) }
            'slot' { $changedSlot = $slot.Replace($mutation.From, $mutation.To) }
            'execution' { $changedExecution = $execution.Replace($mutation.From, $mutation.To) }
            'cli_append' { $changedCli = $cli + $mutation.To }
        }
        Assert-Rejected {
            Assert-Boundary $manifest $changedController $changedSlot $authorization $runtimeLibrary $otherRuntimeSources $changedExecution $changedCli $desktop $rfc
        } $mutation.Label
        $rejected++
    }
    if ($rejected -ne 18) { throw "assembly-to-slot boundary rejected $rejected/18 mutations" }
    Write-Output 'recipient_import_guided_recovery_assembly_to_slot_staging_self_test=passed'
}

Write-Output 'recipient_import_guided_recovery_assembly_to_slot_staging_boundary=verified'
Write-Output 'negative_mutations_rejected=18/18'
Write-Output 'staging=authenticated-assembly-by-value-issuer-created-inside-slot'
Write-Output 'caller_ready_issuer=false'
Write-Output 'runtime_ipc_registered=false'
Write-Output 'cli_registered=false'
Write-Output 'desktop_registered=false'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
