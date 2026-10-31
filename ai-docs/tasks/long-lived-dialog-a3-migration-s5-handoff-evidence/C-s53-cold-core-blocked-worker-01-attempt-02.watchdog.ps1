[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256)
# C2-Q blocked worker: dormant source only; no runtime allocation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-blocked-worker-01-attempt-02'
$e="$w\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence"
$allocation="$e\$attempt.root-allocation.json"
$preflight="$e\$attempt.preflight.ps1"
$watchdog="$e\$attempt.watchdog.ps1"
$core="$w\scripts\migration-s53-cold-preservation.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1"
$deadline="$w\scripts\migration-s5-l1-deadline.ps1"
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$held=[Collections.Generic.List[IO.FileStream]]::new()
$guard=$null

$cleanupReserve=$false
$eventOwners=[Collections.Generic.List[object]]::new();$eventCleanupUnknown=$false;$eventCleanupFirstCause=$null
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}

function Stop-C2Q([string]$Code) {
    $err=[InvalidOperationException]::new('C2Q bounded refusal');$err.Data['c2q_code']=$Code;throw $err
}

function Check {
    $age=Age;$limit=if($cleanupReserve){28}else{20}
    if($age -lt 0 -or $age -ge $limit){Stop-C2Q $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
    if($guard -and -not $cleanupReserve){$guard.CheckWork()}
}

function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        $value=[string]$at.Data['c2q_code']
        if($value -in @('work_expired','public_input','allocation_pin','allocation_schema','allocation_scope','source_pin',
            'output_collision','output_cap','start_claim','bootstrap_reserve','job_setup','core_type_cached','definitions_shape',
            'diagnostic_cap','compiler_error','signature_mismatch','cleanup_unknown','child_return','assertion',
            'fixture_collision','fixture_setup','fixture_hash','premise_failure','typed_refusal','case_cleanup',
            'cleanup_expired','event_scope','event_collision','apartment','child_identity','premature_exit','unexpected_wait_return','unexpected_core_return')){return $value}
        if($at.Data.Contains('colda_code')){return 'core_failure'}
    }
    return 'failed_or_unknown'
}

function Open-Public([string]$Path,[long]$Maximum) {
    Check
    if(-not [IO.Path]::IsPathFullyQualified($Path) -or
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2Q 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2Q 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2Q 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2Q 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2Q 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2Q 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2Q 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}

function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2Q 'source_pin'}
    return $f
}

