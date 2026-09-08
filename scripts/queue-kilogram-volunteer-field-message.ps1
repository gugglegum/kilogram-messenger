[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $IpcFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

foreach ($path in @($CliPath, $IpcFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required volunteer message input is missing: $path"
    }
}
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
$manifestPath = Join-Path $evidence 'manifest.json'
$outputPath = Join-Path $evidence '03-alice-queue.log'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "field manifest is missing: $manifestPath"
}
if (Test-Path -LiteralPath $outputPath) {
    throw "message queue evidence already exists and will not be overwritten: $outputPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$output = @(& $CliPath runtime-ipc-queue-message --ipc-file $IpcFile `
    --conversation ([string]$manifest.conversation_label) `
    --peer-account ([string]$manifest.bob_account_id) `
    --message ([string]$manifest.message_marker) 2>&1)
$exitCode = $LASTEXITCODE
[IO.File]::WriteAllLines($outputPath, [string[]]$output, [Text.UTF8Encoding]::new($false))
foreach ($line in $output) { Write-Output $line }
if ($exitCode -ne 0) {
    throw "queue volunteer field message failed with exit code $exitCode"
}
Write-Output "message_queue_evidence=$outputPath"
