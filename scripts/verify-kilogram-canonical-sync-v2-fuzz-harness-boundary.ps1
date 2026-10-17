[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json'
$toolchainManifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json'
$mutationManifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DECODER-MUTATIONS.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$protocolCargoPath = Join-Path $workspace 'crates\kilogram-protocol\Cargo.toml'
$protocolLibPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$dispatcherPath = Join-Path $workspace 'crates\kilogram-protocol\src\decoder_fuzzing.rs'
$seedTestPath = Join-Path $workspace 'crates\kilogram-protocol\tests\canonical_sync_v2_fuzz_seeds.rs'
$fuzzCargoPath = Join-Path $workspace 'fuzz\Cargo.toml'
$fuzzLockPath = Join-Path $workspace 'fuzz\Cargo.lock'
$fuzzTargetPath = Join-Path $workspace 'fuzz\fuzz_targets\canonical_sync_v2_decoders.rs'
$gitignorePath = Join-Path $workspace '.gitignore'
$preparePath = Join-Path $PSScriptRoot 'prepare-kilogram-canonical-sync-v2-fuzz-corpus.ps1'
$runnerPath = Join-Path $PSScriptRoot 'invoke-kilogram-canonical-sync-v2-fuzz.ps1'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$activeTransportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0200-m1173-canonical-sync-v2-coverage-fuzz-harness.md'
$toolchainRfcPath = Join-Path $workspace 'docs\RFC-0201-m1174-pinned-fuzz-toolchain.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$ToolchainManifest,
        [object]$MutationManifest,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$ProtocolCargo,
        [string]$ProtocolLib,
        [string]$Dispatcher,
        [string]$SeedTest,
        [string]$FuzzCargo,
        [string]$FuzzLock,
        [string]$FuzzTarget,
        [string]$GitIgnore,
        [string]$Preparer,
        [string]$Runner,
        [string]$Wire,
        [string]$ActiveTransport,
        [string]$Cli,
        [string]$Rfc,
        [string]$ToolchainRfc
    )

    $aggregateStatus = 'canonical-sync-v2-coverage-fuzz-harness-verified-campaign-not-run'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.74' -or
        [string]$Manifest.status -cne 'instrumented-fuzz-target-built-smoke-pending' -or
        [string]$Manifest.scope -cne 'history-compatible-canonical-sync-v2-strict-decoders' -or
        [string]$Manifest.target -cne 'canonical_sync_v2_decoders') {
        throw 'sync-v2 fuzz campaign identity changed'
    }
    if ([string]$Manifest.engine.driver -cne 'cargo-fuzz' -or
        [string]$Manifest.engine.minimum_driver_version -cne '0.12.0' -or
        [string]$Manifest.engine.toolchain_contract_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json' -or
        [string]$Manifest.engine.backend -cne 'libFuzzer' -or
        -not [bool]$Manifest.engine.nightly_toolchain_required -or
        -not [bool]$Manifest.engine.address_sanitizer_required -or
        -not [bool]$Manifest.engine.coverage_feedback_required) {
        throw 'sync-v2 fuzz engine contract changed'
    }
    if ([UInt64]$ToolchainManifest.schema_version -ne 1 -or
        [string]$ToolchainManifest.milestone -cne 'M1.1.74' -or
        [string]$ToolchainManifest.status -cne 'pinned-instrumented-fuzz-toolchain-verified' -or
        [string]$ToolchainManifest.campaign_manifest -cne 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json' -or
        [string]$ToolchainManifest.target -cne 'canonical_sync_v2_decoders' -or
        [string]$ToolchainManifest.host.os -cne 'windows' -or
        [string]$ToolchainManifest.host.target_triple -cne 'x86_64-pc-windows-msvc') {
        throw 'sync-v2 fuzz toolchain identity changed'
    }
    if ([string]$ToolchainManifest.toolchain.rustup_name -cne 'nightly' -or
        [string]$ToolchainManifest.toolchain.rustc_release -cne '1.101.0-nightly' -or
        [string]$ToolchainManifest.toolchain.rustc_commit_hash -cne
            '5d8fec0669a64607db9ccbf663e7c23125bf4748' -or
        [string]$ToolchainManifest.toolchain.rustc_commit_date -cne '2026-10-16' -or
        [string]$ToolchainManifest.toolchain.llvm_version -cne '23.1.4' -or
        [string]$ToolchainManifest.toolchain.cargo_fuzz_version -cne '0.13.2' -or
        [string]$ToolchainManifest.toolchain.libfuzzer_sys_version -cne '0.4.13' -or
        [string]$ToolchainManifest.toolchain.dependency_lock -cne 'fuzz/Cargo.lock') {
        throw 'sync-v2 fuzz toolchain pin changed'
    }
    if ([string]$ToolchainManifest.windows_address_sanitizer.component_id -cne
            'Microsoft.VisualStudio.Component.VC.ASAN' -or
        [string]$ToolchainManifest.windows_address_sanitizer.runtime_dll -cne
            'clang_rt.asan_dynamic-x86_64.dll' -or
        [string]$ToolchainManifest.windows_address_sanitizer.runtime_import_library -cne
            'clang_rt.asan_dynamic_runtime_thunk-x86_64.lib' -or
        -not [bool]$ToolchainManifest.windows_address_sanitizer.runtime_path_injected_for_child_process -or
        -not [bool]$ToolchainManifest.windows_address_sanitizer.detected) {
        throw 'sync-v2 fuzz Windows instrumentation prerequisite changed'
    }
    if (-not [bool]$ToolchainManifest.observed.locked_dependencies_resolved -or
        -not [bool]$ToolchainManifest.observed.instrumented_target_built -or
        [UInt64]$ToolchainManifest.observed.build_exit_code -ne 0 -or
        [UInt64]$ToolchainManifest.observed.binary_bytes -ne 2178560 -or
        [string]$ToolchainManifest.observed.binary_sha256 -cne
            '1c53c755571bcdaf2399fd82b1c54c81850253815e6e8a6add9041ee60d5b03a' -or
        [bool]$ToolchainManifest.observed.smoke_campaign_completed -or
        [bool]$ToolchainManifest.observed.sustained_campaign_completed -or
        [bool]$ToolchainManifest.observed.coverage_report_generated -or
        [string]$ToolchainManifest.next_gate -cne 'instrumented-smoke-campaign') {
        throw 'sync-v2 fuzz toolchain evidence changed'
    }
    if ([bool]$ToolchainManifest.activation.active_transport_authorized -or
        [bool]$ToolchainManifest.activation.candidate_identifier_registered -or
        [bool]$ToolchainManifest.activation.candidate_identifier_dialed -or
        [bool]$ToolchainManifest.activation.active_runtime_changed) {
        throw 'sync-v2 fuzz toolchain preparation activated production transport'
    }

    if ([UInt64]$Manifest.selector.prefix_bytes -ne 1 -or
        [UInt64]$Manifest.selector.target_count -ne 13) {
        throw 'sync-v2 fuzz selector boundary changed'
    }
    Assert-ExactStrings @($Manifest.selector.mapping) @(
        'event-key', 'event-carrier', 'inventory', 'event-batch', 'diff',
        'complete', 'pause', 'paused', 'rejected', 'client-frame',
        'server-frame', 'session-checkpoint', 'responder-checkpoint'
    ) 'sync-v2 fuzz selector mapping'

    $seeds = @($Manifest.seeds)
    if ([UInt64]$Manifest.seed_count -ne 20 -or $seeds.Count -ne 20) {
        throw 'sync-v2 fuzz seed inventory changed'
    }
    $allowedDocuments = @(
        'CANONICAL-SYNC-HISTORY-V2-VECTORS.json',
        'CANONICAL-SYNC-V2-VECTORS.json',
        'CANONICAL-SYNC-SESSION-V2-VECTORS.json',
        'CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json'
    )
    $seenIds = @{}
    foreach ($seed in $seeds) {
        $id = [string]$seed.id
        if ($id -cnotmatch '^[a-z0-9]+(?:-[a-z0-9]+)*$' -or $seenIds.ContainsKey($id)) {
            throw "invalid or duplicate fuzz seed ID: $id"
        }
        $seenIds[$id] = $true
        if ([UInt64]$seed.selector -ge 13 -or
            [UInt64]$seed.input_bytes -ne ([UInt64]$seed.payload_bytes + 1) -or
            [UInt64]$seed.input_bytes -gt [UInt64]$Manifest.campaign_contract.maximum_input_bytes -or
            @($seed.parts).Count -eq 0) {
            throw "invalid fuzz seed contract: $id"
        }
        foreach ($part in @($seed.parts)) {
            if ([string]$part.document -cnotin $allowedDocuments -or
                -not ([string]$part.pointer).StartsWith('/')) {
                throw "invalid fuzz seed source: $id"
            }
        }
    }

    if ([UInt64]$Manifest.campaign_contract.minimum_wall_clock_seconds -ne 28800 -or
        [UInt64]$Manifest.campaign_contract.minimum_executions -ne 1000000 -or
        [UInt64]$Manifest.campaign_contract.workers -ne 1 -or
        [UInt64]$Manifest.campaign_contract.jobs -ne 1 -or
        [UInt64]$Manifest.campaign_contract.maximum_input_bytes -ne 8388609 -or
        [UInt64]$Manifest.campaign_contract.per_input_timeout_seconds -ne 10 -or
        [UInt64]$Manifest.campaign_contract.crashes_allowed -ne 0 -or
        -not [bool]$Manifest.campaign_contract.clean_committed_source_required -or
        -not [bool]$Manifest.campaign_contract.coverage_report_required -or
        -not [bool]$Manifest.campaign_contract.machine_readable_record_required) {
        throw 'sync-v2 fuzz campaign resource contract changed'
    }
    if (-not [bool]$Manifest.observed.harness_compiled_with_libfuzzer -or
        [bool]$Manifest.observed.smoke_campaign_completed -or
        [bool]$Manifest.observed.sustained_campaign_completed -or
        [bool]$Manifest.observed.coverage_report_generated -or
        [UInt64]$Manifest.observed.executions -ne 0 -or
        [UInt64]$Manifest.observed.wall_clock_seconds -ne 0) {
        throw 'unverified sync-v2 fuzz result was claimed'
    }
    if ([bool]$Manifest.activation.active_transport_authorized -or
        [bool]$Manifest.activation.candidate_identifier_registered -or
        [bool]$Manifest.activation.candidate_identifier_dialed -or
        [bool]$Manifest.activation.active_runtime_changed) {
        throw 'sync-v2 fuzz harness activated production transport'
    }
    Assert-ExactStrings @($Manifest.remaining_m11_gates) @(
        'execute-sustained-coverage-guided-fuzzing',
        'fresh-hosted-and-field-evidence'
    ) 'sync-v2 fuzz remaining gates'

    if ([UInt64]$MutationManifest.schema_version -ne 1 -or
        [string]$MutationManifest.milestone -cne 'M1.1.72' -or
        [string]$MutationManifest.status -cne 'bounded-canonical-sync-v2-regression-corpus' -or
        [bool]$MutationManifest.coverage_guided -or
        [bool]$MutationManifest.compiler_instrumentation -or
        [UInt64]$MutationManifest.expected_mutations -ne 2863) {
        throw 'bounded mutation prerequisite changed'
    }

    Assert-ContainsAll $ProtocolCargo @(
        '[features]', 'default = []', 'decoder-fuzzing = []'
    ) 'protocol Cargo feature contract'
    Assert-ContainsAll $ProtocolLib @(
        '#[cfg(feature = "decoder-fuzzing")]', 'pub mod decoder_fuzzing;'
    ) 'protocol library fuzz feature gate'
    Assert-ContainsAll $Dispatcher @(
        'pub const CANONICAL_SYNC_V2_DECODER_TARGET_COUNT: u8 = 13;',
        'pub const fn from_selector(selector: u8) -> Option<Self>',
        'pub const fn from_fuzzer_byte(byte: u8) -> Self',
        'pub fn exercise_canonical_sync_v2_decoder(',
        'decode_and_verify_signed_sync_responder_checkpoint_v2(bytes)'
    ) 'sync-v2 fuzz dispatcher'
    Assert-ContainsAll $SeedTest @(
        '#![cfg(feature = "decoder-fuzzing")]',
        'canonical_sync_v2_fuzz_seeds_are_exact_and_accepted',
        'assert_eq!(seeds.len(), 20);',
        'exercise_canonical_sync_v2_decoder(target, &payload)'
    ) 'sync-v2 fuzz seed test'
    Assert-ContainsAll $FuzzCargo @(
        'cargo-fuzz = true', 'libfuzzer-sys = "0.4"',
        'features = ["decoder-fuzzing"]',
        'name = "canonical_sync_v2_decoders"'
    ) 'cargo-fuzz manifest'
    Assert-ContainsAll $FuzzLock @(
        'version = 4', 'name = "libfuzzer-sys"', 'version = "0.4.13"',
        'name = "kilogram-protocol-fuzz"'
    ) 'cargo-fuzz dependency lock'
    Assert-ContainsAll $FuzzTarget @(
        '#![no_main]', 'use libfuzzer_sys::fuzz_target;',
        'input.split_first()', 'CanonicalSyncV2DecoderTarget::from_fuzzer_byte(selector)',
        'std::hint::black_box(exercise_canonical_sync_v2_decoder(target, payload));'
    ) 'libFuzzer target'
    Assert-ContainsAll $GitIgnore @(
        '/fuzz/target/', '/fuzz/artifacts/', '/fuzz/corpus/', '/fuzz/coverage/'
    ) 'generated fuzz output ignore contract'
    Assert-ContainsAll $Preparer @(
        'ConvertFrom-StrictLowerHex', 'Get-JsonPointerValue',
        'canonical_sync_v2_fuzz_seed_corpus=verified',
        'coverage_guided_campaign_executed=false'
    ) 'fuzz seed preparer'
    Assert-ContainsAll $Runner @(
        "[ValidateSet('Preflight', 'Smoke', 'Sustained')]",
        'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json',
        'fuzz_toolchain_contract_satisfied=', 'cargo_fuzz_contract_satisfied=',
        'clang_rt.asan_dynamic-x86_64.dll',
        'clang_rt.asan_dynamic_runtime_thunk-x86_64.lib',
        '$env:Path = "$(Split-Path -Parent $asanRuntimePath);$previousPath"',
        '$env:Path = $previousPath',
        '-AcknowledgeSustainedResourceUse', "`$env:CARGO_BUILD_JOBS = '1'",
        "'-jobs=1', '-workers=1'", 'cargo fuzz coverage',
        'Sustained mode requires -GenerateCoverage.',
        'Sustained mode requires a clean committed source worktree.',
        'source_worktree_clean_before', 'source_worktree_clean_after',
        'Sustained fuzz campaign did not satisfy the closed evidence contract',
        'sustained_gate_satisfied', 'coverage_guided_campaign_executed=false'
    ) 'fuzz runner'
    foreach ($forbidden in @('rustup toolchain install', 'cargo install', '-jobs=2', '-workers=2')) {
        if ($Runner.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "fuzz runner contains forbidden resource or installation surface: $forbidden"
        }
    }

    if ([string]$Migration.milestone -cne 'M1.1.73' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.history_compatible_successor.coverage_guided_fuzz_harness -cne
            'verified-not-run' -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial) {
        throw 'canonical migration fuzz harness state changed'
    }
    $harnessRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'coverage-guided-fuzz-harness'
    })
    $campaignRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'sustained-coverage-guided-fuzzing'
    })
    if ($harnessRequirement.Count -ne 1 -or
        [string]$harnessRequirement[0].status -cne 'verified-not-run' -or
        $campaignRequirement.Count -ne 1 -or
        [string]$campaignRequirement[0].status -cne 'pending') {
        throw 'fuzz harness and campaign gates were conflated'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.73' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.coverage_guided_fuzz_harness -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport fuzz harness state changed'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.73' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.coverage_guided_fuzz_harness_integrated -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility fuzz harness state changed'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    $requiredArtifacts = @(
        'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json',
        'crates/kilogram-protocol/src/decoder_fuzzing.rs',
        'crates/kilogram-protocol/tests/canonical_sync_v2_fuzz_seeds.rs',
        'fuzz/Cargo.toml',
        'fuzz/fuzz_targets/canonical_sync_v2_decoders.rs',
        'scripts/prepare-kilogram-canonical-sync-v2-fuzz-corpus.ps1',
        'scripts/invoke-kilogram-canonical-sync-v2-fuzz.ps1',
        'scripts/verify-kilogram-canonical-sync-v2-fuzz-harness-boundary.ps1',
        'docs/RFC-0200-m1173-canonical-sync-v2-coverage-fuzz-harness.md'
    )
    if ([string]$Coverage.milestone -cne 'M1.1.73' -or
        [string]$Coverage.status -cne $aggregateStatus -or
        $surface.Count -ne 1 -or
        'coverage-guided-fuzz-harness' -cnotin @($surface[0].covered)) {
        throw 'canonical coverage fuzz harness state changed'
    }
    foreach ($artifact in $requiredArtifacts) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing fuzz artifact '$artifact'"
        }
    }
    Assert-ExactStrings @($surface[0].remaining) @(
        'sustained-fuzzing', 'fresh-hosted-and-field-evidence'
    ) 'coverage remaining work'

    foreach ($inactiveSource in @($Wire, $ActiveTransport, $Cli)) {
        if ($inactiveSource.Contains('CANONICAL-SYNC-V2-FUZZ-CAMPAIGN') -or
            $inactiveSource.Contains('decoder_fuzzing')) {
            throw 'opt-in fuzz harness leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'standard `cargo-fuzz`/libFuzzer harness',
        'first byte is a harness selector, not a protocol byte',
        'one job and one worker',
        'at least 28,800 seconds, at least 1,000,000 executions',
        'libFuzzer target has not been compiled here',
        'does not close the sustained coverage-guided fuzzing gate',
        'does not register or dial `kilogram/m1/sync/cbor/2`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "sync-v2 fuzz RFC is missing '$needle'"
        }
    }
    $normalizedToolchainRfc = [regex]::Replace($ToolchainRfc, '\s+', ' ')
    foreach ($needle in @(
        'checked-in `fuzz/Cargo.lock`',
        'rejects an unexpected Rust compiler commit or `cargo-fuzz` version',
        'discovers both the MSVC AddressSanitizer runtime DLL and its import library',
        'does not claim a completed smoke or sustained campaign',
        'does not register or dial `kilogram/m1/sync/cbor/2`'
    )) {
        if ($normalizedToolchainRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "sync-v2 fuzz toolchain RFC is missing '$needle'"
        }
    }
}

