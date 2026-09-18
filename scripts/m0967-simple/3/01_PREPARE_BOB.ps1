. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$alicePublicPath = Wait-M0967File (Join-Path $script:SharedDirectory 'alice-public.json') 1800 'Alice public identity'
$alice = Get-Content -LiteralPath $alicePublicPath -Raw | ConvertFrom-Json

$private = Get-M0967PrivateRoot 'bob' ([string]$run.run_id)
if (Test-Path -LiteralPath $private) { throw "Bob private directory already exists: $private" }
$account = Join-Path $private 'account-root'
$state = Join-Path $private 'state'
$public = Join-Path $private 'public'
$profile = Join-Path $private 'runtime-profile.json'
$ipc = Join-Path $private 'runtime.ipc.json'
$deviceCertificate = Join-Path $public 'device.cert'
$deviceList = Join-Path $public 'device-list.kadl'
New-M0967Directory $private
New-M0967Directory $public

$accountOutput = @(Invoke-M0967Cli @('account-create', '--account-dir', $account))
$bobAccount = Get-M0967ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
$null = Invoke-M0967Cli @('device-enroll', '--account-dir', $account, '--state-dir', $state, '--certificate-file', $deviceCertificate)
$null = Invoke-M0967Cli @('account-device-list', '--account-dir', $account, '--device-certificate-file', $deviceCertificate, '--device-list-file', $deviceList)
$identityOutput = @(Invoke-M0967Cli @('identity', '--state-dir', $state))
$bobDevice = Get-M0967ExactValue $identityOutput 'device_id' '[0-9a-f]{64}'
Write-M0967JsonNew (Join-Path $script:SharedDirectory 'bob-public.json') ([ordered]@{
    account_id = $bobAccount
    device_id = $bobDevice
})

Write-Host 'Bob identity is ready. Waiting for Alice conversation membership...'
$membership = Wait-M0967File (Join-Path $script:SharedDirectory 'conversation.membership') 1800 'conversation membership'
$null = Invoke-M0967Cli @('conversation-membership-install', '--state-dir', $state, '--membership-file', $membership)
$null = Invoke-M0967Cli @(
    'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
    '--allow-account', ([string]$alice.account_id), '--device-list-file', $deviceList,
    '--ticket-file', (Join-Path $script:SharedDirectory 'bob.ticket'), '--ipc-file', $ipc,
    '--route-policy', 'auto', '--relay-wait-seconds', '30', '--disable-volunteer-storage'
)

$bootstrapLog = Join-Path $private 'bob-ticket-bootstrap.log'
$runtime = $null
try {
    $runtime = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $bootstrapLog
    $null = Wait-M0967File (Join-Path $script:SharedDirectory 'bob.ticket') 120 'Bob ticket'
    $aliceTicket = Wait-M0967File (Join-Path $script:SharedDirectory 'alice.ticket') 1800 'Alice ticket'
}
finally { Stop-M0967Process $runtime }
$null = Invoke-M0967Cli @(
    'runtime-contact-add', '--state-dir', $state, '--conversation', ([string]$run.conversation_label),
    '--expect-account', ([string]$alice.account_id), '--descriptor-file', $aliceTicket
)
$null = Invoke-M0967Cli @(
    'runtime-mailbox-offer-create', '--state-dir', $state, '--conversation', ([string]$run.conversation_label),
    '--peer-account', ([string]$alice.account_id), '--peer-device', ([string]$alice.device_id),
    '--service-base-url', ([string]$alice.compatibility_store_url),
    '--store-key', ([string]$alice.compatibility_store_key),
    '--output-file', (Join-Path $script:SharedDirectory 'bob-mailbox.offer')
)
Write-M0967JsonNew (Join-Path $private 'role.json') ([ordered]@{
    profile = $profile; ipc = $ipc; state = $state; alice_account_id = [string]$alice.account_id
})
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'bob-ready.marker'), "ready`n")
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'alice-ready.marker') 1800 'Alice ready marker'
Write-Host 'BOB PREPARED SUCCESSFULLY. This window may be closed.'

