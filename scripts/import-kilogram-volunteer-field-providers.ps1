[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('alice', 'bob')] [string] $Role,
    [Parameter(Mandatory)] [string] $IpcFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

foreach ($path in @($CliPath, $IpcFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required volunteer provider import input is missing: $path"
    }
}
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
$outputPath = Join-Path $evidence "02-$Role-providers.log"
if (Test-Path -LiteralPath $outputPath) {
    throw "provider import evidence already exists and will not be overwritten: $outputPath"
}
$lines = [Collections.Generic.List[string]]::new()
$lines.Add("field_role=$Role")
foreach ($provider in @('provider1', 'provider2')) {
    $offerPath = Join-Path $evidence "01-$provider.offer"
    if (-not (Test-Path -LiteralPath $offerPath -PathType Leaf)) {
        throw "provider offer is missing: $offerPath"
    }
    $commandOutput = @(& $CliPath runtime-ipc-volunteer-provider-import `
        --ipc-file $IpcFile --offer-file $offerPath 2>&1)
    $exitCode = $LASTEXITCODE
    $lines.Add("field_provider=$provider")
    foreach ($line in $commandOutput) { $lines.Add([string]$line) }
    if ($exitCode -ne 0) {
        throw "provider import failed for $provider with exit code $exitCode"
    }
}
$selectionOutput = @(& $CliPath runtime-ipc-volunteer-provider-select `
    --ipc-file $IpcFile --count 3 2>&1)
$selectionExitCode = $LASTEXITCODE
foreach ($line in $selectionOutput) { $lines.Add([string]$line) }
if ($selectionExitCode -ne 0) {
    throw "provider selection check failed with exit code $selectionExitCode"
}
[IO.File]::WriteAllLines($outputPath, $lines, [Text.UTF8Encoding]::new($false))
foreach ($line in $lines) { Write-Output $line }
Write-Output "provider_import_evidence=$outputPath"
Write-Output 'status=volunteer-field-providers-imported'
