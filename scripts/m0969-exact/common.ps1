Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:KitRoot = [IO.Path]::GetFullPath($PSScriptRoot)
$script:CliPath = Join-Path $script:KitRoot 'kilogram-cli.exe'
$script:StorePath = Join-Path $script:KitRoot 'kilogram-ticket-store.exe'
$script:SharedDirectory = Join-Path $script:KitRoot '1\shared'
$script:EvidenceDirectory = Join-Path $script:SharedDirectory 'evidence'
$script:BuildInfoPath = Join-Path $script:KitRoot 'BUILD-INFO.json'
$script:M0969FieldRoutePolicy = 'auto'
$script:M0969FieldRelayUrl = 'https://aps1-1.relay.n0.iroh.link./'
$script:M0972CompatibilityStoreUrl = 'http://127.0.0.1:18787'
$script:M0972CompatibilityStoreKey = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a'
$script:FieldMilestone = $null

function Assert-M0969Kit {
    foreach ($path in @($script:CliPath, $script:BuildInfoPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "M0.9.69 kit file is missing: $path"
        }
    }
    $build = Get-Content -LiteralPath $script:BuildInfoPath -Raw | ConvertFrom-Json
    $milestone = [string]$build.milestone
    if ($milestone -cnotin @('M0.9.69', 'M0.9.72')) {
        throw "Unsupported exact-locator field milestone: $milestone"
    }
    if ($milestone -ceq 'M0.9.69' -and
        -not (Test-Path -LiteralPath $script:StorePath -PathType Leaf)) {
        throw "M0.9.69 compatibility store is missing: $script:StorePath"
    }
    if ($milestone -ceq 'M0.9.72' -and
        (Test-Path -LiteralPath $script:StorePath)) {
        throw 'M0.9.72 must not contain the HTTPS compatibility store executable.'
    }
    foreach ($artifact in @($build.artifacts)) {
        $path = Join-Path $script:KitRoot ([string]$artifact.file).Replace('/', '\')
        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -cne [string]$artifact.sha256 -or [UInt64]$item.Length -ne [UInt64]$artifact.bytes) {
            throw "M0.9.69 kit artifact differs from BUILD-INFO.json: $path"
        }
    }
    $script:FieldMilestone = $milestone
    return $build
}

function Test-M0972CompatibilityEndpointReachable {
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $pending = $client.ConnectAsync('127.0.0.1', 18787)
        if (-not $pending.Wait(750)) { return $false }
        return $client.Connected
    }
    catch { return $false }
    finally { $client.Dispose() }
}

function Assert-M0972HttpsFixtureAbsent {
    if (Test-Path -LiteralPath $script:StorePath) {
        throw 'M0.9.72 contains a forbidden HTTPS compatibility store executable.'
    }
    if (Test-M0972CompatibilityEndpointReachable) {
        throw "M0.9.72 compatibility endpoint is unexpectedly reachable: $script:M0972CompatibilityStoreUrl"
    }
}

function New-M0969Directory {
    param([Parameter(Mandatory)] [string] $Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Write-M0969JsonNew {
    param([Parameter(Mandatory)] [string] $Path, [Parameter(Mandatory)] [object] $Value)
    if (Test-Path -LiteralPath $Path) { throw "Refusing to overwrite run file: $Path" }
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 8) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
}

function Wait-M0969File {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [int] $TimeoutSeconds = 1800,
        [string] $Description = 'Yandex Disk file'
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        if ([DateTime]::UtcNow -ge $deadline) { throw "Timed out waiting for $Description`: $Path" }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            Write-Host "Still waiting for $Description..."
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Seconds 2
    }
    return (Get-Item -LiteralPath $Path).FullName
}

function Wait-M0969FileHashChange {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $PreviousSha256,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 300,
        [string] $Description = 'fresh peer runtime ticket'
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while ([DateTime]::UtcNow -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { throw "Runtime exited while waiting for ${Description}: $Path" }
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            try {
                $current = (Get-FileHash -LiteralPath $Path -Algorithm SHA256 -ErrorAction Stop).Hash
                if ($current -cne $PreviousSha256) { return $current }
            }
            catch {
                # An atomic Yandex Disk replacement can briefly race this read; retry the bounded wait.
            }
        }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            $remaining = [Math]::Max(0, [Math]::Ceiling(($deadline - [DateTime]::UtcNow).TotalSeconds))
            Write-Host "Still waiting for ${Description}; timeout in ${remaining}s."
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Seconds 1
    }
    throw "Timed out waiting for ${Description} to replace its bootstrap ticket: $Path"
}

