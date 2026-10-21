[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-IROH-SESSION.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-INBOX.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$workspaceCargoPath = Join-Path $workspace 'Cargo.toml'
$transportCargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\Cargo.toml'
$transportPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\lib.rs'
$fixturePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\tests\fixtures\m1222-transfer-envelope.b64'
$executionCargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\Cargo.toml'
$activeTransportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0247-m1223-authenticated-iroh-history-transfer-session.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-OmitsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "$Label unexpectedly contains '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Predecessor,
        [object]$Completion,
        [object]$Compatibility,
        [string]$WorkspaceCargo,
        [string]$TransportCargo,
        [string]$Transport,
        [string]$Fixture,
        [string]$ExecutionCargo,
        [string]$ActiveTransport,
        [string]$CliCargo,
        [string]$Cli,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.23' -or
        [string]$Manifest.status -cne 'authenticated-iroh-history-transfer-session-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.22' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'history recovery Iroh session identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.22' -or
        [string]$Predecessor.status -cne 'authenticated-history-recovery-transfer-inbox-verified') {
        throw 'history recovery Iroh session predecessor changed'
    }

    $session = $Manifest.session
    if ([string]$session.alpn -cne 'kilogram/m1/history-recovery/postcard/1' -or
        [int]$session.request_version -ne 1 -or
        [int]$session.response_version -ne 1 -or
        [string]$session.encoding -cne 'canonical-postcard' -or
        [int]$session.io_timeout_seconds -ne 180 -or
        [int64]$session.max_envelope_bytes -ne 9437184 -or
        [int64]$session.max_request_bytes -ne 9441280 -or
        [int64]$session.max_response_bytes -ne 4096 -or
        -not [bool]$session.single_bidirectional_stream -or
        [bool]$session.fallback_protocol) {
        throw 'history recovery Iroh framing boundary changed'
    }

    foreach ($required in @(
        'source_device_signed', 'authenticated_source_endpoint_bound',
        'exact_recipient_endpoint_bound', 'nonzero_session_nonce_bound',
        'exact_transfer_id_bound', 'complete_m1222_envelope_bound',
        'noncanonical_trailing_and_oversized_rejected'
    )) {
        if (-not [bool]$Manifest.request.$required) { throw "request property '$required' changed" }
    }
    foreach ($required in @(
        'recipient_device_signed', 'exact_request_id_bound', 'both_endpoint_ids_bound',
        'transfer_id_bound', 'recipient_device_id_bound', 'stage_disposition_bound',
        'staging_precedes_response'
    )) {
        if (-not [bool]$Manifest.response.$required) { throw "response property '$required' changed" }
    }
    if (-not [bool]$Manifest.retry.exact_request_replay_idempotent -or
        [string]$Manifest.retry.first_result -cne 'staged' -or
        [string]$Manifest.retry.exact_replay_result -cne 'already-staged' -or
        [bool]$Manifest.retry.restart_safe_source_outbox -or
        [bool]$Manifest.retry.automatic_scheduler -or
        [bool]$Manifest.retry.multi_source_scheduling) {
        throw 'history recovery Iroh retry boundary changed'
    }

    foreach ($forbidden in @(
        'transport_crate_registered_by_active_runtime', 'runtime_accepts_history_recovery_alpn',
        'cli_command_added', 'desktop_control_added', 'recipient_inbox_grants_import_authority',
        'message_bodies_in_plaintext', 'execution_crate_gains_network_dependency'
    )) {
        if ([bool]$Manifest.authority.$forbidden) { throw "authority non-claim '$forbidden' changed" }
    }

    if ([int]$Manifest.verification.focused_tests -ne 4 -or
        [int]$Manifest.verification.preexisting_iroh_loopback_control_tests -ne 1 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        -not [bool]$Manifest.verification.local_loopback_network_executed -or
        [bool]$Manifest.verification.external_network_executed -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.second_host_required -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '149/149' -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'history recovery Iroh verification boundary changed'
    }
    foreach ($nonClaim in @(
        'active-runtime-history-transfer', 'restart-safe-source-network-outbox',
        'automatic-network-resume', 'multi-source-recovery-scheduling',
        'runtime-ipc-history-recovery-authority', 'desktop-history-recovery-authority',
        'product-internet-or-relay-evidence',
        'whole-database-rollback-detection-without-external-witness'
    )) {
        if ($nonClaim -notin @($Manifest.non_claims)) { throw "required non-claim '$nonClaim' is missing" }
    }

    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38) {
        throw 'active runtime IPC inventory changed'
    }
    $runtimeSurface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([bool]$Compatibility.changes_wire -or
        $runtimeSurface.Count -ne 1 -or
        [int](@($runtimeSurface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'compatibility inventory changed'
    }

    Assert-ContainsAll $WorkspaceCargo @(
        '"crates/kilogram-peer-history-recovery-transport"'
    ) 'workspace Cargo manifest'
    Assert-ContainsAll $TransportCargo @(
        'name = "kilogram-peer-history-recovery-transport"',
        'iroh.workspace = true', 'tokio.workspace = true',
        'kilogram-peer-history-recovery-execution'
    ) 'history recovery transport manifest'
    Assert-OmitsAll $TransportCargo @('reqwest', 'eframe', 'kilogram-runtime-ipc') 'history recovery transport manifest'
    $productionExecutionCargo = ($ExecutionCargo -split '(?m)^\[dev-dependencies\]\s*$', 2)[0]
    Assert-OmitsAll $productionExecutionCargo @('iroh', 'tokio', 'reqwest', 'eframe', 'kilogram-runtime-ipc') 'offline execution production manifest'
    Assert-OmitsAll ($CliCargo + "`n" + $DesktopCargo) @('kilogram-peer-history-recovery-transport') 'active app manifests'

    Assert-ContainsAll $Transport @(
        'pub const HISTORY_RECOVERY_TRANSFER_ALPN: &[u8] = b"kilogram/m1/history-recovery/postcard/1"',
        'Duration::from_secs(180)',
        'MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES + 4 * 1024',
        'MAX_HISTORY_RECOVERY_TRANSFER_RESPONSE_BYTES: usize = 4 * 1024',
        'kilogram:history-recovery-transfer-session-request:v1\0',
        'kilogram:history-recovery-transfer-session-request-id:v1\0',
        'kilogram:history-recovery-transfer-session-response:v1\0',
        'content.session_nonce == [0_u8; 32]',
        'endpoint_bytes(connection.remote_id()) != prepared.recipient_endpoint_id()',
        'VerifiedTransferSessionRequest::decode_for_recipient',
        'pub(crate) async fn read_history_recovery_transfer_request(',
        'pub(crate) async fn complete_history_recovery_transfer(',
        'inbox.stage_envelope_bytes(&pending.request.wire.content.envelope)',
        'StageHistoryRecoveryTransferOutcome::AlreadyStaged',
        'VerifiedTransferSessionResponse::create(&pending.request, recipient, disposition)',
        'receive.read_to_end(maximum)',
        'send.stopped()',
        'loopback_stages_then_acknowledges_exact_retry',
        'authenticated_endpoint_substitution_fails_before_staging',
        'zero_nonce_and_trailing_bytes_fail_closed'
    ) 'history recovery Iroh implementation'
    Assert-OmitsAll ($ActiveTransport + "`n" + $Cli) @(
        'kilogram/m1/history-recovery/postcard/1',
        'HISTORY_RECOVERY_TRANSFER_ALPN'
    ) 'active runtime transport surface'
    if ($Fixture.Length -lt 16000 -or $Fixture.IndexOf('=', [StringComparison]::Ordinal) -lt 0) {
        throw 'fixed M1.2.22 transfer fixture is missing or implausibly short'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'not an active runtime protocol',
        'recipient compares the signed Endpoint IDs with the authenticated Iroh connection before any durable mutation',
        'Only then does the recipient Device sign a response',
        'recipient inbox still grants neither network nor import authority',
        'Aggregate accepted-tag readiness contains 149 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "history recovery Iroh RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $workspaceCargoPath, $transportCargoPath, $transportPath, $fixturePath,
    $executionCargoPath, $activeTransportPath, $cliCargoPath, $cliPath,
    $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$workspaceCargo = Get-Content -LiteralPath $workspaceCargoPath -Raw
$transportCargo = Get-Content -LiteralPath $transportCargoPath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$fixture = Get-Content -LiteralPath $fixturePath -Raw
$executionCargo = Get-Content -LiteralPath $executionCargoPath -Raw
$activeTransport = Get-Content -LiteralPath $activeTransportPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $workspaceCargo,
    $transportCargo, $transport, $fixture, $executionCargo, $activeTransport,
    $cliCargo, $cli, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.session.alpn = 'kilogram/m1/history-recovery/postcard/2'
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session.max_request_bytes = 9441281
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.request.authenticated_source_endpoint_bound = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.response.staging_precedes_response = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.retry.restart_safe_source_outbox = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.authority.runtime_accepts_history_recovery_alpn = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.verification.external_network_executed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.public_default = 'canonical-v2'
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changedTransport = $transport.Replace('content.session_nonce == [0_u8; 32]', 'false')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $workspaceCargo $transportCargo $changedTransport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $rfc }
    $changedRfc = $rfc.Replace('not an active runtime protocol', 'an active runtime protocol')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $workspaceCargo $transportCargo $transport $fixture $executionCargo $activeTransport $cliCargo $cli $desktopCargo $changedRfc }

    $rejected = 0
    $mutationIndex = 0
    foreach ($mutation in $mutations) {
        $mutationIndex++
        try { & $mutation; throw "negative mutation $mutationIndex unexpectedly passed" }
        catch {
            if ($_.Exception.Message -ceq "negative mutation $mutationIndex unexpectedly passed") { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_iroh_session_boundary=verified'
Write-Output 'milestone=M1.2.23'
Write-Output 'alpn=kilogram/m1/history-recovery/postcard/1'
Write-Output 'active_runtime_registration=false'
Write-Output 'public_default=legacy-only'
