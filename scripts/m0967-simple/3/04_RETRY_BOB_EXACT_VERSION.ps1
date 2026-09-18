$root = Split-Path -Parent $PSScriptRoot
$common = Join-Path $root 'common.ps1'
$receive = Join-Path $PSScriptRoot '02_RECEIVE_BOB.ps1'
$expectedCommonHash = 'EB30E6E7EF50DA83BBEB5C62F90B51C64EBE810916F8CCABAF994F8BA4F4ABE7'
$expectedReceiveHash = '71720A81BC1D84F5DCDDFB8081F5872152A160C0BF679E37D2F06A2CFC853AC4'
$deadline = [DateTime]::UtcNow.AddMinutes(10)
$actualCommonHash = 'missing'
$actualReceiveHash = 'missing'

Write-Host 'Waiting for exact, mutually compatible Bob retry files from Yandex Disk...'
while ([DateTime]::UtcNow -lt $deadline) {
    if (Test-Path -LiteralPath $common -PathType Leaf) {
        $actualCommonHash = (Get-FileHash -LiteralPath $common -Algorithm SHA256).Hash
    }
    if (Test-Path -LiteralPath $receive -PathType Leaf) {
        $actualReceiveHash = (Get-FileHash -LiteralPath $receive -Algorithm SHA256).Hash
    }
    if ($actualCommonHash -ceq $expectedCommonHash -and
        $actualReceiveHash -ceq $expectedReceiveHash) {
        Write-Host 'EXACT BOB RETRY FILES ARE SYNCHRONIZED. Starting the safe receive retry.'
        & $receive
        return
    }
    Start-Sleep -Seconds 2
}
throw "Timed out waiting for exact Bob retry files. common=$actualCommonHash receive=$actualReceiveHash"
