[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256)
# C2-N native read errors: source/static only; no runtime allocation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-native-read-errors-01-attempt-01'
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
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Stop-C2N([string]$Code) {
    $err=[InvalidOperationException]::new('C2N bounded refusal');$err.Data['c2n_code']=$Code;throw $err
}
function Check {
    $age=Age
    if($age -lt 0 -or $age -ge 20){Stop-C2N 'work_expired'}
    if($guard){$guard.CheckWork()}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        $value=[string]$at.Data['c2n_code']
        if($value -in @('work_expired','public_input','allocation_pin','allocation_schema','allocation_scope','source_pin',
            'output_collision','output_cap','start_claim','bootstrap_reserve','job_setup','core_type_cached','definitions_shape',
            'diagnostic_cap','compiler_error','signature_mismatch','cleanup_unknown','child_return','assertion',
            'fixture_collision','fixture_setup','fixture_hash','premise_failure','typed_refusal','case_cleanup')){return $value}
        if($at.Data.Contains('colda_code')){return 'core_failure'}
    }
    return 'failed_or_unknown'
}
function Open-Public([string]$Path,[long]$Maximum) {
    Check
    if(-not [IO.Path]::IsPathFullyQualified($Path) -or
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2N 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2N 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2N 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2N 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2N 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2N 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2N 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2N 'source_pin'}
    return $f
}
function Validate-Allocation {
    Check
    if(-not $IsWindows -or [Environment]::ProcessPath -cne $shell){Stop-C2N 'allocation_scope'}
    $af=Open-Public $allocation 65536
    if((Hash-Stream $af) -cne $AllocationSha256){Stop-C2N 'allocation_pin'}
    $raw=Bytes $af 65536
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    if($text.Length -eq 0 -or $text[0] -eq [char]0xfeff){Stop-C2N 'allocation_schema'}
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        if($doc.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object){Stop-C2N 'allocation_schema'}
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($property in $doc.RootElement.EnumerateObject()){
            if($property.Name -cnotin $fields -or -not $map.TryAdd($property.Name,$property.Value)){Stop-C2N 'allocation_schema'}
        }
        if($map.Count -ne 22){Stop-C2N 'allocation_schema'}
        foreach($name in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256')){
            if($map[$name].ValueKind -ne [Text.Json.JsonValueKind]::String){Stop-C2N 'allocation_schema'}
        }
        if($map['schema_version'].GetInt32() -ne 1 -or $map['classification'].GetString() -cne 'root-approved-cold-core-native-read-errors-c2-r01' -or
           $map['root_thread_id'].GetString() -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or
           $map['activated'].ValueKind -ne [Text.Json.JsonValueKind]::True -or $map['attempt'].GetString() -cne $attempt -or
           $map['operation_set'].GetString() -cne 'cold-native-read-errors-two-only-r01' -or $map['workspace'].GetString() -cne $w -or
           $map['fixture_parent'].GetString() -cne ($w+'\.tmp\C-s53-cold-core-native-read-errors-01-attempt-01') -or
           $map['work_seconds'].GetInt32() -ne 20 -or $map['whole_seconds'].GetInt32() -ne 30 -or
           $map['retry_count'].GetInt32() -ne 0 -or $map['lane_count'].GetInt32() -ne 1 -or
           $map['public_report_bytes'].GetInt32() -ne 65536 -or $map['public_total_bytes'].GetInt32() -ne 196608){Stop-C2N 'allocation_scope'}
        $cases=$map['fixture_cases'];$fixed=@((Case-Specs).Selector)
        if($cases.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $cases.GetArrayLength() -ne 2){Stop-C2N 'allocation_scope'}
        for($n=0;$n -lt 2;$n++){
            if($cases[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $cases[$n].GetString() -cne $fixed[$n]){Stop-C2N 'allocation_scope'}
        }
        $outputs=$map['outputs']
        if($outputs.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $outputs.GetArrayLength() -ne 3){Stop-C2N 'allocation_schema'}
        $names=@('started.json','cases.json','watchdog.json')
        for($n=0;$n -lt 3;$n++){if($outputs[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $outputs[$n].GetString() -cne $names[$n]){Stop-C2N 'allocation_scope'}}
        $pins=[ordered]@{}
        foreach($name in @('preflight','watchdog','core','root_native','deadline','shell')){
            $value=$map[$name+'_sha256'].GetString()
            if($value -cnotmatch '\A[0-9a-f]{64}\z'){Stop-C2N 'allocation_schema'};$pins[$name]=$value
        }
        if($pins.core -cne '9b4a7d6165c11d82ff122ca6c13f8876d616b711424e7355e5508e73c7a2cf06' -or
           $pins.root_native -cne 'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382' -or
           $pins.deadline -cne '0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45' -or
           $pins.shell -cne '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'){Stop-C2N 'source_pin'}
        $pf=Open-Public $preflight 131072;$wf=Open-Public $watchdog 131072
        if((Hash-Stream $pf) -cne $pins.preflight -or (Hash-Stream $wf) -cne $pins.watchdog){Stop-C2N 'source_pin'}
        $sourceFiles=[ordered]@{}
        $sourceFiles.core=Pin $core 79451 $pins.core
        $sourceFiles.root_native=Pin $rootNative 25596 $pins.root_native
        $sourceFiles.deadline=Pin $deadline 6038 $pins.deadline
        $null=Pin $shell 301368 $pins.shell
        return [pscustomobject]@{Pins=$pins;SourceFiles=$sourceFiles}
    }finally{$doc.Dispose()}
}
function New-PublicOutput([string]$Suffix) {
    if($Suffix -cnotin @('started.json','cases.json','watchdog.json')){Stop-C2N 'output_cap'}
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2N 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}
function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}

