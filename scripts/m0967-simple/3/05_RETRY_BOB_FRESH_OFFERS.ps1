$root = Split-Path -Parent $PSScriptRoot
$common = Join-Path $root 'common.ps1'
$receive = Join-Path $PSScriptRoot '02_RECEIVE_BOB.ps1'
$shared = Join-Path $root '1\shared'
$evidence = Join-Path $shared 'evidence'
$manifestPath = Join-Path $evidence '01-provider-offers-publication.json'
$expectedCommonHash = '7DA15846464CDC9C6BFF653B65C5031D02AC10DA37C04E0619D31F62F625E18A'
$expectedReceiveHash = '71720A81BC1D84F5DCDDFB8081F5872152A160C0BF679E37D2F06A2CFC853AC4'
$deadline = [DateTime]::UtcNow.AddMinutes(15)
$actualCommonHash = 'missing'
$actualReceiveHash = 'missing'
$snapshotDirectory = $null

Write-Host 'Waiting for exact Bob scripts and a fresh, consistent two-provider publication from Yandex Disk...'
while ([DateTime]::UtcNow -lt $deadline) {
    $candidateSnapshot = $null
    try {
        if (Test-Path -LiteralPath $common -PathType Leaf) {
            $actualCommonHash = (Get-FileHash -LiteralPath $common -Algorithm SHA256).Hash
        }
        if (Test-Path -LiteralPath $receive -PathType Leaf) {
            $actualReceiveHash = (Get-FileHash -LiteralPath $receive -Algorithm SHA256).Hash
        }
        if ($actualCommonHash -cne $expectedCommonHash -or
            $actualReceiveHash -cne $expectedReceiveHash) {
            Start-Sleep -Seconds 2
            continue
        }
        if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
            Start-Sleep -Seconds 2
            continue
        }

        $manifestHashBefore = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
        $manifest = Get-Content -LiteralPath $manifestPath -Raw -ErrorAction Stop | ConvertFrom-Json
        if ([int]$manifest.version -ne 1 -or @($manifest.providers).Count -ne 2) {
            throw 'provider publication manifest has an unexpected shape'
        }
        $providers = @{}
        foreach ($entry in @($manifest.providers)) {
            $name = [string]$entry.name
            if ($name -cnotmatch '^provider[12]$' -or $providers.ContainsKey($name)) {
                throw 'provider publication manifest has duplicate or unknown provider names'
            }
            $providers[$name] = $entry
        }
        if ($providers.Count -ne 2) { throw 'provider publication manifest does not contain both providers' }

        $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $candidateSnapshot = Join-Path $env:LOCALAPPDATA (
            'Kilogram\M0967\offer-snapshots\' + [Guid]::NewGuid().ToString('N')
        )
        New-Item -ItemType Directory -Path $candidateSnapshot -Force | Out-Null
        $consistent = $true
        foreach ($provider in @('provider1', 'provider2')) {
            $entry = $providers[$provider]
            if ([Int64]$entry.expires_at_unix_seconds -le ($now + 180)) {
                $consistent = $false
                break
            }
            $source = Join-Path $evidence "01-$provider.offer"
            if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
                $consistent = $false
                break
            }
            $expectedHash = ([string]$entry.sha256).ToUpperInvariant()
            if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $expectedHash) {
                $consistent = $false
                break
            }
            $destination = Join-Path $candidateSnapshot "01-$provider.offer"
            [IO.File]::WriteAllBytes($destination, [IO.File]::ReadAllBytes($source))
            if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -cne $expectedHash -or
                (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $expectedHash) {
                $consistent = $false
                break
            }
        }
        $manifestHashAfter = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
        if ($manifestHashAfter -cne $manifestHashBefore) { $consistent = $false }
        if (-not $consistent) {
            Remove-Item -LiteralPath $candidateSnapshot -Recurse -Force
            Start-Sleep -Seconds 2
            continue
        }
        $snapshotDirectory = $candidateSnapshot
        break
    }
    catch {
        if ($null -ne $candidateSnapshot -and (Test-Path -LiteralPath $candidateSnapshot)) {
            Remove-Item -LiteralPath $candidateSnapshot -Recurse -Force
        }
        Start-Sleep -Seconds 2
    }
}
if ($null -eq $snapshotDirectory) {
    throw "Timed out waiting for exact scripts and fresh provider offers. common=$actualCommonHash receive=$actualReceiveHash"
}

Write-Host 'FRESH MATCHING PROVIDER OFFERS ARE SYNCHRONIZED. Starting the safe Bob receive retry.'
$previousOfferDirectory = $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY
try {
    $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY = $snapshotDirectory
    & $receive
}
finally {
    $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY = $previousOfferDirectory
}