function Wait-M0969LogPattern {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 180
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
            if ($null -ne $text -and [regex]::IsMatch(
                $text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline
            )) { return $text }
        }
        $Process.Refresh()
        if ($Process.HasExited) {
            $errorText = if (Test-Path -LiteralPath "$Path.stderr") {
                Get-Content -LiteralPath "$Path.stderr" -Raw
            } else { '' }
            throw "Background process exited before '$Pattern'. $errorText"
        }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            $remaining = [Math]::Max(0, [Math]::Ceiling(($deadline - [DateTime]::UtcNow).TotalSeconds))
            Write-Host "Still working: waiting for runtime evidence; timeout in ${remaining}s."
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Milliseconds 500
    }
    throw "Timed out waiting for '$Pattern' in $Path"
}

function Wait-M0969LogCount {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [int] $Count,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 300
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while ([DateTime]::UtcNow -lt $deadline) {
        $matched = 0
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
            if ($null -ne $text) {
                $matched = [regex]::Matches(
                    $text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline
                ).Count
                if ($matched -ge $Count) { return $text }
            }
        }
        $Process.Refresh()
        if ($Process.HasExited) { throw "Background process exited before $Count matches of '$Pattern'." }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            $remaining = [Math]::Max(0, [Math]::Ceiling(($deadline - [DateTime]::UtcNow).TotalSeconds))
            Write-Host "Still working: observed $matched/$Count required events; timeout in ${remaining}s."
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Seconds 1
    }
    throw "Timed out waiting for $Count matches of '$Pattern' in $Path"
}

function Wait-M0969IpcReady {
    param(
        [Parameter(Mandatory)] [string] $IpcFile,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 120
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while ([DateTime]::UtcNow -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { throw "Runtime exited before its IPC endpoint became ready: $IpcFile" }
        if (Test-Path -LiteralPath $IpcFile -PathType Leaf) {
            $previous = $ErrorActionPreference
            try {
                $ErrorActionPreference = 'SilentlyContinue'
                & $script:CliPath runtime-ipc-ping --ipc-file $IpcFile 1>$null 2>$null
                if ($LASTEXITCODE -eq 0) { return }
            }
            finally { $ErrorActionPreference = $previous }
        }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            $remaining = [Math]::Max(0, [Math]::Ceiling(($deadline - [DateTime]::UtcNow).TotalSeconds))
            Write-Host "Still working: waiting for the local runtime IPC; timeout in ${remaining}s."
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Milliseconds 500
    }
    throw "Timed out waiting for a reachable runtime IPC endpoint: $IpcFile"
}

function Start-M0969Process {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPath
    )
    foreach ($path in @($LogPath, "$LogPath.stderr")) {
        if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite process log: $path" }
    }
    New-M0969Directory (Split-Path -Parent $LogPath)
    return Start-Process -FilePath $FilePath -ArgumentList $Arguments -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput $LogPath -RedirectStandardError "$LogPath.stderr"
}

function Stop-M0969Process {
    param([Diagnostics.Process] $Process)
    if ($null -eq $Process) { return }
    $Process.Refresh()
    if (-not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit(10000) | Out-Null
    }
}

function Publish-M0969File {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $Destination
    )
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) { return }
    $temporary = "$Destination.publish-$PID-$([Guid]::NewGuid().ToString('N'))"
    [IO.File]::WriteAllBytes($temporary, [IO.File]::ReadAllBytes($Source))
    Move-Item -LiteralPath $temporary -Destination $Destination -Force
}

