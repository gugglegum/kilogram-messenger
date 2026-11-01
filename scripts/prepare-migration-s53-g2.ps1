# G2-B preparation only. Never starts a runtime owner, relay or network test.
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('Guard','Relay','Freeze')][string]$Mode,[Parameter(Mandatory)][ValidatePattern('^G2-B-[a-z0-9-]+$')][string]$Prefix)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$watch=[Diagnostics.Stopwatch]::StartNew()
$workspace=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $workspace
$out=Join-Path $workspace '.tmp/g2b-refresh-02'
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
$g2Case='F-s53-capacity-recovery-r2-01'
$baseline='64c2359788ead360314793e47741adacb83b8d4f'
$target='x86_64-pc-windows-msvc';$buildProfile='verification-scale'
$acceptedCapture='.tmp/g2-accounting-01/compile-artifacts-02.json'
$acceptedReceipt='.tmp/g2-accounting-01/result.json'
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
function Write-G2([string]$Name,$Value){$path=Join-Path $evidence $Name;Write-KilogramFrozenJson $path $Value;Row $path}
function Policy {
    [ordered]@{case=$g2Case;selector='live_l1::handoff::capacity_review2::s53_capacity_recovery_preserves_validated_route_public';expected_count=1;image='target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe';source='apps/kilogram-cli/tests/support/migration_s53_capacity_review2.rs';controller_seconds=240;work_seconds=275;whole_seconds=300;package='kilogram-cli';assertion_envelope=$true;loss=$null;activated=$false;build_profile=$buildProfile}
}
function Inputs {Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile $buildProfile -Target $target -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()}
function Preparation-Sources {
    @('prepare-migration-s53-g2.ps1','invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1')|ForEach-Object {Row ('scripts/'+$_)}
}
function Verify-Accounting {
    $captureRow=Row $acceptedCapture;$receiptRow=Row $acceptedReceipt
    Require ($captureRow.bytes -eq 2690143 -and $captureRow.sha256 -ceq '341a070b838ca1bbad36d690d5b26ade9f5bd4237945e576ef93e6c34261d682') 'Exact root-accepted accounting compiler capture.'
    Require ($receiptRow.bytes -eq 28092 -and $receiptRow.sha256 -ceq '429a136a0a9a90be2c480f3d83d517468260d08c0dbda7324a7c0f3d4fdfb5c0') 'Exact root-accepted accounting repair receipt.'
    $accepted=Get-Content -LiteralPath (Join-Path $workspace $acceptedReceipt) -Raw|ConvertFrom-Json -AsHashtable
    $normal=Row $accepted.normal_cli.path
    Require ($normal.bytes -eq $accepted.normal_cli.bytes -and $normal.sha256 -ceq $accepted.normal_cli.sha256) 'Normal CLI bytes match root-accepted non-test compiler image.'
    $capture=Get-Content -LiteralPath (Join-Path $workspace $acceptedCapture) -Raw|ConvertFrom-Json -AsHashtable
    $current=Inputs
    Assert-KilogramFrozenInputs $capture.build_inputs_before $capture.build_inputs_after
    Assert-KilogramFrozenInputs $capture.build_inputs_after $current
    foreach($r in $capture.stable_artifacts){Require ((Get-Item -LiteralPath $r.path).Length -eq $r.bytes -and (Get-KilogramFrozenHash $r.path) -ceq $r.sha256) 'Accepted accounting test image changed.'}
    return $capture
}
function Copy-Proven([string]$Source,[string]$Destination,[string]$Archive) {
    Require (@(Get-Process -ErrorAction Stop|Where-Object ProcessName -Like 'kilogram-*').Count -eq 0) 'Live Kilogram process prevents fixed-path recopy.'
    $before=Row $Destination
    $archivePath=Join-Path $out $Archive
    Require (-not (Test-Path -LiteralPath $archivePath)) 'Old image archive already exists.'
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($archivePath))
    Copy-Item -LiteralPath $Destination -Destination $archivePath
    Require ((Get-KilogramFrozenHash $archivePath) -ceq $before.sha256) 'Old image archive mismatch.'
    $sourceRow=Row $Source
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
    $after=Row $Destination
    Require ($sourceRow.bytes -eq $after.bytes -and $sourceRow.sha256 -ceq $after.sha256) 'Actual fixed image copy mismatch.'
    [ordered]@{source=$sourceRow;before=$before;archive=(Row $archivePath);after=$after}
}
$shellPath=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
$shell=Get-KilogramS53ShellDependency -Executable $shellPath -RequireCurrentHost
$head=(& git rev-parse HEAD).Trim();Require ($LASTEXITCODE -eq 0 -and $head -ceq '2f7e44f756ea2ae13cf270be33889bada356f8f6') 'Reviewed current HEAD required.'

