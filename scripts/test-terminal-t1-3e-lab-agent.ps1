param(
    [Parameter(Mandatory)][ValidateSet('Run','Command','Worker','Wake','Probe')][string]$Action,
    [Parameter(Mandatory)][string]$Config,
    [string]$Request='{}',
    [string]$Attempt,
    [string]$BaselinePin,
    [string]$ProbeId,
    [string]$SessionId
)
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
$cfg = Lab-Config $Config
$homeDir = Join-Path $cfg.base 'ordinary-agent'
$control = Join-Path $homeDir 'control'
$runs = Join-Path $cfg.base 'runs'
foreach ($dir in @($homeDir,$control,$runs)) { [void][IO.Directory]::CreateDirectory($dir); Lab-Plain $dir }
$statePath = Join-Path $control 'state.json'
function Start-LabProcess([string]$Path,[string]$Arguments,[string]$Directory,[ref]$Created) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $info.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+$Path+'" '+$Arguments
    $info.WorkingDirectory = $Directory
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true; $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    $p = [Diagnostics.Process]::Start($info)
    $Created.Value=$p
    $p.StandardInput.Close()
    # Private child streams never enter SSH/public agent receipts.
    $null = $p.StandardOutput.BaseStream.CopyToAsync([IO.Stream]::Null)
    $null = $p.StandardError.BaseStream.CopyToAsync([IO.Stream]::Null)
    return $p
}
function Lab-OwnerClean($Entry,$Process,[string]$Run) {
    $receipt = Lab-Read (Join-Path $Run "receipts/$($Entry.tag).json")
    return $Process.ExitCode -eq 0 -and $receipt -and $receipt.tag -ceq $Entry.tag -and
        $receipt.host -ceq $cfg.host -and $receipt.owner -eq $Process.Id -and
        $receipt.child_exit -eq 0 -and $receipt.membership -eq $true -and $receipt.forced -eq $false -and
        !@($receipt.before).Count -and !@($receipt.after).Count -and !$receipt.descriptor -and !$receipt.error -and
        $receipt.mailbox_eof -eq $true -and $receipt.mailbox_quit -eq $true -and
        ($Entry.mode -cne 'setup' -or $receipt.filtered_setup_eof -eq $true)
}
function Assert-LabCrashDescendants($Watch,[string]$Run,$Owner) {
    # Only the opted-in, exited-owner/output-drained branch calls this boundary.
    # Children is the native owned Job list: never infer emptiness from HasExited.
    $begin=[KilogramS5Deadline]::TickNow()
    $cutoff=[Math]::Min(($begin+2000),[long]$Watch.WorkTick)
    $proof=[ordered]@{v=1;owner_pid=$Owner.Id;owner_started=$Owner.StartTime.ToFileTimeUtc();
        clock='GetTickCount64Milliseconds';begin_tick=$begin;cutoff_tick=$cutoff;end_tick=$null;
        first=$null;last=$null;queries=0;outcome='pending';error=$null}
    try {
        while($true) {
            $Watch.CheckWork()
            if([KilogramS5Deadline]::TickNow() -ge $cutoff){throw 'LAB_CRASH_DESCENDANTS'}
            $before=[KilogramS5Deadline]::TickNow()
            $pids=@($Watch.Children())
            $snapshot=@{before_tick=$before;after_tick=[KilogramS5Deadline]::TickNow();pids=$pids}
            if($null -eq $proof.first){$proof.first=$snapshot}
            $proof.last=$snapshot;$proof.queries++
            $Watch.CheckWork()
            if($snapshot.after_tick -ge $cutoff){throw 'LAB_CRASH_DESCENDANTS'}
            if(!$pids.Count){$proof.outcome='empty';return}
            $remaining=$cutoff-[KilogramS5Deadline]::TickNow()
            if($remaining -le 0){throw 'LAB_CRASH_DESCENDANTS'}
            Start-Sleep -Milliseconds ([int][Math]::Min(20,$remaining))
        }
    } catch {
        $proof.outcome='refused';$proof.error=@{type=$_.Exception.GetType().FullName;message=$_.Exception.Message}
        throw
    } finally {
        $proof.end_tick=[KilogramS5Deadline]::TickNow()
        Lab-Write (Join-Path $Run 'receipts/runtime-fault-descendants.json') $proof
    }
}
function Get-LabOwnerExitVerdict($Watch,$Active,$Child,[string]$Run,$Entry,[int]$CrashCount) {
    $clean=Lab-OwnerClean $Active $Child $Run
    $expected=$null
    if(!$clean -and $CrashCount -eq 0 -and $Active.PSObject.Properties['expected_runtime_crash'] -and $Active.expected_runtime_crash -eq $true) {
        Assert-LabCrashDescendants $Watch $Run $Child
        $Watch.CheckWork()
        $expected=Assert-LabExpectedCrash $Run $Entry $Active $cfg (Lab-Read (Join-Path $Run 'started.json')) $Child
        Lab-Write (Join-Path $Run 'receipts/runtime-fault-reconciled.json') $expected
    }
    return @{clean=$clean;expected=$expected}
}
function Repair-LabUpdate([string]$Stage) {
    $path=Join-Path $Stage 'update.json';$j=Lab-Read $path
    if (!$j -or $j.state -cne 'updating') { return }
    $allowed=Lab-PinSet $j.pins $cfg.suite
    $null=Lab-PinSet $j.originals $cfg.suite
    if(@($j.changed | Select-Object -Unique).Count -ne @($j.changed).Count){throw 'LAB_UPDATE_SCHEMA'}
    foreach($name in @($j.changed)){if($name -cnotin $allowed){throw 'LAB_UPDATE_SCHEMA'}}
    foreach($name in @($j.changed)) {
        $target=Join-Path $cfg.base "bin/$name"
        $actual=Lab-Hash $target
        if($actual -ceq $j.originals.$name) { continue }
        $backup=Join-Path $Stage "$name.original"
        if($actual -cne $j.pins.$name -or (Lab-Hash $backup) -cne $j.originals.$name) { throw 'LAB_UPDATE_DRIFT' }
        [IO.File]::Replace($backup,$target,(Join-Path $Stage "$name.failed"))
    }
    $j.state='rolled_back';Lab-Write $path $j
}
function Invoke-LabBoundedReadiness($SessionJob,[string]$Pin,[string]$Session) {
    Lab-Hex $Pin 64;Lab-Hex $Session
    if(@($SessionJob.Children()).Count){throw 'LAB_ACTIVE'}
    $probe=[guid]::NewGuid().ToString('N');$created=$null;$probeGuard=$null
    try {
        $p=Start-LabProcess $PSCommandPath ('-Action Probe -Config "'+$Config+'" -BaselinePin '+$Pin+' -ProbeId '+$probe+' -SessionId '+$Session) $homeDir ([ref]$created)
        $SessionJob.Verify($p);$probeGuard=$SessionJob.Bound($p,15000)
        if(!$p.WaitForExit(17000)){throw 'LAB_READINESS_TIMEOUT'}
        $answer=Lab-Read (Join-Path $control "$Session/$probe.probe.json")
        if($probeGuard.Expired -or $p.ExitCode -ne 0 -or !$answer -or $answer.id -cne $probe -or $answer.session -cne $Session -or $answer.baseline_sha256 -cne $Pin){throw 'LAB_READINESS_FAILED'}
        return $answer
    } finally {
        try{$SessionJob.CleanupChildren(2000)}catch{$state.blocked='readiness_cleanup_unconfirmed';Lab-Write $statePath $state;throw 'LAB_READINESS_CLEANUP_UNKNOWN'}
        finally{if($probeGuard){$probeGuard.Dispose()};if($created){$created.Dispose()}}
    }
}
if ($Action -ceq 'Probe') {
    Lab-Hex $ProbeId;Lab-Hex $SessionId;Lab-Hex $BaselinePin 64
    if($env:SSH_CONNECTION -or $env:SSH_TTY){throw 'LAB_NOT_INTERACTIVE'}
    $context=[IO.File]::ReadAllText((Join-Path $homeDir 'dpapi-context.ps1'))
    $begin=$context.IndexOf('using System;');$end=$context.IndexOf("'@",$begin)
    Add-Type -TypeDefinition ($context.Substring($begin,$end-$begin))
    $logon=[TerminalT1Dpapi]::ReadLogonContext()
    if($logon[0] -ne 2 -or $logon[1] -eq 0 -or !(Test-Path -LiteralPath "Registry::HKEY_USERS\$($cfg.sid)")){throw 'LAB_NOT_INTERACTIVE'}
    $plain=[Text.Encoding]::UTF8.GetBytes('Kilogram readiness synthetic DPAPI probe')
    $sealed=[TerminalT1Dpapi]::Transform($true,$plain)
    $opened=[TerminalT1Dpapi]::Transform($false,$sealed)
    if([Convert]::ToBase64String($plain) -cne [Convert]::ToBase64String($opened)){throw 'LAB_DPAPI_PROBE'}
    [Array]::Clear($opened,0,$opened.Length);[Array]::Clear($sealed,0,$sealed.Length)
    . "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
    $null=Assert-LabReadOnly $cfg $BaselinePin
    Lab-Write (Join-Path $control "$SessionId/$ProbeId.probe.json") @{id=$ProbeId;session=$SessionId;baseline_sha256=$BaselinePin;backend='com-native-v1';dpapi=$true;logon_type=2;windows_session=[int]$logon[1];sid=$cfg.sid}
    return
}
if ($Action -ceq 'Wake') {
    # Read-only task check plus ordinary Run; the task cannot accept an action override.
    $service = New-Object -ComObject Schedule.Service; $service.Connect()
    $task = $service.GetFolder('\').GetTask("Kilogram-Lab-$($cfg.campaign)-$($cfg.side)")
    . "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
    Assert-LabTask $task (Lab-TaskSpec $cfg)
    $null = $task.Run($null)
    @{state='requested';availability='requires_existing_interactive_session'} | ConvertTo-Json -Compress
    return
}
if ($Action -ceq 'Command') {
    $given = $Request | ConvertFrom-Json
    Lab-Keys $given @('v','session','id','op','args'); Lab-Hex $given.session; Lab-Hex $given.id
    if ($given.v -ne 1 -or $Request.Length -gt 16384) { throw 'LAB_REQUEST' }
    $ready = Lab-Read (Join-Path $control 'ready.json')
    if (!$ready -or $ready.session -cne $given.session) { throw 'LAB_SESSION' }
    try {
        $live = [Diagnostics.Process]::GetProcessById($ready.pid)
        try { if ($live.StartTime.ToFileTimeUtc() -ne $ready.started) { throw 'LAB_UNAVAILABLE' } }
        finally { $live.Dispose() }
    } catch { throw 'LAB_UNAVAILABLE' }
    $folder = Join-Path $control $given.session
    Lab-Plain $folder
    $dispatch = [IO.File]::Open((Join-Path $control 'dispatch.lock'),'OpenOrCreate','Write','None')
    try {
        $path = Join-Path $folder "$($given.id).request.json"
        $previous = Lab-Read $path
        if ($previous) {
            if (($previous | ConvertTo-Json -Depth 10 -Compress) -cne ($given | ConvertTo-Json -Depth 10 -Compress)) { throw 'LAB_REPLAY' }
        } else { Lab-Write $path $given }
    } finally { $dispatch.Dispose() }
    $limit = [Diagnostics.Stopwatch]::StartNew()
    do {
        $reply = Lab-Read (Join-Path $folder "$($given.id).response.json")
        if ($reply) { $reply | ConvertTo-Json -Depth 14 -Compress; return }
        Start-Sleep -Milliseconds 60
    } while ($limit.Elapsed.TotalSeconds -lt 22)
    throw 'LAB_REPLY_UNKNOWN'
}
if ($Action -ceq 'Worker') {
    Lab-Id $Attempt
    $run = Join-Path $runs $Attempt
    $entry = Lab-Read (Join-Path $run 'attempt.json')
    $locks = @(); $watch=$null; $child=$null; $primary=$null; $cleanupError=$null; $forced=$false
    $last=0; $active=$null; $cleanCount=0;$streams=@();$crashCount=0
    try {
        # Same lock covers the entire attempt and idle executable replacement.
        $locks += [IO.File]::Open((Join-Path $control 'images.lock'),'OpenOrCreate','Write','None')
        $names = Lab-PinSet $entry.pins $cfg.suite
        foreach ($name in $names) {
            $path = Join-Path $cfg.base "bin/$name"; Lab-Plain $path
            $file = [IO.File]::OpenRead($path); $locks += $file
            if ((Get-FileHash -InputStream $file).Hash -ine $entry.pins.$name) { throw 'LAB_PIN' }
        }
        . (Join-Path $cfg.base 'bin/deadline.ps1')
        $watch = [KilogramS5Deadline]::new($entry.work_seconds,($entry.work_seconds+15),[long]$entry.start_tick)
        Lab-Write (Join-Path $run 'started.json') @{pid=$PID;started=(Get-Process -Id $PID).StartTime.ToFileTimeUtc();
            generation=$entry.generation;work_tick=$watch.WorkTick;whole_tick=$watch.WholeTick;clock='GetTickCount64Milliseconds'}
        while ($true) {
            $watch.CheckWork()
            if ([IO.File]::Exists((Join-Path $run 'cancel.json'))) { throw 'LAB_CANCELLED' }
            if ($child -and $child.HasExited) {
                if(!$outTask.Wait(1000) -or !$errTask.Wait(1000)){throw 'LAB_OWNER_DRAIN'}
                foreach($s in $streams){$s.Dispose()};$streams=@()
                $verdict=Get-LabOwnerExitVerdict $watch $active $child $run $entry $crashCount
                $clean=$verdict.clean;$expected=$verdict.expected
                Lab-Write (Join-Path $run "owners/$last.done.json") @{tag=$active.tag;generation=$active.generation;
                    state=if($clean){'clean'}elseif($expected){'expected_crash_reconciled'}else{'failed'};
                    worker_pid=$child.Id;worker_start=$activeStart;exit=$child.ExitCode}
                $child.Dispose(); $child=$null
                if (!$clean -and !$expected) { throw 'LAB_OWNER_FAILED' }
                if($expected){$crashCount++}else{$cleanCount++}
            }
            if (!$child) {
                $next = Lab-Read (Join-Path $run "owners/$($last+1).request.json")
                if ($next) {
                    $last++; $active=$next
                    $mode = if ($cfg.suite -ceq 'synthetic') { 'mailbox-fake' } else { $next.mode }
                    $req = [ordered]@{base=$cfg.base;ip=$cfg.ip;hostname=$cfg.host;peer=$cfg.peer;attempt=$Attempt;
                        pins=$entry.pins;ssh_peer=$null;op=$mode;tag=$next.tag;generation=$next.generation;observe_socket=$next.mode -ceq 'chat'}
                    if ($cfg.suite -ceq 'synthetic') { $req.fake_mode=$next.fake_mode }
                    if($next.PSObject.Properties['expected_runtime_crash'] -and $next.expected_runtime_crash -eq $true){$req.runtime_crash=$true}
                    # Use EncodedCommand for JSON, never shell interpolation of a request.
                    $body = '$ErrorActionPreference="Stop";$ProgressPreference="SilentlyContinue";Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";' +
                        "& '"+(Join-Path $cfg.base 'bin/host.ps1').Replace("'","''")+"' -Request '"+
                        ($req | ConvertTo-Json -Depth 6 -Compress).Replace("'","''")+"'"
                    $info = [Diagnostics.ProcessStartInfo]::new()
                    $info.FileName="$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
                    $info.Arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
                    $info.WorkingDirectory=$run; $info.UseShellExecute=$false; $info.CreateNoWindow=$true
                    $info.RedirectStandardInput=$true; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
                    $child=[Diagnostics.Process]::Start($info); $child.StandardInput.Close(); $watch.VerifyChild($child)
                    $outSink=[IO.Stream]::Null;$errSink=[IO.Stream]::Null
                    if($cfg.suite -ceq 'synthetic') {
                        $outSink=[IO.File]::Create((Join-Path $run "owners/$last.synthetic.stdout.log"))
                        $errSink=[IO.File]::Create((Join-Path $run "owners/$last.synthetic.stderr.log"))
                        $streams=@($outSink,$errSink)
                    }
                    $outTask=$child.StandardOutput.BaseStream.CopyToAsync($outSink)
                    $errTask=$child.StandardError.BaseStream.CopyToAsync($errSink)
                    $activeStart=$child.StartTime.ToFileTimeUtc()
                    Lab-Write (Join-Path $run "owners/$last.started.json") @{state='running';tag=$next.tag;generation=$next.generation;
                        worker_pid=$child.Id;worker_start=$activeStart}
                } elseif ([IO.File]::Exists((Join-Path $run 'end.json'))) {
                    if ($cleanCount -eq 0) { throw 'LAB_EMPTY_ATTEMPT' }
                    break
                }
            }
            Start-Sleep -Milliseconds 70
        }
    } catch {
        Lab-Write (Join-Path $run 'worker-error.json') @{type=$_.Exception.GetType().FullName;message=$_.Exception.Message;stack=$_.ScriptStackTrace}
        $primary = if ($watch -and $watch.WorkExpired) { 'work_timeout' }
            elseif ($_.Exception.Message -cmatch '^LAB_[A-Z_]+$') { $_.Exception.Message } else { 'worker_failure' }
    } finally {
        $empty=$false
        try {
            if ($watch) {
                $forced=@($watch.Children()).Count -gt 0 -or $watch.WorkExpired
                if ($forced) { $watch.CleanupChildren(6000) }
                $empty=@($watch.Children()).Count -eq 0
                $watch.Finish()
            } else { $empty=$true } # No child can be launched before the watchdog.
        } catch { $cleanupError='job_cleanup' }
        if ($child) { $child.Dispose() }
        foreach($s in $streams){$s.Dispose()}
        foreach ($file in $locks) { $file.Dispose() }
        Lab-Write (Join-Path $run 'result.json') @{attempt=$Attempt;generation=$entry.generation;session=$entry.session;
            state=if(!$primary -and !$cleanupError -and !$forced){'closed'}else{'failed'};primary_error=$primary;
            cleanup=@{empty_job=$empty;error=$cleanupError;forced=$forced;proof='native_attempt_job'};owners_clean=$cleanCount;
            owners_expected_crash=$crashCount}
    }
    return
}

# Agent lifetime: ordinary interactive identity, no idle deadline, no test on logon.
if ($env:SSH_CONNECTION -or $env:SSH_TTY) { throw 'LAB_NOT_INTERACTIVE' }
$policyNames=@('Assert-LabCrashIntent','Assert-LabCrashPending','Get-LabCrashEvidence','Set-LabCrashRequest','Assert-LabExpectedCrash','Assert-LabCrashReconciled')
$policyText=(@($policyNames|ForEach-Object {(Get-Item -LiteralPath "Function:$_").Definition}) -join "`n")
$policyHasher=[Security.Cryptography.SHA256]::Create()
try{$loadedPolicyHash=([BitConverter]::ToString($policyHasher.ComputeHash([Text.Encoding]::UTF8.GetBytes($policyText)))).Replace('-','').ToLowerInvariant()}
finally{$policyHasher.Dispose()}
$contextPath=Join-Path $homeDir 'dpapi-context.ps1'
$context=[IO.File]::ReadAllText($contextPath)
$begin=$context.IndexOf('using System;'); $end=$context.IndexOf("'@",$begin)
Add-Type -TypeDefinition ($context.Substring($begin,$end-$begin))
$logon=[TerminalT1Dpapi]::ReadLogonContext()
if ($logon[0] -ne 2 -or $logon[1] -eq 0 -or !(Test-Path -LiteralPath "Registry::HKEY_USERS\$($cfg.sid)")) { throw 'LAB_NOT_INTERACTIVE' }
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
$single=[IO.File]::Open((Join-Path $control 'agent.lock'),'OpenOrCreate','Write','None')
$job=$null; $child=$null;$guard=$null
try {
    $old=Lab-Read $statePath
    if ($old) {
        Lab-Hex $old.session
        if (@([KilogramLabJob]::Inspect("Local\Kilogram-Lab-$($old.session)")).Count) { throw 'LAB_RECONCILE_LIVE_JOB' }
        if($old.blocked -ceq 'launch_ownership_unconfirmed'){throw 'LAB_RECONCILE_UNKNOWN'}
        if ($old.active) {
            Lab-Id $old.active
            $run=Join-Path $runs $old.active
            $entry=Lab-Read (Join-Path $run 'attempt.json')
            if (!$entry -or $entry.session -cne $old.session) { throw 'LAB_RECONCILE_UNKNOWN' }
            if (!(Lab-Read (Join-Path $run 'result.json'))) {
                Lab-Write (Join-Path $run 'result.json') @{attempt=$old.active;generation=$entry.generation;session=$entry.session;
                    state='failed';primary_error='agent_lost';cleanup=@{empty_job=$true;error=$null;forced=$true;proof='native_session_job_absent_or_empty'};owners_clean=0}
            }
        }
    }
    # A crash during idle replacement must be reconciled before any worker can lock new images.
    $imageLock=[IO.File]::Open((Join-Path $control 'images.lock'),'OpenOrCreate','Write','None')
    try {
        $staging=Join-Path $cfg.base 'staging'
        if(Test-Path -LiteralPath $staging) {
            Lab-Plain $staging
            foreach($stage in Get-ChildItem -LiteralPath $staging -Directory) { Lab-Hex $stage.Name;Lab-Plain $stage.FullName;Repair-LabUpdate $stage.FullName }
        }
    } finally {$imageLock.Dispose()}
    $session=[guid]::NewGuid().ToString('N')
    $folder=Join-Path $control $session; [void][IO.Directory]::CreateDirectory($folder)
    $job=[KilogramLabJob]::new("Local\Kilogram-Lab-$session")
    $state=[pscustomobject]@{session=$session;active=$null;owner_count=0;blocked=$null}
    Lab-Write $statePath $state
    Lab-Write (Join-Path $control 'ready.json') @{v=1;session=$session;host=$cfg.host;sid=$cfg.sid;pid=$PID;
        started=(Get-Process -Id $PID).StartTime.ToFileTimeUtc();logon_type=[int]$logon[0];windows_session=[int]$logon[1];profile_hive_loaded=$true;administrator=$false;
        crash_policy_version='owned-runtime-crash-v1';crash_policy_sha256=$loadedPolicyHash;
        crash_descendants_version='passive-natural-drain-v1'}
    $stop=$false
    while (!$stop) {
        if ($child -and $child.HasExited) {
            $run=Join-Path $runs $state.active
            $result=Lab-Read (Join-Path $run 'result.json')
            $drain=[Diagnostics.Stopwatch]::StartNew()
            while(@($job.Children()).Count -and $drain.ElapsedMilliseconds -lt 2000) {Start-Sleep -Milliseconds 40}
            if(!$result -and !@($job.Children()).Count) {
                $entry=Lab-Read (Join-Path $run 'attempt.json')
                Lab-Write (Join-Path $run 'result.json') @{attempt=$state.active;generation=$entry.generation;session=$session;
                    state='failed';primary_error=if($guard -and $guard.Expired){'whole_timeout'}else{'worker_lost'};
                    cleanup=@{empty_job=$true;error=$null;forced=$true;proof='native_session_job_empty'};owners_clean=0}
                $result=Lab-Read (Join-Path $run 'result.json')
            }
            if (!$result -or !$result.cleanup.empty_job -or $result.cleanup.error -or @($job.Children()).Count) {
                $state.blocked='worker_exit_without_conclusive_cleanup'
            } else { $state.active=$null; $state.owner_count=0 }
            if($guard){$guard.Dispose();$guard=$null}
            $child.Dispose(); $child=$null; Lab-Write $statePath $state
        }
        foreach ($file in @(Get-ChildItem -LiteralPath $folder -Filter '*.request.json' | Sort-Object Name)) {
            $id=$file.Name.Substring(0,32); Lab-Hex $id
            $response=Join-Path $folder "$id.response.json"
            if ([IO.File]::Exists($response)) { continue }
            $r=Lab-Read $file.FullName
            $value=$null; $errorCode=$null
            try {
                Lab-Keys $r @('v','session','id','op','args')
                if ($r.v -ne 1 -or $r.session -cne $session -or $r.id -cne $id) { throw 'LAB_SESSION' }
                if ($state.blocked -and $r.op -cne 'status') { throw 'LAB_BLOCKED' }
                switch -CaseSensitive ($r.op) {
                    'check' {
                        Lab-Keys $r.args @('baseline_sha256')
                        if($cfg.suite -cne 'field' -or $state.active){throw 'LAB_ACTIVE'}
                        $value=Invoke-LabBoundedReadiness $job $r.args.baseline_sha256 $session
                    }
                    'status' {
                        Lab-Keys $r.args @('attempt'); Lab-Id $r.args.attempt
                        $run=Join-Path $runs $r.args.attempt
                        $value=@{active=$state.active;blocked=$state.blocked;started=(Lab-Read (Join-Path $run 'started.json'));
                            result=(Lab-Read (Join-Path $run 'result.json'));owners=@()}
                        if ([IO.Directory]::Exists((Join-Path $run 'owners'))) {
                            $value.owners=@(Get-ChildItem -LiteralPath (Join-Path $run 'owners') -Filter '*.request.json' | Sort-Object Name | ForEach-Object {
                                $number=$_.Name.Split('.')[0]; $done=Lab-Read (Join-Path $_.DirectoryName "$number.done.json")
                                if ($done) { $done } else { Lab-Read (Join-Path $_.DirectoryName "$number.started.json") }
                            })
                        }
                    }
                    'begin' {
                        Lab-Keys $r.args @('attempt','generation','work_seconds','pins','baseline_sha256'); Lab-Id $r.args.attempt; Lab-Hex $r.args.generation
                        if ($state.active -or @($job.Children()).Count) { throw 'LAB_ACTIVE' }
                        if ($r.args.work_seconds -isnot [int] -or $r.args.work_seconds -lt 2 -or $r.args.work_seconds -gt $cfg.max_work_seconds) { throw 'LAB_BUDGET' }
                        $null=Lab-PinSet $r.args.pins $cfg.suite
                        if($cfg.suite -ceq 'field') {
                            $null=Invoke-LabBoundedReadiness $job $r.args.baseline_sha256 $session
                        } elseif($r.args.baseline_sha256 -cne ''){throw 'LAB_BASELINE_SCOPE'}
                        $run=Join-Path $runs $r.args.attempt
                        if (Test-Path -LiteralPath $run) { throw 'LAB_DUPLICATE_ATTEMPT' }
                        [void][IO.Directory]::CreateDirectory($run)
                        foreach ($sub in @('receipts','owners','in')) { [void][IO.Directory]::CreateDirectory((Join-Path $run $sub)) }
                        Lab-Write (Join-Path $run 'attempt.json') @{attempt=$r.args.attempt;generation=$r.args.generation;session=$session;
                            work_seconds=$r.args.work_seconds;pins=$r.args.pins;start_tick=[KilogramLabJob]::TickNow()}
                        $created=$null
                        try {
                            $state.active=$r.args.attempt; $state.owner_count=0; Lab-Write $statePath $state
                            $child=Start-LabProcess $PSCommandPath ('-Action Worker -Config "'+$Config+'" -Attempt '+$state.active) $run ([ref]$created)
                            $job.Verify($child)
                            $guard=$job.Bound($child,($r.args.work_seconds+15)*1000)
                            $value=@{attempt=$state.active;generation=$r.args.generation;worker_pid=$child.Id;worker_start=$child.StartTime.ToFileTimeUtc()}
                        } catch {
                            $launchError=$_.Exception.Message;$cleanupError=$null;$forced=$false;$empty=$false
                            try {
                                if($guard){$guard.Dispose();$guard=$null}
                                if($created -and !$created.HasExited){$job.Verify($created)}
                                $forced=@($job.Children()).Count -gt 0
                                $job.CleanupChildren(6000)
                                $empty=@($job.Children()).Count -eq 0
                            }catch{$cleanupError='launch_ownership_unconfirmed'}
                            Lab-Write (Join-Path $run 'launch-error.json') @{message=$launchError;process_created=($null -ne $created);
                                worker_pid=if($created){$created.Id}else{$null}}
                            # A worker may have produced its own receipt before a later admission exception.
                            # Preserve that receipt; admission failure remains the authoritative terminal result.
                            $existing=Lab-Read (Join-Path $run 'result.json')
                            if($existing){Lab-Write (Join-Path $run 'worker-result-before-launch-failure.json') $existing}
                            Lab-Write (Join-Path $run 'result.json') @{attempt=$r.args.attempt;generation=$r.args.generation;session=$session;
                                state='failed';primary_error='worker_launch_failed';cleanup=@{empty_job=$empty;error=$cleanupError;
                                forced=$forced;proof='native_session_job_after_launch_exception'};owners_clean=0}
                            if($empty -and !$cleanupError){$state.active=$null;$state.owner_count=0}else{$state.blocked='launch_ownership_unconfirmed'}
                            if($created){$created.Dispose()};$child=$null
                            Lab-Write $statePath $state
                            throw 'LAB_LAUNCH_FAILED'
                        }
                    }
                    { $_ -cin @('owner','end','cancel') } {
                        $extras=if($r.op -ceq 'owner'){@('tag','owner_generation','mode','fake_mode')}else{@()}
                        $faultOpt=$r.op -ceq 'owner' -and $null -ne $r.args.PSObject.Properties['expected_runtime_crash']
                        Lab-Keys $r.args (@('attempt','generation')+$extras+$(if($faultOpt){@('expected_runtime_crash')}else{@()}))
                        Lab-Id $r.args.attempt; Lab-Hex $r.args.generation
                        if ($state.active -cne $r.args.attempt) { throw 'LAB_INACTIVE' }
                        $run=Join-Path $runs $state.active; $entry=Lab-Read (Join-Path $run 'attempt.json')
                        if ($entry.generation -cne $r.args.generation) { throw 'LAB_GENERATION' }
                        if ($r.op -ceq 'owner') {
                            Lab-Id $r.args.tag; Lab-Hex $r.args.owner_generation
                            if($faultOpt -and ($r.args.expected_runtime_crash -isnot [bool] -or $r.args.expected_runtime_crash -ne $true -or $cfg.side -cne 'A' -or $cfg.suite -cne 'field' -or
                                $r.args.tag -cne 'owner-a-1' -or $r.args.mode -cne 'chat' -or $state.owner_count -ne 1)){throw 'LAB_CRASH_SCOPE'}
                            if ($r.args.mode -cnotin @('setup','chat') -or
                                ($cfg.suite -ceq 'field' -and $r.args.fake_mode -cne '') -or
                                ($cfg.suite -ceq 'synthetic' -and $r.args.fake_mode -cnotin @('echo','disconnect','valid'))) { throw 'LAB_OWNER' }
                            $existing=@(Get-ChildItem -LiteralPath (Join-Path $run 'owners') -Filter '*.request.json')
                            foreach ($o in $existing) {
                                $prior=Lab-Read $o.FullName
                                if ($prior.tag -ceq $r.args.tag -or $prior.generation -ceq $r.args.owner_generation) { throw 'LAB_DUPLICATE_OWNER' }
                                $done=Lab-Read ($o.FullName.Replace('.request.json','.done.json'))
                                if(!$done){throw 'LAB_OWNER_ACTIVE'}
                                if($done.state -cne 'clean') {
                                    if($done.state -cne 'expected_crash_reconciled'){throw 'LAB_OWNER_ACTIVE'}
                                    $null=Assert-LabCrashReconciled $run $cfg
                                }
                            }
                            if ((Test-Path -LiteralPath (Join-Path $run 'end.json')) -or (Test-Path -LiteralPath (Join-Path $run 'cancel.json'))) { throw 'LAB_ENDING' }
                            $state.owner_count++
                            $ownerRequest=@{tag=$r.args.tag;generation=$r.args.owner_generation;mode=$r.args.mode;fake_mode=$r.args.fake_mode}
                            if($faultOpt){$ownerRequest.expected_runtime_crash=$true}
                            Lab-Write (Join-Path $run "owners/$($state.owner_count).request.json") $ownerRequest
                            Lab-Write $statePath $state
                            $value=@{state='queued';tag=$r.args.tag;generation=$r.args.owner_generation}
                        } else {
                            Lab-Write (Join-Path $run "$($r.op).json") @{id=$id;generation=$entry.generation}
                            $value=@{state='requested'}
                        }
                    }
                    'update' {
                        Lab-Keys $r.args @('stage','pins'); Lab-Hex $r.args.stage
                        if ($state.active -or @($job.Children()).Count) { throw 'LAB_ACTIVE' }
                        $names=Lab-PinSet $r.args.pins $cfg.suite
                        $imageLock=[IO.File]::Open((Join-Path $control 'images.lock'),'OpenOrCreate','Write','None')
                        try {
                            $stage=Join-Path $cfg.base "staging/$($r.args.stage)"; Lab-Plain $stage
                            foreach ($name in $names) {
                                if ((Lab-Hash (Join-Path $stage $name)) -cne $r.args.pins.$name) { throw 'LAB_PIN' }
                                foreach($suffix in @('next','original','failed')){if(Test-Path -LiteralPath (Join-Path $stage "$name.$suffix")){throw 'LAB_STAGE_COLLISION'}}
                            }
                            # Updates require exact identical bytes or a complete staged set. Preserve backups and journal on failure.
                            $journal=Join-Path $stage 'update.json'
                            if (Test-Path -LiteralPath $journal) { throw 'LAB_UPDATE_REUSED' }
                            $originals=[ordered]@{};foreach($name in $names){$originals[$name]=Lab-Hash (Join-Path $cfg.base "bin/$name")}
                            $record=@{state='updating';names=$names;changed=@();originals=$originals;pins=$r.args.pins}; Lab-Write $journal $record
                            try {
                                foreach ($name in $names) {
                                    $target=Join-Path $cfg.base "bin/$name"; Lab-Plain $target
                                    if ((Lab-Hash $target) -ceq $r.args.pins.$name) { continue }
                                    $next=Join-Path $stage "$name.next"
                                    [IO.File]::Copy((Join-Path $stage $name),$next,$false)
                                    $record.changed+=@($name); Lab-Write $journal $record
                                    [IO.File]::Replace($next,$target,(Join-Path $stage "$name.original"))
                                }
                                $record.state='complete'; Lab-Write $journal $record
                            } catch {
                                try{Repair-LabUpdate $stage}catch{$state.blocked='update_interrupted';Lab-Write $statePath $state}
                                throw 'LAB_UPDATE_FAILED'
                            }
                            $value=@{state='updated';pins=$r.args.pins}
                        } finally { $imageLock.Dispose() }
                    }
                    'stop' {
                        Lab-Keys $r.args @()
                        if ($state.active -or @($job.Children()).Count) { throw 'LAB_ACTIVE' }
                        $stop=$true; $value=@{state='stopped'}
                    }
                    default { throw 'LAB_OPERATION' }
                }
            } catch {
                Lab-Write (Join-Path $folder "$id.error.json") @{type=$_.Exception.GetType().FullName;message=$_.Exception.Message;stack=$_.ScriptStackTrace}
                $errorCode=if($_.Exception.Message -cmatch '^LAB_[A-Z_]+$'){$_.Exception.Message}else{'LAB_INTERNAL'}
            }
            Lab-Write $response @{v=1;session=$session;id=$id;ok=(!$errorCode);error=$errorCode;value=$value}
            if($stop){break}
        }
        Start-Sleep -Milliseconds 70
    }
    $job.Dispose(); $job=$null
} finally {
    $single.Dispose()
    # On unexpected agent failure, process exit closes the Job and kills all owned descendants.
    # Do not disarm a nonempty Job here.
}
