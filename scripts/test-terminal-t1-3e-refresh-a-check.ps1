param([Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
$root=Split-Path $PSScriptRoot -Parent
$out=[IO.Path]::GetFullPath($Output)
$allowed=[IO.Path]::GetFullPath((Join-Path $root '.tmp/terminal-t1-3e/agent-refresh-metadata-prep-01'))+'\'
if(!$out.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $out)){throw 'FRESH_SYNTHETIC_OUTPUT_REQUIRED'}
. "$PSScriptRoot/test-terminal-t1-3e-refresh-a.ps1" -Action Library
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
[void][IO.Directory]::CreateDirectory($out)
Lab-Plain $out
$guard=[KilogramS5Deadline]::new(60,80)
$cases=[Collections.Generic.List[object]]::new()
$payloadNames=@((Lab-Read (Join-Path $root '.tmp/terminal-t1-3e/sender-crash-prep-01/refresh/old-manifest.json')).files.PSObject.Properties.Name)
function Require($Condition,[string]$Name){if(!$Condition){throw "ASSERT:$Name"}}
function New-Case([string]$CaseName,[switch]$Reverse){
    $path=Join-Path $out $CaseName;[void][IO.Directory]::CreateDirectory($path)
    $base=Join-Path $path 'base';$dir=Join-Path $base 'ordinary-agent'
    $old=Join-Path $path 'old';$new=Join-Path $path 'new'
    foreach($folder in @($old,$new,$dir,(Join-Path $dir 'control'))){[void][IO.Directory]::CreateDirectory($folder)}
    $maps=@{old=@{};new=@{}}
    foreach($name in $payloadNames){
        foreach($kind in @('old','new')){
            $content=if($kind -ceq 'new' -and $name -cin $RefreshNames){'new:'+ $name}else{'old:'+ $name}
            $target=Join-Path $(if($kind -ceq 'old'){$old}else{$new}) $name
            [IO.File]::WriteAllText($target,$content);$maps[$kind][$name]=Lab-Hash $target
        }
        [IO.File]::Copy((Join-Path $(if($Reverse){$new}else{$old}) $name),(Join-Path $dir $name))
    }
    $from=if($Reverse){'new'}else{'old'};$to=if($Reverse){'old'}else{'new'}
    $objects=@{};foreach($name in $payloadNames){$objects[$name]=Get-RefreshObject (Join-Path $dir $name)}
    return @{name=$CaseName;plan=@{base=$base;crash_policy_sha256=('a'*64)};planSha=('b'*64);directory=$dir;source=$(if($Reverse){$old}else{$new});
        fromKind=$from;toKind=$to;from=$maps[$from];to=$maps[$to];maps=$maps;action=$(if($Reverse){'Restore'}else{'Apply'});
        operation=(Join-Path $path 'operation');guard=$guard;expectedActor=@{session='initial'};wakes=[Collections.Generic.List[string]]::new();stopCount=0;replaceCount=0;
        objects=$objects;initialObjects=$objects;expectedObjects=$objects;recovery=$false;originalPaths=@{}}
}
function Test-Io {
    return @{
        validate={param($c,$kind,$expected)
            Assert-RefreshFiles $c.directory $c.from
            Assert-RefreshObjects $c.directory $c.initialObjects
            $snapshot=@{ready=[pscustomobject]@{session='initial';pid=1;started=1}}
            $c.initialActor=$snapshot;return $snapshot
        }
        stop={param($c,$kind,$snapshot)
            $c.stopCount++;Stop-LabIdleAgent $c.plan.base $c.handles
            $c.lastStopped=$snapshot;$c.currentActor=$null
        }
        wake={param($c,$kind)
            Assert-RefreshFiles $c.directory $c.maps[$kind]
            Assert-RefreshObjects $c.directory $c.expectedObjects
            Require ($c.handles.Count -eq 0) 'release-before-wake'
            $c.wakes.Add($kind)
            $ready=if($kind -ceq 'new'){[pscustomobject]@{crash_policy_version='owned-runtime-crash-v1';crash_policy_sha256=$c.plan.crash_policy_sha256;crash_descendants_version='passive-natural-drain-v1';session='mock-new'}}else{[pscustomobject]@{session='mock-old'}}
            Assert-RefreshPolicy $kind $ready $c.plan.crash_policy_sha256
            $c.currentActor=@{ready=$ready};return $c.currentActor
        }
    }
}
function New-RecoveryCase([string]$CaseName) {
    $c=New-Case $CaseName -Reverse;$c.recovery=$true;$c.action='Recover';$c.toKind='new';$c.to=$c.maps.new
    $c.source=Join-Path (Split-Path $c.operation -Parent) 'new'
    $c.objects=$c.initialObjects.Clone()
    $folder=Join-Path (Split-Path $c.operation -Parent) 'originals'
    $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
    $security=[Security.AccessControl.DirectorySecurity]::new();$security.SetAccessRuleProtection($true,$false)
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'Modify','ContainerInherit, ObjectInherit','None','Allow'))
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'),'FullControl','ContainerInherit, ObjectInherit','None','Allow'))
    [void][IO.Directory]::CreateDirectory($folder,$security)
    foreach($name in $RefreshNames){
        $path=Join-Path $folder $name
        [IO.File]::WriteAllText($path,('old:'+ $name))
        # Fixture setup only: mark inheritance through the normal Access-section API
        # on this newly created disposable file. Never set owner or installed ACLs.
        $fileSecurity=[IO.File]::GetAccessControl($path);$fileSecurity.SetAccessRuleProtection($false,$true)
        [IO.File]::SetAccessControl($path,$fileSecurity)
        $c.originalPaths[$name]=$path;$c.objects[$name]=Get-RefreshObject $path
        Require ($c.objects[$name].sddl.Contains('D:AI')) 'auto-inherited-fixture-matches-installed-form'
        Require ($c.objects[$name].sddl -cne $c.initialObjects[$name].sddl) 'different-original-DACL-fixture'
    }
    return $c
}
function Finish-Case($c,$r,[string]$Expected){
    Require ($c.handles.Count -eq 0) 'all-locks-released'
    foreach($file in @('agent.lock','images.lock')){
        $handle=[IO.File]::Open((Join-Path $c.directory ('control/'+$file)),'OpenOrCreate','Write','None');$handle.Dispose()
    }
    if($Expected -ceq 'success'){Require $r.success 'success';Assert-RefreshFiles $c.directory $c.to}
    elseif($Expected -ceq 'rollback'){Require (!$r.success -and $r.rollback.availability_verified) 'rollback-verified';Assert-RefreshFiles $c.directory $c.from}
    elseif($Expected -ceq 'blocked'){Require (!$r.success -and !$r.rollback.availability_verified) 'blocked'}
    if($Expected -ceq 'success'){Assert-RefreshObjects $c.directory $c.objects}
    if($Expected -ceq 'rollback'){Assert-RefreshObjects $c.directory $c.initialObjects}
    Write-RefreshRecord (Join-Path (Split-Path $c.operation -Parent) 'check.json') @{pass=$true;expect=$Expected;result=$r;wakes=@($c.wakes);stops=$c.stopCount}
    $cases.Add(@{name=$c.name;pass=$true;expected=$Expected})
}
function Test-InitialProductionPath {
    # Use the actual production validate callback and Get-RefreshActor prelude.
    # Replace external file/task readers; stop precisely before native OpenActor.
    # Neither this fixture nor its IO callbacks initialize initialActor.
    $oldPath=Join-Path $root '.tmp/terminal-t1-3e/agent-refresh-prep-01/source/scripts/test-terminal-t1-3e-refresh-a.ps1'
    Require ((Lab-Hash $oldPath) -ceq 'c0363f127171f74fde5653ff835d3cd5bc4b2701018b482e04dab37d829e61a9') 'original-source-pin'
    $parseErrors=$null;$parseTokens=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile($oldPath,[ref]$parseTokens,[ref]$parseErrors)
    Require (@($parseErrors).Count -eq 0) 'original-parse'
    $fn=$ast.Find({param($node)$node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq 'Invoke-RefreshOperation'},$true)
    . ([scriptblock]::Create($fn.Extent.Text.Replace('function Invoke-RefreshOperation(','function Invoke-RefreshOperationBefore(')))
    $actorBody=(Get-Item Function:Get-RefreshActor).ScriptBlock.ToString()
    $boundary=$actorBody.IndexOf('    $actor=[KilogramLabNative]::OpenActor(')
    Require ($boundary -gt 0) 'native-boundary'
    $prelude=$actorBody.Substring(0,$boundary)+'return @{ready=$ready}'
    Set-Item Function:Get-RefreshActor ([scriptblock]::Create($prelude))
    function Assert-RefreshFiles($Directory,$Pins){}
    function Assert-RefreshObjects($Directory,$Pins){}
    function Lab-Config($Path){return [pscustomobject]@{host='fixture';sid='fixture-sid'}}
    function Lab-TaskSpec($Cfg){return [pscustomobject]@{name='fixture-task'}}
    function Find-LabTask($Name){return [pscustomobject]@{name=$Name}}
    function Assert-LabTask($Task,$Want){}
    function Assert-LabReadOnly($Cfg,$Pin){return @{read_only=$true}}
    function Lab-Read($Path){
        if($Path.EndsWith('ready.json')){return [pscustomobject]@{session=('1'*32);pid=42;started=123;host='fixture';sid='fixture-sid';
            administrator=$false;logon_type=2;windows_session=1;profile_hive_loaded=$true}}
        if($Path.EndsWith('state.json')){return [pscustomobject]@{session=('1'*32);active=$null;blocked=$null}}
        throw 'UNEXPECTED_FIXTURE_READ'
    }
    $results=@{}
    foreach($variant in @('Before','Corrected')){
        # Same initial shape as the executable entry point: no early actor fields.
        $c=@{plan=[pscustomobject]@{baseline_sha256=('a'*64);windows_session=1;crash_policy_sha256=('b'*64)};
            maps=@{old=@{}};directory='synthetic';expectedObjects=@{};action='Validate';fromKind='old';expectedActor=[pscustomobject]@{session=('1'*32);pid=42;started=123}}
        Require (!$c.ContainsKey('initialActor')) 'fixture-must-not-preinitialize'
        $io=Get-RefreshRealIo
        $io.stop={throw 'UNEXPECTED_STOP'};$io.wake={throw 'UNEXPECTED_WAKE'}
        $operation=if($variant -ceq 'Before'){'Invoke-RefreshOperationBefore'}else{'Invoke-RefreshOperation'}
        try{
            $result=& $operation $c $io
            Require ($variant -ceq 'Corrected' -and $result.validated -and $result.read_only -and $c.initialActor.ready.pid -eq 42) 'production-validate-success'
            $results[$variant]=@{pass=$true;validated=$true}
        }catch{
            if($variant -cne 'Before' -or $_.FullyQualifiedErrorId -cne 'PropertyNotFoundStrict' -or $_.Exception.Message -notmatch 'initialActor'){throw}
            $results[$variant]=@{expected_failure=$true;id=$_.FullyQualifiedErrorId;message=$_.Exception.Message;stack=$_.ScriptStackTrace}
        }
    }
    Write-RefreshRecord (Join-Path $out 'initial-production-path.json') @{strict_mode='Latest';results=$results;
        mocked='file/task readers only';real='Invoke-RefreshOperation + production validate callback + Get-RefreshActor pre-native prelude + Assert-RefreshIdentity';
        installed_IO=$false;prelude=$prelude}
}
function Test-NativeImagePath {
    Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-native.cs"
    $who=[Security.Principal.WindowsIdentity]::GetCurrent()
    try{
        $cfg=@{sid=$who.User.Value};$process=Get-Process -Id $PID
        try{$ready=@{pid=$PID;started=$process.StartTime.ToFileTimeUtc()}}finally{$process.Dispose()}
        $oldPath=Join-Path $root '.tmp/terminal-t1-3e/agent-refresh-prep-01/source/scripts/test-terminal-t1-3e-refresh-a.ps1'
        $results=@{}
        foreach($variant in @('Before','Corrected')){
            $path=if($variant -ceq 'Before'){$oldPath}else{Join-Path $PSScriptRoot 'test-terminal-t1-3e-refresh-a.ps1'}
            $errors=$null;$tokens=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($path,[ref]$tokens,[ref]$errors)
            $getter=$ast.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Get-RefreshActor'},$true)
            $call=$getter.Find({param($n)$n -is [Management.Automation.Language.InvokeMemberExpressionAst] -and $n.Member.Value -ceq 'OpenActor'},$true)
            Require ($null -ne $call) 'native-production-call'
            $actor=$null
            try{
                # Evaluate the exact production call, selecting only this fixture process.
                $actor=& ([scriptblock]::Create($call.Extent.Text))
                Require ($variant -ceq 'Corrected' -and $actor.Alive()) 'native-normalized-path'
                $results[$variant]=@{pass=$true;pid=$PID;image=$actor.image;call=$call.Extent.Text}
            }catch{
                if($variant -cne 'Before' -or $_.Exception.ToString() -notmatch 'Actor incarnation/image mismatch'){throw}
                $results[$variant]=@{expected_failure=$true;message=$_.Exception.ToString();call=$call.Extent.Text}
            }finally{if($actor){$actor.Dispose()}}
        }
        Write-RefreshRecord (Join-Path $out 'native-image-path.json') @{results=$results;installed_actor_IO=$false;self_pid=$PID}
    }finally{$who.Dispose()}
}
try {
    $pins=@{};[void][IO.Directory]::CreateDirectory((Join-Path $out 'sources'))
    foreach($name in @('test-terminal-t1-3e-refresh-a.ps1','test-terminal-t1-3e-refresh-a-check.ps1','test-terminal-t1-3e-refresh-files.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-native.cs','migration-s5-l1-deadline.ps1')){
        $source=Join-Path $PSScriptRoot $name;$pins[$name]=Lab-Hash $source;[IO.File]::Copy($source,(Join-Path $out ('sources/'+$name)))
    }
    Write-RefreshRecord (Join-Path $out 'source-pins.json') $pins
    Test-InitialProductionPath
    $cases.Add(@{name='initial-production-path-original-fails-corrected-passes';pass=$true})
    Test-NativeImagePath
    $cases.Add(@{name='native-image-path-original-fails-corrected-passes-self-only';pass=$true})
    $c=New-Case 'validate';$c.action='Validate';$r=Invoke-RefreshOperation $c (Test-Io)
    Require ($r.read_only -and !(Test-Path -LiteralPath $c.operation) -and $c.stopCount -eq 0 -and $c.wakes.Count -eq 0) 'default-read-only'
    Assert-RefreshFiles $c.directory $c.from;$cases.Add(@{name='validate';pass=$true})

    $c=New-Case 'apply';Finish-Case $c (Invoke-RefreshOperation $c (Test-Io)) 'success'
    $c=New-Case 'restore-no-new-fields' -Reverse;Finish-Case $c (Invoke-RefreshOperation $c (Test-Io)) 'success'
    $c=New-RecoveryCase 'recover-original-objects';Finish-Case $c (Invoke-RefreshOperation $c (Test-Io)) 'success'
    foreach($name in $RefreshNames){
        Require (![IO.File]::Exists($c.originalPaths[$name])) 'original-object-moved-not-copied'
        Assert-RefreshObject (Get-RefreshObject (Join-Path $c.directory $name)) $c.objects[$name]
        Assert-RefreshObject (Get-RefreshObject (Join-Path $c.operation ('quarantine/'+$name))) $c.initialObjects[$name]
    }
    foreach($recovery in @($false,$true)){foreach($position in $(if($recovery){1..6}else{1..4})){foreach($afterMove in @($false,$true)){
        $caseName='boundary-'+$recovery+'-'+$position+'-'+$afterMove
        $c=if($recovery){New-RecoveryCase $caseName}else{New-Case $caseName}
        $c.moveCount=0;$c.position=$position;$c.afterMove=$afterMove;$c.faulted=$false;$io=Test-Io
        $io.move={param($s,$d)
            $c.moveCount++
            $inject=(!$c.faulted -and $c.moveCount -eq $c.position)
            if($inject){$c.faulted=$true;if(!$c.afterMove){throw 'EXPECTED_BEFORE_RENAME'}}
            [RefreshAFileObject]::Rename($s,$d)
            if($inject){throw 'EXPECTED_AFTER_RENAME'}
        }
        Finish-Case $c (Invoke-RefreshOperation $c $io) 'rollback'
        if($recovery){foreach($name in $RefreshNames){
            Assert-RefreshObject (Get-RefreshObject $c.originalPaths[$name]) $c.objects[$name]
            Require ((Lab-Hash $c.originalPaths[$name]) -ceq $c.maps.old[$name]) 'recovered-original-bytes-after-fault'
        }}
    }}}
    $c=New-Case 'partial-write-held-handle';$io=Test-Io
    $io.write={param($stream,$bytes)
        Require (![IO.File]::Exists((Join-Path $c.directory $RefreshNames[0]))) 'agent-absent-during-write'
        $blocked=$false;try{$foreign=[IO.File]::Open($stream.Name,'Open','Write','ReadWrite');$foreign.Dispose()}catch{$blocked=$true}
        Require $blocked 'exclusive-handle-denies-foreign-write'
        $stream.Write($bytes,0,3);$stream.SetLength(3);$stream.Flush($true);throw 'EXPECTED_PARTIAL_WRITE'
    }
    Finish-Case $c (Invoke-RefreshOperation $c $io) 'rollback'
    $c=New-Case 'forward-reverse-same-objects';$io=Test-Io
    $first=Invoke-RefreshOperation $c $io;Require $first.success 'forward-first'
    $c.from=$c.maps.new;$c.to=$c.maps.old;$c.fromKind='new';$c.toKind='old';$c.source=Join-Path (Split-Path $c.operation -Parent) 'old'
    $c.operation=Join-Path (Split-Path $c.operation -Parent) 'reverse-operation';$c.action='Restore'
    Finish-Case $c (Invoke-RefreshOperation $c $io) 'success'
    foreach($recover in @($false,$true)){
        $c=if($recover){New-RecoveryCase 'reconcile-recovery-checkpoint'}else{New-Case 'reconcile-forward-checkpoint'}
        $c.handles=@{};$c.sequence=0;$c.cleanup=$false;$c.knownPartial=@{}
        [void][IO.Directory]::CreateDirectory($c.operation)
        foreach($folder in @('working','origin-bytes','quarantine')){[void][IO.Directory]::CreateDirectory((Join-Path $c.operation $folder))}
        Stop-LabIdleAgent $c.plan.base $c.handles
        try{
            foreach($name in $RefreshNames){
                $origin=if($recover){$c.originalPaths[$name]}else{Join-Path $c.directory $name}
                [IO.File]::Copy($origin,(Join-Path $c.operation ('origin-bytes/'+$name)))
            }
            $name=$RefreshNames[0];$destination=if($recover){Join-Path $c.operation ('quarantine/'+$name)}else{Get-RefreshWorkingPath $c $name}
            Move-RefreshObject $c @{} (Join-Path $c.directory $name) $destination $c.initialObjects[$name]
            if($recover){Move-RefreshObject $c @{} $c.originalPaths[$name] (Get-RefreshWorkingPath $c $name) $c.objects[$name]}
            # Reconstruct filesystem context from serialized evidence, not transient flags.
            $basis=@{from=$c.from;to=$c.to;objects=$c.objects;initialObjects=$c.initialObjects;originalPaths=$c.originalPaths;maps=$c.maps}|ConvertTo-Json -Depth 8|ConvertFrom-Json
            $again=@{directory=$c.directory;operation=$c.operation;guard=$guard;cleanup=$true;sequence=$c.sequence;handles=$c.handles;recovery=$recover;knownPartial=@{}}
            foreach($key in @('from','to','objects','initialObjects','originalPaths')){$again[$key]=Refresh-Map ([pscustomobject]@{files=$basis.$key})}
            $again.maps=@{};foreach($key in @('old','new')){$again.maps[$key]=Refresh-Map ([pscustomobject]@{files=$basis.maps.$key})}
            Undo-RefreshObjectUpdate $again @{}
            Assert-RefreshFiles $c.directory $c.from;Assert-RefreshObjects $c.directory $c.initialObjects
            if($recover){foreach($name in $RefreshNames){Assert-RefreshObject (Get-RefreshObject $c.originalPaths[$name]) $c.objects[$name]}}
            $cases.Add(@{name=$c.name;pass=$true})
        }finally{Close-RefreshHandles $c}
    }
    $c=New-Case 'wrong-initial-object';$c.initialObjects=$c.objects.Clone();$c.initialObjects[$RefreshNames[0]]=@{key='00000000:0000000000000000';sddl=$c.objects[$RefreshNames[0]].sddl}
    $refused=$false;try{$null=Invoke-RefreshOperation $c (Test-Io)}catch{$refused=$_.Exception.Message -ceq 'REFRESH_FILE_OBJECT_DRIFT'}
    Require ($refused -and $c.stopCount -eq 0 -and !(Test-Path -LiteralPath $c.operation)) 'metadata-before-stop'
    $cases.Add(@{name=$c.name;pass=$true})
    $legacy=Join-Path $out 'legacy-new-file'
    $legacySD=[Security.AccessControl.FileSecurity]::new()
    $legacySD.SetSecurityDescriptorSddlForm(('D:(A;ID;FA;;;'+[Security.Principal.WindowsIdentity]::GetCurrent().User.Value+')'))
    $legacyStream=[IO.FileStream]::new($legacy,'CreateNew',[Security.AccessControl.FileSystemRights]::FullControl,'None',4096,'None',$legacySD)
    $legacyStream.Dispose();$legacyObject=Get-RefreshObject $legacy;$refused=$false
    try{Assert-RefreshObject $legacyObject $legacyObject}catch{$refused=$_.Exception.Message -ceq 'REFRESH_LEGACY_INHERITANCE_UNSUPPORTED'}
    Require $refused 'legacy-descriptor-rejected-before-move'
    $cases.Add(@{name='unsupported-legacy-inherited-form-refused';pass=$true})
    $c=New-Case 'occupied-rename-destination';$io=Test-Io;$c.injected=$false
    $io.move={param($s,$d)
        if(!$c.injected){$c.injected=$true;[IO.File]::WriteAllText($d,'foreign destination')}
        [RefreshAFileObject]::Rename($s,$d)
    }
    Finish-Case $c (Invoke-RefreshOperation $c $io) 'blocked'
    Require ([IO.File]::ReadAllText((Get-RefreshWorkingPath $c $RefreshNames[0])) -ceq 'foreign destination') 'no-replace-preserves-foreign-destination'
    Assert-RefreshFiles $c.directory $c.from;Assert-RefreshObjects $c.directory $c.initialObjects

    foreach($after in @($false,$true)){
        $c=New-Case $(if($after){'throw-after-native-rename'}else{'second-rename-fails'})
        $io=Test-Io;$c.after=$after
        $io.move={param($s,$d)
            $c.replaceCount++
            if(!$c.after -and $c.replaceCount -eq 2){throw 'SYNTHETIC_SECOND_REPLACE'}
            [RefreshAFileObject]::Rename($s,$d)
            if($c.after -and $c.replaceCount -eq 1){throw 'SYNTHETIC_AFTER_REPLACE'}
        }
        Finish-Case $c (Invoke-RefreshOperation $c $io) 'rollback'
    }
    $c=New-Case 'second-lock-fails';$io=Test-Io
    $held=[IO.File]::Open((Join-Path $c.directory 'control/images.lock'),'OpenOrCreate','Write','None')
    try{$r=Invoke-RefreshOperation $c $io;Require (!$r.success -and $c.attempted.Count -eq 0 -and $c.wakes.Count -eq 0) 'partial-lock-no-mutation'}finally{$held.Dispose()}
    Finish-Case $c $r 'blocked';Assert-RefreshFiles $c.directory $c.from

    $c=New-Case 'wrong-initial-hash';[IO.File]::WriteAllText((Join-Path $c.directory $RefreshNames[0]),'foreign')
    $refused=$false;try{$null=Invoke-RefreshOperation $c (Test-Io)}catch{$refused=$_.Exception.Message -like 'REFRESH_DRIFT:*'}
    Require ($refused -and !(Test-Path -LiteralPath $c.operation) -and $c.stopCount -eq 0) 'reject-before-stop'
    $cases.Add(@{name=$c.name;pass=$true})

    foreach($mode in @('before-second','after-first','captured-race')){
        $c=New-Case ('foreign-'+$mode);$c.foreignMode=$mode;$io=Test-Io
        $io.move={param($s,$d)
            $c.replaceCount++
            if($c.foreignMode -ceq 'captured-race' -and $c.replaceCount -eq 1){[IO.File]::WriteAllText($s,'foreign')}
            [RefreshAFileObject]::Rename($s,$d)
            if($c.replaceCount -eq 1 -and $c.foreignMode -cne 'captured-race'){
                $foreign=if($c.foreignMode -ceq 'before-second'){$RefreshNames[1]}else{$RefreshNames[0]}
                $where=if($c.foreignMode -ceq 'before-second'){Join-Path $c.directory $foreign}else{Get-RefreshWorkingPath $c $foreign}
                [IO.File]::WriteAllText($where,'foreign');throw 'SYNTHETIC_FOREIGN'
            }
        }
        Finish-Case $c (Invoke-RefreshOperation $c $io) 'blocked'
        $foreign=if($mode -ceq 'before-second'){$RefreshNames[1]}else{$RefreshNames[0]}
        Require ([IO.File]::ReadAllText((Find-RefreshOriginal $c $foreign)) -ceq 'foreign') 'foreign-preserved'
        Require ($c.wakes.Count -eq 0) 'foreign-never-executed'
    }
    foreach($known in @($true,$false)){
        $c=New-Case $(if($known){'known-wake-failure'}else{'unknown-wake-failure'});$c.known=$known;$io=Test-Io
        $io.wake={param($c,$kind)
            $c.wakes.Add($kind)
            if($c.wakes.Count -eq 1){
                if($c.known){$c.currentActor=@{ready=[pscustomobject]@{session='known'}}}
                throw 'SYNTHETIC_WAKE_FAILED'
            }
            Assert-RefreshFiles $c.directory $c.from
            Assert-RefreshPolicy 'old' ([pscustomobject]@{session='restored'}) $c.plan.crash_policy_sha256
            return @{ready=@{session='restored'}}
        }
        Finish-Case $c (Invoke-RefreshOperation $c $io) $(if($known){'rollback'}else{'blocked'})
        if(!$known){Assert-RefreshFiles $c.directory $c.to;Require ($c.stopCount -eq 1) 'unknown-not-stopped'}
    }
    $c=New-Case 'stage-fails-after-stop';$io=Test-Io
    $io.stop={param($c,$kind,$snapshot)
        Stop-LabIdleAgent $c.plan.base $c.handles;$c.lastStopped=$snapshot
        [void][IO.Directory]::CreateDirectory((Join-Path $c.operation ('stage/'+$RefreshNames[0])))
    }
    Finish-Case $c (Invoke-RefreshOperation $c $io) 'rollback'

    $missing=$false;try{Assert-RefreshPolicy 'new' ([pscustomobject]@{session='old'}) ('a'*64)}catch{$missing=$_.Exception.Message -ceq 'REFRESH_LOADED_POLICY'}
    Require $missing 'new-policy-fields-required'
    $wrong=$false;try{Assert-RefreshPolicy 'new' ([pscustomobject]@{crash_policy_version='owned-runtime-crash-v1';crash_policy_sha256=('b'*64)}) ('a'*64)}catch{$wrong=$_.Exception.Message -ceq 'REFRESH_LOADED_POLICY'}
    Require $wrong 'new-policy-hash-required'
    $oldLoaded=$false;try{Assert-RefreshPolicy 'new' ([pscustomobject]@{crash_policy_version='owned-runtime-crash-v1';crash_policy_sha256=('a'*64)}) ('a'*64)}catch{$oldLoaded=$_.Exception.Message -ceq 'REFRESH_LOADED_POLICY'}
    Require $oldLoaded 'new-descendants-code-required-even-with-correct-common'
    $wrongLoaded=$false;try{Assert-RefreshPolicy 'new' ([pscustomobject]@{crash_policy_version='owned-runtime-crash-v1';crash_policy_sha256=('a'*64);crash_descendants_version='old'}) ('a'*64)}catch{$wrongLoaded=$_.Exception.Message -ceq 'REFRESH_LOADED_POLICY'}
    Require $wrongLoaded 'new-descendants-version-required'
    $cases.Add(@{name='policy-negative-and-old-compatibility';pass=$true})
    $ready=[pscustomobject]@{session='new';pid=2;started=2};$prior=@{ready=[pscustomobject]@{session='old';pid=1;started=1}}
    Assert-RefreshIdentity $ready $null $true @($prior)
    foreach($variant in @('session','incarnation','expected')){
        $bad=[pscustomobject]@{session='new';pid=2;started=2};$refused=$false
        if($variant -ceq 'session'){$bad.session='old'}elseif($variant -ceq 'incarnation'){$bad.pid=1;$bad.started=1}
        try{Assert-RefreshIdentity $bad $prior.ready ($variant -cne 'expected') @($prior)}catch{$refused=$true}
        Require $refused ('identity-'+$variant)
    }
    $cases.Add(@{name='fresh-and-bound-identity-negatives';pass=$true})
    $rp=[pscustomobject]@{sid='test-sid';windows_session=1;baseline_sha256=('a'*64)}
    $request=[pscustomobject]@{id=('b'*32)}
    $answer=[pscustomobject]@{ok=$true;id=$request.id;session='new';value=[pscustomobject]@{
        id=('c'*32);dpapi=$true;session='new';sid=$rp.sid;windows_session=1;baseline_sha256=$rp.baseline_sha256;backend='com-native-v1';logon_type=2}}
    Assert-RefreshReadiness $answer $request $ready $rp
    foreach($field in @('dpapi','session','sid','windows_session','baseline_sha256','backend','logon_type','id')){
        $copy=$answer|ConvertTo-Json -Depth 5|ConvertFrom-Json;$copy.value.$field='wrong';$refused=$false
        try{Assert-RefreshReadiness $copy $request $ready $rp}catch{$refused=$true};Require $refused ('readiness-'+$field)
    }
    $cases.Add(@{name='readiness-binding-negatives';pass=$true})
    # Exercise production Wake/check glue with only actor/task IO substituted.
    # These mocks cannot dispatch a scheduled task or run the DPAPI probe.
    $savedActor=(Get-Item Function:Get-RefreshActor).ScriptBlock
    $savedAgent=(Get-Item Function:Invoke-RefreshAgent).ScriptBlock
    try{
        Set-Item Function:Get-RefreshActor {param($Context,$Kind,$Expected,[switch]$AllowFresh)
            Assert-RefreshFiles $Context.directory $Context.maps[$Kind]
            $ready=[pscustomobject]@{session='mock-new';pid=2;started=2}
            if($Kind -ceq 'new'){
                $ready|Add-Member crash_policy_version 'owned-runtime-crash-v1'
                $ready|Add-Member crash_policy_sha256 $(if($Context.badPolicy){'bad'}else{$Context.plan.crash_policy_sha256})
                $ready|Add-Member crash_descendants_version 'passive-natural-drain-v1'
            }
            return @{ready=$ready}
        }
        Set-Item Function:Invoke-RefreshAgent {param($Context,$Kind,$Action,$Request)
            $Context.calls.Add($Action)
            if($Action -ceq 'Wake'){return [pscustomobject]@{state='requested'}}
            return [pscustomobject]@{ok=$true;id=$Request.id;session='mock-new';value=[pscustomobject]@{
                id=('d'*32);session='mock-new';sid=$Context.plan.sid;windows_session=1;dpapi=$true;logon_type=2;
                baseline_sha256=$Context.plan.baseline_sha256;backend='com-native-v1'}}
        }
        foreach($mode in @('new','old','wrong-policy')){
            $c=New-Case ('real-wake-glue-'+$mode) -Reverse
            $c.plan.sid='test-sid';$c.plan.windows_session=1;$c.plan.baseline_sha256=('a'*64)
            $c.sequence=0;$c.cleanup=$false;$c.currentActor=$null;$c.badPolicy=($mode -ceq 'wrong-policy');$c.calls=[Collections.Generic.List[string]]::new()
            $kind=if($mode -ceq 'old'){'old'}else{'new'}
            if($kind -ceq 'old'){foreach($name in $RefreshNames){[IO.File]::WriteAllText((Join-Path $c.directory $name),('old:'+ $name))}}
            [void][IO.Directory]::CreateDirectory($c.operation)
            $refused=$false;try{$answer=& (Get-RefreshRealIo).wake $c $kind}catch{if($mode -cne 'wrong-policy'){throw};$refused=$_.Exception.Message -ceq 'REFRESH_LOADED_POLICY'}
            if($mode -ceq 'wrong-policy'){Require ($refused -and $c.currentActor -and ($c.calls -join '|') -ceq 'Wake') 'known-policy-failure-before-probe'}
            else{Require (($c.calls -join '|') -ceq 'Wake|Command' -and $answer.ready.session -ceq 'mock-new') 'production-wake-check-glue'}
        }
    }finally{Set-Item Function:Get-RefreshActor $savedActor;Set-Item Function:Invoke-RefreshAgent $savedAgent}
    $cases.Add(@{name='production-wake-check-glue-mocked-IO';pass=$true})
    $binding=[pscustomobject]@{v=2;side='A';host='DESKTOP-PAUL-RY';sid='S-1-5-21-1776862868-2140151431-2099235586-1001';
        base=(Join-Path $root '.tmp/terminal-t11f');config_sha256='ad8489114462306ad56110bf357eb2e4dbefe20ec096a7636c180211da7e1056';
        baseline_sha256='b46b8cca1f3793a97acb920fd47fabe392a2d8d49bc2d553d1dbac6dc0ab8c42';windows_session=1;
        crash_policy_sha256='b9b280677ed1cb0d612223bfab7ecad306ec976ae5b042a76b77db581aa9da65';work_seconds=180;whole_seconds=200;operation='refresh-a-01';sources=[pscustomobject]@{};
        payload=[pscustomobject]@{
            old=[pscustomobject]@{directory='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh/old-payload';manifest='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh/old-manifest.json';sha256='fc3bf5b0713877c07b335f048922b2c9d52b75ba816a0509f41774c55facbead'};
            new=[pscustomobject]@{directory='.tmp/terminal-t1-3e/descendants-prep-01/refresh/new-payload';manifest='.tmp/terminal-t1-3e/descendants-prep-01/refresh/new-manifest.json';sha256='327143aeeafc72624074b5ca91e539eaaef349eaa9780a374a7d554a6582d71d'}}}
    foreach($file in @('scripts/test-terminal-t1-3e-refresh-a.ps1','scripts/test-terminal-t1-3e-lab-common.ps1','scripts/test-terminal-t1-3e-observe.ps1','scripts/migration-s5-l1-deadline.ps1','scripts/test-terminal-t1-3e-refresh-files.ps1')){
        $binding.sources|Add-Member -NotePropertyName $file -NotePropertyValue (Lab-Hash (Join-Path $root $file))
    }
    Assert-RefreshBindings $binding $root
    foreach($mode in @('side','base','limits','source','payload','manifest','old-candidate')){
        $bad=$binding|ConvertTo-Json -Depth 8|ConvertFrom-Json
        switch($mode){
            'side'{$bad.side='B'} 'base'{$bad.base+='-other'} 'limits'{$bad.work_seconds=181}
            'source'{$bad.sources.'scripts/test-terminal-t1-3e-refresh-a.ps1'='bad'}
            'payload'{$bad.payload.new.directory='../other'} 'manifest'{$bad.payload.old.sha256='bad'}
            'old-candidate'{$bad.payload.new=[pscustomobject]@{directory='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh-02/new-payload';manifest='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh-02/new-manifest.json';sha256='5a2784caff98850ddffe26e1a49e837e49da634abe7f098dff8b1729c7eda764'}}
        }
        $refused=$false;try{Assert-RefreshBindings $bad $root}catch{$refused=$true};Require $refused ('fixed-binding-'+$mode)
    }
    $cases.Add(@{name='fixed-source-payload-side-budget-bindings';pass=$true})
    Write-RefreshRecord (Join-Path $out 'result.json') @{pass=$true;cases=@($cases);count=$cases.Count;
        real_io='same-volume rename and retained file objects, exclusive in-place writes/undo, existing Stop-LabIdleAgent without state/agent';mocked='actor/task/Wake/readiness/DPAPI';installed_actions=$false}
    @{pass=$true;cases=$cases.Count;output=$out}|ConvertTo-Json -Compress
}catch{
    Write-RefreshRecord (Join-Path $out 'failure.json') @{error=$_.Exception.ToString();stack=$_.ScriptStackTrace;completed=@($cases)}
    throw
}finally{$guard.Finish()}
