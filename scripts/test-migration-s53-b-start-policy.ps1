[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
$case='F-s53-b-before-alpn2-two-c-01'
$p=[ordered]@{case=$case;selector='live_l1::handoff::s53_b_before_alpn2_two_c_public';expected_count=1;build_profile='verification-scale';image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';controller_seconds=480;work_seconds=510;whole_seconds=540}
$binding=[ordered]@{classification='b-start-only-current-two-context-candidate';baseline='64c2359788ead360314793e47741adacb83b8d4f';source_head='5f31239ecc3be0eace190db8362c310a99d7e53f';cases=@($p);runtime_graph=@{target='x86_64-pc-windows-msvc';profile='verification-scale';harness_count=6;packages=@('kilogram-cli','kilogram-transport-iroh')};expected_artifacts=@{path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/B-start-02.expected-artifacts.candidate.json';sha256='a'*64}}
Assert-KilogramS53G3Policy $case $binding
if((Get-KilogramS53BindingName $case) -cne 'B-start-02.binding.candidate.json'){throw 'Exact B-start binding path.'}
if(@(Get-KilogramS53ImageRoles $case).Count -ne 9){throw 'B-start requires nine roles and no relay.'}
$count=3
foreach($change in @(
    @{field='selector';value='live_l1::handoff::s53_two_c_pin_resources_routes_public'},
    @{field='controller_seconds';value=481},@{field='work_seconds';value=511},@{field='whole_seconds';value=541},
    @{field='expected_count';value=2},@{field='build_profile';value='verification'}
)){
    $bad=$binding|ConvertTo-Json -Depth 10|ConvertFrom-Json -AsHashtable
    $bad.cases[0][$change.field]=$change.value
    $rejected=$false
    try{Assert-KilogramS53G3Policy $case $bad}catch{$rejected=$true}
    if(-not $rejected){throw ('Accepted changed B-start '+$change.field)}
    $count++
}
$rejected=$false
try{Assert-KilogramS53G3Policy 'F-s53-discovery-integrated-01' $binding}catch{$rejected=$true}
if(-not $rejected){throw 'B-start binding authorized another case.'};$count++
# Existing G3 policy remains distinct, including its original finite limits.
$g3=$binding|ConvertTo-Json -Depth 10|ConvertFrom-Json -AsHashtable
$g3.classification='g3-only-current-two-context-routes-candidate'
$g3.cases[0].case='F-s53-two-c-routes-integrated-01'
$g3.cases[0].selector='live_l1::handoff::s53_two_c_pin_resources_routes_public'
$g3.cases[0].controller_seconds=420;$g3.cases[0].work_seconds=480;$g3.cases[0].whole_seconds=510
$g3.expected_artifacts.path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/G3-two-context-routes-03.expected-artifacts.candidate.json'
Assert-KilogramS53G3Policy 'F-s53-two-c-routes-integrated-01' $g3
$count++
Write-Output "B-start pure policy checks PASS: $count; no runtime activation or process case."

$stale=$binding|ConvertTo-Json -Depth 10|ConvertFrom-Json -AsHashtable
$stale.expected_artifacts.path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/B-start-01.expected-artifacts.candidate.json'
$rejected=$false
try{Assert-KilogramS53G3Policy $case $stale}catch{$rejected=$true}
if(-not $rejected){throw 'Revision01 expected artifact path accepted for revision02.'}
Write-Output 'Revision02 stale-artifact guard PASS (12 total policy checks).'