function Validate-Allocation {
    Check
    if(-not $IsWindows -or [Environment]::ProcessPath -cne $shell){Stop-C2Q 'allocation_scope'}
    $af=Open-Public $allocation 65536
    if((Hash-Stream $af) -cne $AllocationSha256){Stop-C2Q 'allocation_pin'}
    $raw=Bytes $af 65536
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    if($text.Length -eq 0 -or $text[0] -eq [char]0xfeff){Stop-C2Q 'allocation_schema'}
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        if($doc.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object){Stop-C2Q 'allocation_schema'}
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($property in $doc.RootElement.EnumerateObject()){
            if($property.Name -cnotin $fields -or -not $map.TryAdd($property.Name,$property.Value)){Stop-C2Q 'allocation_schema'}
        }
        if($map.Count -ne 22){Stop-C2Q 'allocation_schema'}
        foreach($name in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256')){
            if($map[$name].ValueKind -ne [Text.Json.JsonValueKind]::String){Stop-C2Q 'allocation_schema'}
        }
        if($map['schema_version'].GetInt32() -ne 1 -or $map['classification'].GetString() -cne 'root-approved-cold-core-blocked-worker-c2-r01' -or
           $map['root_thread_id'].GetString() -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or
           $map['activated'].ValueKind -ne [Text.Json.JsonValueKind]::True -or $map['attempt'].GetString() -cne $attempt -or
           $map['operation_set'].GetString() -cne 'cold-blocked-worker-one-only-r01' -or $map['workspace'].GetString() -cne $w -or
           $map['fixture_parent'].GetString() -cne ($w+'\.tmp\C-s53-cold-core-blocked-worker-01-attempt-02') -or
           $map['work_seconds'].GetInt32() -ne 20 -or $map['whole_seconds'].GetInt32() -ne 30 -or
           $map['retry_count'].GetInt32() -ne 0 -or $map['lane_count'].GetInt32() -ne 1 -or
           $map['public_report_bytes'].GetInt32() -ne 65536 -or $map['public_total_bytes'].GetInt32() -ne 196608){Stop-C2Q 'allocation_scope'}
        $cases=$map['fixture_cases'];$fixed=@((Case-Specs).Selector)
        if($cases.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $cases.GetArrayLength() -ne 1){Stop-C2Q 'allocation_scope'}
        for($n=0;$n -lt 1;$n++){
            if($cases[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $cases[$n].GetString() -cne $fixed[$n]){Stop-C2Q 'allocation_scope'}
        }
        $outputs=$map['outputs']
        if($outputs.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $outputs.GetArrayLength() -ne 3){Stop-C2Q 'allocation_schema'}
        $names=@('started.json','witness.json','watchdog.json')
        for($n=0;$n -lt 3;$n++){if($outputs[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $outputs[$n].GetString() -cne $names[$n]){Stop-C2Q 'allocation_scope'}}
        $pins=[ordered]@{}
        foreach($name in @('preflight','watchdog','core','root_native','deadline','shell')){
            $value=$map[$name+'_sha256'].GetString()
            if($value -cnotmatch '\A[0-9a-f]{64}\z'){Stop-C2Q 'allocation_schema'};$pins[$name]=$value
        }
        if($pins.core -cne 'e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e' -or
           $pins.root_native -cne 'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382' -or
           $pins.deadline -cne '0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45' -or
           $pins.shell -cne '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'){Stop-C2Q 'source_pin'}
        $pf=Open-Public $preflight 131072;$wf=Open-Public $watchdog 131072
        if((Hash-Stream $pf) -cne $pins.preflight -or (Hash-Stream $wf) -cne $pins.watchdog){Stop-C2Q 'source_pin'}
        $sourceFiles=[ordered]@{}
        $sourceFiles.core=Pin $core 80889 $pins.core
        $sourceFiles.root_native=Pin $rootNative 25596 $pins.root_native
        $sourceFiles.deadline=Pin $deadline 6038 $pins.deadline
        $null=Pin $shell 301368 $pins.shell
        return [pscustomobject]@{Pins=$pins;SourceFiles=$sourceFiles}
    }finally{$doc.Dispose()}
}

function New-PublicOutput([string]$Suffix) {
    if($Suffix -cnotin @('started.json','witness.json','watchdog.json')){Stop-C2Q 'output_cap'}
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}

function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2Q 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}

function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}

function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2Q $Code}}

