[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-GUIDED-SESSION-START-PRODUCER.json'
$producerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_start_producer.rs'
$controllerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_controller.rs'
$uxPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_guided_session_local_ux.rs'
$runtimeLibraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0274-m1250-authenticated-one-shot-guided-session-start-producer.md'

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
        [object]$Manifest, [string]$Producer, [string]$Controller,
        [string]$Ux, [string]$RuntimeLibrary, [string]$Execution,
        [string]$Cli, [string]$Desktop, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.50' -or
        [string]$Manifest.status -cne 'authenticated-one-shot-guided-session-start-producer-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.49') {
        throw 'guided-session start-producer identity changed'
    }
    foreach ($property in @(
        'process_local', 'contains_exact_authenticated_session_authority',
        'consumed_before_fallible_preflight', 'success_consumes_authority',
        'failure_consumes_authority', 'expiry_consumes_authority',
        'cancel_consumes_authority'
    )) {
        if (-not [bool]$Manifest.handoff.$property) {
            throw "start hand-off invariant '$property' weakened"
        }
    }
    foreach ($property in @(
        'public_constructor', 'serializable', 'cloneable', 'debuggable',
        'restart_recovers_authority'
    )) {
        if ([bool]$Manifest.handoff.$property) {
            throw "start hand-off unsafe capability '$property' enabled"
        }
    }
    if ([int]$Manifest.handoff.maximum_lifetime_seconds -ne 300 -or
        -not [bool]$Manifest.producer.borrows_exact_controller -or
        [int]$Manifest.producer.maximum_start_commands_per_handoff -ne 1 -or
        [bool]$Manifest.producer.session_may_outlive_handoff -or
        [bool]$Manifest.producer.opens_network) {
        throw 'start hand-off/producer cardinality or lifetime changed'
    }
    foreach ($property in @(
        'accepts_state_path', 'accepts_signer', 'accepts_descriptor',
        'accepts_plan_request', 'accepts_coordinator',
        'accepts_membership_reference', 'accepts_membership_snapshot'
    )) {
        if ([bool]$Manifest.producer.$property) {
            throw "start producer accepts forbidden loose input '$property'"
        }
    }
    foreach ($property in @(
        'state_directory_binding', 'account_binding', 'device_binding',
        'descriptor_signature_binding', 'plan_request_binding',
        'active_intent_binding', 'approved_source_binding',
        'coordinator_plan_binding', 'coordinator_sequence_binding',
        'ordered_membership_set_binding', 'each_membership_digest_binding',
        'controller_idle_binding', 'runtime_actor_unattached_binding',
        'canonical_read_adapter_replayed'
    )) {
        if (-not [bool]$Manifest.preflight.$property) {
            throw "start producer preflight '$property' weakened"
        }
    }
    if ([string]$Manifest.output.type -cne 'RecipientImportGuidedSessionLocalUxStart' -or
        [bool]$Manifest.output.cloneable -or
        [bool]$Manifest.output.serializable -or
        [bool]$Manifest.output.debuggable -or
        [bool]$Manifest.output.public_constructor -or
        [bool]$Manifest.output.contains_authority_fragments_as_public_fields) {
        throw 'start-producer output boundary changed'
    }
    foreach ($property in @(
        'registered_runtime_ipc_command', 'registered_cli_command',
        'registered_desktop_control', 'registered_timer',
        'registered_background_start', 'adds_filesystem_discovery',
        'changes_peer_wire', 'changes_runtime_ipc_wire',
        'changes_public_default'
    )) {
        if ([bool]$Manifest.production.$property) {
            throw "start producer production scope '$property' became active"
        }
    }
    if ([string]$Manifest.production.public_default -cne 'legacy-only' -or
        [int]$Manifest.verification.new_stable_path_test_count -ne 2 -or
        [int]$Manifest.verification.combined_guided_session_test_count -ne 6 -or
        [string]$Manifest.verification.readiness_gates -cne '176/176' -or
        -not [bool]$Manifest.verification.format_verified -or
        -not [bool]$Manifest.verification.offline_strict_clippy_verified -or
        [bool]$Manifest.verification.network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'start-producer verification/product boundary changed'
    }

    Require-Text $Producer @(
        'pub struct AuthenticatedGuidedRecipientImportSessionStartHandoff',
        'authority: Option<AuthenticatedGuidedRecipientImportSession>',
        'pub(crate) fn from_authenticated_recovery(',
        'pub fn consumed(&self) -> bool',
        'pub fn cancel(&mut self) -> bool',
        'pub struct RecipientImportGuidedSessionStartProducer',
        "controller: &'controller mut RecipientImportGuidedSessionController<'identity>",
        'pub fn produce(',
        '.authority', '.take()',
        'RecipientImportGuidedSessionStartProducerError::HandoffConsumed',
        'RecipientImportGuidedSessionStartProducerError::HandoffExpired',
        'RecipientImportGuidedSessionStartProducerError::SessionOutlivesHandoff',
        'self.controller.preflight_local_ux_start(',
        'RecipientImportGuidedSessionLocalUxStart::from_authenticated_recovery(',
        'pub fn opened_network(&self) -> bool', 'false'
    ) 'guided-session start producer source'
    foreach ($forbidden in @(
        'pub fn from_authenticated_recovery(',
        'use serde::', 'TcpListener', 'UdpSocket', 'iroh::Endpoint',
        'std::fs::read_dir', 'RuntimeIpcDescriptor', 'DeviceIdentity',
        'ConversationMembershipSnapshot'
    )) {
        if ($Producer.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "start producer contains forbidden '$forbidden'"
        }
    }
    $handoffOffset = $Producer.IndexOf(
        'pub struct AuthenticatedGuidedRecipientImportSessionStartHandoff',
        [StringComparison]::Ordinal
    )
    $handoffPrefixStart = [Math]::Max(0, $handoffOffset - 120)
    $handoffPrefix = $Producer.Substring($handoffPrefixStart, $handoffOffset - $handoffPrefixStart)
    if ($handoffPrefix.IndexOf('#[derive', [StringComparison]::Ordinal) -ge 0) {
        throw 'start hand-off gained a derive attribute'
    }

    Require-Text $Controller @(
        'fn verify_authenticated_recovery(',
        'RuntimeIpcRecipientImportExecutionRequest::decode_and_validate(',
        'recipient_import_membership_set_digest(&self.references)?',
        'recipient_import_membership_digest(membership)',
        'pub(crate) fn preflight_local_ux_start(',
        'fn prepare_start(',
        'authority.verify_controller_binding(',
        'authority.verify_authenticated_recovery()?',
        'RecipientHistoryRecoveryImportReadAdapter::dispatch(',
        '.planned_summary()',
        'SessionAlreadyActive', 'ActorStateChanged'
    ) 'guided-session controller preflight'
    Require-Text $Ux @(
        'pub struct RecipientImportGuidedSessionLocalUxStart',
        'pub(crate) fn from_authenticated_recovery('
    ) 'local UX start command'
    if ($Execution.IndexOf(
        'RecipientImportGuidedSessionLocalUxStart::from_authenticated_recovery(',
        [StringComparison]::Ordinal
    ) -ge 0) {
        throw 'execution fixtures bypass the one-shot start producer'
    }
    Require-Text $Execution @(
        'recipient_import_guided_session_start_producer_is_one_shot',
        'recipient_import_guided_session_start_producer_consumes_failed_authority',
        'RecipientImportGuidedSessionStartProducerError::HandoffConsumed',
        'RecipientImportGuidedSessionStartProducerError::HandoffExpired',
        'RecipientImportGuidedSessionStartProducerError::SessionOutlivesHandoff',
        'RecipientImportGuidedSessionControllerError::StateDirectoryMismatch',
        'RecipientImportGuidedSessionControllerError::OwnerAccountMismatch',
        'RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch',
        'RecipientImportGuidedSessionControllerError::SessionAlreadyActive'
    ) 'guided-session start-producer fixtures'
    Require-Text $RuntimeLibrary @(
        'mod recipient_import_guided_session_start_producer;',
        'AuthenticatedGuidedRecipientImportSessionStartHandoff',
        'RecipientImportGuidedSessionStartProducer'
    ) 'runtime start-producer export'
    foreach ($surface in @($Cli, $Desktop)) {
        if ($surface.IndexOf('RecipientImportGuidedSessionStartProducer', [StringComparison]::Ordinal) -ge 0 -or
            $surface.IndexOf('AuthenticatedGuidedRecipientImportSessionStartHandoff', [StringComparison]::Ordinal) -ge 0) {
            throw 'ordinary CLI/desktop startup registered guided-session start authority'
        }
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'no public constructor and no Clone, Debug, Serialize, or Deserialize implementation',
        'takes authority out of the hand-off before checking time',
        'Replaying the same value cannot produce a second start command',
        'reruns the canonical read adapter against the current active intent',
        'A failed production attempt never leaves reusable authority',
        'public sync default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0274 is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $producerPath, $controllerPath, $uxPath,
    $runtimeLibraryPath, $executionPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$producer = Get-Content -LiteralPath $producerPath -Raw
$controller = Get-Content -LiteralPath $controllerPath -Raw
$ux = Get-Content -LiteralPath $uxPath -Raw
$runtimeLibrary = Get-Content -LiteralPath $runtimeLibraryPath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.handoff.public_constructor = $true
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handoff.cloneable = $true
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handoff.consumed_before_fallible_preflight = $false
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handoff.failure_consumes_authority = $false
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.producer.accepts_state_path = $true
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.producer.maximum_start_commands_per_handoff = 2
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.producer.session_may_outlive_handoff = $true
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.preflight.active_intent_binding = $false
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.preflight.each_membership_digest_binding = $false
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.registered_desktop_control = $true
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.production.public_default = 'enabled'
    $mutations += { Assert-Boundary $changed $producer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedProducer = $producer.Replace(
        'pub(crate) fn from_authenticated_recovery(',
        'pub fn from_authenticated_recovery('
    )
    $mutations += { Assert-Boundary $manifest $changedProducer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedProducer = $producer.Replace(
        'pub struct AuthenticatedGuidedRecipientImportSessionStartHandoff',
        "#[derive(Clone)]`r`npub struct AuthenticatedGuidedRecipientImportSessionStartHandoff"
    )
    $mutations += { Assert-Boundary $manifest $changedProducer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedProducer = $producer.Replace('.take()', '.as_ref()')
    $mutations += { Assert-Boundary $manifest $changedProducer $controller $ux $runtimeLibrary $execution $cli $desktop $rfc }
    $changedExecution = $execution + "`r`nRecipientImportGuidedSessionLocalUxStart::from_authenticated_recovery("
    $mutations += { Assert-Boundary $manifest $producer $controller $ux $runtimeLibrary $changedExecution $cli $desktop $rfc }
    $changedCli = $cli + "`r`nRecipientImportGuidedSessionStartProducer::new"
    $mutations += { Assert-Boundary $manifest $producer $controller $ux $runtimeLibrary $execution $changedCli $desktop $rfc }
    $changedRfc = $rfc.Replace('`legacy-only`', '`enabled`')
    $mutations += { Assert-Boundary $manifest $producer $controller $ux $runtimeLibrary $execution $cli $desktop $changedRfc }
    $rejected = 0
    foreach ($case in $mutations) {
        try {
            & $case
            throw 'mutation was accepted'
        }
        catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 17) { throw "expected 17 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=17/17'
}

Write-Output 'recipient_import_guided_session_start_producer_boundary=verified'
Write-Output 'handoff_lifetime_seconds_max=300'
Write-Output 'start_commands_per_handoff=1'
Write-Output 'failed_attempt_consumes_authority=true'
Write-Output 'ordinary_startup=idle-unattached'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
