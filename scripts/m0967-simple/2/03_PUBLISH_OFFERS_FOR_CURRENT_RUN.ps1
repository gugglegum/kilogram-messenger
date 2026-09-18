. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$stop = Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'
$manifestPath = Join-Path $script:EvidenceDirectory '01-provider-offers-publication.json'

Write-Host 'CURRENT-RUN OFFER PUBLISHER STARTED. It will stop with the providers.'
while (-not (Test-Path -LiteralPath $stop -PathType Leaf)) {
    $publication = [ordered]@{
        version = 1
        published_at_unix_seconds = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        providers = @()
    }
    foreach ($provider in @('provider1', 'provider2')) {
        $logPath = Join-Path $script:EvidenceDirectory "01-$provider.log"
        $lines = @(Get-Content -LiteralPath $logPath -ErrorAction Stop)
        $offers = [Collections.Generic.List[object]]::new()
        for ($index = 0; $index -lt $lines.Count; $index++) {
            if ($lines[$index] -cnotmatch '^runtime_volunteer_storage_offer=([A-Za-z0-9_-]+)$') { continue }
            $encoded = $Matches[1]
            $expires = $null
            for ($next = $index + 1; $next -lt [Math]::Min($index + 5, $lines.Count); $next++) {
                if ($lines[$next] -cmatch '^runtime_volunteer_storage_offer_expires_at_unix_seconds=([0-9]+)$') {
                    $expires = [UInt64]$Matches[1]
                    break
                }
            }
            if ($null -ne $expires) {
                $offers.Add([PSCustomObject]@{ encoded = $encoded; expires = $expires })
            }
        }
        if ($offers.Count -lt 1) { throw "$provider runtime log contains no complete offer+expiry pair" }
        $latest = $offers[$offers.Count - 1]
        $offerPath = Join-Path $script:EvidenceDirectory "01-$provider.offer"
        $offerTemp = "$offerPath.publisher-$PID"
        [IO.File]::WriteAllText($offerTemp, ($latest.encoded + "`n"), [Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $offerTemp -Destination $offerPath -Force
        $publication.providers += [ordered]@{
            name = $provider
            sha256 = (Get-FileHash -LiteralPath $offerPath -Algorithm SHA256).Hash
            expires_at_unix_seconds = $latest.expires
        }
    }
    $manifestTemp = "$manifestPath.publisher-$PID"
    [IO.File]::WriteAllText(
        $manifestTemp,
        (($publication | ConvertTo-Json -Depth 4) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    Move-Item -LiteralPath $manifestTemp -Destination $manifestPath -Force
    Start-Sleep -Seconds 10
}
Write-Host 'CURRENT-RUN OFFER PUBLISHER STOPPED.'
