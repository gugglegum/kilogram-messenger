[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('alice', 'bob')] [string] $Role,
    [Parameter(Mandatory)] [ValidatePattern('^[0-9]{2}-[a-z0-9-]+$')] [string] $Phase,
    [Parameter(Mandatory)] [string] $ProfileFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe'),
    [switch] $DropMailboxCapabilityAckAfterApplyOnce
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($DropMailboxCapabilityAckAfterApplyOnce -and $Role -ne 'bob') {
    throw 'the controlled lost-ACK hook must be armed only on the recipient Bob runtime'
}
foreach ($path in @($CliPath, $ProfileFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "required field-test input is missing: $path"
    }
}
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$resolvedEvidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$logPath = Join-Path $resolvedEvidence "$Phase-$Role.log"
if (Test-Path -LiteralPath $logPath) {
    throw "field evidence already exists and will not be overwritten: $logPath"
}

$faultName = 'KILOGRAM_TEST_DROP_MAILBOX_CAPABILITY_ACK_ONCE'
$previousFault = [Environment]::GetEnvironmentVariable($faultName, 'Process')
try {
    if ($DropMailboxCapabilityAckAfterApplyOnce) {
        [Environment]::SetEnvironmentVariable($faultName, '1', 'Process')
    }
    else {
        [Environment]::SetEnvironmentVariable($faultName, $null, 'Process')
    }

    & $CliPath runtime-from-profile --profile-file $ProfileFile 2>&1 |
        Tee-Object -LiteralPath $logPath
    $exitCode = $LASTEXITCODE
}
finally {
    [Environment]::SetEnvironmentVariable($faultName, $previousFault, 'Process')
}

if ($exitCode -ne 0) {
    throw "Kilogram runtime exited with code $exitCode; evidence remains at $logPath"
}
Write-Output "field_runtime_log=$logPath"

