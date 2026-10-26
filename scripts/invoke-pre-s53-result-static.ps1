[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^S-rb-[a-z0-9-]+$')][string]$Case,
    [Parameter(Mandatory)][ValidateSet('Fmt','FmtCheck','Compile','Clippy')][string]$Action
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$baseline = '9016b578bed8b8b5e43dd0e2e94edc376804e4f0'
$evidence = Join-Path $workspace 'ai-docs/tasks/pre-s53-refactor-rb-evidence'
[void][IO.Directory]::CreateDirectory($evidence)
$recordPath = Join-Path $evidence "$Case.json"
$logPath = Join-Path $evidence "$Case.log"
if ((Test-Path -LiteralPath $recordPath) -or (Test-Path -LiteralPath $logPath)) { throw 'Static IDs are immutable.' }
if ((& git -C $workspace rev-parse HEAD) -cne $baseline) { throw 'R-B baseline changed.' }
function Inventory([string[]]$Paths) {
    @($Paths | Sort-Object -Unique | ForEach-Object {
        $full = Join-Path $workspace $_
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}
function Restore-Offline($Value) {
    if ($null -eq $Value) {
        if (Test-Path -LiteralPath Env:CARGO_NET_OFFLINE) { Remove-Item -LiteralPath Env:CARGO_NET_OFFLINE -ErrorAction Stop }
    } else { [Environment]::SetEnvironmentVariable('CARGO_NET_OFFLINE',[string]$Value,'Process') }
}
$sourcePaths = @(& git -C $workspace ls-files --cached --others --exclude-standard -- '*.rs' '*.ps1' '*.c' '*.h' '*.cpp' 'Cargo.toml' '**/Cargo.toml' 'Cargo.lock' '.cargo/*' 'rust-toolchain*')
$before = Inventory $sourcePaths
$harnesses = @('target/verification/deps/kilogram-cli-bin-kilogram-cli-tests.exe','target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe','target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe','target/verification/deps/kilogram-cli-test-runtime2_process-tests.exe')
$productPaths = @('target/compatibility/long-lived-dialog-migration/kilogram-cli.exe','target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe','target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe')
$productBefore = Inventory $productPaths
$archive = Join-Path $workspace '.tmp/pre-s53-result-baseline-harnesses'
$archiveManifest = Join-Path $evidence 'S-rb-baseline-harnesses.json'
if (-not (Test-Path -LiteralPath $archiveManifest)) {
    if (@(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'kilogram-*' }).Count) { throw 'Live Kilogram process prevents image preservation.' }
    $old = Inventory $harnesses
    foreach ($image in $old) {
        $destination = Join-Path $archive $image.path
        if (Test-Path -LiteralPath $destination) { throw 'Unreconciled image archive exists.' }
        [void][IO.Directory]::CreateDirectory((Split-Path -Parent $destination))
        Copy-Item -LiteralPath (Join-Path $workspace $image.path) -Destination $destination
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $image.sha256) { throw 'Image archive mismatch.' }
    }
    [ordered]@{baseline=$baseline;classification='raw non-executed harness preservation only; no fixture';archive=$archive;images=$old;unchanged_product_images=$productBefore} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $archiveManifest -Encoding utf8
}
$timer = [Diagnostics.Stopwatch]::StartNew()
$priorOffline = [Environment]::GetEnvironmentVariable('CARGO_NET_OFFLINE','Process')
$code = 0
$primaryError = $null
$finalErrors = [Collections.Generic.List[string]]::new()
$after = @()
$sourceComplete = $false
$productAfter = @()
$productComplete = $false
$staged = @()
$argv = @()
try {
    $env:CARGO_NET_OFFLINE = 'true'
    . (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
    $jobs = Set-KilogramCargoResourcePolicy -RequestedJobs 12
    if ($Action -eq 'Compile') {
        if (@(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'kilogram-*' }).Count) { throw 'Live Kilogram process prevents stable recopy.' }
        $argv = @('scripts/run-cargo-tests-stable.ps1','-Package','kilogram-cli','-CargoJobs','12','-Profile','verification')
        & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package kilogram-cli -CargoJobs 12 -Profile verification *> $logPath
        if ($LASTEXITCODE -ne 0) { throw 'Compile-only stable wrapper failed.' }
        $staged = Inventory $harnesses
    } else {
        if ($Action -in @('Fmt','FmtCheck')) {
            $argv = @('fmt','--all')
            if ($Action -eq 'FmtCheck') { $argv += @('--','--check') }
        } else { $argv = @('clippy','--locked','--offline','--jobs',[string]$jobs,'--profile','verification','--package','kilogram-cli','--all-targets','--','-D','warnings') }
        Push-Location -LiteralPath $workspace
        try {
            $oldErrors = $ErrorActionPreference
            try { $ErrorActionPreference='Continue'; & cargo @argv *> $logPath; $nativeCode=$LASTEXITCODE }
            finally { $ErrorActionPreference=$oldErrors }
        } finally { Pop-Location }
        if ($nativeCode -ne 0) { throw "Static $Action exit $nativeCode." }
    }
} catch {
    $code = 1
    $primaryError = $_.Exception.Message
    try { $_ | Out-String | Add-Content -LiteralPath $logPath }
    catch { $finalErrors.Add('Primary log write: '+$_.Exception.Message) }
} finally {
    try { Restore-Offline $priorOffline } catch { $finalErrors.Add('Environment restore: '+$_.Exception.Message) }
    try {
        $after = Inventory $sourcePaths
        $sourceComplete = $true
        if ($Action -ne 'Fmt' -and (ConvertTo-Json -InputObject $before -Depth 6 -Compress) -cne (ConvertTo-Json -InputObject $after -Depth 6 -Compress)) { $finalErrors.Add('Source changed during static verification.') }
    } catch { $finalErrors.Add('Final source inventory incomplete: '+$_.Exception.Message) }
    try {
        $productAfter = Inventory $productPaths
        $productComplete = $true
        if ((ConvertTo-Json -InputObject $productBefore -Compress) -cne (ConvertTo-Json -InputObject $productAfter -Compress)) { $finalErrors.Add('Product/frozen image changed.') }
    } catch { $finalErrors.Add('Final product inventory incomplete: '+$_.Exception.Message) }
    if ($finalErrors.Count -ne 0) { $code=1 }
    $timer.Stop()
    $record = [ordered]@{case=$Case;action=$Action;baseline=$baseline;classification='static-only; no executable test or fixture';native_cwd=$workspace;argv=$argv;priority='BelowNormal';jobs=12;offline=$true;exit_code=$code;primary_error=$primaryError;finalization_errors=$finalErrors.ToArray();elapsed_seconds=$timer.Elapsed.TotalSeconds;sources_before=$before;sources_after=$after;source_inventory_after_complete=$sourceComplete;staged_images=$staged;product_images_before=$productBefore;product_images_after=$productAfter;product_inventory_after_complete=$productComplete}
    try { $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $recordPath -Encoding utf8 }
    catch { $code=1; $record.exit_code=1; [Console]::Error.WriteLine((ConvertTo-Json -InputObject $record -Depth 8)) }
}
Get-Content -LiteralPath $logPath -Tail 16
[ordered]@{case=$Case;exit_code=$code;source_count=$before.Count;elapsed_seconds=$timer.Elapsed.TotalSeconds;staged_images=$staged}|ConvertTo-Json -Depth 5
exit $code
