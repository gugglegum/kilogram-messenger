[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('alice', 'bob', 'provider1', 'provider2')] [string] $Role,
    [Parameter(Mandatory)] [ValidateSet('01', '02-prepare', '03-send', '04-receive', '05-restart')] [string] $Phase,
    [Parameter(Mandatory)] [string] $ProfileFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

foreach ($path in @($CliPath, $ProfileFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required volunteer field-test input is missing: $path"
    }
}
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$evidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$logPath = Join-Path $evidence "$Phase-$Role.log"
if (Test-Path -LiteralPath $logPath) {
    throw "volunteer field evidence already exists and will not be overwritten: $logPath"
}

& $CliPath runtime-from-profile --profile-file $ProfileFile 2>&1 |
    Tee-Object -LiteralPath $logPath
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
    throw "Kilogram runtime exited with code $exitCode; evidence remains at $logPath"
}
Write-Output "volunteer_field_runtime_log=$logPath"
