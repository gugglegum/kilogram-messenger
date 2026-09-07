[CmdletBinding(DefaultParameterSetName = 'Run')]
param(
    [Parameter(Mandatory, ParameterSetName = 'Run')]
    [ValidateSet(
        'store-preflight',
        'alice-01-start',
        'bob-01-start-lost-ack',
        'alice-01-capture-pending',
        'bob-02-start',
        'alice-02-start',
        'alice-02-capture-active',
        'bob-02-capture-active',
        'alice-03-capture-rotation-pending',
        'bob-04-start',
        'alice-04-start',
        'alice-04-capture-rotated',
        'bob-04-capture-rotated',
        'bob-05-start-send',
        'alice-05-start-receive',
        'alice-05-capture-mailbox',
        'alice-06-capture-revocation-pending',
        'bob-07-start',
        'alice-07-start',
        'alice-07-capture-revoked',
        'bob-07-capture-revoked',
        'copy-boundaries',
        'verify'
    )]
    [string] $Step,
    [Parameter(Mandatory, ParameterSetName = 'Run')] [string] $ConfigFile,
    [Parameter(Mandatory, ParameterSetName = 'Run')] [string] $EvidenceDirectory,
    [Parameter(Mandatory, ParameterSetName = 'List')] [switch] $ListSteps
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$orderedSteps = @(
    'store-preflight',
    'alice-01-start',
    'bob-01-start-lost-ack',
    'alice-01-capture-pending',
    'bob-02-start',
    'alice-02-start',
    'alice-02-capture-active',
    'bob-02-capture-active',
    'alice-03-capture-rotation-pending',
    'bob-04-start',
    'alice-04-start',
    'alice-04-capture-rotated',
    'bob-04-capture-rotated',
    'bob-05-start-send',
    'alice-05-start-receive',
    'alice-05-capture-mailbox',
    'alice-06-capture-revocation-pending',
    'bob-07-start',
    'alice-07-start',
    'alice-07-capture-revoked',
    'bob-07-capture-revoked',
    'copy-boundaries',
    'verify'
)
if ($ListSteps) {
    for ($index = 0; $index -lt $orderedSteps.Count; $index++) {
        Write-Output ("{0:D2}={1}" -f ($index + 1), $orderedSteps[$index])
    }
    exit 0
}

function Get-FullPath {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [string] $BaseDirectory
    )
    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    if ([string]::IsNullOrWhiteSpace($BaseDirectory)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path $BaseDirectory $Path))
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Root
    )
    $fullPath = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    $fullRoot = [IO.Path]::GetFullPath($Root).TrimEnd('\')
    return $fullPath.Equals($fullRoot, [StringComparison]::OrdinalIgnoreCase) -or
        $fullPath.StartsWith($fullRoot + '\', [StringComparison]::OrdinalIgnoreCase)
}

function Assert-PrivateLocalPath {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $KitRoot,
        [Parameter(Mandatory)] [string] $EvidenceRoot
    )
    if ((Test-PathWithin $Path $KitRoot) -or (Test-PathWithin $Path $EvidenceRoot)) {
        throw "$Name must remain in a private local directory, outside the shared kit and evidence directory."
    }
}

function Assert-ArtifactHash {
    param(
        [Parameter(Mandatory)] [object] $BuildInfo,
        [Parameter(Mandatory)] [string] $RelativeName,
        [Parameter(Mandatory)] [string] $KitRoot
    )
    $records = @($BuildInfo.artifacts | Where-Object { [string]$_.file -ceq $RelativeName })
    if ($records.Count -ne 1) {
        throw "BUILD-INFO.json must contain exactly one artifact record for $RelativeName"
    }
    $path = Join-Path $KitRoot ($RelativeName.Replace('/', '\'))
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "acceptance artifact is missing: $RelativeName"
    }
    $item = Get-Item -LiteralPath $path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "acceptance artifact must not be a reparse point: $RelativeName"
    }
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($records[0].sha256 -cne $hash -or [UInt64]$records[0].bytes -ne [UInt64]$item.Length) {
        throw "acceptance artifact does not match BUILD-INFO.json: $RelativeName"
    }
    return $path
}

