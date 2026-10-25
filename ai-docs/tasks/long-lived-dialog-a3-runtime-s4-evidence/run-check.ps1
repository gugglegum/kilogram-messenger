[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidateSet('compile','run','clippy','build')][string]$Mode,
    [Parameter(Mandatory=$true)][string]$Name,
    [string[]]$Package=@('kilogram-state'),
    [string]$Filter='a3_s4_',
    [string[]]$SkipTest=@(),
    [string[]]$CargoExtra=@()
)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
foreach($suffix in @('.log','-source.json','-result.json')) {
    if(Test-Path -LiteralPath (Join-Path $PSScriptRoot "$Name$suffix")) {throw "Evidence already exists: $Name"}
}
$previousOffline=$env:CARGO_NET_OFFLINE
$previousTemp=$env:TEMP
$previousTmp=$env:TMP
$env:CARGO_NET_OFFLINE='true'
$testTemp=Join-Path $repo '.tmp/a3-s4/test-temp'
New-Item -ItemType Directory -Path $testTemp -Force | Out-Null
$env:TEMP=$testTemp
$env:TMP=$testTemp
Push-Location $repo
try {
    $paths=@(& git ls-files --modified)+@(& git ls-files --others --exclude-standard)
    $sources=@($paths | Sort-Object -Unique | Where-Object {$_ -match '\.rs$|\.ps1$|(^|/)Cargo\.(toml|lock)$|tests/data/.*\.json$'} | ForEach-Object {
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    $sources | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $PSScriptRoot "$Name-source.json") -Encoding utf8
    $log=Join-Path $PSScriptRoot "$Name.log"
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $code=0
    try {
        if($Mode -eq 'run') {
            & ./scripts/run-cargo-tests-stable.ps1 -Package $Package -Filter $Filter -Profile verification -CargoJobs 12 -Run -NoCapture -SkipTest $SkipTest *> $log
        } else {
            $args2=@('--locked','--offline','--profile','verification')
            foreach($item in $Package){$args2+=@('-p',$item)}
            $verb=$Mode
            if($Mode -eq 'compile'){$verb='test';$args2+='--no-run'}
            $args2+=$CargoExtra
            if($Mode -eq 'clippy'){$args2+=@('--all-targets','--','-D','warnings')}
            & ./scripts/invoke-cargo-friendly.ps1 $verb -CargoJobs 12 -CargoArguments $args2 *> $log
        }
        $code=$LASTEXITCODE
    } catch {$_ | Out-String | Add-Content -LiteralPath $log;$code=1}
    $watch.Stop()
    $exes=@(Get-Content -LiteralPath $log | ForEach-Object {if($_ -match '^stable_test_executable=(.+)$'){$Matches[1]}} | Sort-Object -Unique | ForEach-Object {
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{mode=$Mode;packages=$Package;filter=$Filter;skip=$SkipTest;exit_code=$code;seconds=$watch.Elapsed.TotalSeconds;profile='verification';offline=$true;jobs=12;priority='BelowNormal';executables=$exes} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $PSScriptRoot "$Name-result.json") -Encoding utf8
    Get-Content -LiteralPath $log | Select-Object -Last 30
    "result_exit=$code seconds=$($watch.Elapsed.TotalSeconds)"
} finally {$env:CARGO_NET_OFFLINE=$previousOffline;$env:TEMP=$previousTemp;$env:TMP=$previousTmp;Pop-Location}
exit $code
