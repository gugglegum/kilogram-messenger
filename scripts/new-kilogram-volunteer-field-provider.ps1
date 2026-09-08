[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('provider1', 'provider2')] [string] $ProviderName,
    [Parameter(Mandatory)] [string] $PrivateDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-KilogramCli {
    param([Parameter(Mandatory)] [string[]] $Arguments)
    $lines = @(& $CliPath @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    foreach ($line in $lines) { Write-Output $line }
    if ($exitCode -ne 0) {
        throw "kilogram-cli failed with exit code ${exitCode}: $($Arguments -join ' ')"
    }
}

if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
    throw "kilogram-cli is missing: $CliPath"
}
$privateRoot = [IO.Path]::GetFullPath($PrivateDirectory)
if (Test-Path -LiteralPath $privateRoot) {
    throw "provider private directory already exists and will not be overwritten: $privateRoot"
}
New-Item -ItemType Directory -Path $privateRoot | Out-Null

$accountRoot = Join-Path $privateRoot 'account-root'
$stateDirectory = Join-Path $privateRoot 'state'
$publicDirectory = Join-Path $privateRoot 'public'
$certificateFile = Join-Path $publicDirectory 'device.cert'
$deviceListFile = Join-Path $publicDirectory 'device-list.kadl'
$profileFile = Join-Path $privateRoot 'runtime-profile.json'
$ipcFile = Join-Path $privateRoot 'runtime.ipc.json'
$ticketFile = Join-Path $publicDirectory 'runtime.ticket'
$storageDirectory = Join-Path $privateRoot 'volunteer-storage'
$configFile = Join-Path $privateRoot "$ProviderName.local.psd1"
New-Item -ItemType Directory -Path $publicDirectory | Out-Null

$accountOutput = @(Invoke-KilogramCli @('account-create', '--account-dir', $accountRoot))
$accountMatches = @($accountOutput | Select-String -Pattern '^account_id=([0-9a-f]{64})$')
if ($accountMatches.Count -ne 1) {
    throw 'provider bootstrap did not produce exactly one Account ID'
}
$accountId = $accountMatches[0].Matches[0].Groups[1].Value

$null = Invoke-KilogramCli @(
    'device-enroll', '--account-dir', $accountRoot, '--state-dir', $stateDirectory,
    '--certificate-file', $certificateFile
)
$null = Invoke-KilogramCli @(
    'account-device-list', '--account-dir', $accountRoot,
    '--device-certificate-file', $certificateFile, '--device-list-file', $deviceListFile
)
$null = Invoke-KilogramCli @(
    'runtime-profile-create', '--profile-file', $profileFile, '--state-dir', $stateDirectory,
    '--allow-account', $accountId, '--device-list-file', $deviceListFile,
    '--ticket-file', $ticketFile, '--ipc-file', $ipcFile, '--route-policy', 'auto',
    '--relay-wait-seconds', '30', '--volunteer-storage-data-dir', $storageDirectory
)

$configLines = @(
    '@{',
    "    Role = '$ProviderName'",
    "    ProfileFile = '$($profileFile.Replace("'", "''"))'",
    "    IpcFile = '$($ipcFile.Replace("'", "''"))'",
    "    StateDirectory = '$($stateDirectory.Replace("'", "''"))'",
    '}'
)
[IO.File]::WriteAllLines($configFile, $configLines, [Text.UTF8Encoding]::new($false))

Write-Output "provider_name=$ProviderName"
Write-Output "provider_account_id=$accountId"
Write-Output "provider_profile_file=$profileFile"
Write-Output "provider_config_file=$configFile"
Write-Output 'provider_secrets_copied_to_shared_directory=false'
Write-Output 'status=volunteer-field-provider-ready'
