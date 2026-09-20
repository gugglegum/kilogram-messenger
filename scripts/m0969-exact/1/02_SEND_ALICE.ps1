. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$build = Assert-M0969Kit
$noHttpsCompatibility = [string]$build.milestone -cne 'M0.9.69'
$serviceFreeV2 = [string]$build.milestone -cin @('M0.9.76', 'M0.9.87')
$legacyNoHttpsCompatibility = $noHttpsCompatibility -and -not $serviceFreeV2
$run = Get-M0969Run
$null = Wait-M0969File (Join-Path $script:SharedDirectory 'alice-ready.marker') 180 'Alice ready marker'
$null = Wait-M0969File (Join-Path $script:SharedDirectory 'bob-ready.marker') 180 'Bob ready marker'
$private = Get-M0969PrivateRoot 'alice' ([string]$run.run_id)
$role = Get-Content -LiteralPath (Join-Path $private 'role.json') -Raw | ConvertFrom-Json
$profile = [string]$role.profile
$ipc = [string]$role.ipc
$storeData = [string]$role.store_data

$storeLog = Join-Path $private 'compatibility-store-send.log'
$sendLog = Join-Path $script:EvidenceDirectory '04-send-alice.log'
$queuePath = Join-Path $script:EvidenceDirectory '04-alice-queue.log'
$offlinePath = Join-Path $script:EvidenceDirectory '05-alice-offline.boundary'
foreach ($path in @($sendLog, $queuePath, $offlinePath, (Join-Path $script:SharedDirectory 'alice-sent.marker'))) {
    if (Test-Path -LiteralPath $path) {
        throw "Clean Alice send evidence already exists; refusing a resumed M0.9.69 run: $path"
    }
}