function Case-Specs {
    return @([pscustomobject]@{Name='blocked-scan';Selector='deadline-blocked-scan-r02';InventoryBytes=3586
        InventoryHash='50a6edd1e45a0fb2d0e272d4d6ba10114e223e4b2ef42a81b4bb3f6b08c9799b'
        Acquired=10;FileCount=13;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5'
        Phases=@('locks','inventory','capture_s','capture_bkp','scan_before')})
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

function Assert-CoreSnapshot($o,$s) {
    $expected=[ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$false;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$false;input_handles_released=$false
        cleanup_unknown=$false;receipts_completed=$false;cleanup_native_error=0;failure_code=$null;failure_phase=$null;failure_role=$null
        cause_kind=$null;native_error=0;error_hresult=0;expected_files=5;expected_bytes=52;locks=6;hash_passes=0;hash_bytes=0
        historical_tuple_digest=$s.H;s_before_tuple_digest=$null;bkp_before_tuple_digest=$null;s_after_tuple_digest=$null;bkp_after_tuple_digest=$null
        peak_owned_handles=33;remaining_owned_handles=32;peak_path_bytes=7024;peak_record_bytes=12086928;peak_inventory_bytes=3586;scratch_reserved_bytes=524288
        receipt_bytes_charged=0;receipt_bytes_known_written=0;receipt_write_unknown=$false}
    Assert-Fields $o $expected $false;Need (@(Object-Keys $o).Count -eq 38)
    Need (($o.capture_complete_timestamp -is [long] -or $o.capture_complete_timestamp -is [int]) -and $o.capture_complete_timestamp -gt 0)
    $keys=@('allocation','locks','inventory','capture_s','capture_bkp')
    Need ((@(Object-Keys $o.phase_seconds) -join '|') -ceq ($keys -join '|'))
    foreach($k in $keys){$v=Get-Value $o.phase_seconds $k;Need (($v -is [double] -or $v -is [long] -or $v -is [int] -or $v -is [decimal]) -and [double]::IsFinite([double]$v) -and $v -ge 0 -and $v -lt 20)}
    Need (@(Object-Keys $o.private_receipt_digests).Count -eq 0)
}

function Assert-StreamSnapshot($v,[int]$Directories,[int]$Files) {
    $count=$Directories+$Files
    $expected=[ordered]@{first_calls=$count;next_calls=$Files;opened=$Files;closed=$Files;remaining=0;peak=if($Files -gt 0){1}else{0};
        checks=$count;directories=$Directories;files=$Files;failure_code=$null;native_error=0;cleanup_unknown=$false;cleanup_native_error=0}
    Assert-Fields $v $expected $true
}

function Assert-PremiseSnapshot($p,$s,[bool]$Prepared) {
    $expected=[ordered]@{ready=$true;healthy_physical_layout_verified=$true;bounded_entries=27
        directories=15;files=13;physical_file_bytes=(104+$s.InventoryBytes)
        s_payload_files=5;s_payload_bytes=52;bkp_payload_files=5;bkp_payload_bytes=52;empty_closed_locks=6
        target_object_verified=$true;canonical_names=$true;protected_exact_acl=$true;normal_objects_strict=$true
        streams_verified=28;strict_streams_verified=28
        named_stream_bytes=0;streams_unsupported_accepted=$false;setup_writers_closed=$true
        expected_files_spec=5;expected_bytes_spec=52;healthy_dummy_inventory_premises_verified=$true;core_inventory_not_executed_during_premises=$true
        inventory_bytes=$s.InventoryBytes;inventory_sha256=$s.InventoryHash}
    Assert-Fields $p $expected $true
}

function Assert-SetupSnapshot($a,$s) {
    $expected=[ordered]@{case_created=$true;directories_created=15;file_create_attempts=13;file_create_returned=13;files_validated=13
        file_streams_open=0;file_streams_closed=13;read_streams_open=0;read_streams_closed=26;partial_file_creation_unknown=$false
        tracked_file_results=39;tracked_file_returns=39;tracked_file_close_attempts=39;tracked_file_results_closed=39
        tracked_file_results_remaining=0;partial_file_return_unknown=$false
        namespace_enumerator_results=30;namespace_enumerator_returns=30;namespace_enumerator_close_attempts=30;namespace_enumerators_disposed=30
        namespace_enumerators_remaining=0;partial_namespace_return_unknown=$false}
    Assert-Fields $a $expected $true
}

function Assert-GuardHandoff($o,$s) {
    Assert-Fields $o ([ordered]@{returned=15;close_attempts=15;closed=15;remaining=0;current_slots=0;cleanup_unknown=$false;before_factory=$true}) $true
}

function New-Drain([IO.StreamReader]$Reader) {
    $buffer=[char[]]::new(1024)
    return [pscustomobject]@{Reader=$Reader;Buffer=$buffer;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($buffer,0,$buffer.Length);Eof=$false;Cap=$false}
}

function Pump($Drain) {
    if(-not $Drain -or $Drain.Eof -or -not $Drain.Task.IsCompleted){return}
    $n=$Drain.Task.GetAwaiter().GetResult()
    if($n -eq 0){$Drain.Eof=$true;return}
    if($Drain.Text.Length+$n -gt 4096){$Drain.Cap=$true;Stop-C2Q 'output_cap'}
    $null=$Drain.Text.Append($Drain.Buffer,0,$n)
    $Drain.Task=$Drain.Reader.ReadAsync($Drain.Buffer,0,$Drain.Buffer.Length)
}

function Cause($Exception) {
    $last=$Exception;$native=0;$marks=[ordered]@{};$depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++;$last=$at
        if($at -is [ComponentModel.Win32Exception]){$native=$at.NativeErrorCode}
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2q_code','c2q_fixture_code','c2q_fixture_operation')){
            $value=[string]$at.Data[$key]
            if($value -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$value}
        }
    }
    $type=$last.GetType().FullName
    if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks}
}

