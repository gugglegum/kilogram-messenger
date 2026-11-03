[CmdletBinding()]
param([Parameter(Mandatory)][string]$FixtureIndex,[Parameter(Mandatory)][string]$EvidenceDirectory,[switch]$CompilerIndexOnly,[switch]$PathOnly,[switch]$HeadOnly,[switch]$CompilerFilenameOnly)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1')
$index=ConvertFrom-S54Strict (Read-S54Public $FixtureIndex 16384) 16384
if($CompilerFilenameOnly){
    $workspace=Split-Path -Parent $PSScriptRoot;$evidence=Get-S54Path $EvidenceDirectory
    if(-not $evidence.StartsWith((Get-S54Path (Join-Path $workspace '.tmp/s54-block4n-compiler-parity-01'))+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or $index.classification -cne 'public_schema_fixture_only'){throw 'Fresh public compiler filename fixture required.'}
    $policy=New-S54Policy -FixtureRoot $index.root
    $i=Read-S54Json $index.inputs.inventory 2097152
    $original=Read-S54Json $i.compiler.normal 8388608
    $checks=[Collections.Generic.List[object]]::new()
    function Copy-FilenamePublic($Value){ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 100 -Compress))) 8388608}
    function Check-Filename([string]$Name,$Capture,[bool]$Refusal){
        $refused=$false;try{Assert-S54Compiler $i $policy $Capture $true}catch{$refused=$true}
        if($refused -ne $Refusal){throw ('Shared PS filename compiler outcome/'+$Name)}
        $checks.Add(@{name=$Name;passed=$true;expected_refusal=$Refusal})
    }
    Check-Filename 'actual-shaped PDB before EXE' $original $false
    $c=Copy-FilenamePublic $original;[Array]::Reverse($c.units[0].filenames)
    Check-Filename 'supplied filename permutation' $c $false
    $raw=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($c.raw_compiler_artifact_json[0])) 131072
    [Array]::Reverse($raw.filenames);$c.raw_compiler_artifact_json[0]=ConvertTo-Json -InputObject $raw -Depth 100 -Compress
    Check-Filename 'raw filename permutation' $c $false
    foreach($mutation in @('missing','extra','duplicate','changed','profile','guard','fresh','features','executable','raw_duplicate','unit_duplicate')){
        $c=Copy-FilenamePublic $original
        $changed=[IO.Path]::Combine((Split-Path -Parent $c.units[0].executable),'changed-output.pdb')
        switch($mutation){
            'missing' {$c.units[0].filenames=@($c.units[0].filenames[0])}
            'extra' {$c.units[0].filenames+=@($changed)}
            'duplicate' {$c.units[0].filenames+=@($c.units[0].filenames[0])}
            'changed' {$c.units[0].filenames[0]=$changed}
            'profile' {$c.units[0].profile.opt_level='2'}
            'guard' {$c.frozen_guard_verified=$true}
            'fresh' {$c.units[0].fresh=$false}
            'features' {$c.units[0].features=@('unproved-feature')}
            'executable' {$c.units[0].executable=$changed}
            'raw_duplicate' {$raw=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($c.raw_compiler_artifact_json[0])) 131072;$raw.filenames+=@($raw.filenames[0]);$c.raw_compiler_artifact_json[0]=ConvertTo-Json -InputObject $raw -Depth 100 -Compress}
            'unit_duplicate' {$c.units+=@($c.units[0])}
        }
        Check-Filename $mutation $c $true
    }
    Write-KilogramFrozenJson (Join-Path $evidence 'compiler-filename-results.json') @{classification='fresh public shared PS compiler filename collection checks only';checks=@($checks);count=$checks.Count;runtime_admission=$false;production_Run=$false}
    Write-Output ('compiler_filename_checks='+$checks.Count)
    return
}
if($HeadOnly){
    $workspace=Split-Path -Parent $PSScriptRoot;$evidence=Get-S54Path $EvidenceDirectory
    if(-not $evidence.StartsWith((Join-Path $workspace '.tmp/s54-block4h-head-repair-01/'),[StringComparison]::OrdinalIgnoreCase) -or $index.classification -cne 'fresh_public_HEAD_checks'){throw 'Owned fresh HEAD checks required.'}
    $checks=[Collections.Generic.List[object]]::new();$roots=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $global:HHeadReads=[Collections.Generic.List[object]]::new();$global:HHeadFault='';$global:HHeadFaultPath=''
    $global:HHeadRealRead=(Get-Item Function:Read-S54Public).ScriptBlock
    $global:HHeadReal=(Get-Item Function:Get-S54SourceHead).ScriptBlock
    function Require-Head([bool]$Value,[string]$Name,$Detail=$null){if(-not $Value){throw $Name};$checks.Add(@{name=$Name;passed=$true;detail=$Detail})}
    function Owned-HeadPath([string]$Root,[string]$Relative){
        if(-not $roots.Contains($Root)){throw 'Only roots minted in this test may be written.'}
        $p=Get-S54Path (Join-Path $Root $Relative) -AllowMissing
        if(-not $p.StartsWith($Root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Owned HEAD fixture write only.'};return $p
    }
    function Write-HeadFile([string]$Root,[string]$Relative,$Data){
        $p=Owned-HeadPath $Root $Relative;$parent=Split-Path -Parent $p
        $null=[IO.Directory]::CreateDirectory($parent);$p=Owned-HeadPath $Root $Relative
        [byte[]]$raw=[byte[]]::new(0)
        if($Data -is [byte[]]){$raw=$Data}else{$raw=[Text.Encoding]::UTF8.GetBytes([string]$Data)}
        [IO.File]::WriteAllBytes($p,$raw);return $p
    }
    function New-HeadCase([string]$Name,$Head,$Loose=$null,$Packed=$null){
        $p=Get-S54Path (Join-Path $evidence $Name) -AllowMissing
        if(-not $p.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh HEAD root required.'}
        $null=New-Item -ItemType Directory -Path $p;$null=$roots.Add($p)
        $null=Write-HeadFile $p '.git/HEAD' $Head
        if($null -ne $Loose){$null=Write-HeadFile $p '.git/refs/heads/master' $Loose}
        if($null -ne $Packed){$null=Write-HeadFile $p '.git/packed-refs' $Packed}
        return $p
    }
    function Read-S54Public([string]$Path,[long]$Cap){
        $global:HHeadReads.Add(@{path=$Path;cap=$Cap})
        if([string]::Equals([IO.Path]::GetFullPath($Path),$global:HHeadFaultPath,[StringComparison]::OrdinalIgnoreCase)){
            if($global:HHeadFault -ceq 'read-denied'){throw [UnauthorizedAccessException]::new('injected HEAD read denied')}
            if($global:HHeadFault -ceq 'read-io'){throw [IO.IOException]::new('injected HEAD read IO failure')}
            if($global:HHeadFault -ceq 'appearing-loose'){$global:HHeadFault='';$null=Write-HeadFile $global:HHeadMutationRoot '.git/refs/heads/master' 'malformed appearing loose'}
        }
        $value=& $global:HHeadRealRead $Path $Cap;return ,$value
    }
    function Invoke-HeadCheck([string]$Name,[string]$Root,[string]$Expected,[string]$ErrorPattern='',[switch]$NoPacked,[scriptblock]$Reader=$global:HHeadReal){
        $global:HHeadReads.Clear();$value=$null;$why=$null
        try{$value=& $Reader $Root}catch{$why=$_.Exception.Message}
        $pass=if($ErrorPattern){$null -ne $why -and $why -match $ErrorPattern}else{$null -eq $why -and $value -ceq $Expected}
        $packedReads=@($global:HHeadReads|Where-Object {$_.path.EndsWith('packed-refs',[StringComparison]::Ordinal)}).Count
        Require-Head ($pass -and (-not $NoPacked -or $packedReads -eq 0)) $Name @{value=$value;error=$why;packed_reads=$packedReads;read_caps=@($global:HHeadReads|ForEach-Object {$_.cap}|Sort-Object -Unique);instrumented_reads=$true}
    }
    $a='a'*40;$b='b'*40;$sym='ref: refs/heads/master';$record=$a+' refs/heads/master'
    try{
        Invoke-HeadCheck 'detached trim' (New-HeadCase 'detached' (" `r`n"+$a+" `n")) $a
        Invoke-HeadCheck 'loose valid' (New-HeadCase 'loose' $sym $a) $a -NoPacked
        Invoke-HeadCheck 'loose wins over stale packed' (New-HeadCase 'loose-wins' $sym $a ($b+' refs/heads/master')) $a -NoPacked
        Invoke-HeadCheck 'packed valid with missing parent dirs' (New-HeadCase 'packed' $sym $null ($record+"`n")) $a
        Invoke-HeadCheck 'branch subpath' (New-HeadCase 'subpath' 'ref: refs/heads/topic/branch' $null ($a+' refs/heads/topic/branch')) $a
        $lines=@('# pack-refs with: peeled sorted','# refs/heads/master',('^'+$b),($b+' refs/tags/v1'),($b+' refs/heads/MASTER'),$record)
        Invoke-HeadCheck 'comments peeled unrelated case-different refs ignored' (New-HeadCase 'comments' $sym $null ($lines -join "`n")) $a
        foreach($row in @(@('short',('a'*39)),@('long',('a'*41)),@('uppercase',('A'*40)),@('bad-hex',('g'*40)),@('empty',''),@('symbolic-chain','ref: refs/heads/other'))){
            Invoke-HeadCheck ('present malformed loose never falls back/'+$row[0]) (New-HeadCase ('bad-loose-'+$row[0]) $sym $row[1] $record) '' 'source HEAD digest' -NoPacked
        }
        foreach($row in @(@('detached-short',('a'*39)),@('detached-upper',('A'*40)),@('detached-long',('a'*41)))){Invoke-HeadCheck $row[0] (New-HeadCase $row[0] $row[1]) '' 'source HEAD digest'}
        foreach($row in @(
            @('missing-packed',$null,'missing public path'),@('no-match',($b+' refs/heads/other'),'exact packed'),
            @('case-only',($a+' refs/heads/MASTER'),'exact packed'),@('duplicate',($record+"`n"+$record),'exact packed'),
            @('malformed-duplicate',($record+"`nnot-a-digest refs/heads/master"),'exact packed'),
            @('empty-digest-duplicate',($record+"`n refs/heads/master"),'exact packed'),@('invalid-packed-digest','invalid refs/heads/master','source HEAD digest')
        )){Invoke-HeadCheck $row[0] (New-HeadCase $row[0] $sym $null $row[1]) '' $row[2]}
        Invoke-HeadCheck 'HEAD 4096 accepted after trim' (New-HeadCase 'HEAD-cap' ((' '*4056)+$a)) $a
        Invoke-HeadCheck 'HEAD over cap' (New-HeadCase 'HEAD-over-cap' (' '*4097)) '' 'public file bound'
        Invoke-HeadCheck 'loose 4096 accepted after trim' (New-HeadCase 'loose-cap' $sym ((' '*4056)+$a)) $a -NoPacked
        Invoke-HeadCheck 'loose over cap refuses packed fallback' (New-HeadCase 'loose-over-cap' $sym (' '*4097) $record) '' 'public file bound' -NoPacked
        $packedAtCap='#'+('x'*(1048576-($record.Length+3)))+"`n"+$record+"`n"
        Require-Head ([Text.Encoding]::UTF8.GetByteCount($packedAtCap) -eq 1048576) 'packed cap fixture exact byte count'
        Invoke-HeadCheck 'packed at cap' (New-HeadCase 'packed-cap' $sym $null $packedAtCap) $a
        Invoke-HeadCheck 'packed over cap' (New-HeadCase 'packed-over-cap' $sym $null ($packedAtCap+'x')) '' 'public file bound'
        Invoke-HeadCheck 'invalid UTF8 HEAD' (New-HeadCase 'utf8-head' ([byte[]]@(255))) '' 'UTF|translate|Unable|decode'
        Invoke-HeadCheck 'invalid UTF8 loose does not fall back' (New-HeadCase 'utf8-loose' $sym ([byte[]]@(255)) $record) '' 'UTF|translate|Unable|decode' -NoPacked
        Invoke-HeadCheck 'invalid UTF8 packed' (New-HeadCase 'utf8-packed' $sym $null ([byte[]]@(255))) '' 'UTF|translate|Unable|decode'
        foreach($ref in @('refs/tags/master','refs/heads/','refs/heads/topic/','refs/heads//topic','refs/heads/../master','refs/heads/./master','refs/heads/master:stream','refs/heads/topic\branch','C:/external')){
            $name='bad-ref-'+$checks.Count
            Invoke-HeadCheck $name (New-HeadCase $name ('ref: '+$ref) $null $record) '' 'HEAD ref|absolute path/components' -NoPacked
        }
        $directory=New-HeadCase 'loose-directory' $sym $null $record
        $null=New-Item -ItemType Directory -Path (Owned-HeadPath $directory '.git/refs/heads/master')
        Invoke-HeadCheck 'loose directory refuses packed' $directory '' 'nonreparse file' -NoPacked
        $worktree=New-HeadCase 'gitdir-file' $a
        $gitDir=Get-S54Path (Join-Path $worktree '.git');$heldGit=Owned-HeadPath $worktree 'git-held'
        [IO.Directory]::Move($gitDir,$heldGit);$null=Write-HeadFile $worktree '.git' 'gitdir: C:/not-supported'
        Invoke-HeadCheck 'external gitdir file unsupported without following' $worktree '' 'Git directory required' -NoPacked
        $packedDirectory=New-HeadCase 'packed-directory' $sym
        $null=New-Item -ItemType Directory -Path (Owned-HeadPath $packedDirectory '.git/packed-refs')
        Invoke-HeadCheck 'packed directory is fatal' $packedDirectory '' 'public file bound'
        foreach($dangling in @($false,$true)){
            $r=New-HeadCase ('reparse-'+$dangling) $sym $null $record
            $target=Owned-HeadPath $r 'target';$null=New-Item -ItemType Directory -Path $target
            $loose=Owned-HeadPath $r '.git/refs/heads/master';$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $loose));$null=Owned-HeadPath $r '.git/refs/heads/master'
            $null=New-Item -ItemType Junction -Path $loose -Target $target
            if($dangling){$held=Owned-HeadPath $r 'target-held';$null=Get-S54Path $target;[IO.Directory]::Move($target,$held)}
            Invoke-HeadCheck ('real loose reparse/dangling='+$dangling) $r '' 'reparse' -NoPacked
        }
        function Get-HeadAttributes([string]$Path){
            if([string]::Equals($Path,$global:HHeadFaultPath,[StringComparison]::OrdinalIgnoreCase)){
                if($global:HHeadFault -ceq 'attribute-denied'){throw [UnauthorizedAccessException]::new('injected attributes denied')}
                if($global:HHeadFault -ceq 'attribute-io'){throw [IO.IOException]::new('injected attributes IO failure')}
            };return [IO.File]::GetAttributes($Path)
        }
        $attributeReader=[ScriptBlock]::Create($global:HHeadReal.ToString().Replace('[IO.File]::GetAttributes($Path)','(Get-HeadAttributes $Path)'))
        foreach($fault in @('attribute-denied','attribute-io','read-denied','read-io')){
            $r=New-HeadCase $fault $sym $a $record;$global:HHeadFault=$fault;$global:HHeadFaultPath=[IO.Path]::GetFullPath((Join-Path $r '.git/refs/heads/master'))
            Invoke-HeadCheck ('injected loose failure fatal/'+$fault) $r '' 'injected' -NoPacked -Reader $attributeReader
        }
        $r=New-HeadCase 'false-exists-denied' $sym $a $record;$global:HHeadFault='attribute-denied';$global:HHeadFaultPath=[IO.Path]::GetFullPath((Join-Path $r '.git/refs/heads/master'))
        function Test-Path {
            param([string]$LiteralPath)
            if([string]::Equals([IO.Path]::GetFullPath($LiteralPath),$global:HHeadFaultPath,[StringComparison]::OrdinalIgnoreCase)){return $false}
            Microsoft.PowerShell.Management\Test-Path -LiteralPath $LiteralPath
        }
        try{Invoke-HeadCheck 'injected Test-Path false plus denied attributes cannot authorize fallback' $r '' 'injected' -NoPacked -Reader $attributeReader}
        finally{Remove-Item Function:Test-Path}
        foreach($fault in @('read-denied','read-io')){
            $r=New-HeadCase ('packed-'+$fault) $sym $null $record;$global:HHeadFault=$fault;$global:HHeadFaultPath=[IO.Path]::GetFullPath((Join-Path $r '.git/packed-refs'))
            Invoke-HeadCheck ('injected packed failure fatal/'+$fault) $r '' 'injected'
        }
        $global:HHeadFault=''
        $r=New-HeadCase 'between-calls' $sym $a $record
        Invoke-HeadCheck 'fresh first loose' $r $a -NoPacked
        $null=Write-HeadFile $r '.git/refs/heads/master' $b
        Invoke-HeadCheck 'fresh second loose no cache' $r $b -NoPacked
        $r=New-HeadCase 'between-packed-calls' $sym $null $record
        Invoke-HeadCheck 'fresh first packed' $r $a
        $null=Write-HeadFile $r '.git/packed-refs' ($b+' refs/heads/master')
        Invoke-HeadCheck 'fresh second packed no cache' $r $b
        $null=Write-HeadFile $r '.git/refs/heads/master' 'bad'
        Invoke-HeadCheck 'new malformed loose defeats formerly valid packed' $r '' 'source HEAD digest' -NoPacked
        $r=New-HeadCase 'appearing-loose' $sym $null $record;$global:HHeadMutationRoot=$r;$global:HHeadFault='appearing-loose';$global:HHeadFaultPath=[IO.Path]::GetFullPath((Join-Path $r '.git/packed-refs'))
        Invoke-HeadCheck 'loose appears during packed read refuses' $r '' 'appeared during packed'
        $global:HHeadFault=''
        # Real admission on a newly constructed public F fixture; never F02 data.
        $f=Read-S54Json (Get-S54Pin $index.Fixture 16384) 16384
        if($f.root -cne $index.owned_root -or -not (Split-Path -Leaf $f.root).StartsWith('index-headrepair-',[StringComparison]::Ordinal)){throw 'Exact fresh admission fixture ownership required.'}
        $null=$roots.Add($f.root);$i=Read-S54Json $f.inputs.inventory 2097152
        $branch='refs/heads/branch/subpath'
        $null=Write-HeadFile $f.root '.git/HEAD' ('ref: '+$branch)
        $null=Write-HeadFile $f.root '.git/packed-refs' ($i.source.observed_head+' '+$branch+"`n")
        Set-S54Purpose FeasibilityF
        $policy=New-S54Policy -FixtureRoot $f.root
        $facts=Test-S54Admission $f.inputs $policy
        Require-Head ($facts.classification -ceq 'public_schema_fixture_only' -and $i.identity.attempt -ceq 'F-s54-mixed-hooks-feasibility-01-attempt-06') 'actual complete PS public packed F06 admission'
        $i.source.observed_head=$b;$i.source.implementation=$b;$why=$null
        try{Assert-S54Inventory $i $policy}catch{$why=$_.Exception.Message}
        Require-Head ($why -match 'S54 actual HEAD') 'actual PS admission refuses wrong observed packed HEAD' $why
        foreach($suffix in @('01','02','03','04','05')){
            $i.identity.attempt=$i.identity.case+'-attempt-'+$suffix;$why=$null
            try{Assert-S54Inventory $i $policy}catch{$why=$_.Exception.Message}
            Require-Head ($why -match 'required identity') ('actual inventory refuses consumed F'+$suffix) $why
        }
        # Early real preparation function, isolated prerequisite/test-only intercepts.
        $earlyRoots=@((New-HeadCase 'early-malformed' 'bad'),(New-HeadCase 'early-mismatch' $b),(New-HeadCase 'early-control' $a))
        $savedPolicy=(Get-Item Function:New-S54Policy).ScriptBlock;$savedPublished=(Get-Item Function:Assert-S54PublishedSources).ScriptBlock
        $savedGuard=(Get-Item Function:New-S54ImageGuard).ScriptBlock;$savedCompile=(Get-Item Function:Invoke-S54CompileStage).ScriptBlock
        function New-S54Policy {return @{Root=$global:HHeadEarlyRoot;Resolutions=@(@{name='pwsh.exe';path=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName})}}
        function Assert-S54PublishedSources {return ,@()}
        function git {$global:LASTEXITCODE=0;return $a}
        function New-Item {$global:HHeadReservations++;throw 'H_FORBIDDEN_RESERVATION'}
        function New-S54ImageGuard {$global:HHeadGuards++;throw 'H_FORBIDDEN_GUARD'}
        function Invoke-S54CompileStage {$global:HHeadCompiles++;throw 'H_FORBIDDEN_COMPILE'}
        try{
            for($n=0;$n -lt 3;$n++){
                $global:HHeadEarlyRoot=$earlyRoots[$n];$global:HHeadReservations=0;$global:HHeadGuards=0;$global:HHeadCompiles=0;$why=$null
                try{Invoke-S54Preparation ('c'*64)}catch{$why=$_.Exception.Message}
                $expected=if($n -eq 0){'source HEAD digest'}elseif($n -eq 1){'preparation actual HEAD differs'}else{'H_FORBIDDEN_RESERVATION'}
                Require-Head ($why -match $expected -and $global:HHeadReservations -eq $(if($n -eq 2){1}else{0}) -and $global:HHeadGuards -eq 0 -and $global:HHeadCompiles -eq 0 -and -not (Test-Path -LiteralPath (Join-Path $earlyRoots[$n] '.tmp'))) ('early prerequisite before actual mutation/'+$n) @{error=$why;intercepted_reservations=$global:HHeadReservations;guard_calls=$global:HHeadGuards;compile_calls=$global:HHeadCompiles;isolated_policy=$true}
            }
        }finally{
            Set-Item Function:New-S54Policy $savedPolicy;Set-Item Function:Assert-S54PublishedSources $savedPublished
            Set-Item Function:New-S54ImageGuard $savedGuard;Set-Item Function:Invoke-S54CompileStage $savedCompile
            Remove-Item Function:git,Function:New-Item
        }
        $realHead=Get-S54SourceHead $workspace;$gitHead=& git -C $workspace rev-parse HEAD
        Require-Head ($LASTEXITCODE -eq 0 -and $realHead -ceq $gitHead) 'read-only real checkout HEAD agrees with Git'
        $out=Get-S54Path (Join-Path $evidence 'head-results.json') -AllowMissing
        if(-not $out.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $out)){throw 'Fresh HEAD results required.'}
        Write-KilogramFrozenJson $out @{classification='focused HEAD/early-prerequisite/F04 public admission only';count=$checks.Count;checks=@($checks);fixture_roots=@($roots);production_prepare=$false;real_git_modified=$false;runtime='not_established'}
        Write-Output ('head_checks='+$checks.Count)
    }finally{
        Set-Item Function:Read-S54Public $global:HHeadRealRead;$global:HHeadFault=''
        if(-not (Test-Path -LiteralPath (Join-Path $evidence 'head-results.json'))){
            $partial=Get-S54Path (Join-Path $evidence 'head-partial.json') -AllowMissing
            if(-not $partial.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $partial)){throw 'Fresh owned partial HEAD result required.'}
            Write-KilogramFrozenJson $partial @{classification='partial HEAD cases before failed invocation; not passed suite';count=$checks.Count;checks=@($checks)}
        }
    }
    return
}
if($PathOnly){
    $evidence=Get-S54Path $EvidenceDirectory
    $workspace=Split-Path -Parent $PSScriptRoot
    $owned=Get-S54Path (Join-Path $workspace '.tmp/s54-block4f-path-implementation-01')
    if(-not $evidence.StartsWith($owned+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or $index.classification -cne 'public_path_primitive_fixture_set'){throw 'Fresh owned path regression evidence only.'}
    $script:PathCandidate=(Get-Item Function:Get-S54Path).ScriptBlock
    $null=Read-S54Pin $index.reference 134217728
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile($index.reference.path,[ref]$tokens,[ref]$errors)
    $fn=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Get-S54Path'},$false))[0]
    if($errors.Count){throw 'Published path reference parse.'}
    $referenceText='param('+(@($fn.Parameters|ForEach-Object {$_.Extent.Text}) -join ',')+")`n"+$fn.Body.Extent.Text.Substring(1,$fn.Body.Extent.Text.Length-2)
    $reference=[ScriptBlock]::Create($referenceText)
    $candidateText=$script:PathCandidate.ToString()
    $oldLoop=@($fn.Body.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.ForStatementAst]})[0].Extent.Text
    $normalizedCandidate=($candidateText.Replace("`r",'') -replace '(?m)^[ \t]+','')
    $normalizedOldLoop=($oldLoop.Replace("`r",'') -replace '(?m)^[ \t]+','')
    if(-not $normalizedCandidate.Contains($normalizedOldLoop,[StringComparison]::Ordinal)){throw 'Complete original fallback loop required.'}
    $checks=[Collections.Generic.List[object]]::new()
    function Require-Path([bool]$Pass,[string]$Name,$Detail=$null){if(-not $Pass){throw $Name};$checks.Add(@{name=$Name;passed=$true;detail=$Detail})}
    function New-PathDestination([string]$Relative){
        $p=& $script:PathCandidate (Join-Path $evidence $Relative) -AllowMissing
        if(-not $p.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical owned path fixture destination.'};return $p
    }
    function Path-Outcome($Body,[string]$Path,[bool]$Missing){
        try{$value=& $Body $Path -AllowMissing:$Missing;return @{accepted=$true;value=$value;error=$null}}
        catch{return @{accepted=$false;value=$null;error=$_.Exception.Message}}
    }
    $corpus=[Collections.Generic.List[object]]::new()
    function Add-PathCorpus($Value,[string]$Type){
        if($null -eq $Value){return}
        if($Type -ceq 'p'){$corpus.Add(@{path=[string]$Value;missing=$true});return}
        if(-not $script:S54Schema.ContainsKey($Type)){return}
        foreach($k in $script:S54Schema[$Type].Keys){
            $d=$script:S54Schema[$Type][$k];$v=$Value[$k]
            if($d.StartsWith('n')){if($null -eq $v){continue};$d=$d.Substring(1)}
            if($d.StartsWith('a')){foreach($e in $v){Add-PathCorpus $e $d.Substring(1)}}else{Add-PathCorpus $v $d}
        }
    }
    foreach($pin in $index.captures){
        $capture=Read-S54Json $pin 8388608;Add-PathCorpus $capture 'CompilerCapture'
        foreach($line in $capture.raw_compiler_artifact_json){
            $v=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($line)) 131072;Add-PathCorpus $v 'CargoArtifact'
            foreach($p in @($v.manifest_path,$v.target.src_path)){$corpus.Add(@{path=$p;missing=$false})}
        }
    }
    foreach($p in (Get-S54SourcePaths $workspace)){$corpus.Add(@{path=$p;missing=$false})}
    $script:PathTrace=[Collections.Generic.List[string]]::new();$script:PathNative=$false;$script:PathInjection=''
    function Invoke-PathAttribute([string]$Path){
        $script:PathTrace.Add($Path)
        if($script:PathInjection -cin @('missing','denied')){
            if($script:PathInjection -ceq 'denied'){throw [UnauthorizedAccessException]::new('injected denied native attributes')}
            throw [IO.FileNotFoundException]::new('injected missing native attributes')
        }
        if($script:PathInjection -ceq 'disappear' -and $Path -ceq $script:PathMutate){
            $script:PathInjection='';$null=& $script:PathCandidate $Path
            [IO.File]::Move($Path,(New-PathDestination 'disappeared-file-retained.txt'))
        }
        if($script:PathInjection -ceq 'junction' -and $Path -ceq $script:PathMutate){
            $script:PathInjection='';$null=& $script:PathCandidate $Path
            $held=New-PathDestination 'directory-before-junction-retained'
            [IO.Directory]::Move($Path,$held);$null=New-Item -ItemType Junction -Path $Path -Target $held -ErrorAction Stop
        }
        if($script:PathNative){return [IO.File]::GetAttributes($Path)}
        return (Get-Item -LiteralPath $Path -Force).Attributes
    }
    $oldTrace=[ScriptBlock]::Create($referenceText.Replace('(Get-Item -LiteralPath $p -Force).Attributes','(Invoke-PathAttribute $p)'))
    $newTrace=[ScriptBlock]::Create($candidateText.Replace('[IO.File]::GetAttributes($p)','(Invoke-PathAttribute $p)'))
    $positions=0
    foreach($row in $corpus){
        $script:PathTrace.Clear();$script:PathNative=$false;$a=& $oldTrace $row.path -AllowMissing:$row.missing;$before=$script:PathTrace.ToArray()
        $script:PathTrace.Clear();$script:PathNative=$true;$b=& $newTrace $row.path -AllowMissing:$row.missing
        if($a -cne $b -or ($before -join "`n") -cne ($script:PathTrace -join "`n")){throw 'Actual drive path outcome/attribute observation sequence mismatch.'}
        $positions+=$before.Count
    }
    Require-Path $true 'actual retained corpus path outputs and attribute sequences' @{calls=$corpus.Count;attribute_observations_each=$positions;instrumented=$true}
    Write-KilogramFrozenJson (New-PathDestination 'path-corpus.json') @{classification='completed corpus comparison; later cases may still fail';checks=@($checks)}
    $dir=New-PathDestination 'plain';$null=New-Item -ItemType Directory -Path $dir
    $file=New-PathDestination 'plain/bracket[1].txt';[IO.File]::WriteAllText($file,'public path fixture')
    $junction=New-PathDestination 'junction';$null=New-Item -ItemType Junction -Path $junction -Target $dir
    foreach($c in @(
        @('disk-root',[IO.Path]::GetPathRoot($dir),$false),@('directory',$dir,$false),@('trailing-separator',($dir+'\'),$false),
        @('forward-slash',$file.Replace('\','/'),$false),@('case-alias',$file.ToUpperInvariant(),$false),@('literal-brackets',$file,$false),
        @('required-missing',($dir+'\absent'),$false),@('optional-missing',($dir+'\absent\suffix'),$true),@('file-as-parent',($file+'\suffix'),$true),
        @('reparse-leaf',$junction,$false),@('reparse-ancestor',($junction+'\bracket[1].txt'),$false),@('relative','relative',$true),
        @('dot',($dir+'\.\x'),$true),@('dotdot',($dir+'\..\x'),$true),@('bound',($dir+'\'+('x'*1025)),$true),@('empty','',$true)
    )){
        $script:PathTrace.Clear();$script:PathNative=$false;$a=Path-Outcome $oldTrace $c[1] $c[2];$before=$script:PathTrace.ToArray()
        $script:PathTrace.Clear();$script:PathNative=$true;$b=Path-Outcome $newTrace $c[1] $c[2];$after=$script:PathTrace.ToArray()
        Require-Path ((ConvertTo-KilogramFrozenJson $a) -ceq (ConvertTo-KilogramFrozenJson $b) -and ($before -join "`n") -ceq ($after -join "`n")) ('real path outcome and attribute sequence/'+$c[0]) @{reference=$a;candidate=$b;reference_attributes=$before;candidate_attributes=$after}
    }
    # Isolated filesystem mocks: only lexical UNC/device walks, never network IO.
    $script:PathMockOps=[Collections.Generic.List[string]]::new()
    function Test-Path {param([string]$LiteralPath) $script:PathMockOps.Add('exists/'+$LiteralPath);return $true}
    function Get-Item {param([string]$LiteralPath,[switch]$Force) $script:PathMockOps.Add('attributes/'+$LiteralPath);return @{PSIsContainer=$true;Attributes=[IO.FileAttributes]::Directory}}
    function Split-Path {param([string]$Path,[switch]$Parent) $script:PathMockOps.Add('parent/'+$Path);Microsoft.PowerShell.Management\Split-Path -Path $Path -Parent}
    try{
        foreach($spelling in @('\\server\share','\\server\share\dir\','\\?\UNC\server\share\dir','\\?\C:\dir','\\.\C:\dir')){
            $script:PathMockOps.Clear();$a=Path-Outcome $reference $spelling $false;$before=$script:PathMockOps.ToArray()
            $script:PathMockOps.Clear();$b=Path-Outcome $script:PathCandidate $spelling $false;$after=$script:PathMockOps.ToArray()
            Require-Path ((-not $a.accepted -or $a.value -cnotmatch '^[A-Za-z]:\\') -and (ConvertTo-KilogramFrozenJson $a) -ceq (ConvertTo-KilogramFrozenJson $b) -and ($before -join "`n") -ceq ($after -join "`n")) ('unchanged fallback operations/'+$spelling) @{reference=$a;candidate=$b;reference_operations=$before;candidate_operations=$after;mocked_filesystem=$true;network_access=$false}
        }
        $unc='\\server\share\dir';$old=@();$native=@()
        for($p=$unc;$p;$p=Microsoft.PowerShell.Management\Split-Path -Path $p -Parent){$old+=,$p}
        for($p=$unc;$p;$p=[IO.Path]::GetDirectoryName([IO.Path]::TrimEndingDirectorySeparator($p))){$native+=,$p}
        Require-Path (($old -join '|') -cne ($native -join '|')) 'UNC regression catches unconditional native parent mismatch' @{old=$old;unconditional_native=$native;lexical_only=$true}
    }finally{Remove-Item Function:Test-Path,Function:Get-Item,Function:Split-Path}
    Write-KilogramFrozenJson (New-PathDestination 'path-before-faults.json') @{classification='completed drive and mocked fallback comparisons; fault cases pending';checks=@($checks)}
    foreach($fault in @('missing','denied')){
        $script:PathInjection=$fault;$script:PathNative=$true;$script:PathTrace.Clear()
        $outcome=Path-Outcome $newTrace $file $false
        Require-Path (-not $outcome.accepted -and $script:PathTrace.Count -eq 1 -and $outcome.error -match ('injected '+$fault)) ('injected native lookup failure is fatal/'+$fault) $outcome
    }
    $script:PathInjection=''
    $vanishing=New-PathDestination 'vanishing.txt';[IO.File]::WriteAllText($vanishing,'public disappearing file')
    $script:PathMutate=$vanishing;$script:PathInjection='disappear';$script:PathTrace.Clear()
    $outcome=Path-Outcome $newTrace $vanishing $false
    Require-Path (-not $outcome.accepted -and $script:PathTrace.Count -eq 1 -and -not (Test-Path -LiteralPath $vanishing)) 'real disappearance between exists and first attributes refuses' $outcome
    $mutable=New-PathDestination 'mutable';$null=New-Item -ItemType Directory -Path $mutable
    $leaf=New-PathDestination 'mutable/leaf.txt';[IO.File]::WriteAllText($leaf,'public junction mutation file')
    $script:PathMutate=$mutable;$script:PathInjection='junction';$script:PathTrace.Clear()
    $outcome=Path-Outcome $newTrace $leaf $false
    Require-Path (-not $outcome.accepted -and $outcome.error -ceq 'S54 reparse ancestor.' -and $script:PathTrace.Count -eq 2) 'real junction substitution between leaf and parent attributes refuses' $outcome
    $script:PathInjection='';$script:PathReadCalls=0
    function Get-S54Path([string]$Path,[switch]$AllowMissing){
        if($Path -ceq $file){$script:PathReadCalls++;if($script:PathReadCalls -eq 2){$script:PathInjection='denied'}}
        & $newTrace $Path -AllowMissing:$AllowMissing
    }
    try{
        $why=$null;try{$null=Read-S54Public $file 4096}catch{$why=$_.Exception.Message}
        Require-Path ($script:PathReadCalls -eq 2 -and $why -match 'injected denied') 'injected native lookup failure at post-read path check refuses' @{error=$why;path_calls=$script:PathReadCalls;injected=$true}
    }finally{Set-Item Function:Get-S54Path $script:PathCandidate;$script:PathInjection=''}
    $output=New-PathDestination 'path-results.json'
    Write-KilogramFrozenJson $output @{classification='focused actual path and isolated failure/fallback tests; not UNC interoperability or runtime';count=$checks.Count;checks=@($checks);network_access=$false;production_runtime=$false}
    Write-Output ('path_checks='+$checks.Count)
    return
}
if($CompilerIndexOnly){
    $evidence=Get-S54Path $EvidenceDirectory
    $workspace=Split-Path -Parent $PSScriptRoot
    $newPathEvidence=$evidence.StartsWith((Join-Path $workspace '.tmp/s54-block4f-path-implementation-01/'),[StringComparison]::OrdinalIgnoreCase)
    if((-not $newPathEvidence -and -not $evidence.StartsWith((Join-Path $workspace '.tmp/s54-block4e-index-implementation-01/'),[StringComparison]::OrdinalIgnoreCase)) -or $index.classification -cne 'public_compiler_index_fixture_set'){throw 'Fresh owned compiler-index fixtures only.'}
    $checks=[Collections.Generic.List[object]]::new()
    function Require-Index([bool]$Value,[string]$Name){if(-not $Value){throw $Name};$checks.Add(@{name=$Name;passed=$true})}
    function Refuse-Index([string]$Name,[scriptblock]$Action,[string]$Pattern){
        $why=$null;try{$null=& $Action}catch{$why=$_.Exception.Message}
        Require-Index ($null -ne $why -and $why -match $Pattern) "$Name : $why"
    }
    function Case-Index([string]$Name){
        $row=@($index.cases|Where-Object name -CEQ $Name)
        if($row.Count -ne 1){throw "One fresh fixture required: $Name"}
        $f=Read-S54Json (Get-S54Pin $row[0].index 16384) 16384
        if($f.classification -cne 'public_schema_fixture_only'){throw 'Public fixture scope required.'}
        $p=New-S54Policy -FixtureRoot $f.root
        if(-not (Split-Path -Leaf $p.Root).StartsWith('index-',[StringComparison]::Ordinal)){throw 'Fresh index fixture root required.'}
        if($newPathEvidence -and $p.Root -cne $row[0].root){throw 'Exact newly constructed fixture ownership required.'}
        $i=Read-S54Json $f.inputs.inventory 2097152
        Set-S54Purpose $(if($i.identity.case -ceq (Get-S54Purpose FeasibilityF).Case){'FeasibilityF'}else{'WholeP'})
        return @{root=$p.Root;policy=$p;i=$i;inputs=$f.inputs;normal=(Read-S54Json $i.compiler.normal 8388608);tests=(Read-S54Json $i.compiler.tests 8388608)}
    }
    function Owned-IndexWrite($Context,[string]$Path){
        $full=Get-S54Path $Path -AllowMissing
        if(-not $full.StartsWith($Context.root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Canonical owned public index write only.'};return $full
    }
    $invoke=Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1'
    $matches=@(Select-String -LiteralPath $invoke -SimpleMatch '$sourceIndex.Contains((Get-S54Path $path))')
    Require-Index ($matches.Count -eq 1) 'one production membership decision site'
    $script:IndexQueryLine=$matches[0].LineNumber;$script:IndexInvokePath=$invoke
    $script:IndexRealPath=(Get-Item Function:Get-S54Path).ScriptBlock
    $script:IndexRealRead=(Get-Item Function:Read-S54Pin).ScriptBlock
    $script:IndexRealBuild=(Get-Item Function:New-S54CompilerSourceIndex).ScriptBlock
    $script:IndexRealTail=(Get-Item Function:Assert-S54CompilerSourcesFresh).ScriptBlock
    $script:IndexMaps=[Collections.Generic.List[object]]::new()
    $script:IndexBuilding=$false;$script:IndexTailChecking=$false
    function Reset-Index($Context,$Capture,[string]$Trigger='',[string]$Mutation=''){
        $script:IndexContext=$Context;$script:IndexTrigger=$Trigger;$script:IndexMutation=$Mutation;$script:IndexMutated=$false
        $script:IndexBuilding=$false;$script:IndexTailChecking=$false
        $script:IndexCounts=@{build_calls=0;source_paths=0;queries=0;tail_calls=0;tail_reads=0;caps=@();tail_paths=@()}
        $script:IndexExpectedQueries=0
        foreach($line in $Capture.raw_compiler_artifact_json){
            $v=ConvertFrom-Json -InputObject $line -AsHashtable
            if(([IO.Path]::GetFullPath($v.manifest_path)).StartsWith($Context.root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){$script:IndexExpectedQueries+=2}
        }
    }
    function Mutate-Index {
        $ctx=$script:IndexContext
        $queried=Join-Path $ctx.root 'apps/kilogram-cli/tests/support/migration_s54_p5.rs'
        $helper=Join-Path $ctx.root 'scripts/migration-s54-outer.ps1'
        $path=if($script:IndexMutation -like 'queried-*'){$queried}elseif($script:IndexMutation -eq 'embedded-content'){Join-Path $ctx.root $script:S54Embedded[2]}else{$helper}
        $full=Get-S54Path $path
        if(-not $full.StartsWith($ctx.root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Owned public source mutation only.'}
        $script:IndexMutated=$true
        if($script:IndexMutation -like '*content'){[IO.File]::WriteAllText($full,'changed public fixture source bytes')}
        elseif($script:IndexMutation -like '*missing'){[IO.File]::Delete($full)}
        elseif($script:IndexMutation -like '*reparse'){
            $live=Get-S54Path (Split-Path -Parent $full);$held=Get-S54Path ($live+'-held') -AllowMissing
            foreach($target in @($live,$held)){if(-not $target.StartsWith($ctx.root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Owned public directory move only.'}}
            [IO.Directory]::Move($live,$held);$null=New-Item -ItemType Junction -Path $live -Target $held -ErrorAction Stop
        }else{throw 'Closed test mutation kind.'}
    }
    # Isolated test-process instrumentation. Production has no mutation hooks.
    function Get-S54Path([string]$Path,[switch]$AllowMissing){
        if($script:IndexBuilding){$script:IndexCounts.source_paths++}
        elseif($MyInvocation.ScriptName -ieq $script:IndexInvokePath -and $MyInvocation.ScriptLineNumber -eq $script:IndexQueryLine){
            $script:IndexCounts.queries++
            if($script:IndexTrigger -ceq 'query' -and -not $script:IndexMutated -and [string]::Equals([IO.Path]::GetFullPath($Path),(Join-Path $script:IndexContext.root 'apps/kilogram-cli/tests/support/migration_s54_p5.rs'),[StringComparison]::OrdinalIgnoreCase)){Mutate-Index}
        }
        & $script:IndexRealPath $Path -AllowMissing:$AllowMissing
    }
    function Read-S54Pin($Pin,[long]$Cap){
        if($script:IndexTailChecking){$script:IndexCounts.tail_reads++;$script:IndexCounts.caps+=,$Cap;$script:IndexCounts.tail_paths+=,$Pin.path}
        & $script:IndexRealRead $Pin $Cap
    }
    function New-S54CompilerSourceIndex($Sources){
        $script:IndexCounts.build_calls++;$script:IndexBuilding=$true
        try{$value=& $script:IndexRealBuild $Sources}finally{$script:IndexBuilding=$false}
        $script:IndexMaps.Add($value)
        if($script:IndexTrigger -ceq 'index' -and -not $script:IndexMutated){Mutate-Index}
        return ,$value
    }
    function Assert-S54CompilerSourcesFresh($Sources){
        $script:IndexCounts.tail_calls++
        if($script:IndexTrigger -ceq 'tail' -and -not $script:IndexMutated){
            Require-Index ($script:IndexCounts.queries -eq $script:IndexExpectedQueries) 'mutation occurs after final membership query'
            Mutate-Index
        }
        $script:IndexTailChecking=$true
        try{& $script:IndexRealTail $Sources}finally{$script:IndexTailChecking=$false}
    }
    $indexChecksComplete=$false
    try {
        $ctx=Case-Index 'positive-P'
        foreach($role in @('normal','tests')){
            $cap=$ctx[$role];Reset-Index $ctx $cap
            Assert-S54Compiler $ctx.i $ctx.policy $cap ($role -ceq 'normal')
            $n=$ctx.i.source.files.Count;$counts=$script:IndexCounts
            Require-Index ($counts.build_calls -eq 1 -and $counts.source_paths -eq $n -and $counts.queries -eq $script:IndexExpectedQueries -and $counts.tail_calls -eq 1 -and $counts.tail_reads -eq $n) "linear source+query+tail observation count/$role"
            Require-Index (@($counts.caps|Where-Object {$_ -ne 8388608}).Count -eq 0) "unchanged source cap/$role"
            Assert-S54Equal @($counts.tail_paths|Sort-Object) @($ctx.i.source.files|ForEach-Object {$_.path}|Sort-Object)
            $checks.Add(@{name="actual compiler positive/$role";passed=$true;source_count=$n;query_count=$counts.queries;counts=$counts})
        }
        Require-Index (-not [object]::ReferenceEquals($script:IndexMaps[0],$script:IndexMaps[1])) 'normal/tests own distinct local indexes'
        foreach($name in @('positive-P','positive-F')){
            $ctx=Case-Index $name;Reset-Index $ctx $ctx.normal
            $facts=Test-S54Admission $ctx.inputs $ctx.policy
            Require-Index ($facts.classification -ceq 'public_schema_fixture_only') "full public admission/$name"
            Require-Index ($ctx.i.identity.attempt -ceq (Get-S54Purpose $(if($name -ceq 'positive-F'){'FeasibilityF'}else{'WholeP'})).Attempt) "fixed F04/P01/$name"
        }
        $ctx=Case-Index 'case-separator';$cap=$ctx.normal
        for($n=0;$n -lt $cap.raw_compiler_artifact_json.Count;$n++){
            $v=ConvertFrom-Json -InputObject $cap.raw_compiler_artifact_json[$n] -AsHashtable
            $v.target.src_path=$v.target.src_path.ToUpperInvariant().Replace('\','/')
            $cap.raw_compiler_artifact_json[$n]=ConvertTo-Json -InputObject $v -Depth 100 -Compress
        }
        Reset-Index $ctx $cap;Assert-S54Compiler $ctx.i $ctx.policy $cap $true;Require-Index $true 'actual compiler case/separator membership parity'
        $ctx=Case-Index 'duplicate';$pin=@($ctx.i.source.files|Where-Object {$_.path -ieq (Join-Path $ctx.root 'scripts/migration-s54-outer.ps1')})[0]
        $extra=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $pin -Compress))) 4096
        $extra.path=$extra.path.ToUpperInvariant().Replace('\','/');$ctx.i.source.files+=,$extra
        Reset-Index $ctx $ctx.normal;Refuse-Index 'canonical duplicate compiler source' {Assert-S54Compiler $ctx.i $ctx.policy $ctx.normal $true} 'duplicate canonical compiler source'
        $ctx=Case-Index 'foreign';$cap=$ctx.normal;$foreign=Owned-IndexWrite $ctx (Join-Path $ctx.root 'apps/unadmitted.rs');[IO.File]::WriteAllText($foreign,'public unadmitted target')
        $v=ConvertFrom-Json -InputObject $cap.raw_compiler_artifact_json[0] -AsHashtable;$v.target.src_path=$foreign;$cap.raw_compiler_artifact_json[0]=ConvertTo-Json $v -Depth 100 -Compress
        Reset-Index $ctx $cap;Refuse-Index 'unadmitted workspace target' {Assert-S54Compiler $ctx.i $ctx.policy $cap $true} 'workspace compiler target belongs to admitted live sources'
        foreach($case in @(
            @('queried-content-index','index','queried-content','public pin mismatch'),
            @('queried-missing-query','query','queried-missing','missing public path'),
            @('queried-reparse-query','query','queried-reparse','reparse ancestor'),
            @('unqueried-content-index','index','unqueried-content','public pin mismatch'),
            @('unqueried-missing-index','index','unqueried-missing','missing public path'),
            @('unqueried-reparse-index','index','unqueried-reparse','reparse ancestor'),
            @('after-last-query-content','tail','unqueried-content','public pin mismatch'),
            @('after-last-query-missing','tail','unqueried-missing','missing public path'),
            @('after-last-query-reparse','tail','unqueried-reparse','reparse ancestor'),
            @('after-last-query-embedded','tail','embedded-content','public pin mismatch')
        )){
            $ctx=Case-Index $case[0];Reset-Index $ctx $ctx.normal $case[1] $case[2]
            Refuse-Index $case[0] {Assert-S54Compiler $ctx.i $ctx.policy $ctx.normal $true} $case[3]
            Require-Index $script:IndexMutated ("deterministic mutation reached/"+$case[0])
        }
        foreach($name in @('malformed','profile','graph','raw','unit','tool','origin')){
            $ctx=Case-Index $name;$cap=$ctx.normal;$pattern=$null
            switch($name){
                'malformed' {$cap.unknown=$true;$pattern='closed object CompilerCapture|unknown field CompilerCapture'}
                'profile' {$cap.profile='verification';$pattern='compiler role/profile'}
                'graph' {foreach($x in @($cap.build_inputs_before,$cap.build_inputs_after)){$x.package_names=@($x.package_names|Select-Object -Skip 1)};$pattern='complete package graph'}
                'raw' {$v=ConvertFrom-Json $cap.raw_compiler_artifact_json[0] -AsHashtable;$v.reason='foreign';$cap.raw_compiler_artifact_json[0]=ConvertTo-Json $v -Depth 100 -Compress;$pattern='actual known compiler-artifact'}
                'unit' {$cap.units[0].features=@('invented');$pattern='exact structured identity|duplicate compiler unit'}
                'tool' {[IO.File]::WriteAllText((Owned-IndexWrite $ctx $cap.build_inputs_before.tools[0].path),'changed public tool');$pattern='compiler tool/shim bytes'}
                'origin' {$u=@($cap.units|Where-Object {$_.executable -and -not $_.profile.test -and $_.package -ceq 'kilogram-cli'})[0];[IO.File]::WriteAllText((Owned-IndexWrite $ctx $u.executable),'changed public compiler image');$pattern='actual compiler executable bytes'}
            }
            Reset-Index $ctx $cap;Refuse-Index "retained compiler refusal/$name" {Assert-S54Compiler $ctx.i $ctx.policy $cap $true} $pattern
        }
        foreach($name in @('config','environment','capture','source-set','F01','foreign-selector','image')){
            $ctx=Case-Index $name;$pattern=$null;$full=$false
            switch($name){
                'config' {[IO.File]::WriteAllText((Owned-IndexWrite $ctx $ctx.i.runtime.configs[0].path),'changed public config');$pattern='config presence|config identity/override'}
                'environment' {$ctx.i.runtime.environment[0].value_sha256=('f'*64);$pattern='exact structured identity'}
                'capture' {[IO.File]::AppendAllText((Owned-IndexWrite $ctx $ctx.inputs.capture.path),' ');$pattern='public pin mismatch';$full=$true}
                'source-set' {[IO.File]::WriteAllText((Owned-IndexWrite $ctx (Join-Path $ctx.root 'apps/new-source.rs')),'public new source');$pattern='exact structured identity'}
                'image' {[IO.File]::WriteAllText((Owned-IndexWrite $ctx $ctx.i.images[1].pin.path),'changed fixed public image');$pattern='public pin mismatch'}
                'F01' {$ctx.i.identity.attempt='F-s54-mixed-hooks-feasibility-01-attempt-01';$pattern='required identity'}
                'foreign-selector' {$ctx.i.identity.selector=(Get-S54Purpose WholeP).Selector;$pattern='required identity'}
            }
            Reset-Index $ctx $ctx.normal
            Refuse-Index "retained inventory boundary/$name" {if($full){Test-S54Admission $ctx.inputs $ctx.policy}else{Assert-S54Inventory $ctx.i $ctx.policy}} $pattern
        }
        $resultPath=Get-S54Path (Join-Path $evidence 'compiler-index-results.json') -AllowMissing
        if(-not $resultPath.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $resultPath)){throw 'Fresh owned index results required.'}
        Write-KilogramFrozenJson $resultPath @{classification='real shared compiler/index/tail regressions on fresh public fixtures; no runtime capability';count=$checks.Count;checks=@($checks);instrumented=$true;timing_evidence=$false;production_runtime=$false}
        $indexChecksComplete=$true
        Write-Output ('compiler_index_checks='+$checks.Count)
    }finally{
        if(-not $indexChecksComplete){Write-KilogramFrozenJson (Join-Path $evidence 'compiler-index-partial.json') @{classification='partial checks before failed focused invocation; not a passed matrix';count=$checks.Count;checks=@($checks);instrumented=$true;production_runtime=$false}}
        Set-Item Function:Get-S54Path $script:IndexRealPath
        Set-Item Function:Read-S54Pin $script:IndexRealRead
        Set-Item Function:New-S54CompilerSourceIndex $script:IndexRealBuild
        Set-Item Function:Assert-S54CompilerSourcesFresh $script:IndexRealTail
    }
    return
}
if($index.classification -cne 'public_schema_fixture_only'){throw 'Only exported public fixture is supported.'}
$policy=New-S54Policy -FixtureRoot $index.root
$inputs=$index.inputs
$baseline=@{};foreach($n in @('inventory','activation','launch','capture')){$baseline[$n]=Read-S54Public $inputs[$n].path 2097152}
$i=ConvertFrom-S54Strict $baseline.inventory 2097152
$a=ConvertFrom-S54Strict $baseline.activation 16384
$l=ConvertFrom-S54Strict $baseline.launch 16384
$c=ConvertFrom-S54Strict $baseline.capture 4096
$evidence=Get-S54Path $EvidenceDirectory
if(-not $evidence.StartsWith((Join-Path (Split-Path -Parent $PSScriptRoot) '.tmp/s54-block4b-r1-01/'),[StringComparison]::OrdinalIgnoreCase)){throw 'Owned check evidence only.'}
$results=[Collections.Generic.List[object]]::new()
function Require([bool]$Condition,[string]$Message){if(-not $Condition){throw $Message}}
function Refuses([string]$Name,[scriptblock]$Test,[string]$Pattern) {
    $errorText=$null
    try {& $Test}catch{$errorText=$_.Exception.Message}
    Require ($null -ne $errorText) ('Expected refusal: '+$Name)
    if($Pattern){Require ($errorText -match $Pattern) ("Wrong refusal for ${Name}: $errorText; expected $Pattern")}
    $results.Add([ordered]@{name=$Name;expected_refusal=$true;error=$errorText})
}
function Copy-Public($Value){ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 100 -Compress))) 8388608}
function Seal-Public($I,$A,$L,$C) {
    # Test-only writes to the exported owned public fixture, never real I/A/L/C.
    $objects=@{inventory=$I;activation=$A;launch=$L;capture=$C}
    foreach($n in @('inventory','activation','capture','launch')) {
        if($n -ceq 'activation'){$A.inventory=$inputs.inventory}
        if($n -ceq 'launch'){$L.inventory=$inputs.inventory;$L.activation=$inputs.activation;$L.capture=$inputs.capture}
        $bytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $objects[$n] -Depth 100 -Compress))
        [IO.File]::WriteAllBytes($inputs[$n].path,$bytes);$inputs[$n]=Get-S54Pin $inputs[$n].path
    }
}
function Restore-Public {
    foreach($n in @('inventory','activation','launch','capture')){[IO.File]::WriteAllBytes($inputs[$n].path,$baseline[$n]);$inputs[$n]=Get-S54Pin $inputs[$n].path}
}
try {
    $value=Test-S54Admission $inputs $policy
    Require ($value.classification -ceq 'public_schema_fixture_only' -and $value.whole_attempt_verdict -ceq 'not_established') 'No public fixture can establish whole success.'
    $bytes=New-S54InventoryBytes $i $policy
    Require ($bytes.Length -gt 0 -and $bytes.Length -le 2097152) 'Bounded inventory preparation function.'
    $null=ConvertFrom-S54Strict $bytes 2097152
    $results.Add([ordered]@{name='valid public production-path admission/preparation';passed=$true})
    Refuses 'nested duplicate map' {ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes('{"outer":[{"nested":{"x":1,"x":2}}]}')) 4096}
    Refuses 'BOM' {ConvertFrom-S54Strict ([byte[]](239,187,191,123,125)) 4096}
    Refuses 'invalid UTF8' {ConvertFrom-S54Strict ([byte[]](255)) 4096}
    Refuses 'oversized bytes' {ConvertFrom-S54Strict ([byte[]]::new(4097)) 4096}
    foreach($name in @('unknown','numeric_boolean','S53_case','wrong_selector','duplicate_role','wrong_compiler_argv','wrong_profile')) {
        $bad=Copy-Public $i
        switch($name) {
            'unknown' {$bad.extra=$true}
            'numeric_boolean' {$bad.runtime.cargo_jobs=$true}
            'S53_case' {$bad.identity.case='C-s53-controls-b-only-01'}
            'wrong_selector' {$bad.identity.selector='not-whole'}
            'duplicate_role' {$bad.images[1]=Copy-Public $bad.images[0]}
            'wrong_compiler_argv' {$bad.compiler.normal_argv=@('test','--no-run')}
            'wrong_profile' {$bad.runtime.priority='Normal'}
        }
        Seal-Public $bad (Copy-Public $a) (Copy-Public $l) (Copy-Public $c)
        Refuses $name {Test-S54Admission $inputs $policy};Restore-Public
    }
    $capturePath=$i.compiler.normal.path;$savedCapture=Read-S54Public $capturePath 8388608
    try {
        $badCapture=ConvertFrom-S54Strict $savedCapture 8388608
        $badCapture.build_inputs_before.resolved_graph=@();$badCapture.build_inputs_after.resolved_graph=@()
        [IO.File]::WriteAllBytes($capturePath,[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $badCapture -Depth 100 -Compress)))
        $bad=Copy-Public $i;$bad.compiler.normal=Get-S54Pin $capturePath
        Seal-Public $bad (Copy-Public $a) (Copy-Public $l) (Copy-Public $c)
        Refuses 'missing actual graph facts with restamped outer pins' {Test-S54Admission $inputs $policy}
    }finally{[IO.File]::WriteAllBytes($capturePath,$savedCapture);Restore-Public}
    # Same eighteen production-boundary mutations as the Rust parity test. Each
    # runs directly and through the full path with only public refs restamped.
    $parity=@(
        @('duplicate_artifacts','duplicate compiler image path/role'),
        @('invented_guard','no invented frozen guard'),
        @('wrong_expected_hash','frozen expected-manifest identity'),
        @('missing_guard','frozen expected-manifest identity'),
        @('normal_guard','frozen expected-manifest identity'),
        @('runtime_debuginfo','runtime opt3/debug/overflow profile'),
        @('runtime_debuginfo_array','runtime opt3/debug/overflow profile'),
        @('oversized_compiler_string','bounded unique compiler string'),
        @('duplicate_package_id','unique package ID/name map'),
        @('invalid_dep_kind','bounded dependency kinds'),
        @('duplicate_dep_kind','bounded dependency kinds'),
        @('escaped_executable','compiler executable target/profile path'),
        @('duplicate_features','bounded unique compiler string'),
        @('empty_target_kind','bounded compiler target/output facts'),
        @('duplicate_output','unique compiler output paths'),
        @('unadmitted_workspace_source','workspace compiler target belongs to admitted live sources'),
        @('tool_basename','compiler executable role'),
        @('duplicate_graph_features','bounded unique compiler string')
    )
    foreach($case in $parity){
        $name=$case[0];$normal=$name -ceq 'normal_guard';$role=if($normal){'normal'}else{'tests'}
        $path=$i.compiler[$role].path;$saved=Read-S54Public $path 8388608
        try {
            $capture=ConvertFrom-S54Strict $saved 8388608
            Assert-S54Compiler $i $policy $capture $normal
            switch($name){
                'duplicate_artifacts' {$capture.stable_artifacts=@(1..6|ForEach-Object {Copy-Public $capture.stable_artifacts[0]})}
                'invented_guard' {$capture.frozen_guard_verified=$true}
                'wrong_expected_hash' {$capture.expected_manifest_sha256='0'*64;$capture.frozen_guard_verified=$true}
                'missing_guard' {$capture.expected_manifest_sha256=$i.compiler.frozen_manifest.sha256}
                'normal_guard' {$capture.expected_manifest_sha256=$i.compiler.frozen_manifest.sha256}
                'duplicate_package_id' {$capture.build_inputs_before.package_names[1]=Copy-Public $capture.build_inputs_before.package_names[0]}
                {$_ -cin @('invalid_dep_kind','duplicate_dep_kind')} {
                    $kinds=if($name -ceq 'invalid_dep_kind'){@(@{kind='foreign';target=$null})}else{@(@{kind=$null;target=$null},@{kind=$null;target=$null})}
                    $capture.build_inputs_before.resolved_graph[0].deps=@(@{name='self';pkg='fixture:kilogram-cli';dep_kinds=@($kinds)})
                }
                'duplicate_graph_features' {$capture.build_inputs_before.resolved_graph[0].features=@('same','same')}
                'oversized_compiler_string' {$capture.build_inputs_before.cargo=@(([string][char]0xE9)*513)}
                'tool_basename' {$p=Get-S54Pin (Join-Path $policy.Root 'tools/pwsh.exe');$capture.build_inputs_before.tools[0].path=$p.path;$capture.build_inputs_before.tools[0].sha256=$p.sha256}
                default {
                    $raw=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($capture.raw_compiler_artifact_json[0])) 131072
                    switch($name){
                        'runtime_debuginfo' {$capture.units[0].profile.debuginfo=0;$raw.profile.debuginfo=0}
                        'runtime_debuginfo_array' {$capture.units[0].profile.debuginfo=@('line-tables-only');$raw.profile.debuginfo=@('line-tables-only')}
                        'escaped_executable' {$p=Join-Path $policy.Root $script:S54Images[3].path;$capture.units[0].executable=$p;$raw.executable=$p}
                        'duplicate_features' {$capture.units[0].features=@('same','same');$raw.features=@('same','same')}
                        'empty_target_kind' {$capture.units[0].kind=@();$raw.target.kind=@()}
                        'duplicate_output' {$paths=@($raw.filenames[0],$raw.filenames[0]);$capture.units[0].filenames=$paths;$raw.filenames=$paths}
                        'unadmitted_workspace_source' {$raw.target.src_path=Join-Path $policy.Root 'tools/cargo.exe'}
                    }
                    $capture.raw_compiler_artifact_json[0]=ConvertTo-Json -InputObject $raw -Depth 100 -Compress
                }
            }
            $capture.build_inputs_after=Copy-Public $capture.build_inputs_before
            Refuses "compiler parity direct/$name" {Assert-S54Compiler $i $policy $capture $normal} $case[1]
            [IO.File]::WriteAllBytes($path,[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $capture -Depth 100 -Compress)))
            $bad=Copy-Public $i;$bad.compiler[$role]=Get-S54Pin $path
            Seal-Public $bad (Copy-Public $a) (Copy-Public $l) (Copy-Public $c)
            Refuses "compiler parity bound/$name" {Test-S54Admission $inputs $policy} $case[1]
        }finally{[IO.File]::WriteAllBytes($path,$saved);Restore-Public}
    }
    foreach($name in $script:S54Embedded){
        $path=Join-Path $policy.Root $name;$saved=Read-S54Public $path 8388608
        try {
            [IO.File]::WriteAllText($path,'changed public embedded input')
            Refuses "changed embedded source/$name" {Test-S54Admission $inputs $policy} 'public pin mismatch'
            Remove-Item -LiteralPath $path
            Refuses "missing embedded source/$name" {Test-S54Admission $inputs $policy} 'missing public path'
        }finally{[IO.File]::WriteAllBytes($path,$saved)}
    }
    $depSaved=@{};foreach($p in $i.compiler.embedded.dep_info){$depSaved[$p.path]=Read-S54Public $p.path 1048576}
    try {
        foreach($case in @(@('missing_before','exact embedded before/after input set'),@('changed_after','exact bound pin bytes/hash'),@('duplicate_dep_info','exact compiler dep-info role'),@('unknown_dependency','unadmitted compiler dependency input'),@('missing_dependency','embedded dep-info completeness'))){
            $bad=Copy-Public $i
            switch($case[0]){
                'missing_before' {$bad.compiler.embedded.before=@()}
                'changed_after' {$bad.compiler.embedded.after[0].sha256='0'*64}
                'duplicate_dep_info' {$bad.compiler.embedded.dep_info[1]=Copy-Public $bad.compiler.embedded.dep_info[0]}
                default {
                    for($n=0;$n -lt $bad.compiler.embedded.dep_info.Count;$n++){
                        $path=$bad.compiler.embedded.dep_info[$n].path
                        $source=if($case[0] -ceq 'unknown_dependency'){$p=Join-Path $policy.Root 'apps/public-unadmitted.json';[IO.File]::WriteAllText($p,'unadmitted public compile input');$p}else{Join-Path $policy.Root 'apps/kilogram-cli/tests/support/migration_s54_p5.rs'}
                        [IO.File]::WriteAllText($path,"${path}: $source`n")
                        $bad.compiler.embedded.dep_info[$n]=Get-S54Pin $path
                    }
                }
            }
            Seal-Public $bad (Copy-Public $a) (Copy-Public $l) (Copy-Public $c)
            Refuses ("embedded boundary/"+$case[0]) {Test-S54Admission $inputs $policy} $case[1]
            foreach($p in $depSaved.Keys){[IO.File]::WriteAllBytes($p,$depSaved[$p])};Restore-Public
        }
    }finally{foreach($p in $depSaved.Keys){[IO.File]::WriteAllBytes($p,$depSaved[$p])};Restore-Public}
    foreach($name in @('stale_activation','reset_clock','capture_mismatch','unexpected_cleanup_claim')) {
        $aa=Copy-Public $a;$ll=Copy-Public $l;$cc=Copy-Public $c
        switch($name) {
            'stale_activation' {$aa.not_after_utc_ms=1}
            'reset_clock' {$ll.controller_tick_ms++}
            'capture_mismatch' {$cc.whole_utc_ms=1}
            'unexpected_cleanup_claim' {$ll.cleanup_verified=$true}
        }
        Seal-Public (Copy-Public $i) $aa $ll $cc
        Refuses $name {Test-S54Admission $inputs $policy};Restore-Public
    }
    $source=Join-Path $policy.Root 'Cargo.lock';$saved=Read-S54Public $source 8388608
    try {[IO.File]::WriteAllText($source,'changed public source');Refuses 'changed current source pin' {Test-S54Admission $inputs $policy}}
    finally {[IO.File]::WriteAllBytes($source,$saved)}
    $new=Join-Path $policy.Root 'apps/new-public-source.rs'
    try {[IO.File]::WriteAllText($new,'public unadmitted source');Refuses 'added relevant source set' {Test-S54Admission $inputs $policy}}
    finally {if(Test-Path -LiteralPath $new){Remove-Item -LiteralPath $new}}
    $shadow=Join-Path $policy.Root 'powershell.exe'
    try {[IO.File]::WriteAllText($shadow,'public shadow');Refuses 'memory shell shadow' {Test-S54Admission $inputs $policy}}
    finally {if(Test-Path -LiteralPath $shadow){Remove-Item -LiteralPath $shadow}}
    Refuses 'parent path escape' {Get-S54Path (Join-Path $policy.Root '../escape') -AllowMissing}
    $reparse=Join-Path (Split-Path -Parent $PSScriptRoot) '.tmp/s54-block4a-01/reparse-public-fixture/alias'
    Require (Test-Path -LiteralPath $reparse) 'Required owned reparse fixture.'
    Refuses 'actual junction ancestor' {Get-S54Path $reparse}
    Refuses 'Run missing external bindings refuses before setup' {& (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1') -Action Run}
    Refuses 'Prepare missing explicit root identity refuses before setup' {& (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1') -Action Prepare}
    $final=Test-S54Admission $inputs $policy
    Require ($final.classification -ceq 'public_schema_fixture_only') 'Fixture restored after negative cases.'
    $record=[ordered]@{classification='public schema/policy checks only; no process/profile/network fixture';count=$results.Count;checks=@($results);whole_attempt_verdict='not_established'}
    Write-KilogramFrozenJson (Join-Path $evidence 'powershell-public-results.json') $record
    Write-Output ('public_policy_check_count='+$results.Count)
}finally{Restore-Public}
