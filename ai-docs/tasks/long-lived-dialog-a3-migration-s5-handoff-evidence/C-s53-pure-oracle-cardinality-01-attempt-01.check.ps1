# Dormant r75 source: future separately allocated pure-data invocation only.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$e=$workspace+'\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence'
$attempt='C-s53-pure-oracle-cardinality-01-attempt-01'
$check=$e+'\'+$attempt+'.check.ps1'
$reportPath=$e+'\'+$attempt+'.cases.json'
$oldPrefix=$e+'\C-s53-cold-core-named-streams-01-attempt-01'
$script:stage='bootstrap';$script:primary='not_started';$script:primaryType='';$script:primaryHr=0
$script:privateErrors=[Collections.Generic.List[object]]::new()
$script:localizations=[Collections.Generic.List[object]]::new()
$script:rows=[Collections.Generic.List[object]]::new()
$script:scopes=[ordered]@{}
$script:inputCurrent=$null;$script:inputAttempted=$false;$script:inputUnknown=$false
$script:inputReturns=0;$script:inputClosed=0;$script:outputCurrent=$null;$script:outputAttempted=$false;$script:outputClosed=$false
$script:otherType='';$script:reportWritten=$false;$script:evaluations=0;$script:checkerCalls=0
function R75-Stop([string]$Code) {
 $ex=[InvalidOperationException]::new('R75 bounded data check');$ex.Data['r75_code']=$Code;throw $ex
}
function R75-Need([bool]$Yes,[string]$Code='data') {if(-not $Yes){R75-Stop $Code}}
function R75-Read([string]$Path,[int]$Length,[string]$Hash) {
 
 R75-Need (-not $script:inputUnknown -and $null -eq $script:inputCurrent) 'input_owner'
 $script:inputAttempted=$false
 try{
  $script:inputCurrent=[IO.FileStream]::new($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
  $script:inputReturns++
  R75-Need ($Length -gt 0 -and $Length -le 131072 -and $script:inputCurrent.Length -eq $Length) 'input_pin'
  $b=[byte[]]::new($Length);$n=0
  while($n -lt $Length){;$r=$script:inputCurrent.Read($b,$n,$Length-$n);R75-Need ($r -gt 0) 'input_short';$n+=$r}
  R75-Need ($script:inputCurrent.ReadByte() -eq -1) 'input_grew'
  R75-Need ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b)).ToLowerInvariant() -ceq $Hash) 'input_pin'
  
  return ,$b
 }finally{
  if($script:inputCurrent -and -not $script:inputAttempted){
   $script:inputAttempted=$true
   try{$script:inputCurrent.Dispose();$script:inputClosed++;$script:inputCurrent=$null}catch{$script:inputUnknown=$true;throw}
  }
 }
}
function R75-SafeType($Object) {
 if($null -eq $Object){return ''}
 $t=$Object.GetType().FullName
 R75-Need ($t -is [string] -and $t.Length -le 128 -and $t -cmatch '^[A-Za-z0-9_.+]+$') 'type_projection'
 return $t
}
function R75-Remember($Er) {
 R75-Need ($script:privateErrors.Count -lt 92) 'error_cap'
 $script:privateErrors.Add($Er)
}
function R75-Code($Er) {
 $t=R75-SafeType $Er.Exception
 switch -CaseSensitive ($t) {
  'System.InvalidOperationException' {return 1}
  'System.Management.Automation.PropertyNotFoundException' {return 2}
  'System.Management.Automation.RuntimeException' {return 3}
  default {if($script:otherType -ceq ''){$script:otherType=$t};return 4}
 }
}
function R75-CloneMap($Object,[bool]$Outer=$true) {
 R75-Need ($Object -is [pscustomobject]) 'copy_type'
 $dst=[ordered]@{};$props=@($Object.PSObject.Properties)
 R75-Need ($props.Count -le 38) 'copy_count'
 foreach($p in $props){
  R75-Need ($p.MemberType -eq [Management.Automation.PSMemberTypes]::NoteProperty) 'copy_property'
  $v=$p.Value
  if($Outer -and $p.Name -cin @('phase_seconds','private_receipt_digests')){$dst[$p.Name]=R75-CloneMap $v $false}
  else{
   R75-Need ($null -eq $v -or $v -is [string] -or $v -is [bool] -or (R75-SafeType $v) -cin @('System.SByte','System.Byte','System.Int16','System.UInt16','System.Int32','System.UInt32','System.Int64','System.UInt64','System.Double','System.Single','System.Decimal')) 'copy_scalar'
   $dst[$p.Name]=$v
  }
 }
 return [pscustomobject]$dst
}
function R75-Frame($Er,$Scope) {
 $stack=$Er.ScriptStackTrace
 R75-Need ($stack -is [string] -and $stack.Length -le 8192) 'stack_capture'
 $found=$null;$n=0
 foreach($line in $stack.Split([char]10)){
  $n++;R75-Need ($n -le 16) 'stack_capture'
  $pattern='^at (Assert-Fields|Assert-CoreSnapshot), '+[regex]::Escape($Scope.Virtual)+': line ([0-9]+)\s*$'
  $m=[regex]::Match($line,$pattern)
  if($m.Success -and $null -eq $found){
   $name=$m.Groups[1].Value;$emitted=[int]$m.Groups[2].Value;$mp=$Scope.Map[$name]
   R75-Need ($emitted -ge $mp.E -and $emitted -lt $mp.E+$mp.L) 'stack_capture'
   $found=[pscustomobject]@{F=$name;Line=($mp.O+$emitted-$mp.E);Emitted=$emitted}
  }
 }
 R75-Need ($null -ne $found) 'stack_capture'
 return $found
}
function R75-Clip($Value,[int]$Cap,$Flags) {
 if($null -eq $Value){return ''}
 if($Value -isnot [string]){$Flags.Bad=$true;return ''}
 if($Value.Length -gt $Cap){$Flags.Trunc=$true;return $Value.Substring(0,$Cap)}
 return $Value
}
function R75-Localize($Er,$Scope) {
 $flags=[pscustomobject]@{Bad=$false;Trunc=$false}
 $p=[ordered]@{type='';hr=0;fq='';msg='';stack='';pos='';line='';script='';ml=0;mc=0;mo=-1;cat=0;bad=$false;trunc=$false;complete=$false}
 try{
  $p.type=R75-Clip ($Er.Exception.GetType().FullName) 128 $flags
  $p.hr=[int]$Er.Exception.HResult
  $p.fq=R75-Clip $Er.FullyQualifiedErrorId 128 $flags
  $p.msg=R75-Clip $Er.Exception.Message 256 $flags
  $p.stack=R75-Clip $Er.ScriptStackTrace 1280 $flags
  $p.pos=R75-Clip $Er.InvocationInfo.PositionMessage 512 $flags
  $p.line=R75-Clip $Er.InvocationInfo.Line 256 $flags
  $p.script=R75-Clip $Er.InvocationInfo.ScriptName 96 $flags
  $p.mc=[int]$Er.InvocationInfo.OffsetInLine;$p.cat=[int]$Er.CategoryInfo.Category
  R75-Need ($p.mc -ge 0 -and $p.mc -le 1024 -and $p.cat -ge 0 -and $p.cat -le 255) 'localization'
  $frame=R75-Frame $Er $Scope;$p.ml=$frame.Line
  $fm=$Scope.Map[$frame.F]
  R75-Need ($Er.InvocationInfo.ScriptLineNumber -eq $frame.Emitted -and $p.mc -ge 1) 'localization'
  $p.mo=$fm.Offset+$fm.LineOffsets[$frame.Emitted-$fm.E]+$p.mc-1
  R75-Need ($p.mo -ge 0 -and $p.mo -le 131072) 'localization'
  $wanted=if($Scope.Copy -ceq 'W'){1041}else{207}
  $p.complete=($p.type -ceq 'System.Management.Automation.PropertyNotFoundException' -and $p.fq -cmatch '^PropertyNotFound(Strict)?' -and
   $p.msg -cmatch '(?<![A-Za-z0-9_])Count(?![A-Za-z0-9_])' -and $p.line.Contains('$keys.Count') -and $frame.F -ceq 'Assert-CoreSnapshot' -and $frame.Line -eq $wanted)
 }catch{$flags.Bad=$true}
 $p.bad=$flags.Bad;$p.trunc=$flags.Trunc;$p.complete=$p.complete -and -not $flags.Bad -and -not $flags.Trunc
 return $p
}
function R75-NewRow([string]$Group,[string]$Copy,[string]$Variant,[string]$Name,[int]$Mutation=0) {
 R75-Need ($script:rows.Count -lt 92) 'row_cap'
 $row=[ordered]@{i=$script:rows.Count;g=$Group;c=$Copy;v=$Variant;n=$Name;m=$Mutation;o='not_run';ok=$false;t=0;site=0;want=0;shape=0;len=-1;cnt=-1;bad=$false}
 $script:rows.Add($row)
 return $row
}
function R75-Raw($Scope,$Observation,$Spec,[bool]$Pre) {
 ;$script:checkerCalls++;$script:evaluations++
 $r=& $Scope.Module {
  param($obs,$specification,[bool]$before)
  try{Assert-CoreSnapshot $obs $specification $before;return ,([pscustomobject]@{Thrown=$false;Er=$null})}
  catch{$original=$_;return ,([pscustomobject]@{Thrown=$true;Er=$original})}
 } $Observation $Spec $Pre
 if($r.Thrown){R75-Remember $r.Er}
 
 return $r
}

