[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet(
        '01-alice-pending.status',
        '02-alice-active.status',
        '02-bob-active.status',
        '03-alice-rotation-pending.status',
        '04-alice-rotated.status',
        '04-bob-rotated.status',
        '05-alice-mailbox.status',
        '06-alice-revocation-pending.status',
        '07-alice-revoked.status',
        '07-bob-revoked.status'
    )] [string] $EvidenceFileName,
    [Parameter(Mandatory)] [string] $IpcFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

foreach ($path in @($CliPath, $IpcFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required field-test input is missing: $path"
    }
}
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$resolvedEvidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$outputPath = Join-Path $resolvedEvidence $EvidenceFileName
if (Test-Path -LiteralPath $outputPath) {
    throw "field evidence already exists and will not be overwritten: $outputPath"
}

& $CliPath runtime-ipc-mailbox-status --ipc-file $IpcFile 2>&1 |
    Tee-Object -LiteralPath $outputPath
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
    throw "mailbox status capture exited with code $exitCode; evidence remains at $outputPath"
}
Write-Output "field_mailbox_status=$outputPath"
