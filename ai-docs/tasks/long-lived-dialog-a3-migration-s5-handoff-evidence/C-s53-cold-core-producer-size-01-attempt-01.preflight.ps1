[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,
    [Parameter(Mandatory)][long]$StartTimestamp
)
# Dormant R2 source submission; explicit future native invocation needs root activation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-producer-size-01-attempt-01'
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
$cleanupLimit=450
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
            $map['classification'].GetString() -ceq 'root-approved-cold-core-producer-size-c2-r01' -and
            $map['root_thread_id'].GetString() -ceq '01a164ec-589c-7b41-bc34-3ba158d1f22b' -and
            $map['activated'].ValueKind -eq [Text.Json.JsonValueKind]::True -and
            $map['attempt'].GetString() -ceq $attempt -and $map['operation_set'].GetString() -ceq 'cold-producer-size-one-only-r01' -and
            $map['workspace'].GetString() -ceq $w -and $map['fixture_parent'].GetString() -ceq ($w+'\.tmp\'+$attempt) -and
            $map['work_seconds'].GetInt32() -eq 420 -and $map['whole_seconds'].GetInt32() -eq 480 -and
            $map['retry_count'].GetInt32() -eq 0 -and $map['lane_count'].GetInt32() -eq 1 -and
            $map['public_report_bytes'].GetInt32() -eq 65536 -and $map['public_total_bytes'].GetInt32() -eq 196608) 'allocation_scope'
        $cases=$map['fixture_cases'];Need ($cases.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $cases.GetArrayLength() -eq 1 -and
            $cases[0].ValueKind -eq [Text.Json.JsonValueKind]::String -and $cases[0].GetString() -ceq 'producer-size-r01') 'allocation_scope'
        $outputs=$map['outputs'];Need ($outputs.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $outputs.GetArrayLength() -eq 3) 'allocation_scope'
        $names=@('started.json','producer-size.json','watchdog.json')
        for($i=0;$i -lt 3;$i++){Need ($outputs[$i].ValueKind -eq [Text.Json.JsonValueKind]::String -and $outputs[$i].GetString() -ceq $names[$i]) 'allocation_scope'}
        $pins=[ordered]@{}
        foreach($key in @('preflight','watchdog','core','output','root_native','deadline','fixture_support','shell')){
            $hash=$map[$key+'_sha256'].GetString();Need ($hash -cmatch '\A[0-9a-f]{64}\z') 'allocation_schema';$pins[$key]=$hash
        }
        $fixedPins=[ordered]@{core='4aa345e8ec3a0a61b8b60b87d514d461fa76a2f78c56094c024b1114ce619515';output='a53853c9dd25ae07ec4815787b0d3c592df4644ebd8e0c33c44cec7803d760ea';root_native='a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382';deadline='0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45';fixture_support='852310d69d030e8ce386c14aa35d06bcbfc1c585368e023beee230db8e4eda37';shell='362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'}
        $fixedLengths=[ordered]@{core=81411;output=15663;root_native=25596;deadline=6038;fixture_support=56156;shell=301368}
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
    Assert-Fields $v ([ordered]@{classification='cold-core-producer-size-c2-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
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
    foreach($key in @('deadline','Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-S53ColdOutputAdapter','Initialize-S53ProducerSizeFixtureSupportR01')){
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

function Assert-FixtureAcl([string]$Path,[bool]$Directory) {
    Assert-S53RootPrivateAcl $Path $Directory
    $item=if($Directory){[IO.DirectoryInfo]::new($Path)}else{[IO.FileInfo]::new($Path)}
    $acl=[IO.FileSystemAclExtensions]::GetAccessControl($item)
    Need $acl.AreAccessRulesProtected 'premise_failure'
    $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
    try{$sid=$identity.User.Value}finally{$identity.Dispose()}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($rule in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])){
        Need (-not $rule.IsInherited -and $seen.Add($rule.IdentityReference.Value) -and
            $rule.AccessControlType -eq [Security.AccessControl.AccessControlType]::Allow -and
            $rule.FileSystemRights -eq [Security.AccessControl.FileSystemRights]::FullControl) 'premise_failure'
    }
    Need (-not $controllerCleanupUnknown -and $seen.Count -eq 3 -and $seen.Contains($sid) -and $seen.Contains('S-1-5-18') -and $seen.Contains('S-1-5-32-544')) 'premise_failure'
}
function Set-Phase([string]$Name) {
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    $phaseTimes[$script:phase]=($now-$script:phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
    $script:phase=$Name;$script:phaseStart=$now;Check
}
function Diagnostic($Item,[string]$Kind) {
    $message=[string]$(if($Item -is [Management.Automation.ErrorRecord]){$Item.Exception.Message}else{[string]$Item})
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2R 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2R 'diagnostic_cap'}
}
function Boundary([string]$Name) {
    Need ($coreBoundaries.Count -lt 16)
    $coreBoundaries.Add([ordered]@{name=$Name;qpc=[Diagnostics.Stopwatch]::GetTimestamp()})
}
function Initialize-Reviewed([string]$Name,[scriptblock]$Body) {
    Need (-not $initializeCalls.Contains($Name)) 'core_type_cached';$initializeCalls[$Name]=1
    try {
        & $Body *>&1 | ForEach-Object {
            Check
            if($_ -is [Management.Automation.ErrorRecord]){$script:compilerError=$true;Diagnostic $_ 'error'}
            elseif($_ -is [Management.Automation.WarningRecord]){Diagnostic $_ 'warning'}
            else{Diagnostic $_ 'output'}
        }
    }catch{$script:compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2R 'compiler_error'}
    Need (-not $compilerError) 'compiler_error'
}
function Assert-Definitions([string]$Path,[int]$Functions) {
    $tokens=$null;$errors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($Path,[ref]$tokens,[ref]$errors)
    Need ($errors.Count -eq 0 -and $ast.EndBlock.Statements.Count -eq $Functions -and
        @($ast.EndBlock.Statements|Where-Object {$_ -isnot [Management.Automation.Language.FunctionDefinitionAst]}).Count -eq 0) 'definitions_shape'
}
$binding=$null;$claim=$null;$workerSelf=$null;$workerPid=0;$workerStartedUtc=$null;$workerSelfDisposed=$false;$authorized=$false
$phase='bootstrap';$phaseTimes=[ordered]@{};$phaseStart=[Diagnostics.Stopwatch]::GetTimestamp();$workerScriptQpc=$phaseStart
$diagnostics=[Collections.Generic.List[object]]::new();$diagnosticChars=0;$compilerError=$false;$diagnosticCap=$false
$initializeCalls=[ordered]@{};$typeAbsent=[ordered]@{};$sourceBefore=[ordered]@{};$sourceAfter=[ordered]@{}
$coreObject=$null;$outputOwner=$null;$pendingBinding=$null;$bindingEntryUnknown=$false;$pendingResolved=$false
$coreDisposed=$false;$outputDisposed=$false;$helperDisposed=$false;$controllerCleanupUnknown=$false;$cleanupFirstCause=$null
$setupSummary=$null;$helperSummary=$null;$receiptReport=$null;$corePublic=$null;$preCapture=$null;$coreUtf8=$null
$factoryBefore=0L;$factoryAfter=0L;$caseAcquired=0;$coreHookIndex=0
$coreBoundaries=[Collections.Generic.List[object]]::new()
$coreHooks=@('output_binding','locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before','hash_bkp_before','hash_s_after','hash_bkp_after','scan_after','final_inputs','receipts')
$preparedOutputHandles=0;$afterTransferHandles=0;$outputAncestryCount=0
$outputAfterCore=$false;$outputAfterReceipts=$false;$outputAfterVerifier=$false
$fixtureAccess=$false;$managedDisposed=$false;$publicInputsBeforeClose=0;$jobFinished=$false;$childrenAfter=$null
$failure='not_started';$primaryFailure=$null;$failurePhase=$null;$failureCause=$null;$candidate=$false;$result=1;$reportClosed=$false
$absoluteWorkDeadline=$StartTimestamp+420L*[Diagnostics.Stopwatch]::Frequency
try {
    Need ($StartTimestamp -gt 0 -and $PSCommandPath -ceq $preflight) 'allocation_scope'
    $workerSelf=[Diagnostics.Process]::GetCurrentProcess();$workerSelf.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $workerPid=$workerSelf.Id;$workerStartedUtc=$workerSelf.StartTime.ToUniversalTime().ToString('o')
    $binding=Validate-Allocation;$authorized=$true;$sourceBefore=Source-Pins $binding
    $started=Open-Public "$e\$attempt.started.json" 65536;$claim=Decode-Public (Bytes $started 65536)
    Assert-Fields $claim ([ordered]@{classification='cold-core-producer-size-c2-start-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;permission_consumed=$true
        work_seconds=420;whole_seconds=480;retry_count=0;lane_count=1})
    Need ($claim.supervisor_pid -is [int] -or $claim.supervisor_pid -is [long]) 'start_claim'
    Need ($claim.supervisor_pid -gt 0 -and $claim.native_started_utc -is [string]) 'start_claim'
    Need ((Age) -lt 400) 'bootstrap_reserve'
    Set-Phase 'job_setup'
    $typeAbsent.deadline=$null -eq ('KilogramS5Deadline' -as [type]);Need $typeAbsent.deadline 'core_type_cached'
    Initialize-Reviewed 'deadline' {. $deadline}
    $remainingWork=420-(Age);$remainingWhole=450-(Age);Need ($remainingWork -gt 0 -and $remainingWhole -gt $remainingWork)
    $guard=[KilogramS5Deadline]::new($remainingWork,$remainingWhole);Check
    Set-Phase 'definitions';Assert-Definitions $core 3;Assert-Definitions $output 3;Assert-Definitions $fixtureSupport 1
    $types=[ordered]@{core='Kilogram.S53.ColdA.Core';root_native='S53RootNativeR38';output='Kilogram.S53.ColdOutput.BaselineSmallOwner';fixture_support='Kilogram.S53.ProducerSizeR01.FixtureSupport'}
    foreach($key in $types.Keys){$typeAbsent[$key]=$null -eq ($types[$key] -as [type]);Need $typeAbsent[$key] 'core_type_cached'}
    . $core
    . $rootNative
    . $output
    . $fixtureSupport
    Set-Phase 'compilation'
    foreach($name in @('Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-S53ColdOutputAdapter','Initialize-S53ProducerSizeFixtureSupportR01')){
        Initialize-Reviewed $name ([scriptblock]::Create($name))
    }
    foreach($key in $types.Keys){Need ($null -ne ($types[$key] -as [type])) 'signature_mismatch'}
    Need ([Threading.Thread]::CurrentThread.GetApartmentState().ToString() -ceq 'MTA') 'apartment'
    Set-Phase 'fixture_oracle'
    $primitives=[Kilogram.S53.ProducerSizeR01.Primitives]::new(
        [Action[string]]{param($p) [S53RootNativeR38]::NewDirectory($p)},
        [Func[string,Microsoft.Win32.SafeHandles.SafeFileHandle]]{param($p) return [S53RootNativeR38]::DirectoryHandle($p)},
        [Func[string,IO.FileStream]]{param($p) return [S53RootNativeR38]::NewFile($p)},
        [Func[string,IO.FileStream]]{param($p) return [S53RootNativeR38]::Read($p,$false)},
        [Action[string,bool]]{param($p,$d) Assert-FixtureAcl $p $d})
    $fixtureObject=[Kilogram.S53.ProducerSizeR01.FixtureSupport]::Create($absoluteWorkDeadline,[Action]{Check},$primitives)
    Set-Phase 'fixture_setup';$fixtureStage='dormant_ready';$trackFixturePhases=$true;$fixtureAccess=$true
    $fixtureObject.PrepareAndVerify();$trackFixturePhases=$false
    Set-Phase 'guard_handoff';$fixtureObject.ReleaseSetupGuards();$setupSummary=$fixtureObject.GetSummary();Assert-Summary $setupSummary $false
    Set-Phase 'factory';$factoryBefore=[Diagnostics.Stopwatch]::GetTimestamp()
    $coreObject=New-S53ColdAFixtureCore -Case producer-size-r01 -InventoryBytes 3465145 -InventorySha256 'c40ffdedd028be593c08ded6b05daba44abe0d1a9e1dac86cb6d78632cbf3843' -ExpectedFiles 6956 -ExpectedPayloadBytes 45597399 -WorkDeadlineTimestamp $absoluteWorkDeadline
    $factoryAfter=[Diagnostics.Stopwatch]::GetTimestamp()
    $hook=[Action[string]]{
        param($Name)
        Check
        if($Name -ceq 'expected_file_acquired'){$script:caseAcquired++;Need ($caseAcquired -le 13912);return}
        Need ($script:coreHookIndex -lt 13 -and $Name -ceq $coreHooks[$script:coreHookIndex]);$script:coreHookIndex++;Boundary $Name
        if($Name -ceq 'scan_before'){
            Need ($caseAcquired -eq 13912 -and $setupSummary.ReadyForCore -and $setupSummary.DirectoryOwnersRemaining -eq 5)
            $script:preCapture=Decode-Public ($coreObject.PublicObservationUtf8())
            Assert-Fields $preCapture ([ordered]@{completed=$false;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$false;input_handles_released=$false
                hash_passes=0;hash_bytes=0;remaining_owned_handles=13935;peak_owned_handles=13936;peak_path_bytes=563072;peak_record_bytes=18537456})
            Need ($preCapture.capture_complete_timestamp -ge $factoryAfter -and $preCapture.capture_complete_timestamp -lt $absoluteWorkDeadline)
        }
        if($Name.StartsWith('hash_',[StringComparison]::Ordinal)){Need ($caseAcquired -eq 13912 -and $null -ne $preCapture -and $preCapture.expected_set_captured_before_hash)}
    }
    $coreObject.SetFixtureHook($hook)
    Set-Phase 'output_binding'
    $outputOwner=[Kilogram.S53.ColdOutput.BaselineSmallOwner]::CreateProducerSize($absoluteWorkDeadline)
    $outputOwner.PrepareDirectory();$outputAncestryCount=$outputOwner.AncestryCount;$preparedOutputHandles=$outputOwner.RemainingOwnedHandles
    Need ($outputOwner.DirectoryCreated -and $outputAncestryCount -eq 6 -and $preparedOutputHandles -eq 7 -and -not $outputOwner.CleanupUnknown)
    $creator=$outputOwner.Creator;$aclVerifier=$outputOwner.AclVerifier
    # Preserve the same SafeFileHandle reference until bind returns. On failure,
    # dispose Core first; only then close an actually still-open pending reference.
    $pendingBinding=$outputOwner.TakeDirectory()
    try{$coreObject.BindOwnedOutput($pendingBinding,$creator,$aclVerifier,$AllocationSha256,$binding.Pins.core);$pendingBinding=$null;$pendingResolved=$true}
    catch{$bindingEntryUnknown=$true;throw}
    $afterTransferHandles=$outputOwner.RemainingOwnedHandles;Need ($outputOwner.DirectoryTransferred -and $afterTransferHandles -eq 6)
    Set-Phase 'comparison';$coreObject.CaptureAndCompare();Boundary 'capture_return';$outputOwner.RecheckAncestry();$outputAfterCore=$true
    Set-Phase 'receipts';$coreObject.WriteReceipts();Boundary 'receipts_return';$outputOwner.RecheckAncestry();$outputAfterReceipts=$true
    Set-Phase 'core_dispose';$coreObject.Dispose();$coreDisposed=$true;Boundary 'dispose_return'
    $coreUtf8=$coreObject.PublicObservationUtf8();$corePublic=Decode-Public $coreUtf8;Assert-CoreFinal $corePublic
    Need ($coreHookIndex -eq 13 -and $caseAcquired -eq 13912 -and $null -ne $preCapture -and $preCapture.capture_complete_timestamp -eq $corePublic.capture_complete_timestamp)
    Set-Phase 'receipt_verification'
    $receiptReport=$fixtureObject.VerifyReceipts($coreUtf8,$AllocationSha256,$binding.Pins.core,$factoryBefore,$factoryAfter,$StartTimestamp)
    Need ($receiptReport.Verified)
    $outputOwner.RecheckAncestry();$outputAfterVerifier=$true
    Need ($outputOwner.RemainingOwnedHandles -eq 6 -and $outputOwner.ReceiptCreatedMask -eq 15 -and $outputOwner.ReceiptsTransferred -eq 4 -and -not $outputOwner.CleanupUnknown)
    Set-Phase 'output_dispose';$outputOwner.Dispose();$outputDisposed=$true
    Need ($outputOwner.RemainingOwnedHandles -eq 0 -and -not $outputOwner.CleanupUnknown)
    Set-Phase 'helper_dispose';$fixtureObject.Dispose();$helperDisposed=$true;$helperSummary=$fixtureObject.GetSummary();Assert-Summary $helperSummary $true
    Set-Phase 'source_after';$sourceAfter=Source-Pins $binding;Check
    $failure=$null;$candidate=$true
}catch{$failure=Failure-Code $_.Exception;$primaryFailure=$failure;$failurePhase=$phase;$failureCause=Cause $_.Exception}
finally {
    $trackFixturePhases=$false;$cleanupReserve=$true
    if($coreObject -and -not $coreDisposed){try{$coreObject.Dispose();$coreDisposed=$true}catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}}
    if($pendingBinding){
        try{if(-not $pendingBinding.IsClosed){$pendingBinding.Dispose()};$pendingResolved=$pendingBinding.IsClosed;$pendingBinding=$null}
        catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    }
    if($outputOwner -and -not $outputDisposed){try{$outputOwner.Dispose();$outputDisposed=$true}catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}}
    if($fixtureObject){
        try{if(-not $helperDisposed){$fixtureObject.Dispose();$helperDisposed=$true};$helperSummary=$fixtureObject.GetSummary();if($helperSummary.CleanupUnknown -or $helperSummary.PartialReturnUnknown -or $helperSummary.PartialCreateUnknown){$controllerCleanupUnknown=$true}}
        catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    }
    if($coreObject){try{$f=$coreObject.GetObservation();if($f.CleanupUnknown -or $f.RemainingOwnedHandles -ne 0){$controllerCleanupUnknown=$true};if($null -eq $corePublic){$corePublic=Decode-Public ($coreObject.PublicObservationUtf8())}}catch{$controllerCleanupUnknown=$true}}
    if($outputOwner -and ($outputOwner.CleanupUnknown -or $outputOwner.RemainingOwnedHandles -ne 0)){$controllerCleanupUnknown=$true}
    $publicInputsBeforeClose=$held.Count;$managedDisposed=Close-PublicInputs;if(-not $managedDisposed){$controllerCleanupUnknown=$true}
    if($guard){
        try{$guard.CleanupChildren([int][Math]::Max(0,[Math]::Min(1000,(450-(Age))*1000)));$childrenAfter=@($guard.Children()).Count
            Need ($childrenAfter -eq 0) 'cleanup_unknown';$guard.Finish();$jobFinished=$true}
        catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    }
    if($workerSelf){try{$workerSelf.Dispose();$workerSelfDisposed=$true}catch{$controllerCleanupUnknown=$true}}
}
try {
    Check
    $now=[Diagnostics.Stopwatch]::GetTimestamp();$phaseTimes[$phase]=($now-$phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
    if($controllerCleanupUnknown -or $bindingEntryUnknown -or -not $managedDisposed -or -not $jobFinished){$candidate=$false;if(-not $failure){$failure='cleanup_unknown'}}
    $fixtureObserved=[ordered]@{}
    if($fixtureBoundaries.Count -eq 4){
        $fixtureObserved.create_readback=($fixtureBoundaries[1].qpc-$fixtureBoundaries[0].qpc)/[double][Diagnostics.Stopwatch]::Frequency
        $fixtureObserved.baseline_walk=($fixtureBoundaries[2].qpc-$fixtureBoundaries[1].qpc)/[double][Diagnostics.Stopwatch]::Frequency
        $fixtureObserved.prepared_walk=($fixtureBoundaries[3].qpc-$fixtureBoundaries[2].qpc)/[double][Diagnostics.Stopwatch]::Frequency
    }
    $report=[ordered]@{classification='cold-core-producer-size-c2-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
        result=$(if($candidate){'observed'}else{'failed_or_unknown'});failure=$failure;primary_failure=$primaryFailure;failure_phase=$failurePhase;failure_cause=$failureCause;cleanup_first_cause=$cleanupFirstCause
        assertions_passed=$candidate;whole_attempt_verdict='not_established';child_pid=$workerPid;worker_started_utc=$workerStartedUtc;started_supervisor_pid=$(if($claim){$claim.supervisor_pid}else{0})
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;work_deadline_timestamp=$absoluteWorkDeadline;priority='BelowNormal'
        powershell_version=$PSVersionTable.PSVersion.ToString();dotnet_version=[Environment]::Version.ToString();worker_script_initialized_qpc=$workerScriptQpc;before_worker_phase_clock_seconds=(($workerScriptQpc-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency)
        source_pins_before=$sourceBefore;source_pins_after=$sourceAfter;fresh_types_absent_before=$typeAbsent;initialization_calls=$initializeCalls
        compiler_error=$compilerError;diagnostic_cap=$diagnosticCap;diagnostics=$diagnostics.ToArray();phase_seconds=$phaseTimes
        fixture_phase_boundaries=$fixtureBoundaries.ToArray();fixture_observed_seconds=$fixtureObserved;fixture_probe_count=$fixtureProbeCount;fixture_probe_seconds=($fixtureProbeTicks/[double][Diagnostics.Stopwatch]::Frequency);fixture_phase_semantics='first common-QPC Check after helper stage change; observed boundaries, includes aggregate-probe overhead'
        core_boundaries=$coreBoundaries.ToArray();core_hook_count=$coreHookIndex;core_boundary_semantics='common-QPC hook/return observations, not exact internal phase starts'
        setup_summary=$setupSummary;helper_summary=$helperSummary;helper_disposed=$helperDisposed;core_disposed=$coreDisposed;output_disposed=$outputDisposed
        pending_binding_handle_resolved=$pendingResolved;binding_entry_unknown=$bindingEntryUnknown
        output_created=$(if($outputOwner){$outputOwner.DirectoryCreated}else{$false});output_transferred=$(if($outputOwner){$outputOwner.DirectoryTransferred}else{$false})
        output_ancestry_count=$outputAncestryCount;output_prepared_handles=$preparedOutputHandles;output_after_transfer_handles=$afterTransferHandles
        output_receipt_mask=$(if($outputOwner){$outputOwner.ReceiptCreatedMask}else{0});output_receipts_transferred=$(if($outputOwner){$outputOwner.ReceiptsTransferred}else{0})
        output_remaining_handles=$(if($outputOwner){$outputOwner.RemainingOwnedHandles}else{$null});output_cleanup_unknown=$(if($outputOwner){$outputOwner.CleanupUnknown}else{$true});output_cleanup_native_error=$(if($outputOwner){$outputOwner.CleanupNativeError}else{0})
        output_recheck_after_core=$outputAfterCore;output_recheck_after_receipts=$outputAfterReceipts;output_recheck_after_verifier=$outputAfterVerifier
        acquired_files=$caseAcquired;captured_input_owners=$(if($preCapture){$preCapture.remaining_owned_handles-1}else{0});core_capture_owners_with_e=$(if($preCapture){$preCapture.remaining_owned_handles}else{0})
        factory_before_qpc=$factoryBefore;factory_after_qpc=$factoryAfter;capture_complete_timestamp=$(if($corePublic){$corePublic.capture_complete_timestamp}else{0})
        pre_capture=$preCapture;core_observation=$corePublic;receipts=$receiptReport
        managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose;owned_children_after=$childrenAfter;job_finished=$jobFinished;job_finish_requires_actual_exit_zero=$true;worker_self_disposed=$workerSelfDisposed
        controller_cleanup_unknown=$controllerCleanupUnknown;synthetic_fixture_access=$fixtureAccess;private_dataset_access=$false;fixture_retained_if_created=$true
        work_seconds=420;whole_seconds=480;retry_count=0;lane_count=1;observed_seconds=(Age)}
    if($candidate){Assert-WorkerReport $report $binding.Pins $workerPid $workerStartedUtc $claim.supervisor_pid}
    if($authorized){
        $file=New-PublicOutput 'producer-size.json';$safe=$null
        try{$safe=$file.SafeFileHandle;Write-Public $file $report}finally{$file.Dispose();$reportClosed=$null -ne $safe -and $safe.IsClosed}
        Need $reportClosed 'cleanup_unknown'
    }else{$candidate=$false}
    Check;if($candidate -and $reportClosed){$result=0}
}catch{
    $result=1
    if($null -eq $primaryFailure){$failure=Failure-Code $_.Exception;$primaryFailure=$failure;$failurePhase='reporting';$failureCause=Cause $_.Exception}
    elseif($null -eq $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}
}
if($result -ne 0){
    $diagnostic=ConvertTo-Json -Depth 6 -Compress -InputObject ([ordered]@{classification='cold-core-producer-size-worker-failed';failure=$failure;primary_failure=$primaryFailure;failure_phase=$failurePhase;cause=$failureCause;cleanup_first_cause=$cleanupFirstCause;cleanup_unknown=$controllerCleanupUnknown})
    if($diagnostic.Length -le 4000){[Console]::Error.WriteLine($diagnostic)}
    else{[Console]::Error.WriteLine('{"classification":"cold-core-producer-size-worker-failed","failure":"diagnostic_cap"}')}
}
exit $result
