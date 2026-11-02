# Ignored read-only candidate microbenchmark. Never installs candidate in production.
function Get-ResidualNativePath([string]$Path,[switch]$AllowMissing) {
    if(-not [IO.Path]::IsPathRooted($Path) -or $Path.Length -gt 1024 -or $Path -match '(^|[\\/])\.\.?([\\/]|$)'){throw 'S54 absolute path/components.'}
    $full=[IO.Path]::GetFullPath($Path);$probe=$full
    while(-not (Test-Path -LiteralPath $probe)) {
        if(-not $AllowMissing){throw 'S54 missing public path.'}
        $probe=Split-Path -Parent $probe;if(-not $probe){throw 'S54 missing ancestor.'}
    }
    if($probe -cne $full -and -not (Get-Item -LiteralPath $probe -Force).PSIsContainer){throw 'S54 missing suffix requires a directory.'}
    for($p=$probe;$p;$p=[IO.Path]::GetDirectoryName([IO.Path]::TrimEndingDirectorySeparator($p))){
        if([IO.File]::GetAttributes($p) -band [IO.FileAttributes]::ReparsePoint){throw 'S54 reparse ancestor.'}
    }
    return $full
}
function Invoke-ResidualPathOutcome($Body,[string]$Path,[bool]$Missing) {
    try { $v=& $Body $Path -AllowMissing:$Missing; return @{accepted=$true;value=$v;error=$null} }
    catch { return @{accepted=$false;value=$null;error=$_.Exception.Message} }
}
Pin-History
$reference=(Get-Item Function:Get-S54Path).ScriptBlock
$candidate=(Get-Item Function:Get-ResidualNativePath).ScriptBlock
$corpus=[Collections.Generic.List[object]]::new()
function Add-ResidualPaths($Value,[string]$Type,[string]$Group) {
    if($null -eq $Value){return}
    if($Type -ceq 'p'){$corpus.Add(@{path=[string]$Value;allow_missing=$true;group=$Group});return}
    if(-not $script:S54Schema.ContainsKey($Type)){return}
    foreach($k in $script:S54Schema[$Type].Keys){
        $d=$script:S54Schema[$Type][$k];$v=$Value[$k]
        if($d.StartsWith('n')){if($null -eq $v){continue};$d=$d.Substring(1)}
        if($d.StartsWith('a')){foreach($e in $v){Add-ResidualPaths $e $d.Substring(1) $Group}}
        else {Add-ResidualPaths $v $d $Group}
    }
}
foreach($role in @('normal','tests')){
    $capture=Read-S54Json $ResidualContext[$role] 8388608
    Add-ResidualPaths $capture 'CompilerCapture' ($role+'/wire')
    foreach($line in $capture.raw_compiler_artifact_json){
        $message=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($line)) 131072
        Add-ResidualPaths $message 'CargoArtifact' ($role+'/raw-wire')
        foreach($p in @($message.manifest_path,$message.target.src_path)){$corpus.Add(@{path=$p;allow_missing=$false;group=$role+'/required-input'})}
    }
}
foreach($p in $ResidualSources){$corpus.Add(@{path=$p.path;allow_missing=$false;group='source'})}
$corpusHash=Get-S54Sha ([Text.Encoding]::UTF8.GetBytes((ConvertTo-KilogramFrozenJson @($corpus))))
$runs=[Collections.Generic.List[object]]::new();$referenceValues=[Collections.Generic.List[string]]::new()
foreach($mode in @('reference','native-candidate')){
    $body=if($mode -ceq 'reference'){$reference}else{$candidate}
    $timer=[Diagnostics.Stopwatch]::StartNew();$n=0
    foreach($r in $corpus){
        $value=& $body $r.path -AllowMissing:$r.allow_missing
        if($mode -ceq 'reference'){$referenceValues.Add($value)}elseif($value -cne $referenceValues[$n]){throw 'Corpus path result difference.'}
        $n++
    }
    $timer.Stop();$runs.Add(@{mode=$mode;calls=$n;seconds=$timer.Elapsed.TotalSeconds;all_accepted=$true;output_equal=$true})
}
# Compare parent observation sequence with no filesystem caching or fixture copying.
$unique=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$chains=0;$observations=0
foreach($value in $referenceValues){
    if(-not $unique.Add($value)){continue}
    $a=$value;$b=$value
    while($a -or $b){
        if($a -cne $b){throw 'Parent observation sequence difference.'}
        $observations++;$a=Split-Path -Parent $a;$b=[IO.Path]::GetDirectoryName([IO.Path]::TrimEndingDirectorySeparator($b))
    };$chains++
}
function Assert-ResidualFreshDestination([string]$Name){
    $p=Get-S54Path (Join-Path $base $Name) -AllowMissing
    if(-not $p.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical owned probe destination.'};return $p
}
$fixture=Assert-ResidualFreshDestination 'tiny-public-01';$null=New-Item -ItemType Directory -Path $fixture
$dir=Assert-ResidualFreshDestination 'tiny-public-01/plain';$null=New-Item -ItemType Directory -Path $dir
$file=Assert-ResidualFreshDestination 'tiny-public-01/plain/bracket[1].txt';[IO.File]::WriteAllText($file,'tiny public path probe',[Text.UTF8Encoding]::new($false))
$link=Assert-ResidualFreshDestination 'tiny-public-01/junction';$null=New-Item -ItemType Junction -Path $link -Target $dir
$cases=@(
    @{name='root';path=[IO.Path]::GetPathRoot($dir);missing=$false},
    @{name='directory';path=$dir;missing=$false},
    @{name='trailing-directory-separator';path=$dir+'\';missing=$false},
    @{name='forward-slashes';path=$file.Replace('\','/');missing=$false},
    @{name='case-alias';path=$file.ToUpperInvariant();missing=$false},
    @{name='literal-brackets';path=$file;missing=$false},
    @{name='missing-required';path=$dir+'\absent';missing=$false},
    @{name='missing-optional';path=$dir+'\absent\nested';missing=$true},
    @{name='file-as-missing-parent';path=$file+'\absent';missing=$true},
    @{name='junction-leaf';path=$link;missing=$false},
    @{name='junction-ancestor';path=$link+'\bracket[1].txt';missing=$false},
    @{name='relative';path='relative';missing=$true},
    @{name='dot-component';path=$dir+'\.\x';missing=$true},
    @{name='dotdot-component';path=$dir+'\..\x';missing=$true},
    @{name='overlong';path=$dir+'\'+('a'*1025);missing=$true},
    @{name='empty';path='';missing=$true}
)
$outcomes=[Collections.Generic.List[object]]::new()
foreach($c in $cases){
    $a=Invoke-ResidualPathOutcome $reference $c.path $c.missing;$b=Invoke-ResidualPathOutcome $candidate $c.path $c.missing
    if((ConvertTo-KilogramFrozenJson $a) -cne (ConvertTo-KilogramFrozenJson $b)){throw ('Tiny path outcome difference: '+$c.name)}
    $outcomes.Add(@{name=$c.name;reference=$a;candidate=$b;equal=$true})
}
# A successful earlier query never grants later access through a new junction.
$mutable=Assert-ResidualFreshDestination 'tiny-public-01/mutable';$null=New-Item -ItemType Directory -Path $mutable
$a=Invoke-ResidualPathOutcome $reference $mutable $false;$b=Invoke-ResidualPathOutcome $candidate $mutable $false
if(-not $a.accepted -or -not $b.accepted){throw 'Fresh mutation precondition.'}
$moved=Assert-ResidualFreshDestination 'tiny-public-01/mutable-retained'
$null=Get-S54Path $mutable;Move-Item -LiteralPath $mutable -Destination $moved
$null=Assert-ResidualFreshDestination 'tiny-public-01/mutable';$null=New-Item -ItemType Junction -Path $mutable -Target $dir
$a=Invoke-ResidualPathOutcome $reference $mutable $false;$b=Invoke-ResidualPathOutcome $candidate $mutable $false
if($a.accepted -or $b.accepted -or $a.error -cne 'S54 reparse ancestor.' -or $b.error -cne $a.error){throw 'Fresh reparse mutation refusal.'}
$outcomes.Add(@{name='mutation-between-queries';reference=$a;candidate=$b;equal=$true})
# These predicates stay untouched; verify candidate composition preserves their refusals.
$strictCases=@(
    @{name='duplicate-json-key';body={ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes('{"x":1,"x":2}')) 4096}},
    @{name='case-sensitive-object-key';body={Assert-S54Wire ([ordered]@{Path=$file;bytes=1;sha256=('0'*64)}) 'Pin'}},
    @{name='bool-as-integer';body={Assert-S54Wire ([ordered]@{path=$file;bytes=$true;sha256=('0'*64)}) 'Pin'}},
    @{name='integer-as-bool';body={Assert-S54Value 1 'b'}},
    @{name='negative-uint';body={Assert-S54Value -1 'u'}},
    @{name='string-bound';body={Assert-S54Value ('x'*1025) 's'}},
    @{name='raw-byte-bound';body={Assert-S54Value ('x'*131073) 'raw'}},
    @{name='json-byte-bound';body={ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes('[1]')) 2}},
    @{name='array-order';body={Assert-S54Equal @('a','b') @('b','a')}},
    @{name='null-versus-empty-array';body={Assert-S54Equal $null @()}},
    @{name='number-versus-bool';body={Assert-S54Equal 1 $true}},
    @{name='duplicate-canonical-source-path';body={New-S54CompilerSourceIndex @(@{path=$file},@{path=$file.ToUpperInvariant()})}},
    @{name='array-count-bound';body={Assert-S54Wire @{kind=@('x')*4097;crate_types=@('x');name='x';src_path=$file;edition='2021';doc=$true;doctest=$true;test=$true} 'CargoTarget'}}
)
$strictOutcomes=[Collections.Generic.List[object]]::new()
try{
    foreach($c in $strictCases){
        Set-Item Function:Get-S54Path $reference
        $a=try{$null=& $c.body;@{accepted=$true;error=$null}}catch{@{accepted=$false;error=$_.Exception.Message}}
        Set-Item Function:Get-S54Path $candidate
        $b=try{$null=& $c.body;@{accepted=$true;error=$null}}catch{@{accepted=$false;error=$_.Exception.Message}}
        if($a.accepted -or $b.accepted -or $a.error -cne $b.error){throw ('Unchanged refusal difference: '+$c.name)}
        $strictOutcomes.Add(@{name=$c.name;reference=$a;candidate=$b;equal=$true})
    }
}finally{Set-Item Function:Get-S54Path $reference}
Pin-History
$groups=@($corpus|Group-Object group|ForEach-Object {@{group=$_.Name;calls=$_.Count}})
Write-OwnedResidual 'probe-01.result.json' @{classification='isolated path ancestor primitive prototype; not full admission/runtime';corpus_sha256=$corpusHash;corpus_calls=$corpus.Count;groups=$groups;runs=@($runs);unique_parent_chains=$chains;parent_observations=$observations;parent_sequences_equal=$true;path_cases=@($outcomes);unchanged_refusal_cases=@($strictOutcomes);cache_used=$false;live_walk_every_call=$true;accepted_source_unchanged=$true;source_capture_image_and_failed_F01_pins_preserved=$true;historical_fixtures_written=$false;runtime='not_established'}
Write-Output 'path_primitive_corpus_and_fresh_tiny_refusals=passed;production_source_untouched=true'
