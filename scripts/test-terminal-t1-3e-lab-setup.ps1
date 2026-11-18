# Fixed setup helpers. Loaded by the pinned installer; no arbitrary elevated dispatcher.
function Get-LabSetupSession($Cfg,[string]$SessionPath) {
    $leaf=Split-Path -Leaf $SessionPath
    if($leaf -cnotmatch '^setup-[0-9a-f]{32}$' -or [IO.Path]::GetFullPath($SessionPath) -ine (Join-Path $Cfg.base "deploy/$leaf")){throw 'LAB_SETUP_SESSION'}
    Lab-Plain $SessionPath
    return $SessionPath
}
function Open-LabSetupActor($Cfg,[string]$SessionPath,[ValidateRange(1,45)][int]$AdmissionSeconds=45) {
    $null=Get-LabSetupSession $Cfg $SessionPath
    if($Cfg.side -ceq 'B'){return $null}
    $wait=[Diagnostics.Stopwatch]::StartNew()
    do {
        $row=Lab-Read (Join-Path $SessionPath 'actor.json')
        if($row){break};Start-Sleep -Milliseconds 100
    }while($wait.Elapsed.TotalSeconds -lt $AdmissionSeconds)
    if(!$row){throw "LAB_SSH_ACTOR_TIMEOUT:phase=identity;elapsed_ms=$($wait.ElapsedMilliseconds)"}
    Lab-Keys $row @('pid','started','session')
    if($row.session -cne (Split-Path -Leaf $SessionPath) -or $row.pid -isnot [int] -or $row.started -le 0){throw 'LAB_SSH_ACTOR_SCHEMA'}
    Initialize-LabNative
    $actor=[KilogramLabNative]::OpenActor($row.pid,$row.started,"$env:windir\System32\OpenSSH\ssh.exe",$Cfg.sid,$true)
    $reads=0
    try {
        while($wait.Elapsed.TotalSeconds -lt $AdmissionSeconds){
            if(!$actor.Alive()){throw "LAB_SSH_ACTOR_EXITED:pid=$($actor.pid);started=$($row.started);reads=$reads;elapsed_ms=$($wait.ElapsedMilliseconds)"}
            # Querying a just-created PID with State/Owner filters can itself raise native
            # ObjectNotFound while connect is pending. Enumerate read-only, then filter the
            # exact owner/route/state; provider errors remain failures, never absence.
            $sockets=@(Get-NetTCPConnection -ErrorAction Stop | Where-Object {$_.OwningProcess -eq $actor.pid -and [string]$_.State -ceq 'Established' -and
                $_.LocalAddress -ceq $Cfg.ip -and $_.RemoteAddress -ceq $Cfg.peer -and $_.RemotePort -eq 22})
            $reads++
            if(!$actor.Alive()){throw "LAB_SSH_ACTOR_EXITED:pid=$($actor.pid);started=$($row.started);reads=$reads;elapsed_ms=$($wait.ElapsedMilliseconds)"}
            if($sockets.Count -gt 1){throw "LAB_SSH_ACTOR_SOCKET_AMBIGUOUS:pid=$($actor.pid);reads=$reads"}
            if($sockets.Count -eq 1 -and $wait.Elapsed.TotalSeconds -lt $AdmissionSeconds){return $actor}
            Start-Sleep -Milliseconds 100
        }
        throw "LAB_SSH_ACTOR_TIMEOUT:phase=socket;pid=$($actor.pid);started=$($row.started);reads=$reads;elapsed_ms=$($wait.ElapsedMilliseconds)"
    }catch{$actor.Dispose();throw}
}
function Get-LabSetupSnapshot($Cfg,$Want,$Actor,[IO.FileStream]$InstallLock) {
    if(!$InstallLock -or !$InstallLock.CanWrite -or $InstallLock.SafeFileHandle.IsClosed){throw 'LAB_INSTALL_LOCK_REQUIRED'}
    $snapshot=Get-LabSnapshot $Cfg $Want
    Assert-LabNetwork $Cfg $snapshot $Actor
    # Capability/projection validation belongs to this newly observed transaction state.
    # Owned rules are checked against both stores after creation, before install success.
    $ordinaryBefore=Get-LabOrdinaryState $Cfg $Want -AllowMissing
    $projection=$ordinaryBefore | ConvertTo-Json -Depth 18 -Compress | ConvertFrom-Json
    $projection.rules=@()
    $relevant=@{profiles=$snapshot.profiles;rules=@();network=$snapshot.network;adapter=$snapshot.adapter}
    Assert-LabOrdinaryAgainstCim $Cfg @{rules=@()} $projection $relevant
    return @{snapshot=$snapshot;sha256=(Lab-JsonHash $snapshot);ordinary=$ordinaryBefore}
}
function Invoke-LabTaskProbe($Cfg,$TaskSpec,[string]$Pin) {
    $task=Find-LabTask $TaskSpec.name;Assert-LabTask $task $TaskSpec
    $control=Join-Path $Cfg.base 'ordinary-agent/control'
    $null=$task.Run($null)
    $wait=[Diagnostics.Stopwatch]::StartNew();$ready=$null
    do {
        $ready=Lab-Read (Join-Path $control 'ready.json')
        if($ready){
            try{
                $p=[Diagnostics.Process]::GetProcessById($ready.pid)
                try{if($p.StartTime.ToFileTimeUtc() -ne $ready.started -or $p.HasExited){$ready=$null}}
                finally{$p.Dispose()}
            }catch{$ready=$null}
        }
        if($ready){break};Start-Sleep -Milliseconds 100
    }while($wait.Elapsed.TotalSeconds -lt 25)
    if(!$ready -or $ready.sid -cne $Cfg.sid -or $ready.host -cne $Cfg.host -or $ready.logon_type -ne 2 -or $ready.windows_session -le 0 -or !$ready.profile_hive_loaded -or $ready.administrator){throw 'LAB_TASK_CONTEXT_UNAVAILABLE'}
    Initialize-LabNative
    $agentActor=[KilogramLabNative]::OpenActor($ready.pid,$ready.started,"$env:windir\System32\WindowsPowerShell\v1.0\powershell.exe",$Cfg.sid,$true)
    try {
    Lab-Hex $ready.session
    # Admin submits one immutable fixed read-only check; it cannot override the task action.
    $id=[guid]::NewGuid().ToString('N');$queue=Join-Path $control $ready.session
    $req=@{v=1;session=$ready.session;id=$id;op='check';args=@{baseline_sha256=$Pin}}
    Lab-Write (Join-Path $queue "$id.request.json") $req
    $wait.Restart()
    do {
        $reply=Lab-Read (Join-Path $queue "$id.response.json")
        if($reply){break};Start-Sleep -Milliseconds 80
    }while($wait.Elapsed.TotalSeconds -lt 22)
    if(!$reply -or !$reply.ok -or $reply.id -cne $id -or $reply.session -cne $ready.session -or
       !$reply.value.dpapi -or $reply.value.sid -cne $Cfg.sid -or $reply.value.logon_type -ne 2 -or $reply.value.windows_session -ne $ready.windows_session -or
       $reply.value.backend -cne 'com-native-v1' -or $reply.value.baseline_sha256 -cne $Pin -or !$agentActor.Alive()){throw 'LAB_TASK_READINESS_UNPROVEN'}
    return @{state='verified';session=$ready.session;pid=$ready.pid;started=$ready.started;baseline_sha256=$Pin;dpapi=$true;backend=$reply.value.backend}
    }finally{$agentActor.Dispose()}
}
function Assert-LabPackage([string]$Root,[string]$Pin,[string]$Which,[string[]]$Names,$FixedConfig,$Handles) {
    Lab-Hex $Pin 64
    $manifest=Join-Path $Root 'package.json';Lab-Plain $manifest
    $stream=[IO.File]::OpenRead($manifest);$Handles.Add($stream)
    if((Get-FileHash -InputStream $stream).Hash -ine $Pin){throw 'LAB_PACKAGE_PIN'}
    $package=Lab-Read $manifest
    Lab-Keys $package @('v','side','files');Lab-Keys $package.files $Names
    if($package.v -ne 2 -or $package.side -cne $Which){throw 'LAB_PACKAGE_SCOPE'}
    foreach($name in $Names){
        $path=Join-Path $Root $name;Lab-Plain $path
        $stream=[IO.File]::OpenRead($path);$Handles.Add($stream)
        if((Get-FileHash -InputStream $stream).Hash -ine $package.files.$name){throw 'LAB_PACKAGE_PIN'}
    }
    if(!(Lab-Equal (Lab-Read (Join-Path $Root 'lab.json')) $FixedConfig)){throw 'LAB_PACKAGE_CONFIG'}
    return $package
}
