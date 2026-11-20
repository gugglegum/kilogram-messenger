param([Parameter(Mandatory)][string]$Evidence,[switch]$FixtureLibrary)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
if(!$FixtureLibrary){
    if(Test-Path -LiteralPath $Evidence){throw 'Fresh policy evidence required'}
    [void][IO.Directory]::CreateDirectory($Evidence)
}
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
$guard=if(!$FixtureLibrary){[KilogramS5Deadline]::new(100,115)}else{$null};$owned=@();$cases=@();$passed=$false
function Write-Case($Path,$Value){[void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Path));Lab-Write $Path $Value}
function Start-QuietFixture([string]$Exe,[string]$Arguments) {
    $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$Exe;$info.Arguments=$Arguments
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    return [Diagnostics.Process]::Start($info)
}
function Fixture([string]$Name) {
    $base=Join-Path ([IO.Path]::GetFullPath($Evidence)) $Name;$run=Join-Path $base 'runs/one'
    foreach($d in @('receipts','owners','in','setup/connection','setup/lab-diagnostics-v1')){[void][IO.Directory]::CreateDirectory((Join-Path $run $d))}
    $cfg=[pscustomobject]@{side='A';host=[Environment]::MachineName;base=$base}
    $owner=[pscustomobject]@{mode='chat';tag='owner-a-1';generation=('b'*32);expected_runtime_crash=$true}
    $entry=[pscustomobject]@{session=('a'*32);attempt='one';generation=('c'*32);start_tick=[long]50;pins=@{'kilogram-cli.exe'=('d'*64)}}
    $started=[pscustomobject]@{pid=42;started=[long]134396000000000000;generation=$entry.generation;work_tick=[long]::MaxValue}
    $profile='profile-'+('a'*64)+'.json';Write-Case (Join-Path $run "setup/connection/$profile") @{fixture='profile'}
    Write-Case (Join-Path $run 'setup/connection/ipc.json') @{fixture='stale descriptor; no real IPC token'}
    $contexts=@(('1'*64),('2'*64));$expected=@();$requests=@()
    for($slot=0;$slot -lt 2;$slot++) {
        $key=@{context=$contexts[$slot];request_id=([string]($slot+3)*32)};$event=([string]($slot+5)*64)
        $history=@(@{event_id=$event;text="synthetic-$slot";role=0;ordinal=1})
        $expected+=@{slot=$slot;key=$key;event_id=$event;text="synthetic-$slot";history=$history}
        $sample=@{request=@{state='materialized';event_id=$event};sender_history=$history}
        $requests+=@{slot=$slot;key=$key;first=$sample;second=$sample}
    }
    $proof=@{recipient_stopped=$true;sender_instance=('e'*64);expected=$expected;requests=$requests}
    Write-Case (Join-Path $run 'in/crash-pending.json') $proof
    $intent=[pscustomobject]@{v=1;request=('f'*32);session=$entry.session;attempt='one';attempt_generation=$entry.generation;
        owner_tag=$owner.tag;owner_generation=$owner.generation;worker_pid=$started.pid;worker_started=$started.started;
        runtime_pid=1234;runtime_started=[long]567;runtime_image=(Join-Path $base 'bin/kilogram-cli.exe');runtime_sha256=('d'*64);
        runtime_instance=('e'*64);descriptor_sha256=(Lab-Hash (Join-Path $run 'setup/connection/ipc.json'));
        proof_sha256=(Lab-Hash (Join-Path $run 'in/crash-pending.json'));contexts=$contexts;profile_name=$profile;
        profile_sha256=(Lab-Hash (Join-Path $run "setup/connection/$profile"))}
    $digest=Set-LabCrashRequest $run $intent
    Write-Case (Join-Path $run 'attempt.json') $entry;Write-Case (Join-Path $run 'started.json') $started
    Write-Case (Join-Path $run 'owners/2.request.json') $owner
    Write-Case (Join-Path $run 'receipts/runtime-fault-claim.json') @{digest=$digest;owner_pid=$hostChild.Id;tick=[long]100}
    Write-Case (Join-Path $run 'receipts/runtime-fault-result.json') @{digest=$digest;request=$intent.request;runtime_pid=$intent.runtime_pid;
        runtime_started=$intent.runtime_started;runtime_image=$intent.runtime_image;runtime_instance=$intent.runtime_instance;
        owner_pid=$hostChild.Id;owner_started=$hostStart;chat_pid=2345;exit=137;observed_tick=[long]101}
    $receipt=@{tag=$owner.tag;host=$cfg.host;owner=$hostChild.Id;child=2345;child_exit=3;membership=$true;forced=$false;
        before=@();after=@();work_expired=$false;mailbox_eof=$true;mailbox_quit=$false;descriptor=$true;error='Mailbox launch, stop or exchange failed'}
    Write-Case (Join-Path $run 'receipts/owner-a-1.json') $receipt
    $connection=@{was_running=$false;child_query_failed=$false;ack_attempted=$false;ack=$false;forced=$false;exit_success=$false;
        exit=137;eof=$true;drain_count=2;drain_eof=@($true,$true);drain_failed=@($false,$false)}
    $worker=@{running_ok=$true;saved_ok=$true;stop_graceful=$false;stop_forced=$false;reaped=$true;exit=137;eof=$true}
    foreach($kind in @('connection_final','worker_final')) {
        Write-Case (Join-Path $run ('setup/lab-diagnostics-v1/'+$kind.Replace('_','-')+'-2345.json')) @{
            schema=1;kind=$kind;run_nonce=$entry.generation;pid=2345;runtime_pid=$intent.runtime_pid;runtime_instance=$intent.runtime_instance;
            data=$(if($kind -ceq 'connection_final'){$connection}else{$worker})}
    }
    return @{run=$run;cfg=$cfg;entry=$entry;owner=$owner;started=$started;intent=$intent;receipt=$receipt;connection=$connection;worker=$worker}
}
if($FixtureLibrary){return}
try {
    # Real disposable host-process exit1; parent diagnostic payloads below are synthetic policy fixtures, NOT product behavior.
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName="$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $si.Arguments='-NoProfile -NonInteractive -Command "Start-Sleep -Milliseconds 500;exit 1"';$si.UseShellExecute=$false;$si.CreateNoWindow=$true
    $hostChild=[Diagnostics.Process]::Start($si);$owned+=,$hostChild;$hostStart=$hostChild.StartTime.ToFileTimeUtc()
    if(!$hostChild.WaitForExit(5000) -or $hostChild.ExitCode -ne 1){throw 'Host exit fixture'}
    $f=Fixture 'valid';$v=Assert-LabExpectedCrash $f.run $f.entry $f.owner $f.cfg $f.started $hostChild
    if($v.state -cne 'expected_crash_reconciled'){throw 'Positive policy fixture failed'}
    $cases+=@{name='valid_expected_crash';passed=$true;raw_child_exit=3;raw_host_exit=1}
    Write-Case (Join-Path $f.run 'owners/2.done.json') @{state='expected_crash_reconciled';tag=$f.owner.tag;generation=$f.owner.generation;
        worker_pid=$hostChild.Id;worker_start=$hostStart;exit=1}
    Write-Case (Join-Path $f.run 'receipts/runtime-fault-reconciled.json') $v
    $null=Assert-LabCrashReconciled $f.run $f.cfg
    $digest=Set-LabCrashRequest $f.run $f.intent
    if((Set-LabCrashRequest $f.run $f.intent) -cne $digest){throw 'Idempotent publication'}
    $replay=$f.intent|ConvertTo-Json -Depth 12|ConvertFrom-Json;$replay.request='0'*32
    $refused=$false;try{$null=Set-LabCrashRequest $f.run $replay}catch{$refused=$true}
    if(!$refused){throw 'Changed fault request was accepted'}
    $cases+=@{name='lost_reply_and_changed_request';passed=$true;digest=$digest}
    $faults=@('no_intent','wrong_session','wrong_attempt','wrong_generation','wrong_owner','wrong_owner_generation','wrong_worker','wrong_start',
        'wrong_image','wrong_hash','wrong_context','wrong_instance','proof_pin','profile_changed','descriptor_changed','missing_claim','claim_digest',
        'no_result','wrong_runtime','wrong_runtime_start','watchdog124','unknown_outcome','wrong_host','wrong_host_pid','wrong_chat_exit','forced',
        'leftover_before','leftover_after','work_expired','missing_eof','quit_success','different_error','no_descriptor',
        'no_parent','parent_nonce','parent_pid','parent_instance','runtime_was_running','shutdown_ack','forced_parent','drain_failed','missing_drain',
        'worker_not_saved','worker_not_reaped','extra_runtime_final','unapproved_owner','wrong_side','wrong_mode','claim_tick','string_exit','forged_boolean','profile_name_length')
    foreach($fault in $faults) {
        $f=Fixture $fault;$run=$f.run;$path=$null;$value=$null
        if($fault -eq 'no_intent'){Remove-Item -LiteralPath (Join-Path $run 'receipts/runtime-fault-request.json')}
        elseif($fault -in @('wrong_session','wrong_attempt','wrong_generation','wrong_owner','wrong_owner_generation','wrong_worker','wrong_start','wrong_image','wrong_hash','wrong_context','wrong_instance','proof_pin')) {
            $value=Lab-Read (Join-Path $run 'receipts/runtime-fault-request.json');$path=Join-Path $run 'receipts/runtime-fault-request.json'
            switch($fault){
                'wrong_session'{$value.session='0'*32};'wrong_attempt'{$value.attempt='other'};'wrong_generation'{$value.attempt_generation='0'*32}
                'wrong_owner'{$value.owner_tag='owner-b-1'};'wrong_owner_generation'{$value.owner_generation='0'*32};'wrong_worker'{$value.worker_pid++}
                'wrong_start'{$value.worker_started++};'wrong_image'{$value.runtime_image='wrong.exe'};'wrong_hash'{$value.runtime_sha256='0'*64}
                'wrong_context'{$value.contexts[0]='0'*64};'wrong_instance'{$value.runtime_instance='0'*64};'proof_pin'{$value.proof_sha256='0'*64}
            }
        }elseif($fault -in @('profile_changed','descriptor_changed')) {
            $path=Join-Path $run ('setup/connection/'+$(if($fault -eq 'profile_changed'){$f.intent.profile_name}else{'ipc.json'}));$value=@{changed=$true}
        }elseif($fault -in @('missing_claim','no_result','no_parent')) {
            $path=Join-Path $run $(switch($fault){'missing_claim'{'receipts/runtime-fault-claim.json'};'no_result'{'receipts/runtime-fault-result.json'};'no_parent'{'setup/lab-diagnostics-v1/connection-final-2345.json'}})
            Remove-Item -LiteralPath $path;$path=$null
        }elseif($fault -eq 'claim_digest'){$path=Join-Path $run 'receipts/runtime-fault-claim.json';$value=Lab-Read $path;$value.digest='0'*64}
        elseif($fault -in @('wrong_runtime','wrong_runtime_start','watchdog124','wrong_chat_exit','wrong_host_pid')) {
            if($fault -eq 'wrong_chat_exit'){$path=Join-Path $run 'receipts/owner-a-1.json';$value=Lab-Read $path;$value.child_exit=0}
            else{$path=Join-Path $run 'receipts/runtime-fault-result.json';$value=Lab-Read $path;switch($fault){'wrong_runtime'{$value.runtime_pid++};'wrong_runtime_start'{$value.runtime_started++};'watchdog124'{$value.exit=124};'wrong_host_pid'{$value.owner_pid++}}}
        }elseif($fault -eq 'unknown_outcome'){$path=Join-Path $run 'receipts/runtime-fault-error.json';$value=@{outcome_unknown=$true}}
        elseif($fault -in @('wrong_host','forced','leftover_before','leftover_after','work_expired','missing_eof','quit_success','different_error','no_descriptor')) {
            $path=Join-Path $run 'receipts/owner-a-1.json';$value=Lab-Read $path
            switch($fault){'wrong_host'{$value.host='foreign'};'forced'{$value.forced=$true};'leftover_before'{$value.before=@(1)};'leftover_after'{$value.after=@(1)};
                'work_expired'{$value.work_expired=$true};'missing_eof'{$value.mailbox_eof=$false};'quit_success'{$value.mailbox_quit=$true};'different_error'{$value.error='unknown'};'no_descriptor'{$value.descriptor=$false}}
        }elseif($fault -in @('parent_nonce','parent_pid','parent_instance','runtime_was_running','shutdown_ack','forced_parent','drain_failed','missing_drain')) {
            $path=Join-Path $run 'setup/lab-diagnostics-v1/connection-final-2345.json';$value=Lab-Read $path
            switch($fault){'parent_nonce'{$value.run_nonce='0'*32};'parent_pid'{$value.pid++};'parent_instance'{$value.runtime_instance='0'*64};
                'runtime_was_running'{$value.data.was_running=$true};'shutdown_ack'{$value.data.ack=$true};'forced_parent'{$value.data.forced=$true};
                'drain_failed'{$value.data.drain_failed[0]=$true};'missing_drain'{$value.data.drain_eof=@($true)}}
        }elseif($fault -in @('worker_not_saved','worker_not_reaped')) {
            $path=Join-Path $run 'setup/lab-diagnostics-v1/worker-final-2345.json';$value=Lab-Read $path
            if($fault -eq 'worker_not_saved'){$value.data.saved_ok=$false}else{$value.data.reaped=$false}
        }elseif($fault -eq 'extra_runtime_final'){$path=Join-Path $run 'setup/lab-diagnostics-v1/runtime-final-1234.json';$value=@{unexpected=$true}}
        elseif($fault -eq 'unapproved_owner'){$f.owner.expected_runtime_crash=$false}
        elseif($fault -eq 'wrong_side'){$f.cfg.side='B'}
        elseif($fault -eq 'wrong_mode'){$f.owner.mode='setup'}
        elseif($fault -eq 'claim_tick'){$path=Join-Path $run 'receipts/runtime-fault-claim.json';$value=Lab-Read $path;$value.tick=0}
        elseif($fault -eq 'string_exit'){$path=Join-Path $run 'receipts/runtime-fault-result.json';$value=Lab-Read $path;$value.exit='137'}
        elseif($fault -eq 'forged_boolean'){$path=Join-Path $run 'receipts/owner-a-1.json';$value=Lab-Read $path;$value.mailbox_eof=1}
        elseif($fault -eq 'profile_name_length'){$f.intent.profile_name='profile-'+('a'*32)+'.json';$path=Join-Path $run 'receipts/runtime-fault-request.json';$value=$f.intent}
        else{throw "Missing case $fault"}
        if($path){Write-Case $path $value}
        $rejected=$false;$reason=$null
        try{$null=Assert-LabExpectedCrash $run $f.entry $f.owner $f.cfg $f.started $hostChild}catch{$rejected=$true;$reason=$_.Exception.Message}
        if(!$rejected){throw "Accepted invalid policy: $fault"}
        $cases+=@{name=$fault;rejected=$true;reason=$reason}
    }
    # Execute the actual preconnect observer against disposable sleeping EXEs, no sockets/network or real profile.
    $f=Fixture 'observer';$v=Assert-LabExpectedCrash $f.run $f.entry $f.owner $f.cfg $f.started $hostChild
    Write-Case (Join-Path $f.run 'owners/2.done.json') @{state='expected_crash_reconciled';tag=$f.owner.tag;generation=$f.owner.generation;worker_pid=$hostChild.Id;worker_start=$hostStart;exit=1}
    Write-Case (Join-Path $f.run 'receipts/runtime-fault-reconciled.json') $v
    Write-Case (Join-Path $f.cfg.base 'ordinary-agent/lab.json') $f.cfg
    [IO.File]::Copy((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-common.ps1'),(Join-Path $f.cfg.base 'ordinary-agent/test-terminal-t1-3e-lab-common.ps1'),$false)
    Write-Case (Join-Path $f.cfg.base 'ordinary-agent/control/ready.json') @{session=$f.entry.session}
    Write-Case (Join-Path $f.cfg.base 'ordinary-agent/control/state.json') @{session=$f.entry.session;active='one';blocked=$null}
    $bin=Join-Path $f.cfg.base 'bin';[void][IO.Directory]::CreateDirectory($bin);$exe=Join-Path $bin 'kilogram-cli.exe'
    Add-Type -TypeDefinition 'public static class CrashFixture { public static void Main(string[] args){System.Threading.Thread.Sleep(90000);} }' -OutputAssembly $exe -OutputType ConsoleApplication
    $source=[IO.File]::ReadAllText((Join-Path $PSScriptRoot 'test-terminal-t1-3e-observe.ps1'))
    $source=$source.Replace('E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f',$f.cfg.base).Replace("`$ip = '192.168.0.140'","`$ip = '127.0.0.1'")
    $observer=Join-Path $f.cfg.base 'observer.ps1';[IO.File]::WriteAllText($observer,$source,[Text.UTF8Encoding]::new($false))
    $chat=Start-QuietFixture $exe ('chat "'+(Join-Path $f.run 'setup')+'"');$owned+=,$chat;$guard.VerifyChild($chat)
    $request=@{side='A';host=[Environment]::MachineName;attempt='one';op='preconnect';tag='owner-b-1';crash_restart=$true;crash_contexts=$f.intent.contexts}
    $observed=(& $observer -Request ($request|ConvertTo-Json -Compress))|ConvertFrom-Json
    if(!$observed.disconnected -or !$observed.descriptor -or $observed.chat_pid -ne $chat.Id -or $observed.crash_descriptor_sha256 -cne $f.intent.descriptor_sha256){throw 'Actual crash preconnect failed'}
    $cases+=@{name='actual_stale_preconnect';passed=$true;chat_pid=$chat.Id}
    $hostScript=Join-Path $PSScriptRoot 'test-terminal-t1-1f-host.ps1'
    $wire=@{base=$f.cfg.base;attempt='one';hostname=$f.cfg.host;ip='127.0.0.1';pins=$f.entry.pins;
        agent_session=$f.entry.session;agent_generation=$f.entry.generation;worker_pid=$f.started.pid;worker_started=$f.started.started;
        op='lab-runtime-crash';fault=$f.intent}
    $first=(& $hostScript -Request ($wire|ConvertTo-Json -Depth 12 -Compress))|ConvertFrom-Json
    $again=(& $hostScript -Request ($wire|ConvertTo-Json -Depth 12 -Compress))|ConvertFrom-Json
    if($first.digest -cne $again.digest -or $again.runtime_pid -ne $f.intent.runtime_pid -or $chat.HasExited){throw 'Actual host lost-reply replay'}
    $wire.fault=$f.intent|ConvertTo-Json -Depth 12|ConvertFrom-Json;$wire.fault.request='0'*32
    $rejected=$false;try{$null=& $hostScript -Request ($wire|ConvertTo-Json -Depth 12 -Compress)}catch{$rejected=$true}
    if(!$rejected -or $chat.HasExited){throw 'Actual host accepted changed replay'}
    $cases+=@{name='actual_host_replay_with_live_replacement';passed=$true;replacement_pid=$chat.Id}
    foreach($fault in @('default_stale','contexts','wrong_tag','descriptor','profile','unproven','live_runtime')) {
        $q=$request|ConvertTo-Json -Depth 6|ConvertFrom-Json;$path=$null;$old=$null;$runtime=$null
        if($fault -eq 'default_stale'){$q.crash_restart=$false}
        elseif($fault -eq 'contexts'){$q.crash_contexts[0]='0'*64}
        elseif($fault -eq 'wrong_tag'){$q.tag='owner-a-1'}
        elseif($fault -in @('descriptor','profile','unproven')) {
            $path=Join-Path $f.run $(switch($fault){'descriptor'{'setup/connection/ipc.json'};'profile'{('setup/connection/'+$f.intent.profile_name)};'unproven'{'receipts/runtime-fault-reconciled.json'}})
            $old=[IO.File]::ReadAllBytes($path);Write-Case $path @{changed=$true}
        }elseif($fault -eq 'live_runtime') {
            $runtime=Start-QuietFixture $exe ('runtime-from-profile "'+(Join-Path $f.run 'setup')+'"');$owned+=,$runtime;$guard.VerifyChild($runtime)
        }
        $rejected=$false
        try{$null=& $observer -Request ($q|ConvertTo-Json -Depth 6 -Compress)}catch{$rejected=$true}
        finally{if($path){[IO.File]::WriteAllBytes($path,$old)};if($runtime){$runtime.Kill();$null=$runtime.WaitForExit(2000)}}
        if(!$rejected){throw "Actual preconnect accepted $fault"}
        $cases+=@{name=('preconnect_'+$fault);rejected=$true}
    }
    # New main + old common must fail before publishing ready, even if a logon trigger races refresh.
    $mixed=Join-Path ([IO.Path]::GetFullPath($Evidence)) 'mixed-agent';$agentHome=Join-Path $mixed 'ordinary-agent'
    [void][IO.Directory]::CreateDirectory($agentHome)
    [IO.File]::Copy((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-agent.ps1'),(Join-Path $agentHome 'test-terminal-t1-3e-lab-agent.ps1'),$false)
    $oldCommon=& git -C (Split-Path $PSScriptRoot -Parent) show 'b52bda60412610f86e77936ceb618d682d105b81:scripts/test-terminal-t1-3e-lab-common.ps1'
    if($LASTEXITCODE -ne 0){throw 'Baseline common fixture unavailable'}
    [IO.File]::WriteAllText((Join-Path $agentHome 'test-terminal-t1-3e-lab-common.ps1'),($oldCommon -join "`n"),[Text.UTF8Encoding]::new($false))
    $config=@{v=1;campaign='mixed';side='A';host=[Environment]::MachineName;sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;
        base=$mixed;ip='127.0.0.1';peer='127.0.0.2';interface_alias='synthetic';interface_index=1;interface_guid=[guid]::NewGuid().ToString();suite='synthetic';max_work_seconds=60}
    Write-Case (Join-Path $agentHome 'lab.json') $config
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName="$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $si.Arguments='-NoProfile -NonInteractive -File "'+(Join-Path $agentHome 'test-terminal-t1-3e-lab-agent.ps1')+'" -Action Run -Config "'+(Join-Path $agentHome 'lab.json')+'"'
    $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
    $child=[Diagnostics.Process]::Start($si);$owned+=,$child;$guard.VerifyChild($child)
    $o=$child.StandardOutput.ReadToEndAsync();$e=$child.StandardError.ReadToEndAsync()
    if(!$child.WaitForExit(10000) -or $child.ExitCode -eq 0 -or (Test-Path -LiteralPath (Join-Path $agentHome 'control/ready.json'))){throw 'Mixed policy became ready'}
    [IO.File]::WriteAllText((Join-Path $mixed 'stdout.log'),$o.GetAwaiter().GetResult());[IO.File]::WriteAllText((Join-Path $mixed 'stderr.log'),$e.GetAwaiter().GetResult())
    $cases+=@{name='mixed_loaded_policy_refused';rejected=$true;exit=$child.ExitCode;ready_published=$false}
    $passed=$true
}finally{
    $guard.CleanupChildren(5000);$empty=@($guard.Children()).Count -eq 0;$guard.Finish()
    foreach($child in $owned){$child.Dispose()}
    @{passed=$passed;cases=$cases;empty_owned_job=$empty;parent_diagnostics_synthetic=$true;product_behavior_proved=$false}|ConvertTo-Json -Depth 8|
        Set-Content -LiteralPath (Join-Path $Evidence 'result.json') -Encoding UTF8
}
if(!$passed){throw 'Policy checks failed'}
@{passed=$true;cases=$cases.Count;empty_owned_job=$empty;product_behavior_proved=$false}|ConvertTo-Json -Compress
