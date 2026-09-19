. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0969Kit
$run = Get-M0969Run
$null = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'providers-ready.marker') 900 'providers ready before activation'
$alicePublicPath = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'alice-public.json') 1800 'Alice public identity'
$alice = Get-Content -LiteralPath $alicePublicPath -Raw | ConvertFrom-Json

$private = Get-M0969PrivateRoot 'bob' ([string]$run.run_id)
if (Test-Path -LiteralPath $private) { throw "Bob private directory already exists: $private" }
$account = Join-Path $private 'account-root'
$state = Join-Path $private 'state'
$public = Join-Path $private 'public'
$profile = Join-Path $private 'runtime-profile.json'
$ipc = Join-Path $private 'runtime.ipc.json'
$deviceCertificate = Join-Path $public 'device.cert'
$deviceList = Join-Path $public 'device-list.kadl'
New-M0969Directory $private
New-M0969Directory $public

$accountOutput = @(Invoke-M0969Cli @('account-create', '--account-dir', $account))
$bobAccount = Get-M0969ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
$null = Invoke-M0969Cli @(
    'device-enroll', '--account-dir', $account, '--state-dir', $state,
    '--certificate-file', $deviceCertificate
)
$null = Invoke-M0969Cli @(
    'account-device-list', '--account-dir', $account, '--device-certificate-file', $deviceCertificate,
    '--device-list-file', $deviceList
)
$identityOutput = @(Invoke-M0969Cli @('identity', '--state-dir', $state))
$bobDevice = Get-M0969ExactValue $identityOutput 'device_id' '[0-9a-f]{64}'
Write-M0969JsonNew (Join-Path $script:SharedDirectory 'bob-public.json') ([ordered]@{
    account_id = $bobAccount
    device_id = $bobDevice
})

Write-Host 'Bob identity is ready. Waiting for Alice conversation membership...'
$membership = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'conversation.membership') 1800 'conversation membership'
$null = Invoke-M0969Cli @(
    'conversation-membership-install', '--state-dir', $state, '--membership-file', $membership
)
$null = Invoke-M0969Cli @(
    'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
    '--allow-account', ([string]$alice.account_id), '--device-list-file', $deviceList,
    '--ticket-file', (Join-Path $script:SharedDirectory 'bob.ticket'), '--ipc-file', $ipc,
    '--route-policy', $script:M0969FieldRoutePolicy,
    '--relay-url', $script:M0969FieldRelayUrl,
    '--relay-wait-seconds', '30', '--disable-volunteer-storage'
)

