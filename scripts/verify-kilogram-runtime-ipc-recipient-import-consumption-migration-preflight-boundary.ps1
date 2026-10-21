[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'RUNTIME-IPC-RECIPIENT-IMPORT-CONSUMPTION-MIGRATION.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-PLAN-SESSION-CACHE.json'
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0267-m1243-runtime-ipc-recipient-import-consumption-migration-preflight.md'

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
    param([object]$Manifest, [object]$Predecessor, [string]$Runtime,
          [object]$Completion, [object]$Schema, [object]$Compatibility,
          [string]$Rfc, [int]$LiveBindingGateCount)

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.43' -or
        [string]$Manifest.status -cne 'preflight-locked-inactive' -or
        [string]$Manifest.surface -cne 'runtime-ipc' -or
        [string]$Manifest.encoding -cne 'postcard' -or
        [string]$Manifest.predecessor -cne 'M1.2.42') {
        throw 'migration preflight identity changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.42' -or
        [string]$Predecessor.status -cne 'canonical-recipient-import-consumption-plan-session-cache-verified') {
        throw 'migration preflight predecessor changed'
    }
    if ([int]$Manifest.previous.ipc_version -ne 33 -or
        [string]$Manifest.previous.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        [bool]$Manifest.previous.recipient_import_consumption_command_present -or
        [bool]$Manifest.previous.recipient_import_consumption_response_present -or
        [int]$Manifest.target.ipc_version -ne 34 -or
        [string]$Manifest.target.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        [string]$Manifest.target.command_variant -cne 'RecipientImportConsumption { request: Vec<u8> }' -or
        [string]$Manifest.target.response_variant -cne 'RecipientImportConsumption { signed_response: Vec<u8> }' -or
        -not [bool]$Manifest.target.append_only_enum_extension) {
        throw 'previous or target IPC generation changed'
    }
    $migration = $Manifest.migration
    if ([string]$migration.strategy -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [string]$migration.legacy_descriptor -cne 'rejected-before-connect' -or
        [string]$migration.legacy_request -cne 'rejected-before-actor-dispatch' -or
        [bool]$migration.dual_reader -or [bool]$migration.format_sniffing -or
        [bool]$migration.silent_downgrade -or [bool]$migration.hybrid_version_domain_state_allowed) {
        throw 'migration fail-closed policy changed'
    }
    foreach ($property in @(
        'runtime_command_enum', 'runtime_response_enum', 'ipc_version',
        'descriptor_signature_domain', 'canonical_runtime_ipc_schema',
        'canonical_runtime_ipc_completion_inventory',
        'protocol_compatibility_registry', 'all_live_binding_gates',
        'preserve_historical_manifest_claims'
    )) {
        if (-not [bool]$Manifest.required_atomic_update.$property) {
            throw "atomic migration member '$property' is no longer required"
        }
    }
    if ([int]$Manifest.required_atomic_update.observed_live_binding_gate_files -ne 58 -or
        $LiveBindingGateCount -ne 58) {
        throw 'live version/domain binding gate inventory changed without review'
    }
    foreach ($property in @(
        'active_runtime_changed', 'command_registered', 'runtime_handler',
        'plan_session_cache_owned_by_runtime', 'cli_or_desktop_control',
        'network', 'public_default_changed'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "inactive preflight unexpectedly activated '$property'"
        }
    }
    if ([string]$Manifest.activation_boundary.public_default -cne 'legacy-only') {
        throw 'public default changed in migration preflight'
    }

    Assert-ContainsAll $Runtime @(
        'const IPC_VERSION: u8 = 34;',
        'const DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ipc-descriptor:v15\0";',
        'RecipientImportConsumption {',
        'request: Vec<u8>',
        'signed_response: Vec<u8>'
    ) 'active successor runtime IPC source'
    if ($Runtime.IndexOf('const IPC_VERSION: u8 = 33;', [StringComparison]::Ordinal) -ge 0) {
        throw 'legacy IPC version remained active after successor activation'
    }
    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v15\0' -or
        [string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v34-with-json-descriptor-v15') {
        throw 'canonical runtime ledgers do not describe the activated successor'
    }
    $surface = @($Compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34 -or
        [string](@($surface[0].bindings | Where-Object {
            $_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v15\0') {
        throw 'protocol compatibility registry does not describe the activated successor'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'incompatible byte contract', 'version 34', 'domain `kilogram:runtime-ipc-descriptor:v15\0`',
        'exact-version hard cut', 'every hybrid `33/v15` or `34/v14` state is invalid',
        '58 boundary files', 'Historical manifests continue to describe',
        'M1.2.43 changes no executable bytes', 'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "migration preflight RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $predecessorPath, $runtimePath, $completionPath,
    $schemaPath, $compatibilityPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$versionGates = @(Get-ChildItem -LiteralPath (Join-Path $workspace 'scripts') -Filter '*.ps1' |
    Where-Object { $_.FullName -cne $PSCommandPath -and
        $_.Name -cne 'verify-kilogram-runtime-ipc-recipient-import-consumption-envelope-boundary.ps1' -and
        (Get-Content -LiteralPath $_.FullName -Raw).IndexOf(
        'const IPC_VERSION: u8 = 34;', [StringComparison]::Ordinal) -ge 0 })
$domainGates = @(Get-ChildItem -LiteralPath (Join-Path $workspace 'scripts') -Filter '*.ps1' |
    Where-Object { $_.FullName -cne $PSCommandPath -and
        $_.Name -cne 'verify-kilogram-runtime-ipc-recipient-import-consumption-envelope-boundary.ps1' -and
        (Get-Content -LiteralPath $_.FullName -Raw).IndexOf(
        'kilogram:runtime-ipc-descriptor:v15\0', [StringComparison]::Ordinal) -ge 0 })
$liveBindingGateCount = @($versionGates.FullName + $domainGates.FullName | Sort-Object -Unique).Count
$arguments = @($manifest, $predecessor, $runtime, $completion, $schema,
    $compatibility, $rfc, $liveBindingGateCount)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.target.ipc_version = 33
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.target.descriptor_signature_domain = 'kilogram:runtime-ipc-descriptor:v14\0'
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.migration.dual_reader = $true
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.migration.silent_downgrade = $true
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.required_atomic_update.protocol_compatibility_registry = $false
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.required_atomic_update.observed_live_binding_gate_files = 57
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.command_registered = $true
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.network = $true
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changedRuntime = $runtime.Replace('const IPC_VERSION: u8 = 34;', 'const IPC_VERSION: u8 = 33;')
    $mutations += { Assert-Boundary $manifest $predecessor $changedRuntime $completion $schema $compatibility $rfc $liveBindingGateCount }
    $changed = Copy-JsonObject $manifest; $changed.required_atomic_update.preserve_historical_manifest_claims = $false
    $mutations += { Assert-Boundary $changed $predecessor $runtime $completion $schema $compatibility $rfc $liveBindingGateCount }
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

Write-Output 'runtime_ipc_recipient_import_consumption_migration_preflight_boundary=verified'
Write-Output 'active_runtime_ipc=postcard-v34-json-descriptor-v15'
Write-Output 'target_runtime_ipc=postcard-v34-json-descriptor-v15'
Write-Output 'preflight_fulfilled=true'
Write-Output 'network_executed=false'