$scriptsDirectory = [IO.Path]::GetFullPath($PSScriptRoot)
$kitRoot = [IO.Path]::GetFullPath((Split-Path -Parent $scriptsDirectory))
$buildInfoPath = Join-Path $kitRoot 'BUILD-INFO.json'
if (-not (Test-Path -LiteralPath $buildInfoPath -PathType Leaf)) {
    throw 'BUILD-INFO.json is missing from the acceptance kit root.'
}
$buildInfoItem = Get-Item -LiteralPath $buildInfoPath -Force
if (($buildInfoItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
    $buildInfoItem.Length -gt 65536) {
    throw 'BUILD-INFO.json must be a regular file no larger than 64 KiB.'
}
$buildInfo = Get-Content -LiteralPath $buildInfoPath -Raw | ConvertFrom-Json
if ($buildInfo.schema -ne 1 -or $buildInfo.profile -cne 'debug' -or
    $buildInfo.archive -ne $false -or $buildInfo.network_executed -ne $false -or
    [string]$buildInfo.source_revision -cnotmatch '^[0-9a-f]{40}$') {
    throw 'BUILD-INFO.json does not describe a clean, non-archived debug acceptance kit.'
}
$cliPath = Assert-ArtifactHash $buildInfo 'bin/kilogram-cli.exe' $kitRoot
$null = Assert-ArtifactHash $buildInfo 'bin/kilogram-windows.exe' $kitRoot

$configPath = Get-FullPath $ConfigFile
if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
    throw "local role config is missing: $configPath"
}
$configItem = Get-Item -LiteralPath $configPath -Force
if (($configItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $configItem.Length -gt 65536) {
    throw 'local role config must be a regular file no larger than 64 KiB.'
}
$evidencePath = Get-FullPath $EvidenceDirectory
New-Item -ItemType Directory -Path $evidencePath -Force | Out-Null
$evidencePath = (Resolve-Path -LiteralPath $evidencePath).Path
Assert-PrivateLocalPath 'ConfigFile' $configPath $kitRoot $evidencePath
$config = Import-PowerShellDataFile -LiteralPath $configPath
if ([string]$config.Role -notin @('alice', 'bob')) {
    throw "local role config Role must be exactly 'alice' or 'bob'."
}
$configBase = Split-Path -Parent $configPath

$profilePath = $null
if (-not [string]::IsNullOrWhiteSpace([string]$config.ProfileFile)) {
    $profilePath = Get-FullPath ([string]$config.ProfileFile) $configBase
    Assert-PrivateLocalPath 'ProfileFile' $profilePath $kitRoot $evidencePath
}
$ipcPath = $null
if (-not [string]::IsNullOrWhiteSpace([string]$config.IpcFile)) {
    $ipcPath = Get-FullPath ([string]$config.IpcFile) $configBase
    Assert-PrivateLocalPath 'IpcFile' $ipcPath $kitRoot $evidencePath
}
$storeLogPath = $null
if (-not [string]::IsNullOrWhiteSpace([string]$config.StoreStartupLog)) {
    $storeLogPath = Get-FullPath ([string]$config.StoreStartupLog) $configBase
}

$manifestPath = Join-Path $evidencePath 'manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw 'Create evidence\manifest.json from manifest.example.json before running an acceptance step.'
}
$manifestItem = Get-Item -LiteralPath $manifestPath -Force
if (($manifestItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
    $manifestItem.Length -gt 65536) {
    throw 'evidence manifest must be a regular file no larger than 64 KiB.'
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ([string]$manifest.build_commit -cne [string]$buildInfo.source_revision) {
    throw 'manifest build_commit must equal the exact source_revision in BUILD-INFO.json.'
}

$prerequisites = @{
    'bob-01-start-lost-ack' = @('01-lost-ack-alice.log')
    'alice-01-capture-pending' = @('01-lost-ack-alice.log', '01-lost-ack-bob.log')
    'bob-02-start' = @('01-alice-pending.status')
    'alice-02-start' = @('02-retry-bob.log')
    'alice-02-capture-active' = @('02-retry-alice.log', '02-retry-bob.log')
    'bob-02-capture-active' = @('02-retry-alice.log', '02-retry-bob.log')
    'alice-03-capture-rotation-pending' = @('02-alice-active.status', '02-bob-active.status')
    'bob-04-start' = @('03-alice-rotation-pending.status')
    'alice-04-start' = @('04-rotation-bob.log')
    'alice-04-capture-rotated' = @('04-rotation-alice.log', '04-rotation-bob.log')
    'bob-04-capture-rotated' = @('04-rotation-alice.log', '04-rotation-bob.log')
    'bob-05-start-send' = @('00-store-preflight.log', '05-store.log', '04-alice-rotated.status', '04-bob-rotated.status')
    'alice-05-start-receive' = @('05-mailbox-send-bob.log')
    'alice-05-capture-mailbox' = @('05-mailbox-receive-alice.log')
    'alice-06-capture-revocation-pending' = @('05-alice-mailbox.status')
    'bob-07-start' = @('06-alice-revocation-pending.status')
    'alice-07-start' = @('07-revocation-bob.log')
    'alice-07-capture-revoked' = @('07-revocation-alice.log', '07-revocation-bob.log')
    'bob-07-capture-revoked' = @('07-revocation-alice.log', '07-revocation-bob.log')
    'copy-boundaries' = @('07-alice-revoked.status', '07-bob-revoked.status')
}
if ($prerequisites.ContainsKey($Step)) {
    foreach ($name in $prerequisites[$Step]) {
        if (-not (Test-Path -LiteralPath (Join-Path $evidencePath $name) -PathType Leaf)) {
            throw "step $Step requires earlier evidence file: $name"
        }
    }
}

$rolePrefix = $Step.Split('-')[0]
if ($rolePrefix -in @('alice', 'bob') -and $config.Role -cne $rolePrefix) {
    throw "step $Step requires the $rolePrefix local config, not $($config.Role)."
}

$runtimeSteps = @{
    'alice-01-start' = @{ Role = 'alice'; Phase = '01-lost-ack'; Fault = $false }
    'bob-01-start-lost-ack' = @{ Role = 'bob'; Phase = '01-lost-ack'; Fault = $true }
    'bob-02-start' = @{ Role = 'bob'; Phase = '02-retry'; Fault = $false }
    'alice-02-start' = @{ Role = 'alice'; Phase = '02-retry'; Fault = $false }
    'bob-04-start' = @{ Role = 'bob'; Phase = '04-rotation'; Fault = $false }
    'alice-04-start' = @{ Role = 'alice'; Phase = '04-rotation'; Fault = $false }
    'bob-05-start-send' = @{ Role = 'bob'; Phase = '05-mailbox-send'; Fault = $false }
    'alice-05-start-receive' = @{ Role = 'alice'; Phase = '05-mailbox-receive'; Fault = $false }
    'bob-07-start' = @{ Role = 'bob'; Phase = '07-revocation'; Fault = $false }
    'alice-07-start' = @{ Role = 'alice'; Phase = '07-revocation'; Fault = $false }
}
$captureSteps = @{
    'alice-01-capture-pending' = '01-alice-pending.status'
    'alice-02-capture-active' = '02-alice-active.status'
    'bob-02-capture-active' = '02-bob-active.status'
    'alice-03-capture-rotation-pending' = '03-alice-rotation-pending.status'
    'alice-04-capture-rotated' = '04-alice-rotated.status'
    'bob-04-capture-rotated' = '04-bob-rotated.status'
    'alice-05-capture-mailbox' = '05-alice-mailbox.status'
    'alice-06-capture-revocation-pending' = '06-alice-revocation-pending.status'
    'alice-07-capture-revoked' = '07-alice-revoked.status'
    'bob-07-capture-revoked' = '07-bob-revoked.status'
}

if ($runtimeSteps.ContainsKey($Step)) {
    if ([string]::IsNullOrWhiteSpace($profilePath)) {
        throw 'ProfileFile is required for runtime steps.'
    }
    $entry = $runtimeSteps[$Step]
    $arguments = @(
        '-Role', [string]$entry.Role,
        '-Phase', [string]$entry.Phase,
        '-ProfileFile', $profilePath,
        '-EvidenceDirectory', $evidencePath,
        '-CliPath', $cliPath
    )
    if ($entry.Fault) {
        $arguments += '-DropMailboxCapabilityAckAfterApplyOnce'
    }
    & (Join-Path $scriptsDirectory 'invoke-kilogram-mailbox-field-runtime.ps1') @arguments
}
elseif ($captureSteps.ContainsKey($Step)) {
    if ([string]::IsNullOrWhiteSpace($ipcPath)) {
        throw 'IpcFile is required for status capture steps.'
    }
    & (Join-Path $scriptsDirectory 'capture-kilogram-mailbox-field-status.ps1') `
        -EvidenceFileName $captureSteps[$Step] `
        -IpcFile $ipcPath `
        -EvidenceDirectory $evidencePath `
        -CliPath $cliPath
}
elseif ($Step -ceq 'store-preflight') {
    if ($config.Role -cne 'bob') {
        throw 'store-preflight must run on Bob so it proves cross-machine HTTPS reachability.'
    }
    if ([string]::IsNullOrWhiteSpace($storeLogPath)) {
        throw 'StoreStartupLog is required for store-preflight.'
    }
    $preflightLog = Join-Path $evidencePath '00-store-preflight.log'
    $storeEvidence = Join-Path $evidencePath '05-store.log'
    foreach ($path in @($preflightLog, $storeEvidence)) {
        if (Test-Path -LiteralPath $path) {
            throw "acceptance evidence already exists and will not be overwritten: $path"
        }
    }
    & (Join-Path $scriptsDirectory 'test-kilogram-mailbox-store-preflight.ps1') `
        -ServiceUrl ([string]$manifest.mailbox_service_url) `
        -ExpectedStoreKey ([string]$manifest.mailbox_store_key) `
        -StoreStartupLog $storeLogPath 2>&1 | Tee-Object -LiteralPath $preflightLog
    Copy-Item -LiteralPath $storeLogPath -Destination $storeEvidence
    Write-Output "mailbox_store_evidence=$storeEvidence"
}
elseif ($Step -ceq 'copy-boundaries') {
    $source = Join-Path $kitRoot 'BOUNDARIES.log'
    $destination = Join-Path $evidencePath '08-boundaries.log'
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw 'BOUNDARIES.log is missing from the acceptance kit.'
    }
    if (Test-Path -LiteralPath $destination) {
        throw "acceptance evidence already exists and will not be overwritten: $destination"
    }
    Copy-Item -LiteralPath $source -Destination $destination
    Write-Output "boundary_evidence=$destination"
}
elseif ($Step -ceq 'verify') {
    & (Join-Path $scriptsDirectory 'verify-kilogram-mailbox-field-evidence.ps1') `
        -EvidenceDirectory $evidencePath
}
else {
    throw "unsupported acceptance step: $Step"
}

Write-Output "acceptance_step=$Step"
Write-Output 'status=completed'