$bootstrapLog = Join-Path $private 'bob-ticket-and-provider-bootstrap.log'
$runtime = $null
try {
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $bootstrapLog
    Wait-M0969IpcReady $ipc $runtime 120
    $null = Wait-M0969File (Join-Path $script:SharedDirectory 'bob.ticket') 120 'Bob ticket'
    $aliceTicket = Wait-M0969File `
        (Join-Path $script:SharedDirectory 'alice.ticket') 1800 'Alice ticket'
    $aliceTicketBootstrapHash = (Get-FileHash -LiteralPath $aliceTicket -Algorithm SHA256).Hash
    Import-M0969Providers 'bob-before-activation' $ipc '02-bob-providers-before-activation.log'
}
finally { Stop-M0969Process $runtime }

$null = Invoke-M0969Cli @(
    'runtime-contact-add', '--state-dir', $state, '--conversation', ([string]$run.conversation_label),
    '--expect-account', ([string]$alice.account_id), '--descriptor-file', $aliceTicket
)
$activation = @(Invoke-M0969Cli @(
    'runtime-mailbox-offer-create', '--state-dir', $state,
    '--conversation', ([string]$run.conversation_label),
    '--peer-account', ([string]$alice.account_id), '--peer-device', ([string]$alice.device_id),
    '--service-base-url', ([string]$alice.compatibility_store_url),
    '--store-key', ([string]$alice.compatibility_store_key),
    '--output-file', (Join-Path $script:SharedDirectory 'bob-mailbox.offer')
))
$activationPath = Join-Path $script:EvidenceDirectory '03-bob-mailbox-capability.log'
[IO.File]::WriteAllLines($activationPath, $activation, [Text.UTF8Encoding]::new($false))
if ('mailbox_replica_set_discovery=exact-authenticated' -cnotin $activation -or
    'mailbox_replica_set_store_count=2' -cnotin $activation -or
    'mailbox_replica_set_discovery=legacy-random-fallback' -cin $activation) {
    throw 'Bob mailbox activation did not commit the exact two-provider replica set.'
}
$commitmentId = Get-M0969ExactValue `
    $activation 'mailbox_replica_set_commitment_id' '[0-9a-f]{64}'
$updateId = Get-M0969ExactValue $activation 'mailbox_capability_update_id' '[0-9a-f]{64}'

Write-M0969JsonNew (Join-Path $private 'role.json') ([ordered]@{
    profile = $profile
    ipc = $ipc
    state = $state
    alice_account_id = [string]$alice.account_id
    replica_set_commitment_id = $commitmentId
    capability_update_id = $updateId
})

$convergenceLocal = Join-Path $private 'bob-capability-convergence.log'
$convergenceShared = Join-Path $script:EvidenceDirectory '03-bob-capability-convergence.log'
$runtime = $null
try {
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $convergenceLocal
    Wait-M0969IpcReady $ipc $runtime 120
    $aliceTicketFreshHash = Wait-M0969FileHashChange `
        $aliceTicket $aliceTicketBootstrapHash $runtime 300 'Alice live convergence ticket'
    $contactRefresh = @(Invoke-M0969CliWithRetry @(
        'runtime-ipc-contact-add', '--ipc-file', $ipc,
        '--conversation', ([string]$run.conversation_label),
        '--expect-account', ([string]$alice.account_id), '--descriptor-file', $aliceTicket
    ))
    if ('runtime_contact_update=live-ipc' -cnotin $contactRefresh -or
        'endpoint_candidate_added=false' -cnotin $contactRefresh -or
        'status=runtime-contact-ready' -cnotin $contactRefresh) {
        throw 'Bob did not validate Alice fresh ticket against the enrolled Device.'
    }
    [IO.File]::WriteAllLines(
        (Join-Path $script:EvidenceDirectory '03-bob-live-contact-refresh.log'),
        @(
            "bootstrap_ticket_sha256=$($aliceTicketBootstrapHash.ToLowerInvariant())"
            "live_ticket_sha256=$($aliceTicketFreshHash.ToLowerInvariant())"
        ) + $contactRefresh,
        [Text.UTF8Encoding]::new($false)
    )
    $text = Wait-M0969LogPattern `
        $convergenceLocal '^runtime_mailbox_capability_update_status=acknowledged$' $runtime 300
    if (-not [regex]::IsMatch(
        $text,
        "(?m)^mailbox_capability_update_id=$([regex]::Escape($updateId))$"
    )) { throw 'Bob acknowledged a different mailbox capability update.' }
    [IO.File]::WriteAllText(
        (Join-Path $script:SharedDirectory 'bob-capability-acked.marker'),
        "acknowledged`n"
    )
    $null = Wait-M0969File `
        (Join-Path $script:SharedDirectory 'alice-capability-applied.marker') 180 'Alice capability apply marker'
}
finally {
    Stop-M0969Process $runtime
    Publish-M0969File $convergenceLocal $convergenceShared
    Publish-M0969File "$convergenceLocal.stderr" "$convergenceShared.stderr"
}

[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'bob-ready.marker'), "ready`n")
$null = Wait-M0969File (Join-Path $script:SharedDirectory 'alice-ready.marker') 180 'Alice prepared marker'
Write-Host 'BOB PREPARED: PROVIDERS PRECEDED ACTIVATION AND CAPABILITY IS ACKNOWLEDGED.'
