[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$DecisionSha256,
    [ValidateSet('Supervisor','Parent')][string]$Mode='Supervisor',
    [long]$StartTimestamp=0
)
# Dormant fixed production slice. No approved D1/native plan/private grant is supplied by this source.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$taskRoot="$w\ai-docs\tasks";$e="$taskRoot\long-lived-dialog-a3-migration-s5-handoff-evidence"
$attempt='P2-s53-exhausted-cold-a-01-attempt-01'
$entry="$w\scripts\invoke-migration-s53-cold-preservation.ps1"
$corePath="$w\scripts\migration-s53-cold-preservation.ps1";$outputPath="$w\scripts\migration-s53-cold-output.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1";$deadlinePath="$w\scripts\migration-s5-l1-deadline.ps1"
$fixedShell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$decisionPath="$e\$attempt.D1.json"
$origin="$w\.tmp\migration-s5-l1\F-exhausted-origin-01"
$state="$w\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb"
$backup="$w\.tmp\migration-s5-preserved\F-exhausted-preservation-01"
$container="$w\.tmp\migration-s5-preparation-evidence";$proof="$container\$attempt"
$publicRecords=[Collections.Generic.List[object]]::new();$publicOwned=0;$publicPeak=0;$managedCloseUnknown=$false
$sourceFiles=[ordered]@{};$sourcePins=[ordered]@{};$guard=$null;$cleanupReserve=$false
$cleanupLimit=if($Mode -ceq 'Parent'){50}else{57}
function Stop-Cold([string]$Code) {
    $e=[InvalidOperationException]::new('Cold launcher bounded refusal');$e.Data['cold_launch_code']=$Code;throw $e
}
function Need([bool]$Value,[string]$Code='assertion'){if(-not $Value){Stop-Cold $Code}}
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Check {
    $limit=if($cleanupReserve){$cleanupLimit}else{45};$age=Age
    if($age -lt 0 -or $age -ge $limit){Stop-Cold $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
    if($guard -and -not $cleanupReserve){$guard.CheckWork()}
}
function Cause($Exception) {
    $marks=[ordered]@{};$last=$Exception;$native=0;$chain=[Collections.Generic.List[object]]::new();$at=$Exception
    for($i=0;$at -and $i -lt 16;$i++){
        $next=$at.InnerException
        $last=$at;$type=$at.GetType().FullName
        if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
        $chain.Add([ordered]@{type=$type;hresult=$at.HResult})
        if($at -is [ComponentModel.Win32Exception]){$native=$at.NativeErrorCode}
        foreach($key in @('cold_launch_code','cold_output_code','rootcopies_code','colda_code','colda_phase','colda_role')){
            $v=[string]$at.Data[$key];if($v -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$v}
        }
        $at=$next
    }
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks;chain=$chain.ToArray()}
}
function Public-Register([IO.FileStream]$Stream){
    Need ($publicRecords.Count -lt 40) 'public_record_cap'
    $publicRecords.Add([pscustomobject]@{Stream=$Stream;Handle=$Stream.SafeFileHandle;Closed=$false})
}
function Public-Open([string]$Path,[long]$Maximum,[bool]$Create=$false) {
    Check;Need ($publicOwned -lt 24) 'public_owner_cap'
    $script:publicOwned++;$script:publicPeak=[Math]::Max($publicPeak,$publicOwned);$stream=$null;$registered=$false
    try {
        Need ([IO.Path]::IsPathFullyQualified($Path) -and [IO.Path]::GetFullPath($Path) -ceq $Path) 'public_path'
        $at=[IO.Path]::GetDirectoryName($Path)
        while($at){Check;$attrs=[IO.File]::GetAttributes($at);Need (($attrs -band 1040) -eq 16) 'public_path';$at=[IO.Path]::GetDirectoryName($at)}
        if(-not $Create){Need (([IO.File]::GetAttributes($Path) -band 1040) -eq 0) 'public_path'}
        $stream=if($Create){[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)}else{[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)}
        Need ($stream.Length -ge 0 -and $stream.Length -le $Maximum) 'public_input_cap'
        Public-Register $stream;$registered=$true;return $stream
    }finally{
        if(-not $registered){if($stream){try{$stream.Dispose()}catch{$script:managedCloseUnknown=$true}};$script:publicOwned--}
    }
}
function Public-Close([IO.FileStream]$Stream) {
    if(-not $Stream){return}
    foreach($r in $publicRecords){
        if([object]::ReferenceEquals($r.Stream,$Stream) -and -not $r.Closed){
            try{$r.Stream.Dispose();Need $r.Handle.IsClosed 'managed_close';$r.Closed=$true;$script:publicOwned--}
            catch{$script:managedCloseUnknown=$true;throw}
            return
        }
    }
}
function Public-CloseAll {
    foreach($r in $publicRecords){if(-not $r.Closed){try{Public-Close $r.Stream}catch{$script:managedCloseUnknown=$true}}}
}
function Read-Bytes([IO.FileStream]$Stream,[long]$Maximum) {
    Check;Need ($Stream.Length -gt 0 -and $Stream.Length -le $Maximum) 'public_input_cap'
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));Need ($n -gt 0) 'public_short';$offset+=$n}
    Need ($Stream.ReadByte() -eq -1 -and $Stream.Length -eq $raw.Length) 'public_changed';$Stream.Position=0;return ,$raw
}
function Hash-Stream([IO.FileStream]$Stream) {
    Check;$Stream.Position=0;$sha=[Security.Cryptography.SHA256]::Create()
    try{$h=[Convert]::ToHexString($sha.ComputeHash($Stream)).ToLowerInvariant()}finally{$sha.Dispose()}
    $Stream.Position=0;Check;return $h
}
function Decode([byte[]]$Raw) {
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($Raw);Need ($text.Length -gt 0 -and $text[0] -ne [char]0xfeff) 'json_encoding'
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=12
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try{Unique-Json $doc.RootElement 0}finally{$doc.Dispose()}
    return ConvertFrom-Json -InputObject $text -Depth 12 -DateKind String
}
function Unique-Json($Element,[int]$Depth) {
    Need ($Depth -le 12) 'json_depth'
    if($Element.ValueKind -eq [Text.Json.JsonValueKind]::Object){
        $names=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach($p in $Element.EnumerateObject()){Need ($names.Add($p.Name)) 'json_duplicate';Unique-Json $p.Value ($Depth+1)}
    }elseif($Element.ValueKind -eq [Text.Json.JsonValueKind]::Array){foreach($v in $Element.EnumerateArray()){Unique-Json $v ($Depth+1)}}
}
function Keys($Object) {
    Need ($null -ne $Object) 'schema_object'
    if($Object -is [Collections.IDictionary]){foreach($key in $Object.Keys){$key};return}
    foreach($property in $Object.PSObject.Properties){$property.Name}
}
function Fields($Object,[string[]]$Names){
    $keys=@(Keys $Object);Need ($keys.Count -eq $Names.Count) 'schema_fields'
    foreach($key in $keys){Need ($key -is [string] -and $Names -ccontains $key) 'schema_fields'}
}
function Value($Object,[string]$Key) {
    if($Object -is [Collections.IDictionary]){return $Object[$Key]}
    return $Object.$Key
}
function Exact($Object,[Collections.IDictionary]$Expected) {
    $keys=@(Keys $Object)
    foreach($key in $Expected.Keys){
        Need ($keys -ccontains $key) 'schema_fields';$v=Value $Object $key;$wanted=$Expected[$key]
        if($null -eq $wanted){Need ($null -eq $v) 'schema_value'}
        elseif($wanted -is [string]){Need ($v -is [string] -and $v -ceq $wanted) 'schema_value'}
        elseif($wanted -is [bool]){Need ($v -is [bool] -and $v -eq $wanted) 'schema_type'}
        else{Need ($v -is [int] -or $v -is [long]) 'schema_type';Need ($v -eq $wanted) 'schema_value'}
    }
}
function Array-Exact($Actual,[string[]]$Expected){
    Need ($Actual -is [array] -and $Actual.Count -eq $Expected.Count) 'schema_array'
    for($i=0;$i -lt $Expected.Count;$i++){Need ($Actual[$i] -is [string] -and $Actual[$i] -ceq $Expected[$i]) 'schema_array'}
}
function Pin-Spec {
    return [ordered]@{
        launcher=@($entry,0,'');core=@($corePath,82999,'59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af');output=@($outputPath,21721,'331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98')
        root_native=@($rootNative,25596,'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382')
        deadline=@($deadlinePath,6038,'0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45')
        shell=@($fixedShell,301368,'362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139')
        r42=@("$taskRoot\long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md",20846,'20e427de24a510c12fbd328350edb61bdc9d8c2e8752291423c37eaf6d9655c1')
        r43=@("$taskRoot\long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md",9037,'03e4954c7627b9074717231ba616c3725dc7653b8382ad318c0da0c44d595bb3')
        r44=@("$taskRoot\long-lived-dialog-a3-migration-s5-handoff-r43-acceptance-and-cold-core-task-r44.md",7498,'eccdf671997812dd699e99fc7e600e178f208b0cd2b5340863707b7fed03ff79')
        r41=@("$e\S5.3-r41-rootcopies.root-review.json",7566,'193436d3bd2ac5223ef4af74beaf18b66d32b59e50a303a52b137ff52d2c5780')
        small=@("$e\S5.3-r55-cold-small.root-review.json",4869,'8693c60a808f2c054871fd3f5c05d7ca3e79e08ef3e95e842a4a16bcbf8863a2')
        c2r=@("$e\C-s53-cold-core-producer-size-01-attempt-02.accepted.md",3255,'892b4085172b1c4a2b2cc4a4e9a6c162176f57f6417362be904781a6d21d03b2')
        c2s=@("$e\C-s53-cold-output-wrapper-01-attempt-01.accepted.md",3704,'b8a9d0532ea3f90209b0dd5bcc2d67fa270eda771460d3ac7e1f3f7763eff5ce')
        delta=@("$e\P2-s53-exhausted-cold-a-01.integration-delta.md",10614,'26bd1c419b61cc35eb11612cc38648c3c2b4d530c69e571468e51b938ac2641c')
        integration_accepted=@("$e\P2-s53-exhausted-cold-a-01.integration-accepted.md",4365,'b0611fef9a040cfe6dc1189f48c59a82dc910009789835ee7559f8cb8a4e58ab')
        reserve_amendment=@("$e\P2-s53-exhausted-cold-a-01.control-reserve-amendment.md",2567,'0d4cb8e9f127454692b88d6977b45c82520274d8d1dbf804cea1639a76db842b')
        source_review=@("$e\P2-s53-exhausted-cold-a-01.integration-source-review.md",0,'')
        source_acceptance=@("$e\P2-s53-exhausted-cold-a-01.integration-source-accepted.md",0,'')
    }
}
function Caps {
    return [ordered]@{inventory_bytes=16777216;rows=8000;entries_per_tree=20000;files_per_tree=8000;depth=32;component_chars=255;relative_chars=1024;final_chars=32760
        payload_file_bytes=67108864;payload_tree_bytes=67108864;combined_handles=40126;core_handles=40062;external_reserve=64;public_handles_per_process=24
        path_arena=100663296;record_arena=33554432;scratch=524288;read_buffer=65536;receipt_total=67108864
        owner_receipt=65536;comparison_receipt=33554432;namespace_receipt=16777216;result_receipt=65536
        public_report=65536;public_total=327680;processes=4096;diagnostics_entries=16;diagnostics_chars=6144;stdio_chars=4096
        sampled_os_handles=45000;sampled_peak_working_set=1073741824;maximum_samples=601}
}
function Validate-Decision {
    Check;Need ($IsWindows -and [Environment]::ProcessPath -ceq $fixedShell -and $PSCommandPath -ceq $entry) 'fixed_invocation'
    $df=Public-Open $decisionPath 65536;Need ((Hash-Stream $df) -ceq $DecisionSha256) 'decision_pin'
    $d=Decode (Read-Bytes $df 65536)
    Fields $d @('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','origin','state','backup','output_parent','allow_output_parent_create','output'
        'inventory_bytes','inventory_sha256','expected_files','expected_payload_bytes','allowed_writes','caps','work_seconds','whole_seconds','retry_count','lane_count','pins','public_outputs')
    Exact $d ([ordered]@{schema_version=1;classification='root-approved-s53-exhausted-cold-a';root_thread_id='01a164ec-589c-7b41-bc34-3ba158d1f22b';activated=$true
        attempt=$attempt;operation_set='cold-a-raw-preservation-r43';workspace=$w;origin=$origin;state=$state;backup=$backup;output_parent=$container;output=$proof
        inventory_bytes=6765525;inventory_sha256='c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3';expected_files=6956;expected_payload_bytes=45597399
        work_seconds=45;whole_seconds=60;retry_count=0;lane_count=1})
    Need ($d.allow_output_parent_create -is [bool]) 'decision_parent_class'
    $writes=@('restricted-new-e','restricted-e-receipts','public-aggregate-evidence')
    if($d.allow_output_parent_create){$writes=@('output-parent-create-if-missing')+$writes}
    Array-Exact $d.allowed_writes $writes;Array-Exact $d.public_outputs @('started.json','claim.json','p2.json','parent.json','supervisor.json')
    $caps=Caps;Fields $d.caps @($caps.Keys);Exact $d.caps $caps
    $spec=Pin-Spec;Fields $d.pins @($spec.Keys)
    foreach($key in $spec.Keys){
        $v=Value $d.pins $key;Fields $v @('bytes','sha256')
        Need (($v.bytes -is [int] -or $v.bytes -is [long]) -and $v.bytes -gt 0 -and $v.bytes -le 1048576 -and
            $v.sha256 -is [string] -and $v.sha256 -cmatch '\A[0-9a-f]{64}\z') 'pin_type'
        if($spec[$key][1] -gt 0){Need ($v.bytes -eq $spec[$key][1] -and $v.sha256 -ceq $spec[$key][2]) 'closed_pin'}
        $f=Public-Open $spec[$key][0] 1048576;Need ($f.Length -eq $v.bytes -and (Hash-Stream $f) -ceq $v.sha256) 'source_pin'
        $sourceFiles[$key]=$f;$sourcePins[$key]=$v.sha256
    }
    return $d
}
function Source-Pins {
    $v=[ordered]@{};foreach($key in $sourceFiles.Keys){$v[$key]=Hash-Stream $sourceFiles[$key];Need ($v[$key] -ceq $sourcePins[$key]) 'source_drift'};return $v
}
function Core-Facts {
    if(-not $coreObject){return $null}
    return Decode ($coreObject.PublicObservationUtf8())
}
function Validate-Core($f) {
    if($null -eq $f){return}
    Fields $f @('property','whole_attempt_verdict','continuous_namespace_absence','completed','expected_set_captured_before_hash','observed_entry_sets_equal','input_handles_released','cleanup_unknown','receipts_completed','cleanup_native_error','failure_code','failure_phase','failure_role','cause_kind','native_error','error_hresult','expected_files','expected_bytes','locks','hash_passes','hash_bytes','capture_complete_timestamp','historical_tuple_digest','s_before_tuple_digest','bkp_before_tuple_digest','s_after_tuple_digest','bkp_after_tuple_digest','peak_owned_handles','remaining_owned_handles','peak_path_bytes','peak_record_bytes','peak_inventory_bytes','scratch_reserved_bytes','receipt_bytes_charged','receipt_bytes_known_written','receipt_write_unknown','phase_seconds','private_receipt_digests')
    Exact $f ([ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        expected_files=6956;expected_bytes=45597399;scratch_reserved_bytes=524288})
    foreach($key in @('completed','expected_set_captured_before_hash','observed_entry_sets_equal','input_handles_released','cleanup_unknown','receipts_completed','receipt_write_unknown')){
        Need ((Value $f $key) -is [bool]) 'core_public_type'
    }
    foreach($key in @('remaining_owned_handles','peak_owned_handles','hash_passes','hash_bytes','locks','capture_complete_timestamp','receipt_bytes_charged','receipt_bytes_known_written','peak_path_bytes','peak_record_bytes','peak_inventory_bytes')){
        $v=Value $f $key;Need (($v -is [int] -or $v -is [long]) -and $v -ge 0) 'core_public_type'
    }
    Need ($f.peak_owned_handles -le 40062 -and $f.peak_path_bytes -le 100663296 -and $f.peak_record_bytes -le 33554432 -and $f.peak_inventory_bytes -le 16777216) 'core_public_cap'
    foreach($key in @('historical_tuple_digest','s_before_tuple_digest','bkp_before_tuple_digest','s_after_tuple_digest','bkp_after_tuple_digest')){
        $v=Value $f $key;Need ($null -eq $v -or ($v -is [string] -and $v -cmatch '\A[0-9a-f]{64}\z')) 'core_public_hash'
    }
    foreach($key in @('failure_code','failure_phase','failure_role','cause_kind')){$v=Value $f $key;Need ($null -eq $v -or ($v -is [string] -and $v -cmatch '\A[a-z0-9_]{1,64}\z')) 'core_public_cause'}
    foreach($key in @('native_error','error_hresult','cleanup_native_error')){$v=Value $f $key;Need ($v -is [int] -or $v -is [long]) 'core_public_type'}
    foreach($key in @(Keys $f.phase_seconds)){
        Need ($key -cin @('allocation','output_binding','locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before','hash_bkp_before','hash_s_after','hash_bkp_after','scan_after','final_inputs','receipts')) 'core_public_phase'
        $v=Value $f.phase_seconds $key;Need (($v -is [double] -or $v -is [int] -or $v -is [long]) -and [double]::IsFinite([double]$v) -and $v -ge 0 -and $v -le 60) 'core_public_phase'
    }
    foreach($key in @(Keys $f.private_receipt_digests)){
        Need ($key -cin @('owner.private.json','comparison.private.jsonl','namespace.private.jsonl','result.private.json')) 'core_public_receipt'
        Need ((Value $f.private_receipt_digests $key) -cmatch '\A[0-9a-f]{64}\z') 'core_public_receipt'
    }
}
function Common-Report([string]$Role) {
    return [ordered]@{schema_version=1;classification=('s53-cold-a-'+$Role);attempt=$attempt;decision_sha256=$DecisionSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;whole_attempt_verdict='not_established'}
}
function Report-Fields([string]$Role) {
    $common=@('schema_version','classification','attempt','decision_sha256','start_timestamp','stopwatch_frequency','whole_attempt_verdict')
    $extra=switch -CaseSensitive ($Role) {
        'started' {@('supervisor_pid','supervisor_started_utc','parent_pid','parent_started_utc','source_pins','permission_consumed','work_seconds','whole_seconds')}
        'claim' {@('supervisor_pid','parent_pid','parent_started_utc','started_sha256')}
        'p2' {@('result','failure','cause','source_before','source_after','core','output','parent_guards','compiler','process_checks','managed_public_disposed','managed_public_close_native_observed'
            'public_peak','public_remaining','core_dispose_attempted','wrapper_cleanup','external_bound','interval_ended_before_receipts','p2_qpc')}
        'parent' {@('result','failure','cause','parent_pid','parent_started_utc','p2_sha256','cleanup_known','core_remaining','output_remaining','guard_remaining','public_remaining','public_peak'
            'managed_public_close_native_observed','job_finished','children_after','processes_disposed','observed_seconds')}
        'supervisor' {@('result','failure','cause','parent_pid','parent_started_utc','retained_process_handle','child_exit','child_reaped','stdout_chars','stderr_chars','stdout_eof','stderr_eof','registered_child_released'
            'stdio_disposed','managed_public_disposed','managed_public_close_native_observed','public_peak','public_remaining','forced','cleanup_kill_attempted','cleanup_unknown','observer','completion','wait_trace'
            'source_after','claim_sha256','parent_sha256','p2_sha256','observed_seconds')}
        default {Stop-Cold 'public_role'}
    }
    return @($common)+@($extra)
}
function Validate-Cause($c) {
    if($null -eq $c){return}
    Fields $c @('type','hresult','native_error','marks','chain')
    Need ($c.type -is [string] -and $c.type -cmatch '\A[A-Za-z0-9_.+]{1,128}\z') 'public_cause'
    foreach($k in @('hresult','native_error')){$v=Value $c $k;Need ($v -is [int] -or $v -is [long]) 'public_cause'}
    foreach($k in @(Keys $c.marks)){
        Need ($k -cin @('cold_launch_code','cold_output_code','rootcopies_code','colda_code','colda_phase','colda_role')) 'public_cause'
        $v=Value $c.marks $k;Need ($v -is [string] -and $v -cmatch '\A[a-z0-9_]{1,64}\z') 'public_cause'
    }
    Need (@($c.chain).Count -le 16) 'public_cause'
    foreach($v in $c.chain){Fields $v @('type','hresult');Need ($v.type -is [string] -and $v.type -cmatch '\A[A-Za-z0-9_.+]{1,128}\z' -and ($v.hresult -is [int] -or $v.hresult -is [long])) 'public_cause'}
}
function Validate-SourceMap($m) {
    foreach($k in @(Keys $m)){
        Need (@((Pin-Spec).Keys) -ccontains $k) 'public_source_fields'
        $v=Value $m $k;Need ($v -is [string] -and $v -cmatch '\A[0-9a-f]{64}\z') 'public_source_hash'
    }
}
function Validate-Report([string]$Role,$Object) {
    Fields $Object (Report-Fields $Role)
    Exact $Object ([ordered]@{schema_version=1;classification=('s53-cold-a-'+$Role);attempt=$attempt;decision_sha256=$DecisionSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;whole_attempt_verdict='not_established'})
    if($Role -cin @('started','claim','parent','supervisor')){
        foreach($k in @('parent_pid')){$v=Value $Object $k;Need (($v -is [int] -or $v -is [long]) -and $v -ge 0) 'public_identity'}
        Need ($null -eq $Object.parent_started_utc -or ($Object.parent_started_utc -is [string] -and $Object.parent_started_utc -cmatch '\A[0-9T:.Z-]{20,40}\z')) 'public_identity'
    }
    if($Role -ceq 'started'){Validate-SourceMap $Object.source_pins;Exact $Object ([ordered]@{permission_consumed=$true;work_seconds=45;whole_seconds=60})}
    if($Role -cin @('p2','parent','supervisor')){
        Need ($Object.result -cin @('raw_preservation_observed','completed','failed_or_unknown')) 'public_result'
        Need ($null -eq $Object.failure -or ($Object.failure -is [string] -and $Object.failure -cmatch '\A[a-z0-9_]{1,64}\z')) 'public_failure'
        Fields $Object.cause @('primary','cleanup');Validate-Cause $Object.cause.primary;Validate-Cause $Object.cause.cleanup
    }
    if($Role -ceq 'p2'){
        Validate-Core $Object.core;Need ($Object.external_bound -eq 59) 'control_reserve'
        Validate-SourceMap $Object.source_before;Validate-SourceMap $Object.source_after
        Fields $Object.output @('created','transferred','ancestry','prepared','remaining','receipt_mask','receipt_transfers','cleanup_unknown','native_error','disposed','partial','retained_if_created')
        Fields $Object.parent_guards @('parent_created','parent_existing_verified','acl_rechecked','remaining','peak','open_attempts','native_close_attempts','native_closed','cleanup_unknown','native_error','disposed')
        foreach($k in @('created','transferred','cleanup_unknown','disposed','partial','retained_if_created')){Need ((Value $Object.output $k) -is [bool]) 'public_output'}
        foreach($k in @('parent_created','parent_existing_verified','acl_rechecked','cleanup_unknown','disposed')){Need ((Value $Object.parent_guards $k) -is [bool]) 'public_guard'}
        foreach($k in @('ancestry','prepared','remaining','receipt_mask','receipt_transfers','native_error')){Need ((Value $Object.output $k) -is [int]) 'public_output'}
        foreach($k in @('remaining','peak','open_attempts','native_close_attempts','native_closed','native_error')){Need ((Value $Object.parent_guards $k) -is [int]) 'public_guard'}
        Fields $Object.compiler @('failed','diagnostics','fresh_types','initializers')
        Need ($Object.compiler.failed -is [bool] -and @($Object.compiler.diagnostics).Count -le 16) 'public_compiler'
        foreach($v in $Object.compiler.diagnostics){Fields $v @('type','error');Need ($v.type -is [string] -and $v.type -cmatch '\A[A-Za-z0-9_.+]{1,128}\z' -and $v.error -is [bool]) 'public_compiler'}
        foreach($k in @(Keys $Object.compiler.fresh_types)){Need ($k -cin @('deadline','core','output','root_native','parent_guards') -and (Value $Object.compiler.fresh_types $k) -is [bool]) 'public_compiler'}
        foreach($k in @(Keys $Object.compiler.initializers)){Need ($k -cin @('deadline','Initialize-S53ColdPreservationCore','Initialize-S53ColdOutputAdapter','Initialize-S53RootNative','Initialize-ProductionGuards')) 'public_compiler';Exact $Object.compiler.initializers ([ordered]@{$k=1})}
        if($Object.wrapper_cleanup){
            Fields $Object.wrapper_cleanup @('catch_qpc','pending_observed','pending_closed','pending_invalid','before_observed','before_owned','before_unknown','before_native','before_failure','before_phase','before_role'
                'core_dispose_qpc','core_disposed','after_observed','after_owned','after_unknown','after_native','fallback_needed','fallback_used','fallback_skipped_unknown','pending_closed_after'
                'cleanup_unknown','native_close_unknown','secondary_type','secondary_hresult','diagnostics_complete')
        }
    }
    if($Role -ceq 'supervisor'){
        Need ($Object.stdout_chars -is [int] -and $Object.stderr_chars -is [int] -and $Object.stdout_chars -ge 0 -and $Object.stdout_chars -le 4096 -and $Object.stderr_chars -ge 0 -and $Object.stderr_chars -le 4096) 'public_stdio'
        Validate-SourceMap $Object.source_after
    }
}
function Write-To([IO.FileStream]$File,[string]$Role,$Object) {
    Check;Validate-Report $Role $Object
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Object -Depth 12 -Compress))
    Need ($File.Length -eq 0 -and $raw.Length -le 65536) 'public_output_cap'
    $File.Write($raw,0,$raw.Length);$File.Flush($true)
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant()
}
function Write-Report([string]$Role,$Object) {
    Need ($Role -cin @('started','claim','p2','parent','supervisor')) 'public_role'
    $file=Public-Open "$e\$attempt.$Role.json" 65536 $true
    try{return Write-To $file $Role $Object}finally{Public-Close $file}
}
function Read-Report([string]$Role) {
    Need ($Role -cin @('started','claim','p2','parent')) 'public_role'
    $file=Public-Open "$e\$attempt.$Role.json" 65536
    $object=Decode (Read-Bytes $file 65536);Validate-Report $Role $object
    return [pscustomobject]@{Data=$object;Sha256=(Hash-Stream $file)}
}
function Initialize-Reviewed([string]$Name,[scriptblock]$Body){
    Need (-not $initializers.Contains($Name)) 'cached_type';$initializers[$Name]=1
    try {
        & $Body *>&1 | ForEach-Object {
            Check;Need ($compilerDiagnostics.Count -lt 16) 'compiler_diagnostic_cap'
            # No raw compiler/error text in a public report, including source-line literals.
            $kind=$_.GetType().FullName;Need ($kind.Length -le 128) 'compiler_diagnostic_cap'
            $compilerDiagnostics.Add([ordered]@{type=$kind;error=($_ -is [Management.Automation.ErrorRecord])})
        }
    }catch{$script:compilerFailed=$true;throw}
    Need ($compilerDiagnostics.Count -eq 0) 'compiler_diagnostics'
}
function Wrapper-Cleanup($Exception) {
    $keys=@('catch_qpc','pending_observed','pending_closed','pending_invalid','before_observed','before_owned','before_unknown','before_native','before_failure','before_phase','before_role'
        'core_dispose_qpc','core_disposed','after_observed','after_owned','after_unknown','after_native','fallback_needed','fallback_used','fallback_skipped_unknown','pending_closed_after'
        'cleanup_unknown','native_close_unknown','secondary_type','secondary_hresult','diagnostics_complete')
    $depth=0
    for($at=$Exception;$at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        if(-not $at.Data.Contains('s53_wrapper_diagnostics_complete')){continue}
        $v=[ordered]@{}
        foreach($key in $keys){
            if(-not $at.Data.Contains('s53_wrapper_'+$key)){return $null}
            $item=$at.Data['s53_wrapper_'+$key]
            if($null -ne $item -and $item -isnot [bool] -and $item -isnot [int] -and $item -isnot [long] -and -not ($item -is [string] -and $item.Length -le 128)){return $null}
            $v[$key]=$item
        }
        return $v
    }
    return $null
}
function Initialize-ProductionGuards {
    [CmdletBinding()]
    param()
    if('Kilogram.S53.ColdLaunch.ParentGuards' -as [type]){throw 'Cold parent guard type cached.'}
    $source=@'
using System;
using System.IO;
using System.Text;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using Microsoft.Win32.SafeHandles;
namespace Kilogram.S53.ColdLaunch {
public sealed class ParentGuards : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    }
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr h);
    const string Parent=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence";
    readonly string[] paths={@"E:\",@"E:\AI-Projects",@"E:\AI-Projects\kilogram-p2p-messenger",
        @"E:\AI-Projects\kilogram-p2p-messenger\.tmp",Parent};
    readonly SafeFileHandle[] held=new SafeFileHandle[5];
    readonly Info[] ids=new Info[5];
    readonly string[] acls=new string[5];
    readonly long deadline;
    readonly Func<string,SafeFileHandle> open;
    readonly Action<string> create,verifyNew;
    bool started,disposed,prepared;
    public bool ParentCreated {get;private set;}
    public bool ParentExisted {get;private set;}
    public bool AclRechecked {get;private set;}
    public bool CleanupUnknown {get;private set;}
    public int CleanupNativeError {get;private set;}
    public int Remaining {get;private set;}
    public int Peak {get;private set;}
    public int OpenAttempts {get;private set;}
    public int NativeCloseAttempts {get;private set;}
    public int NativeClosed {get;private set;}
    public ParentGuards(long workDeadline,Func<string,SafeFileHandle> directory,Action<string> newDirectory,Action<string> newAcl) {
        if(workDeadline<=Stopwatch.GetTimestamp()||directory==null||newDirectory==null||newAcl==null)throw new ArgumentException("Fixed parent guard binding.");
        deadline=workDeadline;open=directory;create=newDirectory;verifyNew=newAcl;
    }
    static Exception Refuse(string code) {
        var e=new InvalidOperationException("Cold launcher bounded refusal");e.Data["cold_launch_code"]=code;return e;
    }
    void Work(){if(disposed||CleanupUnknown)throw Refuse("parent_cleanup_unknown");if(Stopwatch.GetTimestamp()>=deadline)throw Refuse("work_expired");}
    static int NativeCode(Exception e) {
        for(int i=0;e!=null&&i<16;i++,e=e.InnerException)if(e is Win32Exception w)return w.NativeErrorCode;
        return 0;
    }
    Info Read(SafeFileHandle h) {
        Work();if(h==null||h.IsClosed||h.IsInvalid)throw Refuse("parent_handle");
        Info value;if(!GetFileInformationByHandle(h,out value))throw new Win32Exception(Marshal.GetLastWin32Error());
        if((value.Attributes&(0x400u|0x40u))!=0||(value.Attributes&16u)==0)throw Refuse("parent_shape");
        return value; // frozen opener already checked the exact final path/no-follow shape
    }
    static bool Same(Info a,Info b){return a.Volume==b.Volume&&a.IndexHigh==b.IndexHigh&&a.IndexLow==b.IndexLow;}
    string Acl(int i) {
        Work();string s=FileSystemAclExtensions.GetAccessControl(new DirectoryInfo(paths[i])).GetSecurityDescriptorSddlForm(
            AccessControlSections.Owner|AccessControlSections.Group|AccessControlSections.Access);
        if(s.Length>16384)throw Refuse("parent_acl_cap");Work();return s;
    }
    bool Close(SafeFileHandle h) {
        if(h==null)return true;
        try {
            if(h.IsClosed||h.IsInvalid){CleanupUnknown=true;return false;}
            NativeCloseAttempts++;bool ok=CloseHandle(h.DangerousGetHandle());int error=ok?0:Marshal.GetLastWin32Error();
            h.SetHandleAsInvalid();h.Dispose(); // one observed native close, never retry an uncertain result
            if(ok){Remaining--;NativeClosed++;}else{CleanupUnknown=true;if(CleanupNativeError==0)CleanupNativeError=error;}
            return ok;
        }catch{CleanupUnknown=true;return false;}
    }
    void Open(int i) {
        Work();if(Remaining>=5)throw Refuse("parent_handle_cap");
        Remaining++;Peak=Math.Max(Peak,Remaining);OpenAttempts++;SafeFileHandle h=null;bool transferred=false,missing=false;
        try {
            try{h=open(paths[i]);}catch(Exception e){int error=NativeCode(e);missing=error==2||error==3;if(!missing)CleanupUnknown=true;throw;}
            ids[i]=Read(h);acls[i]=Acl(i);held[i]=h;h=null;transferred=true;
        }finally{
            if(!transferred){if(h!=null)Close(h);else if(missing)Remaining--;else CleanupUnknown=true;}
        }
    }
    public void Prepare(bool allowCreate) {
        Work();if(started)throw Refuse("parent_one_use");started=true;
        for(int i=0;i<4;i++)Open(i); // no query of the parent leaf before all four prefixes are held
        try {Open(4);ParentExisted=true;}
        catch(Exception e) {
            int error=NativeCode(e);if(error!=2&&error!=3)throw;
            if(!allowCreate){var failure=new InvalidOperationException("Cold launcher bounded refusal",e);failure.Data["cold_launch_code"]="parent_missing";throw failure;}
            Work();create(Parent);ParentCreated=true; // immediate truthful ownership before later validation/open
            Open(4);verifyNew(Parent);
        }
        prepared=true;Recheck();
    }
    public void Recheck() {
        Work();if(!prepared)throw Refuse("parent_not_prepared");
        for(int i=0;i<5;i++)if(!Same(ids[i],Read(held[i]))||acls[i]!=Acl(i))throw Refuse("parent_changed");
        AclRechecked=true;
    }
    public void Dispose() {
        if(disposed)return;
        for(int i=4;i>=0;i--)if(held[i]!=null){Close(held[i]);held[i]=null;}
        disposed=true;if(Remaining!=0)CleanupUnknown=true;
    }
}
}
'@
    Add-Type -TypeDefinition $source -Language CSharp
}
function New-Drain([IO.StreamReader]$Reader) {
    $buffer=[char[]]::new(1024)
    return [pscustomobject]@{Reader=$Reader;Buffer=$buffer;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($buffer,0,$buffer.Length);Eof=$false;Cap=$false}
}
function Pump($Drain) {
    if(-not $Drain -or $Drain.Eof -or -not $Drain.Task.IsCompleted){return}
    $n=$Drain.Task.GetAwaiter().GetResult()
    if($n -eq 0){$Drain.Eof=$true;Note-EOF;return}
    if($Drain.Text.Length+$n -gt 4096){$Drain.Cap=$true;Stop-Cold 'output_cap'}
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
        if($Stage -ceq 'normal'){Set-FirstReason 'unreaped_work_timeout';Stop-Cold 'unreaped_work_timeout'}
        return
    }
    $milliseconds=[int][Math]::Min(20,$remaining)
    $before=[Diagnostics.Stopwatch]::GetTimestamp();$waited=$child.WaitForExit($milliseconds);$after=[Diagnostics.Stopwatch]::GetTimestamp()
    Record-Wait $waited $Stage $milliseconds $before $after
}
function Sample-Child {
    Check;Pump $out;Pump $err
    if($metrics.stopped_for_exit){return}
    try{$alreadyExited=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='pre_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-Cold 'observer_failure'}
    if($alreadyExited){Note-ExitIndication 'has_exited';$metrics.known_exited_skips++;$metrics.stopped_for_exit=$true;return}
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    if($now -lt $nextSampleQpc){return}
    Need ($metrics.sample_attempts -lt 601) 'observer_cap'
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
        Stop-Cold 'observer_failure'
    }
    if($samplePid -ne $childId -or $sampleHandle -ne $retainedProcessHandle -or $sampleStarted -cne $childStartedUtc -or $handles -lt 0 -or $working -le 0){
        $metrics.observer_failed=$true;$metrics.failure_api='identity_or_range';Stop-Cold 'observer_identity'
    }
    Need ($handles -le 45000 -and $working -le 1073741824L) 'observer_resource_cap'
    try{$exitedAfterRead=$child.HasExited}catch{$metrics.observer_failed=$true;$metrics.failure_api='post_query_exit';$metrics.failure_cause=Cause $_.Exception;Stop-Cold 'observer_failure'}
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
function Output-Facts {
    if(-not $outputOwner){return [ordered]@{created=$false;transferred=$false;ancestry=0;prepared=0;remaining=0;receipt_mask=0;receipt_transfers=0;cleanup_unknown=$false;native_error=0;disposed=$false;partial=$false;retained_if_created=$false}}
    return [ordered]@{created=$outputOwner.DirectoryCreated;transferred=$outputOwner.DirectoryTransferred;ancestry=$outputOwner.AncestryCount;prepared=$preparedHandles
        remaining=$outputOwner.RemainingOwnedHandles;receipt_mask=$outputOwner.ReceiptCreatedMask;receipt_transfers=$outputOwner.ReceiptsTransferred
        cleanup_unknown=$outputOwner.CleanupUnknown;native_error=$outputOwner.CleanupNativeError;disposed=$ownerDisposed;partial=[bool]($outputOwner.DirectoryCreated -and (-not $coreFinal -or -not $coreFinal.receipts_completed));retained_if_created=$outputOwner.DirectoryCreated}
}
function Parent-Facts {
    if(-not $parentGuards){return [ordered]@{parent_created=$false;parent_existing_verified=$false;acl_rechecked=$false;remaining=0;peak=0;open_attempts=0;native_close_attempts=0;native_closed=0;cleanup_unknown=$false;native_error=0;disposed=$false}}
    return [ordered]@{parent_created=$parentGuards.ParentCreated;parent_existing_verified=$parentGuards.ParentExisted;acl_rechecked=$parentGuards.AclRechecked
        remaining=$parentGuards.Remaining;peak=$parentGuards.Peak;open_attempts=$parentGuards.OpenAttempts;native_close_attempts=$parentGuards.NativeCloseAttempts
        native_closed=$parentGuards.NativeClosed;cleanup_unknown=$parentGuards.CleanupUnknown;native_error=$parentGuards.CleanupNativeError;disposed=$guardsDisposed}
}
function Process-Check {
    Check;$list=[Diagnostics.Process]::GetProcesses()
    try {
        Need ($list.Length -le 4096) 'process_cap'
        foreach($p in $list){Check;Need ($p.ProcessName -notlike 'kilogram-*') 'conflicting_process'}
        $script:processChecks++
    } finally {
        foreach($p in $list){try{$p.Dispose()}catch{$script:processDisposed=$false}}
    }
}
function Await-Start {
    while($true){
        Check
        try{
            $f=Public-Open "$e\$attempt.started.json" 65536
            $data=Decode (Read-Bytes $f 65536);Validate-Report 'started' $data
            Exact $data ([ordered]@{parent_pid=$self.Id;parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');permission_consumed=$true;work_seconds=45;whole_seconds=60})
            Need (($data.supervisor_pid -is [int] -or $data.supervisor_pid -is [long]) -and $data.supervisor_pid -gt 0 -and $data.supervisor_pid -le [int]::MaxValue -and $data.supervisor_pid -ne $self.Id) 'owned_start'
            Fields $data.source_pins @($sourcePins.Keys);Exact $data.source_pins $sourcePins
            return [pscustomobject]@{Data=$data;Sha256=(Hash-Stream $f)}
        }catch{
            $c=Cause $_.Exception
            if($c.native_error -ne 32 -and -not ($c.type -ceq 'System.IO.IOException' -and $c.hresult -eq -2147024864)){throw} # only the retained Supervisor start writer can still be publishing
            $remaining=[Math]::Floor((45-(Age))*1000);Need ($remaining -ge 1) 'work_expired'
            [Threading.Thread]::Sleep([int][Math]::Min(5,$remaining))
        }
    }
}
function Invoke-Parent {
    $authorized=$false;$claimed=$false;$failure=$null;$failureCause=$null;$cleanupCause=$null;$candidate=$false;$result=1
    $sourceBefore=[ordered]@{};$sourceAfter=[ordered]@{};$claim=$null;$start=$null;$jobFinished=$false;$childrenAfter=$null;$p2Hash=$null
    $script:coreObject=$null;$script:outputOwner=$null;$script:parentGuards=$null;$script:preparedHandles=0;$script:ownerDisposed=$false;$script:guardsDisposed=$false
    $script:processChecks=0;$script:processDisposed=$true
    $coreDisposeAttempted=$false;$bindingFailed=$false;$wrapperCleanup=$null;$coreFinal=$null;$fresh=[ordered]@{};$receiptStarted=$false
    $script:compilerDiagnostics=[Collections.Generic.List[object]]::new();$script:compilerFailed=$false;$script:initializers=[ordered]@{}
    try {
        Need ($StartTimestamp -gt 0) 'internal_parent_only'
        $decision=Validate-Decision;$authorized=$true;$sourceBefore=Source-Pins
        $start=Await-Start # actual retained child PID+UTC is bound by Supervisor, not merely Mode=Parent
        $fresh.deadline=$null -eq ('KilogramS5Deadline' -as [type]);Need $fresh.deadline 'cached_type'
        Initialize-Reviewed 'deadline' {. $deadlinePath}
        $script:guard=[KilogramS5Deadline]::new((45-(Age)),(50-(Age)));Check
        $claim=Common-Report 'claim';$claim.supervisor_pid=$start.Data.supervisor_pid;$claim.parent_pid=$self.Id
        $claim.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');$claim.started_sha256=$start.Sha256
        $null=Write-Report 'claim' $claim;$claimed=$true
        $types=[ordered]@{core='Kilogram.S53.ColdA.Core';output='Kilogram.S53.ColdOutput.BaselineSmallOwner';root_native='S53RootNativeR38';parent_guards='Kilogram.S53.ColdLaunch.ParentGuards'}
        foreach($key in $types.Keys){$fresh[$key]=$null -eq ($types[$key] -as [type]);Need $fresh[$key] 'cached_type'}
        . $corePath
        . $outputPath
        . $rootNative
        foreach($name in @('Initialize-S53ColdPreservationCore','Initialize-S53ColdOutputAdapter','Initialize-S53RootNative','Initialize-ProductionGuards')){
            Initialize-Reviewed $name ([scriptblock]::Create($name))
        }
        Need ([Threading.Thread]::CurrentThread.GetApartmentState().ToString() -ceq 'MTA') 'apartment'
        Process-Check
        $script:parentGuards=[Kilogram.S53.ColdLaunch.ParentGuards]::new($workDeadline,
            [Func[string,Microsoft.Win32.SafeHandles.SafeFileHandle]]{param($p) [S53RootNativeR38]::DirectoryHandle($p)},
            [Action[string]]{param($p) [S53RootNativeR38]::NewDirectory($p)},
            [Action[string]]{param($p) Assert-S53RootPrivateAcl $p $true})
        $parentGuards.Prepare($decision.allow_output_parent_create)
        $script:coreObject=New-S53ColdAProductionCore -WorkDeadlineTimestamp $workDeadline
        $script:outputOwner=[Kilogram.S53.ColdOutput.BaselineSmallOwner]::CreateProduction($workDeadline)
        $outputOwner.PrepareDirectory();$script:preparedHandles=$outputOwner.RemainingOwnedHandles
        Need ($outputOwner.AncestryCount -eq 5 -and $preparedHandles -eq 6) 'production_output_geometry'
        $parentGuards.Recheck();$parentGuards.Dispose();$script:guardsDisposed=$true
        Need (-not $parentGuards.CleanupUnknown -and $parentGuards.Remaining -eq 0) 'parent_cleanup_unknown'
        try{Bind-S53ColdBaselineSmallOutput -Core $coreObject -Owner $outputOwner -DecisionSha256 $DecisionSha256 -CoreSha256 $sourcePins.core}
        catch{$bindingFailed=$true;$wrapperCleanup=Wrapper-Cleanup $_.Exception;throw}
        Need ($outputOwner.RemainingOwnedHandles -eq 5) 'output_transfer'
        $coreObject.CaptureAndCompare()
        $facts=Core-Facts;Validate-Core $facts
        Exact $facts ([ordered]@{completed=$true;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$true;input_handles_released=$true
            cleanup_unknown=$false;failure_code=$null;hash_passes=4;hash_bytes=182389596;locks=6})
        # CaptureAndCompare has released inputs. Writing these retained observations does not extend that interval.
        $receiptStarted=$true;$coreObject.WriteReceipts();$outputOwner.RecheckAncestry();Process-Check
        $sourceAfter=Source-Pins;Check;$candidate=$true
    }catch{$failureCause=Cause $_.Exception;$failure=if($failureCause.marks.Contains('cold_launch_code')){$failureCause.marks.cold_launch_code}else{'private_failed_or_unknown'}}
    finally {
        $script:cleanupReserve=$true
        # Binding wrapper owns failure cleanup after transfer, including uncertain native outcomes. Never retry it.
        if($coreObject -and -not ($bindingFailed -and $outputOwner -and $outputOwner.DirectoryTransferred)){
            try{$coreDisposeAttempted=$true;$coreObject.Dispose()}catch{$candidate=$false;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}
        }elseif($wrapperCleanup){$coreDisposeAttempted=[bool]$wrapperCleanup.core_disposed}
        if($coreObject){try{$coreFinal=Core-Facts;Validate-Core $coreFinal}catch{$candidate=$false;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}}
        try{if($outputOwner){$outputOwner.Dispose();$script:ownerDisposed=$true}}catch{$candidate=$false;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}
        try{if($parentGuards){$parentGuards.Dispose();$script:guardsDisposed=$true}}catch{$candidate=$false;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}
        Public-CloseAll
        if($guard){try{$guard.CleanupChildren([int][Math]::Max(0,[Math]::Min(500,(50-(Age))*1000)));$childrenAfter=@($guard.Children()).Count;Need ($childrenAfter -eq 0) 'owned_children';$guard.Finish();$jobFinished=$true}
            catch{$candidate=$false;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}}
    }
    try {
        Check;$o=Output-Facts;$g=Parent-Facts
        $known=$coreFinal -and $coreFinal.remaining_owned_handles -eq 0 -and -not $coreFinal.cleanup_unknown -and
            $o.remaining -eq 0 -and -not $o.cleanup_unknown -and $g.remaining -eq 0 -and -not $g.cleanup_unknown -and
            -not $managedCloseUnknown -and $publicOwned -eq 0 -and $processDisposed -and $jobFinished -and $childrenAfter -eq 0 -and $null -eq $cleanupCause
        if($bindingFailed -and $outputOwner -and $outputOwner.DirectoryTransferred -and (-not $wrapperCleanup -or -not $wrapperCleanup.diagnostics_complete)){$known=$false}
        if($wrapperCleanup -and ($wrapperCleanup.cleanup_unknown -or $wrapperCleanup.native_close_unknown)){$known=$false}
        $candidate=$candidate -and $known -and $coreFinal.completed -and $coreFinal.receipts_completed -and $coreFinal.hash_bytes -eq 182389596
        if(-not $candidate -and -not $failure){$failure='cleanup_or_evidence_unknown'}
        if($authorized -and $claimed){
            $p2=Common-Report 'p2';$p2.result=if($candidate){'raw_preservation_observed'}else{'failed_or_unknown'};$p2.failure=$failure
            $p2.cause=[ordered]@{primary=$failureCause;cleanup=$cleanupCause};$p2.source_before=$sourceBefore;$p2.source_after=$sourceAfter;$p2.core=$coreFinal
            $p2.output=$o;$p2.parent_guards=$g;$p2.compiler=[ordered]@{failed=$compilerFailed;diagnostics=$compilerDiagnostics.ToArray();fresh_types=$fresh;initializers=$initializers}
            $p2.process_checks=$processChecks;$p2.managed_public_disposed=(-not $managedCloseUnknown -and $publicOwned -eq 0);$p2.managed_public_close_native_observed=$false
            $p2.public_peak=$publicPeak;$p2.public_remaining=$publicOwned;$p2.core_dispose_attempted=$coreDisposeAttempted;$p2.wrapper_cleanup=$wrapperCleanup
            $p2.external_bound=59;$p2.interval_ended_before_receipts=[bool]($receiptStarted -and $coreFinal -and $coreFinal.input_handles_released);$p2.p2_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
            $p2Hash=Write-Report 'p2' $p2
            $parent=Common-Report 'parent';$parent.result=if($candidate){'completed'}else{'failed_or_unknown'};$parent.failure=$failure
            $parent.cause=[ordered]@{primary=$failureCause;cleanup=$cleanupCause};$parent.parent_pid=$self.Id;$parent.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o')
            $parent.p2_sha256=$p2Hash;$parent.cleanup_known=[bool]$known;$parent.core_remaining=if($coreFinal){$coreFinal.remaining_owned_handles}else{$null}
            $parent.output_remaining=$o.remaining;$parent.guard_remaining=$g.remaining;$parent.public_remaining=$publicOwned;$parent.public_peak=$publicPeak
            $parent.managed_public_close_native_observed=$false;$parent.job_finished=$jobFinished;$parent.children_after=$childrenAfter
            $parent.processes_disposed=$processDisposed;$parent.observed_seconds=Age
            $null=Write-Report 'parent' $parent
        }
        Public-CloseAll;Check
        if($candidate -and $p2Hash -and $publicOwned -eq 0 -and -not $managedCloseUnknown){$result=0}
    }catch{if(-not $failureCause){$failureCause=Cause $_.Exception};$result=1}
    if($result -ne 0){[Console]::Error.WriteLine('{"classification":"s53-cold-a-parent-failed","result":"failed_or_unknown"}')}
    return $result
}
function Check-ChildReports($Claim,$P2,$Parent) {
    $c=$Claim.Data;$p=$P2.Data;$r=$Parent.Data
    Exact $c ([ordered]@{supervisor_pid=$self.Id;parent_pid=$childId;parent_started_utc=$childStartedUtc;started_sha256=$startedHash})
    Exact $r ([ordered]@{parent_pid=$childId;parent_started_utc=$childStartedUtc;result='completed';failure=$null;p2_sha256=$P2.Sha256
        cleanup_known=$true;core_remaining=0;output_remaining=0;guard_remaining=0;public_remaining=0;managed_public_close_native_observed=$false
        job_finished=$true;children_after=0;processes_disposed=$true})
    Exact $p ([ordered]@{result='raw_preservation_observed';failure=$null;managed_public_disposed=$true;managed_public_close_native_observed=$false
        public_remaining=0;core_dispose_attempted=$true;external_bound=59;interval_ended_before_receipts=$true;wrapper_cleanup=$null})
    Fields $p.cause @('primary','cleanup');Exact $p.cause ([ordered]@{primary=$null;cleanup=$null})
    Fields $r.cause @('primary','cleanup');Exact $r.cause ([ordered]@{primary=$null;cleanup=$null})
    Exact $p.core ([ordered]@{completed=$true;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$true;input_handles_released=$true
        cleanup_unknown=$false;receipts_completed=$true;failure_code=$null;failure_phase=$null;failure_role=$null;cause_kind=$null
        native_error=0;error_hresult=0;cleanup_native_error=0;locks=6;hash_passes=4;hash_bytes=182389596;remaining_owned_handles=0})
    Need (@(Keys $p.core.private_receipt_digests).Count -eq 4 -and $p.core.receipt_bytes_known_written -eq $p.core.receipt_bytes_charged) 'receipt_accounting'
    Fields $p.output @('created','transferred','ancestry','prepared','remaining','receipt_mask','receipt_transfers','cleanup_unknown','native_error','disposed','partial','retained_if_created')
    Exact $p.output ([ordered]@{created=$true;transferred=$true;ancestry=5;prepared=6;remaining=0;receipt_mask=15;receipt_transfers=4;cleanup_unknown=$false;native_error=0;disposed=$true;partial=$false;retained_if_created=$true})
    Fields $p.parent_guards @('parent_created','parent_existing_verified','acl_rechecked','remaining','peak','open_attempts','native_close_attempts','native_closed','cleanup_unknown','native_error','disposed')
    Exact $p.parent_guards ([ordered]@{acl_rechecked=$true;remaining=0;peak=5;native_close_attempts=5;native_closed=5;cleanup_unknown=$false;native_error=0;disposed=$true})
    Need ($p.parent_guards.parent_created -is [bool] -and $p.parent_guards.parent_existing_verified -is [bool] -and
        $p.parent_guards.parent_created -ne $p.parent_guards.parent_existing_verified) 'parent_ownership'
    Fields $p.compiler @('failed','diagnostics','fresh_types','initializers')
    Exact $p.compiler ([ordered]@{failed=$false});Need (@($p.compiler.diagnostics).Count -eq 0) 'compiler_diagnostics'
    Exact $p.compiler.fresh_types ([ordered]@{deadline=$true;core=$true;output=$true;root_native=$true;parent_guards=$true})
    foreach($key in @('deadline','Initialize-S53ColdPreservationCore','Initialize-S53ColdOutputAdapter','Initialize-S53RootNative','Initialize-ProductionGuards')){Exact $p.compiler.initializers ([ordered]@{$key=1})}
    Need ($p.process_checks -eq 2 -and $p.public_peak -le 24 -and $r.public_peak -le 24 -and $r.observed_seconds -le 50) 'parent_resources'
    foreach($key in $sourcePins.Keys){Exact $p.source_before ([ordered]@{$key=$sourcePins[$key]});Exact $p.source_after ([ordered]@{$key=$sourcePins[$key]})}
    Need ($p.p2_qpc -ge $p.core.capture_complete_timestamp -and $p.p2_qpc -lt ($StartTimestamp+50L*[Diagnostics.Stopwatch]::Frequency)) 'child_clock'
}
function Invoke-Supervisor {
    $authorized=$false;$claimed=$false;$startFile=$null;$startedHash=$null;$failure=$null;$failureCause=$null;$cleanupFirstCause=$null;$normalCandidate=$false;$good=$false
    $script:child=$null;$childStarted=$false;$script:childId=0;$script:childStartedUtc=$null;$script:retainedProcessHandle=0L;$registeredChildReleased=$false
    $script:out=$null;$script:err=$null;$script:reaped=$false;$script:childExit=$null;$stdioDisposed=$false;$cleanupKillAttempted=$false
    $script:exitQpc=0L;$script:reapQpc=0L;$script:eofQpc=0L;$script:nextSampleQpc=0L;$script:normalStage='bootstrap'
    $script:waitTrace=[Collections.Generic.List[object]]::new()
    $script:metrics=[ordered]@{nominal_interval_ms=100;sample_attempts=0;sample_count=0;first_live_qpc=0L;last_live_qpc=0L;first_sample_started_qpc=0L;last_sample_started_qpc=0L
        maximum_successful_sample_gap_seconds=0.0;maximum_measurement_gap_upper_seconds=0.0;maximum_sample_duration_seconds=0.0;total_sample_duration_seconds=0.0
        sampled_peak_os_handles=0;observed_peak_working_set_bytes=0L;known_exited_skips=0;stopped_for_exit=$false;exit_race_count=0;exit_race_api=$null
        exit_race_cause=$null;exit_race_qpc=0L;exit_race_confirmed_exited=$false;observer_failed=$false;failure_api=$null;failure_cause=$null;failure_confirmed_exited=$false
        sampled_handle_peak_can_be_missed=$true;unobserved_exit_tail=$true}
    $script:completion=[ordered]@{exit_indication_basis=$null;first_reap_stage=$null;normal_wait_attempts=0;normal_wait_true=0;normal_wait_false=0;normal_first_wait_seen=$false
        cleanup_wait_attempts=0;cleanup_wait_true=0;cleanup_wait_false=0;cleanup_first_wait_seen=$false;false_after_exit_seen=$false;false_after_eof_seen=$false
        last_wait_qpc=0L;last_wait_result=$null;normal_gate=$null;normal_gate_passed=$false;pre_cleanup_gate=$null;pre_cleanup_first_reason=$null;first_failure_reason=$null}
    $sourceAfter=[ordered]@{};$claimReport=$null;$p2Report=$null;$parentReport=$null;$result=1
    try {
        Need ($StartTimestamp -gt 0) 'clock'
        $decision=Validate-Decision;$authorized=$true
        foreach($role in @('started','claim','p2','parent','supervisor')){Need (-not [IO.File]::Exists("$e\$attempt.$role.json") -and -not [IO.Directory]::Exists("$e\$attempt.$role.json")) 'public_collision'}
        # CreateNew consumes the attempt BEFORE spawn, but publish its body only after retained child identity is known.
        $startFile=Public-Open "$e\$attempt.started.json" 65536 $true;$claimed=$true
        $info=[Diagnostics.ProcessStartInfo]::new($fixedShell);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WorkingDirectory=$w
        $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
        foreach($a in @('-NoLogo','-NoProfile','-NonInteractive','-Mta','-File',$entry,'-DecisionSha256',$DecisionSha256,'-Mode','Parent','-StartTimestamp',$StartTimestamp.ToString([Globalization.CultureInfo]::InvariantCulture))){$info.ArgumentList.Add($a)}
        $script:child=[Diagnostics.Process]::new();$child.StartInfo=$info;$control.Child=$child;Check
        Need ($child.Start()) 'child_start';$childStarted=$true
        $script:childId=$child.Id;$script:childStartedUtc=$child.StartTime.ToUniversalTime().ToString('o');$script:retainedProcessHandle=$child.Handle.ToInt64()
        $child.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
        $script:out=New-Drain $child.StandardOutput;$script:err=New-Drain $child.StandardError
        $start=Common-Report 'started';$start.supervisor_pid=$self.Id;$start.supervisor_started_utc=$self.StartTime.ToUniversalTime().ToString('o')
        $start.parent_pid=$childId;$start.parent_started_utc=$childStartedUtc;$start.source_pins=$sourcePins
        $start.permission_consumed=$true;$start.work_seconds=45;$start.whole_seconds=60
        $startedHash=Write-To $startFile 'started' $start;Public-Close $startFile;$startFile=$null
        $script:normalStage='completion'
        while($true){
            Check;Pump $out;Pump $err;Sample-Child;Check;Pump $out;Pump $err;Note-EOF
            if($child.HasExited){Note-ExitIndication 'has_exited'}
            Need (-not $control.Forced -and -not $control.Unknown) 'forced_or_unknown'
            if(-not $reaped){Wait-ForReap 'normal' 45}
            Check;Pump $out;Pump $err;Note-EOF
            if($reaped -and $childExit -ne 0){Stop-Cold 'child_exit_nonzero'}
            if($reaped -and $out.Eof -and $err.Eof){break}
            if($reaped){$left=[Math]::Floor((45-(Age))*1000);if($left -ge 1){[Threading.Thread]::Sleep([int][Math]::Min(5,$left))}}
        }
        $completion.normal_gate=Completion-Gate
        Need ($reaped -and $childExit -eq 0 -and $out.Eof -and $err.Eof -and $reapQpc -lt $workDeadline -and $eofQpc -lt $workDeadline -and
            -not $control.Forced -and -not $control.Unknown -and -not $metrics.observer_failed -and $metrics.sample_count -gt 0) 'completion_gate'
        $completion.normal_gate_passed=$true
        $script:normalStage='reports'
        $claimReport=Read-Report 'claim';$p2Report=Read-Report 'p2';$parentReport=Read-Report 'parent'
        Check-ChildReports $claimReport $p2Report $parentReport
        Need ($out.Text.Length -eq 0 -and $err.Text.Length -eq 0 -and -not $out.Cap -and -not $err.Cap) 'child_output'
        $sourceAfter=Source-Pins;Check;$normalCandidate=$true;$script:normalStage='completed'
    }catch{
        $failureCause=Cause $_.Exception;$failure=if($failureCause.marks.Contains('cold_launch_code')){$failureCause.marks.cold_launch_code}else{'supervisor_failed_or_unknown'}
        Set-FirstReason $failure
    }finally{
        $completion.pre_cleanup_gate=Completion-Gate;$completion.pre_cleanup_first_reason=$completion.first_failure_reason
        $script:normalStage='cleanup';$script:cleanupReserve=$true
        if($startFile){try{Public-Close $startFile}catch{$control.Unknown=$true}}
        if($child -and $childStarted){
            try {
                if(-not $reaped){if($child.HasExited){Note-ExitIndication 'cleanup_has_exited'}else{$cleanupKillAttempted=$true;$child.Kill()}}
                while((Age) -lt 52 -and -not $reaped){Wait-ForReap 'cleanup' 52;Pump $out;Pump $err}
                if(-not $reaped){$control.Unknown=$true}
            }catch{$control.Unknown=$true;$cleanupFirstCause=Cause $_.Exception}
            while((Age) -lt 54 -and (($out -and -not $out.Eof) -or ($err -and -not $err.Eof))){
                try{Pump $out;Pump $err}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception};break}
                [Threading.Thread]::Sleep(5)
            }
            if(($out -and -not $out.Eof) -or ($err -and -not $err.Eof)){$control.Unknown=$true}
        }
        Public-CloseAll
        try{if($out){$out.Reader.Dispose()};if($err){$err.Reader.Dispose()};$stdioDisposed=$true}catch{$control.Unknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        if($child -and ($reaped -or -not $childStarted)){try{$control.Child=$null;$child.Dispose();$registeredChildReleased=$true}catch{$control.Unknown=$true}}
    }
    try {
        Check
        $good=$normalCandidate -and $completion.normal_gate_passed -and $null -eq $completion.first_failure_reason -and
            $reaped -and $childExit -eq 0 -and $out.Eof -and $err.Eof -and $registeredChildReleased -and $stdioDisposed -and
            -not $managedCloseUnknown -and $publicOwned -eq 0 -and -not $control.Forced -and -not $control.Unknown -and -not $cleanupKillAttempted -and $null -eq $cleanupFirstCause
        if(-not $good -and -not $failure){$failure='cleanup_unknown';Set-FirstReason $failure}
        if($authorized -and $claimed){
            $report=Common-Report 'supervisor';$report.result=if($good){'completed'}else{'failed_or_unknown'};$report.failure=$failure
            $report.cause=[ordered]@{primary=$failureCause;cleanup=$cleanupFirstCause};$report.parent_pid=$childId;$report.parent_started_utc=$childStartedUtc
            $report.retained_process_handle=$retainedProcessHandle;$report.child_exit=$childExit;$report.child_reaped=$reaped
            $report.stdout_chars=if($out){$out.Text.Length}else{0};$report.stderr_chars=if($err){$err.Text.Length}else{0}
            $report.stdout_eof=[bool]($out -and $out.Eof);$report.stderr_eof=[bool]($err -and $err.Eof);$report.registered_child_released=$registeredChildReleased
            $report.stdio_disposed=$stdioDisposed;$report.managed_public_disposed=(-not $managedCloseUnknown -and $publicOwned -eq 0)
            $report.managed_public_close_native_observed=$false;$report.public_peak=$publicPeak;$report.public_remaining=$publicOwned
            $report.forced=$control.Forced;$report.cleanup_kill_attempted=$cleanupKillAttempted;$report.cleanup_unknown=$control.Unknown
            $report.observer=$metrics;$report.completion=$completion;$report.wait_trace=$waitTrace.ToArray();$report.source_after=$sourceAfter
            $report.claim_sha256=if($claimReport){$claimReport.Sha256}else{$null};$report.parent_sha256=if($parentReport){$parentReport.Sha256}else{$null};$report.p2_sha256=if($p2Report){$p2Report.Sha256}else{$null}
            $report.observed_seconds=Age;$null=Write-Report 'supervisor' $report
        }
        Public-CloseAll;Check;if($good -and $publicOwned -eq 0 -and -not $managedCloseUnknown){$result=0}
    }catch{$result=1}
    if($result -eq 0){[Console]::Out.WriteLine('Cold-A raw preservation returned; root actual review required.')}
    else{[Console]::Error.WriteLine('{"classification":"s53-cold-a-supervisor-failed","result":"failed_or_unknown"}')}
    return $result
}
$self=[Diagnostics.Process]::GetCurrentProcess()
$self.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$receivedStart=$StartTimestamp
if($Mode -ceq 'Supervisor' -or $StartTimestamp -le 0){
    $startup=([DateTime]::UtcNow-$self.StartTime.ToUniversalTime()).TotalSeconds
    $StartTimestamp=[Diagnostics.Stopwatch]::GetTimestamp()-[long]($startup*[Diagnostics.Stopwatch]::Frequency)
}
$workDeadline=$StartTimestamp+45L*[Diagnostics.Stopwatch]::Frequency
$done=[Threading.ManualResetEvent]::new($false)
$control=[hashtable]::Synchronized(@{Child=$null;Forced=$false;Unknown=$false})
$guardian=[powershell]::Create()
$null=$guardian.AddScript({
    param($Owner,$Control,$Done,[long]$Start,[bool]$Parent)
    try {
        while(-not $Done.WaitOne(20)){
            $age=([Diagnostics.Stopwatch]::GetTimestamp()-$Start)/[double][Diagnostics.Stopwatch]::Frequency
            if(-not $Parent -and $age -ge 55){
                $Control.Forced=$true;$p=$Control.Child
                if($p){try{if(-not $p.HasExited){$p.Kill()};$null=$p.WaitForExit(20)}catch{$Control.Unknown=$true}}
            }
            if($age -ge $(if($Parent){51}else{59})){$Owner.Kill();return}
        }
    }catch{try{$Owner.Kill()}catch{}}
}).AddArgument($self).AddArgument($control).AddArgument($done).AddArgument($StartTimestamp).AddArgument(($Mode -ceq 'Parent'))
$ticket=$guardian.BeginInvoke() # before public parsing, bootstrap, helper compilation or any private operation
$exit=1
try {
    Need (($Mode -ceq 'Supervisor' -and $receivedStart -eq 0) -or ($Mode -ceq 'Parent' -and $receivedStart -gt 0)) 'internal_parent_only'
    $exit=if($Mode -ceq 'Supervisor'){Invoke-Supervisor}else{Invoke-Parent}
}catch{[Console]::Error.WriteLine('{"classification":"s53-cold-a-launch-failed","result":"failed_or_unknown"}');$exit=1}
finally {
    Public-CloseAll
    if($control.Child){
        # A failed cleanup must leave the independent guardian armed until its fixed self cutoff.
        while((Age) -lt 59){[Threading.Thread]::Sleep(20)}
        $self.Kill()
    }
    $null=$done.Set()
    $left=[Math]::Floor(($cleanupLimit-(Age))*1000)
    if($left -lt 1 -or -not $ticket.AsyncWaitHandle.WaitOne([int][Math]::Min(200,$left))){$exit=1}
    if($ticket.IsCompleted){
        try{$null=$guardian.EndInvoke($ticket)}catch{$exit=1}
        $guardian.Dispose();$done.Dispose()
    }
    if($managedCloseUnknown -or $publicOwned -ne 0 -or $control.Forced -or $control.Unknown){$exit=1}
    $self.Dispose()
}
exit $exit
