$root = Split-Path -Parent $PSScriptRoot
$common = Join-Path $root 'common.ps1'
$receive = Join-Path $PSScriptRoot '02_RECEIVE_BOB.ps1'
$deadline = [DateTime]::UtcNow.AddMinutes(10)

Write-Host 'Waiting for the complete Bob retry fix to arrive through Yandex Disk...'
while ([DateTime]::UtcNow -lt $deadline) {
    $commonReady = (Test-Path -LiteralPath $common -PathType Leaf) -and
        (Get-Content -LiteralPath $common -Raw -ErrorAction SilentlyContinue).Contains(
            'Wait-M0967ProviderOfferFile'
        )
    $receiveReady = (Test-Path -LiteralPath $receive -PathType Leaf) -and
        (Get-Content -LiteralPath $receive -Raw -ErrorAction SilentlyContinue).Contains(
            'RETRYING BOB BEFORE THE FIRST INBOUND COMMIT'
        )
    if ($commonReady -and $receiveReady) {
        Write-Host 'BOB RETRY FIX IS SYNCHRONIZED. Starting the safe receive retry.'
        & $receive
        return
    }
    Start-Sleep -Seconds 2
}
throw 'Timed out waiting for the complete Bob retry fix from Yandex Disk.'
