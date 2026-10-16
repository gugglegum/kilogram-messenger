[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$migrationPath = Join-Path $workspace 'ACTIVE-RUNTIME-IPC-LEGACY-MAILBOX-RETIREMENT.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0175-m1149-active-runtime-ipc-legacy-mailbox-retirement.md'

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
        [string]$DesktopSource,
        [string]$Rfc
    )

    if ([UInt64]$Migration.schema_version -ne 1 -or
        [string]$Migration.milestone -cne 'M1.1.49' -or
        [string]$Migration.status -cne 'active-local-control-migration' -or
        [string]$Migration.surface -cne 'runtime-ipc' -or
        [string]$Migration.encoding -cne 'postcard' -or
        [int]$Migration.previous.ipc_version -ne 28 -or
        [string]$Migration.previous.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v9\0' -or
        [int]$Migration.previous.command_count -ne 31 -or
        [int]$Migration.previous.response_count -ne 28 -or
        [int]$Migration.current.ipc_version -ne 29 -or
        [string]$Migration.current.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v10\0' -or
        [int]$Migration.current.command_count -ne 29 -or
        [int]$Migration.current.response_count -ne 28) {
        throw 'runtime IPC legacy-mailbox migration identity changed'
    }

    $retiredCommands = @('CreateMailboxCapability', 'RotateMailboxCapability')
    $retainedCommands = @(
        'CreateExactMailboxCapability',
        'RotateExactMailboxCapability',
        'RevokeMailboxCapability',
        'MailboxStatus'
    )
    Assert-ExactSequence @($Migration.retired_commands) $retiredCommands 'retired commands'
    Assert-ExactSequence @($Migration.retained_commands) $retainedCommands 'retained commands'

    if (-not [bool]$Migration.scope.changes_active_local_control -or
        [bool]$Migration.scope.changes_peer_network_wire -or
        [bool]$Migration.scope.changes_relay_wire -or
        [bool]$Migration.scope.changes_mailbox_wire -or
        [bool]$Migration.scope.changes_sync_wire -or
        [bool]$Migration.scope.activates_canonical_cbor -or
        [bool]$Migration.migration.dual_reader -or
        [bool]$Migration.migration.silent_downgrade -or
        [string]$Migration.migration.strategy -cne
            'exact-version-hard-cut-for-co-installed-binaries' -or
        -not [bool]$Migration.completion.canonical_runtime_ipc_target_complete -or
        -not [bool]$Migration.completion.active_postcard_adapter_complete -or
        [int]$Migration.completion.postcard_only_activation_blockers -ne 0 -or
        [int]$Migration.completion.remaining_runtime_ipc_surface -ne 0) {
        throw 'runtime IPC legacy-mailbox migration scope or completion changed'
    }

    foreach ($required in @(
        'const IPC_VERSION: u8 = 29;',
        'kilogram:runtime-ipc-descriptor:v10\0',
        'old_version.content.version = 28;',
        'kilogram:runtime-ipc-descriptor:v9\0',
        'version_28_request_is_rejected_before_actor_dispatch',
        'assert!(old_version.verify().is_err());'
    )) {
        if ($RuntimeSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "runtime IPC source is missing '$required'"
        }
    }
    foreach ($retired in $retiredCommands) {
        if ($RuntimeSource.IndexOf("    $retired {", [StringComparison]::Ordinal) -ge 0 -or
            $CliSource.IndexOf("RuntimeIpcCommand::$retired {", [StringComparison]::Ordinal) -ge 0) {
            throw "retired active runtime IPC command '$retired' remains reachable"
        }
    }
    foreach ($retained in @(
        '    CreateExactMailboxCapability {',
        '    RotateExactMailboxCapability {',
        '    RevokeMailboxCapability {',
        '    MailboxStatus,'
        'RuntimeIpcCommand::CreateExactMailboxCapability {',
        'RuntimeIpcCommand::RotateExactMailboxCapability {',
        'RuntimeMailboxProvisioningMode::ExactVolunteer',
        'RuntimeMailboxProvisioningMode::LegacyHttps',
        'acknowledged_legacy_mailbox_upgrades_once_to_service_free_v2'
    )) {
        $source = if ($retained.StartsWith('    ')) { $RuntimeSource } else { $CliSource }
        if ($source.IndexOf($retained, [StringComparison]::Ordinal) -lt 0) {
            throw "retained mailbox behavior '$retained' is missing"
        }
    }
    foreach ($required in @(
        'RuntimeIpcCommand::CreateExactMailboxCapability {',
        'RuntimeIpcCommand::RotateExactMailboxCapability {',
        'Storage: exact signed volunteer replica set (no central HTTPS mailbox)'
    )) {
        if ($DesktopSource.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "desktop exact mailbox path is missing '$required'"
        }
    }

    $activeCommands = @($Completion.active_postcard.commands)
    $activeResponses = @($Completion.active_postcard.responses)
    if ([string]$Completion.milestone -cne 'M1.1.49' -or
        [string]$Completion.status -cne
            'completion-inventory-complete-canonical-target-complete' -or
        -not [bool]$Completion.scope.canonical_target_complete -or
        -not [bool]$Completion.scope.active_postcard_adapter_complete -or
        [bool]$Completion.scope.canonical_cbor_active -or
        [int]$Completion.active_postcard.version -ne 29 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v10\0' -or
        $activeCommands.Count -ne 29 -or $activeResponses.Count -ne 28 -or
        @($Completion.postcard_only_activation_blockers).Count -ne 0 -or
        @($Completion.remaining_canonical_work).Count -ne 0) {
        throw 'completion inventory does not describe complete active Postcard v29'
    }
    foreach ($retired in $retiredCommands) {
        if ($retired -cin @($activeCommands | ForEach-Object { [string]$_.variant })) {
            throw "retired command '$retired' remains in the completion inventory"
        }
    }

    if ([string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v29-with-json-descriptor-v10' -or
        -not [bool]$Schema.activation.current_runtime_changed -or
        @($Schema.semantic_rules).Count -ne 179 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0 -or
        'complete-all-runtime-ipc-commands-and-results' -cin
            @($Schema.activation.required_before_activation)) {
        throw 'canonical runtime IPC schema did not record completed active adapter work'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([string]$Coverage.milestone -cne 'M1.1.50' -or $surface.Count -ne 1 -or
        [string]$surface[0].coverage -cne 'complete' -or
        'active-postcard-legacy-central-https-mailbox-command-retirement' -cnotin
            @($surface[0].covered) -or
        @($surface[0].remaining).Count -ne 0 -or
        [int]$Coverage.summary.complete -ne 17 -or
        [int]$Coverage.summary.partial -ne 1 -or
        (@($Coverage.next_priority) -join ',') -cne 'transport-sync') {
        throw 'canonical coverage did not complete runtime IPC'
    }

    $compatibilitySurface = @($Compatibility.surfaces | Where-Object {
        [string]$_.id -ceq 'runtime-ipc'
    })
    if ($compatibilitySurface.Count -ne 1 -or
        [int](@($compatibilitySurface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'IPC_VERSION'
        })[0].value) -ne 29 -or
        [string](@($compatibilitySurface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v10\0') {
        throw 'protocol compatibility registry does not describe runtime IPC v29/v10'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'removes `CreateMailboxCapability` and `RotateMailboxCapability`',
        'exclusively `CreateExactMailboxCapability` and `RotateExactMailboxCapability`',
        'Legacy state is readable, not newly provisioned',
        'version 29 with domain v10',
        '29 commands and 28 responses',
        'Completion is not activation',
        'changes no peer network wire'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0175 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $migrationPath, $completionPath, $schemaPath, $coveragePath,
    $compatibilityPath, $runtimePath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "runtime IPC legacy-mailbox retirement file is missing: $path"
    }
}

$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$runtimeSource = Get-Content -LiteralPath $runtimePath -Raw
$cliSource = Get-Content -LiteralPath $cliPath -Raw
$desktopSource = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-RetirementBoundary $migration $completion $schema $coverage $compatibility `
    $runtimeSource $cliSource $desktopSource $rfc

$forgedVersion = Copy-JsonObject $migration
$forgedVersion.current.ipc_version = 28
Assert-Rejected {
    Assert-RetirementBoundary $forgedVersion $completion $schema $coverage $compatibility `
        $runtimeSource $cliSource $desktopSource $rfc
} 'forged_current_version'

$missingRetirement = Copy-JsonObject $migration
$missingRetirement.retired_commands = @($missingRetirement.retired_commands | Select-Object -Skip 1)
Assert-Rejected {
    Assert-RetirementBoundary $missingRetirement $completion $schema $coverage $compatibility `
        $runtimeSource $cliSource $desktopSource $rfc
} 'missing_retired_command'

$reintroducedHandler = $cliSource + "`nRuntimeIpcCommand::CreateMailboxCapability {`n"
Assert-Rejected {
    Assert-RetirementBoundary $migration $completion $schema $coverage $compatibility `
        $runtimeSource $reintroducedHandler $desktopSource $rfc
} 'reintroduced_legacy_mailbox_handler'

Write-Output 'runtime_ipc_legacy_mailbox_retirement_boundary=verified'
Write-Output 'active_postcard_version=29'
Write-Output 'active_postcard_commands=29'
Write-Output 'active_postcard_responses=28'
Write-Output 'postcard_only_activation_blockers=0'
Write-Output 'runtime_ipc_surface=complete'
Write-Output 'peer_network_wire_changed=false'
Write-Output 'canonical_cbor_activated=false'
