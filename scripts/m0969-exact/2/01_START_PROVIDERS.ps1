. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0969Kit
$run = Get-M0969Run
New-M0969Directory $script:EvidenceDirectory

$privateBase = Get-M0969PrivateRoot 'providers' ([string]$run.run_id)
if (Test-Path -LiteralPath $privateBase) { throw "Provider private directory already exists: $privateBase" }
New-M0969Directory $privateBase

$profiles = @{}
foreach ($name in @('provider1', 'provider2')) {
    $private = Join-Path $privateBase $name
    $account = Join-Path $private 'account-root'
    $state = Join-Path $private 'state'
    $public = Join-Path $private 'public'
    $certificate = Join-Path $public 'device.cert'
    $deviceList = Join-Path $public 'device-list.kadl'
    $profile = Join-Path $private 'runtime-profile.json'
    New-M0969Directory $private
    New-M0969Directory $public
    $accountOutput = @(Invoke-M0969Cli @('account-create', '--account-dir', $account))
    $accountId = Get-M0969ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
    $null = Invoke-M0969Cli @(
        'device-enroll', '--account-dir', $account, '--state-dir', $state,
        '--certificate-file', $certificate
    )
    $null = Invoke-M0969Cli @(
        'account-device-list', '--account-dir', $account, '--device-certificate-file', $certificate,
        '--device-list-file', $deviceList
    )
    $null = Invoke-M0969Cli @(
        'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
        '--allow-account', $accountId, '--device-list-file', $deviceList,
        '--ticket-file', (Join-Path $private 'runtime.ticket'),
        '--ipc-file', (Join-Path $private 'runtime.ipc.json'),
        '--route-policy', $script:M0969FieldRoutePolicy,
        '--relay-url', $script:M0969FieldRelayUrl,
        '--relay-wait-seconds', '30',
        '--volunteer-storage-data-dir', (Join-Path $private 'volunteer-storage')
    )
    $profiles[$name] = $profile
}

$processes = @{}
try {
    foreach ($name in @('provider1', 'provider2')) {
        $log = Join-Path $script:EvidenceDirectory "01-$name.log"
        $process = Start-M0969Process $script:CliPath @(
            'runtime-from-profile', '--profile-file', $profiles[$name]
        ) $log
        $processes[$name] = $process
        $text = Wait-M0969LogPattern $log '^status=runtime-listening$' $process 180
        if (-not [regex]::IsMatch(
            $text,
            '(?m)^runtime_volunteer_storage_offer=[A-Za-z0-9_-]+$'
        )) { throw "$name did not publish a volunteer offer" }
    }
    Publish-M0969ProviderOffers
    [IO.File]::WriteAllText(
        (Join-Path $script:SharedDirectory 'providers-ready.marker'),
        "ready-before-mailbox-activation`n"
    )
    Write-Host 'PROVIDERS ARE READY BEFORE MAILBOX ACTIVATION.'
    Write-Host 'Leave this window open until final verification stops the providers.'
    $stop = Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'
    $nextPublication = [DateTime]::UtcNow.AddSeconds(10)
    while (-not (Test-Path -LiteralPath $stop -PathType Leaf)) {
        foreach ($entry in $processes.GetEnumerator()) {
            $entry.Value.Refresh()
            if ($entry.Value.HasExited) { throw "$($entry.Key) stopped unexpectedly" }
        }
        if ([DateTime]::UtcNow -ge $nextPublication) {
            Publish-M0969ProviderOffers
            $nextPublication = [DateTime]::UtcNow.AddSeconds(10)
        }
        Start-Sleep -Seconds 2
    }
}
finally {
    foreach ($process in $processes.Values) { Stop-M0969Process $process }
    [IO.File]::WriteAllText(
        (Join-Path $script:SharedDirectory 'providers-stopped.marker'),
        "stopped`n"
    )
}
Write-Host 'PROVIDERS STOPPED.'
