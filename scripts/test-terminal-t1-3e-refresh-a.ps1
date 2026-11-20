param(
    [ValidateSet('Validate','Apply','Restore','Recover','Library')][string]$Action='Validate',
    [string]$Plan,[string]$PlanSha256,[string]$Operation,[string]$Acceptance,[string]$AcceptanceSha256
)
# A-only, two fixed payload files. Library mode is for disposable filesystem tests.
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
$RefreshNames=@('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1')
. "$PSScriptRoot/test-terminal-t1-3e-refresh-files.ps1"
function Refresh-Map($Manifest) {
    $map=@{};foreach($p in $Manifest.files.PSObject.Properties){$map[$p.Name]=$p.Value};return $map
}
function Assert-RefreshFiles([string]$Directory,$Pins) {
    Lab-Plain $Directory
    foreach($name in $Pins.Keys){if((Lab-Hash (Join-Path $Directory $name)) -cne $Pins[$name]){throw "REFRESH_DRIFT:$name"}}
}
function Write-RefreshRecord([string]$Path,$Value) {
    $temporary=$Path+'.'+[guid]::NewGuid().ToString('N')+'.pending'
    try {
        $bytes=[Text.Encoding]::UTF8.GetBytes(($Value|ConvertTo-Json -Depth 16 -Compress))
        if($bytes.Length -gt 131072){throw 'REFRESH_RECORD_CAP'}
        $stream=[IO.File]::Open($temporary,'CreateNew','Write','None')
        try{$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
        [IO.File]::Move($temporary,$Path)
    }finally{if([IO.File]::Exists($temporary)){[IO.File]::Delete($temporary)}}
}
function Refresh-Step($Context,[string]$Name,$Value) {
    $Context.sequence++
    Write-RefreshRecord (Join-Path $Context.operation ('step-{0:d3}.json' -f $Context.sequence)) @{
        step=$Name;value=$Value;utc=[DateTime]::UtcNow.ToString('o')}
}
function Close-RefreshHandles($Context) {
    # Also runs after failure acquiring images.lock: agent.lock may already be held.
    foreach($key in @('images','agent')) {
        if($Context.handles.ContainsKey($key) -and $Context.handles[$key]) {
            $Context.handles[$key].Dispose();$Context.handles.Remove($key)
        }
    }
}
function Assert-RefreshWork($Context) {
    if($Context.guard){if($Context.cleanup){$Context.guard.CheckWhole()}else{$Context.guard.CheckWork()}}
}
function Undo-RefreshFiles($Context,$Io) { Undo-RefreshObjectUpdate $Context $Io }
function Invoke-RefreshOperation($Context,$Io) {
    # Io has exactly three actor/task boundaries. Entry point supplies real implementations;
    # tests substitute them while this transaction preserves exact file objects.
    $Context.handles=@{};$Context.attempted=@();$Context.sequence=0;$Context.cleanup=$false
    $Context.beforeProcess=$null;$Context.currentActor=$null;$Context.lastStopped=$null;$Context.stopRequested=$false
    $Context.initialActor=$null
    $Context.knownPartial=@{}
    $Context.primary=$null;$rollback=$null;$success=$false;$created=$false;$after=$null
    $before=& $Io.validate $Context $Context.fromKind $Context.expectedActor
    if($Context.action -ceq 'Validate'){return @{validated=$true;read_only=$true;actor=$before}}
    if(Test-Path -LiteralPath $Context.operation){throw 'REFRESH_FRESH_OPERATION_REQUIRED'}
    [void][IO.Directory]::CreateDirectory($Context.operation);$created=$true
    foreach($sub in @('stage','backup','working','origin-bytes','quarantine')){[void][IO.Directory]::CreateDirectory((Join-Path $Context.operation $sub))}
    try {
        Refresh-Step $Context 'bound' @{plan_sha256=$Context.planSha;action=$Context.action;before=$before}
        Write-RefreshRecord (Join-Path $Context.operation 'filesystem-basis.json') @{
            directory=$Context.directory;from=$Context.from;to=$Context.to;maps=$Context.maps;objects=$Context.objects;
            initialObjects=$Context.initialObjects;recovery=$Context.recovery;originalPaths=$Context.originalPaths;plan_sha256=$Context.planSha}
        Assert-RefreshWork $Context;$Context.stopRequested=$true
        & $Io.stop $Context $Context.fromKind $before
        if(!$Context.handles.ContainsKey('agent') -or !$Context.handles.ContainsKey('images')){throw 'REFRESH_LOCKS_REQUIRED'}
        Assert-RefreshFiles $Context.directory $Context.from
        Assert-RefreshObjects $Context.directory $Context.initialObjects
        foreach($name in $RefreshNames) {
            [IO.File]::Copy((Join-Path $Context.source $name),(Join-Path $Context.operation ('stage/'+$name)),$false)
            [IO.File]::Copy((Join-Path $Context.directory $name),(Join-Path $Context.operation ('backup/'+$name)),$false)
            if((Lab-Hash (Join-Path $Context.operation ('stage/'+$name))) -cne $Context.to[$name] -or
               (Lab-Hash (Join-Path $Context.operation ('backup/'+$name))) -cne $Context.from[$name]){throw 'REFRESH_STAGE_PIN'}
        }
        Start-RefreshObjectUpdate $Context $Io
        Assert-RefreshFiles $Context.directory $Context.to
        Assert-RefreshObjects $Context.directory $Context.objects
        $Context.expectedObjects=$Context.objects
        Close-RefreshHandles $Context
        $after=& $Io.wake $Context $Context.toKind
        Refresh-Step $Context 'ready' $after;$success=$true
    }catch{
        $Context.primary=$_.Exception.Message;$Context.cleanup=$true
        try {
            Refresh-Step $Context 'failure' @{primary=$Context.primary}
            if($Context.attempted.Count) {
                if(!$Context.handles.ContainsKey('agent') -or !$Context.handles.ContainsKey('images')) {
                    # Only a fully identified, idle actor may be stopped after a failed Wake.
                    if(!$Context.currentActor){throw 'REFRESH_ACTOR_UNKNOWN_NO_ROLLBACK'}
                    & $Io.stop $Context $Context.toKind $Context.currentActor
                }
                Undo-RefreshFiles $Context $Io
                $Context.expectedObjects=$Context.initialObjects
                Close-RefreshHandles $Context
                $restored=& $Io.wake $Context $Context.fromKind
                $rollback=@{payload_restored=$true;availability_verified=$true;actor=$restored}
            }elseif($Context.lastStopped -and $Context.handles.ContainsKey('agent') -and $Context.handles.ContainsKey('images')){
                Assert-RefreshFiles $Context.directory $Context.from
                Assert-RefreshObjects $Context.directory $Context.initialObjects
                Close-RefreshHandles $Context
                $restored=& $Io.wake $Context $Context.fromKind
                $rollback=@{payload_unchanged=$true;availability_verified=$true;actor=$restored}
            }else{$rollback=@{payload_unchanged=$true;availability_verified=$false;stop_requested=$Context.stopRequested}}
        }catch{$rollback=@{blocked=$_.Exception.Message;availability_verified=$false}}
    }finally{
        try{Close-RefreshHandles $Context}finally{
            if($Context.beforeProcess){$Context.beforeProcess.Dispose();$Context.beforeProcess=$null}
            if($created){Write-RefreshRecord (Join-Path $Context.operation 'result.json') @{
                success=$success;primary=$Context.primary;rollback=$rollback;attempted=$Context.attempted;force_used=$false;
                plan_sha256=$Context.planSha;action=$Context.action;before=$before;after=$after;file_objects=$Context.expectedObjects;recovery=$Context.recovery}}
        }
    }
    return @{success=$success;primary=$Context.primary;rollback=$rollback;operation=$Context.operation;after=$after}
}

function Get-RefreshActor($Context,[string]$Kind,$Expected,[switch]$AllowFresh) {
    $p=$Context.plan;$pins=$Context.maps[$Kind];Assert-RefreshFiles $Context.directory $pins
    Assert-RefreshObjects $Context.directory $Context.expectedObjects
    $cfg=Lab-Config (Join-Path $Context.directory 'lab.json');$task=Find-LabTask (Lab-TaskSpec $cfg).name;Assert-LabTask $task (Lab-TaskSpec $cfg)
    $null=Assert-LabReadOnly $cfg $p.baseline_sha256
    $ready=Lab-Read (Join-Path $Context.directory 'control/ready.json');$state=Lab-Read (Join-Path $Context.directory 'control/state.json')
    if(!$ready -or $ready.host -cne $cfg.host -or $ready.sid -cne $cfg.sid -or $ready.administrator -ne $false -or
       $ready.logon_type -ne 2 -or $ready.windows_session -ne $p.windows_session -or $ready.profile_hive_loaded -ne $true -or
       $state.session -cne $ready.session -or $state.active -or $state.blocked){throw 'REFRESH_ACTOR_NOT_IDLE'}
    Lab-Hex $ready.session
    Assert-RefreshIdentity $ready $Expected $AllowFresh @($Context.initialActor,$Context.lastStopped)
    $actor=[KilogramLabNative]::OpenActor($ready.pid,$ready.started,[IO.Path]::GetFullPath("$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"),$cfg.sid,$true)
    try {
        if($actor.app_container -or $actor.elevated -or @($actor.restricting).Count){throw 'REFRESH_ACTOR_TOKEN'}
        $raw=@($actor.groups|Sort-Object sid|ForEach-Object {[ordered]@{sid=$_.sid;attributes=$_.attributes}})
        $expectedRaw=@($p.raw_groups|Sort-Object sid|ForEach-Object {[ordered]@{sid=$_.sid;attributes=$_.attributes}})
        if(($raw|ConvertTo-Json -Compress) -cne ($expectedRaw|ConvertTo-Json -Compress)){throw 'REFRESH_RAW_TOKEN'}
        $identity=[T13eToken]::Read($ready.pid,$ready.started)
        try {
            $groups=@($identity.Groups|ForEach-Object Value|Sort-Object -Unique)
            if($identity.User.Value -cne $cfg.sid -or ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or
               ($groups -join '|') -cne ($p.native_groups -join '|')){throw 'REFRESH_NATIVE_TOKEN'}
        }finally{$identity.Dispose()}
        $members=@([KilogramLabJob]::Inspect("Local\Kilogram-Lab-$($ready.session)"))
        if($members.Count -ne 1 -or $members[0] -ne $ready.pid -or !$actor.Alive()){throw 'REFRESH_SESSION_JOB_UNKNOWN'}
        $process=Get-Process -Id $ready.pid
        try{if($process.StartTime.ToFileTimeUtc() -ne $ready.started -or $process.SessionId -ne $p.windows_session){throw 'REFRESH_ACTOR_SESSION'}}finally{$process.Dispose()}
        return @{ready=$ready;native_groups=$groups;raw_groups=$raw;idle=$true;task_verified=$true;
            task_xml=$task.Xml;task_sddl=$task.GetSecurityDescriptor(7);payload_kind=$Kind;utc=[DateTime]::UtcNow.ToString('o')}
    }finally{$actor.Dispose()}
}
function Invoke-RefreshAgent($Context,[string]$Kind,[string]$Action,$Request) {
    Assert-RefreshFiles $Context.directory $Context.maps[$Kind]
    Assert-RefreshObjects $Context.directory $Context.expectedObjects
    $args=@{Action=$Action;Config=(Join-Path $Context.directory 'lab.json')}
    if($Request){$args.Request=$Request|ConvertTo-Json -Depth 10 -Compress}
    $text=(& (Join-Path $Context.directory 'test-terminal-t1-3e-lab-agent.ps1') @args) -join "`n"
    return $text|ConvertFrom-Json
}
function Get-RefreshRealIo {
    return @{
        validate={param($c,$kind,$expected)
            $snapshot=Get-RefreshActor $c $kind $expected
            Assert-RefreshPolicy $kind $snapshot.ready $c.plan.crash_policy_sha256
            $c.initialActor=$snapshot;return $snapshot
        }
        stop={param($c,$kind,$snapshot)
            $null=Get-RefreshActor $c $kind $snapshot.ready
            $process=Get-Process -Id $snapshot.ready.pid
            $null=$process.Handle # Retain before Stop; never wait on a reopened PID.
            if($process.StartTime.ToFileTimeUtc() -ne $snapshot.ready.started){$process.Dispose();throw 'REFRESH_STOP_PID_REUSE'}
            if($c.beforeProcess){$c.beforeProcess.Dispose()};$c.beforeProcess=$process
            $queue=Join-Path $c.directory ('control/'+$snapshot.ready.session)
            $known=@(Get-ChildItem -LiteralPath $queue -Filter '*.request.json' | ForEach-Object Name)
            try {Stop-LabIdleAgent $c.plan.base $c.handles}finally{
                foreach($entry in @(Get-ChildItem -LiteralPath $queue -Filter '*.request.json')){
                    if($entry.Name -cnotin $known){
                        $request=Lab-Read $entry.FullName
                        $response=Lab-Read ($entry.FullName.Replace('.request.json','.response.json'))
                        Refresh-Step $c 'stop-protocol' @{request=$request;response=$response}
                    }
                }
            }
            if(!$process.WaitForExit(5000)){throw 'REFRESH_STOP_NOT_EXITED'}
            $c.lastStopped=$snapshot;$c.currentActor=$null
            Refresh-Step $c 'stopped-locked' @{session=$snapshot.ready.session;pid=$snapshot.ready.pid;started=$snapshot.ready.started}
        }
        wake={param($c,$kind)
            Assert-RefreshWork $c
            $wake=Invoke-RefreshAgent $c $kind 'Wake' $null
            Refresh-Step $c 'task-wake' $wake
            $limit=[Diagnostics.Stopwatch]::StartNew();$fresh=$null;$last=$null
            do {
                Assert-RefreshWork $c
                try{$fresh=Get-RefreshActor $c $kind $null -AllowFresh;break}catch{$last=$_.Exception.Message}
                Start-Sleep -Milliseconds 150
            }while($limit.ElapsedMilliseconds -lt 30000)
            if(!$fresh){throw "REFRESH_NEW_ACTOR_UNPROVEN:$last"}
            $c.currentActor=$fresh # Known idle actor, even if later policy/readiness fails.
            Assert-RefreshPolicy $kind $fresh.ready $c.plan.crash_policy_sha256
            $request=@{v=1;session=$fresh.ready.session;id=[guid]::NewGuid().ToString('N');op='check';args=@{baseline_sha256=$c.plan.baseline_sha256}}
            Refresh-Step $c 'readiness-intent' $request
            $answer=Invoke-RefreshAgent $c $kind 'Command' $request
            Refresh-Step $c 'readiness-response' $answer
            Assert-RefreshReadiness $answer $request $fresh.ready $c.plan
            $final=Get-RefreshActor $c $kind $fresh.ready
            Assert-RefreshPolicy $kind $final.ready $c.plan.crash_policy_sha256
            return $final
        }
    }
}
function Assert-RefreshPolicy([string]$Kind,$Ready,[string]$Pin) {
    if($Kind -ceq 'new' -and (!$Ready.PSObject.Properties['crash_policy_version'] -or
       !$Ready.PSObject.Properties['crash_policy_sha256'] -or $Ready.crash_policy_version -cne 'owned-runtime-crash-v1' -or
       $Ready.crash_policy_sha256 -cne $Pin)){throw 'REFRESH_LOADED_POLICY'}
}
function Assert-RefreshIdentity($Ready,$Expected,[bool]$Fresh,$Prior) {
    if(!$Fresh -and ($Ready.session -cne $Expected.session -or $Ready.pid -ne $Expected.pid -or $Ready.started -ne $Expected.started)){throw 'REFRESH_ACTOR_CHANGED'}
    if($Fresh){foreach($actor in $Prior){
        if($actor -and ($Ready.session -ceq $actor.ready.session -or
           ($Ready.pid -eq $actor.ready.pid -and $Ready.started -eq $actor.ready.started))){throw 'REFRESH_STALE_READY'}
    }}
}
function Assert-RefreshReadiness($Answer,$Request,$Ready,$Plan) {
    if(!$Answer.ok -or $Answer.id -cne $Request.id -or $Answer.session -cne $Ready.session -or
       $Answer.value.dpapi -ne $true -or $Answer.value.logon_type -ne 2 -or $Answer.value.sid -cne $Plan.sid -or
       $Answer.value.windows_session -ne $Plan.windows_session -or $Answer.value.backend -cne 'com-native-v1' -or
       $Answer.value.session -cne $Ready.session -or $Answer.value.baseline_sha256 -cne $Plan.baseline_sha256){throw 'REFRESH_DPAPI_READINESS'}
    Lab-Hex $Answer.value.id
}
function Assert-RefreshBindings($p,[string]$Root) {
    if($p.v -ne 2 -or $p.base -ine (Join-Path $Root '.tmp/terminal-t11f') -or $p.side -cne 'A' -or
       $p.host -cne 'DESKTOP-PAUL-RY' -or $p.host -cne [Environment]::MachineName -or
       $p.sid -cne 'S-1-5-21-1776862868-2140151431-2099235586-1001' -or
       $p.config_sha256 -cne 'ad8489114462306ad56110bf357eb2e4dbefe20ec096a7636c180211da7e1056' -or
       $p.baseline_sha256 -cne 'b46b8cca1f3793a97acb920fd47fabe392a2d8d49bc2d553d1dbac6dc0ab8c42' -or $p.windows_session -ne 1 -or
       $p.crash_policy_sha256 -cne 'b9b280677ed1cb0d612223bfab7ecad306ec976ae5b042a76b77db581aa9da65' -or
       $p.work_seconds -ne 180 -or $p.whole_seconds -ne 200 -or
       $p.operation -cnotmatch '^(refresh|restore|recover)-a-[0-9]{2}$'){throw 'REFRESH_FIXED_SCOPE'}
    $sources=@('scripts/test-terminal-t1-3e-refresh-a.ps1','scripts/test-terminal-t1-3e-lab-common.ps1',
        'scripts/test-terminal-t1-3e-observe.ps1','scripts/migration-s5-l1-deadline.ps1','scripts/test-terminal-t1-3e-refresh-files.ps1')
    Lab-Keys $p.sources $sources
    foreach($name in $sources){if((Lab-Hash (Join-Path $Root $name)) -cne $p.sources.$name){throw 'REFRESH_SOURCE_PIN'}}
    $fixed=@{
        old=@{directory='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh/old-payload';manifest='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh/old-manifest.json';sha256='fc3bf5b0713877c07b335f048922b2c9d52b75ba816a0509f41774c55facbead'}
        new=@{directory='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh-02/new-payload';manifest='.tmp/terminal-t1-3e/sender-crash-prep-01/refresh-02/new-manifest.json';sha256='5a2784caff98850ddffe26e1a49e837e49da634abe7f098dff8b1729c7eda764'}
    }
    foreach($kind in @('old','new')){foreach($key in @('directory','manifest','sha256')){
        if($p.payload.$kind.$key -cne $fixed[$kind][$key]){throw 'REFRESH_FIXED_PAYLOAD'}
    }}
}

if($Action -ceq 'Library'){return}
if(!$Plan -or !$PlanSha256){throw 'Reviewed refresh plan/hash required'}
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
if((Lab-Hash $Plan) -cne $PlanSha256){throw 'REFRESH_PLAN_PIN'}
$refreshPlan=Lab-Read $Plan
$root=Split-Path $PSScriptRoot -Parent
Assert-RefreshBindings $refreshPlan $root
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
$guard=[KilogramS5Deadline]::new(180,200)
try {
$maps=@{};$roots=@{}
foreach($kind in @('old','new')) {
    $binding=$refreshPlan.payload.$kind;$manifestPath=Join-Path $root $binding.manifest
    if((Lab-Hash $manifestPath) -cne $binding.sha256){throw 'REFRESH_MANIFEST_PIN'}
    $manifest=Lab-Read $manifestPath
    if($manifest.v -ne 2 -or $manifest.side -cne 'A' -or @($manifest.files.PSObject.Properties).Count -ne 15){throw 'REFRESH_PAYLOAD_SET'}
    $maps[$kind]=Refresh-Map $manifest;$roots[$kind]=Join-Path $root $binding.directory;Assert-RefreshFiles $roots[$kind] $maps[$kind]
}
if((($maps.old.Keys|Sort-Object)-join '|') -cne (($maps.new.Keys|Sort-Object)-join '|')){throw 'REFRESH_PAYLOAD_SET'}
foreach($name in $maps.old.Keys){if($name -cnotin $RefreshNames -and $maps.old[$name] -cne $maps.new[$name]){throw 'REFRESH_ONLY_TWO_FILES'}}
if($maps.old['lab.json'] -cne $refreshPlan.config_sha256){throw 'REFRESH_CONFIG_PIN'}
$objects=@{};Lab-Keys $refreshPlan.file_objects @($maps.old.Keys)
foreach($name in $maps.old.Keys){
    $value=$refreshPlan.file_objects.$name;Lab-Keys $value @('key','sddl')
    if($value.key -cnotmatch '^[0-9a-f]{8}:[0-9a-f]{16}$' -or $value.sddl -isnot [string] -or $value.sddl.Length -gt 16384){throw 'REFRESH_OBJECT_PIN'}
    $objects[$name]=$value
}
$recovery=$refreshPlan.phase -ceq 'Recover';$initialObjects=$objects;$originalPaths=@{}
if($recovery){
    Lab-Keys $refreshPlan.from_objects @($maps.old.Keys);$initialObjects=@{}
    foreach($name in $maps.old.Keys){
        $value=$refreshPlan.from_objects.$name;Lab-Keys $value @('key','sddl');$initialObjects[$name]=$value
        if($name -cnotin $RefreshNames -and ($value.key -cne $objects[$name].key -or $value.sddl -cne $objects[$name].sddl)){throw 'REFRESH_ONLY_TWO_OBJECTS'}
    }
    foreach($name in $RefreshNames){
        $originalPaths[$name]=Join-Path $root ('.tmp/terminal-t1-3e/agent-refresh/refresh-a-01/replacement-backup/'+$name)
        Assert-RefreshObject (Get-RefreshObject $originalPaths[$name]) $objects[$name]
        if((Lab-Hash $originalPaths[$name]) -cne $maps.old[$name]){throw 'REFRESH_RECOVERY_ORIGINAL_PIN'}
    }
}
Add-Type -Path (Join-Path $roots.old 'test-terminal-t1-3e-lab-job.cs')
Add-Type -Path (Join-Path $roots.old 'test-terminal-t1-3e-lab-native.cs')
. (Join-Path $roots.old 'test-terminal-t1-3e-lab-install.ps1') -Mode Library
# Reuse the reviewed WindowsIdentity collector, without its command-line query helper.
$observer=[IO.File]::ReadAllText((Join-Path $root 'scripts/test-terminal-t1-3e-observe.ps1')).Replace("`r`n","`n")
$start=$observer.IndexOf("Add-Type -TypeDefinition @'`nusing System;`nusing System.ComponentModel;")
$native=$observer.Substring($start,$observer.IndexOf("`n'@",$start)+4-$start)
$a=$native.IndexOf(' [DllImport("ntdll.dll")]');$b=$native.IndexOf(' public static WindowsIdentity Read(',$a)
. ([scriptblock]::Create($native.Substring(0,$a)+$native.Substring($b)))
$from=if($refreshPlan.phase -ceq 'Apply'){'old'}elseif($refreshPlan.phase -cin @('Restore','Recover')){'new'}else{throw 'REFRESH_PHASE'}
$to=if($recovery){'new'}elseif($from -ceq 'old'){'new'}else{'old'}
$context=@{plan=$refreshPlan;planSha=$PlanSha256;action=$Action;expectedActor=$refreshPlan.expected_actor;fromKind=$from;toKind=$to;
    directory=(Join-Path $refreshPlan.base 'ordinary-agent');from=$maps[$from];to=$maps[$to];maps=$maps;source=$roots[$to];guard=$guard;
    objects=$objects;initialObjects=$initialObjects;expectedObjects=$initialObjects;recovery=$recovery;originalPaths=$originalPaths}
    if($Action -cne 'Validate') {
        if($Action -cne $refreshPlan.phase -or !$Operation -or !$Acceptance -or !$AcceptanceSha256 -or (Lab-Hash $Acceptance) -cne $AcceptanceSha256){throw 'REFRESH_ACCEPTANCE_REQUIRED'}
        $approval=Lab-Read $Acceptance
        $operationRoot=[IO.Path]::GetFullPath((Join-Path $root ('.tmp/terminal-t1-3e/agent-refresh/'+$refreshPlan.operation)))
        Lab-Plain ([IO.Path]::GetDirectoryName($operationRoot))
        if([IO.Path]::GetFullPath($Operation) -ine $operationRoot -or $approval.authorized -ne $true -or $approval.plan_sha256 -cne $PlanSha256 -or
           $approval.phase -cne $Action -or $approval.operation -cne $refreshPlan.operation -or
           $approval.runner_sha256 -cne (Lab-Hash $PSCommandPath) -or
           ((& git -C $root rev-parse HEAD) -join '') -cne $approval.commit){throw 'REFRESH_ACTIVATION_BINDING'}
        $context.operation=$operationRoot
    }
    $result=Invoke-RefreshOperation $context (Get-RefreshRealIo)
    $result|ConvertTo-Json -Depth 12 -Compress
    if($Action -cne 'Validate' -and !$result.success){exit 1}
}finally{$guard.Finish()}
