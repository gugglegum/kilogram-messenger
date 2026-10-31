[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256)
# Dormant R2 source submission; execution requires a separately pinned root activation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-producer-size-01-attempt-02'
$e="$w\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence"
$allocation="$e\$attempt.root-allocation.json"
$preflight="$e\$attempt.preflight.ps1";$watchdog="$e\$attempt.watchdog.ps1"
$core="$w\scripts\migration-s53-cold-preservation.ps1";$output="$w\scripts\migration-s53-cold-output.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1";$deadline="$w\scripts\migration-s5-l1-deadline.ps1"
$fixtureSupport="$e\$attempt.fixture-support.ps1"
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$held=[Collections.Generic.List[IO.FileStream]]::new();$guard=$null;$cleanupReserve=$false
$fixtureObject=$null;$trackFixturePhases=$false;$fixtureStage='';$fixtureProbeCount=0L;$fixtureProbeTicks=0L
$fixtureBoundaries=[Collections.Generic.List[object]]::new()
$cleanupLimit=470
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Stop-C2R([string]$Code) {
    $err=[InvalidOperationException]::new('C2R bounded refusal');$err.Data['c2r_code']=$Code;throw $err
}
function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2R $Code}}
function Check {
    $age=Age;$limit=if($cleanupReserve){$cleanupLimit}else{420}
    if($age -lt 0 -or $age -ge $limit){Stop-C2R $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
    if($guard -and -not $cleanupReserve){$guard.CheckWork()}
    if($trackFixturePhases -and $null -ne $fixtureObject){
        # Read only a bounded aggregate snapshot; no OS query or per-file retention.
        $probeStart=[Diagnostics.Stopwatch]::GetTimestamp();$snapshot=$fixtureObject.GetSummary();$script:fixtureProbeCount++
        if($snapshot.Stage -cne $fixtureStage){
            Need ($fixtureBoundaries.Count -lt 5 -and $snapshot.Stage -cin @('create','baseline','prepared','premises_verified')) 'fixture_stage'
            $script:fixtureStage=$snapshot.Stage
            $fixtureBoundaries.Add([ordered]@{stage=$snapshot.Stage;qpc=[Diagnostics.Stopwatch]::GetTimestamp()})
        }
        $script:fixtureProbeTicks+=[Diagnostics.Stopwatch]::GetTimestamp()-$probeStart
    }
    if((Age) -ge $limit){Stop-C2R $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        if($at.Data.Contains('c2r_code')){return [string]$at.Data['c2r_code']}
        if($at.Data.Contains('c2r_support_code')){return 'fixture_or_receipt_refusal'}
        if($at.Data.Contains('colda_code')){return 'core_refusal'}
        if($at -is [TimeoutException]){return 'work_expired'}
    }
    return 'failed_or_unknown'
}
function Open-Public([string]$Path,[long]$Maximum) {
    Check
    if(-not [IO.Path]::IsPathFullyQualified($Path) -or
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2R 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2R 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2R 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2R 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2R 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2R 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2R 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2R 'source_pin'}
    return $f
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2R 'output_cap'}
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
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2r_code','c2r_support_code','cold_output_code')){
            $value=[string]$at.Data[$key]
            if($value -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$value}
        }
    }
    $type=$last.GetType().FullName
    if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks}
}
function New-PublicOutput([string]$Suffix) {
    Check;Need ($Suffix -cin @('started.json','producer-size.json','watchdog.json')) 'output_cap'
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
            'preflight_sha256','watchdog_sha256','core_sha256','output_sha256','root_native_sha256','deadline_sha256','fixture_support_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        Need ($doc.RootElement.ValueKind -eq [Text.Json.JsonValueKind]::Object) 'allocation_schema'
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($p in $doc.RootElement.EnumerateObject()){Need ($p.Name -cin $fields -and $map.TryAdd($p.Name,$p.Value)) 'allocation_schema'}
        Need ($map.Count -eq 24) 'allocation_schema'
        foreach($key in @('schema_version','work_seconds','whole_seconds','retry_count','lane_count','public_report_bytes','public_total_bytes')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::Number) 'allocation_schema'
        }
        foreach($key in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','output_sha256','root_native_sha256','deadline_sha256','fixture_support_sha256','shell_sha256')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::String) 'allocation_schema'
        }
        Need ($map['schema_version'].GetInt32() -eq 1 -and
            $map['classification'].GetString() -ceq 'root-approved-cold-core-producer-size-c2-r02' -and
            $map['root_thread_id'].GetString() -ceq '01a164ec-589c-7b41-bc34-3ba158d1f22b' -and
            $map['activated'].ValueKind -eq [Text.Json.JsonValueKind]::True -and
            $map['attempt'].GetString() -ceq $attempt -and $map['operation_set'].GetString() -ceq 'cold-producer-size-one-only-r02' -and
            $map['workspace'].GetString() -ceq $w -and $map['fixture_parent'].GetString() -ceq ($w+'\.tmp\'+$attempt) -and
            $map['work_seconds'].GetInt32() -eq 420 -and $map['whole_seconds'].GetInt32() -eq 480 -and
            $map['retry_count'].GetInt32() -eq 0 -and $map['lane_count'].GetInt32() -eq 1 -and
            $map['public_report_bytes'].GetInt32() -eq 65536 -and $map['public_total_bytes'].GetInt32() -eq 196608) 'allocation_scope'
        $cases=$map['fixture_cases'];Need ($cases.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $cases.GetArrayLength() -eq 1 -and
            $cases[0].ValueKind -eq [Text.Json.JsonValueKind]::String -and $cases[0].GetString() -ceq 'producer-size-r02') 'allocation_scope'
        $outputs=$map['outputs'];Need ($outputs.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $outputs.GetArrayLength() -eq 3) 'allocation_scope'
        $names=@('started.json','producer-size.json','watchdog.json')
        for($i=0;$i -lt 3;$i++){Need ($outputs[$i].ValueKind -eq [Text.Json.JsonValueKind]::String -and $outputs[$i].GetString() -ceq $names[$i]) 'allocation_scope'}
        $pins=[ordered]@{}
        foreach($key in @('preflight','watchdog','core','output','root_native','deadline','fixture_support','shell')){
            $hash=$map[$key+'_sha256'].GetString();Need ($hash -cmatch '\A[0-9a-f]{64}\z') 'allocation_schema';$pins[$key]=$hash
        }
        $fixedPins=[ordered]@{core='ed56b2c3b632d81b8a775db45da93bd6657aef6ca47cd1d81f237605c89b2f43';output='40710b0a2993e768a0f0b52d9949947258a3d44281f9f1aa7d10417efb7571c2';root_native='a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382';deadline='0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45';fixture_support='adb6f2ffe7cc45d2d4eee41bf7e0d97114a7edc9a4213bf5d3af8dcf56e3c039';shell='362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'}
        $fixedLengths=[ordered]@{core=81937;output=16100;root_native=25596;deadline=6038;fixture_support=56156;shell=301368}
        foreach($key in $fixedPins.Keys){Need ($pins[$key] -ceq $fixedPins[$key]) 'source_pin'}
        $paths=[ordered]@{preflight=$preflight;watchdog=$watchdog;core=$core;output=$output;root_native=$rootNative;deadline=$deadline;fixture_support=$fixtureSupport;shell=$shell}
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
function Assert-CoreFinal($f) {
    Assert-Fields $f ([ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$true;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$true;input_handles_released=$true;receipts_completed=$true
        cleanup_unknown=$false;receipt_write_unknown=$false;failure_code=$null;failure_phase=$null;failure_role=$null;cause_kind=$null
        cleanup_native_error=0;native_error=0;error_hresult=0;expected_files=6956;expected_bytes=45597399;locks=6;hash_passes=4;hash_bytes=182389596
        peak_owned_handles=13938;remaining_owned_handles=0;peak_path_bytes=563128;peak_record_bytes=18537840;peak_inventory_bytes=3465145;scratch_reserved_bytes=524288})
    foreach($key in @('historical_tuple_digest','s_before_tuple_digest','bkp_before_tuple_digest','s_after_tuple_digest','bkp_after_tuple_digest')){
        Assert-Fields $f ([ordered]@{$key='2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf'})
    }
    Need (@(Object-Keys $f.private_receipt_digests).Count -eq 4)
}
function Assert-Summary($s,[bool]$Final) {
    Assert-Fields $s ([ordered]@{CreatedDirectories=16;CreateAttempts=13915;CreateReturns=13915
        ReadAttempts=$(if($Final){27834}else{27830});ReadReturns=$(if($Final){27834}else{27830})
        FileCloseAttempts=$(if($Final){41749}else{41745});FilesClosed=$(if($Final){41749}else{41745});FileOwnersRemaining=0
        PeakFileOwners=$(if($Final){4}else{1});DirectoryOpenAttempts=$(if($Final){21}else{20});DirectoryReturns=$(if($Final){21}else{20})
        DirectoryCloseAttempts=$(if($Final){21}else{15});DirectoriesClosed=$(if($Final){21}else{15});DirectoryOwnersRemaining=$(if($Final){0}else{5})
        PeakDirectoryOwners=20;EnumerationReturns=$(if($Final){32}else{30});EnumerationsDisposed=$(if($Final){32}else{30});EnumerationRemaining=0;PeakEnumerationOwners=1
        StreamChecks=$(if($Final){41784}else{41775});StreamSearchOpened=$(if($Final){41753}else{41745});StreamSearchClosed=$(if($Final){41753}else{41745});StreamSearchRemaining=0
        BaselineFiles=13915;PreparedFiles=13915;SetupGuardsClosed=15;OracleRows=6956;PeakIdentityEntries=13930;PeakQueueEntries=15
        ReadyForCore=$true;ReceiptsVerified=$Final;Disposed=$Final;CleanupUnknown=$false;PartialCreateUnknown=$false;PartialReturnUnknown=$false
        CleanupFirstType=$null;CleanupFirstHResult=0;CleanupNativeError=0;Stage=$(if($Final){'disposed'}else{'released_for_core'})})
    Need ($s.PeakTokenWindowBytes -ge 1 -and $s.PeakTokenWindowBytes -le 65536)
    Need ($s.PeakReceiptRecordBytes -ge 0 -and $s.PeakReceiptRecordBytes -le 32768)
    if($Final){Need ($s.PeakReceiptRecordBytes -gt 0)}
}
function Assert-WorkerReport($v,$Pins,[int]$WorkerPid,[string]$WorkerStarted,[int]$SupervisorPid) {
    Assert-Fields $v ([ordered]@{classification='cold-core-producer-size-c2-r02';attempt=$attempt;allocation_sha256=$AllocationSha256
        result='observed';failure=$null;primary_failure=$null;failure_phase=$null;failure_cause=$null;cleanup_first_cause=$null;assertions_passed=$true;controller_cleanup_unknown=$false
        whole_attempt_verdict='not_established';child_pid=$WorkerPid;worker_started_utc=$WorkerStarted;started_supervisor_pid=$SupervisorPid
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;work_deadline_timestamp=($StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency)
        priority='BelowNormal';compiler_error=$false;diagnostic_cap=$false;managed_inputs_disposed=$true;public_inputs_before_close=10
        helper_disposed=$true;core_disposed=$true;output_disposed=$true;pending_binding_handle_resolved=$true;binding_entry_unknown=$false
        output_created=$true;output_transferred=$true;output_ancestry_count=6;output_prepared_handles=7;output_after_transfer_handles=6
        output_receipt_mask=15;output_receipts_transferred=4;output_remaining_handles=0;output_cleanup_unknown=$false;output_cleanup_native_error=0
        output_recheck_after_core=$true;output_recheck_after_receipts=$true;output_recheck_after_verifier=$true
        acquired_files=13912;captured_input_owners=13934;core_capture_owners_with_e=13935;core_hook_count=13
        owned_children_after=0;job_finished=$true;job_finish_requires_actual_exit_zero=$true;worker_self_disposed=$true
        synthetic_fixture_access=$true;private_dataset_access=$false;fixture_retained_if_created=$true;work_seconds=420;whole_seconds=480;retry_count=0;lane_count=1})
    foreach($key in @('powershell_version','dotnet_version')){Need ($v.$key -is [string] -and $v.$key -cmatch '\A[A-Za-z0-9._+-]{1,64}\z')}
    Need ($v.worker_started_utc -is [string] -and $v.worker_started_utc -cmatch '\A[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+Z\z')
    foreach($key in $Pins.Keys){
        Assert-Fields $v.source_pins_before ([ordered]@{$key=$Pins[$key]})
        Assert-Fields $v.source_pins_after ([ordered]@{$key=$Pins[$key]})
    }
    foreach($key in @('deadline','core','root_native','output','fixture_support')){Assert-Fields $v.fresh_types_absent_before ([ordered]@{$key=$true})}
    foreach($key in @('deadline','Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-S53ColdOutputAdapter','Initialize-S53ProducerSizeFixtureSupportR02')){
        Assert-Fields $v.initialization_calls ([ordered]@{$key=1})
    }
    Assert-Summary $v.setup_summary $false;Assert-Summary $v.helper_summary $true;Assert-CoreFinal $v.core_observation
    Assert-Fields $v.pre_capture ([ordered]@{expected_set_captured_before_hash=$true;completed=$false;observed_entry_sets_equal=$false;input_handles_released=$false
        receipts_completed=$false;expected_files=6956;expected_bytes=45597399;locks=6;hash_passes=0;hash_bytes=0
        remaining_owned_handles=13935;peak_owned_handles=13936;peak_path_bytes=563072;peak_record_bytes=18537456;peak_inventory_bytes=3465145;scratch_reserved_bytes=524288})
    Need ($v.capture_complete_timestamp -eq $v.pre_capture.capture_complete_timestamp -and
        $v.capture_complete_timestamp -eq $v.core_observation.capture_complete_timestamp -and
        $v.capture_complete_timestamp -ge $v.factory_after_qpc -and $v.factory_after_qpc -ge $v.factory_before_qpc -and
        $v.factory_before_qpc -gt $StartTimestamp -and $v.capture_complete_timestamp -lt ($StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency))
    $r=$v.receipts;Assert-Fields $r ([ordered]@{Verified=$true;Files=4;ComparisonRows=6956;NamespaceRows=11})
    Need (@($r.Sizes).Count -eq 4 -and @($r.Digests).Count -eq 4)
    $names=@('owner.private.json','comparison.private.jsonl','namespace.private.jsonl','result.private.json')
    $caps=@(32769,11484356,7007,32769);$sum=0L
    for($i=0;$i -lt 4;$i++){
        Need ($r.Sizes[$i] -is [long] -or $r.Sizes[$i] -is [int]);Need ($r.Sizes[$i] -gt 0 -and $r.Sizes[$i] -le $caps[$i])
        Need ($r.Digests[$i] -is [string] -and $r.Digests[$i] -cmatch '\A[0-9a-f]{64}\z')
        Assert-Fields $v.core_observation.private_receipt_digests ([ordered]@{$names[$i]=$r.Digests[$i]});$sum+=$r.Sizes[$i]
    }
    Need ($sum -eq $r.Bytes -and $sum -le 11556901 -and $sum -eq $v.core_observation.receipt_bytes_charged -and $sum -eq $v.core_observation.receipt_bytes_known_written)
    Need (@($v.fixture_phase_boundaries).Count -eq 4 -and $v.fixture_probe_count -gt 0)
    $previous=$StartTimestamp;$expected=@('create','baseline','prepared','premises_verified')
    for($i=0;$i -lt 4;$i++){Assert-Fields $v.fixture_phase_boundaries[$i] ([ordered]@{stage=$expected[$i]});Need ($v.fixture_phase_boundaries[$i].qpc -ge $previous);$previous=$v.fixture_phase_boundaries[$i].qpc}
    Assert-Fields $v.fixture_observed_seconds ([ordered]@{}) $false
    Need (@(Object-Keys $v.fixture_observed_seconds).Count -eq 3)
    foreach($key in @('create_readback','baseline_walk','prepared_walk')){
        $d=Get-Value $v.fixture_observed_seconds $key;Need ($d -is [double] -or $d -is [int] -or $d -is [long]);Need ([double]::IsFinite([double]$d) -and $d -ge 0 -and $d -le 420)
    }
    Need ($v.fixture_probe_seconds -ge 0 -and $v.fixture_probe_seconds -le 420)
    Need (@($v.core_boundaries).Count -eq 16)
    $previous=$StartTimestamp;$expected=@('output_binding','locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before','hash_bkp_before','hash_s_after','hash_bkp_after','scan_after','final_inputs','capture_return','receipts','receipts_return','dispose_return')
    for($i=0;$i -lt 16;$i++){Assert-Fields $v.core_boundaries[$i] ([ordered]@{name=$expected[$i]});Need ($v.core_boundaries[$i].qpc -ge $previous);$previous=$v.core_boundaries[$i].qpc}
    Need ($v.capture_complete_timestamp -le $v.core_boundaries[5].qpc)
    Need (@($v.diagnostics).Count -le 16)
    foreach($key in @(Object-Keys $v.phase_seconds)){$value=Get-Value $v.phase_seconds $key;Need ($value -is [double] -or $value -is [long] -or $value -is [int]);Need ([double]::IsFinite([double]$value) -and $value -ge 0 -and $value -le 480)}
}

function New-Drain([IO.StreamReader]$Reader) {
    $buffer=[char[]]::new(1024)
    return [pscustomobject]@{Reader=$Reader;Buffer=$buffer;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($buffer,0,$buffer.Length);Eof=$false;Cap=$false}
}
function Pump($Drain) {
    if(-not $Drain -or $Drain.Eof -or -not $Drain.Task.IsCompleted){return}
    $n=$Drain.Task.GetAwaiter().GetResult()
    if($n -eq 0){$Drain.Eof=$true;Note-EOF;return}
    if($Drain.Text.Length+$n -gt 4096){$Drain.Cap=$true;Stop-C2R 'output_cap'}
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
        if($Stage -ceq 'normal'){Set-FirstReason 'unreaped_work_timeout';Stop-C2R 'unreaped_work_timeout'}
        return
    }
    $milliseconds=[int][Math]::Min(20,$remaining)
    $before=[Diagnostics.Stopwatch]::GetTimestamp();$waited=$child.WaitForExit($milliseconds);$after=[Diagnostics.Stopwatch]::GetTimestamp()
    Record-Wait $waited $Stage $milliseconds $before $after
}
function Sample-Child {
    Check;Pump $out;Pump $err
    if($metrics.stopped_for_exit){return}
    try{$alreadyExited=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='pre_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-C2R 'observer_failure'}
    if($alreadyExited){Note-ExitIndication 'has_exited';$metrics.known_exited_skips++;$metrics.stopped_for_exit=$true;return}
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    if($now -lt $nextSampleQpc){return}
    Need ($metrics.sample_attempts -lt 4801) 'observer_cap'
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
        Stop-C2R 'observer_failure'
    }
    if($samplePid -ne $childId -or $sampleHandle -ne $retainedProcessHandle -or $sampleStarted -cne $childStartedUtc -or $handles -lt 0 -or $working -le 0){
        $metrics.observer_failed=$true;$metrics.failure_api='identity_or_range';Stop-C2R 'observer_identity'
    }
    try{$exitedAfterRead=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='post_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-C2R 'observer_failure'}
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
            if($age -ge 455){
                $Control.Forced=$true;$p=$Control.Child
                if($p){try{if(-not $p.HasExited){$p.Kill()};$null=$p.WaitForExit(20)}catch{$Control.Unknown=$true}}
            }
            if($age -ge 475){$Owner.Kill();return}
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
    foreach($suffix in @('started.json','producer-size.json','watchdog.json')){
        Need (-not [IO.File]::Exists("$e\$attempt.$suffix") -and -not [IO.Directory]::Exists("$e\$attempt.$suffix")) 'output_collision'
    }
    Need ((Age) -lt 400) 'bootstrap_reserve'
    $marker=New-PublicOutput 'started.json';$claimed=$true
    try{Write-Public $marker ([ordered]@{classification='cold-core-producer-size-c2-start-r02';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;supervisor_pid=$self.Id
        native_started_utc=$self.StartTime.ToUniversalTime().ToString('o');permission_consumed=$true;work_seconds=420;whole_seconds=480
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
        if($control.Forced -or $control.Unknown){Set-FirstReason 'completion_forced_or_unknown';Stop-C2R 'completion_forced_or_unknown'}
        if(-not $reaped){Wait-ForReap 'normal' 420}
        Check;Pump $out;Pump $err;Note-EOF
        if($reaped -and $childExit -ne 0){Set-FirstReason 'child_exit_nonzero';Stop-C2R 'child_exit_nonzero'}
        if($reaped -and $out.Eof -and $err.Eof){break}
        if($reaped){
            $remaining=[Math]::Floor((420-(Age))*1000)
            if($remaining -ge 1){[Threading.Thread]::Sleep([int][Math]::Min(5,$remaining))}
        }
    }
    $completion.normal_gate=Completion-Gate
    Need ($reaped -and $childExit -eq 0 -and $out.Eof -and $err.Eof -and -not $control.Forced -and -not $control.Unknown -and
        $exitQpc -gt $StartTimestamp -and $reapQpc -gt $StartTimestamp -and $eofQpc -gt $StartTimestamp -and
        $exitQpc -lt ($StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency) -and
        $reapQpc -lt ($StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency) -and
        $eofQpc -lt ($StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency)) 'completion_gate'
    $completion.normal_gate_passed=$true
    Need (-not $metrics.observer_failed -and $metrics.sample_count -gt 0 -and $metrics.sample_attempts -le 4801) 'observer_failure'
    if($metrics.exit_race_count -gt 0){Need ($metrics.exit_race_count -eq 1 -and $metrics.exit_race_confirmed_exited -and $metrics.stopped_for_exit) 'observer_race'}
    $normalStage='worker_report'
    $report=Open-Public "$e\$attempt.producer-size.json" 65536;$childReport=Decode-Public (Bytes $report 65536)
    Assert-WorkerReport $childReport $binding.Pins $childId $childStartedUtc $self.Id
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
            while((Age) -lt 450 -and -not $reaped){
                Wait-ForReap 'cleanup' 450;Pump $out;Pump $err
            }
            if(-not $reaped){$control.Unknown=$true;Set-FirstReason 'cleanup_unreaped'}
            else{$childExit=$child.ExitCode}
        }catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        while((Age) -lt 460 -and (($out -and -not $out.Eof) -or ($err -and -not $err.Eof))){
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
        $metrics.sample_count -gt 0 -and $null -eq $cleanupFirstCause -and $publicInputsBeforeClose -eq 10
    if(-not $good -and -not $failure){$failure='cleanup_unknown';Set-FirstReason 'cleanup_unknown'}
    if($authorized -and $claimed){
        $file=New-PublicOutput 'watchdog.json';$safe=$null
        try{
            $safe=$file.SafeFileHandle
            Write-Public $file ([ordered]@{classification='cold-core-producer-size-c2-watchdog-r02';attempt=$attempt;allocation_sha256=$AllocationSha256
                result=$(if($good){'observed'}else{'failed_or_unknown'});failure=$failure;failure_cause=$failureCause;cleanup_first_cause=$cleanupFirstCause
                whole_attempt_verdict='not_established';child_pid=$childId;child_started_utc=$childStartedUtc;retained_process_handle=$retainedProcessHandle
                child_exit=$childExit;child_reaped=$reaped;registered_child_released=$registeredChildReleased;stdio_disposed=$stdioDisposed
                forced=$control.Forced;cleanup_kill_attempted=$cleanupKillAttempted;cleanup_unknown=$control.Unknown;managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose
                stdout=$(if($out){$out.Text.ToString()}else{''});stderr=$(if($err){$err.Text.ToString()}else{''})
                stdout_eof=$(if($out){$out.Eof}else{$false});stderr_eof=$(if($err){$err.Eof}else{$false});output_cap=($out -and $out.Cap) -or ($err -and $err.Cap)
                observer=$metrics;completion=$completion;wait_trace=$waitTrace.ToArray();source_pins_after=$sourceAfter;start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
                worker_spawn_qpc=$spawnQpc;parent_before_spawn_seconds=(($spawnQpc-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency);exit_observed_qpc=$exitQpc;exit_indication_qpc=$exitQpc;reap_observed_qpc=$reapQpc;both_eof_qpc=$eofQpc;private_dataset_access=$false;owned_worker_processes_after=$(if($reaped){0}else{$null})
                work_seconds=420;whole_seconds=480;worker_cleanup_seconds=450;parent_cleanup_seconds=470;guardian_child_seconds=455;guardian_self_seconds=475
                lane_count=1;retry_count=0;observed_seconds=(Age)})
        }finally{$file.Dispose();$reportClosed=$null -ne $safe -and $safe.IsClosed}
        Need $reportClosed 'cleanup_unknown'
    }else{$good=$false}
    Check
    if($control.Child -and -not $reaped){
        while((Age) -lt 475){[Threading.Thread]::Sleep(20)}
        $self.Kill();exit 1
    }
    $null=$done.Set()
    if(-not $ticket.AsyncWaitHandle.WaitOne(300)){Stop-C2R 'cleanup_unknown'}
    $null=$guardian.EndInvoke($ticket)
    if($good -and $reportClosed -and -not $control.Unknown -and -not $control.Forced -and (Age) -lt 470){$result=0}
}catch{
    $result=1
    if($null -eq $failureCause){$failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception}
    elseif($null -eq $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}
    Set-FirstReason $(if($failure){$failure}else{'final_report_or_guardian_failure'})
}
finally {
    if($ticket.IsCompleted){$guardian.Dispose();$done.Dispose();if($child){$child.Dispose()};$self.Dispose()}
}
if($result -eq 0){[Console]::Out.WriteLine('ColdA producer-size positive returned; root actual-return review required.')}
else{
    if($null -eq $completion.first_failure_reason){
        if($control.Forced -or $control.Unknown){Set-FirstReason 'final_force_or_unknown'}
        elseif((Age) -ge 470){Set-FirstReason 'parent_cleanup_deadline'}
        else{Set-FirstReason 'final_return_failed'}
        if($null -eq $failure){$failure=$completion.first_failure_reason}
    }
    $diagnostic=ConvertTo-Json -Depth 6 -Compress -InputObject ([ordered]@{classification='cold-core-producer-size-supervisor-failed';first_failure_reason=$completion.first_failure_reason;pre_cleanup_first_reason=$completion.pre_cleanup_first_reason;failure=$failure;cause=$failureCause;cleanup_first_cause=$cleanupFirstCause;forced=$control.Forced;cleanup_unknown=$control.Unknown})
    if($diagnostic.Length -le 4000){[Console]::Error.WriteLine($diagnostic)}
    else{[Console]::Error.WriteLine('{"classification":"cold-core-producer-size-supervisor-failed","failure":"diagnostic_cap"}')}
}
exit $result