$manifest=@(
    @{N='Stop-C2F';B=140;H='6040135db8b63b5b3e1c32a5f1ff139dda2ffa825b6591cf17ec84026ae87685';W=22;S=19;L=3},
    @{N='Need';B=95;H='f2c04690f0ec00bf20a314b88ad34a5308d1bd136074f27bbdef471bc72ad7dc';W=154;S=151;L=1},
    @{N='Case-Specs';B=1361;H='a59e39b415be1f27002469389a07daae0b7eb8286912abdbf5ad3e5df6d54995';W=155;S=152;L=6},
    @{N='Object-Keys';B=236;H='24f7e18060adf1cb937462e8aec871c4e0bc068ac624af056bc83f2dda4ea354';W=992;S=158;L=6},
    @{N='Get-Value';B=193;H='63dc95cfefe0750b76d1444da587976c0dfe07a5cb7fbe0590093a34314a0e43';W=998;S=164;L=5},
    @{N='Assert-Fields';B=865;H='2bde8f63bb7656b83605ffc4ee192527ec2b94624e1d9d4bfb67e2dfba1c1171';W=1003;S=169;L=17},
    @{N='Assert-CoreSnapshot';B=2220;H='59dfe85c0860fc12c00ea7880e96595372b96a567bb213633e3ef5f1918b5623';W=1020;S=186;L=28}
)
$expectedSpecs=@(
[ordered]@{Name='file-stream';Selector='streams-file-payload';Directory=$false;InventoryBytes=3578;InventoryHash='78adc55b5edb030063343e8249b926e063c999f9d6dc4fe2761586341a93e26c';PreHook='capture_s';FailurePhase='capture_s';FailureRole='s';CoreLocks=6;Acquired=2;PreLocks=6;PreOwned=26;PrePeak=27;PreRecord=12084624;PrePath=6920;PeakHandles=29;PeakRecord=12085392;PeakPath=6920;InventoryPeak=3578;PreInventoryPeak=3578;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';SFiles=5;SBytes=52;BFiles=5;BBytes=52;FileCount=13;TargetRelative='state/one/state/payload.bin';Phases=@('locks','inventory','capture_s');PreKeys=@('allocation','locks','inventory')},
[ordered]@{Name='directory-stream';Selector='streams-directory-payload';Directory=$true;InventoryBytes=3598;InventoryHash='341a377c3c01d0318e4aced118fa30d24966680719ebe4c46fcca9eed46d2433';PreHook='locks';FailurePhase='locks';FailureRole='s_one_lock';CoreLocks=2;Acquired=0;PreLocks=0;PreOwned=0;PrePeak=0;PreRecord=12073840;PrePath=0;PeakHandles=15;PeakRecord=12079216;PeakPath=3016;InventoryPeak=0;PreInventoryPeak=0;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';SFiles=5;SBytes=52;BFiles=5;BBytes=52;FileCount=13;TargetRelative='state/one/state';Phases=@('locks');PreKeys=@('allocation')}
)
function R75-Scope([string]$Copy,[string]$Variant,[string]$Text,[string]$Path) {
 ;$tt=$null;$ee=$null
 $a=[Management.Automation.Language.Parser]::ParseInput($Text,$Path,[ref]$tt,[ref]$ee)
 R75-Need ($ee.Count -eq 0) 'source_ast'
 $all=@($a.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst]},$true))
 $pieces=[Collections.Generic.List[string]]::new();$map=[ordered]@{};$emitted=1;$checker=''
 foreach($m in $manifest){
  
  $definitionsFound=@($all|Where-Object Name -CEQ $m.N);R75-Need ($definitionsFound.Count -eq 1) 'definition'
  $f=$definitionsFound[0];$text=$f.Extent.Text;$b=[Text.UTF8Encoding]::new($false,$true).GetBytes($text)
  $origin=if($Copy -ceq 'W'){$m.W}else{$m.S}
  R75-Need ($f.Extent.StartLineNumber -eq $origin -and $f.Extent.EndLineNumber-$origin+1 -eq $m.L -and $b.Length -eq $m.B -and
   [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b)).ToLowerInvariant() -ceq $m.H) 'definition_pin'
  $calls=@($f.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst]},$true))
  foreach($call in $calls){R75-Need ($call.GetCommandName() -cin @($manifest.N)) 'closure'}
  $lineOffsets=[Collections.Generic.List[int]]::new();$offset=0
  foreach($sourceLine in $f.Extent.Text.Split([char]10)){$lineOffsets.Add($offset);$offset+=$sourceLine.Length+1}
  $map[$m.N]=[pscustomobject]@{E=$emitted;O=$origin;L=$m.L;Offset=$f.Extent.StartOffset;LineOffsets=$lineOffsets.ToArray()}
  if($m.N -ceq 'Assert-CoreSnapshot'){
   $old='    $keys=if($Pre){@($s.PreKeys)}else{@(''allocation'')+@($s.Phases)}'
   $next='    $keys=@(if($Pre){@($s.PreKeys)}else{@(''allocation'')+@($s.Phases)})'
   R75-Need (([regex]::Matches($text,[regex]::Escape($old))).Count -eq 1) 'candidate_inverse'
   if($Variant -ceq 'C'){
    $text=$text.Replace($old,$next);$cb=[Text.UTF8Encoding]::new($false,$true).GetBytes($text)
    R75-Need ($cb.Length -eq 2223 -and [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($cb)).ToLowerInvariant() -ceq
     'a57bf1b7465ea8a25616b0d36d5331743d5f433aac8bc5cc170db018906509d8' -and $text.Replace($next,$old) -ceq $f.Extent.Text) 'candidate_inverse'
   }
   $checker=$text
  }
  $pieces.Add($text);$emitted+=$m.L;
 }
 $virtual='r75_'+$Copy+'_'+$Variant+'.pure.ps1'
 $definitions=[string]::Join([string][char]10,$pieces)
 $dt=$null;$de=$null;$da=[Management.Automation.Language.Parser]::ParseInput($definitions,$virtual,[ref]$dt,[ref]$de)
 R75-Need ($de.Count -eq 0 -and $da.EndBlock.Statements.Count -eq 7) 'extraction_ast'
 foreach($statement in $da.EndBlock.Statements){R75-Need ($statement -is [Management.Automation.Language.FunctionDefinitionAst]) 'extraction_ast'}
 $module=New-Module -Name ('R75_'+$Copy+'_'+$Variant) -ArgumentList @($da.GetScriptBlock()) -ScriptBlock {
  param([scriptblock]$definitions)
  Set-StrictMode -Version Latest
  . $definitions
 }
 $specs=@(& $module {Case-Specs});R75-Need ($specs.Count -eq 2) 'spec'
 for($j=0;$j -lt 2;$j++){
  $ps=@($specs[$j].PSObject.Properties);$want=$expectedSpecs[$j]
  R75-Need ($ps.Count -eq $want.Count) 'spec'
  foreach($property in $ps){
   R75-Need ($property.MemberType -eq [Management.Automation.PSMemberTypes]::NoteProperty -and $want.Contains($property.Name)) 'spec'
   $actual=$property.Value;$v=$want[$property.Name]
   if($v -is [object[]]){
    R75-Need ($actual -is [object[]] -and $actual.Count -eq $v.Count -and ($actual -join '|') -ceq ($v -join '|')) 'spec'
    foreach($element in $actual){R75-Need ($element -is [string]) 'spec'}
   }else{R75-Need ($actual.GetType() -eq $v.GetType() -and $actual -ceq $v) 'spec'}
  }
 }
 $st=$null;$se=$null;$sa=[Management.Automation.Language.Parser]::ParseInput($checker,[ref]$st,[ref]$se)
 $assignment=@($sa.FindAll({param($n)$n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$keys'},$true))
 R75-Need ($se.Count -eq 0 -and $assignment.Count -eq 1) 'shape_ast'
 $xt=$null;$xe=$null;$xa=[Management.Automation.Language.Parser]::ParseInput($assignment[0].Extent.Text,$virtual+'.shape',[ref]$xt,[ref]$xe)
 R75-Need ($xe.Count -eq 0 -and $xa.EndBlock.Statements.Count -eq 1) 'shape_ast'
 
 return [pscustomobject]@{Copy=$Copy;Variant=$Variant;Module=$module;Specs=$specs;Map=$map;Virtual=$virtual;Shape=$xa.GetScriptBlock()}
}
function R75-Snapshot([string]$Name) {
 switch -CaseSensitive ($Name){
  'FP' {return $script:observed.cases[0].pre_hook_core_observation}
  'FF' {return $script:observed.cases[0].core_observation}
  'DP' {return $script:observed.cases[1].pre_hook_core_observation}
  'DF' {
   $x=R75-CloneMap $script:observed.cases[1].pre_hook_core_observation
   $x.input_handles_released=$true;$x.failure_code='named_stream';$x.failure_phase='locks';$x.failure_role='s_one_lock';$x.cause_kind='bounded_refusal'
   $x.error_hresult=[long]-2146233079;$x.locks=[long]2;$x.peak_owned_handles=[long]15
   $x.peak_record_bytes=[long]12079216;$x.peak_path_bytes=[long]3016
   $x.phase_seconds=[pscustomobject][ordered]@{allocation=$x.phase_seconds.allocation;locks=[double]0.125}
   return $x
  }
  default {R75-Stop 'baseline_name'}
 }
}
function R75-Mutate([int]$Id) {
 $x=R75-CloneMap (R75-Snapshot 'FP')
 switch($Id){
  1 {$x.phase_seconds.PSObject.Properties.Remove('locks')}
  2 {$x.phase_seconds.PSObject.Properties.Add([Management.Automation.PSNoteProperty]::new('extra',[double]0.125))}
  3 {$x.phase_seconds=[pscustomobject][ordered]@{inventory=$x.phase_seconds.inventory;allocation=$x.phase_seconds.allocation;locks=$x.phase_seconds.locks}}
  4 {$x.expected_files='5'}
  5 {$x.completed=[int]0}
  6 {$x.phase_seconds.allocation='0.125'}
  7 {$x.phase_seconds.allocation=[double]::NaN}
  8 {$x.phase_seconds.allocation=[double]-0.001}
  9 {$x.phase_seconds.allocation=[double]20}
  10 {$x.PSObject.Properties.Remove('scratch_reserved_bytes')}
  11 {$x.PSObject.Properties.Add([Management.Automation.PSNoteProperty]::new('extra',[int]0))}
  12 {$x.private_receipt_digests.PSObject.Properties.Add([Management.Automation.PSNoteProperty]::new('unexpected','00'))}
  13 {$x.cleanup_unknown=$true}
  default {R75-Stop 'mutation'}
 }
 return $x
}
function R75-Baseline($Row,[bool]$Localize=$false) {
 $scope=$script:scopes[$Row.c+$Row.v];$idx=if($Row.n -cin @('DP','DF')){1}else{0}
 $obs=R75-Snapshot $Row.n;$pre=$Row.n -cin @('FP','DP')
 $r=R75-Raw $scope $obs $scope.Specs[$idx] $pre
 $Row.o=if($r.Thrown){'throw'}else{'pass'}
 if($r.Thrown){$Row.t=R75-Code $r.Er}
 if($Localize){
  if($r.Thrown){
   $detail=R75-Localize $r.Er $scope;$script:localizations.Add($detail)
   $Row.site=$detail.ml;$Row.want=if($Row.c -ceq 'W'){1041}else{207}
   $Row.bad=$detail.bad -or $detail.trunc;$Row.ok=$detail.complete
  }else{$Row.bad=$true}
 }else{$Row.ok=-not $r.Thrown}
 if(-not $Localize){R75-Need $Row.ok 'baseline'}
}
function R75-Shape($Row) {
 ;$scope=$script:scopes[$Row.c+$Row.v]
 $pre=$Row.n.StartsWith('P');$n=[int]$Row.n.Substring(1)
 $a=@();$b=@()
 if($n -ge 1){if($pre){$a=@('allocation')}else{$b=@('locks')}}
 if($n -eq 3){if($pre){$a=@('allocation','locks','inventory')}else{$b=@('locks','inventory','capture_s')}}
 $spec=[pscustomobject]@{PreKeys=$a;Phases=$b}
 $r=& $scope.Module {
  param([scriptblock]$assignment,$specification,[bool]$before)
  $s=$specification;$Pre=$before
  try{. $assignment;return ,([pscustomobject]@{Value=$keys;Er=$null})}catch{$original=$_;return ,([pscustomobject]@{Value=$null;Er=$original})}
 } $scope.Shape $spec $pre
 $script:evaluations++
 if($null -ne $r.Er){R75-Remember $r.Er;$Row.o='throw';$Row.t=R75-Code $r.Er;R75-Stop 'shape_assignment'}
 $value=$r.Value;$expected=@(if($pre){$a}else{@('allocation')+$b})
 $Row.shape=if($null -eq $value){0}elseif($value -is [object[]]){1}elseif($value -is [string]){2}else{3}
 $elements=@($value);if($null -eq $value){$elements=@()}
 $Row.len=$elements.Count
 R75-Need (($elements -join '|') -ceq ($expected -join '|') -and $elements.Count -eq $expected.Count) 'shape_order'
 if($Row.v -ceq 'C'){R75-Need ($Row.shape -eq 1) 'shape_array'}
 elseif($expected.Count -eq 0){R75-Need ($Row.shape -eq 0) 'old_shape'}
 elseif($expected.Count -eq 1){R75-Need ($Row.shape -eq 2) 'old_shape'}
 else{R75-Need ($Row.shape -eq 1) 'old_shape'}
 $countError=$null
 try{$Row.cnt=[int]$value.Count;$Row.o='pass'}catch{$countError=$_;R75-Remember $countError;$Row.o='throw';$Row.t=R75-Code $countError}
 if($Row.v -ceq 'C' -or $expected.Count -gt 1){R75-Need ($null -eq $countError -and $Row.cnt -eq $expected.Count) 'shape_count'}
 elseif($null -eq $countError){R75-Need ($Row.cnt -eq $expected.Count) 'old_shape_count'}
 else{R75-Need ($Row.t -in @(2,3) -and $countError.Exception.Message -cmatch 'Count') 'old_shape_count'}
 $Row.ok=$true;
}
function R75-Negative($Row) {
 $scope=$script:scopes[$Row.c+$Row.v]
 $wants=@(0,1041,1041,1041,1015,1011,1044,1045,1045,1045,1007,1037,1039,1011)
 $fn=if($Row.m -in @(4,5,10,13)){'Assert-Fields'}else{'Assert-CoreSnapshot'}
 $w=$scope.Map[$fn];$wm=@($manifest|Where-Object N -CEQ $fn)[0]
 $Row.want=$w.O+$wants[$Row.m]-$wm.W
 $r=R75-Raw $scope (R75-Mutate $Row.m) $scope.Specs[0] $true
 $Row.o=if($r.Thrown){'throw'}else{'pass'}
 if($r.Thrown){
  $Row.t=R75-Code $r.Er
  try{$frame=R75-Frame $r.Er $scope}catch{$Row.bad=$true;throw};$Row.site=$frame.Line
  $Row.ok=($Row.t -eq 1 -and $r.Er.Exception.Data.Contains('c2f_code') -and $r.Er.Exception.Data['c2f_code'] -ceq 'assertion' -and $frame.F -ceq $fn -and $frame.Line -eq $Row.want)
 }
 R75-Need $Row.ok 'negative_site'
}
function R75-Write {
 
 R75-Need (-not $script:inputUnknown -and $null -eq $script:inputCurrent -and $script:inputReturns -eq $script:inputClosed) 'input_cleanup'
 $ok=($script:primary -ceq 'none' -and $script:evaluations -eq 92 -and $script:checkerCalls -eq 68 -and $script:localizations.Count -eq 2 -and @($script:rows|Where-Object {-not $_.ok}).Count -eq 0)
 $value=[ordered]@{
  schema=1;attempt=$attempt;result=if($ok){'observed'}else{'failed_or_unknown'}
  primary_code=$script:primary;primary_type=$script:primaryType;primary_hresult=$script:primaryHr;other_type=$script:otherType;stage=$script:stage
  evaluations=$script:evaluations;checker_calls=$script:checkerCalls;matrix=[ordered]@{shape=24;baseline=16;negative=52;total=92}
  input_returns=$script:inputReturns;input_closed=$script:inputClosed;input_unknown=$script:inputUnknown
  synthetic_directory_final=$true;all38_assertions_retained=$true;original_r73_error_reconstructed=$false
  type_table=@('none','System.InvalidOperationException','System.Management.Automation.PropertyNotFoundException','System.Management.Automation.RuntimeException','unexpected')
  localizations=@($script:localizations.ToArray());rows=@($script:rows.ToArray())
 }
 $json=ConvertTo-Json -InputObject $value -Depth 12 -Compress
 $bytes=[Text.UTF8Encoding]::new($false,$true).GetBytes($json+[char]10)
 R75-Need ($bytes.Length -le 49152) 'report_cap'
 ;$script:outputAttempted=$true
 try{
  $script:outputCurrent=[IO.FileStream]::new($reportPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
  $script:outputCurrent.Write($bytes,0,$bytes.Length);$script:outputCurrent.Flush($true);$script:reportWritten=$true
 }finally{
  if($script:outputCurrent){
   try{$script:outputCurrent.Dispose();$script:outputClosed=$true;$script:outputCurrent=$null}catch{$script:inputUnknown=$true;throw}
  }
 }
 
 R75-Need ($script:outputClosed -and $script:reportWritten) 'output_cleanup'
 return $ok
}

# Fixed order: two old directory replays;24 shape;14 remaining baseline;52 negative.
foreach($cp in @('W','S')){$null=R75-NewRow 'B' $cp 'O' 'DP'}
foreach($cp in @('W','S')){foreach($vr in @('O','C')){foreach($nm in @('P0','P1','P3','F0','F1','F3')){$null=R75-NewRow 'S' $cp $vr $nm}}}
foreach($cp in @('W','S')){foreach($vr in @('O','C')){foreach($nm in @('FP','FF','DP','DF')){
 if(-not($vr -ceq 'O' -and $nm -ceq 'DP')){$null=R75-NewRow 'B' $cp $vr $nm}
}}}
foreach($cp in @('W','S')){foreach($vr in @('O','C')){for($mutation=1;$mutation -le 13;$mutation++){$null=R75-NewRow 'N' $cp $vr 'FP' $mutation}}}
$success=$false
try{
 R75-Need ($script:rows.Count -eq 92 -and $PSCommandPath -ceq $check -and $PSVersionTable.PSVersion.ToString() -ceq '7.6.5') 'entry'
 $script:stage='inputs'
 $wb=R75-Read ($oldPrefix+'.preflight.ps1') 110834 '2c375e0092ba96784ae72ccaa847e30fe851f3c8c8f26f26108812d7384aac00'
 $sb=R75-Read ($oldPrefix+'.watchdog.ps1') 35543 '2f4f3fe968d20ef3bd332c853b8c88d8f3b2e673b94f0ba601ce4b3b8a5b34f4'
 $jb=R75-Read ($oldPrefix+'.cases.json') 20429 'b7d1be3b9a653d88cff607b76b80c4c3d3cb743e0d7d18c6d9243e3e5ea6dbb4'
 
 $script:observed=ConvertFrom-Json -InputObject ([Text.UTF8Encoding]::new($false,$true).GetString($jb))
 
 R75-Need ($script:observed.attempt -ceq 'C-s53-cold-core-named-streams-01-attempt-01' -and $script:observed.result -ceq 'failed_or_unknown' -and $script:observed.cases.Count -eq 2 -and
  $script:observed.cases[0].name -ceq 'file-stream' -and $script:observed.cases[1].name -ceq 'directory-stream') 'public_report'
 $script:stage='extraction'
 foreach($cp in @('W','S')){
  $text=if($cp -ceq 'W'){[Text.UTF8Encoding]::new($false,$true).GetString($wb)}else{[Text.UTF8Encoding]::new($false,$true).GetString($sb)}
  $path=if($cp -ceq 'W'){$oldPrefix+'.preflight.ps1'}else{$oldPrefix+'.watchdog.ps1'}
  foreach($vr in @('O','C')){$script:scopes[$cp+$vr]=R75-Scope $cp $vr $text $path}
 }
 $script:stage='localization'
 R75-Baseline $script:rows[0] $true
 R75-Baseline $script:rows[1] $true
 R75-Need ($script:checkerCalls -eq 2 -and $script:localizations.Count -eq 2 -and $script:rows[0].ok -and $script:rows[1].ok) 'localization'
 for($index=2;$index -lt 92;$index++){
  ;$r=$script:rows[$index]
  switch -CaseSensitive ($r.g){
   'S' {$script:stage='shape';R75-Shape $r}
   'B' {$script:stage='baseline';R75-Baseline $r}
   'N' {$script:stage='negative';R75-Negative $r}
   default {R75-Stop 'row_kind'}
  }
  
 }
 R75-Need ($script:evaluations -eq 92 -and $script:checkerCalls -eq 68 -and @($script:rows|Where-Object {-not $_.ok}).Count -eq 0) 'matrix'
 $script:stage='complete';$script:primary='none'
}catch{
 $original=$_
 try{
  $script:primary=if($original.Exception.Data.Contains('r75_code')){[string]$original.Exception.Data['r75_code']}else{'unexpected'}
  if($script:primary -cnotmatch '^[a-z_]{1,32}$'){$script:primary='unexpected'}
  $script:primaryType=R75-SafeType $original.Exception;$script:primaryHr=[int]$original.Exception.HResult
 }catch{$script:primary='capture_failure';$script:primaryType='';$script:primaryHr=0}
}
try{$success=R75-Write}catch{
 $originalWrite=$_
 $success=$false
 # The original ErrorRecord remains private. Export only a small safe type/HRESULT marker.
 try{
  $writeType=R75-SafeType $originalWrite.Exception;$writeHr=[int]$originalWrite.Exception.HResult
   [Console]::Error.WriteLine('{"classification":"r75_output_failure","type":"'+$writeType+'","hresult":'+[string]$writeHr+'}')
 }catch{}
}
if($success){exit 0}
exit 1