function Invoke-M0969Cli {
    param([Parameter(Mandatory)] [string[]] $Arguments)
    $previous = $ErrorActionPreference
    $stderrPath = [IO.Path]::GetTempFileName()
    $output = @()
    $stderr = @()
    $exitCode = -1
    try {
        $ErrorActionPreference = 'SilentlyContinue'
        $output = @(& $script:CliPath @Arguments 2> $stderrPath)
        $exitCode = $LASTEXITCODE
        if (Test-Path -LiteralPath $stderrPath -PathType Leaf) {
            $stderr = @([IO.File]::ReadAllLines($stderrPath))
        }
    }
    finally {
        $ErrorActionPreference = $previous
        Remove-Item -LiteralPath $stderrPath -Force -ErrorAction SilentlyContinue
    }
    $text = @(
        @($output | ForEach-Object { [string]$_ })
        @($stderr | ForEach-Object { [string]$_ })
    )
    if ($exitCode -ne 0) {
        throw "kilogram-cli failed ($exitCode): $($Arguments -join ' ')`n$($text -join "`n")"
    }
    return [string[]]$text
}

function Invoke-M0969CliWithRetry {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [ValidateRange(1, 30)] [int] $Attempts = 12,
        [ValidateRange(100, 10000)] [int] $DelayMilliseconds = 750,
        [Collections.Generic.List[string]] $Evidence
    )
    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        try {
            Write-Host "Running $($Arguments[0]) (attempt $attempt/$Attempts)..."
            return @(Invoke-M0969Cli $Arguments)
        }
        catch {
            $message = ([string]$_.Exception.Message) -replace "`r?`n", ' | '
            if ($null -ne $Evidence) { $Evidence.Add("field_retry_attempt=$attempt/$Attempts error=$message") }
            if ($attempt -eq $Attempts) { throw }
            Start-Sleep -Milliseconds $DelayMilliseconds
        }
    }
}

function Get-M0969ExactValue {
    param(
        [Parameter(Mandatory)] [string[]] $Lines,
        [Parameter(Mandatory)] [string] $Name,
        [string] $Pattern = '.+'
    )
    $matches = @($Lines | Select-String -Pattern "^$([regex]::Escape($Name))=($Pattern)$")
    if ($matches.Count -ne 1) { throw "Expected exactly one $Name value, found $($matches.Count)." }
    return $matches[0].Matches[0].Groups[1].Value
}

function Get-M0969Run {
    $path = Wait-M0969File (Join-Path $script:SharedDirectory 'run.json') 1800 'run manifest'
    return (Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)
}

function Get-M0969PrivateRoot {
    param([Parameter(Mandatory)] [string] $Role, [Parameter(Mandatory)] [string] $RunId)
    $rootName = if ($script:FieldMilestone -ceq 'M0.9.72') { 'M0972' } else { 'M0969' }
    return Join-Path $env:LOCALAPPDATA "Kilogram\$rootName\$RunId\$Role"
}

function Publish-M0969ProviderOffers {
    $publication = [ordered]@{
        schema = 1
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
            if ($null -ne $expires) { $offers.Add([PSCustomObject]@{ encoded = $encoded; expires = $expires }) }
        }
        if ($offers.Count -lt 1) { throw "$provider runtime log contains no complete offer+expiry pair" }
        $latest = $offers[$offers.Count - 1]
        $offerPath = Join-Path $script:EvidenceDirectory "01-$provider.offer"
        $offerTemporary = "$offerPath.publish-$PID"
        [IO.File]::WriteAllText($offerTemporary, ($latest.encoded + "`n"), [Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $offerTemporary -Destination $offerPath -Force
        $publication.providers += [ordered]@{
            name = $provider
            sha256 = (Get-FileHash -LiteralPath $offerPath -Algorithm SHA256).Hash
            expires_at_unix_seconds = $latest.expires
        }
    }
    $manifestPath = Join-Path $script:EvidenceDirectory '01-provider-offers-publication.json'
    $manifestTemporary = "$manifestPath.publish-$PID"
    [IO.File]::WriteAllText(
        $manifestTemporary,
        (($publication | ConvertTo-Json -Depth 4) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    Move-Item -LiteralPath $manifestTemporary -Destination $manifestPath -Force
}

function Get-M0969ProviderOfferSnapshot {
    param([int] $TimeoutSeconds = 900)
    $manifestPath = Join-Path $script:EvidenceDirectory '01-provider-offers-publication.json'
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $nextProgress = [DateTime]::UtcNow
    while ([DateTime]::UtcNow -lt $deadline) {
        $candidate = $null
        try {
            if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { throw 'publication manifest is absent' }
            $manifestHash = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
            $manifest = Get-Content -LiteralPath $manifestPath -Raw -ErrorAction Stop | ConvertFrom-Json
            if ([int]$manifest.schema -ne 1 -or @($manifest.providers).Count -ne 2) {
                throw 'publication manifest has an unexpected shape'
            }
            $entries = @{}
            foreach ($entry in @($manifest.providers)) {
                $name = [string]$entry.name
                if ($name -cnotmatch '^provider[12]$' -or $entries.ContainsKey($name)) {
                    throw 'publication manifest has duplicate or unknown providers'
                }
                $entries[$name] = $entry
            }
            if ($entries.Count -ne 2) { throw 'publication manifest is incomplete' }
            $candidate = Join-Path $env:LOCALAPPDATA (
                'Kilogram\M0969\offer-snapshots\' + [Guid]::NewGuid().ToString('N')
            )
            New-Item -ItemType Directory -Path $candidate -Force | Out-Null
            $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            foreach ($provider in @('provider1', 'provider2')) {
                $entry = $entries[$provider]
                if ([Int64]$entry.expires_at_unix_seconds -le ($now + 180)) {
                    throw "$provider offer is too close to expiry"
                }
                $source = Join-Path $script:EvidenceDirectory "01-$provider.offer"
                $expectedHash = ([string]$entry.sha256).ToUpperInvariant()
                if (-not (Test-Path -LiteralPath $source -PathType Leaf) -or
                    (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $expectedHash) {
                    throw "$provider offer and publication manifest are not synchronized"
                }
                $destination = Join-Path $candidate "01-$provider.offer"
                [IO.File]::WriteAllBytes($destination, [IO.File]::ReadAllBytes($source))
                if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -cne $expectedHash) {
                    throw "$provider local offer snapshot is inconsistent"
                }
            }
            if ((Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash -cne $manifestHash) {
                throw 'publication manifest changed while offers were copied'
            }
            return $candidate
        }
        catch {
            if ($null -ne $candidate -and (Test-Path -LiteralPath $candidate)) {
                Remove-Item -LiteralPath $candidate -Recurse -Force
            }
            if ([DateTime]::UtcNow -ge $nextProgress) {
                Write-Host "Still synchronizing provider offers: $($_.Exception.Message)"
                $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
            }
            Start-Sleep -Seconds 2
        }
    }
    throw 'Timed out waiting for one fresh, consistent two-provider publication.'
}

function Import-M0969Providers {
    param(
        [Parameter(Mandatory)] [string] $Role,
        [Parameter(Mandatory)] [string] $IpcFile,
        [Parameter(Mandatory)] [string] $EvidenceName
    )
    $outputPath = Join-Path $script:EvidenceDirectory $EvidenceName
    if (Test-Path -LiteralPath $outputPath) { throw "Provider import evidence exists: $outputPath" }
    $snapshot = Get-M0969ProviderOfferSnapshot
    $lines = [Collections.Generic.List[string]]::new()
    $lines.Add("field_role=$Role")
    $lines.Add('field_provider_snapshot=manifest-hash-consistent')
    try {
        foreach ($provider in @('provider1', 'provider2')) {
            Write-Host "Importing the fresh $provider offer..."
            $offer = Join-Path $snapshot "01-$provider.offer"
            $result = @(Invoke-M0969CliWithRetry `
                @('runtime-ipc-volunteer-provider-import', '--ipc-file', $IpcFile, '--offer-file', $offer) `
                12 750 $lines)
            $lines.Add("field_provider=$provider")
            foreach ($line in $result) { $lines.Add([string]$line) }
        }
        Write-Host 'Selecting the two imported volunteer providers...'
        $selection = @(Invoke-M0969CliWithRetry `
            @('runtime-ipc-volunteer-provider-select', '--ipc-file', $IpcFile, '--count', '2') `
            12 750 $lines)
        foreach ($line in $selection) { $lines.Add([string]$line) }
        [IO.File]::WriteAllLines($outputPath, $lines, [Text.UTF8Encoding]::new($false))
    }
    finally {
        if (Test-Path -LiteralPath $snapshot) { Remove-Item -LiteralPath $snapshot -Recurse -Force }
    }
}

function Get-M0969ProviderStoreKeys {
    # Provider runtimes keep their diagnostic logs open while serving mailbox
    # traffic. Use the closed import evidence that was produced before mailbox
    # activation instead; it binds the exact provider set causally and can be
    # synchronized while both providers remain online.
    $path = Join-Path $script:EvidenceDirectory '02-bob-providers-before-activation.log'
    $deadline = [DateTime]::UtcNow.AddSeconds(1800)
    $nextProgress = [DateTime]::UtcNow
    while ($true) {
        $keys = @()
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            try {
                $text = Get-Content -LiteralPath $path -Raw -ErrorAction Stop
                $keys = @([regex]::Matches(
                    $text,
                    '(?m)^provider_offer_id=[0-9a-f]{64} .* store_key=([0-9a-f]{64}) .*$'
                ) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
            }
            catch {
                # Yandex Disk can expose a placeholder before its content is locally readable.
                $keys = @()
            }
        }
        if ($keys.Count -eq 2) { return [string[]]$keys }
        if ($keys.Count -gt 2) {
            throw 'Bob pre-activation evidence contains more than two provider store keys.'
        }
        if ([DateTime]::UtcNow -ge $deadline) {
            throw "Timed out waiting for complete Bob pre-activation provider evidence: $path"
        }
        if ([DateTime]::UtcNow -ge $nextProgress) {
            Write-Host 'Still waiting for complete Bob pre-activation provider evidence from Yandex Disk...'
            $nextProgress = [DateTime]::UtcNow.AddSeconds(15)
        }
        Start-Sleep -Seconds 2
    }
}
