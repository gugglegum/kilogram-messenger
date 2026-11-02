# Single final genuine, uninstrumented predicate measurement. No build/stage/runtime.
$policy=New-S54Policy;$root=$policy.Root
$old=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-02.result.json') 1048576) 1048576
$normal=Read-S54Json $old.normal 8388608;$tests=Read-S54Json $old.tests 8388608
$paths=Get-S54SourcePaths $root;$fresh=@($paths|ForEach-Object {Get-S54Pin $_ 8388608})
$changed=@('scripts/invoke-migration-s54-whole.ps1','scripts/test-migration-s54-admission.ps1')
foreach($name in $changed){
    if($name -cnotin $script:S54Helpers -or $name -cin $script:S54Helpers[0..2]){throw 'Helper-only dependency scope required.'}
    foreach($capture in @($normal,$tests)){if(@($capture.build_inputs_before.source_inputs|Where-Object path -CEQ $name).Count){throw 'Frozen compiler input changed; no rebuild authorized.'}}
}
$baseline=Read-S54Json (Get-S54Pin (Join-Path $base 'baseline.json') 16384) 16384
$failed=Read-S54Json (Get-S54Pin (Join-Path $root 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block4d-f-preparation-report-01.pins.json') 65536) 65536
$failedPins=@($failed.administration)+@($failed.public_artifacts)
$residual=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4f-residual-cost-01/seal-01.result.json') 1048576) 1048576
function Assert-MeasurePreservation {
    if((& git rev-parse HEAD) -cne $baseline.head -or @(& git diff --cached --name-only).Count){throw 'HEAD/index preservation.'}
    foreach($p in $baseline.navigation){$null=Read-S54Pin $p 8388608}
    foreach($p in $residual.source_pins){if([IO.Path]::GetRelativePath($root,$p.path).Replace('\','/') -cnotin $changed){$null=Read-S54Pin $p 8388608}}
    Assert-S54Equal @($paths|Sort-Object) @((Get-S54SourcePaths $root)|Sort-Object)
    foreach($p in $fresh){$null=Read-S54Pin $p 8388608}
    foreach($p in @($old.normal,$old.tests)+@($old.test_origins_after)+@($old.test_dep_info_after)+@($old.normal_origins)+@($old.normal_dep_info)+$failedPins){$null=Read-S54Pin $p 134217728}
    foreach($r in $old.images){$null=Read-S54Pin $r.pin 134217728}
    foreach($p in @('.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-02','.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-02','.tmp/migration-s54-integrated/P-s54-whole-resumed-l1-01-attempt-01','.tmp/migration-s54-public-evidence/P-s54-whole-resumed-l1-01-attempt-01')){if(Test-Path -LiteralPath (Join-Path $root $p)){throw 'Production allocation forbidden.'}}
}
Assert-MeasurePreservation
$proof=@{source=@{files=$fresh};images=$old.images;runtime=@{environment=$normal.build_inputs_before.environment;configs=$normal.build_inputs_before.configs;resolutions=$policy.Resolutions}}
$source=Get-S54Path (Join-Path $root 'scripts/invoke-migration-s54-whole.ps1')
foreach($fn in @('Get-S54Path','Read-S54Public','Assert-S54Compiler','Assert-S54Embedded')){if((Get-Item ('Function:'+$fn)).ScriptBlock.File -cne $source){throw 'Actual uninstrumented production functions required.'}}
Write-OwnedPath ($Step+'.before.json') @{classification='fresh final candidate pins; actual unchanged retained captures/images';sources=$fresh;normal=$old.normal;tests=$old.tests;images=$old.images;changed_helpers=$changed;outside_frozen_compiler_input_projection=$true;compiler_rebuild_needed=$false;instrumented=$false;function_source=$source}
$times=[Collections.Generic.List[object]]::new()
foreach($role in @('normal','tests')){
    $timer=[Diagnostics.Stopwatch]::StartNew();$passed=$false
    try{Assert-S54Compiler $proof $policy $(if($role -ceq 'normal'){$normal}else{$tests}) ($role -ceq 'normal');$passed=$true}
    finally{$timer.Stop();$row=@{role=$role;seconds=$timer.Elapsed.TotalSeconds;passed=$passed;instrumented=$false};$times.Add($row);Write-OwnedPath ($Step+'.'+$role+'.json') $row}
}
$embeddedBefore=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-public-02/embedded-before.json') 65536) 65536
$proof.compiler=@{embedded=@{before=$embeddedBefore.inputs;after=$old.embedded_after;dep_info=$old.test_dep_info_after}}
$timer=[Diagnostics.Stopwatch]::StartNew();Assert-S54Embedded $proof $policy $tests;$timer.Stop();$embeddedSeconds=$timer.Elapsed.TotalSeconds
Assert-MeasurePreservation
$rows=@(Get-CimInstance Win32_Process -Property ProcessId,ExecutablePath -OperationTimeoutSec 3 -ErrorAction Stop)
$images=@($old.images.pin.path);$active=@($rows|Where-Object {$_.ExecutablePath -and $_.ExecutablePath -in $images}|ForEach-Object {@{pid=$_.ProcessId;path=$_.ExecutablePath}})
if($active.Count){throw 'Unexpected exact image process; none stopped.'}
Write-OwnedPath ($Step+'.result.json') @{classification='one uninstrumented final candidate compiler pair plus embedded; not production inventory/runtime';head=$baseline.head;timings=@($times);pair_seconds=($times[0].seconds+$times[1].seconds);full_embedded_seconds=$embeddedSeconds;source_count=$fresh.Count;captures=@($old.normal,$old.tests);images=$old.images;normal_origins=$old.normal_origins;normal_dep_info=$old.normal_dep_info;test_origins_after=$old.test_origins_after;test_dep_info_after=$old.test_dep_info_after;failed_F01_pins=$failedPins;source_capture_image_and_failed_F01_pins_preserved=$true;navigation_preserved=$true;fresh_candidate_context=$true;outside_frozen_compiler_input_projection=$true;compiler_rebuild_needed=$false;instrumented=$false;native_process_inventory_succeeded=$true;active_exact_images=$active;production_F02_P01_directories_absent=$true;runtime='not_established'}
Write-Output 'final_uninstrumented_compiler_pair_and_embedded=passed;compiler_rebuild=false;runtime=false'
