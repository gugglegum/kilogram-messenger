[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RUNTIME-QUARANTINE-OBSERVABILITY.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$storePath = Join-Path $workspace 'crates\kilogram-peer-capability-store\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0233-m1209-runtime-quarantine-observability.md'

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

function Assert-ExactStrings {
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

function Get-SourceSlice {
    param([string]$Source, [string]$Start, [string]$End, [string]$Label)
    $startIndex = $Source.IndexOf($Start, [StringComparison]::Ordinal)
    if ($startIndex -lt 0) { throw "$Label start marker is missing" }
    $endIndex = $Source.IndexOf($End, $startIndex + $Start.Length, [StringComparison]::Ordinal)
    if ($endIndex -lt 0) { throw "$Label end marker is missing" }
    return $Source.Substring($startIndex, $endIndex - $startIndex)
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Ipc,
        [string]$Store,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.9' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-runtime-quarantine-observability-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'runtime quarantine observability identity or scope changed'
    }

    $runtimeIpc = $Manifest.runtime_ipc
    if ([string]$runtimeIpc.operation -cne 'read-only-page' -or
        [string]$runtimeIpc.command -cne 'PeerCapabilityQuarantinePage' -or
        [string]$runtimeIpc.response -cne 'PeerCapabilityQuarantinePage' -or
        [int]$runtimeIpc.version -ne 31 -or
        [string]$runtimeIpc.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v12\0' -or
        [int]$runtimeIpc.previous_version -ne 30 -or
        [string]$runtimeIpc.previous_descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v11\0' -or
        [string]$runtimeIpc.migration -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [bool]$runtimeIpc.dual_reader -or
        [bool]$runtimeIpc.mutation_or_recovery_authority) {
        throw 'runtime quarantine IPC boundary changed'
    }

    $projection = $Manifest.bounded_projection
    if ([int]$projection.maximum_page_items -ne 64 -or
        [int]$projection.maximum_runtime_contacts -ne 256 -or
        [int]$projection.maximum_pending_queue_items -ne 4096 -or
        [string]$projection.cursor -cne 'opaque-contact-id-exclusive' -or
        [string]$projection.ordering -cne 'ascending-opaque-contact-id' -or
        -not [bool]$projection.durable_reason_vocabulary_is_bounded -or
        -not [bool]$projection.restart_reloads_durable_quarantine -or
        [bool]$projection.restart_does_not_reconstruct_process_local_suppression -ne $true) {
        throw 'runtime quarantine bounded projection changed'
    }
    Assert-ExactStrings @($projection.distinguishes) @(
        'durable-capability-quarantine',
        'process-local-scheduler-suppression',
        'ordinary-network-retry'
    ) 'runtime quarantine classes'

    $storage = $Manifest.storage_boundary
    if (-not [bool]$storage.opens_capability_store_read_only -or
        -not [bool]$storage.missing_store_is_valid_empty_state -or
        [bool]$storage.missing_store_creates_directory -or
        [bool]$storage.missing_store_creates_database -or
        [bool]$storage.observation_mutates_database -or
        -not [bool]$storage.wrong_owner_or_corrupt_store_fails_closed) {
        throw 'runtime quarantine read-only storage boundary changed'
    }

    $privacy = $Manifest.privacy_boundary
    if (-not [bool]$privacy.exposes_opaque_contact_id -or
        [bool]$privacy.exposes_raw_account_id -or [bool]$privacy.exposes_raw_device_id -or
        [bool]$privacy.exposes_queue_id -or [bool]$privacy.exposes_ticket_or_endpoint -or
        [bool]$privacy.exposes_ip_or_relay_url -or [bool]$privacy.exposes_message_or_ciphertext -or
        [bool]$privacy.exposes_filesystem_path -or [bool]$privacy.exposes_secret_or_capability_bytes) {
        throw 'runtime quarantine privacy boundary changed'
    }

    $desktopContract = $Manifest.desktop
    if (-not [bool]$desktopContract.loads_after_authenticated_connect -or
        -not [bool]$desktopContract.refreshes_after_runtime_status_refresh -or
        -not [bool]$desktopContract.manual_refresh -or
        -not [bool]$desktopContract.labels_view_read_only -or
        -not [bool]$desktopContract.shows_durable_scheduler_and_network_counts_separately -or
        [bool]$desktopContract.clear_action_present -or [bool]$desktopContract.retry_action_present -or
        [bool]$desktopContract.downgrade_action_present -or [bool]$desktopContract.recovery_action_present) {
        throw 'runtime quarantine desktop authority changed'
    }
    $activation = $Manifest.activation_boundary
    if ([bool]$activation.automatic_quarantine_clear_enabled -or
        [bool]$activation.remote_quarantine_clear_enabled -or
        [bool]$activation.observer_can_retry_network_work -or
        [bool]$activation.observer_can_downgrade_capability_floor -or
        [bool]$activation.public_canonical_rollout_enabled) {
        throw 'runtime quarantine observer gained mutation authority'
    }

    $commands = @($Completion.canonical.commands)
    $results = @($Completion.canonical.results)
    $pairs = @($Completion.canonical.command_result_pairs)
    $pair = @($pairs | Where-Object { [string]$_.command -ceq 'peer-capability-quarantine-page' })
    if ($commands.Count -ne 39 -or $results.Count -ne 38 -or $pairs.Count -ne 39 -or
        $pair.Count -ne 1 -or [string]$pair[0].result -cne 'peer-capability-quarantine-page-result' -or
        [string]$pair[0].module -cne 'read' -or [string]$pair[0].kind -cne 'read' -or
        [int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.active_postcard.commands).Count -ne 34 -or
        @($Completion.active_postcard.responses).Count -ne 33 -or
        'PeerCapabilityQuarantinePage' -cnotin @($Completion.active_postcard.commands.variant) -or
        'PeerCapabilityQuarantinePage' -cnotin @($Completion.active_postcard.responses.variant)) {
        throw 'runtime quarantine completion inventory changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard-v34-with-json-descriptor-v15' -or
        [int]$Schema.limits.maximum_peer_capability_page_size -ne 64 -or
        [int]$Schema.limits.maximum_peer_capability_contacts -ne 256 -or
        [int]$Schema.limits.maximum_peer_capability_queue_items -ne 4096 -or
        @($Schema.covered_commands).Count -ne 39 -or @($Schema.covered_results).Count -ne 38 -or
        @($Schema.semantic_rules).Count -ne 192 -or
        'peer-capability-quarantine-page' -cnotin @($Schema.covered_commands) -or
        'peer-capability-quarantine-page-result' -cnotin @($Schema.covered_results)) {
        throw 'runtime quarantine canonical schema changed'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34 -or
        [string](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v15\0') {
        throw 'runtime quarantine compatibility binding changed'
    }

    Assert-ContainsAll $Ipc @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15\0',
        'pub const MAX_RUNTIME_IPC_PEER_CAPABILITY_PAGE_SIZE: u16 = 64;',
        'pub const MAX_RUNTIME_IPC_PEER_CAPABILITY_CONTACTS: u16 = 256;',
        'pub const MAX_RUNTIME_IPC_PEER_CAPABILITY_QUEUE_ITEMS: u16 = 4_096;',
        'pub const MAX_RUNTIME_IPC_PEER_CAPABILITY_CANDIDATES_PER_CONTACT: u8 = 4;',
        'PeerCapabilityQuarantinePage {',
        'pub struct RuntimeIpcPeerCapabilityCursor',
        'pub struct RuntimeIpcPeerCapabilityStatusItem',
        'pub struct RuntimeIpcPeerCapabilityPage',
        'peer_capability_page_is_bounded_monotonic_and_privacy_preserving',
        'version_32_request_is_rejected_before_actor_dispatch'
    ) 'runtime IPC source'
    Assert-ContainsAll $Ipc @(
        'self.total_relevant_contact_count <= MAX_RUNTIME_IPC_PEER_CAPABILITY_CONTACTS',
        'self.pending_queue_count <= MAX_RUNTIME_IPC_PEER_CAPABILITY_QUEUE_ITEMS',
        '<= MAX_RUNTIME_IPC_PEER_CAPABILITY_CANDIDATES_PER_CONTACT',
        '.checked_add(self.network_retry_queue_count)',
        'MAX_RUNTIME_IPC_PEER_CAPABILITY_CONTACTS + 1',
        'MAX_RUNTIME_IPC_PEER_CAPABILITY_QUEUE_ITEMS;',
        'MAX_RUNTIME_IPC_PEER_CAPABILITY_CANDIDATES_PER_CONTACT + 1'
    ) 'runtime IPC quarantine bounds'
    $item = Get-SourceSlice $Ipc 'pub struct RuntimeIpcPeerCapabilityStatusItem {' 'impl RuntimeIpcPeerCapabilityStatusItem {' 'peer capability IPC item'
    foreach ($forbidden in @(
        'account_id', 'device_id', 'queue_id', 'ticket', 'endpoint', 'message',
        'ciphertext', 'filesystem', 'secret', 'capability_bytes'
    )) {
        if ($item.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "peer capability IPC item exposes forbidden field '$forbidden'"
        }
    }

    Assert-ContainsAll $Store @(
        'pub struct PeerCapabilityReadStore',
        'pub fn open_existing(',
        'PeerCapabilityStoreError::StoreNotInitialized',
        'ReadOnlyDatabase::open(database_path)',
        'verify_database_metadata(&database, owner_device_id)?;',
        'verify_all_records_from_database(&database, owner_device_id)?;',
        'read_only_open_refuses_absent_store_without_creating_state',
        'read_only_open_observes_durable_quarantine_without_mutation'
    ) 'peer capability read store'
    $readStore = Get-SourceSlice $Store 'impl PeerCapabilityReadStore {' 'impl PeerCapabilityStore {' 'peer capability read store implementation'
    foreach ($forbidden in @('create_dir_all', 'Database::create', 'initialize_database', '.write(')) {
        if ($readStore.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "peer capability read store contains forbidden mutation '$forbidden'"
        }
    }

    Assert-ContainsAll $Cli @(
        'fn collect_runtime_peer_capability_quarantine_page(',
        'PeerCapabilityReadStore::open_existing(',
        'scheduler_suppressed_contact_count',
        'network_retry_contact_count',
        'RuntimeIpcCommand::PeerCapabilityQuarantinePage { cursor, limit }',
        'runtime_peer_capability_observer_does_not_create_an_absent_store',
        'runtime_outbox_quarantine_blocks_before_materialization_and_recovers_after_restart'
    ) 'runtime quarantine actor'
    Assert-ContainsAll $Desktop @(
        'RefreshPeerCapabilityStatus',
        'WorkerRequest::PeerCapabilityStatus',
        'RuntimeIpcCommand::PeerCapabilityQuarantinePage',
        'Peer capability safety (read only)',
        'This panel grants no clear, retry, downgrade, or recovery authority.'
    ) 'runtime quarantine desktop'
    foreach ($forbidden in @(
        'ClearPeerCapabilityQuarantine', 'RetryPeerCapabilityQuarantine',
        'DowngradePeerCapabilityFloor', 'RecoverPeerCapabilityQuarantine'
    )) {
        if ($Ipc.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0 -or
            $Desktop.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "runtime quarantine observer contains forbidden authority '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'public default remains `legacy-only`',
        'never creates a directory or database',
        'exclusive opaque-contact cursor',
        'does not contain Account or Device IDs',
        'version 31 and descriptor signature domain v12',
        'offers no clear, retry, downgrade, re-enrollment, or recovery control'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "runtime quarantine RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $completionPath, $schemaPath, $compatibilityPath, $ipcPath,
    $storePath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $completion $schema $compatibility $ipc $store $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.bounded_projection.maximum_page_items = 65
    $mutations += [pscustomobject]@{ Label = 'unbounded_page'; Manifest = $changed; Ipc = $ipc; Store = $store; Desktop = $desktop }

    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.mutation_or_recovery_authority = $true
    $mutations += [pscustomobject]@{ Label = 'recovery_authority'; Manifest = $changed; Ipc = $ipc; Store = $store; Desktop = $desktop }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_boundary.exposes_raw_device_id = $true
    $mutations += [pscustomobject]@{ Label = 'raw_device_id'; Manifest = $changed; Ipc = $ipc; Store = $store; Desktop = $desktop }

    $changed = Copy-JsonObject $manifest
    $changed.public_default = 'dual-reader-prefer-canonical'
    $mutations += [pscustomobject]@{ Label = 'public_default_change'; Manifest = $changed; Ipc = $ipc; Store = $store; Desktop = $desktop }

    $mutations += [pscustomobject]@{
        Label = 'creating_read_store'; Manifest = $manifest; Ipc = $ipc
        Store = $store.Replace('ReadOnlyDatabase::open(database_path)', 'Database::create(database_path)')
        Desktop = $desktop
    }
    $mutations += [pscustomobject]@{
        Label = 'raw_id_field'; Manifest = $manifest
        Ipc = $ipc.Replace(
            'pub struct RuntimeIpcPeerCapabilityStatusItem {',
            "pub struct RuntimeIpcPeerCapabilityStatusItem {`r`n    pub raw_device_id: DeviceId,"
        )
        Store = $store; Desktop = $desktop
    }
    $mutations += [pscustomobject]@{
        Label = 'desktop_clear_action'; Manifest = $manifest; Ipc = $ipc; Store = $store
        Desktop = "$desktop`r`nenum Injected { ClearPeerCapabilityQuarantine }"
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            Assert-Contract $mutation.Manifest $completion $schema $compatibility $mutation.Ipc $mutation.Store $cli $mutation.Desktop $rfc
        } catch {
            Write-Output "$($mutation.Label)=rejected"
            $rejected++
            continue
        }
        throw "$($mutation.Label) mutation was unexpectedly accepted"
    }
    if ($rejected -ne $mutations.Count) { throw 'runtime quarantine mutation matrix was incomplete' }
    Write-Output 'runtime_quarantine_observability_negative_mutations=7/7'
    Write-Output 'canonical_sync_v2_runtime_quarantine_observability_self_test=passed'
    exit 0
}

Write-Output 'runtime_ipc_version=31'
Write-Output 'runtime_ipc_descriptor_domain=v12'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_runtime_quarantine_observability_boundary=verified'