function Acquire-FixedEvents([bool]$Create) {
    Need ($eventOwners.Count -eq 0) 'event_scope'
    foreach($role in @('ready','block')){
        Check
        $slot=[pscustomobject]@{Role=$role;Object=$null;Safe=$null;Attempted=$false;Returned=$false;CreatedNew=$null;CloseAttempted=$false;Closed=$false}
        $eventOwners.Add($slot)
        try{
            $slot.Attempted=$true
            $name=if($role -ceq 'ready'){'Local\Kilogram.S53.C2Q01a02.ready'}else{'Local\Kilogram.S53.C2Q01a02.block'}
            if($Create){$created=$false;$slot.Object=[Threading.EventWaitHandle]::new($false,[Threading.EventResetMode]::ManualReset,$name,[ref]$created);$slot.CreatedNew=$created}
            else{$slot.Object=[Threading.EventWaitHandle]::OpenExisting($name)}
            $slot.Returned=$true;$slot.Safe=$slot.Object.SafeWaitHandle
            Need ($null -ne $slot.Safe -and -not $slot.Safe.IsClosed -and -not $slot.Safe.IsInvalid) 'event_scope'
            if($Create){Need $slot.CreatedNew 'event_collision'}
        }catch{
            # The slot precedes constructor/getter/validation, including a returned collision reference.
            $null=Close-FixedEvents
            throw
        }
    }
    Need (-not $eventOwners[0].Object.WaitOne(0) -and -not $eventOwners[1].Object.WaitOne(0)) 'event_scope'
}
function Close-FixedEvents {
    foreach($slot in $eventOwners){
        if($slot.CloseAttempted){continue}
        if(-not $slot.Returned -or $null -eq $slot.Object){if($slot.Attempted){$script:eventCleanupUnknown=$true};continue}
        $slot.CloseAttempted=$true
        try{$slot.Object.Dispose();if($slot.Safe -and $slot.Safe.IsClosed){$slot.Closed=$true}else{$script:eventCleanupUnknown=$true}}
        catch{$script:eventCleanupUnknown=$true;if(-not $script:eventCleanupFirstCause){$script:eventCleanupFirstCause=Cause $_.Exception}}
    }
    return (-not $eventCleanupUnknown -and @($eventOwners|Where-Object {-not $_.Closed}).Count -eq 0)
}
function Event-Facts {
    return [ordered]@{slots=$eventOwners.Count;returned=@($eventOwners|Where-Object Returned).Count
        close_attempts=@($eventOwners|Where-Object CloseAttempted).Count;closed=@($eventOwners|Where-Object Closed).Count
        remaining=@($eventOwners|Where-Object {$_.Returned -and -not $_.Closed}).Count;cleanup_unknown=$eventCleanupUnknown
        records=@($eventOwners|ForEach-Object {[ordered]@{role=$_.Role;returned=$_.Returned;created_new=$_.CreatedNew;close_attempted=$_.CloseAttempted;closed=$_.Closed}})}
}
function Assert-EventFacts($f,[bool]$Parent,[bool]$Closed) {
    Assert-Fields $f ([ordered]@{slots=2;returned=2;close_attempts=if($Closed){2}else{0};closed=if($Closed){2}else{0}
        remaining=if($Closed){0}else{2};cleanup_unknown=$false}) $false
    Need (@(Object-Keys $f).Count -eq 7 -and $f.records.Count -eq 2)
    for($i=0;$i -lt 2;$i++){
        Assert-Fields $f.records[$i] ([ordered]@{role=if($i -eq 0){'ready'}else{'block'};returned=$true
            created_new=if($Parent){$true}else{$null};close_attempted=$Closed;closed=$Closed}) $true
    }
}