function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2N $Code}}
function Case-Specs {
    return @(
        [pscustomobject]@{Name='read-locked';Selector='native-read-locked';IsRead=$true;InventoryBytes=3598;InventoryHash='548a151083eed305ed3ea885fcf3ed772645d597ea010e6f7098c4687f62588f';FailureCode='input_busy';FailurePhase='hash_s_before';FailureRole='s';NativeError=33;PreHook='hash_s_before';Acquired=10;PreAcquired=10;PreOwned=33;PrePeak=35;PeakHandles=35;PreRecord=12087312;PeakRecord=12087312;PathPeak=7208;FileCount=13;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';Phases=@('locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before')},
        [pscustomobject]@{Name='streams-unsupported';Selector='native-streams-unsupported';IsRead=$false;InventoryBytes=3630;InventoryHash='480b2d04e399f6805a32aa96de50fb4b1a1593560202fddd6980c11775b5b8cb';FailureCode='streams_unsupported';FailurePhase='capture_s';FailureRole='s';NativeError=87;PreHook='capture_s';Acquired=2;PreAcquired=0;PreOwned=26;PrePeak=27;PeakHandles=29;PreRecord=12084624;PeakRecord=12085392;PathPeak=7488;FileCount=13;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';Phases=@('locks','inventory','capture_s')}
    )
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
function Assert-CoreSnapshot($o,$s,[bool]$Pre) {
    $expected=[ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$false;expected_set_captured_before_hash=$s.IsRead;observed_entry_sets_equal=$false;input_handles_released=(-not $Pre)
        cleanup_unknown=$false;receipts_completed=$false;cleanup_native_error=0
        failure_code=if($Pre){$null}else{$s.FailureCode};failure_phase=if($Pre){$null}else{$s.FailurePhase};failure_role=if($Pre){$null}else{$s.FailureRole}
        cause_kind=if($Pre){$null}else{'win32'};native_error=if($Pre){0}else{$s.NativeError};error_hresult=if($Pre){0}else{-2147467259}
        expected_files=5;expected_bytes=52;locks=6;hash_passes=0;hash_bytes=if(-not $Pre -and $s.IsRead){23}else{0}
        historical_tuple_digest=$s.H;s_before_tuple_digest=$null;bkp_before_tuple_digest=$null;s_after_tuple_digest=$null;bkp_after_tuple_digest=$null
        peak_owned_handles=if($Pre){$s.PrePeak}else{$s.PeakHandles};remaining_owned_handles=if($Pre){$s.PreOwned}else{0}
        peak_path_bytes=$s.PathPeak;peak_record_bytes=if($Pre){$s.PreRecord}else{$s.PeakRecord};peak_inventory_bytes=$s.InventoryBytes;scratch_reserved_bytes=524288
        receipt_bytes_charged=0;receipt_bytes_known_written=0;receipt_write_unknown=$false}
    Assert-Fields $o $expected $false;Need (@(Object-Keys $o).Count -eq 38)
    $capture=Get-Value $o 'capture_complete_timestamp';Need ($capture -is [long] -or $capture -is [int])
    if($s.IsRead){Need ($capture -gt 0)}else{Need ($capture -eq 0)}
    $keys=@('allocation')+@($s.Phases);if($Pre){$keys=@($keys[0..($keys.Count-2)])}
    $actualKeys=@(Object-Keys $o.phase_seconds);Need ($actualKeys.Count -eq $keys.Count -and ($actualKeys -join '|') -ceq ($keys -join '|'))
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
function Validate-CoreError($t,$cause,$s) {
    Assert-Fields $t ([ordered]@{matched=$true;code=$s.FailureCode;phase=$s.FailurePhase;role=$s.FailureRole;native_error=$s.NativeError;cause_kind='win32'}) $false
    Assert-Fields $t.normalized_marks ([ordered]@{colda_code=$s.FailureCode;colda_phase=$s.FailurePhase;colda_role=$s.FailureRole;colda_native_error=$s.NativeError}) $true
    Assert-Fields $t.original_marks ([ordered]@{colda_code=$s.FailureCode;colda_native_error=$s.NativeError}) $true
    Need ($t.chain.Count -eq 4)
    Assert-Fields $t.chain[0] ([ordered]@{type='System.Management.Automation.MethodInvocationException';hresult=-2146233087;native_error=0}) $true
    foreach($node in $t.chain[1..2]){Assert-Fields $node ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0}) $true}
    Assert-Fields $t.chain[3] ([ordered]@{type='System.ComponentModel.Win32Exception';hresult=-2147467259;native_error=$s.NativeError}) $true
    Assert-Fields $cause ([ordered]@{type='System.ComponentModel.Win32Exception';hresult=-2147467259;native_error=$s.NativeError}) $false
    Assert-Fields $cause.marks ([ordered]@{colda_code=$s.FailureCode;colda_phase=$s.FailurePhase;colda_role=$s.FailureRole}) $true
}
function Assert-LockSnapshot($h,[string]$Stage,[long]$Deadline) {
    Need ($Stage -cin @('baseline','locked','final'));$hook=$Stage -cne 'baseline';$final=$Stage -ceq 'final'
    Assert-Fields $h ([ordered]@{baseline_opens=1;baseline_verified=$true;baseline_closed=$true;hook_opens=if($hook){1}else{0}
        hook_verified=$hook;same_object_verified=$hook;lock_calls=if($hook){1}else{0};lock_returned=$hook;lock_succeeded=$hook;lock_error=0;lock_held=($hook -and -not $final)
        unlock_calls=if($final){1}else{0};unlock_returned=$final;unlock_succeeded=$final;unlock_error=0
        close_calls=if($final){2}else{1};closed_calls=if($final){2}else{1};remaining_owned_handles=if($Stage -ceq 'locked'){1}else{0};current_owned=$false
        info_calls=if($final){4}elseif($hook){3}else{1};acl_calls=if($final){4}elseif($hook){3}else{1};post_verified=$final
        cleanup_unknown=$false;cleanup_native_error=0;disposed=$final}) $false
    Need (@(Object-Keys $h).Count -eq 36)
    $keys=@('baseline_before_qpc','baseline_after_qpc')
    if($hook){$keys+=@('hook_before_qpc','lock_before_qpc','lock_after_qpc','hook_after_qpc')}
    if($final){$keys+=@('post_qpc','dispose_before_qpc','unlock_before_qpc','unlock_after_qpc','dispose_after_qpc')}
    $last=$StartTimestamp;foreach($k in $keys){$v=Get-Value $h $k;Need (($v -is [long] -or $v -is [int]) -and $v -ge $last -and $v -lt $Deadline);$last=$v}
    $all=@('baseline_before_qpc','baseline_after_qpc','hook_before_qpc','hook_after_qpc','lock_before_qpc','lock_after_qpc',
        'post_qpc','dispose_before_qpc','dispose_after_qpc','unlock_before_qpc','unlock_after_qpc')
    foreach($k in $all){if($k -cnotin $keys){Need ((Get-Value $h $k) -eq 0)}}
}



function Assert-CaseRecord($r,$s,[long]$Deadline) {
    Assert-Fields $r ([ordered]@{name=$s.Name;selector=$s.Selector;result='observed';failure=$null;failure_cause=$null;failure_stage=$null;cleanup_first_cause=$null
        core_factory_invoked=$true;core_capture_invoked=$true;core_dispose_attempted=$true;core_disposed=$true
        core_success_returned=$false;core_exception_observed=$true;pre_hook_seen=$true;pre_hook_acquired_events=$s.PreAcquired;hook_failure=$null
        controller_directories_closed=$true;remaining_controller_directories=0;unreleased_current_directories=0;controller_cleanup_unknown=$false
        acquired_files=$s.Acquired;output_owner_bound=$false;proof_created=$false;receipt_count=0}) $false
    Assert-SetupSnapshot $r.setup $s;Assert-GuardHandoff $r.guard_handoff $s
    Assert-PremiseSnapshot $r.premises.baseline $s $false;Assert-PremiseSnapshot $r.premises.prepared $s $true
    Assert-Fields $r.premises ([ordered]@{ready=$true;directories=15;files=13;physical_file_bytes=(104+$s.InventoryBytes);named_stream_bytes=0;streams_verified=56}) $false
    Assert-StreamSnapshot $r.baseline_stream_check 15 13;Assert-StreamSnapshot $r.prepared_strict_stream_check 15 13
    Assert-CoreSnapshot $r.pre_hook_core_observation $s $true
    Assert-CoreSnapshot $r.core_observation $s $false;Assert-CoreSnapshot $r.after_dispose_observation $s $false
    Need ($r.capture_qpc -eq $r.core_observation.capture_complete_timestamp -and $r.capture_qpc -eq $r.pre_hook_core_observation.capture_complete_timestamp -and
        $r.capture_qpc -eq $r.after_dispose_observation.capture_complete_timestamp)
    Validate-CoreError $r.core_refusal $r.primary_core_cause $s
    $times=@($r.setup_started_qpc,$r.baseline_complete_qpc,$r.prepared_complete_qpc,$r.guards_closed_qpc,$r.factory_before_qpc,$r.factory_after_qpc,
        $r.pre_hook_qpc,$r.core_failure_qpc,$r.core_disposed_qpc)
    $last=$StartTimestamp;foreach($t in $times){Need (($t -is [long] -or $t -is [int]) -and $t -ge $last -and $t -lt $Deadline);$last=$t}
    Need ($r.hook_events.Count -eq $s.Phases.Count);$last=$r.factory_after_qpc;$selected=0L
    for($i=0;$i -lt $s.Phases.Count;$i++){
        $e=$r.hook_events[$i];Need (@(Object-Keys $e).Count -eq 2 -and $e.name -ceq $s.Phases[$i] -and
            ($e.qpc -is [long] -or $e.qpc -is [int]) -and $e.qpc -ge $last -and $e.qpc -le $r.core_failure_qpc)
        $last=$e.qpc;if($e.name -ceq $s.PreHook){$selected=$e.qpc}
    }
    Need ($selected -gt 0 -and $r.pre_hook_qpc -ge $selected)
    if($s.IsRead){
        Assert-LockSnapshot $r.lock_baseline 'baseline' $Deadline;Assert-LockSnapshot $r.lock_at_hook 'locked' $Deadline;Assert-LockSnapshot $r.lock_final 'final' $Deadline
        Need ($r.lock_baseline.baseline_before_qpc -ge $r.prepared_complete_qpc -and $r.lock_baseline.baseline_after_qpc -le $r.guards_closed_qpc -and
            $r.lock_at_hook.baseline_before_qpc -eq $r.lock_baseline.baseline_before_qpc -and $r.lock_final.baseline_before_qpc -eq $r.lock_baseline.baseline_before_qpc -and
            $r.lock_at_hook.hook_before_qpc -ge $r.pre_hook_qpc -and $r.lock_at_hook.hook_after_qpc -le $r.core_failure_qpc -and
            $r.lock_final.hook_before_qpc -eq $r.lock_at_hook.hook_before_qpc -and $r.lock_final.lock_after_qpc -eq $r.lock_at_hook.lock_after_qpc -and
            $r.lock_final.post_qpc -ge $r.core_disposed_qpc -and $r.helper_disposed_qpc -ge $r.lock_final.dispose_after_qpc -and $r.helper_disposed_qpc -lt $Deadline)
        Need ($r.capture_qpc -gt $r.factory_after_qpc -and $r.capture_qpc -le $selected)
    }else{
        Need ($null -eq $r.lock_baseline -and $null -eq $r.lock_at_hook -and $null -eq $r.lock_final -and $r.helper_disposed_qpc -eq 0)
    }
}




function Validate-Cases($Report) {
    Need ($Report.case_count -eq 2 -and $Report.cases_observed -eq 2 -and $Report.cases.Count -eq 2 -and $Report.case_order.Count -eq 2) 'child_return'
    $specs=@(Case-Specs);for($i=0;$i -lt 2;$i++){Need ($Report.case_order[$i] -ceq $specs[$i].Name);Assert-CaseRecord $Report.cases[$i] $specs[$i] $Report.work_deadline_timestamp}
    Assert-Fields $Report.validated_geometry ([ordered]@{directories=31;files=26;stream_checks=113;logical_file_bytes=7436;named_stream_bytes=0;all_logical_stream_bytes=7436}) $true
    Assert-Fields $Report.directory_guard_ledger ([ordered]@{returned=35;close_attempts=35;closed=35;remaining=0;current_slots=0;cleanup_unknown=$false}) $true
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
$child=$null;$binding=$null;$claimed=$false;$authorized=$false;$result=1;$failure='not_started'
$out=$null;$err=$null;$reaped=$false;$managedDisposed=$false;$childReport=$null
$childExit=$null;$childId=$null;$stdioDisposed=$false
function New-Drain([IO.StreamReader]$Reader) {
    $buffer=[char[]]::new(1024)
    return [pscustomobject]@{Reader=$Reader;Buffer=$buffer;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($buffer,0,$buffer.Length);Eof=$false;Cap=$false}
}
function Pump($Drain) {
    if(-not $Drain -or $Drain.Eof -or -not $Drain.Task.IsCompleted){return}
    $n=$Drain.Task.GetAwaiter().GetResult()
    if($n -eq 0){$Drain.Eof=$true;return}
    if($Drain.Text.Length+$n -gt 4096){$Drain.Cap=$true;Stop-C2N 'output_cap'}
    $null=$Drain.Text.Append($Drain.Buffer,0,$n)
    $Drain.Task=$Drain.Reader.ReadAsync($Drain.Buffer,0,$Drain.Buffer.Length)
}
try {
    if($PSCommandPath -cne $watchdog){Stop-C2N 'allocation_scope'}
    $binding=Validate-Allocation;$authorized=$true
    foreach($suffix in @('started.json','cases.json','watchdog.json')){
        if([IO.File]::Exists("$e\$attempt.$suffix") -or [IO.Directory]::Exists("$e\$attempt.$suffix")){Stop-C2N 'output_collision'}
    }
    if((Age) -ge 18){Stop-C2N 'bootstrap_reserve'}
    $marker=New-PublicOutput 'started.json';$claimed=$true
    try {
        Write-Public $marker ([ordered]@{
            classification='cold-core-native-read-errors-c2-start-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
            start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
            supervisor_pid=$self.Id;native_started_utc=$self.StartTime.ToUniversalTime().ToString('o')
            permission_consumed=$true;work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1
            core_sha256=$binding.Pins.core;pins=$binding.Pins;whole_attempt_verdict='not_established'
        })
    }finally{$marker.Dispose()}
    Check
    $info=[Diagnostics.ProcessStartInfo]::new($shell)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WorkingDirectory=$w
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($argValue in @('-NoLogo','-NoProfile','-NonInteractive','-File',$preflight,'-AllocationSha256',$AllocationSha256,
        '-StartTimestamp',$StartTimestamp.ToString([Globalization.CultureInfo]::InvariantCulture))){$info.ArgumentList.Add($argValue)}
    $child=[Diagnostics.Process]::new();$child.StartInfo=$info;$control.Child=$child
    Check;if(-not $child.Start()){Stop-C2N 'child_return'}
    $childId=$child.Id
    $child.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $out=New-Drain $child.StandardOutput;$err=New-Drain $child.StandardError
    while($true){
        Check;Pump $out;Pump $err
        if($child.HasExited -and $out.Eof -and $err.Eof){break}
        $null=$child.WaitForExit(20)
    }
    if($child.ExitCode -ne 0 -or $control.Forced){Stop-C2N 'child_return'}
    $report=Open-Public "$e\$attempt.cases.json" 65536
    $childReport=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $report 65536))|ConvertFrom-Json
    if($childReport.classification -cne 'cold-core-native-read-errors-c2-r01' -or $childReport.attempt -cne $attempt -or
        $childReport.allocation_sha256 -cne $AllocationSha256 -or $childReport.result -cne 'observed' -or
        $childReport.assertions_passed -ne $true -or $null -ne $childReport.failure -or $null -ne $childReport.primary_failure -or
        $childReport.child_pid -ne $childId -or $childReport.start_timestamp -ne $StartTimestamp -or
        $childReport.started_supervisor_pid -ne $self.Id -or $childReport.stopwatch_frequency -ne [Diagnostics.Stopwatch]::Frequency -or
        $childReport.work_deadline_timestamp -ne ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency) -or
        $childReport.compiler_error -ne $false -or $childReport.diagnostic_cap -ne $false -or
        $childReport.synthetic_parent_created -ne $true -or $childReport.family_directories_created -ne 1 -or
        $childReport.family_ancestry_disposed -ne $true -or $childReport.family_remaining_handles -ne 0 -or
        $childReport.controller_cleanup_unknown -ne $false -or $childReport.unreleased_current_directories -ne 0 -or $childReport.fixture_retained_if_created -ne $true -or
        $childReport.output_owner_bound -ne $false -or $childReport.proof_created -ne $false -or $childReport.receipt_count -ne 0 -or
        $childReport.receipt_bytes -ne 0 -or @($childReport.receipt_digests.PSObject.Properties).Count -ne 0 -or
        $childReport.owned_children_after -ne 0 -or $childReport.managed_inputs_disposed -ne $true -or
        $childReport.job_finish_requires_actual_exit_zero -ne $true -or $childReport.priority -cne 'BelowNormal' -or
        $childReport.synthetic_fixture_access -ne $true -or $childReport.private_dataset_access -ne $false -or
        $childReport.native_false_close_path_tested -ne $false -or $childReport.whole_attempt_verdict -cne 'not_established' -or
        $childReport.work_seconds -ne 20 -or $childReport.whole_seconds -ne 30 -or $childReport.retry_count -ne 0 -or $childReport.lane_count -ne 1 -or
        $out.Text.Length -ne 0 -or $err.Text.Length -ne 0){Stop-C2N 'child_return'}
    $f=$childReport.family_stream_check
    Need ($f.first_calls -eq 1 -and $f.next_calls -eq 0 -and $f.opened -eq 0 -and $f.closed -eq 0 -and $f.remaining -eq 0 -and
        $f.peak -eq 0 -and $f.checks -eq 1 -and $f.directories -eq 1 -and $f.files -eq 0 -and
        $null -eq $f.failure_code -and $f.native_error -eq 0 -and $f.cleanup_unknown -eq $false -and $f.cleanup_native_error -eq 0) 'child_return'
    foreach($key in @('core','root_native','deadline')){
        if($childReport.source_pins_before.$key -cne $binding.Pins[$key] -or
            $childReport.source_pins_after.$key -cne $binding.Pins[$key]){Stop-C2N 'child_return'}
    }
    foreach($key in @('deadline','core','root_native','premise_streams','range_lock')){
        if($childReport.fresh_types_absent_before.$key -ne $true){Stop-C2N 'child_return'}
    }
    foreach($key in @('Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-C2NPremises')){
        if($childReport.initialization_calls.$key -ne 1){Stop-C2N 'child_return'}
    }
    Validate-Cases $childReport
    Check;$failure=$null;$result=0
}catch{$failure=Failure-Code $_.Exception}
finally {
    if($child){
        while((Age) -lt 26.5 -and -not $reaped) {
            try{if(-not $child.HasExited){$child.Kill()};$reaped=$child.WaitForExit(20)}catch{$control.Unknown=$true}
            try{Pump $out;Pump $err}catch{$result=1;$control.Unknown=$true}
            if(-not $reaped){[Threading.Thread]::Sleep(10)}
        }
        if(-not $reaped){$control.Unknown=$true}
        while((Age) -lt 27.5 -and (($out -and -not $out.Eof) -or ($err -and -not $err.Eof))){
            try{Pump $out;Pump $err}catch{$result=1;$control.Unknown=$true;break}
            [Threading.Thread]::Sleep(5)
        }
        if(($out -and -not $out.Eof) -or ($err -and -not $err.Eof)){$control.Unknown=$true}
    }
    $managedDisposed=Close-PublicInputs
    if(-not $managedDisposed){$control.Unknown=$true}
    # Keep the hard guardian armed while disposing pipe resources; no post-disarm blocking I/O.
    try{if($out){$out.Reader.Dispose()};if($err){$err.Reader.Dispose()};$stdioDisposed=$true}catch{$control.Unknown=$true}
    if($child -and $reaped){
        try{$childExit=$child.ExitCode;$control.Child=$null;$child.Dispose();$child=$null}catch{$control.Unknown=$true}
    }
}
try {
    if($control.Unknown -or $control.Forced -or (Age) -ge 28){$result=1;if(-not $failure){$failure='cleanup_unknown'}}
    if($authorized -and $claimed) {
        $file=New-PublicOutput 'watchdog.json'
        try {
            Write-Public $file ([ordered]@{
                classification='cold-core-native-read-errors-c2-watchdog-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
                result=if($result -eq 0){'observed'}else{'failed_or_unknown'};failure=$failure;whole_attempt_verdict='not_established'
                child_pid=$childId;child_exit=$childExit;child_reaped=$reaped;stdio_disposed=$stdioDisposed
                forced=$control.Forced;cleanup_unknown=$control.Unknown;managed_inputs_disposed=$managedDisposed
                stdout=if($out){$out.Text.ToString()}else{''};stderr=if($err){$err.Text.ToString()}else{''}
                stdout_eof=if($out){$out.Eof}else{$false};stderr_eof=if($err){$err.Eof}else{$false}
                output_cap=($out -and $out.Cap) -or ($err -and $err.Cap)
                work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1;observed_seconds=(Age)
                case_order=@((Case-Specs).Name);case_count=2;cases_observed=if($childReport){$childReport.cases_observed}else{0};private_dataset_access=$false
            })
        }finally{$file.Dispose()}
    }else{$result=1}
    if((Age) -ge 28){$result=1}
    if($control.Child -and -not $reaped){
        # Do not disarm ownership protection for an unreaped child. The guardian remains active to self-cutoff29.
        while((Age) -lt 29){[Threading.Thread]::Sleep(20)}
        $self.Kill();exit 1
    }
    $null=$done.Set()
    if(-not $ticket.AsyncWaitHandle.WaitOne(300)){Stop-C2N 'cleanup_unknown'}
    $null=$guardian.EndInvoke($ticket)
}catch{$result=1}
finally {
    if($ticket.IsCompleted){
        $guardian.Dispose();$done.Dispose()
        if($child){$child.Dispose()};$self.Dispose()
    }
}
if($result -eq 0){[Console]::Out.WriteLine('ColdA two-native-read-errors supervisor returned; root actual-return review required.')}
exit $result
