[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-RECOVERY-LAUNCH-AUTHORIZATION.json'
$authorizationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_recovery_launch_authorization.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0278-m1254-explicit-local-guided-recovery-launch-authorization.md'

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
        [object]$Manifest, [string]$Authorization, [string]$Controller,
        [string]$RuntimeLibrary, [string]$Execution, [string]$Cli,
        [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.54' -or
        [string]$Manifest.status -cne 'explicit-local-one-shot-launch-authorization-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.53') {
        throw 'launch-authorization identity changed'
    }

    if ([string]$Manifest.authorization.type -cne 'RecipientImportGuidedRecoveryLaunchAuthorization' -or
        [int]$Manifest.authorization.maximum_lifetime_seconds -ne 300) {
        throw 'launch-authorization type or maximum lifetime changed'
    }
    foreach ($property in @(
        'process_local', 'holds_exactly_one_issuer',
        'crate_private_authenticated_issuer_constructor', 'short_lived',
        'approve_consumes_self', 'cancel_consumes_self',
        'consumed_before_fallible_approval', 'returns_only_installation_owner'
    )) {
        if (-not [bool]$Manifest.authorization.$property) {
            throw "launch-authorization invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'public_constructor', 'cloneable', 'serializable', 'debuggable',
        'returns_issuer', 'returns_raw_session', 'returns_host',
        'replayable_after_success', 'replayable_after_cancel',
        'replayable_after_expiry', 'replayable_after_mismatch',
        'replayable_after_failure', 'restart_recovers_authorization'
    )) {
        if ([bool]$Manifest.authorization.$property) {
            throw "launch-authorization unsafe capability '$property' enabled"
        }
    }

    foreach ($property in @(
        'membership_count_only', 'expires_at_only',
        'domain_bound_confirmation_phrase_only'
    )) {
        if (-not [bool]$Manifest.preview.$property) {
            throw "launch preview invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'contains_state_directory', 'contains_account_id', 'contains_device_id',
        'contains_descriptor', 'contains_signer', 'contains_coordinator',
        'contains_membership_snapshot', 'contains_request_or_nonce',
        'contains_authority'
    )) {
        if ([bool]$Manifest.preview.$property) {
            throw "launch preview leaked '$property'"
        }
    }

    foreach ($property in @(
        'typed_command', 'exact_phrase_required', 'clock_regression_rejected',
        'expiry_rejected', 'mismatch_rejected', 'issuer_revalidated_on_success'
    )) {
        if (-not [bool]$Manifest.approval.$property) {
            throw "launch approval invariant '$property' weakened"
        }
    }

    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'ordinary_startup_creates_authorization',
        'ordinary_startup_approves_authorization',
        'ordinary_startup_creates_owner', 'ordinary_startup_creates_host',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "launch-authorization production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [double]$Manifest.verification.new_stable_path_test_seconds -le 0 -or
        [int]$Manifest.verification.combined_guided_recovery_test_count -ne 14 -or
        [double]$Manifest.verification.combined_guided_recovery_test_seconds -le 0 -or
        [string]$Manifest.verification.readiness_gates -cne '180/180' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        -not [bool]$Manifest.verification.exact_approval_success_tested -or
        -not [bool]$Manifest.verification.invalid_lifetime_rejected -or
        -not [bool]$Manifest.verification.clock_regression_rejected -or
        -not [bool]$Manifest.verification.expiry_rejected -or
        -not [bool]$Manifest.verification.phrase_mismatch_rejected -or
        -not [bool]$Manifest.verification.cancel_consumes_authorization -or
        -not [bool]$Manifest.verification.downstream_failure_consumes_authorization -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'launch-authorization verification/product boundary changed'
    }

    Require-Text $Authorization @(
        "pub struct RecipientImportGuidedRecoveryLaunchAuthorization<'identity>",
        "issuer: RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        'preview: RecipientImportGuidedRecoveryLaunchPreview',
        'issued_at_unix_seconds: u64',
        'pub(crate) fn from_authenticated_issuer(',
        "issuer: RecipientImportGuidedRecoveryAssemblyIssuer<'identity>",
        'pub fn preview(&self)', 'pub fn approve(', 'self,',
        'approval: RecipientImportGuidedRecoveryLaunchApproval',
        'let Self {', 'issuer,', 'preview,', 'issued_at_unix_seconds,',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ConfirmationMismatch',
        'issuer.issue().map_err(Into::into)', 'pub fn cancel(self) {}',
        'pub struct RecipientImportGuidedRecoveryLaunchPreview',
        'membership_count: usize', 'expires_at_unix_seconds: u64',
        'confirmation_phrase: String',
        'pub struct RecipientImportGuidedRecoveryLaunchApproval',
        'const LAUNCH_CONFIRMATION_DOMAIN',
        'MAX_GUIDED_RECIPIENT_IMPORT_LAUNCH_AUTHORIZATION_LIFETIME_SECONDS: u64 = 5 * 60'
    ) 'launch-authorization source'
    foreach ($forbidden in @(
        'pub fn from_authenticated_issuer(', 'use serde::',
        '#[derive(Serialize', '#[derive(Deserialize', 'impl Serialize',
        'impl Deserialize', 'TcpListener', 'UdpSocket', 'iroh::Endpoint',
        'std::fs::read_dir'
    )) {
        if ($Authorization.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "launch-authorization source contains forbidden '$forbidden'"
        }
    }
    if ([regex]::IsMatch(
        $Authorization,
        "(?m)^[ \t]*#\[derive[^\]]*\][ \t]*\r?\n[ \t]*pub struct RecipientImportGuidedRecoveryLaunchAuthorization"
    )) {
        throw 'launch authorization gained a derive attribute'
    }
    if (-not [regex]::IsMatch(
        $Authorization,
        '(?s)pub struct RecipientImportGuidedRecoveryLaunchAuthorization<''identity>.*?pub fn approve\(\s*self,'
    )) {
        throw 'launch authorization approval no longer consumes self'
    }
    $extractOffset = $Authorization.IndexOf('let Self {', [StringComparison]::Ordinal)
    $clockOffset = $Authorization.IndexOf('if now_unix_seconds < issued_at_unix_seconds', [StringComparison]::Ordinal)
    if ($extractOffset -lt 0 -or $clockOffset -lt 0 -or $extractOffset -gt $clockOffset) {
        throw 'launch authorization is no longer consumed before fallible approval checks'
    }
    $previewMatch = [regex]::Match(
        $Authorization,
        '(?s)pub struct RecipientImportGuidedRecoveryLaunchPreview \{(?<body>.*?)\r?\n\}'
    )
    if (-not $previewMatch.Success) {
        throw 'launch preview structure is unavailable'
    }
    foreach ($forbidden in @(
        'Path', 'Account', 'Device', 'Descriptor', 'signer', 'coordinator',
        'membership_snapshot', 'request', 'nonce', 'authority', 'issuer'
    )) {
        if ($previewMatch.Groups['body'].Value.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "launch preview source leaked '$forbidden'"
        }
    }

    Require-Text $Controller @(
        'const RECOVERY_LAUNCH_BINDING_DOMAIN',
        'pub(crate) struct AuthenticatedGuidedRecoveryLaunchBinding',
        'assembly_digest: [u8; 32]', 'membership_count: usize',
        'membership_count_u64: u64',
        'pub(crate) fn launch_binding(', '&self,',
        'let encoded_plan_request = self.input.plan_request.encode()?;',
        'hasher.update(&self.input.coordination.plan_id());',
        'hasher.update(&self.input.coordination.sequence().to_le_bytes());'
    ) 'issuer launch binding'
    foreach ($forbidden in @(
        'pub struct AuthenticatedGuidedRecoveryLaunchBinding',
        'pub fn launch_binding('
    )) {
        if ($Controller.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "issuer launch binding exposed '$forbidden'"
        }
    }

    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_recovery_launch_authorization;',
        'RecipientImportGuidedRecoveryLaunchAuthorization',
        'RecipientImportGuidedRecoveryLaunchApproval',
        'RecipientImportGuidedRecoveryLaunchPreview'
    ) 'runtime launch-authorization export'
    if ($RuntimeLibrary.IndexOf(
        'AuthenticatedGuidedRecoveryLaunchBinding',
        [StringComparison]::Ordinal
    ) -ge 0) {
        throw 'opaque launch binding was publicly re-exported'
    }
    Require-Text $Execution @(
        'recipient_import_guided_recovery_launch_authorization_requires_explicit_exact_approval',
        'recipient_import_guided_recovery_launch_authorization_is_one_shot_and_fail_closed',
        'RecipientImportGuidedRecoveryLaunchApproval::new(phrase)',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired',
        'RecipientImportGuidedRecoveryLaunchAuthorizationError::ConfirmationMismatch',
        '.cancel();', 'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch'
    ) 'launch-authorization real-storage fixtures'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf(
            'RecipientImportGuidedRecoveryLaunchAuthorization',
            [StringComparison]::Ordinal
        ) -ge 0) {
            throw 'ordinary CLI/desktop startup registered launch authorization'
        }
    }

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'owns exactly one M1.2.53 authenticated assembly issuer',
        'hard maximum lifetime of 300 seconds',
        'safe preview', 'approve(self, typed_approval, now)',
        'destructures and consumes the authorization before checking',
        'cancel(self)', 'no reusable grant',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0278 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $authorizationPath, $controllerPath, $runtimeLibraryPath,
    $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$authorization = Get-Content -LiteralPath $authorizationPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.authorization.public_constructor = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authorization.cloneable = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authorization.approve_consumes_self = $false
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authorization.replayable_after_expiry = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.preview.contains_account_id = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.approval.exact_phrase_required = $false
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.registered_cli_command = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.ordinary_startup_approves_authorization = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'auto'
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.network_executed = $true
    $mutations += ({ Assert-Boundary $changed $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedAuthorization = $authorization.Replace(
        'pub(crate) fn from_authenticated_issuer(',
        'pub fn from_authenticated_issuer('
    )
    $mutations += ({ Assert-Boundary $manifest $changedAuthorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedAuthorization = [regex]::Replace(
        $authorization,
        '(?s)(pub fn approve\(\s+)self,',
        '${1}&mut self,',
        1
    )
    $mutations += ({ Assert-Boundary $manifest $changedAuthorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedAuthorization = $authorization.Replace(
        "pub struct RecipientImportGuidedRecoveryLaunchAuthorization<'identity>",
        "#[derive(Clone)]`r`npub struct RecipientImportGuidedRecoveryLaunchAuthorization<'identity>"
    )
    $mutations += ({ Assert-Boundary $manifest $changedAuthorization $controller $runtimeLibrary $execution $cli $desktop $rfc }.GetNewClosure())
    $changedCli = $cli + "`r`nRecipientImportGuidedRecoveryLaunchAuthorization::from_authenticated_issuer"
    $mutations += ({ Assert-Boundary $manifest $authorization $controller $runtimeLibrary $execution $changedCli $desktop $rfc }.GetNewClosure())

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
        throw "launch-authorization boundary rejected $rejected/$($mutations.Count) negative mutations"
    }
    Write-Output "negative_mutations=$rejected/$($mutations.Count)"
}

Assert-Boundary $manifest $authorization $controller $runtimeLibrary $execution $cli $desktop $rfc
Write-Output 'recipient_import_guided_recovery_launch_authorization_boundary=verified'
Write-Output 'authorization=explicit-local-one-shot'
Write-Output 'preview=safe-domain-bound-phrase'
Write-Output 'approval=typed-exact-consume-before-checks'
Write-Output 'ordinary_startup=no-authorization-no-approval-no-owner-no-host'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
