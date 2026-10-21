[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$migrationPath = Join-Path $workspace 'ACTIVE-RUNTIME-IPC-ENDPOINT-BOOTSTRAP-RETIREMENT.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0174-m1148-active-runtime-ipc-endpoint-bootstrap-retirement.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactSequence {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualStrings = @($Actual | ForEach-Object { [string]$_ })
    if ($actualStrings.Count -ne $Expected.Count) {
        throw "$Label count changed: expected $($Expected.Count), got $($actualStrings.Count)"
    }
    for ($index = 0; $index -lt $Expected.Count; $index++) {
        if ($actualStrings[$index] -cne $Expected[$index]) {
            throw "$Label changed at index ${index}: expected '$($Expected[$index])', got '$($actualStrings[$index])'"
        }
    }
}

function Assert-RetirementBoundary {
    param(
        [object]$Migration,
        [object]$Completion,
        [object]$Schema,
        [object]$Coverage,
        [object]$Compatibility,
        [string]$RuntimeSource,
        [string]$CliSource,
        [string]$Rfc
    )

    if ([UInt64]$Migration.schema_version -ne 1 -or
        [string]$Migration.milestone -cne 'M1.1.48' -or
        [string]$Migration.status -cne 'active-local-control-migration' -or
        [string]$Migration.surface -cne 'runtime-ipc' -or
        [string]$Migration.encoding -cne 'postcard' -or
        [int]$Migration.previous.ipc_version -ne 27 -or
        [string]$Migration.previous.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v8\0' -or
        [int]$Migration.previous.command_count -ne 34 -or
        [int]$Migration.previous.response_count -ne 31 -or
        [int]$Migration.current.ipc_version -ne 28 -or
        [string]$Migration.current.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v9\0' -or
        [int]$Migration.current.command_count -ne 31 -or
        [int]$Migration.current.response_count -ne 28) {
        throw 'runtime IPC endpoint-bootstrap migration identity changed'
    }

    $retiredCommands = @(
        'ExportEndpointAnnouncements',
        'ImportEndpointAnnouncements',
        'PushEndpointAnnouncements'
    )
    $retiredResponses = @(
        'EndpointAnnouncementsExported',
        'EndpointAnnouncementsImported',
        'EndpointAnnouncementsPushed'
    )
    Assert-ExactSequence @($Migration.retired_commands) $retiredCommands 'retired commands'
    Assert-ExactSequence @($Migration.retired_responses) $retiredResponses 'retired responses'

    if (-not [bool]$Migration.scope.changes_active_local_control -or
        [bool]$Migration.scope.changes_peer_network_wire -or
        [bool]$Migration.scope.changes_relay_wire -or
        [bool]$Migration.scope.changes_mailbox_wire -or
        [bool]$Migration.scope.changes_sync_wire -or
        [bool]$Migration.scope.activates_canonical_cbor -or
        [bool]$Migration.migration.dual_reader -or
        [bool]$Migration.migration.silent_downgrade -or
        [string]$Migration.migration.strategy -cne
            'exact-version-hard-cut-for-co-installed-binaries') {
        throw 'runtime IPC endpoint-bootstrap migration scope changed'
    }

    foreach ($required in @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15\0',
        'old_version.content.version = 30;',
        'kilogram:runtime-ipc-descriptor:v11\0',
        'version_32_request_is_rejected_before_actor_dispatch',
        'assert!(old_version.verify().is_err());'
    )) {
        if ($RuntimeSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "runtime IPC source is missing '$required'"
        }
    }
    foreach ($retired in $retiredCommands + $retiredResponses) {
        if ($RuntimeSource.IndexOf($retired, [StringComparison]::Ordinal) -ge 0) {
            throw "retired active runtime IPC variant '$retired' remains in the enum source"
        }
    }
    foreach ($retired in @(
        'RuntimeIpcExportEndpointAnnouncements',
        'RuntimeIpcImportEndpointAnnouncements',
        'RuntimeIpcPushEndpointAnnouncements',
        'runtime_ipc_export_endpoint_announcements',
        'runtime_ipc_import_endpoint_announcements',
        'runtime_ipc_push_endpoint_announcements'
    )) {
        if ($CliSource.IndexOf($retired, [StringComparison]::Ordinal) -ge 0) {
            throw "retired public CLI surface '$retired' remains"
        }
    }
    foreach ($retained in @(
        'ClientRequest::EndpointAnnouncementPush',
        'export_runtime_endpoint_announcements',
        'import_runtime_endpoint_announcements',
        'push_runtime_endpoint_announcements',
        'RuntimeIpcCommand::ConfigureOwnDeviceAnnouncementAutomation',
        'RuntimeIpcCommand::ConfigureOwnDeviceTicketDiscovery',
        'RuntimeIpcCommand::ConfigureOwnDeviceRosterAutomation'
    )) {
        if ($CliSource.IndexOf($retained, [StringComparison]::Ordinal) -lt 0) {
            throw "retained endpoint-announcement behavior '$retained' is missing"
        }
    }

    $activeCommands = @($Completion.active_postcard.commands)
    $activeResponses = @($Completion.active_postcard.responses)
    if ([string]$Completion.milestone -cne 'M1.1.49' -or
        -not [bool]$Completion.scope.changes_active_local_control -or
        [int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v15\0' -or
        $activeCommands.Count -ne 34 -or $activeResponses.Count -ne 33) {
        throw 'completion inventory does not describe active Postcard v33'
    }
    foreach ($retired in $retiredCommands) {
        if ($retired -cin @($activeCommands | ForEach-Object { [string]$_.variant })) {
            throw "retired command '$retired' remains in the completion inventory"
        }
    }
    foreach ($retired in $retiredResponses) {
        if ($retired -cin @($activeResponses | ForEach-Object { [string]$_.variant })) {
            throw "retired response '$retired' remains in the completion inventory"
        }
    }
    Assert-ExactSequence @($Completion.postcard_only_activation_blockers) `
        @() 'remaining Postcard blockers'
    Assert-ExactSequence @($Completion.remaining_canonical_work) `
        @() 'remaining canonical work'

    if ([string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v34-with-json-descriptor-v15' -or
        -not [bool]$Schema.activation.current_runtime_changed -or
        @($Schema.semantic_rules).Count -ne 192) {
        throw 'canonical runtime IPC schema did not record the active migration'
    }
    Assert-ExactSequence @($Schema.remaining_runtime_ipc_surface) @() 'schema remaining work'

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        'active-postcard-endpoint-bootstrap-helper-retirement' -cnotin @($surface[0].covered) -or
        @($surface[0].remaining).Count -ne 0) {
        throw 'canonical coverage did not record endpoint bootstrap retirement'
    }

    $compatibilitySurface = @($Compatibility.surfaces | Where-Object {
        [string]$_.id -ceq 'runtime-ipc'
    })
    if ($compatibilitySurface.Count -ne 1 -or
        [int](@($compatibilitySurface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'IPC_VERSION'
        })[0].value) -ne 34 -or
        [string](@($compatibilitySurface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v15\0') {
        throw 'protocol compatibility registry does not describe runtime IPC v33/v14'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'removes the three manual endpoint-announcement bootstrap commands',
        'does not remove endpoint announcements',
        'version 28 with domain v9',
        '31 commands and 28 responses',
        'changes no peer network, relay, mailbox or sync wire',
        'legacy-central-https-mailbox-command-removal'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0174 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $migrationPath, $completionPath, $schemaPath, $coveragePath,
    $compatibilityPath, $runtimePath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "runtime IPC endpoint-bootstrap retirement file is missing: $path"
    }
}

$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$runtimeSource = Get-Content -LiteralPath $runtimePath -Raw
$cliSource = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-RetirementBoundary $migration $completion $schema $coverage $compatibility `
    $runtimeSource $cliSource $rfc

$forgedVersion = Copy-JsonObject $migration
$forgedVersion.current.ipc_version = 27
Assert-Rejected {
    Assert-RetirementBoundary $forgedVersion $completion $schema $coverage $compatibility `
        $runtimeSource $cliSource $rfc
} 'forged_current_version'

$missingRetirement = Copy-JsonObject $migration
$missingRetirement.retired_commands = @($missingRetirement.retired_commands | Select-Object -Skip 1)
Assert-Rejected {
    Assert-RetirementBoundary $missingRetirement $completion $schema $coverage $compatibility `
        $runtimeSource $cliSource $rfc
} 'missing_retired_command'

$reintroducedCli = $cliSource + "`nRuntimeIpcExportEndpointAnnouncements`n"
Assert-Rejected {
    Assert-RetirementBoundary $migration $completion $schema $coverage $compatibility `
        $runtimeSource $reintroducedCli $rfc
} 'reintroduced_cli_command'

Write-Output 'runtime_ipc_endpoint_bootstrap_retirement_boundary=verified'
Write-Output 'active_postcard_version=31'
Write-Output 'active_postcard_commands=30'
Write-Output 'active_postcard_responses=29'
Write-Output 'postcard_only_activation_blockers=0'
Write-Output 'peer_network_wire_changed=false'
Write-Output 'canonical_cbor_activated=false'
