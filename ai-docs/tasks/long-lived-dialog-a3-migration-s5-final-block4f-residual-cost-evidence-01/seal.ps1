# Final read-only pin comparison with the successful profile's original carriers.
$profile=Read-S54Json (Get-S54Pin (Join-Path $base 'profile-01.preservation.json') 1048576) 1048576
foreach($p in @($profile.source_pins)+@($profile.normal,$profile.tests)+@($profile.images|ForEach-Object {$_.pin})){$null=Read-S54Pin $p 134217728}
if($profile.head -cne (& git rev-parse HEAD) -or @(& git diff --cached --name-only).Count){throw 'Final HEAD/index preservation.'}
Pin-History
$old=Read-S54Json (Get-S54Pin (Join-Path $root 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block4d-f-preparation-report-01.pins.json') 65536) 65536
$failedPins=@($old.administration)+@($old.public_artifacts)
$rows=@(Get-CimInstance Win32_Process -Property ProcessId,ExecutablePath -OperationTimeoutSec 3 -ErrorAction Stop)
$imagePaths=@($profile.images.pin.path)
$active=@($rows|Where-Object {$_.ExecutablePath -and $_.ExecutablePath -in $imagePaths}|ForEach-Object {@{pid=$_.ProcessId;path=$_.ExecutablePath}})
if($active.Count){throw 'Unexpected exact image process; none stopped.'}
$diagnosticPins=@(Get-ChildItem -LiteralPath $base -File | Sort-Object Name | ForEach-Object {Get-S54Pin $_.FullName 8388608})
Write-OwnedResidual 'seal-01.result.json' @{classification='final source/capture/image/history preservation; not runtime acceptance';head=(& git rev-parse HEAD);index_empty=$true;original_profile_pins_reverified=$true;source_count=$profile.source_count;source_pins=$profile.source_pins;captures=@($profile.normal,$profile.tests);images=$profile.images;retained_context=(Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-02.result.json') 1048576);failed_F01_pins=$failedPins;failed_F01_pins_verified=$true;normal_origins=$ResidualContext.normal_origins;normal_dep_info=$ResidualContext.normal_dep_info;test_origins_after=$ResidualContext.test_origins_after;test_dep_info_after=$ResidualContext.test_dep_info_after;native_process_inventory_succeeded=$true;active_exact_images=$active;production_F02_P01_directories_absent=$true;diagnostic_file_pins_before_seal=$diagnosticPins;navigation=(Read-S54Json (Get-S54Pin (Join-Path $base 'baseline.json') 16384) 16384).navigation;accepted_source_unchanged=$true;historical_fixtures_written=$false;runtime='not_established'}
Write-Output 'final_profile_source_capture_image_and_failed_F01_pins=preserved;no_runtime_allocation=true'
