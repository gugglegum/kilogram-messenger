# G3 preparation only: pure guards, nonexecuting builds/copies and inactive freeze.
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('Guard','Build','Freeze','CandidateGuard')][string]$Mode,[Parameter(Mandatory)][ValidatePattern('^G3-prep-[a-z0-9-]+$')][string]$Prefix)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$watch=[Diagnostics.Stopwatch]::StartNew()
$workspace=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $workspace
$out=Join-Path $workspace '.tmp/g3repair-03'
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
$g3Case='F-s53-two-c-routes-integrated-01'
$baseline='64c2359788ead360314793e47741adacb83b8d4f'
$target='x86_64-pc-windows-msvc';$buildProfile='verification-scale'
. (Join-Path $PSScriptRoot 'cargo-frozen-artifacts.ps1')
. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
$null=Set-KilogramCargoResourcePolicy -RequestedJobs 2
$env:CARGO_NET_OFFLINE='true'
function Require([bool]$Value,[string]$Why){if(-not $Value){throw $Why}}
function Row([string]$Path){
    $full=[IO.Path]::GetFullPath($Path,$workspace)
    Require ($full.StartsWith($workspace+'\',[StringComparison]::OrdinalIgnoreCase)) 'Public preparation path must remain inside workspace.'
    [ordered]@{path=[IO.Path]::GetRelativePath($workspace,$full).Replace('\','/');bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-KilogramFrozenHash $full)}
}
function Write-G3([string]$Name,$Value){$path=Join-Path $evidence $Name;Write-KilogramFrozenJson $path $Value;Row $path}
function Policy {
    [ordered]@{case=$g3Case;selector='live_l1::handoff::s53_two_c_pin_resources_routes_public';expected_count=1;image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';source='apps/kilogram-cli/tests/support/migration_s53_two_context.rs';controller_seconds=420;work_seconds=480;whole_seconds=510;package='kilogram-cli';assertion_envelope=$true;loss=$null;activated=$false;build_profile=$buildProfile}
}
function Graph { [ordered]@{packages=@('kilogram-cli','kilogram-transport-iroh');harness_count=6;profile=$buildProfile;target=$target} }
function Inputs {Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile $buildProfile -Target $target -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()}
function Preparation-Sources {
    @('prepare-migration-s53-g3.ps1','invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1')|ForEach-Object {Row ('scripts/'+$_)}
}
function Accepted-Sources {
    $before=Get-Content (Join-Path $out 'before.json') -Raw|ConvertFrom-Json -AsHashtable
    $current=Get-Content (Join-Path $out 'current-source-pins-03.json') -Raw|ConvertFrom-Json -AsHashtable
    foreach($pin in $current.source){$actual=Row $pin.path;Require ($actual.bytes -eq $pin.bytes -and $actual.sha256 -ceq $pin.sha256) 'Reviewed repair/current preparation source changed.'}
    foreach($pin in $before.protected){$actual=Row $pin.path;Require ($actual.bytes -eq $pin.bytes -and $actual.sha256 -ceq $pin.sha256) 'Unrelated dirty source changed.'}
    foreach($entry in $before.preimages){$actual=Row $entry.archive.path;Require ($actual.bytes -eq $entry.original.bytes -and $actual.sha256 -ceq $entry.original.sha256) 'Preparation preimage changed.'}
    return $before
}
function Copy-Proven([string]$Source,[string]$Destination,[string]$Archive) {
    $processes=@(Get-Process -ErrorAction Stop|Where-Object ProcessName -Like 'kilogram-*')
    Require ($processes.Count -eq 0) 'Live Kilogram process prevents fixed-path recopy.'
    $before=Row $Destination;$archivePath=Join-Path $out $Archive
    Require (-not (Test-Path -LiteralPath $archivePath)) 'Old image archive already exists.'
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($archivePath))
    Copy-Item -LiteralPath $Destination -Destination $archivePath
    Require ((Get-KilogramFrozenHash $archivePath) -ceq $before.sha256) 'Old image archive mismatch.'
    $sourceRow=Row $Source
    $copied=$sourceRow.bytes -ne $before.bytes -or $sourceRow.sha256 -cne $before.sha256
    if($copied){Copy-Item -LiteralPath $Source -Destination $Destination -Force}
    $after=Row $Destination
    Require ($sourceRow.bytes -eq $after.bytes -and $sourceRow.sha256 -ceq $after.sha256) 'Actual fixed image copy mismatch.'
    [ordered]@{source=$sourceRow;before=$before;archive=(Row $archivePath);after=$after;live_kilogram_processes_before_copy=$processes.Count;copied=$copied;reused_identical=(-not $copied)}
}
$shellPath=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
$shell=Get-KilogramS53ShellDependency -Executable $shellPath -RequireCurrentHost
$head=(& git rev-parse HEAD).Trim();Require ($LASTEXITCODE -eq 0 -and $head -ceq '9aa47d938af990c2cb61ea6c2ffe8dc1db02a9fa') 'Reviewed current HEAD required.'
$accepted=Accepted-Sources

if($Mode -ceq 'Guard'){
    $checks=[Collections.Generic.List[object]]::new()
    function Pass([string]$Name,$Detail=$null){$checks.Add(@{name=$Name;pass=$true;detail=$Detail})}
    function Refuses([string]$Name,[scriptblock]$Action){
        $refused=$false;$reason=$null
        try{& $Action|Out-Null}catch{$refused=$true;$reason=$_.Exception.Message}
        Require $refused ('Real guard accepted negative '+$Name)
        $checks.Add(@{name=$Name;pass=$true;refused=$true;reason=$reason})
    }
    foreach($name in @('invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1','prepare-migration-s53-g3.ps1')){
        $tokens=$null;$errors=$null
        $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
        Require ($errors.Count -eq 0) ('PowerShell parse: '+$name)
        if($name -like 'invoke-*'){
            foreach($command in @('Get-KilogramS53BindingName','Assert-KilogramS53G3FrozenAllocation')){Require (@($ast.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq $command},$true)).Count -eq 1) ('Actual wrapper uses common '+$command)}
        }
        Pass ('parse-'+$name)
    }
    $expectedPath='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/G3-two-context-routes-03.expected-artifacts.candidate.json'
    function Fixture([string]$Case){
        $p=if($Case -ceq $g3Case){Policy}elseif($Case -ceq 'F-s53-capacity-recovery-r2-01'){
            @{case=$Case;selector='live_l1::handoff::capacity_review2::s53_capacity_recovery_preserves_validated_route_public';expected_count=1;build_profile=$buildProfile;image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';controller_seconds=240;work_seconds=275;whole_seconds=300}
        }else{@{case=$Case}}
        [ordered]@{classification=if($Case -ceq $g3Case){'g3-only-current-two-context-routes-candidate'}elseif($Case -ceq 'F-s53-capacity-recovery-r2-01'){'g2-only-current-capacity-route-candidate'}else{'synthetic-nine-role-guard'};baseline=$baseline;source_head=$head;inputs=@(@{path='guard-source.txt';bytes=1;sha256=('0'*64)});code_source_count=1;images=@(Get-KilogramS53ImageRoles $Case|ForEach-Object {@{path=$_;bytes=1;sha256=('0'*64)}});runtime_dependencies=@($shell);runtime_graph=(Graph);cases=@($p);expected_artifacts=@{path=$expectedPath;bytes=1;sha256=('0'*64)}}
    }
    function Envelope([string]$Name,[string]$Case,$Binding){
        New-KilogramS53InventoryEnvelope -Case $Case -Attempt ($Case+'-attempt-01') -Workspace (Join-Path $out ('guards/'+$Prefix+'/'+$Name)) -Binding $Binding -BindingHash ('1'*64) -ActivationSha256 ('2'*64) -Shell $shell -Predecessors @() -Prerequisites @()
    }
    function Save-Envelope($Envelope){
        [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Envelope.InventoryPath))
        $file=[IO.File]::Open($Envelope.InventoryPath,[IO.FileMode]::CreateNew);try{$file.Write($Envelope.Bytes)}finally{$file.Dispose()}
        Assert-KilogramS53WrittenInventory -Path $Envelope.InventoryPath -Envelope $Envelope
    }
    function Reencode($Envelope){$Envelope.Bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Envelope.Value -Depth 30));$Envelope.ByteCount=$Envelope.Bytes.LongLength}
    $old='F-s53-committed-before-publication-01';$g2='F-s53-capacity-recovery-r2-01';$g1='F-s53-exhausted-integrated-01'
    foreach($pair in @(@($g3Case,'G3-two-context-routes-03.binding.candidate.json'),@($g2,'G2-capacity-route-02.binding.candidate.json'),@($g1,'C-s53-g1-continuation-C2-01.binding.candidate.json'),@($old,'S5.3-integration-binding-r30.json'))){Require ((Get-KilogramS53BindingName $pair[0]) -ceq $pair[1]) 'Exact G3/G2/G1/legacy route';Pass ('route-'+$pair[0])}
    foreach($c in @($g3Case,$g2,$old)){$env=Envelope ('positive-'+$c) $c (Fixture $c);$saved=Save-Envelope $env;Require ($saved.ByteCount -eq $env.ByteCount) 'Real written positive';Pass ('builder-and-written-'+$c) $env.Value.images.Count}
    # G1 deliberately still requires its one existing public prerequisite.
    $b=Fixture $g1;Refuses 'G1-missing-required-prerequisite' {Envelope 'g1-missing' $g1 $b}
    $b=Fixture $g1;$env=New-KilogramS53InventoryEnvelope -Case $g1 -Attempt ($g1+'-attempt-01') -Workspace (Join-Path $out ('guards/'+$Prefix+'/g1-positive')) -Binding $b -BindingHash ('1'*64) -ActivationSha256 ('2'*64) -Shell $shell -Predecessors @() -Prerequisites @(@{path='public.json';bytes=1;sha256=('0'*64);classification='root-allocated public replica manifest; no preparation performed by adapter'});$null=Save-Envelope $env;Pass 'G1-one-public-prerequisite-retained'
    foreach($mutation in @('missing-relay','missing-harness','extra','duplicate')){
        $b=Fixture $g3Case
        switch($mutation){
            'missing-relay' {$b.images=@($b.images|Where-Object path -NotLike '*kilogram-relay.exe')}
            'missing-harness' {$b.images=@($b.images|Where-Object path -NotLike '*local_relay_transport-tests.exe')}
            'extra' {$b.images+=@{path='target/extra.exe';bytes=1;sha256=('0'*64)}}
            'duplicate' {$b.images[9]=$b.images[0]}
        }
        Refuses ('builder-'+$mutation) {Envelope ('builder-'+$mutation) $g3Case $b}
        $env=Envelope ('written-'+$mutation) $g3Case (Fixture $g3Case);$env.Value.images=$b.images;Reencode $env
        Refuses ('written-'+$mutation) {Save-Envelope $env}
    }
    foreach($mutation in @('classification','selector','expected_count','source_head','case','extra-case','harness_count','packages','target','controller_seconds','expected-path')){
        $b=Fixture $g3Case
        switch($mutation){
            'classification' {$b.classification='g2-only-current-capacity-route-candidate'}
            'selector' {$b.cases[0].selector='wrong'}
            'expected_count' {$b.cases[0].expected_count=2}
            'source_head' {$b.source_head='not-a-revision'}
            'case' {$b.cases[0].case=$g2}
            'extra-case' {$b.cases+=@{case=$old}}
            'harness_count' {$b.runtime_graph.harness_count=9}
            'packages' {$b.runtime_graph.packages+=@('kilogram-state')}
            'target' {$b.runtime_graph.target='wrong'}
            'controller_seconds' {$b.cases[0].controller_seconds=440}
            'expected-path' {$b.expected_artifacts.path='wrong.json'}
        }
        Refuses ('policy-'+$mutation) {Envelope ('policy-'+$mutation) $g3Case $b}
    }
    $b=Fixture $g3Case;Refuses 'cross-case-G3-classification' {Envelope 'cross-case' $old $b}
    $b=Fixture $g3Case;$b.expected_artifacts.path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/G3-two-context-routes-01.expected-artifacts.candidate.json';Refuses 'previous-candidate01-expected-path' {Envelope 'previous-candidate01' $g3Case $b}
    $b=Fixture $g3Case;$b.expected_artifacts.path='ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/G3-two-context-routes-02.expected-artifacts.candidate.json';Refuses 'previous-candidate02-expected-path' {Envelope 'previous-candidate02' $g3Case $b}
    $b=Fixture $old;$b.cases[0].selector=(Policy).selector;Refuses 'cross-case-G3-selector' {Envelope 'cross-selector' $old $b}
    $b=Fixture $g3Case
    Refuses 'G3-prerequisite' {New-KilogramS53InventoryEnvelope -Case $g3Case -Attempt ($g3Case+'-attempt-01') -Workspace (Join-Path $out 'guards/prerequisite') -Binding $b -BindingHash ('1'*64) -ActivationSha256 ('2'*64) -Shell $shell -Predecessors @() -Prerequisites @(@{path='public.json';bytes=1;sha256=('0'*64);classification='root-allocated public replica manifest; no preparation performed by adapter'})}
    $env=Envelope 'written-prerequisite' $g3Case $b;$env.Value.prerequisites=@(@{path='public.json';bytes=1;sha256=('0'*64);classification='root-allocated public replica manifest; no preparation performed by adapter'});Reencode $env
    Refuses 'written-G3-prerequisite' {Save-Envelope $env}
    $matched=Read-KilogramS53CaseBaseline $workspace $g3Case $b;Require ($matched.status -ceq 'verified_match' -and $matched.cleanup_verified) 'Actual current HEAD guard match.'
    $b.source_head='0'*40;$changed=Read-KilogramS53CaseBaseline $workspace $g3Case $b;Require ($changed.status -ceq 'verified_changed' -and $changed.cleanup_verified) 'Actual wrong HEAD guard refusal.';Pass 'actual-current-and-wrong-HEAD' @{matched=$matched;changed=$changed}
    $m=@{profile=$buildProfile;target=$target;build_inputs=@{packages=@('kilogram-cli','kilogram-transport-iroh')};artifacts=@(Get-KilogramS53ImageRoles $g3Case|Where-Object {$_ -like 'target/verification-scale/deps/*'}|ForEach-Object {@{path=(Join-Path $workspace $_);profile=$buildProfile}})}
    Assert-KilogramS53G3RuntimeGraph $m;Pass 'six-harness-graph'
    $m.build_inputs.packages+=@('kilogram-state');Refuses 'nine-harness-package-selection' {Assert-KilogramS53G3RuntimeGraph $m}
    $m.build_inputs.packages=@('kilogram-cli','kilogram-transport-iroh');$m.artifacts+=@($m.artifacts[0],$m.artifacts[0],$m.artifacts[0]);Refuses 'nine-harness-artifact-selection' {Assert-KilogramS53G3RuntimeGraph $m}
    $nine=Join-Path $workspace '.tmp/g3b2-01/expected-01.json';Refuses 'actual-state-nine-harness-manifest' {Read-KilogramFrozenManifest $nine (Get-KilogramFrozenHash $nine) 6}
    Write-G3 ($Prefix+'.guards.json') @{classification='G3 real pure guards; synthetic public inventories only; no runtime admission';sources=@(Preparation-Sources);checks=$checks.ToArray();elapsed_seconds=$watch.Elapsed.TotalSeconds;activated=$false}
    exit 0
}

