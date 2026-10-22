[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-ASSEMBLY-ISSUER.json'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$hostPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_host.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$runtimeSourceDirectory = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0277-m1253-authenticated-one-shot-recovery-assembly-issuer.md'

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
        [object]$Manifest, [string]$Controller, [string]$HostSource,
        [string]$RuntimeLibrary, [string]$OtherRuntimeSources,
        [string]$Execution, [string]$Cli, [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.53' -or
        [string]$Manifest.status -cne 'authenticated-one-shot-recovery-assembly-issuer-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.52') {
        throw 'recovery-assembly issuer identity changed'
    }

    if ([string]$Manifest.issuer.type -cne 'RecipientImportGuidedRecoveryAssemblyIssuer') {
        throw 'recovery-assembly issuer type changed'
    }
    foreach ($property in @(
        'process_local', 'crate_private_authenticated_input_constructor',
        'issue_consumes_self', 'input_extracted_before_fallible_validation',
        'emits_only_installation_owner'
    )) {
        if (-not [bool]$Manifest.issuer.$property) {
            throw "recovery-assembly issuer invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'public_constructor', 'accepts_ambient_filesystem_state', 'cloneable',
        'serializable', 'debuggable', 'returns_raw_session', 'returns_host',
        'returns_handoff', 'returns_controller', 'returns_memberships',
        'replayable_after_success', 'replayable_after_failure',
        'restart_recovers_issuer'
    )) {
        if ([bool]$Manifest.issuer.$property) {
            throw "recovery-assembly issuer unsafe capability '$property' enabled"
        }
    }

    foreach ($property in @(
        'contains_state_directory', 'contains_owner_account',
        'contains_owner_device', 'contains_exact_local_signer',
        'contains_signed_plan', 'contains_exact_coordinator',
        'contains_ordered_loaded_memberships'
    )) {
        if (-not [bool]$Manifest.authenticated_input.$property) {
            throw "authenticated issuer input invariant '$property' weakened"
        }
    }
    foreach ($property in @('public_type', 'cloneable', 'serializable', 'debuggable')) {
        if ([bool]$Manifest.authenticated_input.$property) {
            throw "authenticated issuer input unsafe capability '$property' enabled"
        }
    }

    foreach ($property in @(
        'session_constructor_private', 'production_assembly_path_is_issuer_only',
        'test_fixture_explicitly_cfg_test', 'rechecks_state_directory',
        'rechecks_owner_account', 'rechecks_owner_device',
        'rechecks_exact_signer', 'revalidates_signed_plan',
        'rechecks_coordinator_plan_and_sequence',
        'rechecks_ordered_membership_references', 'rechecks_membership_digests'
    )) {
        if (-not [bool]$Manifest.validation.$property) {
            throw "recovery-assembly validation invariant '$property' weakened"
        }
    }

    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_issuer', 'ordinary_startup_creates_owner',
        'ordinary_startup_creates_host', 'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "recovery-assembly issuer production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 12 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.readiness_gates -cne '179/179' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        -not [bool]$Manifest.verification.successful_owner_emission_tested -or
        -not [bool]$Manifest.verification.foreign_signer_rejected -or
        -not [bool]$Manifest.verification.foreign_state_rejected -or
        -not [bool]$Manifest.verification.changed_membership_set_rejected -or
        -not [bool]$Manifest.verification.issuer_consumed_by_issue -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recovery-assembly issuer verification/product boundary changed'
    }

    Require-Text $Controller @(
        "pub(crate) struct RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'state_directory: PathBuf', 'owner_account_id: AccountId',
        'owner_device_id: DeviceId', "signer: &'identity DeviceIdentity",
        'descriptor: RuntimeIpcDescriptor',
        'plan_request: RuntimeIpcRecipientImportExecutionRequest',
        'coordination: RecipientRecoveryCoordinationPlan',
        'memberships: Vec<GuidedRecipientImportMembership>',
        "pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        "input: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>",
        'pub(crate) fn from_authenticated_plan_assembly(',
        'pub(crate) fn into_issuer(self)',
        '#[cfg(test)]',
        'pub(crate) fn from_authenticated_plan_assembly_fixture(',
        'pub fn issue(', 'self,',
        'let RecipientImportGuidedRecoveryAssemblyStagingInput {',
        'AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(',
        'let installation = authority.into_host_installation(signer)?;',
        'RecipientImportGuidedRecoveryHostInstallationOwner::from_authenticated_recovery(',
        '    fn from_authenticated_recovery(',
        '#[cfg(test)]',
        'pub(crate) fn authenticated_guided_recipient_import_session_fixture(',
        'RuntimeIpcRecipientImportExecutionRequest::decode_and_validate(',
        'coordinator_plan_id.to_bytes()? != self.coordination.plan_id()',
        'requested_memberships != &self.references',
        'recipient_import_membership_digest(membership)'
    ) 'recovery-assembly issuer source'
    foreach ($forbidden in @(
        'pub fn from_authenticated_plan_assembly(',
        'pub fn from_authenticated_recovery(',
        'pub(crate) fn from_authenticated_recovery(',
        'pub struct RecipientImportGuidedRecoveryAssemblyStagingInput',
        'TcpListener', 'UdpSocket', 'iroh::Endpoint', 'std::fs::read_dir'
    )) {
        if ($Controller.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recovery-assembly issuer source contains forbidden '$forbidden'"
        }
    }
    foreach ($type in @(
        'RecipientImportGuidedRecoveryAssemblyStagingInput',
        'RecipientImportGuidedRecoveryAssemblyIssuer'
    )) {
        if ([regex]::IsMatch(
            $Controller,
            "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*(?:pub )?struct $type"
        )) {
            throw "$type gained a derive attribute"
        }
    }
    if (-not [regex]::IsMatch(
        $Controller,
        '(?s)pub struct RecipientImportGuidedRecoveryAssemblyIssuer<''identity>.*?pub fn issue\(\s*self,'
    )) {
        throw 'recovery-assembly issuer issue no longer consumes self'
    }
    if (-not [regex]::IsMatch(
        $Controller,
        '(?s)#\[cfg\(test\)\].{0,160}pub\(crate\) fn authenticated_guided_recipient_import_session_fixture\('
    )) {
        throw 'loose session fixture is no longer explicitly test-only'
    }
    if ([regex]::Matches(
        $Controller,
        'AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery\('
    ).Count -ne 2) {
        throw 'private loose session assembly is no longer limited to issuer plus test fixture'
    }
    $extractOffset = $Controller.IndexOf(
        'let RecipientImportGuidedRecoveryAssemblyStagingInput {',
        [StringComparison]::Ordinal
    )
    $validateOffset = $Controller.LastIndexOf(
        'let authority = AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(',
        [StringComparison]::Ordinal
    )
    if ($extractOffset -lt 0 -or $validateOffset -lt 0 -or $extractOffset -gt $validateOffset) {
        throw 'issuer no longer consumes its input before fallible assembly validation'
    }
    if ($OtherRuntimeSources.IndexOf(
        'AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(',
        [StringComparison]::Ordinal
    ) -ge 0 -or $Execution.IndexOf(
        'AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(',
        [StringComparison]::Ordinal
    ) -ge 0) {
        throw 'loose session assembly escaped the issuer/test fixture source boundary'
    }

    Require-Text $HostSource @(
        "pub struct RecipientImportGuidedRecoveryHostInstallationOwner<'identity>",
        'pub(crate) fn from_authenticated_recovery(', 'pub fn install(', 'self,'
    ) 'installation-owner predecessor'
    Require-Text $RuntimeLibrary @(
        'RecipientImportGuidedRecoveryAssemblyIssuer',
        'RecipientImportGuidedRecoveryHostInstallationOwner'
    ) 'runtime issuer export'
    Require-Text $Execution @(
        'recipient_import_guided_recovery_assembly_issuer_emits_one_installation_owner',
        'recipient_import_guided_recovery_assembly_issuer_is_one_shot_and_fail_closed',
        'guided_recovery_assembly_issuer(', '.issue()?',
        'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch',
        'RecipientImportGuidedSessionControllerError::StateDirectoryMismatch',
        'RecipientImportGuidedSessionControllerError::MembershipSetChanged'
    ) 'issuer real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf(
            'RecipientImportGuidedRecoveryAssemblyIssuer',
            [StringComparison]::Ordinal
        ) -ge 0) {
            throw 'ordinary CLI/desktop startup registered the recovery-assembly issuer'
        }
    }

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'entire authenticated input',
        'issuer has no public constructor',
        '`issue(self)` destructures and consumes',
        'complete canonical verification',
        'emits only one',
        'constructor is now private',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0277 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $controllerPath, $hostPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$controller = Get-Content -LiteralPath $controllerPath -Raw
