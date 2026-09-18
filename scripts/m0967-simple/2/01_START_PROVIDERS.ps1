. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'alice-ready.marker') 1800 'Alice ready marker'
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'bob-ready.marker') 1800 'Bob ready marker'
$privateBase = Get-M0967PrivateRoot 'providers' ([string]$run.run_id)
if (Test-Path -LiteralPath $privateBase) { throw "Provider private directory already exists: $privateBase" }
New-M0967Directory $privateBase

$profiles = @{}
foreach ($name in @('provider1', 'provider2')) {
    $private = Join-Path $privateBase $name
    $account = Join-Path $private 'account-root'
    $state = Join-Path $private 'state'
    $public = Join-Path $private 'public'
    $certificate = Join-Path $public 'device.cert'
    $deviceList = Join-Path $public 'device-list.kadl'
    $profile = Join-Path $private 'runtime-profile.json'
    New-M0967Directory $private
    New-M0967Directory $public
    $accountOutput = @(Invoke-M0967Cli @('account-create', '--account-dir', $account))
    $accountId = Get-M0967ExactValue $accountOutput 'account_id' '[0-9a-f]{64}'
    $null = Invoke-M0967Cli @('device-enroll', '--account-dir', $account, '--state-dir', $state, '--certificate-file', $certificate)
    $null = Invoke-M0967Cli @('account-device-list', '--account-dir', $account, '--device-certificate-file', $certificate, '--device-list-file', $deviceList)
    $null = Invoke-M0967Cli @(
        'runtime-profile-create', '--profile-file', $profile, '--state-dir', $state,
        '--allow-account', $accountId, '--device-list-file', $deviceList,
        '--ticket-file', (Join-Path $private 'runtime.ticket'), '--ipc-file', (Join-Path $private 'runtime.ipc.json'),
        '--route-policy', 'auto', '--relay-wait-seconds', '30',
        '--volunteer-storage-data-dir', (Join-Path $private 'volunteer-storage')
    )
    $profiles[$name] = $profile
}

$processes = @{}
try {
    foreach ($name in @('provider1', 'provider2')) {
        $log = Join-Path $script:EvidenceDirectory "01-$name.log"
        $process = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profiles[$name]) $log
        $processes[$name] = $process
        $text = Wait-M0967LogPattern $log '^status=runtime-listening$' $process 180
        $offerMatches = [regex]::Matches($text, '(?m)^runtime_volunteer_storage_offer=([A-Za-z0-9_-]+)$')
        if ($offerMatches.Count -lt 1) { throw "$name did not publish a volunteer offer" }
        [IO.File]::WriteAllText(
            (Join-Path $script:EvidenceDirectory "01-$name.offer"),
            ($offerMatches[$offerMatches.Count - 1].Groups[1].Value + "`n"),
            [Text.UTF8Encoding]::new($false)
        )
    }
    [IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'providers-ready.marker'), "ready`n")
    Write-Host 'PROVIDERS ARE READY. Leave this window open until the final verification stops them.'
    $stop = Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'
    $nextOfferRefresh = [DateTime]::UtcNow
    while (-not (Test-Path -LiteralPath $stop -PathType Leaf)) {
        foreach ($entry in $processes.GetEnumerator()) {
            if ($entry.Value.HasExited) { throw "$($entry.Key) stopped unexpectedly" }
        }
        if ([DateTime]::UtcNow -ge $nextOfferRefresh) {
            Update-M0967ProviderOfferFiles
            $nextOfferRefresh = [DateTime]::UtcNow.AddSeconds(10)
        }
        Start-Sleep -Seconds 2
    }
}
finally {
    foreach ($process in $processes.Values) { Stop-M0967Process $process }
    [IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'providers-stopped.marker'), "stopped`n")
}
Write-Host 'PROVIDERS STOPPED.'
