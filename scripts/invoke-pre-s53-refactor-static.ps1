[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^S-ra-[a-z0-9-]+$')][string]$Case,
    [Parameter(Mandatory)][ValidateSet('Fmt','FmtCheck','Compile','Build','Clippy')][string]$Action
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$evidence = Join-Path $workspace 'ai-docs/tasks/pre-s53-refactor-evidence'
[void][IO.Directory]::CreateDirectory($evidence)
$recordPath = Join-Path $evidence "$Case.json"
$logPath = Join-Path $evidence "$Case.log"
if ((Test-Path -LiteralPath $recordPath) -or (Test-Path -LiteralPath $logPath)) { throw 'Static IDs are immutable.' }
if ((& git -C $workspace rev-parse HEAD) -cne 'eafd0f873eb31b64377e8e65e06bc84a911f6dc6') { throw 'R-A baseline changed.' }
function Inventory([string[]]$Paths) {
    @($Paths | Sort-Object -Unique | ForEach-Object {
        $full = Join-Path $workspace $_
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}
$sourcePaths = @(& git -C $workspace ls-files --cached --others --exclude-standard -- '*.rs' '*.ps1' '*.c' '*.h' '*.cpp' 'Cargo.toml' '**/Cargo.toml' 'Cargo.lock' '.cargo/*' 'rust-toolchain*')
$before = Inventory $sourcePaths
$fixed = @('target/compatibility/long-lived-dialog-migration/kilogram-cli.exe','target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe')
$harnesses = @('target/verification/deps/kilogram-cli-bin-kilogram-cli-tests.exe','target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe','target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe','target/verification/deps/kilogram-cli-test-runtime2_process-tests.exe')
$frozen = 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe'
$frozenBefore = Inventory @($frozen)
$archive = Join-Path $workspace '.tmp/pre-s53-refactor-baseline-images'
$archiveManifest = Join-Path $evidence 'S-ra-baseline-images.json'
if (-not (Test-Path -LiteralPath $archiveManifest)) {
    $old = Inventory @($fixed + $harnesses)
    foreach ($image in $old) {
        $destination = Join-Path $archive $image.path
        if (Test-Path -LiteralPath $destination) { throw 'Unreconciled image archive exists.' }
        [void][IO.Directory]::CreateDirectory((Split-Path -Parent $destination))
        Copy-Item -LiteralPath (Join-Path $workspace $image.path) -Destination $destination
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $image.sha256) { throw 'Image archive mismatch.' }
    }
    [ordered]@{baseline='eafd0f873eb31b64377e8e65e06bc84a911f6dc6';classification='static image preservation only; never executed';archive=$archive;images=$old;frozen=$frozenBefore} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $archiveManifest -Encoding utf8
}
$timer = [Diagnostics.Stopwatch]::StartNew()
$priorOffline = $env:CARGO_NET_OFFLINE
$code = 0
$primaryError = $null
$finalizationErrors = [Collections.Generic.List[string]]::new()
$after = @()
$frozenAfter = @()
$sourceInventoryComplete = $false
$frozenInventoryComplete = $false
$argv = @()
$stagedImages = @()
try {
    $env:CARGO_NET_OFFLINE = 'true'
    . (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
    $jobs = Set-KilogramCargoResourcePolicy -RequestedJobs 12
    if ($Action -eq 'Compile') {
        if (@(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'kilogram-*' }).Count) { throw 'Live Kilogram process prevents stable image recopy.' }
        $argv = @('scripts/run-cargo-tests-stable.ps1','-Package','kilogram-cli','-CargoJobs','12','-Profile','verification')
        & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package kilogram-cli -CargoJobs 12 -Profile verification *> $logPath
        $code = $LASTEXITCODE
        if ($code -ne 0) { throw 'Compile-only wrapper failed.' }
        $stagedImages = Inventory $harnesses
    } else {
        if ($Action -in @('Fmt','FmtCheck')) {
            $argv = @('fmt','--all')
            if ($Action -eq 'FmtCheck') { $argv += @('--','--check') }
        } else {
            $argv = @($Action.ToLowerInvariant(),'--locked','--offline','--jobs',[string]$jobs,'--profile','verification','--package','kilogram-cli')
            if ($Action -eq 'Build') { $argv += @('--bin','kilogram-cli') }
            if ($Action -eq 'Clippy') { $argv += @('--all-targets','--','-D','warnings') }
        }
        Push-Location -LiteralPath $workspace
        try {
            $oldErrors = $ErrorActionPreference
            try {
                $ErrorActionPreference = 'Continue'
                & cargo @argv *> $logPath
                $code = $LASTEXITCODE
            } finally { $ErrorActionPreference = $oldErrors }
        } finally { Pop-Location }
        if ($code -ne 0) { throw "Static $Action failed." }
        if ($Action -eq 'Build') {
            if (@(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'kilogram-*' }).Count) { throw 'Live Kilogram process prevents fixed image stage.' }
            $built = Join-Path $workspace 'target/verification/kilogram-cli.exe'
            $builtHash = (Get-FileHash -LiteralPath $built -Algorithm SHA256).Hash.ToLowerInvariant()
            foreach ($image in $fixed) { Copy-Item -LiteralPath $built -Destination (Join-Path $workspace $image) -Force }
            $stagedImages = Inventory $fixed
            if (@($stagedImages | Where-Object { $_.sha256 -cne $builtHash }).Count) { throw 'Staged image differs from built image.' }
        }
    }
} catch {
    $code = 1
    $primaryError = $_.Exception.Message
    try { $_ | Out-String | Add-Content -LiteralPath $logPath }
    catch { $finalizationErrors.Add('Primary error log: '+$_.Exception.Message) }
} finally {
    $env:CARGO_NET_OFFLINE = $priorOffline
    try {
        $after = Inventory $sourcePaths
        $sourceInventoryComplete = $true
        if ($Action -ne 'Fmt' -and (ConvertTo-Json -InputObject $before -Depth 6 -Compress) -cne (ConvertTo-Json -InputObject $after -Depth 6 -Compress)) { $finalizationErrors.Add('Source changed during static verification.') }
    } catch { $finalizationErrors.Add('Final source inventory incomplete: '+$_.Exception.Message) }
    try {
        $frozenAfter = Inventory @($frozen)
        $frozenInventoryComplete = $true
        if ((ConvertTo-Json -InputObject $frozenBefore -Compress) -cne (ConvertTo-Json -InputObject $frozenAfter -Compress)) { $finalizationErrors.Add('Frozen baseline image changed.') }
    } catch { $finalizationErrors.Add('Final frozen-image inventory incomplete: '+$_.Exception.Message) }
    if ($finalizationErrors.Count -ne 0) { $code = 1 }
    $timer.Stop()
    $record = [ordered]@{case=$Case;action=$Action;classification='static-only; no fixture or executable test';baseline='eafd0f873eb31b64377e8e65e06bc84a911f6dc6';native_cwd=$workspace;argv=$argv;priority='BelowNormal';cargo_jobs=12;offline=$true;exit_code=$code;primary_error=$primaryError;finalization_errors=$finalizationErrors.ToArray();elapsed_seconds=$timer.Elapsed.TotalSeconds;sources_before=$before;sources_after=$after;source_inventory_after_complete=$sourceInventoryComplete;staged_images=$stagedImages;frozen_images_before=$frozenBefore;frozen_images_after=$frozenAfter;frozen_inventory_after_complete=$frozenInventoryComplete}
    try { $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $recordPath -Encoding utf8 }
    catch {
        $code = 1
        $record.exit_code = 1
        $record.finalization_errors = @($record.finalization_errors) + @('Case record write failed: '+$_.Exception.Message)
        [Console]::Error.WriteLine((ConvertTo-Json -InputObject $record -Depth 8))
    }
}
Get-Content -LiteralPath $logPath -Tail 25
[ordered]@{case=$Case;exit_code=$code;source_count=$before.Count;elapsed_seconds=$timer.Elapsed.TotalSeconds;staged_images=$stagedImages}|ConvertTo-Json -Depth 5
exit $code
