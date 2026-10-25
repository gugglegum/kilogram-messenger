[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidateSet('compile','run','clippy')][string]$Mode,
    [Parameter(Mandatory=$true)][string]$Name,
    [string]$Filter='a3_s3_',
    [switch]$UpdateVectors,
    [switch]$SkipLongCases,
    [switch]$WorkspaceTemp
)
$ErrorActionPreference='Stop'
foreach($suffix in @('.log','-source.json','-result.json')) {
    if(Test-Path -LiteralPath (Join-Path $PSScriptRoot "$Name$suffix")) {
        throw "Evidence name already exists: $Name. Choose a fresh name; prior runs are immutable."
    }
}
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$previousOffline=$env:CARGO_NET_OFFLINE
$env:CARGO_NET_OFFLINE='true'
$previousTemp=$env:TEMP
$previousTmp=$env:TMP
if ($WorkspaceTemp) {
    $testTemp=Join-Path $repo '.tmp/a3-s3/test-temp'
    New-Item -ItemType Directory -Force -Path $testTemp | Out-Null
    $env:TEMP=$testTemp
    $env:TMP=$testTemp
}
Push-Location $repo
try {
    $paths = @(& git ls-files --modified) + @(& git ls-files --others --exclude-standard)
    $rows = @($paths | Sort-Object -Unique | Where-Object { $_ -match '\.rs$|s3-differential\.json$' } | ForEach-Object {
        $item=Get-Item -LiteralPath $_
        [ordered]@{path=$_;bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    $rows | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $PSScriptRoot "$Name-source.json") -Encoding utf8
    $log=Join-Path $PSScriptRoot "$Name.log"
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $code=0
    try {
        if ($Mode -eq 'run') {
            if ($UpdateVectors) {$env:KILOGRAM_S3_UPDATE_VECTORS='1'}
            $skip=@()
            if ($SkipLongCases) {$skip=@('ordered_dialog::protocol2_component_tests::a3_s3_n01_real_native_indexed_long_chain','ordered_dialog::protocol2_component_tests::a3_s3_n02_real_native_indexed_duplex_late_tails')}
            & ./scripts/run-cargo-tests-stable.ps1 -Package @('kilogram-protocol','kilogram-ratchet','kilogram-state','kilogram-cli') -Filter $Filter -Profile verification -CargoJobs 12 -Run -NoCapture -SkipTest $skip *> $log
            $code=$LASTEXITCODE
        } else {
            $args2=@('--locked','--offline','--profile','verification','-p','kilogram-protocol','-p','kilogram-ratchet','-p','kilogram-state','-p','kilogram-cli')
            if ($Mode -eq 'compile') {
                $args2+=@('--no-run')
                & ./scripts/invoke-cargo-friendly.ps1 test -CargoJobs 12 -CargoArguments $args2 *> $log
            } else {
                $args2+=@('--all-targets','--','-D','warnings')
                & ./scripts/invoke-cargo-friendly.ps1 clippy -CargoJobs 12 -CargoArguments $args2 *> $log
            }
            $code=$LASTEXITCODE
        }
    } catch {
        $_ | Out-String | Add-Content -LiteralPath $log
        $code=1
    } finally {
        if ($UpdateVectors) {Remove-Item Env:KILOGRAM_S3_UPDATE_VECTORS -ErrorAction SilentlyContinue}
    }
    $watch.Stop()
    $exes=@(Get-Content -LiteralPath $log | ForEach-Object {
        if ($_ -match '^stable_test_executable=(.+)$') {$Matches[1]}
    } | Sort-Object -Unique | ForEach-Object {
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{exit_code=$code;seconds=$watch.Elapsed.TotalSeconds;mode=$Mode;filter=$Filter;skip_long_cases=[bool]$SkipLongCases;workspace_temp=[bool]$WorkspaceTemp;offline=$true;profile='verification';jobs=12;priority='BelowNormal';executables=$exes} |
        ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $PSScriptRoot "$Name-result.json") -Encoding utf8
    Get-Content -LiteralPath $log | Select-Object -Last 28
    "result_exit=$code seconds=$($watch.Elapsed.TotalSeconds)"
} finally {$env:CARGO_NET_OFFLINE=$previousOffline;$env:TEMP=$previousTemp;$env:TMP=$previousTmp;Pop-Location}
exit $code
