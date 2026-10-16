[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$runtimePath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rfcPath = Join-Path $workspace 'docs\RFC-0172-m1146-runtime-ipc-completion-inventory.md'

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

function Assert-Unique {
    param([string[]]$Values, [string]$Label)
    $unique = @($Values | Sort-Object -Unique)
    if ($unique.Count -ne $Values.Count) {
        throw "$Label contains a duplicate"
    }
}

function Get-RustEnumVariants {
    param([string]$Source, [string]$EnumName)
    $lines = @($Source -split "`r?`n")
    $start = -1
    for ($index = 0; $index -lt $lines.Count; $index++) {
        if ($lines[$index] -ceq "pub enum $EnumName {") {
            $start = $index
            break
        }
    }
    if ($start -lt 0) {
        throw "Rust enum $EnumName is missing"
    }
    $depth = 1
    $variants = @()
    for ($index = $start + 1; $index -lt $lines.Count; $index++) {
        $line = $lines[$index]
        if ($depth -eq 1 -and $line -match '^    ([A-Z][A-Za-z0-9_]*)') {
            $variants += $Matches[1]
        }
        $depth += [regex]::Matches($line, '\{').Count
        $depth -= [regex]::Matches($line, '\}').Count
        if ($depth -eq 0) {
            return $variants
        }
        if ($depth -lt 0) {
            throw "Rust enum $EnumName has unbalanced braces"
        }
    }
    throw "Rust enum $EnumName is unterminated"
}

function Assert-CompletionInventory {
    param(
        [object]$Manifest,
        [object]$Schema,
        [object]$Coverage,
        [object]$Compatibility,
        [string]$RuntimeSource,
        [string]$Cddl,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.49' -or
        [string]$Manifest.status -cne 'completion-inventory-complete-canonical-target-complete' -or
        [string]$Manifest.surface -cne 'runtime-ipc' -or
        -not [bool]$Manifest.scope.canonical_target_complete -or
        -not [bool]$Manifest.scope.active_postcard_adapter_complete -or
        [bool]$Manifest.scope.canonical_cbor_active -or
        -not [bool]$Manifest.scope.changes_active_local_control -or
        [bool]$Manifest.scope.changes_peer_network_wire) {
        throw 'runtime IPC completion inventory scope changed'
    }

    $commands = @($Manifest.canonical.commands | ForEach-Object { [string]$_ })
    $results = @($Manifest.canonical.results | ForEach-Object { [string]$_ })
    $universalResults = @($Manifest.canonical.universal_results | ForEach-Object { [string]$_ })
    $pairs = @($Manifest.canonical.command_result_pairs)
    if ($commands.Count -ne 34 -or $results.Count -ne 33 -or $pairs.Count -ne 34) {
        throw 'canonical runtime IPC inventory cardinality changed'
    }
    Assert-Unique $commands 'canonical command inventory'
    Assert-Unique $results 'canonical result inventory'
    Assert-ExactSequence $universalResults @('error') 'universal canonical result inventory'
    Assert-ExactSequence @($Schema.covered_commands) $commands 'schema canonical command inventory'
    Assert-ExactSequence @($Schema.covered_results) $results 'schema canonical result inventory'

    $pairCommands = @($pairs | ForEach-Object { [string]$_.command })
    Assert-Unique $pairCommands 'canonical command/result pairing'
    Assert-ExactSequence $pairCommands $commands 'canonical command/result pairing order'
    $allowedModules = @(
        'core', 'read', 'contact', 'directory', 'queue', 'outbox', 'provider',
        'mailbox-status', 'mailbox-mutation', 'publication', 'publication-network',
        'publication-conflict', 'own-device-announcement', 'own-device-discovery',
        'own-device-roster', 'recovery-plan', 'recovery-execution', 'ticket-automation'
    )
    $allowedKinds = @('core-control', 'read', 'mutation', 'network-mutation')
    foreach ($pair in $pairs) {
        $command = [string]$pair.command
        $result = [string]$pair.result
        if ($result -cnotin $results -or [string]$pair.module -cnotin $allowedModules -or
            [string]$pair.kind -cnotin $allowedKinds) {
            throw "canonical pairing for '$command' is invalid"
        }
        if ($Cddl -notmatch ('\[' + [regex]::Escape($command) + ':')) {
            throw "canonical CDDL is missing command '$command'"
        }
        if ($Cddl -notmatch ('\[' + [regex]::Escape($result) + ':')) {
            throw "canonical CDDL is missing result '$result'"
        }
    }
    $pairedResults = @($pairs | ForEach-Object { [string]$_.result }) + $universalResults
    Assert-Unique @($pairedResults | Sort-Object -Unique) 'canonical unique result projection'
    Assert-ExactSequence @($pairedResults | Sort-Object -Unique) @($results | Sort-Object -Unique) `
        'canonical result coverage'

    $activeCommands = @($Manifest.active_postcard.commands)
    $activeResponses = @($Manifest.active_postcard.responses)
    if ([int]$Manifest.active_postcard.version -ne 29 -or
        [string]$Manifest.active_postcard.descriptor_signature_domain -cne
            'kilogram:runtime-ipc-descriptor:v10\0' -or
        $RuntimeSource.IndexOf('const IPC_VERSION: u8 = 29;', [StringComparison]::Ordinal) -lt 0 -or
        $RuntimeSource.IndexOf('kilogram:runtime-ipc-descriptor:v10\0', [StringComparison]::Ordinal) -lt 0) {
        throw 'active Postcard runtime IPC identity changed'
    }
    $activeCommandNames = @($activeCommands | ForEach-Object { [string]$_.variant })
    $activeResponseNames = @($activeResponses | ForEach-Object { [string]$_.variant })
    Assert-Unique $activeCommandNames 'active Postcard command inventory'
    Assert-Unique $activeResponseNames 'active Postcard response inventory'
    Assert-ExactSequence (Get-RustEnumVariants $RuntimeSource 'RuntimeIpcCommand') `
        $activeCommandNames 'active Postcard command source inventory'
    Assert-ExactSequence (Get-RustEnumVariants $RuntimeSource 'RuntimeIpcResponse') `
        $activeResponseNames 'active Postcard response source inventory'

    $allowedDispositions = @(
        'canonical-target',
        'canonical-bounded-replacement'
    )
    foreach ($entry in $activeCommands) {
        if ([string]$entry.disposition -cnotin $allowedDispositions) {
            throw "active command '$($entry.variant)' has an unknown disposition"
        }
        foreach ($target in @($entry.canonical_commands)) {
            if ([string]$target -cnotin $commands) {
                throw "active command '$($entry.variant)' references unknown canonical command '$target'"
            }
        }
    }
    foreach ($entry in $activeResponses) {
        if ([string]$entry.disposition -cnotin $allowedDispositions) {
            throw "active response '$($entry.variant)' has an unknown disposition"
        }
        foreach ($target in @($entry.canonical_results)) {
            if ([string]$target -cnotin $results) {
                throw "active response '$($entry.variant)' references unknown canonical result '$target'"
            }
        }
    }

    $expectedBlockers = @()
    Assert-ExactSequence @($Manifest.postcard_only_activation_blockers) $expectedBlockers `
        'Postcard-only activation blockers'
    $remaining = @()
    Assert-ExactSequence @($Manifest.remaining_canonical_work) $remaining `
        'completion inventory remaining work'
    Assert-ExactSequence @($Schema.remaining_runtime_ipc_surface) $remaining `
        'canonical schema remaining work'

    if ([string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.status -cne 'candidate-not-active' -or
        [string]$Schema.activation.current_runtime_encoding -cne
            'postcard-v29-with-json-descriptor-v10' -or
        -not [bool]$Schema.activation.current_runtime_changed -or
        @($Schema.semantic_rules).Count -ne 179) {
        throw 'canonical runtime IPC schema did not record the exhaustive inventory'
    }
    $runtimeSurface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([string]$Coverage.milestone -cne 'M1.1.50' -or $runtimeSurface.Count -ne 1 -or
        [string]$runtimeSurface[0].coverage -cne 'complete' -or
        'exhaustive-command-result-and-active-adapter-inventory' -cnotin
            @($runtimeSurface[0].covered) -or
        'foreground-contact-ticket-automation-policy-and-status' -cnotin
            @($runtimeSurface[0].covered) -or
        'active-postcard-endpoint-bootstrap-helper-retirement' -cnotin
            @($runtimeSurface[0].covered) -or
        'active-postcard-legacy-central-https-mailbox-command-retirement' -cnotin
            @($runtimeSurface[0].covered) -or
        (@($runtimeSurface[0].remaining) -join ',') -cne ($remaining -join ',')) {
        throw 'canonical coverage did not retain the explicit remaining active adapter work'
    }
    $compatibilitySurface = @($Compatibility.surfaces | Where-Object {
        [string]$_.id -ceq 'runtime-ipc'
    })
    if ($compatibilitySurface.Count -ne 1 -or
        [string]$compatibilitySurface[0].encoding -cne 'postcard' -or
        [string]$compatibilitySurface[0].compatibility -cne 'exact-version' -or
        [int](@($compatibilitySurface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'IPC_VERSION'
        })[0].value) -ne 29) {
        throw 'protocol compatibility registry no longer describes active IPC v29'
    }

    $activationRequirements = @(
        'assign-explicit-canonical-local-control-protocol-identifier-and-version',
        'implement-parallel-cbor-reader-and-writer-with-downgrade-tests',
        'implement-atomic-durable-adapters-and-idempotency-records',
        'run-sustained-coverage-guided-fuzzing',
        'complete-independent-hpke-conformance',
        'collect-fresh-desktop-and-cli-multi-device-field-evidence'
    )
    Assert-ExactSequence @($Manifest.activation_requirements) $activationRequirements `
        'canonical runtime IPC activation requirements'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'inventory is exhaustive',
        'does not make the canonical target complete',
        'ticket automation policy and status',
        'endpoint-announcement bootstrap helpers',
        'legacy central-HTTPS mailbox commands',
        'seven Postcard-only activation blockers',
        'does not activate canonical CBOR',
        'does not change any active wire',
        'fresh field evidence'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0172 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $schemaPath, $coveragePath, $compatibilityPath,
    $runtimePath, $cddlPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "runtime IPC completion inventory file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$runtimeSource = Get-Content -LiteralPath $runtimePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-CompletionInventory $manifest $schema $coverage $compatibility $runtimeSource $cddl $rfc

$missingActive = Copy-JsonObject $manifest
$missingActive.active_postcard.commands = @($missingActive.active_postcard.commands | Select-Object -Skip 1)
Assert-Rejected {
    Assert-CompletionInventory $missingActive $schema $coverage $compatibility $runtimeSource $cddl $rfc
} 'missing_active_postcard_command'

$unpaired = Copy-JsonObject $manifest
$unpaired.canonical.command_result_pairs[0].result = 'untracked-result'
Assert-Rejected {
    Assert-CompletionInventory $unpaired $schema $coverage $compatibility $runtimeSource $cddl $rfc
} 'untracked_canonical_result'

$forgedBlocker = Copy-JsonObject $manifest
$forgedBlocker.postcard_only_activation_blockers = @('CreateMailboxCapability')
Assert-Rejected {
    Assert-CompletionInventory $forgedBlocker $schema $coverage $compatibility $runtimeSource $cddl $rfc
} 'forged_postcard_activation_blocker'

$falseCompletion = Copy-JsonObject $manifest
$falseCompletion.scope.canonical_target_complete = $false
Assert-Rejected {
    Assert-CompletionInventory $falseCompletion $schema $coverage $compatibility $runtimeSource $cddl $rfc
} 'false_canonical_completion'

$untrackedSource = [regex]::Replace(
    $runtimeSource,
    '(?m)^    Ping,$',
    '    UntrackedCommand,',
    1
)
Assert-Rejected {
    Assert-CompletionInventory $manifest $schema $coverage $compatibility $untrackedSource $cddl $rfc
} 'untracked_active_postcard_variant'

Write-Output 'runtime_ipc_completion_inventory_boundary=verified'
Write-Output 'canonical_commands=34'
Write-Output 'canonical_results=33'
Write-Output 'active_postcard_commands=29'
Write-Output 'active_postcard_responses=28'
Write-Output 'postcard_only_activation_blockers=0'
Write-Output 'canonical_target_complete=true'
Write-Output 'canonical_cbor_activated=false'
Write-Output 'active_wire_changed=false'
