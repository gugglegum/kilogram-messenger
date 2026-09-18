. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0969Kit
$run = Get-M0969Run
$null = Wait-M0969File (Join-Path $script:SharedDirectory 'alice-sent.marker') 1800 'Alice sent marker'
$private = Get-M0969PrivateRoot 'bob' ([string]$run.run_id)
$role = Get-Content -LiteralPath (Join-Path $private 'role.json') -Raw | ConvertFrom-Json
$profile = [string]$role.profile
$ipc = [string]$role.ipc
$state = [string]$role.state
$commitmentId = [string]$role.replica_set_commitment_id
$providerKeys = @(Get-M0969ProviderStoreKeys)

$receiveShared = Join-Path $script:EvidenceDirectory '06-receive-bob.log'
$restartShared = Join-Path $script:EvidenceDirectory '07-restart-bob.log'
$historyPath = Join-Path $script:EvidenceDirectory '07-bob-history.log'
foreach ($path in @($receiveShared, $restartShared, $historyPath, (Join-Path $script:SharedDirectory 'bob-complete.marker'))) {
    if (Test-Path -LiteralPath $path) {
        throw "Clean Bob receive evidence already exists; refusing a resumed M0.9.69 run: $path"
    }
}

$attemptDirectory = Join-Path $private 'receive-attempt'
New-M0969Directory $attemptDirectory
$receiveLocal = Join-Path $attemptDirectory '06-receive-bob.log'
$restartLocal = Join-Path $attemptDirectory '07-restart-bob.log'
$runtime = $null
try {
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $receiveLocal
    Wait-M0969IpcReady $ipc $runtime 120
    Import-M0969Providers 'bob-before-receive' $ipc '06-bob-providers-before-receive.log'
    $null = Wait-M0969LogCount `
        $receiveLocal '^runtime_mailbox_inbound_source=volunteer-iroh$' 2 $runtime 360
    $receiveText = Wait-M0969LogCount `
        $receiveLocal '^runtime_mailbox_replica_delete_status=deleted-after-commit$' 2 $runtime 180

    foreach ($pattern in @(
        '^runtime_mailbox_replica_set_discovery=exact-authenticated$',
        '^runtime_mailbox_replica_set_resolved=2/2$'
    )) {
        if (-not [regex]::IsMatch(
            $receiveText, $pattern, [Text.RegularExpressions.RegexOptions]::Multiline
        )) { throw "Bob receive is missing exact-locator evidence: $pattern" }
    }
    if ([regex]::IsMatch(
        $receiveText,
        '(?m)^runtime_mailbox_replica_set_discovery=legacy-random-fallback$'
    )) { throw 'Bob used forbidden legacy random provider sampling.' }
    $receiveCommitments = @([regex]::Matches(
        $receiveText,
        '(?m)^runtime_mailbox_replica_set_commitment_id=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $pollKeys = @([regex]::Matches(
        $receiveText,
        '(?m)^runtime_mailbox_replica_poll_store_key=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $sourceKeys = @([regex]::Matches(
        $receiveText,
        '(?m)^runtime_mailbox_replica_source_store_key=([0-9a-f]{64})$'
    ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ($receiveCommitments.Count -ne 1 -or $receiveCommitments[0] -cne $commitmentId) {
        throw 'Bob did not resolve his own authenticated replica-set commitment.'
    }
    if ($pollKeys.Count -ne 2 -or $sourceKeys.Count -ne 2 -or
        @(Compare-Object $providerKeys $pollKeys).Count -ne 0 -or
        @(Compare-Object $providerKeys $sourceKeys).Count -ne 0) {
        throw 'Bob polled or committed a provider outside the precommitted replica set.'
    }
}
finally {
    Stop-M0969Process $runtime
    Publish-M0969File $receiveLocal $receiveShared
    Publish-M0969File "$receiveLocal.stderr" "$receiveShared.stderr"
}

$restart = $null
try {
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $restart = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $restartLocal
    Wait-M0969IpcReady $ipc $restart 180
    $null = Wait-M0969LogPattern $restartLocal '^status=runtime-listening$' $restart 30
    Write-Host 'Bob restart is healthy; observing for 20 seconds to reject replica redelivery...'
    Start-Sleep -Seconds 20
}
finally {
    Stop-M0969Process $restart
    Publish-M0969File $restartLocal $restartShared
    Publish-M0969File "$restartLocal.stderr" "$restartShared.stderr"
}
$restartText = Get-Content -LiteralPath $restartLocal -Raw
if ([regex]::IsMatch($restartText, '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$')) {
    throw 'Deleted volunteer replica was delivered again after restart.'
}
if ([regex]::IsMatch(
    $restartText,
    '(?m)^runtime_mailbox_replica_set_discovery=legacy-random-fallback$'
)) { throw 'Bob restart fell back to forbidden random provider sampling.' }

$history = @(Invoke-M0969Cli @(
    'history', '--state-dir', $state, '--conversation', ([string]$run.conversation_label)
))
[IO.File]::WriteAllLines($historyPath, $history, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'bob-complete.marker'), "complete`n")
Write-Host 'BOB EXACT-LOCATOR RECEIVE, DELETE, AND RESTART COMPLETED SUCCESSFULLY.'
