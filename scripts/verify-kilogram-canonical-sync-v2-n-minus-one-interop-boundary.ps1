[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$evidencePath = Join-Path $workspace 'CANONICAL-SYNC-V2-N-MINUS-ONE-INTEROP.json'
$harnessPath = Join-Path $workspace 'scripts\invoke-kilogram-canonical-sync-v2-n-minus-one-two-host.ps1'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$offlinePath = Join-Path $workspace 'apps\kilogram-offline\src\lib.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$decisionPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PUBLIC-ROLLOUT-DECISION.json'
$rolloutPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-LOCAL-AUTHORITY.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0215-m1191-n-minus-one-production-interop.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Scenario {
    param(
        [object]$Scenario,
        [string]$Name,
        [string]$Initiator,
        [string]$Responder,
        [string]$RoutePolicy,
        [string]$SelectedPath,
        [UInt64]$EventCount,
        [string]$Digest
    )

    if ([string]$Scenario.name -cne $Name -or
        [string]$Scenario.initiator_version -cne $Initiator -or
        [string]$Scenario.responder_version -cne $Responder -or
        [string]$Scenario.route_policy -cne $RoutePolicy -or
        [string]$Scenario.selected_path -cne $SelectedPath -or
        [string]$Scenario.protocol -cne 'kilogram/m0/sync/8' -or
        -not [bool]$Scenario.delivery_acknowledged -or
        -not [bool]$Scenario.listener_restart -or
        -not [bool]$Scenario.synchronized -or
        -not [bool]$Scenario.histories_equal -or
        [UInt64]$Scenario.event_count -ne $EventCount -or
        [string]$Scenario.inventory_digest -cne $Digest -or
        [bool]$Scenario.canonical_selected -or
        [bool]$Scenario.fallback_after_selection) {
        throw "N/N-1 scenario '$Name' changed"
    }
}

function Assert-Contract {
    param(
        [object]$Evidence,
        [string]$Harness,
        [string]$Cli,
        [string]$Offline,
        [string]$Transport,
        [object]$PriorDecision,
        [object]$Rollout,
        [string]$Rfc
    )

    if ([UInt64]$Evidence.schema_version -ne 1 -or
        [string]$Evidence.milestone -cne 'M1.1.91' -or
        [string]$Evidence.status -cne
            'canonical-sync-v2-n-minus-one-production-interop-verified' -or
        [string]$Evidence.run_id -cne 'm1191-interop-attempt9' -or
        [DateTimeOffset]$Evidence.collected_at_utc -ne
            [DateTimeOffset]::Parse(
                '2026-10-19T01:27:16.3460521Z',
                [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::RoundtripKind
            )) {
        throw 'N/N-1 evidence identity changed'
    }

    if ([string]$Evidence.accepted.tag -cne 'M1.0.0' -or
        [string]$Evidence.accepted.revision -cne
            '60b20b364c8df2324a0a287b4bcb6c056ae12c39' -or
        [string]$Evidence.accepted.binary_sha256 -cne
            '24c5681000af166cdbe63e61942c0d0dfbc5e1fca0772e257d99dd7984cde7c6' -or
        [UInt64]$Evidence.accepted.binary_bytes -ne 57975296 -or
        [string]$Evidence.successor.revision -cne
            '6d95cb73359f44f20908d52b4fd2eaf7869cdede' -or
        [string]$Evidence.successor.binary_sha256 -cne
            '22265eb35c8a94f84ab3e83c67d12235080e80c9e8f81fc56f60362897e1e13e' -or
        [UInt64]$Evidence.successor.binary_bytes -ne 60089856) {
        throw 'exact N/N-1 executable provenance changed'
    }

    if ([string]$Evidence.build.profile -cne 'debug' -or
        [string]$Evidence.build.target -cne 'x86_64-pc-windows-msvc' -or
        [UInt64]$Evidence.build.cargo_jobs -ne 2 -or
        [bool]$Evidence.build.release_build -or
        [bool]$Evidence.build.archive_created -or
        -not [bool]$Evidence.topology.distinct_physical_hosts -or
        [string]::IsNullOrWhiteSpace([string]$Evidence.topology.controller_host) -or
        [string]::IsNullOrWhiteSpace([string]$Evidence.topology.remote_host) -or
        [string]$Evidence.topology.controller_host -ceq [string]$Evidence.topology.remote_host) {
        throw 'N/N-1 build or physical topology changed'
    }

    if ([string]$Evidence.protocol.common_sync_alpn -cne 'kilogram/m0/sync/8' -or
        [string]$Evidence.protocol.canonical_sync_alpn -cne 'kilogram/m1/sync/cbor/2' -or
        [bool]$Evidence.protocol.canonical_selected -or
        [string]$Evidence.protocol.public_default -cne 'legacy-only') {
        throw 'N/N-1 negotiated protocol boundary changed'
    }

    $matrix = @($Evidence.matrix)
    if ($matrix.Count -ne 4) { throw 'N/N-1 matrix must contain exactly four scenarios' }
    Assert-Scenario $matrix[0] '01-current-to-previous-direct' 'N' 'N-1' 'direct-only' 'direct' 6 '3c08d0e52a91d46d6736729bac12d9b74a8eb2ffa744b54465dd1961e238fb18'
    Assert-Scenario $matrix[1] '02-previous-to-current-direct' 'N-1' 'N' 'direct-only' 'direct' 6 'cf3de1bd61c56f5cd3959e2990dbe6d4f98f8434f331a384784b89c5f074d014'
    Assert-Scenario $matrix[2] '03-current-to-previous-relay' 'N' 'N-1' 'relay-only' 'relay' 6 '650f558d4428dce30418acffe20574601703a9d21d428b3e3e95ac7b3e1794ea'
    Assert-Scenario $matrix[3] '04-previous-to-current-relay' 'N-1' 'N' 'relay-only' 'relay' 6 'cb873a3ffd7b8a5a94a69831d1dbcb71db310fccd49cdcbc64dece0dc41d9bf3'

    $upgrade = $Evidence.upgrade_chain
    if ([string]$upgrade.name -cne '05-forward-upgrade' -or
        -not [bool]$upgrade.state_reused -or
        -not [bool]$upgrade.forward_only -or
        [bool]$upgrade.downgrade_after_upgrade_attempted) {
        throw 'forward-only upgrade contract changed'
    }
    Assert-Scenario $upgrade.baseline '05a-nminus1-baseline' 'N-1' 'N-1' 'direct-only' 'direct' 6 'f77ed43bbc51a2e26c164b49090ca9e520c534401ec9b31b2f0a78320f52e95c'
    Assert-Scenario $upgrade.responder_upgrade '05b-responder-upgraded' 'N-1' 'N' 'direct-only' 'direct' 12 'db0abaa1b286e4796867f0ab23801e8328aea57f686c0d2d1555115664b39019'
    Assert-Scenario $upgrade.initiator_upgrade '05c-initiator-upgraded' 'N' 'N' 'relay-only' 'relay' 18 'cb21ee1a1935fb165e29fec4e4ec80da5496386196581c08b2809a492476e84a'
    if (-not [bool]$upgrade.final_histories_equal -or
        [UInt64]$upgrade.final_event_count -ne 18 -or
        [string]$upgrade.final_inventory_digest -cne
            'cb21ee1a1935fb165e29fec4e4ec80da5496386196581c08b2809a492476e84a') {
        throw 'forward-upgrade final history changed'
    }

    if ([bool]$Evidence.privacy.raw_logs_committed -or
        [bool]$Evidence.privacy.state_committed -or
        [bool]$Evidence.privacy.account_or_device_ids_exported -or
        [bool]$Evidence.privacy.message_bodies_exported) {
        throw 'N/N-1 privacy boundary changed'
    }

    if ([string]$PriorDecision.status -cne
            'canonical-sync-v2-public-rollout-decision-ready-held' -or
        [string]$PriorDecision.decision.verdict -cne 'hold' -or
        [bool]$PriorDecision.decision.eligible -or
        [string]$PriorDecision.public_default -cne 'legacy-only' -or
        [string]$Rollout.status -cne
            'canonical-sync-v2-rollout-local-authority-verified-dormant' -or
        [bool]$Rollout.runtime_rollout_integration_enabled) {
        throw 'public rollout hold or dormant authority was weakened'
    }

    Assert-ContainsAll $Harness @(
        "`$runnerDirectory = Join-Path `$workspace '.tmp\kilogram-field-runner'",
        "`$localRunner = Join-Path `$runnerDirectory 'kilogram-cli.exe'",
        'KilogramLab/n-minus-one-stable',
        '$remoteRunner = "$remoteRunnerDirectory/kilogram-cli.exe"',
        "Get-FileHash -LiteralPath `$localRunner -Algorithm SHA256",
        '[Security.Cryptography.SHA256]::Create()',
        '.ComputeHash([Text.Encoding]::UTF8.GetBytes',
        "Invoke-MatrixScenario '01-current-to-previous-direct'",
        "Invoke-MatrixScenario '02-previous-to-current-direct'",
        "Invoke-MatrixScenario '03-current-to-previous-relay'",
        "Invoke-MatrixScenario '04-previous-to-current-relay'",
        "Invoke-Exchange `$pairPrevious '05a-nminus1-baseline' 'direct-only'",
        "Invoke-Exchange `$responderUpgradedPair '05b-responder-upgraded' 'direct-only'",
        "Invoke-Exchange `$fullyUpgradedPair '05c-initiator-upgraded' 'relay-only'",
        "protocol = 'kilogram/m0/sync/8'",
        'listener_restart = $true',
        'histories_equal = $true',
        'raw_logs_committed = $false'
    ) 'N/N-1 two-host harness'
    foreach ($forbidden in @('[Security.Cryptography.SHA256]::HashData', '[Convert]::ToHexString')) {
        if ($Harness.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "N/N-1 harness regained unsupported Windows PowerShell API '$forbidden'"
        }
    }

    Assert-ContainsAll $Cli @(
        'const SYNC_FOLLOW_UP_STREAM_TIMEOUT: Duration = Duration::from_secs(120);',
        'const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v11\0";',
        'const TICKET_VERSION: u8 = 11;',
        'struct LegacyConnectionTicketContent',
        'sync_transport_capability: Option<SignedSyncTransportCapability>',
        'let legacy = LegacyConnectionTicketContent {',
        'write_intermediate_server_response(&mut inventory_send, &diff_response).await?;',
        'accept_sync_follow_up_bi(connection, "accept sync event batch stream")',
        'accept_sync_follow_up_bi(connection, "accept next sync inventory stream")'
    ) 'production CLI compatibility implementation'
    Assert-ContainsAll $Offline @(
        'sync_transport_capability: Option<SignedSyncTransportCapability>',
        'struct OfflineLegacyConnectionTicketContent',
        'let legacy = OfflineLegacyConnectionTicketContent {'
    ) 'offline ticket compatibility implementation'
    Assert-ContainsAll $Transport @(
        'pub async fn write_intermediate_server_response(',
        'follow-up request on another stream of the same live connection',
        'preserves the M1.0.0 stream lifecycle',
        'write_server_response_frame(send, response).await'
    ) 'transport response lifecycle'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'four bidirectional direct/relay combinations passed',
        'same durable state then passed N-1/N-1, N-1/N and N/N',
        'roughly 53.8 seconds',
        'bounded 120-second follow-up deadline',
        'stable executable paths',
        'public default remains `legacy-only`',
        'does not authorize public canonical rollout',
        'optional-extension stripping before the first selection is not proven detectable'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "N/N-1 RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $evidencePath, $harnessPath, $cliPath, $offlinePath, $transportPath,
    $decisionPath, $rolloutPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "N/N-1 interop file is missing: $path"
    }
}

$evidenceRaw = Get-Content -LiteralPath $evidencePath -Raw
foreach ($forbiddenField in @('"account_id"\s*:', '"device_id"\s*:', '"message_body"\s*:', '"plaintext"\s*:')) {
    if ($evidenceRaw -match $forbiddenField) {
        throw "tracked N/N-1 evidence contains forbidden raw identity or content field '$forbiddenField'"
    }
}
$evidence = $evidenceRaw | ConvertFrom-Json
$harness = Get-Content -LiteralPath $harnessPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$offline = Get-Content -LiteralPath $offlinePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$priorDecision = Get-Content -LiteralPath $decisionPath -Raw | ConvertFrom-Json
$rollout = Get-Content -LiteralPath $rolloutPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $evidence $harness $cli $offline $transport $priorDecision $rollout $rfc

if ($SelfTest) {
    $wrongBinary = Copy-JsonObject $evidence
    $wrongBinary.accepted.binary_sha256 = '00' * 32
    Assert-Rejected {
        Assert-Contract $wrongBinary $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'wrong_accepted_executable'

    $missingDirection = Copy-JsonObject $evidence
    $missingDirection.matrix = @($missingDirection.matrix | Select-Object -First 3)
    Assert-Rejected {
        Assert-Contract $missingDirection $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'incomplete_bidirectional_matrix'

    $wrongRoute = Copy-JsonObject $evidence
    $wrongRoute.matrix[2].selected_path = 'direct'
    Assert-Rejected {
        Assert-Contract $wrongRoute $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'relay_path_not_observed'

    $lostHistory = Copy-JsonObject $evidence
    $lostHistory.upgrade_chain.final_histories_equal = $false
    Assert-Rejected {
        Assert-Contract $lostHistory $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'upgrade_history_diverged'

    $downgrade = Copy-JsonObject $evidence
    $downgrade.upgrade_chain.downgrade_after_upgrade_attempted = $true
    Assert-Rejected {
        Assert-Contract $downgrade $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'downgrade_attempted'

    $canonicalDefault = Copy-JsonObject $evidence
    $canonicalDefault.protocol.canonical_selected = $true
    $canonicalDefault.protocol.public_default = 'canonical-preferred'
    Assert-Rejected {
        Assert-Contract $canonicalDefault $harness $cli $offline $transport $priorDecision $rollout $rfc
    } 'public_default_activated'

    $unstableHarness = $harness.Replace("'.tmp\kilogram-field-runner'", "('.tmp\kilogram-field-runner-' + `$RunId)")
    Assert-Rejected {
        Assert-Contract $evidence $unstableHarness $cli $offline $transport $priorDecision $rollout $rfc
    } 'unstable_firewall_executable_path'

    Write-Output 'canonical_sync_v2_n_minus_one_interop_self_test=passed'
}

Write-Output 'canonical_sync_v2_n_minus_one_interop_boundary=verified'
Write-Output 'matrix_scenarios=4/4'
Write-Output 'forward_upgrade_chain=verified'
Write-Output 'accepted_tag=M1.0.0'
Write-Output 'public_default=legacy-only'
Write-Output 'stable_firewall_executable_paths=verified'
Write-Output 'raw_logs_committed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
