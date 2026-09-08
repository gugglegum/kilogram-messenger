[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $StateDirectory,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
    throw "kilogram-cli is missing: $CliPath"
}
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
$manifestPath = Join-Path $evidence 'manifest.json'
$outputPath = Join-Path $evidence '05-bob-history.log'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "field manifest is missing: $manifestPath"
}
if (Test-Path -LiteralPath $outputPath) {
    throw "history evidence already exists and will not be overwritten: $outputPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$output = @(& $CliPath history --state-dir $StateDirectory `
    --conversation ([string]$manifest.conversation_label) 2>&1)
$exitCode = $LASTEXITCODE
[IO.File]::WriteAllLines($outputPath, [string[]]$output, [Text.UTF8Encoding]::new($false))
foreach ($line in $output) { Write-Output $line }
if ($exitCode -ne 0) {
    throw "history capture failed with exit code $exitCode; stop Bob runtime first"
}
Write-Output "history_evidence=$outputPath"
