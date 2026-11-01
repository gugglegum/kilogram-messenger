[CmdletBinding()]
param([Parameter(Mandatory)][string]$CompilerCapture,[Parameter(Mandatory)][string]$ShellExecutable)
# Compile-only provenance -> inactive binding. Never activates or runs a case.
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
$case='F-s53-b-before-alpn2-two-c-01'
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
function Require([bool]$Value,[string]$Why){if(-not $Value){throw $Why}}
function Row([string]$Path){
    $full=[IO.Path]::GetFullPath($Path,$workspace)
    Require ($full.StartsWith($workspace+'\',[StringComparison]::OrdinalIgnoreCase)) 'Binding path must stay in workspace.'
    [ordered]@{path=[IO.Path]::GetRelativePath($workspace,$full).Replace('\','/');bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-KilogramFrozenHash $full)}
}
$expectedPath=Join-Path $evidence 'B-start-02.expected-artifacts.candidate.json'
$bindingPath=Join-Path $evidence (Get-KilogramS53BindingName $case)
$imagePath=Join-Path $evidence 'B-start-02.images.candidate.json'
foreach($p in @($expectedPath,$bindingPath,$imagePath)){Require (-not(Test-Path -LiteralPath $p)) 'Preserve immutable prior preparation.'}
Require (@(Get-Process -ErrorAction Stop|Where-Object ProcessName -Like 'kilogram-*').Count -eq 0) 'No live Kilogram before fixed-path copy.'
$shell=Get-KilogramS53ShellDependency -Executable $ShellExecutable -RequireCurrentHost
$head=(& git rev-parse HEAD).Trim();Require ($LASTEXITCODE -eq 0 -and $head -cmatch '^[0-9a-f]{40}$') 'Actual HEAD.'
$capture=Get-Content -LiteralPath $CompilerCapture -Raw|ConvertFrom-Json -AsHashtable
Require ($capture.classification -ceq 'nonexecuting-compiler-artifact-provenance') 'Nonexecuting compiler capture required.'
$inputs=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $capture.build_inputs_before $inputs
Assert-KilogramFrozenInputs $capture.build_inputs_after $inputs
$normal=@($capture.units|Where-Object {$_.package -ceq 'kilogram-cli' -and $_.target -ceq 'kilogram-cli' -and $_.profile.test -eq $false -and $_.executable})
$normalPath=Join-Path $workspace 'target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe'
Require ($normal.Count -eq 1 -and $normal[0].kind.Count -eq 1 -and $normal[0].kind[0] -ceq 'bin' -and $normal[0].profile.opt_level -ceq '3' -and $normal[0].profile.debug_assertions -and $normal[0].profile.overflow_checks -and $normal[0].executable -ceq $normalPath.Replace('/','\')) 'Actual normal CLI compiler/profile/output.'
$destination=Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe'
$before=Row $destination;$source=Row $normalPath
$archive=Join-Path $workspace '.tmp/b-start-parser-repair02/preparation-old-cli.exe'
Require (-not(Test-Path -LiteralPath $archive)) 'Fresh preimage required.'
Copy-Item -LiteralPath $destination -Destination $archive
Require ((Get-KilogramFrozenHash $archive) -ceq $before.sha256) 'Preimage copy verified.'
if($before.sha256 -cne $source.sha256 -or $before.bytes -ne $source.bytes){Copy-Item -LiteralPath $normalPath -Destination $destination -Force}
$after=Row $destination
Require ($after.sha256 -ceq $source.sha256 -and $after.bytes -eq $source.bytes) 'Fixed CLI contents match actual compiler image.'
$expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile='verification-scale';target='x86_64-pc-windows-msvc';stable_directory=(Join-Path $workspace 'target/verification-scale/deps').Replace('/','\');build_inputs=$inputs;units_identity=@(Get-KilogramFrozenUnitIdentity $capture.units);artifacts=$capture.stable_artifacts}
Assert-KilogramS53G3RuntimeGraph $expected
Assert-KilogramFrozenArtifacts $expected @(Get-KilogramFrozenStableRows @($capture.stable_artifacts|ForEach-Object {[pscustomobject]@{Package=$_.package;Kind=$_.kind;Target=$_.target;Path=$_.path}}) 'verification-scale') 'verification-scale' 'x86_64-pc-windows-msvc' $expected.stable_directory 6
Write-KilogramFrozenJson $expectedPath $expected
Write-KilogramFrozenJson $imagePath ([ordered]@{classification='B-start nonexecuting normal compiler/fixed image provenance';activated=$false;source_head=$head;capture=(Row $CompilerCapture);normal_unit=$normal[0];source=$source;before=$before;archive=(Row $archive);after=$after})
$sources=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
foreach($p in $inputs.source_inputs.path){$sources[$p]=Row $p}
foreach($p in @('scripts/prepare-migration-s53-b-start.ps1','scripts/test-migration-s53-b-start-policy.ps1','scripts/invoke-migration-s53-integration-case.ps1','scripts/invoke-migration-s53-integration-outer.ps1','scripts/migration-s53-metadata.ps1','scripts/migration-s53-runtime-dependencies.ps1','scripts/migration-s53-predecessor-proof.ps1','scripts/migration-s5-l1-deadline.ps1',$CompilerCapture,$imagePath,(Join-Path $workspace '.tmp/b-start-parser-repair02/before.json'))){$r=Row $p;$sources[$r.path]=$r}
$images=@(Get-KilogramS53ImageRoles $case|ForEach-Object {Row $_});Assert-KilogramS53ImageRoles $case $images
$policy=[ordered]@{case=$case;selector='live_l1::handoff::s53_b_before_alpn2_two_c_public';expected_count=1;image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';source='apps/kilogram-cli/tests/support/migration_s53_b_start.rs';controller_seconds=480;work_seconds=510;whole_seconds=540;package='kilogram-cli';assertion_envelope=$true;loss=$null;activated=$false;build_profile='verification-scale'}
$binding=[ordered]@{classification='b-start-only-current-two-context-candidate';revision='B-start-parser-repair-02';baseline='64c2359788ead360314793e47741adacb83b8d4f';source_head=$head;inputs=@($sources.Keys|Sort-Object -CaseSensitive|ForEach-Object {$sources[$_]});code_source_count=$inputs.source_inputs.Count;images=$images;runtime_dependencies=@($shell);runtime_graph=@{packages=@('kilogram-cli','kilogram-transport-iroh');harness_count=6;profile='verification-scale';target='x86_64-pc-windows-msvc'};cases=@($policy);expected_artifacts=(Row $expectedPath);normal_cli_provenance=(Row $imagePath);activated=$false;proposed_only=$true;scope='One controlled B-start startup case only, zero prerequisites; root review/activation required.'}
Assert-KilogramS53G3Policy $case $binding
$post=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $inputs $post
Write-KilogramFrozenJson $bindingPath $binding
Row $expectedPath;Row $imagePath;Row $bindingPath