if($Mode -ceq 'CandidateGuard'){
    $bindingPath=Join-Path $evidence (Get-KilogramS53BindingName $g3Case)
    $binding=Get-Content $bindingPath -Raw|ConvertFrom-Json -AsHashtable
    $checks=[Collections.Generic.List[object]]::new()
    foreach($name in @('invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1')){
        $tokens=$null;$errors=$null
        $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
        $calls=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq 'Assert-KilogramS53G3FrozenAllocation'},$true))
        Require ($errors.Count -eq 0 -and $calls.Count -eq 1) 'One actual wrapper preflight required.'
        $actualCall=[scriptblock]::Create($calls[0].Extent.Text)
        foreach($mutation in @('positive','wrong-head','wrong-allocation','wrong-frozen-hash','nine-graph')){
            $binding=Get-Content $bindingPath -Raw|ConvertFrom-Json -AsHashtable
            $Case=$g3Case;$allocated=@{selector=(Policy).selector;build_profile=$buildProfile;work_seconds=480;whole_seconds=510};$allocation=@($allocated)
            $activation=@{expected_artifacts_sha256=$binding.expected_artifacts.sha256}
            switch($mutation){
                'wrong-head' {$binding.source_head='0'*40}
                'wrong-allocation' {$allocated.whole_seconds=511}
                'wrong-frozen-hash' {$activation.expected_artifacts_sha256='0'*64}
                'nine-graph' {$binding.runtime_graph.harness_count=9}
            }
            $refused=$false;$reason=$null
            try{& $actualCall}catch{$refused=$true;$reason=$_.Exception.Message}
            Require ($refused -eq ($mutation -cne 'positive')) ('Actual wrapper preflight result '+$name+'/'+$mutation)
            $checks.Add(@{name=($name+'/'+$mutation);pass=$true;refused=$refused;reason=$reason;actual_ast_command=$calls[0].Extent.Text})
        }
    }
    Write-G3 ($Prefix+'.candidate-guards.json') @{classification='G3 actual wrapper frozen preflight checks; no wrapper/process execution or runtime admission';binding=(Row $bindingPath);checks=$checks.ToArray();sources=@(Preparation-Sources);elapsed_seconds=$watch.Elapsed.TotalSeconds;activated=$false}
    exit 0
}

