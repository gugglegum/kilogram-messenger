param([ValidateSet('normal','grace','forced')][string]$Kind)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$taskRoot="$w\ai-docs\tasks";$e="$taskRoot\long-lived-dialog-a3-migration-s5-handoff-evidence"
$fixedShell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$entry="$w\scripts\invoke-migration-s53-cold-preservation.ps1";$corePath="$w\scripts\migration-s53-cold-preservation.ps1";$outputPath="$w\scripts\migration-s53-cold-output.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1";$deadlinePath="$w\scripts\migration-s5-l1-deadline.ps1"
$attempt="C-s53-cold-timeout-controls-01-attempt-01.$Kind"
$raw=[IO.File]::ReadAllBytes("$e\$attempt.context.json");if($raw.Length -gt 16384){throw 'context cap'}
$cfg=[Text.UTF8Encoding]::new($false,$true).GetString($raw) | ConvertFrom-Json -Depth 12 -DateKind String
$StartTimestamp=[long]$cfg.start;$DecisionSha256=$cfg.decision;$sourcePins=[ordered]@{}
foreach($property in $cfg.pins.PSObject.Properties){$sourcePins[$property.Name]=$property.Value}
$cleanupReserve=$false;$cleanupLimit=4;$guard=$null;$managedCloseUnknown=$false;$publicOwned=0;$publicPeak=0
$publicRecords=[Collections.Generic.List[object]]::new()
$self=[Diagnostics.Process]::GetCurrentProcess();$outputOwner=$null;$parentGuards=$null
$preparedHandles=0;$ownerDisposed=$false;$guardsDisposed=$false;$coreFinal=$null
function Stop-Cold([string]$Code) {
    $e=[InvalidOperationException]::new('Cold launcher bounded refusal');$e.Data['cold_launch_code']=$Code;throw $e
}
function Need([bool]$Value,[string]$Code='assertion'){if(-not $Value){Stop-Cold $Code}}
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Check {
    $limit=if($cleanupReserve){$cleanupLimit}else{2};$age=Age
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
        launcher=@($entry,0,'');core=@($corePath,82999,'eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f');output=@($outputPath,21721,'ced08d88656e7bad142d1e23a826c42e7be4f75da8380c838167610504db7ae0')
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
            'source_after','claim_sha256','parent_sha256','p2_sha256','diagnostics','observed_seconds')}
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
        foreach($k in @('ancestry','prepared','remaining','receipt_mask','receipt_transfers','native_error')){$v=Value $Object.output $k;Need (($v -is [int] -or $v -is [long]) -and $v -ge [int]::MinValue -and $v -le [int]::MaxValue) 'public_output'}
        foreach($k in @('remaining','peak','open_attempts','native_close_attempts','native_closed','native_error')){$v=Value $Object.parent_guards $k;Need (($v -is [int] -or $v -is [long]) -and $v -ge [int]::MinValue -and $v -le [int]::MaxValue) 'public_guard'}
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
        Need (($Object.stdout_chars -is [int] -or $Object.stdout_chars -is [long]) -and ($Object.stderr_chars -is [int] -or $Object.stderr_chars -is [long]) -and $Object.stdout_chars -ge 0 -and $Object.stdout_chars -le 4096 -and $Object.stderr_chars -ge 0 -and $Object.stderr_chars -le 4096) 'public_stdio'
        Validate-SourceMap $Object.source_after
        Fields $Object.diagnostics @('state','cause')
        Need ($Object.diagnostics.state -is [string] -and $Object.diagnostics.state -cin @('not_collected','normal_validated','bound','unknown')) 'public_diagnostics'
        Validate-Cause $Object.diagnostics.cause
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
$start=Await-Start
$claim=Common-Report 'claim';$claim.supervisor_pid=$cfg.supervisor_pid;$claim.parent_pid=$self.Id
$claim.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');$claim.started_sha256=$start.Sha256
$null=Write-Report 'claim' $claim;Public-CloseAll
if($Kind -ceq 'forced'){[Threading.Thread]::Sleep([Threading.Timeout]::Infinite)}
$target=if($Kind -ceq 'normal'){1}else{3}
$left=[Math]::Floor(($target-(Age))*1000);if($left -gt 0){[Threading.Thread]::Sleep([int]$left)}
if($Kind -ceq 'normal'){$self.Dispose();exit 0}
$cleanupReserve=$true
try{Stop-Cold 'work_expired'}catch{$cause=Cause $_.Exception}
$p=Common-Report 'p2';$p.result='failed_or_unknown';$p.failure='work_expired';$p.cause=[ordered]@{primary=$cause;cleanup=$null}
$p.source_before=$sourcePins;$p.source_after=[ordered]@{};$p.core=$null;$p.output=Output-Facts;$p.parent_guards=Parent-Facts
$p.compiler=[ordered]@{failed=$false;diagnostics=@();fresh_types=[ordered]@{};initializers=[ordered]@{}}
$p.process_checks=0;$p.managed_public_disposed=$true;$p.managed_public_close_native_observed=$false
$p.public_peak=$publicPeak;$p.public_remaining=$publicOwned;$p.core_dispose_attempted=$false;$p.wrapper_cleanup=$null
$p.external_bound=59;$p.interval_ended_before_receipts=$false;$p.p2_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
$h=Write-Report 'p2' $p
$r=Common-Report 'parent';$r.result='failed_or_unknown';$r.failure='work_expired';$r.cause=$p.cause
$r.parent_pid=$self.Id;$r.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');$r.p2_sha256=$h
$r.cleanup_known=$false;$r.core_remaining=$null;$r.output_remaining=0;$r.guard_remaining=0
$r.public_remaining=$publicOwned;$r.public_peak=$publicPeak;$r.managed_public_close_native_observed=$false
$r.job_finished=$false;$r.children_after=$null;$r.processes_disposed=$true;$r.observed_seconds=Age
$null=Write-Report 'parent' $r;Public-CloseAll
if($managedCloseUnknown -or $publicOwned -ne 0){exit 2}
$self.Dispose();exit 0 # Intentionally late0; host MUST retain failed timeout.