$hostSource = Get-Content -LiteralPath $hostPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$otherRuntimeSources = @(
    Get-ChildItem -LiteralPath $runtimeSourceDirectory -Filter '*.rs' -File |
        Where-Object FullName -cne $controllerPath |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }
) -join "`n"
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.issuer.public_constructor = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.issuer.issue_consumes_self = $false
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.issuer.emits_only_installation_owner = $false
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.issuer.returns_raw_session = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.issuer.replayable_after_failure = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authenticated_input.cloneable = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.validation.session_constructor_private = $false
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.validation.rechecks_exact_signer = $false
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.registered_cli_command = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.ordinary_startup_creates_issuer = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'auto'
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.network_executed = $true
    $mutations += ({ Assert-Boundary $changed $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = $controller.Replace(
        'pub(crate) fn from_authenticated_plan_assembly(',
        'pub fn from_authenticated_plan_assembly('
    )
    $mutations += ({ Assert-Boundary $manifest $changedController $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = $controller.Replace(
        '    fn from_authenticated_recovery(',
        '    pub(crate) fn from_authenticated_recovery('
    )
    $mutations += ({ Assert-Boundary $manifest $changedController $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = $controller.Replace(
        "pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity>"
    )
    $mutations += ({ Assert-Boundary $manifest $changedController $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changedController = [regex]::Replace(
        $controller,
        '(?s)(pub fn issue\(\s+)self,',
        '${1}&mut self,',
        1
    )
    $mutations += ({ Assert-Boundary $manifest $changedController $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc }.GetNewClosure())
    $changedCli = $cli + "`r`nRecipientImportGuidedRecoveryAssemblyIssuer::from_authenticated_plan_assembly"
    $mutations += ({ Assert-Boundary $manifest $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $changedCli $desktop $rfc }.GetNewClosure())

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
        throw "recovery-assembly issuer boundary rejected $rejected/$($mutations.Count) negative mutations"
    }
    Write-Output "negative_mutations=$rejected/$($mutations.Count)"
}

Assert-Boundary $manifest $controller $hostSource $runtimeLibrary $otherRuntimeSources $execution $cli $desktop $rfc
Write-Output 'recipient_import_guided_recovery_assembly_issuer_boundary=verified'
Write-Output 'issuer=one-shot-authenticated-full-assembly'
Write-Output 'issuer_output=installation-owner-only'
Write-Output 'session_constructor=private'
Write-Output 'ordinary_startup=no-issuer-no-owner-no-host'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