if($Mode -ceq 'Build'){
    $before=Inputs;$preparationBefore=@(Preparation-Sources);$legacyBefore=Row 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe'
    $arguments=@('build','--locked','--offline','--jobs','2','--target',$target,'--profile',$buildProfile,'-p','kilogram-transport-iroh','--example','s4_loopback_relay','--message-format=json-render-diagnostics')
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $rawPath=Join-Path $out ($Prefix+'.relay.compiler.jsonl');$errPath=Join-Path $out ($Prefix+'.relay.stderr.log')
    Require (-not(Test-Path $rawPath) -and -not(Test-Path $errPath)) 'Fresh relay build outputs required.'
    $compiler=@(& cargo @arguments 2> $errPath);$relayExit=$LASTEXITCODE;$timer.Stop()
    [IO.File]::WriteAllLines($rawPath,[string[]]$compiler,[Text.UTF8Encoding]::new($false))
    Require ($relayExit -eq 0) 'Relay compile failed; preserve raw compiler/stderr.'
    $units=@($compiler|ForEach-Object {try{$_|ConvertFrom-Json -AsHashtable}catch{}}|Where-Object {$_.reason -ceq 'compiler-artifact' -and $_.target.name -ceq 's4_loopback_relay' -and $_.executable})
    Require ($units.Count -eq 1 -and $units[0].target.kind.Count -eq 1 -and $units[0].target.kind[0] -ceq 'example' -and $units[0].profile.test -eq $false -and $units[0].profile.opt_level -ceq '3' -and $units[0].profile.debug_assertions -and $units[0].profile.overflow_checks) 'Actual normal relay compiler profile proof.'
    $package=@($before.package_names|Where-Object name -CEQ 'kilogram-transport-iroh')
    Require ($package.Count -eq 1 -and $units[0].package_id -ceq $package[0].id -and [IO.Path]::GetFullPath($units[0].executable) -ceq (Join-Path $workspace 'target/x86_64-pc-windows-msvc/verification-scale/examples/s4_loopback_relay.exe').Replace('/','\')) 'Exact relay compiler package/output proof.'
    $relayCopy=Copy-Proven $units[0].executable (Join-Path $workspace 'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe') ($Prefix+'/old-relay.exe')
    $relaySeconds=$timer.Elapsed.TotalSeconds
    $capturePath=Join-Path $out ($Prefix+'.compile-artifacts.json')
    $timer.Restart()
    & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -CargoJobs 2 -Profile $buildProfile -CargoTarget $target -StableArtifactsDirectory (Join-Path $workspace 'target/verification-scale/deps') -BuildEvidencePath $capturePath -SkipCacheStatus
    $compileExit=$LASTEXITCODE;$timer.Stop();Require ($compileExit -eq 0) 'Actual fresh six-harness compile-only failed.'
    $capture=Get-Content -LiteralPath $capturePath -Raw|ConvertFrom-Json -AsHashtable
    Assert-KilogramFrozenInputs $before $capture.build_inputs_before
    Assert-KilogramFrozenInputs $capture.build_inputs_before $capture.build_inputs_after
    $normal=@($capture.units|Where-Object {$_.package -ceq 'kilogram-cli' -and $_.target -ceq 'kilogram-cli' -and $_.profile.test -eq $false -and $_.executable})
    Require ($normal.Count -eq 1 -and $normal[0].kind.Count -eq 1 -and $normal[0].kind[0] -ceq 'bin' -and $normal[0].profile.opt_level -ceq '3' -and $normal[0].profile.debug_assertions -and $normal[0].profile.overflow_checks -and $normal[0].executable -ceq (Join-Path $workspace 'target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe').Replace('/','\')) 'Actual normal non-test CLI compiler/profile/output proof.'
    $normalCopy=Copy-Proven $normal[0].executable (Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe') ($Prefix+'/old-normal-cli.exe')
    $after=Inputs;Assert-KilogramFrozenInputs $before $after;Assert-KilogramFrozenInputs $preparationBefore @(Preparation-Sources)
    Require ((Get-KilogramFrozenHash (Join-Path $workspace $legacyBefore.path)) -ceq $legacyBefore.sha256) 'Frozen legacy CLI unchanged.'
    Require ((Get-KilogramFrozenHash (Join-Path $workspace $relayCopy.after.path)) -ceq $relayCopy.after.sha256) 'Fixed relay preserved across normal-graph example rebuild.'
    $null=Accepted-Sources
    Write-G3 ($Prefix+'.images.json') @{classification='G3 actual nonexecuting standalone relay build/copy and normal CLI compiler/copy provenance';activated=$false;sources=@(Preparation-Sources);source_head=$head;inputs_before=$before;inputs_after=$after;normal_cli_capture=(Row $capturePath);normal_cli_unit=$normal[0];normal_cli_copy=$normalCopy;relay_arguments=$arguments;relay_exit_code=$relayExit;relay_compile_seconds=$relaySeconds;normal_compile_seconds=$timer.Elapsed.TotalSeconds;relay_artifact=$units[0];relay_compiler_jsonl=(Row $rawPath);relay_stderr=(Row $errPath);relay_copy=$relayCopy;normal_graph_relay_after=(Row $units[0].executable);legacy_unchanged=$legacyBefore;preimages=(Row (Join-Path $out 'before.json'));current_source_pins=(Row (Join-Path $out 'current-source-pins-03.json'));elapsed_seconds=$watch.Elapsed.TotalSeconds}
    exit 0
}

if($Mode -ceq 'Freeze'){
    $current=Inputs;$preparationBefore=@(Preparation-Sources)
    $imagePath=Join-Path $evidence ($Prefix+'.images.json');$provenance=Get-Content -LiteralPath $imagePath -Raw|ConvertFrom-Json -AsHashtable
    Require ($provenance.source_head -ceq $head -and $provenance.activated -eq $false -and $provenance.relay_exit_code -eq 0) 'Current inactive image provenance required.'
    Assert-KilogramFrozenInputs $provenance.inputs_after $current
    $guardPath=Join-Path $evidence ($Prefix+'.guards.json');$guard=Get-Content -LiteralPath $guardPath -Raw|ConvertFrom-Json -AsHashtable
    foreach($document in @($provenance,$guard)){foreach($tuple in $document.sources){$actual=Row $tuple.path;Require ($actual.bytes -eq $tuple.bytes -and $actual.sha256 -ceq $tuple.sha256) 'Preparation source changed after checks/capture.'}}
    foreach($copy in @($provenance.normal_cli_copy,$provenance.relay_copy)){$r=Row $copy.after.path;Require ($r.bytes -eq $copy.after.bytes -and $r.sha256 -ceq $copy.after.sha256 -and $r.sha256 -ceq $copy.source.sha256) 'Current copied image tuple matches provenance.'}
    $capture=Get-Content -LiteralPath (Join-Path $workspace $provenance.normal_cli_capture.path) -Raw|ConvertFrom-Json -AsHashtable
    Assert-KilogramFrozenInputs $capture.build_inputs_before $current;Assert-KilogramFrozenInputs $capture.build_inputs_after $current
    $expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile=$buildProfile;target=$target;stable_directory=[IO.Path]::GetFullPath((Join-Path $workspace 'target/verification-scale/deps'));build_inputs=$current;units_identity=@(Get-KilogramFrozenUnitIdentity $capture.units);artifacts=$capture.stable_artifacts}
    Assert-KilogramS53G3RuntimeGraph $expected
    $actualRows=@($capture.stable_artifacts|ForEach-Object {
        $actual=Row $_.path
        [ordered]@{package=$_.package;kind=$_.kind;target=$_.target;path=$_.path;bytes=$actual.bytes;sha256=$actual.sha256;profile=$buildProfile}
    })
    Assert-KilogramFrozenArtifacts $expected $actualRows $buildProfile $target $expected.stable_directory 6
    $expectedRow=Write-G3 'G3-two-context-routes-03.expected-artifacts.candidate.json' $expected
    $null=Read-KilogramFrozenManifest (Join-Path $workspace $expectedRow.path) $expectedRow.sha256 6
    $sources=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($r in $current.source_inputs){$sources[$r.path]=Row $r.path}
    foreach($name in @('prepare-migration-s53-g3.ps1','invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1','migration-s53-runtime-dependencies.ps1','migration-s53-predecessor-proof.ps1','migration-s5-l1-deadline.ps1')){$p='scripts/'+$name;$sources[$p]=Row $p}
    foreach($tuple in @($provenance.normal_cli_capture,$provenance.relay_compiler_jsonl,$provenance.relay_stderr,$provenance.preimages,$provenance.current_source_pins,(Row $imagePath),(Row $guardPath))){$sources[$tuple.path]=Row $tuple.path}
    $images=@(Get-KilogramS53ImageRoles $g3Case|ForEach-Object {Row $_});Assert-KilogramS53ImageRoles $g3Case $images
    $binding=[ordered]@{classification='g3-only-current-two-context-routes-candidate';revision='G3-preparation-03';baseline=$baseline;source_head=$head;inputs=@($sources.Keys|Sort-Object -CaseSensitive|ForEach-Object {$sources[$_]});code_source_count=$current.source_inputs.Count;images=$images;runtime_dependencies=@($shell);runtime_graph=(Graph);cases=@(Policy);expected_artifacts=$expectedRow;normal_cli_provenance=(Row $imagePath);activated=$false;proposed_only=$true;scope='Fresh two-context G3 only; root activation and full-array admission required later; zero prerequisites';compiler_capture=$provenance.normal_cli_capture}
    Assert-KilogramS53G3Policy $g3Case $binding
    $bindingRow=Write-G3 (Get-KilogramS53BindingName $g3Case) $binding
    $post=Inputs;Assert-KilogramFrozenInputs $current $post;Assert-KilogramFrozenInputs $preparationBefore @(Preparation-Sources);$null=Accepted-Sources
    Write-G3 ($Prefix+'.frozen.json') @{classification='G3 prepared inactive source/image binding; no runtime admission';binding=$bindingRow;expected=$expectedRow;source_count=$binding.inputs.Count;code_source_count=$current.source_inputs.Count;image_count=$images.Count;harness_count=$capture.stable_artifacts.Count;raw_unit_count=$capture.units.Count;source_head=$head;before_after_inputs_equal=$true;elapsed_seconds=$watch.Elapsed.TotalSeconds;activated=$false}
}