function Assert-Witness($v,$s,$Pins,[int]$ChildPid,[string]$ChildStartUtc,[int]$ParentPid) {
    Assert-Fields $v ([ordered]@{schema_version=1;classification='cold-core-blocked-worker-witness-r01';attempt=$attempt
        allocation_sha256=$AllocationSha256;case=$s.Name;selector=$s.Selector;phase='scan_before'
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
        supervisor_pid=$ParentPid;worker_pid=$ChildPid;worker_started_utc=$ChildStartUtc;worker_process_path=$shell;apartment='MTA'
        acquired_files=10;factory_returned=$true;capture_invoked=$true;case_guards_closed=$true
        worker_public_inputs_owned=8;family_guards_owned=5;family_guard_cleanup_unknown=$false;job_work_expired=$false;compiler_error=$false;diagnostic_cap=$false
        output_owner_bound=$false;proof_created=$false;receipt_count=0
        write_protocol='flush_close_before_signal';wait_protocol='SignalAndWait_ready_never_signaled_block'
        private_dataset_access=$false;whole_attempt_verdict='not_established'}) $false
    Need (@(Object-Keys $v).Count -eq 50)
    Need (($v.witness_qpc -is [long] -or $v.witness_qpc -is [int]) -and $v.witness_qpc -gt $StartTimestamp -and
        $v.witness_qpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency))
    Need ($v.job_children_before_block.Count -eq 0 -and $v.diagnostics.Count -eq 0)
    Need (@(Object-Keys $v.pins).Count -eq 6)
    foreach($k in @('preflight','watchdog','core','root_native','deadline','shell')){Need ((Get-Value $v.pins $k) -ceq $Pins[$k]) 'source_pin'}
    foreach($n in @('source_pins_before','source_pins_at_witness')){
        $f=Get-Value $v $n;Need (@(Object-Keys $f).Count -eq 3)
        foreach($k in @('core','root_native','deadline')){Need ((Get-Value $f $k) -ceq $Pins[$k]) 'source_pin'}
    }
    Assert-Fields $v.fresh_types_absent_before ([ordered]@{deadline=$true;core=$true;root_native=$true;premise_streams=$true}) $true
    Assert-Fields $v.initialization_calls ([ordered]@{'Initialize-S53ColdPreservationCore'=1;'Initialize-S53RootNative'=1;'Initialize-C2QPremises'=1}) $true
    Assert-SetupSnapshot $v.setup $s;Assert-GuardHandoff $v.guard_handoff $s
    Assert-Fields $v.premises ([ordered]@{ready=$true;directories=15;files=13;physical_file_bytes=3690;named_stream_bytes=0;streams_verified=56}) $false
    Assert-PremiseSnapshot $v.premises.baseline $s $false;Assert-PremiseSnapshot $v.premises.prepared $s $true
    Assert-StreamSnapshot $v.baseline_stream_check 15 13;Assert-StreamSnapshot $v.prepared_stream_check 15 13
    Assert-EventFacts $v.worker_events $false $false
    Assert-Fields $v.directory_guard_ledger ([ordered]@{returned=20;close_attempts=15;closed=15;remaining=5;current_slots=0;cleanup_unknown=$false}) $true
    Assert-CoreSnapshot $v.last_core_observation $s
    $times=@($v.setup_started_qpc,$v.factory_after_qpc,$v.last_core_observation.capture_complete_timestamp,$v.witness_qpc)
    $last=$StartTimestamp;foreach($q in $times){Need (($q -is [long] -or $q -is [int]) -and $q -gt $last -and $q -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency));$last=$q}
    Need ($v.hook_events.Count -eq 5);$last=$v.factory_after_qpc
    for($i=0;$i -lt 5;$i++){$e=$v.hook_events[$i];Assert-Fields $e ([ordered]@{name=$s.Phases[$i]}) $false
        Need (@(Object-Keys $e).Count -eq 2 -and ($e.qpc -is [long] -or $e.qpc -is [int]) -and $e.qpc -ge $last -and $e.qpc -le $v.witness_qpc);$last=$e.qpc}
    Need ($v.last_core_observation.capture_complete_timestamp -le $v.hook_events[4].qpc)
}

$self=[Diagnostics.Process]::GetCurrentProcess()
$self.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$startup=([DateTime]::UtcNow-$self.StartTime.ToUniversalTime()).TotalSeconds
$StartTimestamp=[Diagnostics.Stopwatch]::GetTimestamp()-[long]($startup*[Diagnostics.Stopwatch]::Frequency)
$done=[Threading.ManualResetEvent]::new($false)
$control=[hashtable]::Synchronized(@{Child=$null;Forced=$false;Unknown=$false})
# R40 pattern: armed before filesystem validation, helper load or compiler setup.
$guardian=[powershell]::Create()
$null=$guardian.AddScript({
    param($Owner,$Control,$Done,[long]$Start)
    try {
        while(-not $Done.WaitOne(20)){
            $age=([Diagnostics.Stopwatch]::GetTimestamp()-$Start)/[double][Diagnostics.Stopwatch]::Frequency
            if($age -ge 27){
                $Control.Forced=$true;$p=$Control.Child
                if($p){try{if(-not $p.HasExited){$p.Kill()};$null=$p.WaitForExit(20)}catch{$Control.Unknown=$true}}
            }
            if($age -ge 29){$Owner.Kill();return}
        }
    }catch{try{$Owner.Kill()}catch{}}
}).AddArgument($self).AddArgument($control).AddArgument($done).AddArgument($StartTimestamp)
$ticket=$guardian.BeginInvoke()

