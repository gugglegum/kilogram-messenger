[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256)
# Dormant public-only control validation. One future root allocation, no private production entrypoint.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-production-control-01-attempt-01'
$e="$w\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence"
$allocation="$e\$attempt.root-allocation.json"
$preflight="$e\$attempt.preflight.ps1";$watchdog="$e\$attempt.watchdog.ps1"
$production="$w\scripts\invoke-migration-s53-cold-preservation.ps1";$output="$w\scripts\migration-s53-cold-output.ps1"
$deadline="$w\scripts\migration-s5-l1-deadline.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1"
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$held=[Collections.Generic.List[IO.FileStream]]::new();$guard=$null;$cleanupReserve=$false
$cleanupLimit=28
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Stop-C2Control([string]$Code) {
    $err=[InvalidOperationException]::new('C2Control bounded refusal');$err.Data['c2control_code']=$Code;throw $err
}
function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2Control $Code}}
function Check {
    $age=Age;$limit=if($cleanupReserve){$cleanupLimit}else{20}
    if($age -lt 0 -or $age -ge $limit){Stop-C2Control $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
    if($guard -and -not $cleanupReserve){$guard.CheckWork()}
    if((Age) -ge $limit){Stop-C2Control $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        if($at.Data.Contains('c2control_code')){return [string]$at.Data['c2control_code']}
        if($at.Data.Contains('c2control_support_code')){return 'fixture_refusal'}
        if($at.Data.Contains('cold_launch_code')){return 'control_refusal'}
        if($at.Data.Contains('colda_code')){return 'core_refusal'}
        if($at -is [TimeoutException]){return 'work_expired'}
    }
    return 'failed_or_unknown'
}
function Open-Public([string]$Path,[long]$Maximum) {
    Check
    if(-not [IO.Path]::IsPathFullyQualified($Path) -or
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2Control 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2Control 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2Control 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2Control 'public_input'}
        $held.Add($f);$result=$f;$f=$null;return $result
    }finally{if($f){$f.Dispose()}}
}
function Hash-Stream([IO.FileStream]$Stream) {
    Check;$Stream.Position=0;$sha=[Security.Cryptography.SHA256]::Create()
    try{$digest=[Convert]::ToHexString($sha.ComputeHash($Stream)).ToLowerInvariant()}finally{$sha.Dispose()}
    $Stream.Position=0;Check;return $digest
}
function Bytes([IO.FileStream]$Stream,[long]$Maximum) {
    Check
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2Control 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2Control 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2Control 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2Control 'source_pin'}
    return $f
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2Control 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}
function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}
function Object-Keys($Value) {
    Need ($null -ne $Value)
    if($Value -is [Collections.IDictionary]){
        foreach($key in $Value.Keys){Need ($key -is [string]);$key}
    }else{foreach($p in $Value.PSObject.Properties){$p.Name}}
}
function Get-Value($Value,[string]$Key) {
    $keys=@(Object-Keys $Value);Need ($keys -ccontains $Key)
    if($Value -is [Collections.IDictionary]){return $Value[$Key]}
    return $Value.$Key
}
function Assert-Fields($Actual,[Collections.IDictionary]$Expected,[bool]$Exact=$false) {
    $keys=@(Object-Keys $Actual)
    if($Exact){Need ($keys.Count -eq $Expected.Count)}
    foreach($key in $Expected.Keys){
        Need ($keys -ccontains $key)
        $v=if($Actual -is [Collections.IDictionary]){$Actual[$key]}else{$Actual.$key}
        $wanted=$Expected[$key]
        if($null -eq $wanted){Need ($null -eq $v)}
        elseif($wanted -is [bool]){Need ($v -is [bool] -and $v -eq $wanted)}
        elseif($wanted -is [string]){Need ($v -is [string] -and $v -ceq $wanted)}
        else{
            Need ($null -ne $v)
            Need ($v.GetType().FullName -cin @('System.SByte','System.Byte','System.Int16','System.UInt16','System.Int32','System.UInt32','System.Int64','System.UInt64'))
            Need ([decimal]$v -eq [decimal]$wanted)
        }
    }
}
function Cause($Exception) {
    $last=$Exception;$native=0;$marks=[ordered]@{};$depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++;$last=$at
        if($at -is [ComponentModel.Win32Exception]){$native=$at.NativeErrorCode}
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2control_code','c2control_support_code','cold_output_code','cold_launch_code')){
            $value=[string]$at.Data[$key]
            if($value -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$value}
        }
    }
    $type=$last.GetType().FullName
    if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks}
}
function New-PublicOutput([string]$Suffix) {
    Check;Need ($Suffix -cin @('started.json','cases.json','watchdog.json','protocol-match.started.json','protocol-wrong-pid.started.json')) 'output_cap'
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}
function Decode-Public([byte[]]$Raw) {
    Need ($Raw.Length -gt 0 -and $Raw.Length -le 65536) 'public_input'
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($Raw)
    Need ($text[0] -ne [char]0xfeff) 'public_input'
    return ConvertFrom-Json -InputObject $text -DateKind String -Depth 16
}
function Validate-Allocation {
    Check;Need ($IsWindows -and [Environment]::ProcessPath -ceq $shell) 'allocation_scope'
    $af=Open-Public $allocation 65536;Need ((Hash-Stream $af) -ceq $AllocationSha256) 'allocation_pin'
    $raw=Bytes $af 65536;$text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    Need ($text.Length -gt 0 -and $text[0] -ne [char]0xfeff) 'allocation_schema'
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try{
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','production_sha256','output_sha256','deadline_sha256','root_native_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','protocol_report_bytes','public_total_bytes')
        Need ($doc.RootElement.ValueKind -eq [Text.Json.JsonValueKind]::Object) 'allocation_schema'
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($p in $doc.RootElement.EnumerateObject()){Need ($p.Name -cin $fields -and $map.TryAdd($p.Name,$p.Value)) 'allocation_schema'}
        Need ($map.Count -eq 24) 'allocation_schema'
        foreach($key in @('schema_version','work_seconds','whole_seconds','retry_count','lane_count','public_report_bytes','protocol_report_bytes','public_total_bytes')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::Number) 'allocation_schema'
        }
        foreach($key in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','production_sha256','output_sha256','deadline_sha256','root_native_sha256','shell_sha256')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::String) 'allocation_schema'
        }
        Need ($map['schema_version'].GetInt32() -eq 1 -and
            $map['classification'].GetString() -ceq 'root-approved-cold-production-control-c2-t01' -and
            $map['root_thread_id'].GetString() -ceq '01a164ec-589c-7b41-bc34-3ba158d1f22b' -and
            $map['activated'].ValueKind -eq [Text.Json.JsonValueKind]::True -and
            $map['attempt'].GetString() -ceq $attempt -and $map['operation_set'].GetString() -ceq 'cold-production-control-two-parent-two-protocol-r01' -and
            $map['workspace'].GetString() -ceq $w -and $map['fixture_parent'].GetString() -ceq ($w+'\.tmp\'+$attempt) -and
            $map['work_seconds'].GetInt32() -eq 20 -and $map['whole_seconds'].GetInt32() -eq 30 -and
            $map['retry_count'].GetInt32() -eq 0 -and $map['lane_count'].GetInt32() -eq 1 -and
            $map['public_report_bytes'].GetInt32() -eq 65536 -and $map['public_total_bytes'].GetInt32() -eq 204800 -and $map['protocol_report_bytes'].GetInt32() -eq 4096) 'allocation_scope'
        $cases=$map['fixture_cases'];Need ($cases.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $cases.GetArrayLength() -eq 4) 'allocation_scope'
        $caseNames=@('parent-create','parent-existing','protocol-match','protocol-wrong-pid')
        for($i=0;$i -lt 4;$i++){Need ($cases[$i].ValueKind -eq [Text.Json.JsonValueKind]::String -and $cases[$i].GetString() -ceq $caseNames[$i]) 'allocation_scope'}
        $outputs=$map['outputs'];Need ($outputs.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $outputs.GetArrayLength() -eq 5) 'allocation_scope'
        $names=@('started.json','cases.json','watchdog.json','protocol-match.started.json','protocol-wrong-pid.started.json')
        for($i=0;$i -lt 5;$i++){Need ($outputs[$i].ValueKind -eq [Text.Json.JsonValueKind]::String -and $outputs[$i].GetString() -ceq $names[$i]) 'allocation_scope'}
        $pins=[ordered]@{}
        foreach($key in @('preflight','watchdog','production','output','deadline','root_native','shell')){
            $hash=$map[$key+'_sha256'].GetString();Need ($hash -cmatch '\A[0-9a-f]{64}\z') 'allocation_schema';$pins[$key]=$hash
        }
        $fixedPins=[ordered]@{production='12661c746bfad302d5dabf0acdb5ea621005f5650120300a5e971d7507e6d976';output='331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98';root_native='a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382';deadline='0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45';shell='362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'}
        $fixedLengths=[ordered]@{production=71277;output=21721;root_native=25596;deadline=6038;shell=301368}
        foreach($key in $fixedPins.Keys){Need ($pins[$key] -ceq $fixedPins[$key]) 'source_pin'}
        $paths=[ordered]@{preflight=$preflight;watchdog=$watchdog;production=$production;output=$output;deadline=$deadline;root_native=$rootNative;shell=$shell}
        $files=[ordered]@{}
        foreach($key in $paths.Keys){
            if($key -cin @('preflight','watchdog')){
                $files[$key]=Open-Public $paths[$key] 131072;Need ((Hash-Stream $files[$key]) -ceq $pins[$key]) 'source_pin'
            }else{$files[$key]=Pin $paths[$key] $fixedLengths[$key] $pins[$key]}
        }
        return [pscustomobject]@{Pins=$pins;SourceFiles=$files}
    }finally{$doc.Dispose()}
}
function Source-Pins($Binding) {
    $v=[ordered]@{}
    foreach($key in $Binding.SourceFiles.Keys){$v[$key]=Hash-Stream $Binding.SourceFiles[$key];Need ($v[$key] -ceq $Binding.Pins[$key]) 'source_pin'}
    return $v
}
function Assert-ParentCase($c,[bool]$Created) {
    Assert-Fields $c ([ordered]@{name=$(if($Created){'parent-create'}else{'parent-existing'});observed=$true;parent_created=$Created;parent_existing=(-not $Created)
        guard_peak=5;guard_open_attempts=$(if($Created){6}else{5});guard_close_attempts=5;guard_native_closed=5;guard_remaining=0;guard_cleanup_unknown=$false;guard_native_error=0
        owner_created=$true;owner_transferred=$false;owner_ancestry=5;owner_prepared=6;owner_after_handoff=6;owner_remaining=0;owner_cleanup_unknown=$false;owner_native_error=0
        owner_receipt_mask=0;owner_receipt_transfers=0;guard_disposed=$true;owner_disposed=$true;core_invoked=$false;capture_invoked=$false;receipt_invoked=$false})
    Need ($c.acl_before_sha256 -cmatch '\A[0-9a-f]{64}\z' -and $c.acl_before_sha256 -ceq $c.acl_after_sha256) 'parent_acl_preserved'
    Need ($c.completed_qpc -gt $StartTimestamp -and $c.completed_qpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency)) 'case_clock'
}
function Assert-Protocol($p,[bool]$Match) {
    Assert-Fields $p ([ordered]@{name=$(if($Match){'protocol-match'}else{'protocol-wrong-pid'});function_test_only=$true;production_entrypoint_invoked=$false;writer_managed_closed=$true;definition_count=26})
    Need ($p.record_bytes -gt 0 -and $p.record_bytes -le 4096 -and $p.record_sha256 -cmatch '\A[0-9a-f]{64}\z' -and $p.definitions_sha256 -cmatch '\A[0-9a-f]{64}\z')
    Assert-Fields $p.protocol ([ordered]@{returned=$Match;public_peak=1;public_remaining=0;managed_disposed=$true;managed_native_close_observed=$false;claim_created=$false;private_work_invoked=$false})
    if($Match){Need ($null -eq $p.protocol.cause -and $p.protocol.started_sha256 -ceq $p.record_sha256) 'protocol_match'}
    else{
        Need ($null -eq $p.protocol.started_sha256) 'protocol_wrong_pid'
        Assert-Fields $p.protocol.cause ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0})
        Assert-Fields $p.protocol.cause.marks ([ordered]@{cold_launch_code='schema_value'})
        Need (@($p.protocol.cause.chain).Count -ge 1 -and @($p.protocol.cause.chain).Count -le 16) 'protocol_wrong_pid'
    }
}
function Assert-WorkerReport($v,$Pins,[int]$WorkerPid,[string]$WorkerStarted,[int]$SupervisorPid) {
    Assert-Fields $v ([ordered]@{classification='cold-production-control-c2-t01';attempt=$attempt;allocation_sha256=$AllocationSha256;result='observed';failure=$null;failure_cause=$null
        cleanup_cause=$null;controller_unknown=$false;whole_attempt_verdict='not_established';child_pid=$WorkerPid;worker_started_utc=$WorkerStarted;started_supervisor_pid=$SupervisorPid
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1
        protocol_function_test_only=$true;production_entrypoint_invoked=$false;core_type_absent=$true;capture_invoked=$false;inventory_invoked=$false;receipt_invoked=$false;private_dataset_access=$false
        managed_inputs_disposed=$true;public_inputs_before_close=9;worker_self_disposed=$true;job_finished=$true;owned_children_after=0;compiler_error=$false;diagnostic_cap=$false
        native_guard_closes=10;owner_lifecycles=2;explicit_helper_peak_bound=11;combined_controlled_bound=32;fixture_retained=$true})
    foreach($k in $Pins.Keys){Assert-Fields $v.source_before ([ordered]@{$k=$Pins[$k]});Assert-Fields $v.source_after ([ordered]@{$k=$Pins[$k]})}
    Need (@($v.parent_cases).Count -eq 2 -and @($v.protocol_cases).Count -eq 2 -and @($v.mapping).Count -eq 3 -and @($v.diagnostics).Count -eq 0)
    Assert-ParentCase $v.parent_cases[0] $true;Assert-ParentCase $v.parent_cases[1] $false
    Need ($v.parent_cases[0].acl_after_sha256 -ceq $v.parent_cases[1].acl_before_sha256) 'same_parent_acl'
    Assert-Protocol $v.protocol_cases[0] $true;Assert-Protocol $v.protocol_cases[1] $false
    Need ($v.protocol_cases[0].definitions_sha256 -ceq $v.protocol_cases[1].definitions_sha256)
    foreach($m in $v.mapping){Need ($m.source_sha256 -ceq $m.inverse_sha256 -and $m.mapped_sha256 -cmatch '\A[0-9a-f]{64}\z' -and $m.path_replacements -eq 1)}
    foreach($k in @('deadline','root-native','parent-guards','parent-create','parent-existing')){Assert-Fields $v.initialization_calls ([ordered]@{$k=1})}
}
function New-Drain([IO.StreamReader]$Reader) {
    $buffer=[char[]]::new(1024)
    return [pscustomobject]@{Reader=$Reader;Buffer=$buffer;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($buffer,0,$buffer.Length);Eof=$false;Cap=$false}
}
function Pump($Drain) {
    if(-not $Drain -or $Drain.Eof -or -not $Drain.Task.IsCompleted){return}
    $n=$Drain.Task.GetAwaiter().GetResult()
    if($n -eq 0){$Drain.Eof=$true;Note-EOF;return}
    if($Drain.Text.Length+$n -gt 4096){$Drain.Cap=$true;Stop-C2Control 'output_cap'}
    $null=$Drain.Text.Append($Drain.Buffer,0,$n)
    $Drain.Task=$Drain.Reader.ReadAsync($Drain.Buffer,0,$Drain.Buffer.Length)
}
function Set-FirstReason([string]$Reason) {
    if($null -eq $completion.first_failure_reason){$completion.first_failure_reason=$Reason}
}
function Note-ExitIndication([string]$Basis) {
    if($script:exitQpc -eq 0){
        $script:exitQpc=[Diagnostics.Stopwatch]::GetTimestamp()
        $completion.exit_indication_basis=$Basis
    }
    $metrics.stopped_for_exit=$true
}
function Note-EOF {
    if($script:eofQpc -eq 0 -and $out -and $err -and $out.Eof -and $err.Eof){$script:eofQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
}
function Completion-Gate {
    return [ordered]@{qpc=[Diagnostics.Stopwatch]::GetTimestamp();stage=$normalStage
        exit_indicated=($exitQpc -gt 0);exit_indication_qpc=$exitQpc;reaped=$reaped;reap_qpc=$reapQpc;child_exit=$childExit
        stdout_eof=[bool]($out -and $out.Eof);stderr_eof=[bool]($err -and $err.Eof);both_eof_qpc=$eofQpc
        forced=$control.Forced;unknown=$control.Unknown;observer_failed=$metrics.observer_failed
        first_failure_reason=$completion.first_failure_reason}
}
function Record-Wait([bool]$Waited,[string]$Stage,[int]$Milliseconds,[long]$Before,[long]$After) {
    Need ($Stage -cin @('normal','cleanup') -and $Milliseconds -ge 1 -and $Milliseconds -le 20 -and $After -ge $Before) 'completion_trace'
    $attemptKey=$Stage+'_wait_attempts';$completion[$attemptKey]++
    $countKey=$Stage+$(if($Waited){'_wait_true'}else{'_wait_false'});$completion[$countKey]++
    $completion.last_wait_qpc=$After;$completion.last_wait_result=$Waited
    $labels=[Collections.Generic.List[string]]::new()
    $firstKey=$Stage+'_first_wait_seen'
    if(-not $completion[$firstKey]){$completion[$firstKey]=$true;$labels.Add($Stage+'_first_wait')}
    if($Stage -ceq 'normal' -and -not $Waited){
        if($exitQpc -gt 0 -and -not $completion.false_after_exit_seen){$completion.false_after_exit_seen=$true;$labels.Add('normal_false_after_exit')}
        if($out -and $err -and $out.Eof -and $err.Eof -and -not $completion.false_after_eof_seen){$completion.false_after_eof_seen=$true;$labels.Add('normal_false_after_eof')}
    }
    if($Waited -and -not $script:reaped){
        # Only a real successful wait advances this state; it is never reset.
        $script:reaped=$true;$script:reapQpc=$After;$completion.first_reap_stage=$Stage
        Note-ExitIndication 'wait_signal'
        $script:childExit=$child.ExitCode # read only after the successful signal wait
        $labels.Add($Stage+'_reap_success')
    }
    foreach($label in $labels){
        Need ($waitTrace.Count -lt 8) 'completion_trace_cap'
        $waitTrace.Add([ordered]@{kind=$label;waited=$Waited;milliseconds=$Milliseconds;before_qpc=$Before;after_qpc=$After
            exit_indicated=($exitQpc -gt 0);stdout_eof=[bool]($out -and $out.Eof);stderr_eof=[bool]($err -and $err.Eof)
            reaped=$reaped;child_exit=$childExit})
    }
}
function Wait-ForReap([string]$Stage,[int]$LimitSeconds) {
    if($reaped){return}
    $remaining=[Math]::Floor(($LimitSeconds-(Age))*1000)
    if($remaining -lt 1){
        if($Stage -ceq 'normal'){Set-FirstReason 'unreaped_work_timeout';Stop-C2Control 'unreaped_work_timeout'}
        return
    }
    $milliseconds=[int][Math]::Min(20,$remaining)
    $before=[Diagnostics.Stopwatch]::GetTimestamp();$waited=$child.WaitForExit($milliseconds);$after=[Diagnostics.Stopwatch]::GetTimestamp()
    Record-Wait $waited $Stage $milliseconds $before $after
}
function Sample-Child {
    Check;Pump $out;Pump $err
    if($metrics.stopped_for_exit){return}
    try{$alreadyExited=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='pre_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-C2Control 'observer_failure'}
    if($alreadyExited){Note-ExitIndication 'has_exited';$metrics.known_exited_skips++;$metrics.stopped_for_exit=$true;return}
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    if($now -lt $nextSampleQpc){return}
    Need ($metrics.sample_attempts -lt 301) 'observer_cap'
    $metrics.sample_attempts++;$script:nextSampleQpc=$now+[long]([Diagnostics.Stopwatch]::Frequency/10)
    $api='refresh';$samplePid=0;$sampleHandle=0L;$sampleStarted=$null;$handles=0;$working=0L
    try {
        $child.Refresh()
        $api='pid';$samplePid=$child.Id
        $api='retained_handle';$sampleHandle=$child.Handle.ToInt64()
        $api='start_time';$sampleStarted=$child.StartTime.ToUniversalTime().ToString('o')
        $api='handle_count';$handles=$child.HandleCount
        $api='peak_working_set';$working=$child.PeakWorkingSet64
    }catch{
        $failedFinished=[Diagnostics.Stopwatch]::GetTimestamp();$attemptDuration=($failedFinished-$now)/[double][Diagnostics.Stopwatch]::Frequency
        $metrics.maximum_sample_duration_seconds=[Math]::Max($metrics.maximum_sample_duration_seconds,$attemptDuration);$metrics.total_sample_duration_seconds+=$attemptDuration
        $cause=Cause $_.Exception;$invalidOperation=$cause.type -ceq 'System.InvalidOperationException';$hasWin32=$false
        for($at=$_.Exception;$null -ne $at;$at=$at.InnerException){if($at -is [ComponentModel.Win32Exception]){$hasWin32=$true}}
        $exited=$false;$exitCheckFailed=$false
        try{$exited=$child.HasExited}catch{$exitCheckFailed=$true}
        # Narrow natural race only: an InvalidOperationException while reading
        # Process APIs, retained identity already fixed, then confirmed exited.
        # Win32/access/live/identity failures never become a successful race.
        if($invalidOperation -and -not $hasWin32 -and $cause.native_error -eq 0 -and $exited -and -not $exitCheckFailed -and
            $api -cin @('refresh','pid','retained_handle','start_time','handle_count','peak_working_set')){
            Note-ExitIndication 'confirmed_api_exit'
            $metrics.exit_race_count++;$metrics.exit_race_api=$api;$metrics.exit_race_cause=$cause
            $metrics.exit_race_confirmed_exited=$true;$metrics.stopped_for_exit=$true
            $metrics.exit_race_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
            return
        }
        $metrics.observer_failed=$true;$metrics.failure_api=$api;$metrics.failure_cause=$cause;$metrics.failure_confirmed_exited=$exited
        Stop-C2Control 'observer_failure'
    }
    if($samplePid -ne $childId -or $sampleHandle -ne $retainedProcessHandle -or $sampleStarted -cne $childStartedUtc -or $handles -lt 0 -or $working -le 0){
        $metrics.observer_failed=$true;$metrics.failure_api='identity_or_range';Stop-C2Control 'observer_identity'
    }
    Need ($handles -le 1024 -and $working -le 1073741824L) 'observer_resource_cap'
    try{$exitedAfterRead=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='post_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-C2Control 'observer_failure'}
    $finished=[Diagnostics.Stopwatch]::GetTimestamp();$duration=($finished-$now)/[double][Diagnostics.Stopwatch]::Frequency
    Need ($duration -ge 0) 'observer_clock'
    if($exitedAfterRead){
        # Successful sequential reads that straddled exit are not a live sample.
        $metrics.maximum_sample_duration_seconds=[Math]::Max($metrics.maximum_sample_duration_seconds,$duration);$metrics.total_sample_duration_seconds+=$duration
        Note-ExitIndication 'post_query_exit'
        $metrics.exit_race_count++;$metrics.exit_race_api='post_query_exit';$metrics.exit_race_cause=$null
        $metrics.exit_race_confirmed_exited=$true;$metrics.stopped_for_exit=$true;$metrics.exit_race_qpc=$finished
        return
    }
    if($metrics.sample_count -eq 0){$metrics.first_live_qpc=$finished;$metrics.first_sample_started_qpc=$now}
    else{$gap=($finished-$metrics.last_live_qpc)/[double][Diagnostics.Stopwatch]::Frequency;$metrics.maximum_successful_sample_gap_seconds=[Math]::Max($metrics.maximum_successful_sample_gap_seconds,$gap);$uncertainGap=($finished-$metrics.last_sample_started_qpc)/[double][Diagnostics.Stopwatch]::Frequency;$metrics.maximum_measurement_gap_upper_seconds=[Math]::Max($metrics.maximum_measurement_gap_upper_seconds,$uncertainGap)}
    $metrics.last_live_qpc=$finished;$metrics.last_sample_started_qpc=$now;$metrics.sample_count++
    $metrics.maximum_sample_duration_seconds=[Math]::Max($metrics.maximum_sample_duration_seconds,$duration)
    $metrics.total_sample_duration_seconds+=$duration
    $metrics.sampled_peak_os_handles=[Math]::Max($metrics.sampled_peak_os_handles,$handles)
    $metrics.observed_peak_working_set_bytes=[Math]::Max($metrics.observed_peak_working_set_bytes,$working)
    Check;Pump $out;Pump $err;Check
}
$self=[Diagnostics.Process]::GetCurrentProcess();$self.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$startup=([DateTime]::UtcNow-$self.StartTime.ToUniversalTime()).TotalSeconds
$StartTimestamp=[Diagnostics.Stopwatch]::GetTimestamp()-[long]($startup*[Diagnostics.Stopwatch]::Frequency)
$done=[Threading.ManualResetEvent]::new($false)
$control=[hashtable]::Synchronized(@{Child=$null;Forced=$false;Unknown=$false})
# Accepted guardian model; armed before filesystem validation/helper/compiler.
$guardian=[powershell]::Create()
$null=$guardian.AddScript({
    param($Owner,$Control,$Done,[long]$Start)
    try {
        while(-not $Done.WaitOne(20)){
            $age=([Diagnostics.Stopwatch]::GetTimestamp()-$Start)/[double][Diagnostics.Stopwatch]::Frequency
            if($age -ge 25){
                $Control.Forced=$true;$p=$Control.Child
                if($p){try{if(-not $p.HasExited){$p.Kill()};$null=$p.WaitForExit(20)}catch{$Control.Unknown=$true}}
            }
            if($age -ge 29){$Owner.Kill();return}
        }
    }catch{try{$Owner.Kill()}catch{}}
}).AddArgument($self).AddArgument($control).AddArgument($done).AddArgument($StartTimestamp)
$ticket=$guardian.BeginInvoke()

$binding=$null;$authorized=$false;$claimed=$false;$child=$null;$childStarted=$false;$childReport=$null
$childId=0;$childStartedUtc=$null;$retainedProcessHandle=0L;$registeredChildReleased=$false
$out=$null;$err=$null;$reaped=$false;$childExit=$null;$stdioDisposed=$false;$managedDisposed=$false;$publicInputsBeforeClose=0
$failure='not_started';$failureCause=$null;$cleanupFirstCause=$null;$result=1;$normalCandidate=$false;$cleanupKillAttempted=$false
$exitQpc=0L;$reapQpc=0L;$eofQpc=0L;$spawnQpc=0L;$sourceAfter=[ordered]@{};$nextSampleQpc=0L;$reportClosed=$false
$metrics=[ordered]@{nominal_interval_ms=100;sample_attempts=0;sample_count=0;first_live_qpc=0L;last_live_qpc=0L;first_sample_started_qpc=0L;last_sample_started_qpc=0L
    maximum_successful_sample_gap_seconds=0.0;maximum_measurement_gap_upper_seconds=0.0;maximum_sample_duration_seconds=0.0;total_sample_duration_seconds=0.0
    sampled_peak_os_handles=0;observed_peak_working_set_bytes=0L
    known_exited_skips=0;stopped_for_exit=$false;exit_race_count=0;exit_race_api=$null;exit_race_cause=$null;exit_race_qpc=0L;exit_race_confirmed_exited=$false
    observer_failed=$false;failure_api=$null;failure_cause=$null;failure_confirmed_exited=$false
    exit_observed_qpc=0L;exit_tail_seconds=$null;exit_tail_upper_seconds=$null;exit_tail_basis='successful sample return to observed exit; upper bound starts at that sample start; API measurement lies within its interval';startup_to_first_sample_seconds=$null;unobserved_exit_tail=$true;sampled_handle_peak_can_be_missed=$true
    working_set_basis='Process lifetime OS high-water through last live observation; includes shell/compiler/setup/CLR/native overhead'
    handle_basis='sequential refreshed Process API samples on retained worker identity; no guarantee of exact OS handle peak'}
$normalStage='bootstrap'
$waitTrace=[Collections.Generic.List[object]]::new()
$completion=[ordered]@{exit_indication_basis=$null;first_reap_stage=$null
    normal_wait_attempts=0;normal_wait_true=0;normal_wait_false=0;normal_first_wait_seen=$false
    cleanup_wait_attempts=0;cleanup_wait_true=0;cleanup_wait_false=0;cleanup_first_wait_seen=$false
    false_after_exit_seen=$false;false_after_eof_seen=$false;last_wait_qpc=0L;last_wait_result=$null
    normal_gate=$null;normal_gate_passed=$false;pre_cleanup_gate=$null;pre_cleanup_first_reason=$null;first_failure_reason=$null}
try {
    Need ($PSCommandPath -ceq $watchdog) 'allocation_scope'
    $binding=Validate-Allocation;$authorized=$true
    foreach($suffix in @('started.json','cases.json','watchdog.json','protocol-match.started.json','protocol-wrong-pid.started.json')){
        Need (-not [IO.File]::Exists("$e\$attempt.$suffix") -and -not [IO.Directory]::Exists("$e\$attempt.$suffix")) 'output_collision'
    }
    Need ((Age) -lt 15) 'bootstrap_reserve'
    $marker=New-PublicOutput 'started.json';$claimed=$true
    try{Write-Public $marker ([ordered]@{classification='cold-production-control-c2-start-t01';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;supervisor_pid=$self.Id
        native_started_utc=$self.StartTime.ToUniversalTime().ToString('o');permission_consumed=$true;work_seconds=20;whole_seconds=30
        retry_count=0;lane_count=1;pins=$binding.Pins;whole_attempt_verdict='not_established'})}finally{$marker.Dispose()}
    $info=[Diagnostics.ProcessStartInfo]::new($shell);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WorkingDirectory=$w
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($argValue in @('-NoLogo','-NoProfile','-NonInteractive','-Mta','-File',$preflight,'-AllocationSha256',$AllocationSha256,
        '-StartTimestamp',$StartTimestamp.ToString([Globalization.CultureInfo]::InvariantCulture))){$info.ArgumentList.Add($argValue)}
    $child=[Diagnostics.Process]::new();$child.StartInfo=$info;$control.Child=$child
    $normalStage='spawn';Check;$spawnQpc=[Diagnostics.Stopwatch]::GetTimestamp();Need ($child.Start()) 'child_start_failed';$childStarted=$true
    $childId=$child.Id;$retainedProcessHandle=$child.Handle.ToInt64();$childStartedUtc=$child.StartTime.ToUniversalTime().ToString('o')
    $child.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $out=New-Drain $child.StandardOutput;$err=New-Drain $child.StandardError
    $normalStage='completion'
    while($true){
        Check;Pump $out;Pump $err;Sample-Child;Check;Pump $out;Pump $err;Note-EOF
        if($child.HasExited){Note-ExitIndication 'has_exited'}
        if($control.Forced -or $control.Unknown){Set-FirstReason 'completion_forced_or_unknown';Stop-C2Control 'completion_forced_or_unknown'}
        if(-not $reaped){Wait-ForReap 'normal' 20}
        Check;Pump $out;Pump $err;Note-EOF
        if($reaped -and $childExit -ne 0){Set-FirstReason 'child_exit_nonzero';Stop-C2Control 'child_exit_nonzero'}
        if($reaped -and $out.Eof -and $err.Eof){break}
        if($reaped){
            $remaining=[Math]::Floor((20-(Age))*1000)
            if($remaining -ge 1){[Threading.Thread]::Sleep([int][Math]::Min(5,$remaining))}
        }
    }
    $completion.normal_gate=Completion-Gate
    Need ($reaped -and $childExit -eq 0 -and $out.Eof -and $err.Eof -and -not $control.Forced -and -not $control.Unknown -and
        $exitQpc -gt $StartTimestamp -and $reapQpc -gt $StartTimestamp -and $eofQpc -gt $StartTimestamp -and
        $exitQpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency) -and
        $reapQpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency) -and
        $eofQpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency)) 'completion_gate'
    $completion.normal_gate_passed=$true
    Need (-not $metrics.observer_failed -and $metrics.sample_count -gt 0 -and $metrics.sample_attempts -le 301) 'observer_failure'
    if($metrics.exit_race_count -gt 0){Need ($metrics.exit_race_count -eq 1 -and $metrics.exit_race_confirmed_exited -and $metrics.stopped_for_exit) 'observer_race'}
    $normalStage='worker_report'
    $report=Open-Public "$e\$attempt.cases.json" 65536;$childReport=Decode-Public (Bytes $report 65536)
    Assert-WorkerReport $childReport $binding.Pins $childId $childStartedUtc $self.Id
    foreach($p in $childReport.protocol_cases){
        $record=Open-Public "$e\$attempt.$($p.name).started.json" 4096
        Need ($record.Length -eq $p.record_bytes -and (Hash-Stream $record) -ceq $p.record_sha256) 'protocol_record_pin'
        $data=Decode-Public (Bytes $record 4096)
        Assert-Fields $data ([ordered]@{schema_version=1;classification='s53-cold-a-started';attempt=($attempt+'.'+$p.name);decision_sha256=$AllocationSha256
            start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;whole_attempt_verdict='not_established'
            supervisor_pid=$self.Id;supervisor_started_utc=$self.StartTime.ToUniversalTime().ToString('o')
            parent_pid=$(if($p.name -ceq 'protocol-match'){$childId}else{if($childId -eq 1){2}else{1}});parent_started_utc=$childStartedUtc
            permission_consumed=$true;work_seconds=45;whole_seconds=60})
        Need (@(Object-Keys $data).Count -eq 15 -and @(Object-Keys $data.source_pins).Count -eq 2) 'protocol_record_schema'
        Assert-Fields $data.source_pins ([ordered]@{launcher=$binding.Pins.production;output=$binding.Pins.output})
    }
    Need ($out.Text.Length -eq 0 -and $err.Text.Length -eq 0 -and -not $out.Cap -and -not $err.Cap) 'child_output'
    $normalStage='source_after';$sourceAfter=Source-Pins $binding;Check;$normalCandidate=$true;$failure=$null;$normalStage='completed'
}catch{
    $failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception
    if($failure -ceq 'work_expired' -and $normalStage -ceq 'completion'){
        if(-not $reaped){Set-FirstReason 'unreaped_work_timeout'}
        elseif(-not $out.Eof -or -not $err.Eof){Set-FirstReason 'eof_work_timeout'}
        else{Set-FirstReason 'completion_work_timeout'}
    }elseif($failure -ceq 'work_expired'){Set-FirstReason ($normalStage+'_work_timeout')}
    elseif($normalStage -ceq 'worker_report'){Set-FirstReason 'worker_report_validation_failed'}
    elseif($normalStage -ceq 'source_after'){Set-FirstReason 'source_integrity_failed'}
    else{Set-FirstReason $failure}
}
finally {
    # Capture original normal state and reason before cleanup can reap/drain.
    $completion.pre_cleanup_gate=Completion-Gate
    $completion.pre_cleanup_first_reason=$completion.first_failure_reason
    $normalStage='cleanup'
    $cleanupReserve=$true
    if($child -and $childStarted){
        try{
            if(-not $reaped){
                if($child.HasExited){Note-ExitIndication 'cleanup_has_exited'}
                else{$cleanupKillAttempted=$true;$child.Kill()}
            }
            while((Age) -lt 24 -and -not $reaped){
                Wait-ForReap 'cleanup' 24;Pump $out;Pump $err
            }
            if(-not $reaped){$control.Unknown=$true;Set-FirstReason 'cleanup_unreaped'}
            else{$childExit=$child.ExitCode}
        }catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        while((Age) -lt 26 -and (($out -and -not $out.Eof) -or ($err -and -not $err.Eof))){
            try{Pump $out;Pump $err}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception};break}
            [Threading.Thread]::Sleep(5)
        }
        if(($out -and -not $out.Eof) -or ($err -and -not $err.Eof)){$control.Unknown=$true}
        elseif($out -and $err -and $eofQpc -eq 0){$eofQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
    }
    $publicInputsBeforeClose=$held.Count;$managedDisposed=Close-PublicInputs;if(-not $managedDisposed){$control.Unknown=$true}
    # Keep guardian armed while readers and process handles are disposed.
    try{if($out){$out.Reader.Dispose()};if($err){$err.Reader.Dispose()};$stdioDisposed=$true}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    if($child -and ($reaped -or -not $childStarted)){
        try{$control.Child=$null;$child.Dispose();$child=$null;$registeredChildReleased=$true}catch{$control.Unknown=$true}
    }
}
try {
    Check
    if($metrics.sample_count -gt 0){
        $metrics.startup_to_first_sample_seconds=($metrics.first_live_qpc-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency
        if($exitQpc -gt 0){$metrics.exit_tail_seconds=($exitQpc-$metrics.last_live_qpc)/[double][Diagnostics.Stopwatch]::Frequency;$metrics.exit_tail_upper_seconds=($exitQpc-$metrics.last_sample_started_qpc)/[double][Diagnostics.Stopwatch]::Frequency;Need ($metrics.exit_tail_seconds -ge 0) 'observer_clock'}
    }
    $metrics.exit_observed_qpc=$exitQpc
    $good=$normalCandidate -and $completion.normal_gate_passed -and $null -eq $completion.first_failure_reason -and $reaped -and $childExit -eq 0 -and $registeredChildReleased -and
        $stdioDisposed -and $managedDisposed -and $out.Eof -and $err.Eof -and -not $out.Cap -and -not $err.Cap -and
        -not $control.Unknown -and -not $control.Forced -and -not $cleanupKillAttempted -and -not $metrics.observer_failed -and
        $metrics.sample_count -gt 0 -and $null -eq $cleanupFirstCause -and $publicInputsBeforeClose -eq 11
    if(-not $good -and -not $failure){$failure='cleanup_unknown';Set-FirstReason 'cleanup_unknown'}
    if($authorized -and $claimed){
        $file=New-PublicOutput 'watchdog.json';$safe=$null
        try{
            $safe=$file.SafeFileHandle
            Write-Public $file ([ordered]@{classification='cold-production-control-c2-watchdog-t01';attempt=$attempt;allocation_sha256=$AllocationSha256
                result=$(if($good){'observed'}else{'failed_or_unknown'});failure=$failure;failure_cause=$failureCause;cleanup_first_cause=$cleanupFirstCause
                whole_attempt_verdict='not_established';child_pid=$childId;child_started_utc=$childStartedUtc;retained_process_handle=$retainedProcessHandle
                child_exit=$childExit;child_reaped=$reaped;registered_child_released=$registeredChildReleased;stdio_disposed=$stdioDisposed
                forced=$control.Forced;cleanup_kill_attempted=$cleanupKillAttempted;cleanup_unknown=$control.Unknown;managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose
                stdout=$(if($out){$out.Text.ToString()}else{''});stderr=$(if($err){$err.Text.ToString()}else{''})
                stdout_eof=$(if($out){$out.Eof}else{$false});stderr_eof=$(if($err){$err.Eof}else{$false});output_cap=($out -and $out.Cap) -or ($err -and $err.Cap)
                resource_caps=[ordered]@{sampled_os_handles=1024;sampled_peak_working_set_bytes=1073741824L;maximum_sample_attempts=301};observer=$metrics;completion=$completion;wait_trace=$waitTrace.ToArray();source_pins_after=$sourceAfter;start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
                worker_spawn_qpc=$spawnQpc;parent_before_spawn_seconds=(($spawnQpc-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency);exit_observed_qpc=$exitQpc;exit_indication_qpc=$exitQpc;reap_observed_qpc=$reapQpc;both_eof_qpc=$eofQpc;private_dataset_access=$false;owned_worker_processes_after=$(if($reaped){0}else{$null})
                work_seconds=20;whole_seconds=30;worker_cleanup_seconds=24;parent_cleanup_seconds=28;guardian_child_seconds=25;guardian_self_seconds=29
                lane_count=1;retry_count=0;observed_seconds=(Age)})
        }finally{$file.Dispose();$reportClosed=$null -ne $safe -and $safe.IsClosed}
        Need $reportClosed 'cleanup_unknown'
    }else{$good=$false}
    Check
    if($control.Child -and -not $reaped){
        while((Age) -lt 29){[Threading.Thread]::Sleep(20)}
        $self.Kill();exit 1
    }
    $null=$done.Set()
    if(-not $ticket.AsyncWaitHandle.WaitOne(300)){Stop-C2Control 'cleanup_unknown'}
    $null=$guardian.EndInvoke($ticket)
    if($good -and $reportClosed -and -not $control.Unknown -and -not $control.Forced -and (Age) -lt 28){$result=0}
}catch{
    $result=1
    if($null -eq $failureCause){$failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception}
    elseif($null -eq $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}
    Set-FirstReason $(if($failure){$failure}else{'final_report_or_guardian_failure'})
}
finally {
    if($ticket.IsCompleted){$guardian.Dispose();$done.Dispose();if($child){$child.Dispose()};$self.Dispose()}
}
if($result -eq 0){[Console]::Out.WriteLine('C2-T focused public controls returned; root actual-return review required.')}
else{
    if($null -eq $completion.first_failure_reason){
        if($control.Forced -or $control.Unknown){Set-FirstReason 'final_force_or_unknown'}
        elseif((Age) -ge 28){Set-FirstReason 'parent_cleanup_deadline'}
        else{Set-FirstReason 'final_return_failed'}
        if($null -eq $failure){$failure=$completion.first_failure_reason}
    }
    $diagnostic=ConvertTo-Json -Depth 6 -Compress -InputObject ([ordered]@{classification='cold-production-control-supervisor-failed';first_failure_reason=$completion.first_failure_reason;pre_cleanup_first_reason=$completion.pre_cleanup_first_reason;failure=$failure;cause=$failureCause;cleanup_first_cause=$cleanupFirstCause;forced=$control.Forced;cleanup_unknown=$control.Unknown})
    if($diagnostic.Length -le 150){[Console]::Error.WriteLine($diagnostic)}
    else{[Console]::Error.WriteLine('{"classification":"cold-production-control-supervisor-failed","failure":"diagnostic_cap"}')}
}
exit $result
