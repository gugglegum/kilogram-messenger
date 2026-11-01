[CmdletBinding()]
param([Parameter(Mandatory)][string]$CompilerCapture,[Parameter(Mandatory)][string]$ShellExecutable,[Parameter(Mandatory)][string]$MemoryProbeFile)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
Set-Location $workspace
. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$null=Set-KilogramCargoResourcePolicy -RequestedJobs 2
$env:CARGO_NET_OFFLINE='true'
. (Join-Path $PSScriptRoot 'cargo-frozen-artifacts.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
function Require([bool]$Value,[string]$Why){if(-not $Value){throw $Why}}
function Row([string]$Path){
    $full=[IO.Path]::GetFullPath($Path,$workspace)
    Require ($full.StartsWith($workspace+'\',[StringComparison]::OrdinalIgnoreCase)) 'G4 source/image must stay in workspace.'
    [ordered]@{path=[IO.Path]::GetRelativePath($workspace,$full).Replace('\','/');bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-KilogramFrozenHash $full)}
}
$expectedPath=Join-Path $evidence 'G4-01.expected-artifacts.candidate.json'
$imagePath=Join-Path $evidence 'G4-01.images.candidate.json'
$cases=@('F-s53-fresh-regression-01','F-s53-fresh-history-regression-01')
foreach($p in @($expectedPath,$imagePath)+@($cases|ForEach-Object {Join-Path $evidence (Get-KilogramS53BindingName $_)})){Require (-not(Test-Path -LiteralPath $p)) 'Immutable new G4 preparation required.'}
Require (@(Get-Process -ErrorAction Stop|Where-Object ProcessName -Like 'kilogram-*').Count -eq 0) 'No live Kilogram before fixed runtime CLI copy.'
$shell=Get-KilogramS53ShellDependency -Executable $ShellExecutable -RequireCurrentHost
$probe=Get-Content -LiteralPath $MemoryProbeFile -Raw|ConvertFrom-Json
Require ($probe.actual_exit_code -eq 0 -and $probe.cleanup_verified -and $probe.owned_children_after_cleanup -eq 0 -and $probe.job_membership_verified -and $probe.test_selectors_executed -eq 0) 'Successful bounded non-network memory-shell probe required.'
$deps=@($shell,(Get-KilogramS53MemoryShellDependency $workspace))
Assert-KilogramS53RuntimeDependencies $cases[0] $deps @($probe.shell,$probe.memory_shell)
$head=(& git rev-parse HEAD).Trim();Require ($LASTEXITCODE -eq 0 -and $head -cmatch '^[0-9a-f]{40}$') 'Actual current source HEAD.'
$capture=Get-Content -LiteralPath $CompilerCapture -Raw|ConvertFrom-Json -AsHashtable
Require ($capture.classification -ceq 'nonexecuting-compiler-artifact-provenance') 'Actual nonexecuting compiler capture required.'
$inputs=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $capture.build_inputs_before $inputs;Assert-KilogramFrozenInputs $capture.build_inputs_after $inputs
$normal=@($capture.units|Where-Object {$_.package -ceq 'kilogram-cli' -and $_.target -ceq 'kilogram-cli' -and $_.profile.test -eq $false -and $_.executable})
$normalPath=Join-Path $workspace 'target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe'
Require ($normal.Count -eq 1 -and $normal[0].kind.Count -eq 1 -and $normal[0].kind[0] -ceq 'bin' -and $normal[0].profile.opt_level -ceq '3' -and $normal[0].profile.debug_assertions -and $normal[0].profile.overflow_checks -and $normal[0].executable -ceq $normalPath.Replace('/','\')) 'Actual normal compiler/profile/output.'
$destination=Join-Path $workspace 'target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe'
$before=Row $destination;$source=Row $normalPath
$preimages=Join-Path $workspace '.tmp/g4-preparation-01/before.json'
$pre=Get-Content -LiteralPath $preimages -Raw|ConvertFrom-Json
$old=@($pre.rows|Where-Object path -CEQ 'target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe')
Require ($old.Count -eq 1 -and $old[0].bytes -eq $before.bytes -and $old[0].sha256 -ceq $before.sha256 -and (Get-KilogramFrozenHash (Join-Path $workspace $old[0].archive)) -ceq $before.sha256) 'Accepted runtime CLI preimage archived before replacement.'
if($before.sha256 -cne $source.sha256 -or $before.bytes -ne $source.bytes){Copy-Item -LiteralPath $normalPath -Destination $destination -Force}
$after=Row $destination;Require ($after.sha256 -ceq $source.sha256 -and $after.bytes -eq $source.bytes) 'Fixed fresh runtime CLI matches current normal compiler image.'
$expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile='verification-scale';target='x86_64-pc-windows-msvc';stable_directory=(Join-Path $workspace 'target/verification-scale/deps').Replace('/','\');build_inputs=$inputs;units_identity=@(Get-KilogramFrozenUnitIdentity $capture.units);artifacts=$capture.stable_artifacts}
Assert-KilogramS53G3RuntimeGraph $expected
$actual=@(Get-KilogramFrozenStableRows @($capture.stable_artifacts|ForEach-Object {[pscustomobject]@{Package=$_.package;Kind=$_.kind;Target=$_.target;Path=$_.path}}) 'verification-scale')
Assert-KilogramFrozenArtifacts $expected $actual 'verification-scale' 'x86_64-pc-windows-msvc' $expected.stable_directory 6
Write-KilogramFrozenJson $expectedPath $expected
Write-KilogramFrozenJson $imagePath ([ordered]@{classification='G4 current normal CLI compiler/fixed runtime path provenance; no regression executed';source_head=$head;activated=$false;compiler_capture=(Row $CompilerCapture);normal_unit=$normal[0];source=$source;before=$before;archive=$old[0];after=$after;memory_probe=(Row $MemoryProbeFile);runtime_dependencies=$deps;preimages=(Row $preimages)})
$sources=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
foreach($p in $inputs.source_inputs.path){$sources[$p]=Row $p}
foreach($p in @('scripts/prepare-migration-s53-g4.ps1','scripts/test-migration-s53-g4-policy.ps1','scripts/probe-migration-s53-g4-memory-shell.ps1','scripts/invoke-migration-s53-integration-case.ps1','scripts/invoke-migration-s53-integration-outer.ps1','scripts/migration-s53-metadata.ps1','scripts/migration-s53-runtime-dependencies.ps1','scripts/migration-s53-predecessor-proof.ps1','scripts/migration-s5-l1-deadline.ps1',$CompilerCapture,$imagePath,$MemoryProbeFile,$preimages)){$r=Row $p;$sources[$r.path]=$r}
foreach($case in $cases){
    $p=Get-KilogramS53G4Policy $case
    $images=@(Get-KilogramS53ImageRoles $case|ForEach-Object {Row $_});Assert-KilogramS53ImageRoles $case $images
    $binding=[ordered]@{classification='g4-current-fresh-regression-candidate';revision='G4-preparation-01';baseline='64c2359788ead360314793e47741adacb83b8d4f';source_head=$head;inputs=@($sources.Keys|Sort-Object -CaseSensitive|ForEach-Object {$sources[$_]});code_source_count=$inputs.source_inputs.Count;images=$images;runtime_dependencies=$deps;runtime_graph=@{packages=@('kilogram-cli','kilogram-transport-iroh');harness_count=6;profile='verification-scale';target='x86_64-pc-windows-msvc'};cases=@($p);expected_artifacts=(Row $expectedPath);normal_cli_provenance=(Row $imagePath);activated=$false;proposed_only=$true;scope='One existing fresh S4 selector, native marker/log assertions and outer cleanup; zero prerequisites, no relay.'}
    Assert-KilogramS53G4Policy $case $binding
    Write-KilogramFrozenJson (Join-Path $evidence (Get-KilogramS53BindingName $case)) $binding
}
$post=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $inputs $post
Row $expectedPath;Row $imagePath
foreach($case in $cases){Row (Join-Path $evidence (Get-KilogramS53BindingName $case))}