if($Mode -ceq 'Guard'){
    $checks=[Collections.Generic.List[object]]::new()
    foreach($name in @('invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1','prepare-migration-s53-g2.ps1')){
        $tokens=$null;$errors=$null
        $ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
        Require ($errors.Count -eq 0) ('PowerShell parse: '+$name)
        if($name -like 'invoke-*'){
            Require (@($ast.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq 'Get-KilogramS53BindingName'},$true)).Count -eq 1) 'Both actual wrappers must use common binding routing.'
        }
        $checks.Add(@{name=('parse-'+$name);pass=$true})
    }
    function Fixture([string]$Case){
        [ordered]@{classification=if($Case -ceq $g2Case){'g2-only-current-capacity-route-candidate'}else{'synthetic-nine-role-guard'};baseline=$baseline;source_head=$head;inputs=@(@{path='guard-source.txt';bytes=1;sha256=('0'*64)});code_source_count=1;images=@(Get-KilogramS53ImageRoles $Case|ForEach-Object {@{path=$_;bytes=1;sha256=('0'*64)}});runtime_dependencies=@($shell);cases=@(if($Case -ceq $g2Case){Policy}else{@{case=$Case}});expected_artifacts=@{path='synthetic-expected.json';bytes=1;sha256=('0'*64)}}
    }
    function Envelope([string]$Name,[string]$Case,$Binding){
        $fixtureRoot=Join-Path $out ('guards/'+$Prefix+'/'+$Name)
        New-KilogramS53InventoryEnvelope -Case $Case -Attempt ($Case+'-attempt-01') -Workspace $fixtureRoot -Binding $Binding -BindingHash ('1'*64) -ActivationSha256 ('2'*64) -Shell $shell -Predecessors @() -Prerequisites @()
    }
    function Save-Envelope($Envelope){
        [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Envelope.InventoryPath))
        [IO.File]::WriteAllBytes($Envelope.InventoryPath,$Envelope.Bytes)
        Assert-KilogramS53WrittenInventory -Path $Envelope.InventoryPath -Envelope $Envelope
    }
    function Refuses([string]$Name,[scriptblock]$Action){
        $refused=$false;$reason=$null
        try{& $Action|Out-Null}catch{$refused=$true;$reason=$_.Exception.Message}
        Require $refused ('Real guard accepted negative '+$Name)
        $checks.Add(@{name=$Name;pass=$true;refused=$true;reason=$reason})
    }
    $old='F-s53-committed-before-publication-01'
    Require ((Get-KilogramS53BindingName $g2Case) -ceq 'G2-capacity-route-02.binding.candidate.json') 'G2 route'
    Require ((Get-KilogramS53BindingName 'F-s53-exhausted-integrated-01') -ceq 'C-s53-g1-continuation-C2-01.binding.candidate.json' -and (Get-KilogramS53BindingName $old) -ceq 'S5.3-integration-binding-r30.json') 'G1/other routes retained'
    foreach($c in @($g2Case,$old)){$env=Envelope ('positive-'+$c) $c (Fixture $c);$saved=Save-Envelope $env;Require ($saved.ByteCount -eq $env.ByteCount) 'Real written positive';$checks.Add(@{name=('builder-and-written-'+$c);pass=$true;roles=$env.Value.images.Count})}
    foreach($mutation in @('missing','extra','duplicate')){
        $b=Fixture $g2Case
        if($mutation -ceq 'missing'){$b.images=@($b.images|Where-Object path -NotLike '*kilogram-relay.exe')}
        if($mutation -ceq 'extra'){$b.images+=@{path='target/extra.exe';bytes=1;sha256=('0'*64)}}
        if($mutation -ceq 'duplicate'){$b.images[9]=$b.images[0]}
        Refuses ('builder-'+$mutation) {Envelope ('builder-'+$mutation) $g2Case $b}
        $env=Envelope ('written-'+$mutation) $g2Case (Fixture $g2Case)
        $env.Value.images=$b.images
        $env.Bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $env.Value -Depth 30))
        $env.ByteCount=$env.Bytes.LongLength
        Refuses ('written-'+$mutation) {Save-Envelope $env}
    }
    $b=Fixture $g2Case;Refuses 'wrong-case' {Envelope 'wrong-case' $old $b}
    $b=Fixture $g2Case;$b.cases+=@{case=$old};Refuses 'extra-case' {Envelope 'extra-case' $g2Case $b}
    $b=Fixture $g2Case
    Refuses 'G2-prerequisite' {New-KilogramS53InventoryEnvelope -Case $g2Case -Attempt ($g2Case+'-attempt-01') -Workspace (Join-Path $out 'guards/prerequisite') -Binding $b -BindingHash ('1'*64) -ActivationSha256 ('2'*64) -Shell $shell -Predecessors @() -Prerequisites @(@{path='public-prerequisite.json';bytes=1;sha256=('0'*64);classification='root-allocated public replica manifest; no preparation performed by adapter'})}
    $env=Envelope 'written-prerequisite' $g2Case (Fixture $g2Case);$env.Value.prerequisites=@(@{path='public-prerequisite.json';bytes=1;sha256=('0'*64);classification='root-allocated public replica manifest; no preparation performed by adapter'});$env.Bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $env.Value -Depth 30));$env.ByteCount=$env.Bytes.LongLength
    Refuses 'written-G2-prerequisite' {Save-Envelope $env}
    foreach($field in @('selector','expected_count')){$b=Fixture $g2Case;if($field -ceq 'selector'){$b.cases[0].selector='wrong'}else{$b.cases[0].expected_count=2};Refuses ('wrong-'+$field) {Envelope ('wrong-'+$field) $g2Case $b}}
    $b=Fixture $old;$b.images[8]=@{path='target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe';bytes=1;sha256=('0'*64)}
    Refuses 'other-nine-wrong-role' {Envelope 'other-nine-wrong-role' $old $b}
    $env=Envelope 'other-written-wrong-role' $old (Fixture $old);$env.Value.images=$b.images;$env.Bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $env.Value -Depth 30));$env.ByteCount=$env.Bytes.LongLength
    Refuses 'other-written-wrong-role' {Save-Envelope $env}
    $b=Fixture $g2Case;$matched=Read-KilogramS53CaseBaseline $workspace $g2Case $b
    Require ($matched.status -ceq 'verified_match' -and $matched.cleanup_verified) 'Actual current HEAD guard match.'
    $b.source_head='0'*40;$changed=Read-KilogramS53CaseBaseline $workspace $g2Case $b
    Require ($changed.status -ceq 'verified_changed' -and $changed.cleanup_verified) 'Actual wrong HEAD guard refusal.'
    $checks.Add(@{name='actual-current-and-wrong-HEAD';pass=$true;matched=$matched;changed=$changed})
    Write-G2 ($Prefix+'.guards.json') @{classification='G2-B real pure guard checks; synthetic public inventories only; no runtime admission';sources=@(Preparation-Sources);checks=$checks.ToArray();elapsed_seconds=$watch.Elapsed.TotalSeconds;activated=$false}
    exit 0
}

