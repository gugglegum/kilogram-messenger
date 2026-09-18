$root = Split-Path -Parent $PSScriptRoot
$common = Join-Path $root 'common.ps1'
$receive = Join-Path $PSScriptRoot '02_RECEIVE_BOB.ps1'
$cli = Join-Path $root 'kilogram-cli.exe'
$buildInfoPath = Join-Path $root 'BUILD-INFO.json'
$shared = Join-Path $root '1\shared'
$evidence = Join-Path $shared 'evidence'
$manifestPath = Join-Path $evidence '01-provider-offers-publication.json'
$expectedRevision = 'b095d7a5a2ff2f5ab2e1b8439b9ccd64e1e3bfd3'
$expectedCliHash = '07F61075A82CBB1DA421B49E13FFE6F79EAC8EAD331EC644D076C6AF345AC25D'
$expectedCliBytes = 57240576
$expectedCommonHash = '28A053F766345B48CA7815670ED87D20DF7C4B6DB142DB647587CE198684D3E9'
$expectedReceiveHash = 'A3A94B0F9D0EA89923A3433042692C64247C377048E3B2CC9FA57FD361CAC15C'
$deadline = [DateTime]::UtcNow.AddMinutes(20)
$nextProgress = [DateTime]::UtcNow
$snapshotDirectory = $null

Write-Host 'Waiting for the exact fixed CLI, recovery scripts and fresh provider offers from Yandex Disk...'
while ([DateTime]::UtcNow -lt $deadline) {
    $candidateSnapshot = $null
    try {
        foreach ($path in @($common, $receive, $cli, $buildInfoPath, $manifestPath)) {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "not synchronized: $path" }
        }
        if ((Get-FileHash -LiteralPath $common -Algorithm SHA256).Hash -cne $expectedCommonHash -or
            (Get-FileHash -LiteralPath $receive -Algorithm SHA256).Hash -cne $expectedReceiveHash -or
            (Get-FileHash -LiteralPath $cli -Algorithm SHA256).Hash -cne $expectedCliHash -or
            (Get-Item -LiteralPath $cli).Length -ne $expectedCliBytes) {
            throw 'fixed files are not synchronized yet'
        }
        $build = Get-Content -LiteralPath $buildInfoPath -Raw -ErrorAction Stop | ConvertFrom-Json
        $cliArtifact = @($build.artifacts | Where-Object { [string]$_.file -ceq 'kilogram-cli.exe' })
        if ([string]$build.source_revision -cne $expectedRevision -or $cliArtifact.Count -ne 1 -or
            ([string]$cliArtifact[0].sha256).ToUpperInvariant() -cne $expectedCliHash -or
            [Int64]$cliArtifact[0].bytes -ne $expectedCliBytes) {
            throw 'BUILD-INFO.json does not describe the fixed CLI yet'
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
        if ($providers.Count -ne 2) { throw 'provider publication is incomplete' }

        $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $candidateSnapshot = Join-Path $env:LOCALAPPDATA (
            'Kilogram\M0967\offer-snapshots\' + [Guid]::NewGuid().ToString('N')
        )
        New-Item -ItemType Directory -Path $candidateSnapshot -Force | Out-Null
        foreach ($provider in @('provider1', 'provider2')) {
            $entry = $providers[$provider]
            if ([Int64]$entry.expires_at_unix_seconds -le ($now + 180)) {
                throw "$provider offer is too close to expiry"
            }
            $source = Join-Path $evidence "01-$provider.offer"
            $expectedHash = ([string]$entry.sha256).ToUpperInvariant()
            if (-not (Test-Path -LiteralPath $source -PathType Leaf) -or
                (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $expectedHash) {
                throw "$provider offer and publication manifest are not synchronized"
            }
            $destination = Join-Path $candidateSnapshot "01-$provider.offer"
            [IO.File]::WriteAllBytes($destination, [IO.File]::ReadAllBytes($source))
            if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -cne $expectedHash) {
                throw "$provider local offer snapshot is inconsistent"
            }
        }
        if ((Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash -cne $manifestHashBefore) {
            throw 'provider publication changed while it was copied'
        }
        $snapshotDirectory = $candidateSnapshot
        break
    }
    catch {
        if ($null -ne $candidateSnapshot -and (Test-Path -LiteralPath $candidateSnapshot)) {
            Remove-Item -LiteralPath $candidateSnapshot -Recurse -Force
        }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            Write-Host "Still synchronizing: $($_.Exception.Message)"
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Seconds 2
    }
}
if ($null -eq $snapshotDirectory) {
    throw 'Timed out waiting for the exact fixed recovery set and fresh provider offers.'
}

Write-Host 'FIXED CLI AND FRESH PROVIDER OFFERS ARE READY.'
Write-Host 'Resuming the one already committed Bob message; no new message will be created.'
$previousOfferDirectory = $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY
try {
    $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY = $snapshotDirectory
    & $receive
}
finally {
    $env:KILOGRAM_M0967_PROVIDER_OFFER_DIRECTORY = $previousOfferDirectory
}
