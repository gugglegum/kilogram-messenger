. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$build = Assert-M0969Kit
$noHttpsCompatibility = [string]$build.milestone -ceq 'M0.9.72'
$labelPrefix = if ($noHttpsCompatibility) { 'm0972' } else { 'm0969' }
New-M0969Directory $script:SharedDirectory
New-M0969Directory $script:EvidenceDirectory
if ($noHttpsCompatibility) {
    Assert-M0972HttpsFixtureAbsent
    [IO.File]::WriteAllLines(
        (Join-Path $script:EvidenceDirectory '00-https-fixture-absence.log'),
        @(
            'field_phase=before-identity-and-mailbox-activation'
            'https_fixture_binary_present=false'
            'https_fixture_process_started=false'
            "compatibility_endpoint=$script:M0972CompatibilityStoreUrl"
            'compatibility_endpoint_reachable=false'
        ),
        [Text.UTF8Encoding]::new($false)
    )
}

$runPath = Join-Path $script:SharedDirectory 'run.json'
if (Test-Path -LiteralPath $runPath) {
    throw 'This kit already has a run. Generate a fresh kit for another clean field attempt.'
}
$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$run = [ordered]@{
    schema = 1
    run_id = $runId
    build_commit = [string]$build.source_revision
    conversation_label = "$labelPrefix-$runId"
    message_marker = "kilogram-$labelPrefix-$runId"
}
Write-M0969JsonNew $runPath $run

$private = Get-M0969PrivateRoot 'alice' $runId
if (Test-Path -LiteralPath $private) { throw "Alice private directory already exists: $private" }
$account = Join-Path $private 'account-root'
$state = Join-Path $private 'state'
$public = Join-Path $private 'public'
$profile = Join-Path $private 'runtime-profile.json'
$ipc = Join-Path $private 'runtime.ipc.json'
$deviceCertificate = Join-Path $public 'device.cert'
$deviceList = Join-Path $public 'device-list.kadl'
$storeData = if ($noHttpsCompatibility) { '' } else { Join-Path $private 'compatibility-store' }
New-M0969Directory $private
New-M0969Directory $public

$accountOutput = @(Invoke-M0969Cli @('account-create', '--account-dir', $account))
$aliceAccount = Get-M0969ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
$null = Invoke-M0969Cli @(
    'device-enroll', '--account-dir', $account, '--state-dir', $state,
    '--certificate-file', $deviceCertificate
)
$null = Invoke-M0969Cli @(
    'account-device-list', '--account-dir', $account, '--device-certificate-file', $deviceCertificate,
    '--device-list-file', $deviceList
)
$identityOutput = @(Invoke-M0969Cli @('identity', '--state-dir', $state))
$aliceDevice = Get-M0969ExactValue $identityOutput 'device_id' '[0-9a-f]{64}'