$child=$null;$binding=$null;$claimed=$false;$authorized=$false;$result=1
$out=$null;$err=$null;$reaped=$false;$managedDisposed=$false;$stdioDisposed=$false
$childExit=$null;$childId=0;$childStartedUtc=$null;$retainedProcessHandle=0L;$registeredChildReleased=$false;$publicInputsBeforeClose=0
$readySeen=$false;$witnessValid=$false;$witness=$null;$witnessLength=0;$witnessSha=$null
$waitLoopEntered=$false;$primaryReason='not_started';$primaryCause=$null;$cleanupFirstCause=$null;$cutoffObserved=$false
$aliveBeforeKill=$false;$killAttempted=$false;$killReturned=$false;$workerForced=$false;$cleanupKillAttempted=$false
$readyQpc=0L;$witnessAcceptedQpc=0L;$cutoffQpc=0L;$killBeforeQpc=0L;$killAfterQpc=0L;$exitQpc=0L;$eofQpc=0L
$eventsClosed=$false;$parentEventsBefore=$null;$parentEventsAfter=$null;$sourceAfter=[ordered]@{}
try {
    if($PSCommandPath -cne $watchdog){Stop-C2Q 'allocation_scope'}
    $binding=Validate-Allocation;$authorized=$true
    foreach($suffix in @('started.json','witness.json','watchdog.json')){
        if([IO.File]::Exists("$e\$attempt.$suffix") -or [IO.Directory]::Exists("$e\$attempt.$suffix")){Stop-C2Q 'output_collision'}
    }
    if((Age) -ge 18){Stop-C2Q 'bootstrap_reserve'}
    $marker=New-PublicOutput 'started.json';$claimed=$true
    try{Write-Public $marker ([ordered]@{classification='cold-core-blocked-worker-c2-start-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;supervisor_pid=$self.Id
        native_started_utc=$self.StartTime.ToUniversalTime().ToString('o');permission_consumed=$true
        work_seconds=20;whole_seconds=30;lane_count=1;retry_count=0;core_sha256=$binding.Pins.core;pins=$binding.Pins;whole_attempt_verdict='not_established'})}
    finally{$marker.Dispose()}
    Acquire-FixedEvents $true;$parentEventsBefore=Event-Facts;Assert-EventFacts $parentEventsBefore $true $false
    $info=[Diagnostics.ProcessStartInfo]::new($shell);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WorkingDirectory=$w
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($argValue in @('-NoLogo','-NoProfile','-NonInteractive','-Mta','-File',$preflight,'-AllocationSha256',$AllocationSha256,
        '-StartTimestamp',$StartTimestamp.ToString([Globalization.CultureInfo]::InvariantCulture))){$info.ArgumentList.Add($argValue)}
    $child=[Diagnostics.Process]::new();$child.StartInfo=$info;$control.Child=$child
    Check;if(-not $child.Start()){Stop-C2Q 'child_return'}
    $childId=$child.Id;$retainedProcessHandle=$child.Handle.ToInt64();$childStartedUtc=$child.StartTime.ToUniversalTime().ToString('o')
    Need ($childId -gt 0 -and $retainedProcessHandle -gt 0) 'child_identity'
    $child.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $out=New-Drain $child.StandardOutput;$err=New-Drain $child.StandardError
    $waitLoopEntered=$true
    while($true){
        Check;Pump $out;Pump $err
        if($child.HasExited){Stop-C2Q 'premature_exit'}
        if(-not $readySeen -and $eventOwners[0].Object.WaitOne(0)){
            $readyQpc=[Diagnostics.Stopwatch]::GetTimestamp();$readySeen=$true
            Need (-not $child.HasExited -and $child.Id -eq $childId -and $child.Handle.ToInt64() -eq $retainedProcessHandle -and
                $child.StartTime.ToUniversalTime().ToString('o') -ceq $childStartedUtc) 'child_identity'
            $wf=Open-Public "$e\$attempt.witness.json" 65536
            $witnessBytes=Bytes $wf 65536;$witnessLength=$witnessBytes.Length;$witnessSha=Hash-Stream $wf
            $witness=[Text.UTF8Encoding]::new($false,$true).GetString($witnessBytes)|ConvertFrom-Json -Depth 12 -DateKind String
            Assert-Witness $witness (Case-Specs)[0] $binding.Pins $childId $childStartedUtc $self.Id
            Check;Need (-not $child.HasExited) 'premature_exit'
            $witnessAcceptedQpc=[Diagnostics.Stopwatch]::GetTimestamp();$witnessValid=$true
        }
        $null=$child.WaitForExit(20)
    }
}catch{
    $primaryReason=Failure-Code $_.Exception;$primaryCause=Cause $_.Exception
    if($primaryReason -ceq 'work_expired'){$cutoffQpc=[Diagnostics.Stopwatch]::GetTimestamp();$cutoffObserved=$true}
}
finally {
    # Everything below is cleanup/report work, never a fresh 20-second work interval.
    $cleanupReserve=$true
    if($child){
        if($cutoffObserved -and $waitLoopEntered -and $witnessValid -and $readySeen){
            try{
                $aliveBeforeKill=-not $child.HasExited
                if($aliveBeforeKill -and $child.Id -eq $childId -and $child.Handle.ToInt64() -eq $retainedProcessHandle -and
                    $child.StartTime.ToUniversalTime().ToString('o') -ceq $childStartedUtc -and (Age) -lt 26.5){
                    $killBeforeQpc=[Diagnostics.Stopwatch]::GetTimestamp();$workerForced=$true;$killAttempted=$true
                    $child.Kill();$killReturned=$true;$killAfterQpc=[Diagnostics.Stopwatch]::GetTimestamp()
                }
            }catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        # Preserve existing bounded Kill/reap cleanup on all paths; it cannot establish the primary oracle.
        while((Age) -lt 26.5 -and -not $reaped){
            try{
                if(-not $child.HasExited){$cleanupKillAttempted=$true;$child.Kill()}
                $reaped=$child.WaitForExit(20);if($reaped -and $exitQpc -eq 0){$exitQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
            }catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
            try{Pump $out;Pump $err}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
            if(-not $reaped){[Threading.Thread]::Sleep(10)}
        }
        if(-not $reaped){$control.Unknown=$true}
        while((Age) -lt 27.5 -and (($out -and -not $out.Eof) -or ($err -and -not $err.Eof))){
            try{Pump $out;Pump $err}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception};break}
            [Threading.Thread]::Sleep(5)
        }
        if($out -and $err -and $out.Eof -and $err.Eof){$eofQpc=[Diagnostics.Stopwatch]::GetTimestamp()}else{$control.Unknown=$true}
        if($reaped){try{$childExit=$child.ExitCode}catch{$control.Unknown=$true}}
    }
    try{
        if($binding){foreach($k in @('core','root_native','deadline')){$sourceAfter[$k]=Hash-Stream $binding.SourceFiles[$k];Need ($sourceAfter[$k] -ceq $binding.Pins[$k]) 'source_pin'}}
    }catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    $eventsClosed=Close-FixedEvents;$parentEventsAfter=Event-Facts
    if(-not $eventsClosed){$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=$eventCleanupFirstCause}}
    $publicInputsBeforeClose=$held.Count;$managedDisposed=Close-PublicInputs;if(-not $managedDisposed){$control.Unknown=$true}
    # Guardian remains armed during pipe/event/input cleanup.
    try{if($out){$out.Reader.Dispose()};if($err){$err.Reader.Dispose()};$stdioDisposed=$true}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    if($child -and $reaped){try{$control.Child=$null;$child.Dispose();$child=$null;$registeredChildReleased=$true}catch{$control.Unknown=$true}}
}
try {
    $negativeObserved=$cutoffObserved -and $witnessValid -and $readySeen -and $aliveBeforeKill -and $killAttempted -and $killReturned -and
        $reaped -and $null -ne $childExit -and $childExit -ne 0 -and $registeredChildReleased -and $eventsClosed -and $managedDisposed -and $publicInputsBeforeClose -eq 8 -and $stdioDisposed -and
        $out.Eof -and $err.Eof -and $out.Text.Length -eq 0 -and $err.Text.Length -eq 0 -and -not $out.Cap -and -not $err.Cap -and
        -not $control.Unknown -and -not $control.Forced
    if($negativeObserved){
        Assert-EventFacts $parentEventsAfter $true $true
        Need ($readyQpc -gt $witness.witness_qpc -and $witnessAcceptedQpc -ge $readyQpc -and
            $witnessAcceptedQpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency) -and
            $cutoffQpc -ge ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency) -and
            $killBeforeQpc -ge $cutoffQpc -and $killAfterQpc -ge $killBeforeQpc -and $exitQpc -ge $killAfterQpc -and
            $exitQpc -lt ($StartTimestamp+[long](26.5*[Diagnostics.Stopwatch]::Frequency)) -and
            $eofQpc -ge $exitQpc -and $eofQpc -lt ($StartTimestamp+[long](27.5*[Diagnostics.Stopwatch]::Frequency))) 'assertion'
        Need ($primaryReason -ceq 'work_expired' -and $primaryCause.marks.c2q_code -ceq 'work_expired' -and
            $primaryCause.type -ceq 'System.InvalidOperationException' -and $primaryCause.hresult -eq -2146233079) 'assertion'
    }
    Check
    if($authorized -and $claimed){
        $file=New-PublicOutput 'watchdog.json'
        try{Write-Public $file ([ordered]@{classification='cold-core-blocked-worker-c2-watchdog-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
            result=if($negativeObserved){'observed_expected_forced_timeout'}else{'failed_or_unknown'};primary_reason=$primaryReason;primary_cause=$primaryCause
            primary_mechanism=if($negativeObserved){'supervisor_work_cutoff'}else{$null};cleanup_first_cause=$cleanupFirstCause
            reached_witness_valid=$witnessValid;ready_seen=$readySeen;child_alive_before_kill=$aliveBeforeKill;kill_attempted=$killAttempted;kill_returned=$killReturned
            forced=($workerForced -or $control.Forced -or $cleanupKillAttempted);guardian_forced=$control.Forced;cleanup_kill_attempted=$cleanupKillAttempted
            child_pid=$childId;child_started_utc=$childStartedUtc;retained_process_handle=$retainedProcessHandle;child_exit=$childExit
            child_reaped=$reaped;registered_child_released=$registeredChildReleased;stdout_eof=if($out){$out.Eof}else{$false};stderr_eof=if($err){$err.Eof}else{$false}
            stdout=if($out){$out.Text.ToString()}else{''};stderr=if($err){$err.Text.ToString()}else{''};output_cap=($out -and $out.Cap) -or ($err -and $err.Cap)
            stdio_disposed=$stdioDisposed;managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose;supervisor_cleanup_unknown=$control.Unknown;parent_events_before=$parentEventsBefore;parent_events_after=$parentEventsAfter
            observed_owned_child_processes_after=if($reaped -and $registeredChildReleased){0}else{$null}
            owned_process_absence_basis='witness job children0; no spawn path after witness; retained worker exit/reap/EOF'
            worker_final_core_observation=$null;worker_finally_completion_observed=$false;worker_close_count=$null;worker_application_cleanup_state=if($workerForced -or $control.Forced -or $cleanupKillAttempted){'not_observed_forced_exit'}else{'not_established'}
            os_release_basis=if($reaped){'observed process exit and Windows handle-release semantics; not per-handle instrumentation'}else{'not_established'}
            hard_fallback_branch_exercised=if($negativeObserved){$false}elseif($control.Forced){$true}else{$null};cooperative_worker_timeout_observed=$false
            witness_length=$witnessLength;witness_sha256=$witnessSha;last_core_observation=if($witness){$witness.last_core_observation}else{$null}
            ready_qpc=$readyQpc;witness_accepted_qpc=$witnessAcceptedQpc;cutoff_qpc=$cutoffQpc;kill_before_qpc=$killBeforeQpc;kill_after_qpc=$killAfterQpc
            child_exit_observed_qpc=$exitQpc;both_eof_qpc=$eofQpc;start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
            pins=$binding.Pins;source_pins_after=$sourceAfter;work_seconds=20;whole_seconds=30;lane_count=1;retry_count=0
            observed_seconds=(Age);whole_attempt_verdict='not_established';private_dataset_access=$false})}
        finally{$file.Dispose()}
        if($negativeObserved){$result=0}
    }
    Check;if($control.Unknown -or $control.Forced){$result=1}
    if($control.Child -and -not $reaped){while((Age) -lt 29){[Threading.Thread]::Sleep(20)};$self.Kill();exit 1}
    $null=$done.Set()
    if(-not $ticket.AsyncWaitHandle.WaitOne(300)){Stop-C2Q 'cleanup_unknown'}
    $null=$guardian.EndInvoke($ticket)
    if($control.Unknown -or $control.Forced -or (Age) -ge 28){$result=1}
}catch{$result=1}
finally {if($ticket.IsCompleted){$guardian.Dispose();$done.Dispose();if($child){$child.Dispose()};$self.Dispose()}}
if($result -eq 0){[Console]::Out.WriteLine('ColdA blocked-worker expected external cutoff returned; root actual-return review required.')}
exit $result
