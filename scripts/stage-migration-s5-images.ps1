[CmdletBinding()]
param([string]$EvidenceName='static-field-images-01')
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-storage-evidence'
$recordPath=Join-Path $evidence "$EvidenceName.json"
$logPath=Join-Path $evidence "$EvidenceName.log"
if((Test-Path -LiteralPath $recordPath) -or (Test-Path -LiteralPath $logPath)){throw 'Every static build/stage requires a new evidence name.'}
function Get-BuildSourceSnapshot {
    $paths=@(& git -C $workspace ls-files --cached --others --exclude-standard -- '*.rs' 'Cargo.toml' '**/Cargo.toml' 'Cargo.lock' '.cargo/*' 'rust-toolchain*' '*.c' '*.h' '*.cpp' 'scripts/invoke-cargo-friendly.ps1' 'scripts/cargo-resource-policy.ps1' 'scripts/stage-migration-s5-images.ps1')
    @($paths|Sort-Object -Unique|ForEach-Object {
        $full=Join-Path $workspace $_
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}
$baseline=(& git -C $workspace rev-parse HEAD)
$sourcesBefore=@(Get-BuildSourceSnapshot)
$buildArguments=@('--locked','--offline','--profile','verification','-p','kilogram-cli','--bin','kilogram-cli')
$directory=Join-Path $workspace 'target/compatibility/long-lived-dialog-migration'
$fixed=@((Join-Path $directory 'kilogram-cli.exe'),(Join-Path $directory 'kilogram-l1-control-peer.exe'))
$live=@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $fixed -contains $_.ExecutablePath} | Select-Object ProcessId,ExecutablePath)
if($live.Count){throw 'A fixed migration/control-peer executable is still owned by a live child.'}
$frozen=@('target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe','target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe','target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe','target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe')
$before=@($frozen|ForEach-Object {[ordered]@{path=$_;bytes=(Get-Item -LiteralPath (Join-Path $workspace $_)).Length;sha256=(Get-FileHash -LiteralPath (Join-Path $workspace $_) -Algorithm SHA256).Hash.ToLowerInvariant()}})
$clock=[Diagnostics.Stopwatch]::StartNew()
$previousOffline=$env:CARGO_NET_OFFLINE
try{
    $env:CARGO_NET_OFFLINE='true'
    & (Join-Path $PSScriptRoot 'invoke-cargo-friendly.ps1') -CargoCommand build -CargoJobs 12 -CargoArguments $buildArguments *> $logPath
    if($LASTEXITCODE -ne 0){throw 'Migration field CLI build failed.'}
    [void](New-Item -ItemType Directory -Path $directory -Force)
    foreach($path in $fixed){Copy-Item -LiteralPath (Join-Path $workspace 'target/verification/kilogram-cli.exe') -Destination $path -Force}
}finally{$env:CARGO_NET_OFFLINE=$previousOffline}
$clock.Stop()
foreach($entry in $before){if((Get-FileHash -LiteralPath (Join-Path $workspace $entry.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256){throw 'Frozen image changed.'}}
$sourcesAfter=@(Get-BuildSourceSnapshot)
if(($sourcesBefore|ConvertTo-Json -Compress) -cne ($sourcesAfter|ConvertTo-Json -Compress)){throw 'Build source set/content changed during build/stage.'}
if((& git -C $workspace rev-parse HEAD) -ne $baseline){throw 'Baseline changed during build/stage.'}
$built=Join-Path $workspace 'target/verification/kilogram-cli.exe'
$builtHash=(Get-FileHash -LiteralPath $built -Algorithm SHA256).Hash.ToLowerInvariant()
$fixedImages=@($fixed|ForEach-Object{[ordered]@{path=$_;bytes=(Get-Item -LiteralPath $_).Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})
foreach($entry in $fixedImages){if($entry.sha256 -ne $builtHash){throw 'Copied fixed image does not match the just-built binary.'}}
$record=[ordered]@{baseline=$baseline;scope='static build/stage only; no case or fixture execution';effective_cargo_argv=(@('build','--jobs','12')+$buildArguments);profile='verification';priority='BelowNormal';cargo_net_offline='true';rustflags=$env:RUSTFLAGS;cargo_encoded_rustflags=$env:CARGO_ENCODED_RUSTFLAGS;build_log="$EvidenceName.log";elapsed_seconds=$clock.Elapsed.TotalSeconds;pre_stage_live=$live;build_source_scope='All non-ignored workspace Rust/C/C++ source and Cargo/config/toolchain inputs plus resource/build/stage wrappers; dependencies resolved by locked Cargo.lock';sources_before=$sourcesBefore;sources_after=$sourcesAfter;sources_equal=$true;built_image=[ordered]@{path=$built;bytes=(Get-Item -LiteralPath $built).Length;sha256=$builtHash};fixed_images=$fixedImages;frozen_images=$before;control_peer='Current public ALPN1 actor holds actual controls on the remote side using its repaired writable debug marker. No scheduler/pause flag is passed to frozen d9b.'}
$record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $recordPath -Encoding utf8
Get-Content -LiteralPath $logPath -Tail 12
[ordered]@{baseline=$baseline;sources_equal=$true;source_count=$sourcesBefore.Count;fixed_images=$fixedImages}|ConvertTo-Json -Depth 5
