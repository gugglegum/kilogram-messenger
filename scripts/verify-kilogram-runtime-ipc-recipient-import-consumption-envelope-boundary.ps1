[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'RUNTIME-IPC-RECIPIENT-IMPORT-CONSUMPTION-ENVELOPE.json'
$preflightPath = Join-Path $workspace 'RUNTIME-IPC-RECIPIENT-IMPORT-CONSUMPTION-MIGRATION.json'
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$runtimeCargoPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\Cargo.toml'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rfcPath = Join-Path $workspace 'docs\RFC-0268-m1244-active-runtime-ipc-recipient-import-consumption-envelope.md'

function Copy-JsonObject { param([object]$Value) $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json }
function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param(
        [object]$Manifest, [object]$Preflight, [string]$Runtime, [string]$Cli,
        [string]$Desktop, [string]$RuntimeCargo, [string]$CliCargo,
        [object]$Completion, [object]$Schema, [object]$Compatibility,
        [string]$Cddl, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.44' -or
        [string]$Manifest.status -cne 'active-envelope-handler-fail-closed' -or
        [string]$Manifest.surface -cne 'runtime-ipc' -or
        [string]$Manifest.encoding -cne 'postcard' -or
        [string]$Manifest.predecessor -cne 'M1.2.43') {
        throw 'recipient import-consumption envelope identity changed'
    }
    if ([string]$Preflight.milestone -cne 'M1.2.43' -or
        [int]$Preflight.previous.ipc_version -ne 33 -or
        [string]$Preflight.previous.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v14\0' -or
        [int]$Preflight.target.ipc_version -ne 34 -or
        [string]$Preflight.target.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v15\0') {
        throw 'historical migration preflight claims changed'
    }
    if ([int]$Manifest.previous.ipc_version -ne 33 -or
        [string]$Manifest.previous.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v14\0' -or
        [bool]$Manifest.previous.recipient_import_consumption_command_present -or
        [bool]$Manifest.previous.recipient_import_consumption_response_present -or
        [int]$Manifest.current.ipc_version -ne 34 -or
        [string]$Manifest.current.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v15\0' -or
        [string]$Manifest.current.command_variant -cne
            'RecipientImportConsumption { request: Vec<u8> }' -or
        [string]$Manifest.current.response_variant -cne
            'RecipientImportConsumption { signed_response: Vec<u8> }' -or
        -not [bool]$Manifest.current.append_only_enum_extension) {
        throw 'recipient import-consumption envelope generation changed'
    }
    $migration = $Manifest.migration
    if ([string]$migration.strategy -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [string]$migration.legacy_descriptor -cne 'rejected-before-connect' -or
        [string]$migration.legacy_request -cne 'rejected-before-actor-dispatch' -or
        [bool]$migration.dual_reader -or [bool]$migration.format_sniffing -or
        [bool]$migration.silent_downgrade -or
        [bool]$migration.hybrid_version_domain_state_allowed) {
        throw 'recipient import-consumption migration became permissive'
    }
    if (-not [bool]$Manifest.handler.command_recognized -or
        [bool]$Manifest.handler.service_configured -or
        [string]$Manifest.handler.response -cne 'RuntimeIpcResponse::Error' -or
        [string]$Manifest.handler.error -cne
            'recipient import-consumption runtime service is not configured' -or
        [bool]$Manifest.handler.signed_response_reachable) {
        throw 'fail-closed placeholder handler boundary changed'
    }
    foreach ($property in @(
        'nested_request_reinterpreted_by_outer_envelope',
        'plan_session_cache_owned_by_runtime', 'dispatcher_wired',
        'mutation_authority_granted', 'cli_control', 'desktop_control'
    )) {
        if ([bool]$Manifest.authority_boundary.$property) {
            throw "inactive authority member '$property' became active"
        }
    }
    if (-not [bool]$Manifest.scope.changes_active_local_control -or
        [bool]$Manifest.scope.changes_peer_network_wire -or
        [bool]$Manifest.scope.changes_relay_wire -or
        [bool]$Manifest.scope.changes_mailbox_wire -or
        [bool]$Manifest.scope.changes_sync_wire -or
        [bool]$Manifest.scope.network -or
        [bool]$Manifest.scope.public_default_changed -or
        [string]$Manifest.scope.public_default -cne 'legacy-only') {
        throw 'recipient import-consumption envelope scope changed'
    }

    Assert-ContainsAll $Runtime @(
        'const IPC_VERSION: u8 = 34;',
        'const DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ipc-descriptor:v15\0";',
        'version_33_request_is_rejected_before_actor_dispatch',
        'version: 33,',
        'version_33_descriptor_signed_with_v14_domain_is_rejected',
        'b"kilogram:runtime-ipc-descriptor:v14\0".to_vec()'
    ) 'runtime IPC source'
    if ([regex]::Matches($Runtime, 'const IPC_VERSION: u8 = 34;').Count -ne 1 -or
        [regex]::Matches($Runtime, [regex]::Escape(
            'const DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ipc-descriptor:v15\0";')).Count -ne 1 -or
        [regex]::Matches($Runtime,
            'RecipientImportConsumption\s*\{\s*request: Vec<u8>,\s*\}').Count -ne 1 -or
        [regex]::Matches($Runtime,
            'RecipientImportConsumption\s*\{\s*signed_response: Vec<u8>,\s*\}').Count -ne 1) {
        throw 'runtime IPC generation constants are not singular'
    }
    Assert-ContainsAll $Cli @(
        'RuntimeIpcCommand::RecipientImportConsumption { request } => {',
        'recipient_import_consumption.dispatch(',
        'RuntimeIpcResponse::RecipientImportConsumption {',
        'runtime_recipient_import_consumption_context=unattached'
    ) 'runtime actor successor'
    if ([regex]::Matches($Cli, 'RuntimeIpcCommand::RecipientImportConsumption').Count -ne 1 -or
        $Desktop.IndexOf('RecipientImportConsumption', [StringComparison]::Ordinal) -ge 0) {
        throw 'recipient import-consumption gained an unreviewed client path'
    }
    if ($RuntimeCargo.IndexOf('kilogram-peer-history-recovery-runtime',
            [StringComparison]::Ordinal) -ge 0 -or
        $CliCargo.IndexOf('kilogram-peer-history-recovery-runtime',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'runtime IPC envelope successor dependency boundary changed'
    }

    if (@($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38 -or
        @($Completion.canonical.command_result_pairs).Count -ne 39 -or
        [int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.active_postcard.commands).Count -ne 34 -or
        @($Completion.active_postcard.responses).Count -ne 33) {
        throw 'runtime IPC completion inventory does not describe v34 envelope'
    }
    $pair = @($Completion.canonical.command_result_pairs | Where-Object {
        [string]$_.command -ceq 'recipient-import-consumption'
    })
    if ($pair.Count -ne 1 -or
        [string]$pair[0].result -cne 'recipient-import-consumption-result' -or
        [string]$pair[0].module -cne 'recovery-consumption' -or
        [string]$pair[0].kind -cne 'mutation-envelope' -or
        'RecipientImportConsumption' -cnotin @($Completion.active_postcard.commands.variant) -or
        'RecipientImportConsumption' -cnotin @($Completion.active_postcard.responses.variant)) {
        throw 'recipient import-consumption completion pair changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v34-with-json-descriptor-v15' -or
        @($Schema.covered_commands).Count -ne 39 -or
        @($Schema.covered_results).Count -ne 38 -or
        'recipient-import-consumption' -cnotin @($Schema.covered_commands) -or
        'recipient-import-consumption-result' -cnotin @($Schema.covered_results)) {
        throw 'runtime IPC schema does not cover the v34 envelope'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'IPC_VERSION'
        })[0].value) -ne 34 -or
        [string](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v15\0') {
        throw 'protocol compatibility registry does not describe v34 envelope'
    }
    Assert-ContainsAll $Cddl @(
        'runtime-ipc-recipient-import-consumption-request-v1 = [',
        'signed-runtime-ipc-recipient-import-consumption-response-v1 = [',
        'runtime-ipc-recipient-import-consumption-request-v1 /',
        'signed-runtime-ipc-recipient-import-consumption-response-v1 /'
    ) 'canonical runtime IPC CDDL'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Postcard runtime IPC advances from version 33 to version 34',
        'rejected before actor dispatch',
        'service is not configured',
        'success response is registered in the byte contract but is unreachable',
        'separate M1.2.45 stage', '170 gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0268 is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $preflightPath, $runtimePath, $cliPath,
    $desktopPath, $runtimeCargoPath, $cliCargoPath, $completionPath, $schemaPath,
    $compatibilityPath, $cddlPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$preflight = Get-Content -LiteralPath $preflightPath -Raw | ConvertFrom-Json
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$runtimeCargo = Get-Content -LiteralPath $runtimeCargoPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $preflight, $runtime, $cli, $desktop, $runtimeCargo,
    $cliCargo, $completion, $schema, $compatibility, $cddl, $rfc)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.current.ipc_version = 33
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.current.descriptor_signature_domain = 'kilogram:runtime-ipc-descriptor:v14\0'
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.migration.dual_reader = $true
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handler.service_configured = $true
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.handler.signed_response_reachable = $true
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.authority_boundary.mutation_authority_granted = $true
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changed = Copy-JsonObject $manifest; $changed.scope.network = $true
    $mutations += { Assert-Boundary $changed $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changedRuntime = $runtime.Replace('const IPC_VERSION: u8 = 34;', 'const IPC_VERSION: u8 = 33;')
    $mutations += { Assert-Boundary $manifest $preflight $changedRuntime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changedCli = $cli.Replace('recipient_import_consumption.dispatch(', 'recipient_import_consumption.accept_without_dispatch(')
    $mutations += { Assert-Boundary $manifest $preflight $runtime $changedCli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $cddl $rfc }
    $changedCddl = $cddl.Replace('runtime-ipc-recipient-import-consumption-request-v1 /', '')
    $mutations += { Assert-Boundary $manifest $preflight $runtime $cli $desktop $runtimeCargo $cliCargo $completion $schema $compatibility $changedCddl $rfc }
    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'mutation was accepted' } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=10/10'
}

Write-Output 'runtime_ipc_recipient_import_consumption_envelope_boundary=verified'
Write-Output 'active_runtime_ipc=postcard-v34-json-descriptor-v15'
Write-Output 'runtime_service_configured=process-owned-context-gated'
Write-Output 'signed_success_reachable=guided-context-only'
Write-Output 'public_default=legacy-only'
Write-Output 'network_executed=false'