$paths = @(
    $manifestPath, $toolchainManifestPath, $mutationManifestPath, $migrationPath, $transportManifestPath,
    $compatibilityPath, $coveragePath, $protocolCargoPath, $protocolLibPath,
    $dispatcherPath, $seedTestPath, $fuzzCargoPath, $fuzzLockPath, $fuzzTargetPath, $gitignorePath, $preparePath,
    $runnerPath, $wirePath, $activeTransportPath, $cliPath, $rfcPath, $toolchainRfcPath
)
foreach ($path in $paths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "sync-v2 fuzz harness file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$toolchainManifest = Get-Content -LiteralPath $toolchainManifestPath -Raw | ConvertFrom-Json
$mutationManifest = Get-Content -LiteralPath $mutationManifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$protocolCargo = Get-Content -LiteralPath $protocolCargoPath -Raw
$protocolLib = Get-Content -LiteralPath $protocolLibPath -Raw
$dispatcher = Get-Content -LiteralPath $dispatcherPath -Raw
$seedTest = Get-Content -LiteralPath $seedTestPath -Raw
$fuzzCargo = Get-Content -LiteralPath $fuzzCargoPath -Raw
$fuzzLock = Get-Content -LiteralPath $fuzzLockPath -Raw
$fuzzTarget = Get-Content -LiteralPath $fuzzTargetPath -Raw
$gitignore = Get-Content -LiteralPath $gitignorePath -Raw
$preparer = Get-Content -LiteralPath $preparePath -Raw
$runner = Get-Content -LiteralPath $runnerPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$activeTransport = Get-Content -LiteralPath $activeTransportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$toolchainRfc = Get-Content -LiteralPath $toolchainRfcPath -Raw

$arguments = @(
    $toolchainManifest, $mutationManifest, $migration, $transportManifest, $compatibility, $coverage,
    $protocolCargo, $protocolLib, $dispatcher, $seedTest, $fuzzCargo, $fuzzLock, $fuzzTarget, $gitignore,
    $preparer, $runner, $wire, $activeTransport, $cli, $rfc, $toolchainRfc
)
Assert-Contract $manifest @arguments

if ($SelfTest) {
    $completed = Copy-JsonObject $manifest
    $completed.observed.sustained_campaign_completed = $true
    Assert-Rejected { Assert-Contract $completed @arguments } 'false_campaign_completion'

    $selector = Copy-JsonObject $manifest
    $selector.selector.mapping[0] = 'different-decoder'
    Assert-Rejected { Assert-Contract $selector @arguments } 'selector_mapping_drift'

    $seedCount = Copy-JsonObject $manifest
    $seedCount.seed_count = 19
    Assert-Rejected { Assert-Contract $seedCount @arguments } 'seed_count_drift'

    $activated = Copy-JsonObject $manifest
    $activated.activation.active_transport_authorized = $true
    Assert-Rejected { Assert-Contract $activated @arguments } 'premature_transport_activation'

    $parallel = Copy-JsonObject $manifest
    $parallel.campaign_contract.workers = 2
    Assert-Rejected { Assert-Contract $parallel @arguments } 'worker_count_drift'

    $floatingToolchain = Copy-JsonObject $toolchainManifest
    $floatingToolchain.toolchain.rustc_commit_hash = 'unverified-floating-nightly'
    $toolchainArguments = @(
        $floatingToolchain, $mutationManifest, $migration, $transportManifest,
        $compatibility, $coverage, $protocolCargo, $protocolLib, $dispatcher,
        $seedTest, $fuzzCargo, $fuzzLock, $fuzzTarget, $gitignore, $preparer,
        $runner, $wire, $activeTransport, $cli, $rfc, $toolchainRfc
    )
    Assert-Rejected { Assert-Contract $manifest @toolchainArguments } 'floating_toolchain_drift'

    Write-Output 'canonical_sync_v2_fuzz_harness_self_test=passed'
}

Write-Output 'canonical_sync_v2_fuzz_harness_boundary=verified'
Write-Output 'strict_decoder_targets=13'
Write-Output 'exact_seed_count=20'
Write-Output 'instrumented_target_built=true'
Write-Output 'instrumented_campaign_completed=false'
Write-Output 'coverage_report_generated=false'
Write-Output 'sustained_coverage_guided_fuzzing=pending'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
