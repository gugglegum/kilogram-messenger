[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$migrationPath = Join-Path $workspace 'ACTIVE-RUNTIME-IPC-MIGRATION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0171-m1145-active-runtime-ipc-conversation-pagination.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-ContainsExact {
    param([string]$Text, [string[]]$Required, [string]$Label)
    foreach ($value in $Required) {
        if ($Text.IndexOf($value, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$value'"
        }
    }
}

function Assert-Boundary {
    param(
        [string]$Runtime,
        [string]$Cli,
        [string]$Desktop,
        [object]$Migration,
        [object]$Schema,
        [object]$Coverage,
        [object]$Compatibility,
        [string]$Rfc
    )

    if ([UInt64]$Migration.schema_version -ne 1 -or
        [string]$Migration.milestone -cne 'M1.1.45' -or
        [string]$Migration.status -cne 'active-local-control-migration' -or
        [string]$Migration.surface -cne 'runtime-ipc' -or
        [string]$Migration.encoding -cne 'postcard' -or
        [int]$Migration.previous.ipc_version -ne 26 -or
        [string]$Migration.previous.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v7\0' -or
        [string]$Migration.previous.conversation_command -cne 'conversation-list-unpaged' -or
        [int]$Migration.current.ipc_version -ne 27 -or
        [string]$Migration.current.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v8\0' -or
        [string]$Migration.current.conversation_command -cne 'conversation-list-page' -or
        [int]$Migration.current.maximum_page_size -ne 64 -or
        [int]$Migration.current.desktop_maximum_refresh_pages -ne 1024) {
        throw 'active runtime IPC migration manifest header changed'
    }
    if ([string]$Migration.migration.strategy -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [string]$Migration.migration.legacy_descriptor -cne 'rejected-before-connect' -or
        [string]$Migration.migration.legacy_request -cne 'rejected-before-actor-dispatch' -or
        [bool]$Migration.migration.dual_reader -or [bool]$Migration.migration.silent_downgrade -or
        -not [bool]$Migration.scope.changes_active_local_control -or
        [bool]$Migration.scope.changes_peer_network_wire -or
        [bool]$Migration.scope.changes_relay_wire -or
        [bool]$Migration.scope.changes_mailbox_wire -or
        [bool]$Migration.scope.changes_sync_wire -or
        [bool]$Migration.scope.activates_canonical_cbor) {
        throw 'active runtime IPC migration scope changed'
    }
    if ((@($Migration.pagination.cursor) -join ',') -cne
            'exact-utf8-conversation-label,peer-account-id' -or
        (@($Migration.pagination.ordering) -join ',') -cne
            'utf8-label-bytewise-ascending,peer-account-id-bytewise-ascending' -or
        [string]$Migration.pagination.continuation -cne
            'full-page-only-and-equals-last-returned-key' -or
        [string]$Migration.pagination.server_expensive_contact_projection -cne
            'selected-page-only' -or
        [string]$Migration.pagination.desktop_behavior -cne
            'validate-and-collect-all-pages-or-fail-explicitly') {
        throw 'active runtime IPC pagination contract changed'
    }

    Assert-ContainsExact $Runtime @(
        'const IPC_VERSION: u8 = 32;',
        'kilogram:runtime-ipc-descriptor:v13\0',
        'pub const MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE: u16 = 64;',
        'ConversationListPage {',
        'pub struct RuntimeIpcConversationCursor',
        'pub struct RuntimeIpcConversationPage',
        'pub fn validate_for_request(',
        'runtime IPC conversation page did not advance beyond its request cursor',
        'version_31_request_is_rejected_before_actor_dispatch',
        'conversation_page_requires_exact_monotonic_continuation'
    ) 'runtime IPC implementation'
    if ($Runtime.IndexOf('const IPC_VERSION: u8 = 30;', [StringComparison]::Ordinal) -ge 0 -or
        [regex]::Matches($Runtime, [regex]::Escape('kilogram:runtime-ipc-descriptor:v11\0')).Count -ne 1) {
        throw 'legacy runtime IPC version/domain is not isolated to the downgrade regression'
    }

    Assert-ContainsExact $Cli @(
        'fn collect_runtime_conversation_page(',
        'contacts.partition_point(',
        'for contact in &contacts[start..end]',
        'runtime contacts contain a duplicated conversation cursor key',
        'page.validate_for_request(cursor.as_ref(), limit)?;',
        'RuntimeIpcCommand::ConversationListPage { cursor, limit }',
        'RuntimeIpcResponse::ConversationListPage(page)'
    ) 'runtime actor adapter'
    if ($Cli.IndexOf('fn collect_runtime_conversation_list(', [StringComparison]::Ordinal) -ge 0 -or
        $Cli.IndexOf('RuntimeIpcCommand::ConversationList =>', [StringComparison]::Ordinal) -ge 0) {
        throw 'active unpaged runtime conversation list remains reachable'
    }

    Assert-ContainsExact $Desktop @(
        'const MAX_CONVERSATION_REFRESH_PAGES: usize = 1_024;',
        'for _ in 0..MAX_CONVERSATION_REFRESH_PAGES',
        'page.validate_for_request(',
        'RuntimeIpcCommand::ConversationListPage {',
        'RuntimeIpcResponse::ConversationListPage(page)',
        'conversation_refresh_collects_multiple_strict_pages',
        'Runtime conversation list exceeded the desktop safety limit'
    ) 'desktop pagination adapter'

    if ([string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v32-with-json-descriptor-v13' -or
        -not [bool]$Schema.activation.current_runtime_changed -or
        @($Schema.semantic_rules).Count -ne 185 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0) {
        throw 'canonical runtime IPC aggregate did not record the active adapter migration'
    }
    $runtimeSurface = @($Coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
    if ($runtimeSurface.Count -ne 1 -or
        'active-postcard-conversation-list-pagination-migration' -cnotin
            @($runtimeSurface[0].covered) -or
        @($runtimeSurface[0].remaining).Count -ne 0) {
        throw 'canonical coverage did not close the active unpaged-list migration'
    }
    $source = @($Compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
    if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
        [string]$source[0].compatibility -cne 'exact-version' -or
        [string]$source[0].migration -cne 'new-version-explicit-migration' -or
        [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 32 -or
        [string](@($source[0].bindings | Where-Object {
            $_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v13\0') {
        throw 'protocol compatibility registry does not exactly describe active IPC v32'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @('explicitly paginated', 'version 26 descriptors and',
        'fails before actor dispatch', 'at most one page', '1,024 pages',
        'does not change any peer-to-peer', 'does not activate canonical CBOR')) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0171 is missing '$required'"
        }
    }
}

foreach ($path in @($runtimePath, $cliPath, $desktopPath, $migrationPath, $schemaPath,
    $coveragePath, $compatibilityPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "runtime IPC pagination migration file is missing: $path"
    }
}
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $runtime $cli $desktop $migration $schema $coverage $compatibility $rfc

if ($SelfTest) {
    $oldManifest = $migration | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $oldManifest.current.ipc_version = 26
    Assert-Rejected {
        Assert-Boundary $runtime $cli $desktop $oldManifest $schema $coverage $compatibility $rfc
    } 'legacy_manifest_version'
    $unboundedDesktop = $desktop.Replace(
        'const MAX_CONVERSATION_REFRESH_PAGES: usize = 1_024;',
        'const MAX_CONVERSATION_REFRESH_PAGES: usize = usize::MAX;')
    Assert-Rejected {
        Assert-Boundary $runtime $cli $unboundedDesktop $migration $schema $coverage $compatibility $rfc
    } 'unbounded_desktop_refresh'
    $unpagedCli = $cli.Replace(
        'fn collect_runtime_conversation_page(',
        'fn collect_runtime_conversation_list(')
    Assert-Rejected {
        Assert-Boundary $runtime $unpagedCli $desktop $migration $schema $coverage $compatibility $rfc
    } 'unpaged_runtime_adapter'
    Write-Output 'runtime_ipc_conversation_pagination_migration_self_test=passed'
}

Write-Output 'runtime_ipc_conversation_pagination_migration_boundary=verified'
Write-Output 'active_runtime_ipc=postcard-v30-json-descriptor-v11'
Write-Output 'conversation_page_limit=64'
Write-Output 'desktop_refresh_page_limit=1024'
Write-Output 'legacy_v26_request_dispatch=false'
Write-Output 'peer_network_wire_changed=false'
Write-Output 'canonical_cbor_activated=false'