$activation = @(Get-Content -LiteralPath (Join-Path $script:EvidenceDirectory '03-bob-mailbox-capability.log'))
$commitmentId = Get-M0969ExactValue `
    $activation 'mailbox_replica_set_commitment_id' '[0-9a-f]{64}'
$providerKeys = @(Get-M0969ProviderStoreKeys)
$store = $null
$runtime = $null
try {
    if ($legacyNoHttpsCompatibility) {
        Assert-M0972HttpsFixtureAbsent
        [IO.File]::WriteAllLines(
            (Join-Path $script:EvidenceDirectory '04-https-fixture-absence.log'),
            @(
                'field_phase=immediately-before-send'
                'https_fixture_binary_present=false'
                'https_fixture_process_started=false'
                "compatibility_endpoint=$script:M0972CompatibilityStoreUrl"
                'compatibility_endpoint_reachable=false'
            ),
            [Text.UTF8Encoding]::new($false)
        )
    } elseif ($serviceFreeV2 -and (Test-Path -LiteralPath $script:StorePath)) {
        if ([string]$build.milestone -ceq 'M0.9.76') {
            throw 'M0.9.76 must not contain or start the HTTPS compatibility store.'
        }
        throw 'M0.9.87 must not contain or start the HTTPS compatibility store.'
    }
    if (-not $noHttpsCompatibility) {
        $store = Start-M0969Process $script:StorePath @('--data-dir', $storeData) $storeLog
        $null = Wait-M0969LogPattern $storeLog '^status=listening$' $store 60
    }
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $sendLog
    Wait-M0969IpcReady $ipc $runtime 120
    Import-M0969Providers 'alice-before-send' $ipc '04-alice-providers-before-send.log'

    $queueOutput = @(Invoke-M0969Cli @(
        'runtime-ipc-queue-message', '--ipc-file', $ipc,
        '--conversation', ([string]$run.conversation_label),
        '--peer-account', ([string]$role.bob_account_id),
        '--message', ([string]$run.message_marker)
    ))
    [IO.File]::WriteAllLines($queuePath, $queueOutput, [Text.UTF8Encoding]::new($false))
    $null = Wait-M0969LogPattern $sendLog '^runtime_outbound_status=mailbox-stored$' $runtime 240
    $sendText = Wait-M0969LogPattern `
        $sendLog '^runtime_mailbox_replication_status=satisfied$' $runtime 300
    foreach ($pattern in @(
        '^runtime_mailbox_replica_set_discovery=exact-authenticated$',
        '^runtime_mailbox_replica_set_resolved=2/2$',
        '^runtime_mailbox_replication_receipts=2/2$'
    )) {
        if (-not [regex]::IsMatch(
            $sendText, $pattern, [Text.RegularExpressions.RegexOptions]::Multiline
        )) { throw "Alice send is missing exact-locator evidence: $pattern" }
    }
    if ([regex]::IsMatch($sendText, '(?m)^runtime_mailbox_replica_set_discovery=legacy-random-fallback$')) {
        throw 'Alice used forbidden legacy random provider sampling.'
    }
    if ($noHttpsCompatibility) {
        $compatibilityPattern = if ($serviceFreeV2) {
            '^runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer$'
        } else {
            '^runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability$'
        }
        foreach ($pattern in @(
            $compatibilityPattern,
            '^runtime_mailbox_http_put=not-attempted$',
            '^runtime_mailbox_delivery_durability=exact-volunteer-replication$'
        )) {
            if ([regex]::Matches(
                $sendText, $pattern, [Text.RegularExpressions.RegexOptions]::Multiline
            ).Count -ne 1) { throw "Alice no-HTTPS evidence is missing or ambiguous: $pattern" }
        }
        foreach ($forbidden in @(
            '^runtime_mailbox_http_put=attempted$',
            '^runtime_mailbox_delivery_durability=https-compatibility$',
            '^runtime_mailbox_exact_completion_status=failed(?: |$)',
            '^runtime_mailbox_https_compatibility_copy=retained-',
            $(if ($serviceFreeV2) {
                '^runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability$'
            } else {
                '^runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer$'
            })
        )) {
            if ([regex]::IsMatch(
                $sendText, $forbidden, [Text.RegularExpressions.RegexOptions]::Multiline
            )) { throw "Alice unexpectedly entered HTTPS compatibility delivery: $forbidden" }
        }
    }
    $sendCommitments = @([regex]::Matches(
        $sendText,
        '(?m)^runtime_mailbox_replica_set_commitment_id=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ($sendCommitments.Count -ne 1 -or $sendCommitments[0] -cne $commitmentId) {
        throw 'Alice did not use the exact commitment signed by Bob.'
    }
    $receiptKeys = @([regex]::Matches(
        $sendText,
        '(?m)^runtime_mailbox_replication_receipt_store_key=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $attemptKeys = @([regex]::Matches(
        $sendText,
        '(?m)^runtime_mailbox_replication_provider_attempt_store_key=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ($receiptKeys.Count -ne 2 -or $attemptKeys.Count -ne 2 -or
        @(Compare-Object $providerKeys $receiptKeys).Count -ne 0 -or
        @(Compare-Object $providerKeys $attemptKeys).Count -ne 0) {
        throw 'Alice exact locator attempts or receipts differ from the two precommitted providers.'
    }
}
finally {
    Stop-M0969Process $runtime
    Stop-M0969Process $store
}

$previous = $ErrorActionPreference
try {
    $ErrorActionPreference = 'SilentlyContinue'
    & $script:CliPath runtime-ipc-ping --ipc-file $ipc 1>$null 2>$null
    $probeExitCode = $LASTEXITCODE
}
finally { $ErrorActionPreference = $previous }
if ($probeExitCode -eq 0) { throw 'Alice runtime is still reachable after stop.' }
if ($legacyNoHttpsCompatibility) { Assert-M0972HttpsFixtureAbsent }

$offlineEvidence = @(
    'alice_replication_status=satisfied',
    "alice_replica_set_commitment_id=$commitmentId",
    "alice_replica_receipt_store_keys=$($providerKeys -join ',')",
    'alice_replica_discovery=exact-authenticated',
    'alice_runtime_ipc_reachable=false',
    "sender_stop_observed_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
)
if ($legacyNoHttpsCompatibility) {
    $offlineEvidence += @(
        'https_fixture_binary_present=false',
        'compatibility_endpoint_reachable=false',
        'runtime_mailbox_http_put=not-attempted'
    )
} elseif ($serviceFreeV2) {
    $offlineEvidence += @(
        'mailbox_capability_format=v2-exact-volunteer',
        'mailbox_service_descriptor=absent',
        'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
        'runtime_mailbox_http_put=not-attempted'
    )
}
[IO.File]::WriteAllLines($offlinePath, $offlineEvidence, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'alice-sent.marker'), "sent`n")
if ($serviceFreeV2) {
    Write-Host 'ALICE SERVICE-FREE V2 SEND COMPLETED; NO CENTRAL MAILBOX DESCRIPTOR EXISTS.'
} elseif ($noHttpsCompatibility) {
    Write-Host 'ALICE NO-HTTPS SEND COMPLETED; EXACT VOLUNTEER DURABILITY IS COMMITTED.'
} else {
    Write-Host 'ALICE EXACT-LOCATOR SEND COMPLETED; ALICE RUNTIME AND HTTPS FIXTURE ARE OFFLINE.'
}
