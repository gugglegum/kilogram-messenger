[CmdletBinding()]
param(
 [Parameter(Mandatory)][ValidatePattern('^[CF]-[a-z0-9-]+$')][string]$Case,
 [ValidateSet('C','F')][string]$Set='F',
 [ValidateSet('Tests','SmallOrigin','ExhaustedOrigin','ColdStatus','DeadlineProbe','PreservationProbe')][string]$Action='Tests',
 [string]$Filter,
 [string[]]$Package=@('kilogram-cli'),
 [string]$FixtureDirectory,
 [string]$ResumeDirectory,
 [string]$LongApprovalFile,
 [ValidateSet('Restricted','NativeToken')][string]$ExecutionMode='Restricted'
)
$timer=[Diagnostics.Stopwatch]::StartNew()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence'
$recordPath=Join-Path $evidence "$Case.json"
if((Test-Path -LiteralPath $recordPath) -or (Test-Path -LiteralPath (Join-Path $evidence "$Case.started.json"))){throw 'Contributing attempt IDs/started markers are immutable; unfinished attempts require root reconciliation.'}
$ledgerPath=Join-Path $evidence 'contributing-cases.json'
$ledger=Get-Content -LiteralPath $ledgerPath -Raw|ConvertFrom-Json
if([double]$ledger.charged_elapsed_seconds.$Set -ge [double]$ledger.new_ceilings_seconds.$Set){throw 'Budget exhausted; callback root before another attempt.'}
if($Action -in @('SmallOrigin','ExhaustedOrigin','ColdStatus') -and $Set -ne 'F'){throw 'Real fixture actions require F classification.'}
if($Action -in @('DeadlineProbe','PreservationProbe') -and $Set -ne 'C'){throw 'Mechanism probe requires C classification.'}
if($Case -notlike "$Set-*"){throw 'Case prefix must match the contributing set.'}
if($Action -eq 'ExhaustedOrigin' -and [double]$ledger.new_ceilings_seconds.F-[double]$ledger.charged_elapsed_seconds.F -lt 1215){throw 'Long origin requires850+140+225 contingency remaining.'}
$deadline=$null;$decision=$null;$readyException=$false
$phasePreservationC=$Case -ceq 'C-boundary-phase-preservation-01'
$checkpointPreservationF=$Case -ceq 'F-exhausted-preservation-01'
$preservationCompletion=$phasePreservationC -or $checkpointPreservationF
if($preservationCompletion){
 $expectedSet=if($phasePreservationC){'C'}else{'F'}
 $expectedFilter=if($phasePreservationC){'migration_s5_l1_phase_preservation_opaque_held_locks'}else{'migration_s5_l1_stopped_checkpoint_preservation_only'}
 if($Set -cne $expectedSet -or $Action -cne 'Tests' -or $Filter -cne $expectedFilter -or $Package.Count -ne 1 -or $Package[0] -cne 'kilogram-cli' -or $ResumeDirectory){throw 'Preservation completion case/selector/package mismatch.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-boundary03-completion-review.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne 'c98a14c445985acfa4e2295f9519848b73e8316a9d72b764900a6b36750211e0'){throw 'Narrow preservation authority changed.'}
 foreach($fixedHelper in @(@('scripts/preserve-migration-s5-l1-input.ps1','25fe5a7c80821fb83a0fab5117585a48e6afe729094c1d45931aaa1556b044f3'),@('apps/kilogram-cli/tests/support/migration_s5_l1_obligations.rs','68bc85bb08508bb73b9a89344da22518e6d1bc1ee76797c3e1bf50ad4dd06b00'))){
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $fixedHelper[0])).Hash.ToLowerInvariant() -cne $fixedHelper[1]){throw 'Raw preservation implementation must remain unchanged.'}
 }
 foreach($prior03 in @(@('F-exhausted-boundary-03.json','6da9c4ae43dad5234df1da2aba156e0a972a7d3895069d4cf87fcdbe7932cb48'),@('F-exhausted-boundary-03.log','739839e858e1a5131e0bbe09063851f4fc2ab193d96297fd2466679b78f4a38f'),@('F-exhausted-boundary-03-observation.json','7f2adda05a3e1a845b4b7580db54e402283fad608e0d2c24de98a7b82f5f05f0'),@('F-exhausted-boundary-03-phases.jsonl','940df52d347a96dcb48d1f62b7fbe2cdfc5440a62f96868f4872aa86de94c4af'),@('F-exhausted-boundary-03-checkpoint-info.json','2b359e4623b2e603977342df69f441e35e0276643ee420a751a3515e361771b3'),@('F-exhausted-boundary-03-two-indexed-reconnect-barrier.log','545f0849be3035bb86521b03bd3dce5fd972c472cce603f27ae5c722647c8b8f'))){
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence $prior03[0])).Hash.ToLowerInvariant() -cne $prior03[1]){throw 'Completed case03 evidence changed.'}
 }
 $public03=Get-Content -Raw -LiteralPath (Join-Path $evidence 'F-exhausted-boundary-03-observation.json')|ConvertFrom-Json
 foreach($binding03 in $public03.exports){
  $export03=Join-Path $workspace $binding03.export.path
  if((Get-Item -LiteralPath $export03).Length -ne $binding03.export.bytes -or (Get-FileHash -Algorithm SHA256 -LiteralPath $export03).Hash.ToLowerInvariant() -cne $binding03.export.sha256){throw 'Approved case03 public export changed.'}
 }
 $failed03=@($ledger.cases|Where-Object {$_.case -ceq 'F-exhausted-boundary-03'})
 if($failed03.Count -ne 1 -or $failed03[0].exit_code -ne 1 -or $failed03[0].elapsed_seconds -ne 649.592232 -or $failed03[0].deadline.work_expired -or $failed03[0].deadline.owned_children_after_cleanup -ne 0){throw 'Boundary03 must remain the charged whole failure.'}
 if($phasePreservationC){
  if($FixtureDirectory -or [double]$ledger.new_ceilings_seconds.C-[double]$ledger.charged_elapsed_seconds.C -lt 40){throw 'Opaque C requires no genuine fixture and remaining whole40.'}
 }else{
  $expectedInput=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb'))
  if(-not $FixtureDirectory -or -not ([IO.Path]::GetFullPath($FixtureDirectory)).Equals($expectedInput,[StringComparison]::OrdinalIgnoreCase)){throw 'Preservation only accepts the exact stopped case03 path.'}
  $component=@($ledger.cases|Where-Object {$_.case -ceq 'C-boundary-phase-preservation-01'})
  if($component.Count -ne 1 -or $component[0].exit_code -ne 0 -or $component[0].deadline.work_expired -or $component[0].deadline.owned_children_after_cleanup -ne 0){throw 'Actual whole opaque Phases C PASS is required first.'}
  if([double]$ledger.new_ceilings_seconds.F-[double]$ledger.charged_elapsed_seconds.F -lt 365){throw 'Preservation whole140 plus225 contingency does not fit.'}
  if(@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.ProcessName -like 'kilogram-*'}).Count){throw 'Known Kilogram process still active; stopped-only preservation refused.'}
 }
 if(Test-Path -LiteralPath (Join-Path $evidence "$Case-result.json")){throw 'Completion result is immutable.'}
 if(-not $LongApprovalFile){throw 'Preservation completion requires a separate later root go-ahead.'}
 $approval=Get-Content -Raw -LiteralPath $LongApprovalFile|ConvertFrom-Json
 if($approval.approved_case -cne $Case -or $approval.root_thread -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or $approval.runner_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash.ToLowerInvariant() -or $approval.ledger_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $ledgerPath).Hash.ToLowerInvariant()){throw 'Completion go-ahead does not bind this case/runner/current ledger.'}
}
$boundaryExtended=$Case -ceq 'F-exhausted-boundary-03'
$boundaryObserved=$Case -ceq 'F-exhausted-boundary-02'
$boundaryGroup=$Case -ceq 'F-exhausted-boundary-01' -or $boundaryObserved -or $boundaryExtended
if($boundaryGroup){
 if($Set -cne 'F' -or $Action -cne 'Tests' -or $Filter -cne 'migration_s5_l1_genuine_exhausted512_refusal_receive_history_checkpoint' -or $Package.Count -ne 1 -or $Package[0] -cne 'kilogram-cli' -or $ResumeDirectory){throw 'Exhausted boundary selector/classification mismatch.'}
 $expectedOrigin=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-l1/F-exhausted-origin-01'))
 if(-not $FixtureDirectory -or -not ([IO.Path]::GetFullPath($FixtureDirectory)).Equals($expectedOrigin,[StringComparison]::OrdinalIgnoreCase)){throw 'Boundary requires the one genuine frozen511/16 origin.'}
 $boundaryReserve=if($boundaryExtended){1025}elseif($boundaryObserved){825}else{365}
 if([double]$ledger.new_ceilings_seconds.F-[double]$ledger.charged_elapsed_seconds.F -lt $boundaryReserve){throw 'Boundary whole allowance plus contingency225 no longer fit.'}
}
if($Action -eq 'ExhaustedOrigin' -or $boundaryGroup){
 if($ExecutionMode -cne 'NativeToken'){throw 'Long/boundary local network execution requires ordinary native-token approval.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-field-continuation-review-4.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '2dac985cfdcede0729a1c61156dea3f252fc861eeebf6f2a98053fcd9074b4bb'){throw 'First-long readiness authority changed.'}
 foreach($acceptedInput in @(@('F-review1-own-f-02.json','aa04de33caa56d3a23ca3d52f4cb8dedc7a8acd963baab0afe96b36d6348d72f'),@('F-review1-unacked-01.json','bc5b98b29d16a175ebeabfc16498a1d1d0cb2785d07ef3d909b7843ae710b5bc'),@('F-isolation-02.json','bb10d5256bbf8087e24a558c7ed4050dc1c96f2390d0c2e4495be5bda4257d70'),@('F-capacity-01.json','6121aaeded1b847fdfc219d577e26c40a4b8f5b37486c0fa461fc3b0c990f14d'),@('F-capacity-01.log','0856083c34de6b4034b7b36707a4a21f6f81e0aca24fde9d7574013ec68cc709'),@('F-review1-ready-02.json','20540287fc8e478ba72bdfc49786f8b6c6a90e02b298ce3b1fdef7745278e7be'),@('F-review1-ready-02.log','c09dfa264573ff3873f8bca52998dadcff9d22a52748b59451d9c32ebb52f463'),@('F-review1-ready-02-parent-result.json','fcfb7da3b292b8427f99f49e20fa55a067cc0a727f7936900179520e3edcccf0'))){
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence $acceptedInput[0])).Hash.ToLowerInvariant() -cne $acceptedInput[1]){throw 'Root-accepted small-gate artifact changed.'}
 }
 foreach($goodCase in @('F-review1-own-f-02','F-review1-unacked-01','F-isolation-02')){
  $entry=@($ledger.cases|Where-Object {$_.case -ceq $goodCase})
  if($entry.Count -ne 1 -or $entry[0].exit_code -ne 0){throw 'Required prior whole PASS missing.'}
 }
 foreach($splitCase in @('F-capacity-01','F-review1-ready-02')){
  $entry=@($ledger.cases|Where-Object {$_.case -ceq $splitCase})
  if($entry.Count -ne 1 -or $entry[0].exit_code -ne 1 -or $entry[0].deadline.owned_children_after_cleanup -ne 0 -or $entry[0].deadline.work_expired -ne ($splitCase -ceq 'F-review1-ready-02')){throw 'Accepted split verdict changed; historical FAIL must remain.'}
 }
 if($boundaryObserved -or $boundaryExtended){
  $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-boundary-continuation-review-1.md'
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne 'a9b3fd75ce257362297a9ec7c4fe00133917a42bf2ecbd4a921d41bfa4870341'){throw 'Instrumented-boundary preparation authority changed.'}
  foreach($priorBoundary in @(@('F-exhausted-boundary-01.json','072b522817e7f62e9bbc0f930f3d3969c2b56675c4ae348931e4b113d895753b'),@('F-exhausted-boundary-01.log','b7b5efa02401eef44abd6488f64b2f0ca3a5b975e90e877945fb51d9e69f55ab'))){
   if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence $priorBoundary[0])).Hash.ToLowerInvariant() -cne $priorBoundary[1]){throw 'Prior failed boundary provenance changed.'}
  }
  $failedBoundary=@($ledger.cases|Where-Object {$_.case -ceq 'F-exhausted-boundary-01'})
  if($failedBoundary.Count -ne 1 -or $failedBoundary[0].exit_code -ne 1 -or -not $failedBoundary[0].deadline.work_expired -or $failedBoundary[0].deadline.owned_children_after_cleanup -ne 0){throw 'Boundary01 must remain its exact charged cutoff.'}
 }
 if($boundaryExtended){
  $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-boundary-continuation-review-2.md'
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne 'e6ebba9e204eea61d53438012d2bcf11f98cd9159a34f4008d69e08e0cea6f0f'){throw 'Boundary03 static-preparation and budget authority changed.'}
  foreach($priorBoundary02 in @(@('F-exhausted-boundary-02.json','dea5e8749393815f297aa9bb5cf714e353cce8da2a860944d3a3f9f2b224357b'),@('F-exhausted-boundary-02.log','c914820f5ddab5e3acd89690aa71f248cf81ad9bab0964f22e380c349c617bcb'),@('F-exhausted-boundary-02-observation.json','150b9067d9e737cb82f79a13ff736a86ba8cc62b8ecac7a155b646ec56f68b38'),@('F-exhausted-boundary-02-phases.jsonl','51ec859236a9d0ca0855f5b38a5abbc42f2b5ada7cbce6da7245dd5232e1247e'))){
   if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence $priorBoundary02[0])).Hash.ToLowerInvariant() -cne $priorBoundary02[1]){throw 'Prior boundary02 evidence changed.'}
  }
  $failedBoundary02=@($ledger.cases|Where-Object {$_.case -ceq 'F-exhausted-boundary-02'})
  if($failedBoundary02.Count -ne 1 -or $failedBoundary02[0].exit_code -ne 1 -or $failedBoundary02[0].elapsed_seconds -ne 568.1367185 -or $failedBoundary02[0].deadline.work_expired -or $failedBoundary02[0].deadline.owned_children_after_cleanup -ne 0){throw 'Boundary02 must remain its exact charged controller cutoff.'}
  if($ledger.new_ceilings_seconds.F -ne 3650 -or @($ledger.budget_amendments|Where-Object {$_.sha256 -ceq 'e6ebba9e204eea61d53438012d2bcf11f98cd9159a34f4008d69e08e0cea6f0f'}).Count -ne 1){throw 'Boundary03 requires the explicit activated budget amendment4.'}
 }
}
$fieldCases=[ordered]@{
 'F-review1-own-f-02'=@{filter='migration_s5_l1_review1_genuine_partial_own_f_both_genesis_roles';whole=120;work=100}
 'F-review1-unacked-01'=@{filter='migration_s5_l1_review1_unacked128_before_native_and_one_exact_receipt';whole=240;work=220}
 'F-review1-ready-02'=@{filter='migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork';whole=160;work=140}
 'F-isolation-02'=@{filter='migration_s5_l1_different_peer_c2_survives_signed_fork_and_account_pin';whole=100;work=85}
 'F-capacity-01'=@{filter='migration_s5_process_capacity_crash_and_predecessor_fences';whole=45;work=40}
}
$reviewField=$fieldCases.Contains($Case)
if($reviewField){
 $fieldPolicy=$fieldCases[$Case]
 if($Set -cne 'F' -or $Action -cne 'Tests' -or $Filter -cne $fieldPolicy.filter -or $Package.Count -ne 1 -or $Package[0] -cne 'kilogram-cli' -or $ResumeDirectory){throw 'Review2 F case classification/selector/package mismatch.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-implementation-review-2.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne 'd1fe3ec11a5e9c36d38f0ca874bef609649a30ba70fdcd114e2ee1f7de9550ad'){throw 'Review2 F authority changed.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-field-continuation-review-1.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '8dde9b2b0cc26de3beb2dc94800a6d628b2431879bb27e33c61933c905efaa40'){throw 'Native-execution continuation authority changed.'}
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence 'F-review1-own-f-01.json')).Hash.ToLowerInvariant() -cne '43b7cf2e3e7815ef1c258c7f502ec705a4df2f281ad077b2d1d588f4f3210c0c'){throw 'Prior failed attempt provenance changed.'}
 if($ExecutionMode -cne 'NativeToken'){throw 'Continuation requires native execution through the ordinary tool approval layer.'}
 $orderedCases=@($fieldCases.Keys);$fieldIndex=[Array]::IndexOf($orderedCases,$Case)
 if($fieldIndex -ge 2){
  $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-field-continuation-review-2.md'
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '28b1385688e394a8219fea1ee576cf6d244e8318c1186bdb1ff9c53759d3bb21'){throw 'Ready fixture continuation authority changed.'}
  foreach($priorBinding in @(@('F-review1-own-f-02','aa04de33caa56d3a23ca3d52f4cb8dedc7a8acd963baab0afe96b36d6348d72f'),@('F-review1-unacked-01','bc5b98b29d16a175ebeabfc16498a1d1d0cb2785d07ef3d909b7843ae710b5bc'),@('F-review1-ready-01','9d3419460b7d031833ed904b705cbbe87be4424fef75f2819eea447bb66f5c60'))){
   if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence ($priorBinding[0]+'.json'))).Hash.ToLowerInvariant() -cne $priorBinding[1]){throw 'Prior reviewed Ready continuation provenance changed.'}
  }
  $arityPassed=@($ledger.cases|Where-Object {$_.case -ceq 'C-ready-arity-fixture-01'})
  if($arityPassed.Count -ne 1 -or $arityPassed[0].exit_code -ne 0){throw 'Actual focused Ready arity component PASS required.'}
 }
 if($fieldIndex -ge 3){
  $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-field-continuation-review-3.md'
  if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne 'c05a408a653f9ac7d35fffd31cd50df15620b726e5357013d8525ea9e669bf02'){throw 'Root-reviewed Ready split-result authority changed.'}
  foreach($readyBinding in @(@('F-review1-ready-02.json','20540287fc8e478ba72bdfc49786f8b6c6a90e02b298ce3b1fdef7745278e7be'),@('F-review1-ready-02.log','c09dfa264573ff3873f8bca52998dadcff9d22a52748b59451d9c32ebb52f463'),@('F-review1-ready-02-parent-result.json','fcfb7da3b292b8427f99f49e20fa55a067cc0a727f7936900179520e3edcccf0'))){
   if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $evidence $readyBinding[0])).Hash.ToLowerInvariant() -cne $readyBinding[1]){throw 'Root-reviewed Ready split-result artifact changed.'}
  }
  $readyException=$true
 }
 for($previous=0;$previous -lt $fieldIndex;$previous++){
  $passed=@($ledger.cases|Where-Object {$_.case -ceq $orderedCases[$previous]})
  if($readyException -and $orderedCases[$previous] -ceq 'F-review1-ready-02' -and $passed.Count -eq 1 -and $passed[0].exit_code -eq 1 -and $passed[0].deadline.work_expired -eq $true -and $passed[0].deadline.owned_children_after_cleanup -eq 0){continue}
  if($passed.Count -ne 1 -or $passed[0].exit_code -ne 0){throw 'Review2 F groups must pass serially; callback root after a failure.'}
 }
 $required=1215
 for($remaining=$fieldIndex;$remaining -lt $orderedCases.Count;$remaining++){$required+=$fieldCases[$orderedCases[$remaining]].whole}
 if([double]$ledger.new_ceilings_seconds.F-[double]$ledger.charged_elapsed_seconds.F -lt $required){throw 'Review2 remaining upper bounds plus long1215 reserve no longer fit; callback root.'}
}
$arityGroup=$Case -ceq 'C-ready-arity-fixture-01'
if($arityGroup){
 if($Set -cne 'C' -or $Action -cne 'Tests' -or $Filter -cne 'indexed_l1_ready_arity_fixture' -or $Package.Count -ne 1 -or $Package[0] -cne 'kilogram-cli'){throw 'Ready arity C selector/classification mismatch.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-field-continuation-review-2.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '28b1385688e394a8219fea1ee576cf6d244e8318c1186bdb1ff9c53759d3bb21'){throw 'Ready arity C authority changed.'}
 if([double]$ledger.new_ceilings_seconds.C-[double]$ledger.charged_elapsed_seconds.C -lt 40){throw 'Ready arity whole40 no longer fits C budget.'}
}
$reviewGroup=$Case -ceq 'C-review1-01'
if($reviewGroup){
 if($Set -cne 'C' -or $Action -cne 'Tests' -or $Filter -cne 'indexed_l1_review1'){throw 'Review1 C group selector/classification mismatch.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-implementation-review-1.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '49f8c28e0fd7a46e46df8f93622ac366362a7d48340eb2cd14c6e6e5b4e7fd49'){throw 'Review1 C authority changed.'}
}
if($ResumeDirectory){
 if($Case -cne 'F-obligations-resume-03' -or $Action -ne 'Tests' -or $Filter -cne 'migration_s5_l1_obligations_full64_head_saved_captures_and_c2'){throw 'Only the root-approved continuation is authorized.'}
 $expected=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-storage/l1-obligations-Iw3nQI'))
 if([IO.Path]::GetFullPath($ResumeDirectory) -cne $expected){throw 'Wrong stopped active branch.'}
 $decision='ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-obligations-continuation-review-3.md'
 if((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $workspace $decision)).Hash.ToLowerInvariant() -cne '8a075a753718605c1dd2659846b54673b2fb3930c0e1f229a662223a1b82d46c'){throw 'Continuation decision changed.'}
}
if($Action -eq 'ExhaustedOrigin'){
 if($Case -cne 'F-exhausted-origin-01'){throw 'Only the first fixed-ID origin may run without a new root retry decision.'}
 if(@($ledger.cases|Where-Object {$_.action -eq 'ExhaustedOrigin'}).Count -or @(Get-ChildItem -LiteralPath $evidence -Filter 'F-exhausted-origin-*.started.json').Count){throw 'A previous long attempt requires a new root decision; automatic repeats refused.'}
 if(-not $LongApprovalFile){throw 'First long origin requires root readiness go-ahead binding.'}
 $approval=Get-Content -Raw -LiteralPath $LongApprovalFile|ConvertFrom-Json
 if($approval.approved_case -cne $Case -or $approval.root_thread -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or $approval.runner_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash.ToLowerInvariant() -or $approval.ledger_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $ledgerPath).Hash.ToLowerInvariant()){throw 'Long go-ahead does not bind this exact runner/case/ledger.'}
}
if($boundaryGroup){
 if(-not $LongApprovalFile){throw 'Boundary requires its own root go-ahead after origin accounting.'}
 $approval=Get-Content -Raw -LiteralPath $LongApprovalFile|ConvertFrom-Json
 $originRecordPath=Join-Path $evidence 'F-exhausted-origin-01.json'
 $originCase=Get-Content -Raw -LiteralPath $originRecordPath|ConvertFrom-Json
 if($originCase.exit_code -ne 0 -or $originCase.deadline.work_expired -or $originCase.deadline.owned_children_after_cleanup -ne 0){throw 'Actual whole origin PASS required before boundary.'}
 if($approval.approved_case -cne $Case -or $approval.root_thread -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or $approval.runner_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash.ToLowerInvariant() -or $approval.ledger_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $ledgerPath).Hash.ToLowerInvariant() -or $approval.origin_case_sha256 -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $originRecordPath).Hash.ToLowerInvariant()){throw 'Boundary go-ahead does not bind the actual runner/ledger/origin.'}
}
if($ResumeDirectory -or $Action -eq 'ExhaustedOrigin' -or $reviewGroup -or $reviewField -or $arityGroup -or $boundaryGroup -or $preservationCompletion){
 . (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
 $whole=if($phasePreservationC){40}elseif($checkpointPreservationF){140}elseif($boundaryExtended){800}elseif($boundaryObserved){600}elseif($boundaryGroup){140}elseif($arityGroup){40}elseif($reviewField){$fieldPolicy.whole}elseif($reviewGroup){120}elseif($ResumeDirectory){240}else{900};$work=if($phasePreservationC){30}elseif($checkpointPreservationF){120}elseif($boundaryExtended){780}elseif($boundaryObserved){580}elseif($boundaryGroup){120}elseif($arityGroup){30}elseif($reviewField){$fieldPolicy.work}elseif($reviewGroup){100}elseif($ResumeDirectory){210}else{850}
 $deadline=[KilogramS5Deadline]::new($work-$timer.Elapsed.TotalSeconds,$whole-$timer.Elapsed.TotalSeconds)
 [ordered]@{case=$Case;started_utc=[DateTime]::UtcNow.ToString('O');owner_pid=$PID;whole_seconds=$whole;work_seconds=$work;root_decision=$decision;action=$Action;classification='attempt-start-not-final-result'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $evidence "$Case.started.json") -Encoding utf8
}
$code=0;$failure=$null;$argv=@();$fixture=$null
$priorOffline=$env:CARGO_NET_OFFLINE;$priorFixture=$env:KILOGRAM_S5_FIXTURE_DIR
$priorResume=$env:KILOGRAM_S5_L1_RESUME_DIR
$priorCase=$env:KILOGRAM_S5_CASE
$priorPowerShell=$env:KILOGRAM_S5_POWERSHELL
$sourcePaths=@(& git -C $workspace diff --name-only)+@(& git -C $workspace ls-files --others --exclude-standard)
$sourcePaths=@($sourcePaths|Where-Object {$_ -match '\.(rs|ps1)$'})+@('Cargo.lock','Cargo.toml','scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1')
function Inventory($Paths) { @($Paths|Sort-Object -Unique|ForEach-Object {
 $full=Join-Path $workspace $_
 [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
}) }
$sources=Inventory $sourcePaths
$images=Inventory @('target/compatibility/long-lived-dialog-migration/kilogram-cli.exe','target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe')
try {
 $env:CARGO_NET_OFFLINE='true'
 $env:KILOGRAM_S5_CASE=$Case
 $env:KILOGRAM_S5_POWERSHELL=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
 if($deadline){$deadline.CheckWork()}
 if($FixtureDirectory){$env:KILOGRAM_S5_FIXTURE_DIR=[IO.Path]::GetFullPath($FixtureDirectory)}
 if($ResumeDirectory){$env:KILOGRAM_S5_L1_RESUME_DIR=[IO.Path]::GetFullPath($ResumeDirectory)}else{Remove-Item Env:/KILOGRAM_S5_L1_RESUME_DIR -ErrorAction SilentlyContinue}
 if($Action -eq 'PreservationProbe') {
  $argv=@('scripts/test-migration-s5-l1-preservation.ps1','-Case',$Case)
  & (Join-Path $PSScriptRoot 'test-migration-s5-l1-preservation.ps1') -Case $Case *> (Join-Path $evidence "$Case.log")
 } elseif($Action -eq 'DeadlineProbe') {
  $argv=@('scripts/test-migration-s5-l1-deadline.ps1','-OutputDirectory',(Join-Path $workspace ".tmp/migration-s5-deadline/$Case"))
  & (Join-Path $PSScriptRoot 'test-migration-s5-l1-deadline.ps1') -OutputDirectory (Join-Path $workspace ".tmp/migration-s5-deadline/$Case") *> (Join-Path $evidence "$Case.log")
 } elseif($Action -eq 'Tests') {
  if(-not $Filter){throw 'Focused selector required.'}
  $live=@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.ProcessName -like 'kilogram-*-tests'})
  if($live.Count){throw 'Live stable harness prevents recopy.'}
  if($ResumeDirectory){
   $fixed=@([IO.Path]::GetFullPath((Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe')),[IO.Path]::GetFullPath((Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe')),[IO.Path]::GetFullPath((Join-Path $workspace 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe')))
   if(@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.Path -and $fixed -contains $_.Path}).Count){throw 'Known fixed runtime still active; resume prohibited.'}
  }
  $argv=@('scripts/run-cargo-tests-stable.ps1','-Package')+$Package+@('-CargoJobs','12','-Profile','verification','-Filter',$Filter,'-Run','-NoCapture')
  & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package $Package -CargoJobs 12 -Profile verification -Filter $Filter -Run -NoCapture *> (Join-Path $evidence "$Case.log")
  if($reviewField -or $arityGroup -or $boundaryGroup -or $preservationCompletion){
   $matchingLog=Get-Content -LiteralPath (Join-Path $evidence "$Case.log") -Raw
   . (Join-Path $PSScriptRoot 'migration-s5-l1-test-result.ps1')
   if(-not (Test-KilogramS5SingleTestLog $matchingLog $Filter)){throw 'Review2 requires exactly one matching test, one one-test PASS summary and wrapper completion.'}
  }
 } elseif($Action -eq 'ColdStatus') {
  if(-not $FixtureDirectory){throw 'Cold diagnostic requires the stopped active branch.'}
  $argv=@()
  foreach($side in @('one','two')) {
   $profile=Get-Content -LiteralPath (Join-Path $FixtureDirectory "$side/indexed.profile") -Raw|ConvertFrom-Json
   $args=@('ordered','storage-upgrade','--check','--state-dir',[string]$profile.state_dir,'--expect-account',[string]$profile.account,'--expect-device',[string]$profile.device,'--config-file',(Join-Path $FixtureDirectory "$side/migration.json"))
   $argv+=,@($args)
   & (Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe') @args *> (Join-Path $evidence "$Case-$side-status.json")
   if($LASTEXITCODE -ne 0){throw 'Cold diagnostic failed.'}
  }
  'Completed stopped-branch public status diagnostics.'|Set-Content -LiteralPath (Join-Path $evidence "$Case.log")
 } else {
  $fixture=Join-Path $workspace ".tmp/migration-s5-l1/$Case"
  $countA=if($Action -eq 'ExhaustedOrigin'){511}else{4}
  $countB=if($Action -eq 'ExhaustedOrigin'){16}else{4}
  $argv=@('scripts/test-ordered-dialog-process.ps1','-Executable','target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe','-RunDirectory',$fixture,'-Scenario','Duplex','-MessagesA',"$countA",'-MessagesB',"$countB",'-Burst','16')
  & (Join-Path $PSScriptRoot 'test-ordered-dialog-process.ps1') -Executable 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe' -RunDirectory $fixture -Scenario Duplex -MessagesA $countA -MessagesB $countB -Burst 16 -DeadlineGuard $deadline *> (Join-Path $evidence "$Case.log")
 }
 foreach($item in @($sources)+@($images)){
  if((Get-FileHash -LiteralPath (Join-Path $workspace $item.path) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $item.sha256){throw "Source/image changed during contribution: $($item.path)"}
 }
} catch {$code=1;$failure=$_.Exception.Message;$_|Out-String|Add-Content -LiteralPath (Join-Path $evidence "$Case.log")}
finally {
 $env:CARGO_NET_OFFLINE=$priorOffline;$env:KILOGRAM_S5_FIXTURE_DIR=$priorFixture;$env:KILOGRAM_S5_L1_RESUME_DIR=$priorResume;$env:KILOGRAM_S5_CASE=$priorCase;$env:KILOGRAM_S5_POWERSHELL=$priorPowerShell
 if($deadline){$deadline.CleanupChildren(5000);if($deadline.WorkExpired){$code=1;$failure='Whole-attempt work cutoff reached; owned children terminated.'}}
}
Get-Content -LiteralPath (Join-Path $evidence "$Case.log") -Tail 50
$timer.Stop()
$record=[ordered]@{case=$Case;contributing_set=$Set;action=$Action;argv=$argv;fixture=$fixture;fixture_input=$FixtureDirectory;resume_active_input=$ResumeDirectory;exit_code=$code;error=$failure;elapsed_seconds=$timer.Elapsed.TotalSeconds;sources=$sources;images=$images;classification='whole-contributing-runner';features=@();rule='Includes all runner setup, cached compile/stage, real fixture creation/copy, waits, recovery, kill and cleanup; failures retained.'}
if($preservationCompletion){$record['preservation_completion']=[ordered]@{root_decision=$decision;root_go_ahead=$LongApprovalFile;root_go_ahead_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $LongApprovalFile).Hash.ToLowerInvariant();execution_mode_declared=$ExecutionMode;approval_provenance='separate root technical go-ahead and ordinary tool execution review; mode is a declaration, not token introspection';scope=$(if($phasePreservationC){'opaque actual Phases/helper regression with two real held locks'}else{'stopped case03 preservation only, no typed opens/runtime/recovery/restore'});boundary03_whole_result='FAIL';lost_parent_result_reconstructed=$false}}
if($boundaryGroup -or $Action -eq 'ExhaustedOrigin'){$record['long_approval']=[ordered]@{path=$LongApprovalFile;sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $LongApprovalFile).Hash.ToLowerInvariant();execution_mode=$ExecutionMode;approval_provenance='require_escalated tool invocation and separate root go-ahead; declaration is not token introspection';small_evidence_acceptance='Ready02 and capacity01 remain whole FAIL, scoped functional acceptance only under continuation3/4'}}
if($readyException){$record['reviewed_predecessor']=[ordered]@{case='F-review1-ready-02';whole_result='FAIL';scope='Root accepted completed functional R3 assertions only';decision_sha256='c05a408a653f9ac7d35fffd31cd50df15620b726e5357013d8525ea9e669bf02';case_sha256='20540287fc8e478ba72bdfc49786f8b6c6a90e02b298ce3b1fdef7745278e7be';log_sha256='c09dfa264573ff3873f8bca52998dadcff9d22a52748b59451d9c32ebb52f463';parent_sha256='fcfb7da3b292b8427f99f49e20fa55a067cc0a727f7936900179520e3edcccf0'}}
if($reviewField){$record['execution_mode']=[ordered]@{declared=$ExecutionMode;approval_provenance='require_escalated tool invocation, independently recorded by the execution approval layer; declaration is not token introspection';prior_failed_case='F-review1-own-f-01';review2_sha256='d1fe3ec11a5e9c36d38f0ca874bef609649a30ba70fdcd114e2ee1f7de9550ad';continuation_sha256='8dde9b2b0cc26de3beb2dc94800a6d628b2431879bb27e33c61933c905efaa40';ready_fixture_continuation_sha256='28b1385688e394a8219fea1ee576cf6d244e8318c1186bdb1ff9c53759d3bb21'}}
if($deadline){$record['deadline']=[ordered]@{work_seconds=$work;whole_seconds=$whole;work_expired=$deadline.WorkExpired;owned_children_after_cleanup=@($deadline.Children()).Count;root_decision=$decision;mechanism='non-breakaway self-owned Windows job; independent native whole-deadline thread remains armed through finalization'}}
$record|ConvertTo-Json -Depth 9|Set-Content -LiteralPath $recordPath -Encoding utf8
$ledger.cases=@($ledger.cases)+@($record)
$ledger.charged_elapsed_seconds.$Set=[double]$ledger.charged_elapsed_seconds.$Set+$timer.Elapsed.TotalSeconds
$ledger|ConvertTo-Json -Depth 13|Set-Content -LiteralPath $ledgerPath -Encoding utf8
if($deadline){$deadline.Finish()}
if([double]$ledger.charged_elapsed_seconds.$Set -gt [double]$ledger.new_ceilings_seconds.$Set){throw 'Contributing ceiling exceeded; retain and callback root.'}
exit $code