if($Mode -ceq 'Relay'){
    $a=Verify-Accounting;$before=Inputs
    $preparationBefore=@(Preparation-Sources)
    $legacyBefore=Row 'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe'
    $normal=@($a.units|Where-Object {$_.package -ceq 'kilogram-cli' -and $_.target -ceq 'kilogram-cli' -and $_.profile.test -eq $false -and $_.executable})
    Require ($normal.Count -eq 1 -and $normal[0].kind.Count -eq 1 -and $normal[0].kind[0] -ceq 'bin' -and $normal[0].profile.opt_level -ceq '3' -and $normal[0].profile.debug_assertions -and $normal[0].profile.overflow_checks) 'Normal non-test CLI profile proof.'
    $normalCopy=Copy-Proven $normal[0].executable (Join-Path $workspace 'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe') ($Prefix+'/old-normal-cli.exe')
    $arguments=@('build','--locked','--offline','--jobs','2','--target',$target,'--profile',$buildProfile,'-p','kilogram-transport-iroh','--example','s4_loopback_relay','--message-format=json-render-diagnostics')
    $timer=[Diagnostics.Stopwatch]::StartNew()
    $compiler=@(& cargo @arguments 2> (Join-Path $out ($Prefix+'.relay.stderr.log')));$exit=$LASTEXITCODE
    $timer.Stop()
    [IO.File]::WriteAllLines((Join-Path $out ($Prefix+'.relay.compiler.jsonl')),[string[]]$compiler,[Text.UTF8Encoding]::new($false))
    Require ($exit -eq 0) 'Relay compile returned failure; raw compiler/stderr retained.'
    $units=@($compiler|ForEach-Object {try{$_|ConvertFrom-Json -AsHashtable}catch{}}|Where-Object {$_.reason -ceq 'compiler-artifact' -and $_.target.name -ceq 's4_loopback_relay' -and $_.executable})
    Require ($units.Count -eq 1 -and $units[0].target.kind.Count -eq 1 -and $units[0].target.kind[0] -ceq 'example' -and $units[0].profile.test -eq $false -and $units[0].profile.opt_level -ceq '3' -and $units[0].profile.debug_assertions -and $units[0].profile.overflow_checks) 'Actual relay compiler profile/artifact proof.'
    $package=@($before.package_names|Where-Object name -CEQ 'kilogram-transport-iroh')
    Require ($package.Count -eq 1 -and $units[0].package_id -ceq $package[0].id) 'Actual relay package identity.'
    Require ([IO.Path]::GetFullPath($units[0].executable) -ceq (Join-Path $workspace 'target/x86_64-pc-windows-msvc/verification-scale/examples/s4_loopback_relay.exe').Replace('/','\')) 'Exact relay compiler output path.'
    $relayCopy=Copy-Proven $units[0].executable (Join-Path $workspace 'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe') ($Prefix+'/old-relay.exe')
    $after=Inputs;Assert-KilogramFrozenInputs $before $after
    Assert-KilogramFrozenInputs $preparationBefore @(Preparation-Sources)
    Require ((Get-KilogramFrozenHash (Join-Path $workspace $legacyBefore.path)) -ceq $legacyBefore.sha256) 'Frozen legacy CLI unchanged.'
    $value=@{classification='G2-B actual nonexecuting normal CLI reuse and relay build/copy provenance';activated=$false;sources=@(Preparation-Sources);source_head=$head;inputs_before=$before;inputs_after=$after;normal_cli_capture=(Row $acceptedCapture);accounting_repair_receipt=(Row $acceptedReceipt);normal_cli_unit=$normal[0];normal_cli_copy=$normalCopy;relay_arguments=$arguments;relay_exit_code=$exit;relay_compile_seconds=$timer.Elapsed.TotalSeconds;relay_artifact=$units[0];relay_compiler_jsonl=(Row (Join-Path $out ($Prefix+'.relay.compiler.jsonl')));relay_stderr=(Row (Join-Path $out ($Prefix+'.relay.stderr.log')));relay_copy=$relayCopy;legacy_unchanged=$legacyBefore;elapsed_seconds=$watch.Elapsed.TotalSeconds}
    Write-G2 ($Prefix+'.images.json') $value
    exit 0
}

if($Mode -ceq 'Freeze'){
    $a=Verify-Accounting;$current=Inputs
    $preparationBefore=@(Preparation-Sources)
    $imagePath=Join-Path $evidence ($Prefix+'.images.json');$provenance=Get-Content -LiteralPath $imagePath -Raw|ConvertFrom-Json -AsHashtable
    Require ($provenance.source_head -ceq $head -and $provenance.activated -eq $false -and $provenance.relay_exit_code -eq 0) 'Current inactive image provenance required.'
    Assert-KilogramFrozenInputs $provenance.inputs_after $current
    $guard=Get-Content -LiteralPath (Join-Path $evidence ($Prefix+'.guards.json')) -Raw|ConvertFrom-Json -AsHashtable
    foreach($document in @($provenance,$guard)){foreach($tuple in $document.sources){$actual=Row $tuple.path;Require ($actual.bytes -eq $tuple.bytes -and $actual.sha256 -ceq $tuple.sha256) 'Preparation sources changed after checks/capture.'}}
    foreach($copy in @($provenance.normal_cli_copy,$provenance.relay_copy)){$r=Row $copy.after.path;Require ($r.bytes -eq $copy.after.bytes -and $r.sha256 -ceq $copy.after.sha256 -and $r.sha256 -ceq $copy.source.sha256) 'Current copied image tuple matches provenance.'}
    $expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile=$buildProfile;target=$target;stable_directory=[IO.Path]::GetFullPath((Join-Path $workspace 'target/verification-scale/deps'));build_inputs=$current;units_identity=@(Get-KilogramFrozenUnitIdentity $a.units);artifacts=$a.stable_artifacts}
    $expectedRow=Write-G2 'G2-capacity-route-02.expected-artifacts.candidate.json' $expected
    $sources=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($r in $current.source_inputs){$sources[$r.path]=Row $r.path}
    foreach($name in @('prepare-migration-s53-g2.ps1','invoke-migration-s53-integration-case.ps1','invoke-migration-s53-integration-outer.ps1','migration-s53-metadata.ps1','migration-s53-runtime-dependencies.ps1','migration-s53-predecessor-proof.ps1','migration-s5-l1-deadline.ps1')){$p='scripts/'+$name;$sources[$p]=Row $p}
    $sources[$provenance.normal_cli_capture.path]=Row $provenance.normal_cli_capture.path
    $sources[$provenance.accounting_repair_receipt.path]=Row $provenance.accounting_repair_receipt.path
    foreach($tuple in @($provenance.relay_compiler_jsonl,$provenance.relay_stderr)){$sources[$tuple.path]=Row $tuple.path}
    $sources[[IO.Path]::GetRelativePath($workspace,$imagePath).Replace('\','/')]=Row $imagePath
    $guardPath=Join-Path $evidence ($Prefix+'.guards.json');$sources[[IO.Path]::GetRelativePath($workspace,$guardPath).Replace('\','/')]=Row $guardPath
    $images=@(Get-KilogramS53ImageRoles $g2Case|ForEach-Object {Row $_})
    Assert-KilogramS53ImageRoles $g2Case $images
    $binding=[ordered]@{classification='g2-only-current-capacity-route-candidate';revision='G2-B-02';baseline=$baseline;source_head=$head;inputs=@($sources.Keys|Sort-Object -CaseSensitive|ForEach-Object {$sources[$_]});code_source_count=$current.source_inputs.Count;images=$images;runtime_dependencies=@($shell);cases=@(Policy);expected_artifacts=$expectedRow;normal_cli_provenance=(Row $imagePath);activated=$false;proposed_only=$true;scope='Fresh capacity/route G2 only; root activation and full-array admission required later; no exhausted fixture';reused_accounting_compile=(Row $acceptedCapture);accounting_repair_receipt=(Row $acceptedReceipt)}
    Assert-KilogramS53G2Policy $g2Case $binding
    $bindingRow=Write-G2 (Get-KilogramS53BindingName $g2Case) $binding
    $post=Inputs;Assert-KilogramFrozenInputs $current $post
    Assert-KilogramFrozenInputs $preparationBefore @(Preparation-Sources)
    Write-G2 ($Prefix+'.frozen.json') @{classification='G2-B prepared inactive source/image binding; no runtime admission';binding=$bindingRow;expected=$expectedRow;source_count=$binding.inputs.Count;image_count=$images.Count;source_head=$head;before_after_inputs_equal=$true;accepted_accounting_build_inputs_reused_exactly=$true;elapsed_seconds=$watch.Elapsed.TotalSeconds;activated=$false}
}
