[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,
    [Parameter(Mandatory)][long]$StartTimestamp
)
# Dormant source only. Requires separately pinned root activation; work20/whole30, lane1/retry0.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-output-wrapper-01-attempt-01'
$e="$w\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence"
$allocation="$e\$attempt.root-allocation.json"
$preflight="$e\$attempt.preflight.ps1";$watchdog="$e\$attempt.watchdog.ps1"
$core="$w\scripts\migration-s53-cold-preservation.ps1";$output="$w\scripts\migration-s53-cold-output.ps1"
$deadline="$w\scripts\migration-s5-l1-deadline.ps1"
$fixtureSupport="$e\$attempt.fixture-support.ps1"
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$held=[Collections.Generic.List[IO.FileStream]]::new();$guard=$null;$cleanupReserve=$false
$cleanupLimit=24
function Age {([Diagnostics.Stopwatch]::GetTimestamp()-$StartTimestamp)/[double][Diagnostics.Stopwatch]::Frequency}
function Stop-C2S([string]$Code) {
    $err=[InvalidOperationException]::new('C2S bounded refusal');$err.Data['c2s_code']=$Code;throw $err
}
function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2S $Code}}
function Check {
    $age=Age;$limit=if($cleanupReserve){$cleanupLimit}else{20}
    if($age -lt 0 -or $age -ge $limit){Stop-C2S $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
    if($guard -and -not $cleanupReserve){$guard.CheckWork()}
    if((Age) -ge $limit){Stop-C2S $(if($cleanupReserve){'cleanup_expired'}else{'work_expired'})}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        if($at.Data.Contains('c2s_code')){return [string]$at.Data['c2s_code']}
        if($at.Data.Contains('c2s_support_code')){return 'fixture_refusal'}
        if($at.Data.Contains('colda_code')){return 'core_refusal'}
        if($at -is [TimeoutException]){return 'work_expired'}
    }
    return 'failed_or_unknown'
}
function Open-Public([string]$Path,[long]$Maximum) {
    Check
    if(-not [IO.Path]::IsPathFullyQualified($Path) -or
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2S 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2S 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2S 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2S 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2S 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2S 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2S 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2S 'source_pin'}
    return $f
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2S 'output_cap'}
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
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2s_code','c2s_support_code','cold_output_code')){
            $value=[string]$at.Data[$key]
            if($value -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$value}
        }
    }
    $type=$last.GetType().FullName
    if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks}
}
function New-PublicOutput([string]$Suffix) {
    Check;Need ($Suffix -cin @('started.json','cases.json','watchdog.json')) 'output_cap'
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
            'preflight_sha256','watchdog_sha256','core_sha256','output_sha256','deadline_sha256','fixture_support_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        Need ($doc.RootElement.ValueKind -eq [Text.Json.JsonValueKind]::Object) 'allocation_schema'
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($p in $doc.RootElement.EnumerateObject()){Need ($p.Name -cin $fields -and $map.TryAdd($p.Name,$p.Value)) 'allocation_schema'}
        Need ($map.Count -eq 23) 'allocation_schema'
        foreach($key in @('schema_version','work_seconds','whole_seconds','retry_count','lane_count','public_report_bytes','public_total_bytes')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::Number) 'allocation_schema'
        }
        foreach($key in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','output_sha256','deadline_sha256','fixture_support_sha256','shell_sha256')){
            Need ($map[$key].ValueKind -eq [Text.Json.JsonValueKind]::String) 'allocation_schema'
        }
        Need ($map['schema_version'].GetInt32() -eq 1 -and
            $map['classification'].GetString() -ceq 'root-approved-cold-output-wrapper-c2-s01' -and
            $map['root_thread_id'].GetString() -ceq '01a164ec-589c-7b41-bc34-3ba158d1f22b' -and
            $map['activated'].ValueKind -eq [Text.Json.JsonValueKind]::True -and
            $map['attempt'].GetString() -ceq $attempt -and $map['operation_set'].GetString() -ceq 'cold-output-wrapper-two-bind-only-r01' -and
            $map['workspace'].GetString() -ceq $w -and $map['fixture_parent'].GetString() -ceq ($w+'\.tmp\'+$attempt) -and
            $map['work_seconds'].GetInt32() -eq 20 -and $map['whole_seconds'].GetInt32() -eq 30 -and
            $map['retry_count'].GetInt32() -eq 0 -and $map['lane_count'].GetInt32() -eq 1 -and
            $map['public_report_bytes'].GetInt32() -eq 65536 -and $map['public_total_bytes'].GetInt32() -eq 196608) 'allocation_scope'
        $cases=$map['fixture_cases'];Need ($cases.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $cases.GetArrayLength() -eq 2 -and
            $cases[0].ValueKind -eq [Text.Json.JsonValueKind]::String -and $cases[0].GetString() -ceq 'wrapper-healthy-r01' -and
            $cases[1].ValueKind -eq [Text.Json.JsonValueKind]::String -and $cases[1].GetString() -ceq 'wrapper-output-acl-r01') 'allocation_scope'
        $outputs=$map['outputs'];Need ($outputs.ValueKind -eq [Text.Json.JsonValueKind]::Array -and $outputs.GetArrayLength() -eq 3) 'allocation_scope'
        $names=@('started.json','cases.json','watchdog.json')
        for($i=0;$i -lt 3;$i++){Need ($outputs[$i].ValueKind -eq [Text.Json.JsonValueKind]::String -and $outputs[$i].GetString() -ceq $names[$i]) 'allocation_scope'}
        $pins=[ordered]@{}
        foreach($key in @('preflight','watchdog','core','output','deadline','fixture_support','shell')){
            $hash=$map[$key+'_sha256'].GetString();Need ($hash -cmatch '\A[0-9a-f]{64}\z') 'allocation_schema';$pins[$key]=$hash
        }
        $fixedPins=[ordered]@{core='4288f66bbc634f336fb962624fe15aaf6561f760f7d2c096579bb94e31711612';output='9d4b2a4fed3ffd76a1393514dff69f77baa5e20567e25c171be1c4203ec3a4b9';deadline='0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45';fixture_support='4838fa35fe41551191e1bccf33b9c54115e53587a46f1634b7804e807698b7c8';shell='362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'}
        $fixedLengths=[ordered]@{core=82836;output=21296;deadline=6038;fixture_support=21111;shell=301368}
        foreach($key in $fixedPins.Keys){Need ($pins[$key] -ceq $fixedPins[$key]) 'source_pin'}
        $paths=[ordered]@{preflight=$preflight;watchdog=$watchdog;core=$core;output=$output;deadline=$deadline;fixture_support=$fixtureSupport;shell=$shell}
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
function Assert-BindFacts($f,[bool]$Failure,[bool]$Disposed) {
    Assert-Fields $f ([ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$false;expected_set_captured_before_hash=$false;observed_entry_sets_equal=$false;input_handles_released=$Disposed;receipts_completed=$false
        cleanup_unknown=$false;receipt_write_unknown=$false;cleanup_native_error=0;expected_files=2;expected_bytes=0;locks=0;hash_passes=0;hash_bytes=0
        capture_complete_timestamp=0;remaining_owned_handles=$(if($Disposed -or $Failure){0}else{1});peak_owned_handles=1;peak_inventory_bytes=0;peak_path_bytes=0;peak_record_bytes=12073840
        receipt_bytes_charged=0;receipt_bytes_known_written=0;scratch_reserved_bytes=524288
        failure_code=$(if($Failure){'output_acl'}else{$null});failure_phase=$(if($Failure){'output_binding'}else{$null})
        failure_role=$(if($Failure){'output'}else{$null});cause_kind=$(if($Failure){'unexpected'}else{$null});native_error=0;error_hresult=$(if($Failure){-2146233079}else{0})})
    Need (@(Object-Keys $f.private_receipt_digests).Count -eq 0) 'unexpected_receipts'
}
function Assert-WrapperDiagnostics($d) {
    Assert-Fields $d ([ordered]@{diagnostics_complete=$true;pending_observed=$true;pending_closed=$true
        before_observed=$true;before_owned=0;before_unknown=$false;before_native=0;before_failure='output_acl';before_phase='output_binding';before_role='output'
        core_disposed=$true;after_observed=$true;after_owned=0;after_unknown=$false;after_native=0
        fallback_needed=$false;fallback_used=$false;fallback_skipped_unknown=$false;pending_closed_after=$true
        cleanup_unknown=$false;native_close_unknown=$false;secondary_type=$null;secondary_hresult=0})
    Need ($d.pending_invalid -is [bool]) 'wrapper_pending_state'
    Need ($d.catch_qpc -gt $StartTimestamp -and $d.core_dispose_qpc -ge $d.catch_qpc -and
        $d.core_dispose_qpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency)) 'wrapper_cleanup_order'
}
function Assert-Case($c,[bool]$Failure) {
    Assert-Fields $c ([ordered]@{name=$(if($Failure){'output-acl'}else{'healthy'});selector=$(if($Failure){'wrapper-output-acl-r01'}else{'wrapper-healthy-r01'})
        observed=$true;binding_succeeded=(-not $Failure);expected_refusal=$Failure
        owner_prepared_handles=7;owner_after_binding_handles=6;owner_ancestry=6;owner_final_handles=0;owner_cleanup_unknown=$false;owner_cleanup_native_error=0
        directory_created=$true;directory_transferred=$true;receipt_mask=0;receipts_transferred=0;core_disposed=$true;owner_disposed=$true
        exact_delegate=$true;helper_before_bind_handles=$(if($Failure){8}else{7});helper_after_case_handles=7
        inventory_metadata_bytes=$(if($Failure){2025}else{2013});inventory_metadata_sha256=$(if($Failure){'ea7a2bdab58553178221136f7fd467e24f29199f506529bb0aaf05a8a1ccd4e1'}else{'3de9737b02c04660f89b36eda638267f846a2c66e8ba3286022274791620a136'})
        input_inventory_created=$false;input_inventory_read=$false;capture_invoked=$false;positive_preservation_proof=$false})
    Assert-BindFacts $c.after_binding $Failure $Failure;Assert-BindFacts $c.final_core $Failure $true
    Need ($c.before_bind_qpc -gt $StartTimestamp -and $c.after_bind_qpc -ge $c.before_bind_qpc -and
        $c.cleanup_done_qpc -ge $c.after_bind_qpc -and $c.cleanup_done_qpc -lt ($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency))
    if($Failure){
        Assert-WrapperDiagnostics $c.wrapper
        Assert-Fields $c.cause ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0})
        Assert-Fields $c.cause.marks ([ordered]@{colda_code='output_acl';colda_phase='output_binding';colda_role='output';cold_output_code='private_acl';rootcopies_code='private_acl'})
        Need (@($c.chain).Count -gt 0 -and @($c.chain).Count -le 16)
        Need ($c.wrapper.catch_qpc -ge $c.before_bind_qpc -and $c.wrapper.core_dispose_qpc -le $c.after_bind_qpc)
    }else{Need ($null -eq $c.wrapper -and $null -eq $c.cause -and @($c.chain).Count -eq 0)}
    $h=$c.helper
    Assert-Fields $h ([ordered]@{created_directories=3;root_guards=7;remaining_owned_handles=7;editor_owned=$false;search_owned=$false;cleanup_unknown=$false;cleanup_native_error=0
        completed_cases=$(if($Failure){2}else{1});empty_checks=$(if($Failure){4}else{2});editor_opens=$(if($Failure){3}else{2});post_verified=$true
        set_calls=$(if($Failure){1}else{0});set_result=$(if($Failure){0}else{-1});pin_calls=$(if($Failure){1}else{0});pin_frees=$(if($Failure){1}else{0});pin_owned=$false})
    Need ($h.find_opened -eq $h.find_closed -and $h.find_opened -le $h.empty_checks -and $h.peak_owned_handles -le 9)
    Need ($h.before_acl_sha256 -cmatch '\A[0-9a-f]{64}\z' -and $h.after_acl_sha256 -cmatch '\A[0-9a-f]{64}\z')
    Need ($h.healthy_identity -cmatch '\A[0-9a-f]{8}:[0-9a-f]{16}\z' -and $h.post_qpc -ge $c.after_bind_qpc -and $h.post_qpc -le $c.cleanup_done_qpc) 'helper_identity_or_order'
    if($Failure){
        Need ($h.acl_identity -cmatch '\A[0-9a-f]{8}:[0-9a-f]{16}\z' -and $h.acl_identity -cne $h.healthy_identity) 'helper_identity'
        foreach($key in @('user_ace_hex','system_ace_hex')){Need ((Get-Value $h $key) -cmatch '\A[0-9A-F]{2,512}\z') 'helper_ace_bytes'}
        Assert-Fields $h ([ordered]@{narrowed=$true;dacl_queries=1;valid_acl_calls=1;dacl_present=$true;dacl_nonnull=$true;dacl_valid=$true
            ace_bytes_preserved=$true;owner_group_preserved=$true})
        Need ($h.before_acl_sha256 -cne $h.after_acl_sha256 -and $h.narrow_before_qpc -le $h.narrow_after_qpc -and $h.narrow_after_qpc -lt $c.before_bind_qpc)
    }else{Need ($null -eq $h.acl_identity -and $h.before_acl_sha256 -ceq $h.after_acl_sha256)}
}
function Assert-WorkerReport($v,$Pins,[int]$WorkerPid,[string]$WorkerStarted,[int]$SupervisorPid) {
    Assert-Fields $v ([ordered]@{classification='cold-output-wrapper-c2-s01';attempt=$attempt;allocation_sha256=$AllocationSha256
        result='observed';failure=$null;failure_cause=$null;cleanup_first_cause=$null;controller_cleanup_unknown=$false
        whole_attempt_verdict='not_established';child_pid=$WorkerPid;worker_started_utc=$WorkerStarted;started_supervisor_pid=$SupervisorPid
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;work_deadline_timestamp=($StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency)
        compiler_error=$false;diagnostic_cap=$false;managed_inputs_disposed=$true;public_inputs_before_close=9
        helper_disposed=$true;owned_children_after=0;job_finished=$true;worker_self_disposed=$true
        synthetic_fixture_access=$true;private_dataset_access=$false;fixture_retained_if_created=$true;work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1})
    foreach($key in $Pins.Keys){
        Assert-Fields $v.source_pins_before ([ordered]@{$key=$Pins[$key]});Assert-Fields $v.source_pins_after ([ordered]@{$key=$Pins[$key]})
    }
    foreach($key in @('deadline','core','output','fixture_support')){Assert-Fields $v.fresh_types_absent_before ([ordered]@{$key=$true})}
    foreach($key in @('deadline','Initialize-S53ColdPreservationCore','Initialize-S53ColdOutputAdapter','Initialize-S53WrapperFixtureSupportR01')){
        Assert-Fields $v.initialization_calls ([ordered]@{$key=1})
    }
    Need (@($v.cases).Count -eq 2 -and @($v.diagnostics).Count -eq 0)
    Assert-Case $v.cases[0] $false;Assert-Case $v.cases[1] $true
    Assert-Fields $v.helper_summary ([ordered]@{created_directories=3;remaining_owned_handles=0;root_guards=7;disposed=$true;completed_cases=2;cleanup_unknown=$false;cleanup_native_error=0
        editor_owned=$false;search_owned=$false;pin_owned=$false;empty_checks=4;editor_opens=3;close_calls=10;closed_calls=10;set_calls=1;set_result=0})
}
function Diagnostic($Item,[string]$Kind) {
    $message=[string]$(if($Item -is [Management.Automation.ErrorRecord]){$Item.Exception.Message}else{[string]$Item})
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2S 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2S 'diagnostic_cap'}
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
    }catch{$script:compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2S 'compiler_error'}
    Need (-not $compilerError) 'compiler_error'
}
function Assert-Definitions([string]$Path,[int]$Functions) {
    $tokens=$null;$errors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($Path,[ref]$tokens,[ref]$errors)
    Need ($errors.Count -eq 0 -and $ast.EndBlock.Statements.Count -eq $Functions -and
        @($ast.EndBlock.Statements|Where-Object {$_ -isnot [Management.Automation.Language.FunctionDefinitionAst]}).Count -eq 0) 'definitions_shape'
}
function Inventory-Literal([string]$Name) {
    if($Name -ceq 'healthy') { return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\healthy\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\healthy\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\healthy\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\healthy\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@+"`n" }
    if($Name -ceq 'output-acl') { return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\output-acl\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\output-acl\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\output-acl\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-output-wrapper-01-attempt-01\\output-acl\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@+"`n" }
    Stop-C2S 'metadata_scope'
}
function Core-Facts($Core) { return Decode-Public ($Core.PublicObservationUtf8()) }
function Wrapper-Diagnostics($Exception) {
    $keys=@('catch_qpc','pending_observed','pending_closed','pending_invalid','before_observed','before_owned','before_unknown','before_native','before_failure','before_phase','before_role'
        'core_dispose_qpc','core_disposed','after_observed','after_owned','after_unknown','after_native','fallback_needed','fallback_used','fallback_skipped_unknown','pending_closed_after'
        'cleanup_unknown','native_close_unknown','secondary_type','secondary_hresult','diagnostics_complete')
    $found=$null;$depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++;if(-not $at.Data.Contains('s53_wrapper_diagnostics_complete')){continue}
        Need ($null -eq $found) 'wrapper_diagnostics'
        $found=[ordered]@{}
        foreach($key in $keys){
            Need ($at.Data.Contains('s53_wrapper_'+$key)) 'wrapper_diagnostics'
            $v=$at.Data['s53_wrapper_'+$key]
            Need ($null -eq $v -or $v -is [bool] -or $v -is [long] -or $v -is [int] -or ($v -is [string] -and $v.Length -le 128)) 'wrapper_diagnostics'
            $found[$key]=$v
        }
    }
    Need ($null -ne $found) 'wrapper_diagnostics'
    return $found
}
function Exception-Chain($Exception) {
    $v=[Collections.Generic.List[object]]::new();$at=$Exception
    while($at){
        Need ($v.Count -lt 16) 'exception_chain_cap'
        $type=$at.GetType().FullName;Need ($type -cmatch '\A[A-Za-z0-9_.+]{1,128}\z') 'exception_chain_cap'
        $v.Add([ordered]@{type=$type;hresult=$at.HResult});$at=$at.InnerException
    }
    return ,$v.ToArray()
}
function Invoke-BindCase([string]$Name) {
    Check;Need ($Name -cin @('healthy','output-acl'))
    $fail=$Name -ceq 'output-acl';$selector=if($fail){'wrapper-output-acl-r01'}else{'wrapper-healthy-r01'}
    $literal=Inventory-Literal $Name;$raw=[Text.UTF8Encoding]::new($false).GetBytes($literal)
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant()
    Need ($raw.Length -eq $(if($fail){2025}else{2013}) -and $hash -ceq $(if($fail){'ea7a2bdab58553178221136f7fd467e24f29199f506529bb0aaf05a8a1ccd4e1'}else{'3de9737b02c04660f89b36eda638267f846a2c66e8ba3286022274791620a136'})) 'metadata_pin'
    $obj=$null;$owner=$null;$coreDisposed=$false;$ownerDisposed=$false;$bindingCallFailed=$false;$caseResult=$null
    try {
        $obj=New-S53ColdAFixtureCore -Case $selector -InventoryBytes $raw.Length -InventorySha256 $hash -ExpectedFiles 2 -ExpectedPayloadBytes 0 -WorkDeadlineTimestamp $absoluteWorkDeadline
        $owner=if($fail){[Kilogram.S53.ColdOutput.BaselineSmallOwner]::CreateWrapperAclR01($absoluteWorkDeadline)}else{[Kilogram.S53.ColdOutput.BaselineSmallOwner]::CreateWrapperHealthyR01($absoluteWorkDeadline)}
        $owner.PrepareDirectory();Need ($owner.AncestryCount -eq 6 -and $owner.RemainingOwnedHandles -eq 7)
        $fixtureObject.BeginCase($Name)
        $creator=$owner.Creator;$verifier=$owner.AclVerifier
        $delegateOk=$creator.Target -eq $owner -and $verifier.Target -eq $owner -and
            $creator.Method.Name -ceq 'CreateReceipt' -and $verifier.Method.Name -ceq 'VerifyExactAcl' -and
            $verifier.Method.DeclaringType.FullName -ceq 'Kilogram.S53.ColdOutput.BaselineSmallOwner'
        Need $delegateOk 'real_delegate'
        $helperBefore=$fixtureObject.RemainingOwnedHandles;$preparedHandles=$owner.RemainingOwnedHandles;$ancestryCount=$owner.AncestryCount
        $before=[Diagnostics.Stopwatch]::GetTimestamp();$caught=$null;$wrap=$null;$cause=$null;$chain=@();$succeeded=$false
        try {
            Bind-S53ColdBaselineSmallOutput -Core $obj -Owner $owner -DecisionSha256 $AllocationSha256 -CoreSha256 $binding.Pins.core
            $succeeded=$true
        }catch{
            $bindingCallFailed=$true;$caught=$_;$cause=Cause $caught.Exception;$chain=Exception-Chain $caught.Exception
            $script:observedBindFailure=[ordered]@{name=$Name;cause=$cause;chain=$chain}
            if(-not $fail){throw}
            $wrap=Wrapper-Diagnostics $caught.Exception
        }
        $after=[Diagnostics.Stopwatch]::GetTimestamp();Check
        Need ($succeeded -eq (-not $fail) -and ($null -ne $caught) -eq $fail) 'case_outcome'
        $afterBinding=Core-Facts $obj;Assert-BindFacts $afterBinding $fail $fail
        $afterOwner=$owner.RemainingOwnedHandles
        if($fail){Assert-WrapperDiagnostics $wrap;$coreDisposed=$true}else{$obj.Dispose();$coreDisposed=$true}
        $final=Core-Facts $obj
        $fixtureObject.EndCase();$hs=$fixtureObject.Snapshot()
        $owner.RecheckAncestry();$owner.Dispose();$ownerDisposed=$true
        $caseResult=[ordered]@{name=$Name;selector=$selector;observed=$true;binding_succeeded=$succeeded;expected_refusal=$fail
            before_bind_qpc=$before;after_bind_qpc=$after;cleanup_done_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
            owner_prepared_handles=$preparedHandles;owner_after_binding_handles=$afterOwner;owner_ancestry=$ancestryCount
            owner_final_handles=$owner.RemainingOwnedHandles;owner_cleanup_unknown=$owner.CleanupUnknown;owner_cleanup_native_error=$owner.CleanupNativeError
            directory_created=$owner.DirectoryCreated;directory_transferred=$owner.DirectoryTransferred;receipt_mask=$owner.ReceiptCreatedMask;receipts_transferred=$owner.ReceiptsTransferred
            core_disposed=$coreDisposed;owner_disposed=$ownerDisposed;exact_delegate=$delegateOk;helper_before_bind_handles=$helperBefore;helper_after_case_handles=$fixtureObject.RemainingOwnedHandles
            inventory_metadata_bytes=$raw.Length;inventory_metadata_sha256=$hash;input_inventory_created=$false;input_inventory_read=$false;capture_invoked=$false;positive_preservation_proof=$false
            after_binding=$afterBinding;final_core=$final;wrapper=$wrap;cause=$cause;chain=$chain;helper=$hs}
        Assert-Case $caseResult $fail
    } finally {
        # A post-Take failing wrapper owns Core cleanup. Do not retry its uncertain close in the caller.
        try{if($obj -and -not $coreDisposed -and -not ($bindingCallFailed -and $owner -and $owner.DirectoryTransferred)){$obj.Dispose()}}catch{$script:controllerCleanupUnknown=$true;if(-not $script:cleanupFirstCause){$script:cleanupFirstCause=Cause $_.Exception}}
        try{if($obj){$f=$obj.GetObservation();if($f.CleanupUnknown -or $f.RemainingOwnedHandles -ne 0){$script:controllerCleanupUnknown=$true}}}catch{$script:controllerCleanupUnknown=$true;if(-not $script:cleanupFirstCause){$script:cleanupFirstCause=Cause $_.Exception}}
        try{if($owner -and -not $ownerDisposed){$owner.Dispose()};if($owner -and ($owner.CleanupUnknown -or $owner.RemainingOwnedHandles -ne 0)){$script:controllerCleanupUnknown=$true}}catch{$script:controllerCleanupUnknown=$true;if(-not $script:cleanupFirstCause){$script:cleanupFirstCause=Cause $_.Exception}}
    }
    return $caseResult
}
$binding=$null;$claim=$null;$authorized=$false;$workerSelf=$null;$workerPid=0;$workerStartedUtc=$null;$workerSelfDisposed=$false
$sourceBefore=[ordered]@{};$sourceAfter=[ordered]@{};$typeAbsent=[ordered]@{};$initializeCalls=[ordered]@{}
$diagnostics=[Collections.Generic.List[object]]::new();$diagnosticChars=0;$compilerError=$false;$diagnosticCap=$false
$fixtureObject=$null;$helperSummary=$null;$helperDisposed=$false;$fixtureAccess=$false
$cases=[Collections.Generic.List[object]]::new();$controllerCleanupUnknown=$false;$cleanupFirstCause=$null
$managedDisposed=$false;$publicInputsBeforeClose=0;$childrenAfter=$null;$jobFinished=$false;$reportClosed=$false
$failure='not_started';$failureCause=$null;$observedBindFailure=$null;$candidate=$false;$result=1
$absoluteWorkDeadline=$StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency
try {
    Need ($StartTimestamp -gt 0 -and $PSCommandPath -ceq $preflight) 'allocation_scope'
    $workerSelf=[Diagnostics.Process]::GetCurrentProcess();$workerSelf.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $workerPid=$workerSelf.Id;$workerStartedUtc=$workerSelf.StartTime.ToUniversalTime().ToString('o')
    $binding=Validate-Allocation;$authorized=$true;$sourceBefore=Source-Pins $binding
    $started=Open-Public "$e\$attempt.started.json" 65536;$claim=Decode-Public (Bytes $started 65536)
    Assert-Fields $claim ([ordered]@{classification='cold-output-wrapper-c2-start-s01';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;permission_consumed=$true;work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1})
    Need ($claim.supervisor_pid -gt 0 -and (Age) -lt 15) 'bootstrap_reserve'
    $typeAbsent.deadline=$null -eq ('KilogramS5Deadline' -as [type]);Need $typeAbsent.deadline 'core_type_cached'
    Initialize-Reviewed 'deadline' {. $deadline}
    $guard=[KilogramS5Deadline]::new((20-(Age)),(24-(Age)));Check
    Assert-Definitions $core 3;Assert-Definitions $output 3;Assert-Definitions $fixtureSupport 1
    $types=[ordered]@{core='Kilogram.S53.ColdA.Core';output='Kilogram.S53.ColdOutput.BaselineSmallOwner';fixture_support='Kilogram.S53.WrapperR01.FixtureSupport'}
    foreach($key in $types.Keys){$typeAbsent[$key]=$null -eq ($types[$key] -as [type]);Need $typeAbsent[$key] 'core_type_cached'}
    . $core
    . $output
    . $fixtureSupport
    foreach($name in @('Initialize-S53ColdPreservationCore','Initialize-S53ColdOutputAdapter','Initialize-S53WrapperFixtureSupportR01')){
        Initialize-Reviewed $name ([scriptblock]::Create($name))
    }
    foreach($key in $types.Keys){Need ($null -ne ($types[$key] -as [type])) 'signature_mismatch'}
    Need ($diagnostics.Count -eq 0 -and [Threading.Thread]::CurrentThread.GetApartmentState().ToString() -ceq 'MTA') 'compiler_or_apartment'
    $fixtureObject=[Kilogram.S53.WrapperR01.FixtureSupport]::new($absoluteWorkDeadline);$fixtureAccess=$true
    $fixtureObject.PrepareRoots();Need ($fixtureObject.RemainingOwnedHandles -eq 7)
    $cases.Add((Invoke-BindCase 'healthy'));Check
    $cases.Add((Invoke-BindCase 'output-acl'));Check
    $sourceAfter=Source-Pins $binding;Check;$candidate=$true;$failure=$null
}catch{$failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception}
finally {
    $cleanupReserve=$true
    try{if($fixtureObject){$fixtureObject.Dispose();$helperDisposed=$true;$helperSummary=$fixtureObject.Snapshot()
        if($fixtureObject.CleanupUnknown -or $fixtureObject.RemainingOwnedHandles -ne 0){$controllerCleanupUnknown=$true}}}
    catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    $publicInputsBeforeClose=$held.Count;$managedDisposed=Close-PublicInputs;if(-not $managedDisposed){$controllerCleanupUnknown=$true}
    if($guard){
        try{$guard.CleanupChildren([int][Math]::Max(0,[Math]::Min(500,(24-(Age))*1000)));$childrenAfter=@($guard.Children()).Count
            Need ($childrenAfter -eq 0) 'cleanup_unknown'}
        catch{$controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
    }
    if($workerSelf){try{$workerSelf.Dispose();$workerSelfDisposed=$true}catch{$controllerCleanupUnknown=$true}}
}
try {
    Check
    if($controllerCleanupUnknown -or -not $managedDisposed -or -not $helperDisposed -or $childrenAfter -ne 0){$candidate=$false;if(-not $failure){$failure='cleanup_unknown'}}
    # Parent's independent guardian remains armed through report-close and actual worker exit.
    $report=[ordered]@{classification='cold-output-wrapper-c2-s01';attempt=$attempt;allocation_sha256=$AllocationSha256
        result=$(if($candidate){'observed'}else{'failed_or_unknown'});failure=$failure;failure_cause=$failureCause;observed_bind_failure=$observedBindFailure;cleanup_first_cause=$cleanupFirstCause
        whole_attempt_verdict='not_established';child_pid=$workerPid;worker_started_utc=$workerStartedUtc;started_supervisor_pid=$(if($claim){$claim.supervisor_pid}else{0})
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;work_deadline_timestamp=$absoluteWorkDeadline
        source_pins_before=$sourceBefore;source_pins_after=$sourceAfter;fresh_types_absent_before=$typeAbsent;initialization_calls=$initializeCalls
        compiler_error=$compilerError;diagnostic_cap=$diagnosticCap;diagnostics=$diagnostics.ToArray();cases=$cases.ToArray();helper_summary=$helperSummary;helper_disposed=$helperDisposed
        managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose;owned_children_after=$childrenAfter
        job_finished=$false;worker_self_disposed=$workerSelfDisposed;controller_cleanup_unknown=$controllerCleanupUnknown
        synthetic_fixture_access=$fixtureAccess;private_dataset_access=$false;fixture_retained_if_created=$true
        work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1;observed_seconds=(Age)}
    if($candidate){
        # Require actual Finish success, then report under the parent's still-armed guardian.
        $guard.Finish();$jobFinished=$true;$report.job_finished=$true
        Assert-WorkerReport $report $binding.Pins $workerPid $workerStartedUtc $claim.supervisor_pid
    }
    if($authorized){
        $file=New-PublicOutput 'cases.json';$safe=$null
        try{$safe=$file.SafeFileHandle;Write-Public $file $report}finally{$file.Dispose();$reportClosed=$null -ne $safe -and $safe.IsClosed}
        Need $reportClosed 'cleanup_unknown'
    }else{$candidate=$false}
    Check;if($candidate -and $reportClosed -and $jobFinished){$result=0}
}catch{
    $result=1;if($null -eq $failureCause){$failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception}
}
if($result -ne 0){
    $diagnostic=ConvertTo-Json -Depth 6 -Compress -InputObject ([ordered]@{classification='cold-output-wrapper-worker-failed';failure=$failure;cause=$failureCause;cleanup_first_cause=$cleanupFirstCause;cleanup_unknown=$controllerCleanupUnknown})
    if($diagnostic.Length -le 4000){[Console]::Error.WriteLine($diagnostic)}else{[Console]::Error.WriteLine('{"classification":"cold-output-wrapper-worker-failed","failure":"diagnostic_cap"}')}
}
exit $result
