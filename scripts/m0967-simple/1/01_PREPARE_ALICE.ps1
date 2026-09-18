. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$build = Assert-M0967Kit
New-M0967Directory $script:SharedDirectory
New-M0967Directory $script:EvidenceDirectory

$runPath = Join-Path $script:SharedDirectory 'run.json'
if (Test-Path -LiteralPath $runPath) { throw 'This kit already has a run. Use a fresh generated kit for another attempt.' }
$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$run = [ordered]@{
    schema = 1
    run_id = $runId
    build_commit = [string]$build.source_revision
    conversation_label = "m0967-$runId"
    message_marker = "kilogram-m0967-$runId"
}
Write-M0967JsonNew $runPath $run

$private = Get-M0967PrivateRoot 'alice' $runId
if (Test-Path -LiteralPath $private) { throw "Alice private directory already exists: $private" }
$account = Join-Path $private 'account-root'
$state = Join-Path $private 'state'
$public = Join-Path $private 'public'
$profile = Join-Path $private 'runtime-profile.json'
$ipc = Join-Path $private 'runtime.ipc.json'
$deviceCertificate = Join-Path $public 'device.cert'
$deviceList = Join-Path $public 'device-list.kadl'
$storeData = Join-Path $private 'compatibility-store'
New-M0967Directory $private
New-M0967Directory $public

$accountOutput = @(Invoke-M0967Cli @('account-create', '--account-dir', $account))
$aliceAccount = Get-M0967ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
$null = Invoke-M0967Cli @('device-enroll', '--account-dir', $account, '--state-dir', $state, '--certificate-file', $deviceCertificate)
$null = Invoke-M0967Cli @('account-device-list', '--account-dir', $account, '--device-certificate-file', $deviceCertificate, '--device-list-file', $deviceList)
$identityOutput = @(Invoke-M0967Cli @('identity', '--state-dir', $state))
$aliceDevice = Get-M0967ExactValue $identityOutput 'device_id' '[0-9a-f]{64}'

$storeBootstrapLog = Join-Path $private 'compatibility-store-bootstrap.log'
$storeProcess = $null
try {
    $storeProcess = Start-M0967Process $script:StorePath @('--data-dir', $storeData) $storeBootstrapLog
    $storeText = Wait-M0967LogPattern $storeBootstrapLog '^blind_mailbox_store_key=([0-9a-f]{64})$' $storeProcess 60
    $storeKey = [regex]::Match($storeText, '(?m)^blind_mailbox_store_key=([0-9a-f]{64})$').Groups[1].Value
}
finally { Stop-M0967Process $storeProcess }

Write-M0967JsonNew (Join-Path $script:SharedDirectory 'alice-public.json') ([ordered]@{
    account_id = $aliceAccount
    device_id = $aliceDevice
    compatibility_store_key = $storeKey
    compatibility_store_url = 'http://127.0.0.1:8787'
})
Write-Host 'Alice identity is ready. Waiting for Bob setup from Yandex Disk...'
$bobPublicPath = Wait-M0967File (Join-Path $script:SharedDirectory 'bob-public.json') 1800 'Bob public identity'
$bob = Get-Content -LiteralPath $bobPublicPath -Raw | ConvertFrom-Json

$membership = Join-Path $script:SharedDirectory 'conversation.membership'
$null = Invoke-M0967Cli @(
    'conversation-create', '--account-dir', $account, '--conversation', $run.conversation_label,
    '--member-account', $aliceAccount, '--member-account', ([string]$bob.account_id),
    '--membership-file', $membership
)
$null = Invoke-M0967Cli @('conversation-membership-install', '--state-dir', $state, '--membership-file', $membership)
$null = Invoke-M0967Cli @(
    'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
    '--allow-account', ([string]$bob.account_id), '--device-list-file', $deviceList,
    '--ticket-file', (Join-Path $script:SharedDirectory 'alice.ticket'), '--ipc-file', $ipc,
    '--route-policy', 'auto', '--relay-wait-seconds', '30', '--disable-volunteer-storage'
)

$bootstrapLog = Join-Path $private 'alice-ticket-bootstrap.log'
$runtime = $null
try {
    $runtime = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $bootstrapLog
    $null = Wait-M0967File (Join-Path $script:SharedDirectory 'alice.ticket') 120 'Alice ticket'
    $bobTicket = Wait-M0967File (Join-Path $script:SharedDirectory 'bob.ticket') 1800 'Bob ticket'
}
finally { Stop-M0967Process $runtime }
$null = Invoke-M0967Cli @(
    'runtime-contact-add', '--state-dir', $state, '--conversation', $run.conversation_label,
    '--expect-account', ([string]$bob.account_id), '--descriptor-file', $bobTicket
)
$offer = Wait-M0967File (Join-Path $script:SharedDirectory 'bob-mailbox.offer') 1800 'Bob mailbox offer'
$null = Invoke-M0967Cli @(
    'runtime-mailbox-offer-import', '--state-dir', $state, '--conversation', $run.conversation_label,
    '--peer-account', ([string]$bob.account_id), '--offer-file', $offer
)

Write-M0967JsonNew (Join-Path $script:EvidenceDirectory 'manifest.json') ([ordered]@{
    schema = 1
    run_id = $runId
    build_commit = [string]$build.source_revision
    conversation_label = $run.conversation_label
    alice_account_id = $aliceAccount
    bob_account_id = [string]$bob.account_id
    message_marker = $run.message_marker
})
Write-M0967JsonNew (Join-Path $private 'role.json') ([ordered]@{
    profile = $profile; ipc = $ipc; state = $state; store_data = $storeData
    bob_account_id = [string]$bob.account_id
})
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'alice-ready.marker'), "ready`n")
Write-Host 'ALICE PREPARED SUCCESSFULLY. This window may be closed.'

