[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
$workspace=Split-Path -Parent $PSScriptRoot
$checks=0
foreach($case in @('F-s53-fresh-regression-01','F-s53-fresh-history-regression-01')){
    $p=Get-KilogramS53G4Policy $case
    $shell=@{role='metadata-shell';path='C:\reviewed\pwsh.exe';bytes=1;sha256='a'*64}
    $memory=@{role='runtime2-memory-shell';path='C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe';bytes=2;sha256='b'*64}
    $deps=@($shell,$memory)
    $binding=@{classification='g4-current-fresh-regression-candidate';baseline='64c2359788ead360314793e47741adacb83b8d4f';source_head='8b3237a593c529d55af9a556fa44ad3f7a704a7c';cases=@($p);runtime_dependencies=$deps;runtime_graph=@{packages=@('kilogram-cli','kilogram-transport-iroh');harness_count=6;profile='verification-scale';target='x86_64-pc-windows-msvc'};expected_artifacts=@{path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/G4-01.expected-artifacts.candidate.json';sha256='a'*64};inputs=@(@{path='apps/kilogram-cli/tests/runtime2_process.rs';bytes=1;sha256='a'*64});code_source_count=1;images=@(Get-KilogramS53ImageRoles $case|ForEach-Object {@{path=$_;bytes=1;sha256='a'*64}})}
    Assert-KilogramS53G3Policy $case $binding
    if($binding.images.Count -ne 8 -or @($binding.images|Where-Object path -Like '*relay.exe').Count){throw 'G4 reachable image roles differ.'};$checks++
    $inventoryArguments=@{Case=$case;Attempt=($case+'-attempt-01');Workspace=$workspace;Binding=$binding;BindingHash=('c'*64);ActivationSha256=('0'*64);Shell=$deps;Predecessors=@();Prerequisites=@()}
    $envelope=New-KilogramS53InventoryEnvelope @inventoryArguments
    if($envelope.Value.runtime_dependencies.Count -ne 2 -or $envelope.Value.images.Count -ne 8){throw 'Complete G4 arrays lost.'};$checks++
    Assert-KilogramS53RuntimeDependencies $case $deps $deps;$checks++
    foreach($mutation in @(@{field='selector';value='wrong'},@{field='work_seconds';value=999},@{field='assertion_envelope';value=$true},@{field='native_marker';value='wrong'})){
        $bad=$binding|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable;$bad.cases[0][$mutation.field]=$mutation.value
        $rejected=$false;try{Assert-KilogramS53G3Policy $case $bad}catch{$rejected=$true}
        if(-not $rejected){throw ('Accepted wrong G4 '+$mutation.field)};$checks++
    }
    foreach($badDeps in @(@($shell),@($memory,$shell),@($shell,$memory,$memory))){
        $bad=$inventoryArguments.Clone();$bad.Shell=$badDeps
        $rejected=$false;try{$null=New-KilogramS53InventoryEnvelope @bad}catch{$rejected=$true}
        if(-not $rejected){throw 'Accepted incomplete/reordered/extra runtime dependency.'};$checks++
    }
    $badMemory=$memory.Clone();$badMemory.sha256='d'*64
    $rejected=$false;try{Assert-KilogramS53RuntimeDependencies $case @($shell,$badMemory) $deps}catch{$rejected=$true}
    if(-not $rejected){throw 'Accepted memory-shell drift.'};$checks++
}
if(@(Get-KilogramS53RuntimeRoles 'F-s53-b-before-alpn2-two-c-01').Count -ne 1){throw 'Legacy dependency count changed.'};$checks++
$legacy=$binding|ConvertTo-Json -Depth 12|ConvertFrom-Json -AsHashtable
$legacyCase='F-s53-b-before-alpn2-two-c-01'
$legacy.classification='b-start-only-current-two-context-candidate'
$legacy.cases=@(@{case=$legacyCase;selector='live_l1::handoff::s53_b_before_alpn2_two_c_public';expected_count=1;build_profile='verification-scale';package='kilogram-cli';image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';controller_seconds=480;work_seconds=510;whole_seconds=540})
$legacy.expected_artifacts.path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/B-start-02.expected-artifacts.candidate.json'
$legacy.runtime_dependencies=@($shell)
$legacy.images=@(Get-KilogramS53ImageRoles $legacyCase|ForEach-Object {@{path=$_;bytes=1;sha256='a'*64}})
$legacyArgs=$inventoryArguments.Clone();$legacyArgs.Case=$legacyCase;$legacyArgs.Attempt=$legacyCase+'-attempt-03';$legacyArgs.Binding=$legacy;$legacyArgs.Shell=$shell
$legacyEnvelope=New-KilogramS53InventoryEnvelope @legacyArgs
if($legacyEnvelope.Value.runtime_dependencies.Count -ne 1){throw 'Legacy single-shell envelope changed.'};$checks++
$legacyArgs.Shell=$deps
$rejected=$false;try{$null=New-KilogramS53InventoryEnvelope @legacyArgs}catch{$rejected=$true}
if(-not $rejected){throw 'Legacy envelope accepted extra G4 dependency.'};$checks++
Write-Output "G4 pure policy/full-inventory checks PASS: $checks; no runtime test."
