# Shared plain-file JSON protocol. This file never changes firewall or Task Scheduler.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
function Lab-Plain([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    while ($item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'LAB_REPARSE' }
        $item = if ($item -is [IO.FileInfo]) { $item.Directory } else { $item.Parent }
    }
}
function Lab-Read([string]$Path,[ValidateRange(2,2097152)][int]$MaxBytes=131072) {
    if (![IO.File]::Exists($Path)) { return $null }
    Lab-Plain $Path
    $stream = [IO.File]::Open($Path,'Open','Read',('ReadWrite, Delete'))
    try {
        if ($stream.Length -lt 2 -or $stream.Length -gt $MaxBytes) { throw 'LAB_JSON_SIZE' }
        $reader = [IO.StreamReader]::new($stream,[Text.UTF8Encoding]::new($false,$true))
        try { return ($reader.ReadToEnd() | ConvertFrom-Json) } finally { $reader.Dispose() }
    } finally { $stream.Dispose() }
}
function Lab-Write([string]$Path,$Value,[ValidateRange(2,2097152)][int]$MaxBytes=131072) {
    Lab-Plain ([IO.Path]::GetDirectoryName($Path))
    if ([IO.File]::Exists($Path)) { Lab-Plain $Path }
    $temp = "$Path.$([guid]::NewGuid().ToString('N')).pending"
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 18 -Compress)+[char]10)
    if($bytes.Length -lt 2 -or $bytes.Length -gt $MaxBytes){throw 'LAB_JSON_SIZE'}
    $f = [IO.File]::Open($temp,'CreateNew','Write','None')
    try { $f.Write($bytes,0,$bytes.Length); $f.Flush($true) } finally { $f.Dispose() }
    try {
        if ([IO.File]::Exists($Path)) { [IO.File]::Replace($temp,$Path,[NullString]::Value) }
        else { [IO.File]::Move($temp,$Path) }
    } finally { [IO.File]::Delete($temp) }
}
function Lab-Keys($Value,[string[]]$Keys) {
    if ($Value -isnot [pscustomobject] -or
        (@($Value.PSObject.Properties | ForEach-Object Name | Sort-Object) -join '|') -cne (@($Keys | Sort-Object) -join '|')) {
        throw 'LAB_SCHEMA'
    }
}
function Lab-Hex($Value,[int]$Length=32) {
    if ($Value -isnot [string] -or $Value -cnotmatch "\A[0-9a-f]{$Length}\z") { throw 'LAB_HEX' }
}
function Lab-Id($Value) {
    if ($Value -isnot [string] -or $Value -cnotmatch '\A[a-z0-9][a-z0-9-]{0,30}\z') { throw 'LAB_ID' }
}
function Lab-Hash([string]$Path) { Lab-Plain $Path; (Get-FileHash -LiteralPath $Path).Hash.ToLowerInvariant() }
function Assert-LabCrashIntent($Intent,$Entry,$Owner,$Cfg,$Started) {
    Lab-Keys $Intent @('v','request','session','attempt','attempt_generation','owner_tag','owner_generation','worker_pid','worker_started',
        'runtime_pid','runtime_started','runtime_image','runtime_sha256','runtime_instance','descriptor_sha256','proof_sha256','contexts','profile_name','profile_sha256')
    foreach($k in @('request','session','attempt_generation','owner_generation')){Lab-Hex $Intent.$k}
    foreach($k in @('runtime_sha256','runtime_instance','descriptor_sha256','proof_sha256','profile_sha256')){Lab-Hex $Intent.$k 64}
    Lab-Id $Intent.attempt
    if($Intent.v -ne 1 -or $Cfg.side -cne 'A' -or $Owner.mode -cne 'chat' -or $Owner.tag -cne 'owner-a-1' -or
       !$Owner.PSObject.Properties['expected_runtime_crash'] -or $Owner.expected_runtime_crash -isnot [bool] -or $Owner.expected_runtime_crash -ne $true -or
       $Intent.session -cne $Entry.session -or $Intent.attempt -cne $Entry.attempt -or $Intent.attempt_generation -cne $Entry.generation -or
       $Intent.owner_tag -cne $Owner.tag -or $Intent.owner_generation -cne $Owner.generation -or
       $Intent.worker_pid -ne $Started.pid -or $Intent.worker_started -ne $Started.started -or
       $Intent.runtime_sha256 -cne $Entry.pins.'kilogram-cli.exe' -or
       $Intent.runtime_image -ine (Join-Path $Cfg.base 'bin/kilogram-cli.exe') -or
       $Intent.profile_name -cnotmatch '^profile-[0-9a-f]{64}\.json$' -or
       @($Intent.contexts).Count -ne 2 -or $Intent.contexts[0] -ceq $Intent.contexts[1]) {throw 'LAB_CRASH_BINDING'}
    foreach($c in $Intent.contexts){Lab-Hex $c 64}
    foreach($k in @('runtime_pid','runtime_started','worker_pid','worker_started')) {
        if(($Intent.$k -isnot [int] -and $Intent.$k -isnot [long]) -or $Intent.$k -le 0){throw 'LAB_CRASH_PROCESS'}
    }
}
function Assert-LabCrashPending($Proof,$Intent) {
    if($Proof.recipient_stopped -ne $true -or $Proof.sender_instance -cne $Intent.runtime_instance -or
       @($Proof.expected).Count -ne 2 -or @($Proof.requests).Count -ne 2){throw 'LAB_CRASH_PENDING'}
    for($i=0;$i -lt 2;$i++) {
        $f=$Proof.expected[$i];$p=$Proof.requests[$i]
        Lab-Hex $f.key.request_id;Lab-Hex $f.event_id 64
        if($f.slot -ne $i -or $p.slot -ne $i -or $f.key.context -cne $Intent.contexts[$i] -or
           $p.key.context -cne $f.key.context -or $p.key.request_id -cne $f.key.request_id -or @($f.history).Count -ne 1 -or
           $f.history[0].event_id -cne $f.event_id -or $f.history[0].text -cne $f.text){throw 'LAB_CRASH_PENDING'}
        foreach($sample in @($p.first,$p.second)) {
            if($sample.request.state -cne 'materialized' -or $sample.request.event_id -cne $f.event_id -or
               ($sample.sender_history|ConvertTo-Json -Depth 12 -Compress) -cne ($f.history|ConvertTo-Json -Depth 12 -Compress)){throw 'LAB_CRASH_PENDING'}
        }
    }
}
function Get-LabCrashEvidence([string]$Run,$Entry,$Owner,$Cfg,$Started) {
    $intent=Lab-Read (Join-Path $Run 'receipts/runtime-fault-request.json')
    if(!$intent){throw 'LAB_CRASH_NO_INTENT'}
    Assert-LabCrashIntent $intent $Entry $Owner $Cfg $Started
    $digest=Lab-Hash (Join-Path $Run 'receipts/runtime-fault-request.json')
    $proof=Join-Path $Run 'in/crash-pending.json'
    if((Lab-Hash $proof) -cne $intent.proof_sha256){throw 'LAB_CRASH_PROOF_PIN'}
    Assert-LabCrashPending (Lab-Read $proof) $intent
    return @{intent=$intent;digest=$digest}
}
function Set-LabCrashRequest([string]$Run,$Intent) {
    $path=Join-Path $Run 'receipts/runtime-fault-request.json';Lab-Plain ([IO.Path]::GetDirectoryName($path))
    $json=$Intent|ConvertTo-Json -Depth 12 -Compress
    if(![IO.File]::Exists($path)) {
        $temp=$path+'.'+[guid]::NewGuid().ToString('N')+'.pending'
        try {
            $bytes=[Text.Encoding]::UTF8.GetBytes($json);$f=[IO.File]::Open($temp,'CreateNew','Write','None')
            try{$f.Write($bytes,0,$bytes.Length);$f.Flush($true)}finally{$f.Dispose()}
            try{[IO.File]::Move($temp,$path)}catch{if(![IO.File]::Exists($path)){throw}}
        }finally{if([IO.File]::Exists($temp)){[IO.File]::Delete($temp)}}
    }
    $prior=Lab-Read $path
    if(($prior|ConvertTo-Json -Depth 12 -Compress) -cne $json){throw 'LAB_CRASH_REPLAY'}
    return Lab-Hash $path
}
# Separate expected-fault predicate; Lab-OwnerClean remains the normal rule.
function Assert-LabExpectedCrash([string]$Run,$Entry,$Owner,$Cfg,$Started,$Process) {
    $binding=Get-LabCrashEvidence $Run $Entry $Owner $Cfg $Started;$i=$binding.intent
    $fault=Lab-Read (Join-Path $Run 'receipts/runtime-fault-result.json')
    $claim=Lab-Read (Join-Path $Run 'receipts/runtime-fault-claim.json')
    $receipt=Lab-Read (Join-Path $Run "receipts/$($Owner.tag).json")
    if(!$fault -or !$claim -or !$receipt){throw 'LAB_CRASH_OUTCOME'}
    foreach($k in @('runtime_pid','runtime_started','owner_pid','owner_started','chat_pid','exit','observed_tick')) {
        if(($fault.$k -isnot [int] -and $fault.$k -isnot [long]) -or $fault.$k -le 0){throw 'LAB_CRASH_OUTCOME'}
    }
    if(($claim.tick -isnot [int] -and $claim.tick -isnot [long]) -or $claim.tick -le 0 -or $claim.tick -lt $Entry.start_tick){throw 'LAB_CRASH_OUTCOME'}
    foreach($k in @('membership','forced','work_expired','mailbox_eof','mailbox_quit','descriptor')) {
        if($receipt.$k -isnot [bool]){throw 'LAB_CRASH_OUTCOME'}
    }
    if(!$fault -or !$claim -or !$receipt -or (Test-Path -LiteralPath (Join-Path $Run 'receipts/runtime-fault-error.json')) -or
       $claim.digest -cne $binding.digest -or $fault.digest -cne $binding.digest -or
       $fault.request -cne $i.request -or $fault.runtime_pid -ne $i.runtime_pid -or $fault.runtime_started -ne $i.runtime_started -or
       $fault.runtime_image -ine $i.runtime_image -or $fault.runtime_instance -cne $i.runtime_instance -or $fault.exit -ne 137 -or
       $fault.owner_pid -ne $Process.Id -or $fault.owner_started -ne $Process.StartTime.ToFileTimeUtc() -or
       $claim.owner_pid -ne $Process.Id -or $fault.chat_pid -ne $receipt.child -or $fault.chat_pid -eq $i.runtime_pid -or
       $fault.observed_tick -lt $claim.tick -or $fault.observed_tick -ge $Started.work_tick -or
       $Process.ExitCode -ne 1 -or $receipt.owner -ne $Process.Id -or $receipt.host -cne $Cfg.host -or $receipt.tag -cne $Owner.tag -or
       $receipt.child_exit -ne 3 -or $receipt.membership -ne $true -or $receipt.forced -ne $false -or
       @($receipt.before).Count -or @($receipt.after).Count -or $receipt.work_expired -ne $false -or
       $receipt.mailbox_eof -ne $true -or $receipt.mailbox_quit -ne $false -or $receipt.descriptor -ne $true -or
       $receipt.error -cne 'Mailbox launch, stop or exchange failed' -or
       (Lab-Hash (Join-Path $Run 'setup/connection/ipc.json')) -cne $i.descriptor_sha256 -or
       (Lab-Hash (Join-Path $Run ('setup/connection/'+$i.profile_name))) -cne $i.profile_sha256){throw 'LAB_CRASH_OUTCOME'}
    $dir=Join-Path $Run 'setup/lab-diagnostics-v1'
    $kinds=@{}
    foreach($name in @('connection-final','worker-final')) {
        $v=Lab-Read (Join-Path $dir "$name-$($fault.chat_pid).json")
        if(!$v -or $v.schema -ne 1 -or $v.run_nonce -cne $Entry.generation -or $v.pid -ne $fault.chat_pid -or
           $v.runtime_pid -ne $i.runtime_pid -or $v.runtime_instance -cne $i.runtime_instance -or
           $v.kind.Replace('_','-') -cne $name){throw 'LAB_CRASH_PARENT'}
        $kinds[$name]=$v.data
    }
    $c=$kinds['connection-final'];$w=$kinds['worker-final']
    foreach($k in @('was_running','child_query_failed','ack_attempted','ack','forced','exit_success','eof')){if($c.$k -isnot [bool]){throw 'LAB_CRASH_PARENT'}}
    foreach($k in @('running_ok','saved_ok','stop_graceful','stop_forced','reaped','eof')){if($w.$k -isnot [bool]){throw 'LAB_CRASH_PARENT'}}
    foreach($b in @($c.drain_eof)+@($c.drain_failed)){if($b -isnot [bool]){throw 'LAB_CRASH_PARENT'}}
    if($c.was_running -ne $false -or $c.child_query_failed -ne $false -or $c.ack_attempted -ne $false -or $c.ack -ne $false -or
       $c.forced -ne $false -or $c.exit_success -ne $false -or $c.exit -ne 137 -or $c.eof -ne $true -or
       $c.drain_count -ne 2 -or @($c.drain_eof).Count -ne 2 -or @($c.drain_failed).Count -ne 2 -or
       @($c.drain_eof|Where-Object {$_ -ne $true}).Count -or @($c.drain_failed|Where-Object {$_ -ne $false}).Count -or
       $w.running_ok -ne $true -or $w.saved_ok -ne $true -or $w.stop_graceful -ne $false -or $w.stop_forced -ne $false -or
       $w.reaped -ne $true -or $w.exit -ne 137 -or $w.eof -ne $true){throw 'LAB_CRASH_PARENT'}
    foreach($kind in @('runtime-stopping','runtime-network-stopped','runtime-final')) {
        if(Test-Path -LiteralPath (Join-Path $dir "$kind-$($i.runtime_pid).json")){throw 'LAB_CRASH_NOT_ABRUPT'}
    }
    return @{state='expected_crash_reconciled';request=$i.request;digest=$binding.digest;runtime_pid=$i.runtime_pid;
        runtime_started=$i.runtime_started;runtime_instance=$i.runtime_instance;descriptor_sha256=$i.descriptor_sha256}
}
function Assert-LabCrashReconciled([string]$Run,$Cfg) {
    $entry=Lab-Read (Join-Path $Run 'attempt.json');$started=Lab-Read (Join-Path $Run 'started.json')
    $owner=Lab-Read (Join-Path $Run 'owners/2.request.json');$done=Lab-Read (Join-Path $Run 'owners/2.done.json')
    if(!$done -or $done.state -cne 'expected_crash_reconciled' -or $done.tag -cne $owner.tag -or $done.generation -cne $owner.generation){throw 'LAB_CRASH_NOT_RECONCILED'}
    $process=[pscustomobject]@{Id=$done.worker_pid;StartTime=[DateTime]::FromFileTimeUtc($done.worker_start);ExitCode=$done.exit}
    $expected=Assert-LabExpectedCrash $Run $entry $owner $Cfg $started $process
    $saved=Lab-Read (Join-Path $Run 'receipts/runtime-fault-reconciled.json')
    foreach($k in $expected.Keys){if(!$saved -or $saved.$k -cne $expected[$k]){throw 'LAB_CRASH_NOT_RECONCILED'}}
    return $expected
}
function Lab-Config([string]$Path) {
    $cfg = Lab-Read $Path
    Lab-Keys $cfg @('v','campaign','side','host','sid','base','ip','peer','interface_alias','interface_index','interface_guid','suite','max_work_seconds')
    if ($cfg.v -ne 1 -or $cfg.side -cnotin @('A','B') -or $cfg.suite -cnotin @('field','synthetic') -or
        $cfg.sid -notmatch '^S-1-5-21-(\d+-){3}\d+$' -or $cfg.max_work_seconds -lt 10 -or
        $cfg.max_work_seconds -gt 1180 -or $cfg.base -match '["\r\n]' -or $cfg.base.EndsWith('\')) { throw 'LAB_CONFIG' }
    Lab-Id $cfg.campaign
    Lab-Plain $cfg.base
    $who = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        if ([Environment]::MachineName -cne $cfg.host -or $who.User.Value -cne $cfg.sid -or
            ([Security.Principal.WindowsPrincipal]::new($who)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'LAB_TOKEN'
        }
    } finally { $who.Dispose() }
    return $cfg
}
function Lab-PinSet($Pins,[string]$Suite) {
    $names = @('host.ps1','deadline.ps1','mailbox.cs','setup-pipe.cs','kilogram-cli.exe','kilogram-bootstrap.exe')
    if ($Suite -ceq 'synthetic') { $names += 'mailbox-fake.ps1' }
    Lab-Keys $Pins $names
    foreach ($p in $Pins.PSObject.Properties) { Lab-Hex $p.Value 64 }
    return $names
}
function Stop-LabIdleAgent([string]$Base,$Handles) {
    $agentControl=Join-Path $Base 'ordinary-agent/control'
    if(!(Test-Path -LiteralPath $agentControl)){return}
    Lab-Plain $agentControl
    $state=Lab-Read (Join-Path $agentControl 'state.json')
    if($state){
        Lab-Hex $state.session
        if($state.active){throw 'LAB_STOP_ATTEMPT_FIRST'}
        $locked=$false
        try{$lock=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None');$lock.Dispose()}catch{$locked=$true}
        if($locked){
            $id=[guid]::NewGuid().ToString('N');$queue=Join-Path $agentControl $state.session
            Lab-Write (Join-Path $queue "$id.request.json") @{v=1;session=$state.session;id=$id;op='stop';args=@{}}
            $wait=[Diagnostics.Stopwatch]::StartNew();$reply=$null
            do{$reply=Lab-Read (Join-Path $queue "$id.response.json");if($reply){break};Start-Sleep -Milliseconds 80}while($wait.Elapsed.TotalSeconds -lt 6)
            if(!$reply -or !$reply.ok){throw 'LAB_AGENT_STOP_UNCONFIRMED'}
            do{
                $locked=$false
                try{$lock=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None');$lock.Dispose()}catch{$locked=$true}
                if(!$locked){break};Start-Sleep -Milliseconds 80
            }while($wait.Elapsed.TotalSeconds -lt 8)
            if($locked){throw 'LAB_AGENT_STOP_UNCONFIRMED'}
        }
    }
    # Held until the caller completes teardown, so a logon trigger cannot race a new agent/update.
    $Handles.agent=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None')
    $Handles.images=[IO.File]::Open((Join-Path $agentControl 'images.lock'),'OpenOrCreate','Write','None')
    if($state -and @([KilogramLabJob]::Inspect("Local\Kilogram-Lab-$($state.session)")).Count){throw 'LAB_AGENT_STILL_RUNNING'}
}
