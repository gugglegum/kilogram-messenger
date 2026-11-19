# Administrative preflight only. All service/socket/token operations are read-only.
function Get-LabServerFacts($Cfg) {
    $rows=@(Get-CimInstance -ClassName Win32_Service -Filter "Name='sshd'" -ErrorAction Stop)
    $image=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
    if($rows.Count -ne 1 -or $rows[0].State -cne 'Running' -or [int]$rows[0].ProcessId -le 0 -or
       [string]$rows[0].StartName -inotmatch '^(LocalSystem|NT AUTHORITY\\SYSTEM)$' -or
       ([Environment]::ExpandEnvironmentVariables([string]$rows[0].PathName)).Trim('"') -ine $image){throw 'LAB_SERVER_SERVICE_BINDING'}
    $tcp=@(Get-NetTCPConnection -ErrorAction Stop)
    $listeners=@($tcp|Where-Object {[int]$_.LocalPort -eq 22 -and [string]$_.State -ceq 'Listen' -and $_.LocalAddress -in @('0.0.0.0','::',$Cfg.ip)})
    if(!$listeners.Count -or @($listeners|Where-Object {[int]$_.OwningProcess -ne [int]$rows[0].ProcessId}).Count){throw 'LAB_SERVER_LISTENER_BINDING'}
    $flows=@($tcp|Where-Object {[int]$_.LocalPort -eq 22 -and [string]$_.State -ceq 'Established' -and $_.LocalAddress -ceq $Cfg.ip -and $_.RemoteAddress -ceq $Cfg.peer})
    $owners=@(@([int]$rows[0].ProcessId)+@($flows|ForEach-Object {[int]$_.OwningProcess})|Sort-Object -Unique)
    if($owners.Count -gt 16 -or @($owners|Where-Object {$_ -le 0}).Count){throw 'LAB_SERVER_OWNER_BOUND'}
    @{service_pid=[int]$rows[0].ProcessId;service_account=[string]$rows[0].StartName;image=$image;owners=$owners;
        listeners=@($listeners|ForEach-Object {@{pid=[int]$_.OwningProcess;address=[string]$_.LocalAddress;port=22}});
        flows=@($flows|ForEach-Object {@{pid=[int]$_.OwningProcess;local=[string]$_.LocalAddress;remote=[string]$_.RemoteAddress;remote_port=[int]$_.RemotePort}})}
}
function Open-LabServerActors($Cfg) {
    if($Cfg.side -cne 'B'){throw 'LAB_SERVER_SIDE'}
    Initialize-LabNative
    $facts=Get-LabServerFacts $Cfg;$members=@();$bound=$null
    try{
        foreach($id in $facts.owners){
            try{$members+=,[KilogramLabNative]::OpenServerActor($id,$facts.image,$Cfg.sid,($id -eq $facts.service_pid))}
            catch{throw "LAB_SERVER_TOKEN_UNAVAILABLE:pid=$id;service=$($facts.service_pid);error=$($_.Exception.Message)"}
        }
        $bound=[KilogramLabNative]::BindServerActors($members,$facts.service_pid,$facts.image,$Cfg.sid)
        $null=Assert-LabServerActors $Cfg $bound
        return $bound
    }catch{if($bound){$bound.Dispose()}else{foreach($a in $members){$a.Dispose()}};throw}
}
function Assert-LabServerActors($Cfg,$Actors) {
    $image=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
    if($Cfg.side -cne 'B' -or $Actors -isnot [KilogramLabNative+ServerActors] -or !$Actors.Alive() -or
       $Actors.ordinary_sid -cne $Cfg.sid -or $Actors.image -ine $image){throw 'LAB_SERVER_ACTOR_BINDING'}
    $facts=Get-LabServerFacts $Cfg
    if($facts.service_pid -ne $Actors.service_pid -or @($facts.owners|Where-Object {!$Actors.Contains($_)}).Count){throw 'LAB_SERVER_OWNER_CHANGED'}
    if(!$Actors.Alive()){throw 'LAB_SERVER_ACTOR_EXITED'}
    return $facts
}
function Get-LabSshEvidence($Cfg,$Snapshot,$Actor) {
    $members=@();$binding=$null
    if($Actor -and $Actor -is [KilogramLabNative+ServerActors]){$binding=Assert-LabServerActors $Cfg $Actor;$members=@($Actor.Members())}
    elseif($Actor -and $Actor -is [KilogramLabNative+Actor]){$members=@($Actor)}
    $rules=@(foreach($rule in $Snapshot.policy){
        if((Lab-RuleField $rule Action) -ine 'Block'){continue}
        $copy=$rule|ConvertTo-Json -Depth 12 -Compress|ConvertFrom-Json;$copy.LocalUser='Any'
        foreach($reply in @($false,$true)){
            if((Test-LabSshRule $copy $Cfg ($Cfg.side -ceq 'A') $false $reply) -ge 0){
                @{rule=$rule;reply=$reply;verdict=(Test-LabSshRule $rule $Cfg ($Cfg.side -ceq 'A') $false $reply $Actor)}
            }
        }
    })
    if($rules.Count -gt 32){throw 'LAB_SSH_EVIDENCE_BOUND'}
    @{role=$(if($Cfg.side -ceq 'B'){'server'}else{'client'});binding=$binding;rules=$rules;
        actors=@($members|ForEach-Object {@{pid=$_.pid;started=$_.started;image=$_.image;user=$_.user;elevated=$_.elevated;groups=$_.groups;restricting=$_.restricting;alive=$_.Alive()}})}
}
