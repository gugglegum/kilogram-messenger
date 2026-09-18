Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:KitRoot = [IO.Path]::GetFullPath($PSScriptRoot)
$script:CliPath = Join-Path $script:KitRoot 'kilogram-cli.exe'
$script:StorePath = Join-Path $script:KitRoot 'kilogram-ticket-store.exe'
$script:SharedDirectory = Join-Path $script:KitRoot '1\shared'
$script:EvidenceDirectory = Join-Path $script:SharedDirectory 'evidence'
$script:BuildInfoPath = Join-Path $script:KitRoot 'BUILD-INFO.json'

function Assert-M0967Kit {
    foreach ($path in @($script:CliPath, $script:StorePath, $script:BuildInfoPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "M0.9.67 kit file is missing: $path"
        }
    }
    $build = Get-Content -LiteralPath $script:BuildInfoPath -Raw | ConvertFrom-Json
    foreach ($artifact in @($build.artifacts)) {
        $path = Join-Path $script:KitRoot ([string]$artifact.file).Replace('/', '\')
        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -cne [string]$artifact.sha256 -or [UInt64]$item.Length -ne [UInt64]$artifact.bytes) {
            throw "M0.9.67 kit artifact differs from BUILD-INFO.json: $path"
        }
    }
    return $build
}

function New-M0967Directory {
    param([Parameter(Mandatory)] [string] $Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Write-M0967JsonNew {
    param([Parameter(Mandatory)] [string] $Path, [Parameter(Mandatory)] [object] $Value)
    if (Test-Path -LiteralPath $Path) { throw "Refusing to overwrite run file: $Path" }
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 6) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
}

function Wait-M0967File {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [int] $TimeoutSeconds = 1800,
        [string] $Description = 'Yandex Disk file'
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        if ([DateTime]::UtcNow -ge $deadline) { throw "Timed out waiting for $Description`: $Path" }
        Start-Sleep -Seconds 2
    }
    return (Get-Item -LiteralPath $Path).FullName
}

function Wait-M0967LogPattern {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 180
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
            if ($null -ne $text -and [regex]::IsMatch(
                $text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline
            )) { return $text }
        }
        if ($Process.HasExited) {
            $errorText = if (Test-Path -LiteralPath "$Path.stderr") {
                Get-Content -LiteralPath "$Path.stderr" -Raw
            } else { '' }
            throw "Background process exited before '$Pattern'. $errorText"
        }
        Start-Sleep -Milliseconds 500
    }
    throw "Timed out waiting for '$Pattern' in $Path"
}

function Wait-M0967LogCount {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Pattern,
        [Parameter(Mandatory)] [int] $Count,
        [Parameter(Mandatory)] [Diagnostics.Process] $Process,
        [int] $TimeoutSeconds = 240
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $text = Get-Content -LiteralPath $Path -Raw -ErrorAction SilentlyContinue
            if ($null -ne $text -and [regex]::Matches(
                $text, $Pattern, [Text.RegularExpressions.RegexOptions]::Multiline
            ).Count -ge $Count) {
                return $text
            }
        }
        if ($Process.HasExited) { throw "Background process exited before $Count matches of '$Pattern'." }
        Start-Sleep -Seconds 1
    }
    throw "Timed out waiting for $Count matches of '$Pattern' in $Path"
}

function Start-M0967Process {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $LogPath
    )
    foreach ($path in @($LogPath, "$LogPath.stderr")) {
        if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite process log: $path" }
    }
    New-M0967Directory (Split-Path -Parent $LogPath)
    return Start-Process -FilePath $FilePath -ArgumentList $Arguments -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput $LogPath -RedirectStandardError "$LogPath.stderr"
}

function Stop-M0967Process {
    param([Diagnostics.Process] $Process)
    if ($null -eq $Process) { return }
    $Process.Refresh()
    if (-not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit(10000) | Out-Null
    }
}

function Invoke-M0967Cli {
    param([Parameter(Mandatory)] [string[]] $Arguments)
    $output = @(& $script:CliPath @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) { throw "kilogram-cli failed ($exitCode): $($Arguments -join ' ')" }
    return [string[]]$output
}

function Get-M0967ExactValue {
    param(
        [Parameter(Mandatory)] [string[]] $Lines,
        [Parameter(Mandatory)] [string] $Name,
        [string] $Pattern = '.+'
    )
    $matches = @($Lines | Select-String -Pattern "^$([regex]::Escape($Name))=($Pattern)$")
    if ($matches.Count -ne 1) { throw "Expected exactly one $Name value, found $($matches.Count)." }
    return $matches[0].Matches[0].Groups[1].Value
}

function Get-M0967Run {
    $path = Wait-M0967File (Join-Path $script:SharedDirectory 'run.json') 1800 'run manifest'
    return (Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)
}

function Get-M0967PrivateRoot {
    param([Parameter(Mandatory)] [string] $Role, [Parameter(Mandatory)] [string] $RunId)
    return Join-Path $env:LOCALAPPDATA "Kilogram\M0967\$RunId\$Role"
}

function Import-M0967Providers {
    param([Parameter(Mandatory)] [string] $Role, [Parameter(Mandatory)] [string] $IpcFile)
    $outputPath = Join-Path $script:EvidenceDirectory "02-$Role-providers.log"
    if (Test-Path -LiteralPath $outputPath) { throw "Provider import evidence exists: $outputPath" }
    $lines = [Collections.Generic.List[string]]::new()
    $lines.Add("field_role=$Role")
    foreach ($provider in @('provider1', 'provider2')) {
        $offer = Wait-M0967File (Join-Path $script:EvidenceDirectory "01-$provider.offer") 900 "$provider offer"
        $result = @(Invoke-M0967Cli @('runtime-ipc-volunteer-provider-import', '--ipc-file', $IpcFile, '--offer-file', $offer))
        $lines.Add("field_provider=$provider")
        foreach ($line in $result) { $lines.Add([string]$line) }
    }
    $selection = @(Invoke-M0967Cli @('runtime-ipc-volunteer-provider-select', '--ipc-file', $IpcFile, '--count', '3'))
    foreach ($line in $selection) { $lines.Add([string]$line) }
    [IO.File]::WriteAllLines($outputPath, $lines, [Text.UTF8Encoding]::new($false))
}
