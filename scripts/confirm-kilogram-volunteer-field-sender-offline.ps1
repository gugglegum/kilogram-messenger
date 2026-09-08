[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $IpcFile,
    [Parameter(Mandatory)] [string] $EvidenceDirectory,
    [string] $CliPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\kilogram-cli.exe')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $CliPath -PathType Leaf)) {
    throw "kilogram-cli is missing: $CliPath"
}
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
$sendLogPath = Join-Path $evidence '03-send-alice.log'
$outputPath = Join-Path $evidence '03-alice-offline.boundary'
if (-not (Test-Path -LiteralPath $sendLogPath -PathType Leaf)) {
    throw "Alice send runtime log is missing: $sendLogPath"
}
if (Test-Path -LiteralPath $outputPath) {
    throw "sender-offline evidence already exists and will not be overwritten: $outputPath"
}
$sendText = Get-Content -LiteralPath $sendLogPath -Raw
if (-not [regex]::IsMatch($sendText, '(?m)^runtime_mailbox_replication_receipts=2/2$') -or
    -not [regex]::IsMatch($sendText, '(?m)^runtime_mailbox_replication_status=satisfied$')) {
    throw 'Alice runtime has not retained the required two volunteer receipts'
}
$probe = @(& $CliPath runtime-ipc-ping --ipc-file $IpcFile 2>&1)
$probeExitCode = $LASTEXITCODE
if ($probeExitCode -eq 0) {
    throw 'Alice runtime is still reachable; stop it before recording the offline boundary'
}
$receiptKeys = @([regex]::Matches(
    $sendText,
    '(?m)^runtime_mailbox_replication_receipt_store_key=([0-9a-f]{64})$'
) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
if ($receiptKeys.Count -lt 2) {
    throw 'Alice send log does not contain two distinct volunteer receipt store keys'
}
$lines = @(
    'alice_replication_status=satisfied',
    "alice_replica_receipt_store_keys=$($receiptKeys -join ',')",
    'alice_runtime_ipc_reachable=false',
    "sender_stop_observed_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
)
[IO.File]::WriteAllLines($outputPath, $lines, [Text.UTF8Encoding]::new($false))
foreach ($line in $lines) { Write-Output $line }
Write-Output "sender_offline_evidence=$outputPath"