$storeKey = $script:M0972CompatibilityStoreKey
$storeUrl = $script:M0972CompatibilityStoreUrl
if (-not $noHttpsCompatibility) {
    $storeBootstrapLog = Join-Path $private 'compatibility-store-bootstrap.log'
    $storeProcess = $null
    try {
        $storeProcess = Start-M0969Process $script:StorePath @('--data-dir', $storeData) $storeBootstrapLog
        $storeText = Wait-M0969LogPattern `
            $storeBootstrapLog '^blind_mailbox_store_key=([0-9a-f]{64})$' $storeProcess 60
        $storeKey = [regex]::Match(
            $storeText,
            '(?m)^blind_mailbox_store_key=([0-9a-f]{64})$'
        ).Groups[1].Value
        $storeUrl = 'http://127.0.0.1:8787'
    }
    finally { Stop-M0969Process $storeProcess }
}

Write-M0969JsonNew (Join-Path $script:SharedDirectory 'alice-public.json') ([ordered]@{
    account_id = $aliceAccount
    device_id = $aliceDevice
    compatibility_store_key = $storeKey
    compatibility_store_url = $storeUrl
})
Write-Host 'Alice identity is ready. Start providers, then prepare Bob on the laptop.'
$bobPublicPath = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'bob-public.json') 1800 'Bob public identity'
$bob = Get-Content -LiteralPath $bobPublicPath -Raw | ConvertFrom-Json

$membership = Join-Path $script:SharedDirectory 'conversation.membership'
$null = Invoke-M0969Cli @(
    'conversation-create', '--account-dir', $account, '--conversation', $run.conversation_label,
    '--member-account', $aliceAccount, '--member-account', ([string]$bob.account_id),
    '--membership-file', $membership
)
$null = Invoke-M0969Cli @(
    'conversation-membership-install', '--state-dir', $state, '--membership-file', $membership
)
$null = Invoke-M0969Cli @(
    'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
    '--allow-account', ([string]$bob.account_id), '--device-list-file', $deviceList,
    '--ticket-file', (Join-Path $script:SharedDirectory 'alice.ticket'), '--ipc-file', $ipc,
    '--route-policy', $script:M0969FieldRoutePolicy,
    '--relay-url', $script:M0969FieldRelayUrl,
    '--relay-wait-seconds', '30', '--disable-volunteer-storage'
)

$bootstrapLog = Join-Path $private 'alice-ticket-bootstrap.log'
$runtime = $null
try {
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $bootstrapLog
    $null = Wait-M0969File (Join-Path $script:SharedDirectory 'alice.ticket') 120 'Alice ticket'
    $bobTicket = Wait-M0969File (Join-Path $script:SharedDirectory 'bob.ticket') 1800 'Bob ticket'
    $bobTicketBootstrapHash = (Get-FileHash -LiteralPath $bobTicket -Algorithm SHA256).Hash
}
finally { Stop-M0969Process $runtime }
$null = Invoke-M0969Cli @(
    'runtime-contact-add', '--state-dir', $state, '--conversation', $run.conversation_label,
    '--expect-account', ([string]$bob.account_id), '--descriptor-file', $bobTicket
)

$offer = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'bob-mailbox.offer') 1800 'Bob exact-locator mailbox offer'
$offerImport = @(Invoke-M0969Cli @(
    'runtime-mailbox-offer-import', '--state-dir', $state, '--conversation', $run.conversation_label,
    '--peer-account', ([string]$bob.account_id), '--offer-file', $offer
))
[IO.File]::WriteAllLines(
    (Join-Path $script:EvidenceDirectory '03-alice-mailbox-offer-import.log'),
    $offerImport,
    [Text.UTF8Encoding]::new($false)
)

$convergenceLocal = Join-Path $private 'alice-capability-convergence.log'
$convergenceShared = Join-Path $script:EvidenceDirectory '03-alice-capability-convergence.log'
$runtime = $null
try {
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $runtime = Start-M0969Process `
        $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $convergenceLocal
    Wait-M0969IpcReady $ipc $runtime 120
    $bobTicketFreshHash = Wait-M0969FileHashChange `
        $bobTicket $bobTicketBootstrapHash $runtime 300 'Bob live convergence ticket'
    $contactRefresh = @(Invoke-M0969CliWithRetry @(
        'runtime-ipc-contact-add', '--ipc-file', $ipc,
        '--conversation', ([string]$run.conversation_label),
        '--expect-account', ([string]$bob.account_id), '--descriptor-file', $bobTicket
    ))
    if ('runtime_contact_update=live-ipc' -cnotin $contactRefresh -or
        'endpoint_candidate_added=false' -cnotin $contactRefresh -or
        'status=runtime-contact-ready' -cnotin $contactRefresh) {
        throw 'Alice did not validate Bob fresh ticket against the enrolled Device.'
    }
    [IO.File]::WriteAllLines(
        (Join-Path $script:EvidenceDirectory '03-alice-live-contact-refresh.log'),
        @(
            "bootstrap_ticket_sha256=$($bobTicketBootstrapHash.ToLowerInvariant())"
            "live_ticket_sha256=$($bobTicketFreshHash.ToLowerInvariant())"
        ) + $contactRefresh,
        [Text.UTF8Encoding]::new($false)
    )
    $text = Wait-M0969LogPattern `
        $convergenceLocal '^status=runtime-mailbox-capability-updated$' $runtime 300
    if (-not [regex]::IsMatch(
        $text,
        '(?m)^mailbox_capability_update_store=(Inserted|AlreadyPresent)$'
    )) { throw 'Alice did not durably apply Bob mailbox capability update.' }
    [IO.File]::WriteAllText(
        (Join-Path $script:SharedDirectory 'alice-capability-applied.marker'),
        "applied`n"
    )
    $null = Wait-M0969File `
        (Join-Path $script:SharedDirectory 'bob-capability-acked.marker') 180 'Bob capability ACK'
}
finally {
    Stop-M0969Process $runtime
    Publish-M0969File $convergenceLocal $convergenceShared
    Publish-M0969File "$convergenceLocal.stderr" "$convergenceShared.stderr"
}

Write-M0969JsonNew (Join-Path $script:EvidenceDirectory 'manifest.json') ([ordered]@{
    schema = 1
    run_id = $runId
    build_commit = [string]$build.source_revision
    conversation_label = $run.conversation_label
    alice_account_id = $aliceAccount
    bob_account_id = [string]$bob.account_id
    message_marker = $run.message_marker
    route_policy = $script:M0969FieldRoutePolicy
    relay_url = $script:M0969FieldRelayUrl
    evidence_milestone = [string]$build.milestone
    https_fixture_present_at_start = (-not $noHttpsCompatibility)
    compatibility_endpoint = $storeUrl
})
Write-M0969JsonNew (Join-Path $private 'role.json') ([ordered]@{
    profile = $profile
    ipc = $ipc
    state = $state
    store_data = $storeData
    bob_account_id = [string]$bob.account_id
})
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'alice-ready.marker'), "ready`n")
$null = Wait-M0969File (Join-Path $script:SharedDirectory 'bob-ready.marker') 180 'Bob prepared marker'
Write-Host 'ALICE PREPARED WITH THE AUTHENTICATED EXACT LOCATOR.'
