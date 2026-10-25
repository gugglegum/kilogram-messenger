[CmdletBinding()]
param([string]$Name = 'native-schedules-1')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$log = Join-Path $PSScriptRoot "$Name.log"
$result = Join-Path $PSScriptRoot "$Name-result.json"
if ((Test-Path -LiteralPath $log) -or (Test-Path -LiteralPath $result)) {
    throw 'Choose a new evidence name; previous runs are immutable.'
}
$oldOffline = $env:CARGO_NET_OFFLINE
$env:CARGO_NET_OFFLINE = 'true'
Push-Location $repo
try {
    $sources = @('Cargo.toml','Cargo.lock','crates/kilogram-ratchet/Cargo.toml',
        'crates/kilogram-ratchet/src/lib.rs','crates/kilogram-ratchet/src/ordered.rs',
        'crates/kilogram-ratchet/src/ordered_v2.rs',
        'crates/kilogram-ratchet/tests/a3_s3_root_review.rs',
        'scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1')
    $hashes = @($sources | ForEach-Object {
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;
            sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $code = 0
    try {
        & ./scripts/run-cargo-tests-stable.ps1 -Package kilogram-ratchet `
            -Filter a3_s3_root_native_delayed_delivery_schedules -Profile verification `
            -CargoJobs 12 -Run -NoCapture *> $log
        $code = $LASTEXITCODE
    } catch {
        $_ | Out-String | Add-Content -LiteralPath $log
        $code = 1
    }
    $watch.Stop()
    $executables = @(Get-Content -LiteralPath $log | ForEach-Object {
        if ($_ -match '^stable_test_executable=(.+)$') {$Matches[1]}
    } | Sort-Object -Unique | ForEach-Object {
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;
            sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{exit_code=$code;seconds=$watch.Elapsed.TotalSeconds;
        profile='verification';offline=$true;jobs=12;priority='BelowNormal';
        filter='a3_s3_root_native_delayed_delivery_schedules';sources=$hashes;
        executables=$executables} | ConvertTo-Json -Depth 6 |
        Set-Content -LiteralPath $result -Encoding utf8
    Get-Content -LiteralPath $log | Select-Object -Last 22
    "root_probe_exit=$code elapsed_seconds=$($watch.Elapsed.TotalSeconds)"
} finally {
    Pop-Location
    $env:CARGO_NET_OFFLINE = $oldOffline
}
exit $code
