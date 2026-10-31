[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,[Parameter(Mandatory)][long]$StartTimestamp)
# Dormant control-only test. Root activation required; never invokes production bootstrap or private roles.
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
$cleanupLimit=24
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
function Diagnostic($Item,[string]$Kind) {
    $message=[string]$(if($Item -is [Management.Automation.ErrorRecord]){$Item.Exception.Message}else{[string]$Item})
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2Control 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2Control 'diagnostic_cap'}
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
    }catch{$script:compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2Control 'compiler_error'}
    Need (-not $compilerError) 'compiler_error'
}
function Sha-Text([string]$Text) {return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.UTF8Encoding]::new($false).GetBytes($Text))).ToLowerInvariant()}
function Definition([string]$Text,[string]$Name) {
    $tokens=$null;$errors=$null;$ast=[Management.Automation.Language.Parser]::ParseInput($Text,[ref]$tokens,[ref]$errors)
    Need ($errors.Count -eq 0) 'definition_parse'
    $nodes=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $Name},$true))
    Need ($nodes.Count -eq 1) 'definition_count'
    return $nodes[0].Extent.Text
}
function Remap([string]$Text,[string]$Old,[string]$New,[int]$Count) {
    Need ($Text.IndexOf($New,[StringComparison]::Ordinal) -lt 0 -and [regex]::Matches($Text,[regex]::Escape($Old)).Count -eq $Count) 'literal_mapping'
    return $Text.Replace($Old,$New)
}
function Initialize-MappedOwner([string]$Case) {
    Need ($Case -cin @('parent-create','parent-existing')) 'mapping_scope'
    $ns=if($Case -ceq 'parent-create'){'Kilogram.S53.ControlCreated'}else{'Kilogram.S53.ControlExisting'}
    $target=if($Case -ceq 'parent-create'){$family+'\created-output'}else{$family+'\existing-output'}
    $original=Definition $outputText 'Initialize-S53ColdOutputAdapter'
    $mapped=Remap $original 'Kilogram.S53.ColdOutput' $ns 2
    $mapped=Remap $mapped $productionE $target 1
    $inverse=$mapped.Replace($target,$productionE).Replace($ns,'Kilogram.S53.ColdOutput')
    Need ($inverse -ceq $original) 'inverse_mapping'
    $mapping.Add([ordered]@{role=$Case;function='Initialize-S53ColdOutputAdapter';source_sha256=(Sha-Text $original);mapped_sha256=(Sha-Text $mapped);inverse_sha256=(Sha-Text $inverse)
        namespace_replacements=2;path_replacements=1;fixed_target=$target})
    $typeName=$ns+'.BaselineSmallOwner';Need ($null -eq ($typeName -as [type])) 'cached_type'
    Initialize-Reviewed $Case { . ([scriptblock]::Create($mapped));Initialize-S53ColdOutputAdapter }
    return $typeName -as [type]
}
function Initialize-MappedGuards {
    $original=Definition $productionText 'Initialize-ProductionGuards'
    $mapped=Remap $original $productionParent $family 1
    $inverse=$mapped.Replace($family,$productionParent);Need ($inverse -ceq $original) 'inverse_mapping'
    $mapping.Add([ordered]@{role='parent-guards';function='Initialize-ProductionGuards';source_sha256=(Sha-Text $original);mapped_sha256=(Sha-Text $mapped);inverse_sha256=(Sha-Text $inverse)
        namespace_replacements=0;path_replacements=1;fixed_target=$family})
    Need ($null -eq ('Kilogram.S53.ColdLaunch.ParentGuards' -as [type])) 'cached_type'
    Initialize-Reviewed 'parent-guards' {. ([scriptblock]::Create($mapped));Initialize-ProductionGuards}
}
function Parent-AclDigest {
    Check;$acl=[IO.FileSystemAclExtensions]::GetAccessControl([IO.DirectoryInfo]::new($family))
    $s=$acl.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]'Owner,Group,Access')
    Need ($s.Length -le 16384) 'acl_cap';return Sha-Text $s
}
function Parent-Case([string]$Name,[type]$OwnerType) {
    Check;$pg=$null;$owner=$null;$pgDisposed=$false;$ownerDisposed=$false;$observed=$null
    try {
        $pg=[Kilogram.S53.ColdLaunch.ParentGuards]::new($absoluteWorkDeadline,
            [Func[string,Microsoft.Win32.SafeHandles.SafeFileHandle]]{param($p)[S53RootNativeR38]::DirectoryHandle($p)},
            [Action[string]]{param($p)[S53RootNativeR38]::NewDirectory($p)},
            [Action[string]]{param($p)Assert-S53RootPrivateAcl $p $true})
        $pg.Prepare($true)
        $created=$Name -ceq 'parent-create'
        Need ($pg.ParentCreated -eq $created -and $pg.ParentExisted -eq (-not $created) -and $pg.Remaining -eq 5) 'parent_outcome'
        $aclBefore=Parent-AclDigest
        $owner=$OwnerType::CreateProduction($absoluteWorkDeadline);$owner.PrepareDirectory()
        $prepared=$owner.RemainingOwnedHandles;$ancestry=$owner.AncestryCount
        Need ($prepared -eq 6 -and $ancestry -eq 5) 'owner_geometry'
        $pg.Recheck();$pg.Dispose();$pgDisposed=$true
        Need ($pg.Remaining -eq 0 -and -not $pg.CleanupUnknown -and $pg.NativeCloseAttempts -eq 5 -and $pg.NativeClosed -eq 5) 'guard_cleanup'
        $afterHandoff=$owner.RemainingOwnedHandles
        $owner.RecheckAncestry();$owner.Dispose();$ownerDisposed=$true
        $aclAfter=Parent-AclDigest
        $observed=[ordered]@{name=$Name;observed=$true;parent_created=$pg.ParentCreated;parent_existing=$pg.ParentExisted;guard_peak=$pg.Peak;guard_open_attempts=$pg.OpenAttempts
            guard_close_attempts=$pg.NativeCloseAttempts;guard_native_closed=$pg.NativeClosed;guard_remaining=$pg.Remaining;guard_cleanup_unknown=$pg.CleanupUnknown;guard_native_error=$pg.CleanupNativeError
            owner_created=$owner.DirectoryCreated;owner_transferred=$owner.DirectoryTransferred;owner_ancestry=$ancestry;owner_prepared=$prepared;owner_after_handoff=$afterHandoff
            owner_remaining=$owner.RemainingOwnedHandles;owner_cleanup_unknown=$owner.CleanupUnknown;owner_native_error=$owner.CleanupNativeError;owner_receipt_mask=$owner.ReceiptCreatedMask;owner_receipt_transfers=$owner.ReceiptsTransferred
            guard_disposed=$pgDisposed;owner_disposed=$ownerDisposed;acl_before_sha256=$aclBefore;acl_after_sha256=$aclAfter;completed_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
            core_invoked=$false;capture_invoked=$false;receipt_invoked=$false}
        Assert-ParentCase $observed $created
    }finally{
        try{if($owner -and -not $ownerDisposed){$owner.Dispose()};if($owner -and ($owner.CleanupUnknown -or $owner.RemainingOwnedHandles -ne 0)){$script:controllerUnknown=$true}}
        catch{$script:controllerUnknown=$true;if(-not $script:cleanupCause){$script:cleanupCause=Cause $_.Exception}}
        try{if($pg -and -not $pgDisposed){$pg.Dispose()};if($pg -and ($pg.CleanupUnknown -or $pg.Remaining -ne 0)){$script:controllerUnknown=$true}}
        catch{$script:controllerUnknown=$true;if(-not $script:cleanupCause){$script:cleanupCause=Cause $_.Exception}}
    }
    return $observed
}
function Protocol-Case([string]$Name) {
    Need ($Name -cin @('protocol-match','protocol-wrong-pid')) 'protocol_scope'
    $fnNames=@('Stop-Cold','Need','Age','Check','Cause','Public-Register','Public-Open','Public-Close','Public-CloseAll','Read-Bytes','Hash-Stream','Decode','Unique-Json','Keys','Fields','Value','Exact','Array-Exact'
        'Pin-Spec','Validate-Core','Common-Report','Report-Fields','Validate-Cause','Validate-SourceMap','Validate-Report','Await-Start')
    $parts=[Collections.Generic.List[string]]::new()
    foreach($fnName in $fnNames){$d=Definition $productionText $fnName;$parts.Add($d)}
    $definitions=[string]::Join("
",$parts)
    Need ($definitions -cnotmatch '::GetProcesses\(|Invoke-Parent|Invoke-Supervisor|CaptureAndCompare\(') 'protocol_reachability'
    Need ((Sha-Text $definitions) -ceq 'ea95c30fa17ba918391708d70aa83a3c887426546f9f76d6a1e897b0ceb2a878') 'protocol_definitions_pin'
    $module=New-Module -Name ('S53Control_'+$Name.Replace('-','_')) -ScriptBlock ([scriptblock]::Create("Set-StrictMode -Version Latest
"+$definitions))
    $suffix=$Name+'.started.json';$fixtureAttempt=$attempt+'.'+$Name
    $record=[ordered]@{schema_version=1;classification='s53-cold-a-started';attempt=$fixtureAttempt;decision_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;whole_attempt_verdict='not_established'
        supervisor_pid=$claim.supervisor_pid;supervisor_started_utc=$claim.native_started_utc;parent_pid=$(if($Name -ceq 'protocol-match'){$workerSelf.Id}else{if($workerSelf.Id -eq 1){2}else{1}})
        parent_started_utc=$workerStartedUtc;source_pins=[ordered]@{launcher=$binding.Pins.production;output=$binding.Pins.output}
        permission_consumed=$true;work_seconds=45;whole_seconds=60}
    $file=New-PublicOutput $suffix;$safe=$file.SafeFileHandle
    try{$raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $record -Depth 6 -Compress));Need ($raw.Length -le 4096);$file.Write($raw,0,$raw.Length);$file.Flush($true)}
    finally{$file.Dispose();Need $safe.IsClosed 'protocol_writer_close'}
    $hash=Sha-Text ([Text.UTF8Encoding]::new($false).GetString($raw));$value=$null
    try {
        $value=& $module {
            param($PublicDirectory,$FixedAttempt,$Qpc,$SelfObject,$DeadlineGuard,$PinMap,$DecisionPin,$TaskRoot,$EntryPath,$CorePath,$OutputFile,$RootFile,$DeadlineFile,$Shell)
            $script:ErrorActionPreference='Stop'
            $script:e=$PublicDirectory;$script:attempt=$FixedAttempt;$script:StartTimestamp=$Qpc;$script:self=$SelfObject;$script:guard=$DeadlineGuard
            $script:DecisionSha256=$DecisionPin;$script:sourcePins=$PinMap;$script:cleanupReserve=$false;$script:cleanupLimit=24
            $script:publicRecords=[Collections.Generic.List[object]]::new();$script:publicOwned=0;$script:publicPeak=0;$script:managedCloseUnknown=$false
            $script:taskRoot=$TaskRoot;$script:entry=$EntryPath;$script:corePath=$CorePath;$script:outputPath=$OutputFile
            $script:rootNative=$RootFile;$script:deadlinePath=$DeadlineFile;$script:fixedShell=$Shell
            $returned=$false;$failure=$null;$ready=$null
            try{$ready=Await-Start;$returned=$true}catch{$failure=Cause $_.Exception}
            finally{Public-CloseAll}
            return [ordered]@{returned=$returned;cause=$failure;started_sha256=$(if($ready){$ready.Sha256}else{$null})
                public_peak=$publicPeak;public_remaining=$publicOwned;managed_disposed=(-not $managedCloseUnknown -and $publicOwned -eq 0);managed_native_close_observed=$false
                claim_created=$false;private_work_invoked=$false}
        } $e $fixtureAttempt $StartTimestamp $workerSelf $guard $record.source_pins $AllocationSha256 "$w\ai-docs\tasks" $production "$w\scripts\migration-s53-cold-preservation.ps1" $output $rootNative $deadline $shell
    }finally{Remove-Module -ModuleInfo $module -Force}
    $result=[ordered]@{name=$Name;function_test_only=$true;production_entrypoint_invoked=$false;record_sha256=$hash;record_bytes=$raw.Length;writer_managed_closed=$safe.IsClosed
        definitions_sha256=(Sha-Text $definitions);definition_count=$fnNames.Count;source_names=$fnNames;protocol=$value}
    Assert-Protocol $result ($Name -ceq 'protocol-match')
    return $result
}
$productionText=$null;$outputText=$null;$rootText=$null;$binding=$null;$claim=$null;$authorized=$false;$workerSelf=$null;$workerStartedUtc=$null;$workerPid=0;$workerSelfDisposed=$false
$family="$w\.tmp\$attempt"
$productionParent="$w\.tmp\migration-s5-preparation-evidence"
$productionE="$productionParent\P2-s53-exhausted-cold-a-01-attempt-01"
$mapping=[Collections.Generic.List[object]]::new();$parentCases=[Collections.Generic.List[object]]::new();$protocolCases=[Collections.Generic.List[object]]::new()
$initializeCalls=[ordered]@{};$diagnostics=[Collections.Generic.List[object]]::new();$diagnosticChars=0;$compilerError=$false;$diagnosticCap=$false
$controllerUnknown=$false;$cleanupCause=$null;$sourceBefore=[ordered]@{};$sourceAfter=[ordered]@{};$managedDisposed=$false;$publicInputsBeforeClose=0
$childrenAfter=$null;$jobFinished=$false;$reportClosed=$false;$failure='not_started';$failureCause=$null;$candidate=$false;$result=1
$absoluteWorkDeadline=$StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency
try {
    Need ($StartTimestamp -gt 0 -and $PSCommandPath -ceq $preflight) 'allocation_scope'
    $workerSelf=[Diagnostics.Process]::GetCurrentProcess();$workerSelf.PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    $workerPid=$workerSelf.Id;$workerStartedUtc=$workerSelf.StartTime.ToUniversalTime().ToString('o')
    $binding=Validate-Allocation;$authorized=$true;$sourceBefore=Source-Pins $binding
    $started=Open-Public "$e\$attempt.started.json" 65536;$claim=Decode-Public (Bytes $started 65536)
    Assert-Fields $claim ([ordered]@{classification='cold-production-control-c2-start-t01';attempt=$attempt;allocation_sha256=$AllocationSha256
        start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;permission_consumed=$true;work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1})
    Need ($claim.supervisor_pid -gt 0 -and (Age) -lt 15) 'bootstrap_reserve'
    Need ($null -eq ('KilogramS5Deadline' -as [type])) 'cached_type'
    Initialize-Reviewed 'deadline' {. $deadline};$guard=[KilogramS5Deadline]::new((20-(Age)),(24-(Age)));Check
    $productionText=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $binding.SourceFiles.production 131072))
    $outputText=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $binding.SourceFiles.output 131072))
    $rootText=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $binding.SourceFiles.root_native 131072))
    Need ($null -eq ('Kilogram.S53.ColdA.Core' -as [type]) -and $null -eq ('S53RootNativeR38' -as [type])) 'cached_type'
    $rootDefinitions=[string]::Join("
",@((Definition $rootText 'Stop-S53RootCopy'),(Definition $rootText 'Initialize-S53RootNative'),(Definition $rootText 'Assert-S53RootPrivateAcl')))
    . ([scriptblock]::Create($rootDefinitions))
    Initialize-Reviewed 'root-native' {Initialize-S53RootNative}
    Initialize-MappedGuards
    $newType=Initialize-MappedOwner 'parent-create'
    $existingType=Initialize-MappedOwner 'parent-existing'
    $parentCases.Add((Parent-Case 'parent-create' $newType));Check
    $parentCases.Add((Parent-Case 'parent-existing' $existingType));Check
    $protocolCases.Add((Protocol-Case 'protocol-match'));Check
    $protocolCases.Add((Protocol-Case 'protocol-wrong-pid'));Check
    $sourceAfter=Source-Pins $binding
    Need ($null -eq ('Kilogram.S53.ColdA.Core' -as [type])) 'unexpected_core'
    Check;$candidate=$true;$failure=$null
}catch{$failure=Failure-Code $_.Exception;$failureCause=Cause $_.Exception}
finally {
    $cleanupReserve=$true
    $publicInputsBeforeClose=$held.Count;$managedDisposed=Close-PublicInputs
    if(-not $managedDisposed){$controllerUnknown=$true}
    if($guard){try{$guard.CleanupChildren([int][Math]::Max(0,[Math]::Min(500,(24-(Age))*1000)));$childrenAfter=@($guard.Children()).Count
        Need ($childrenAfter -eq 0) 'owned_children';$guard.Finish();$jobFinished=$true}catch{$controllerUnknown=$true;if(-not $cleanupCause){$cleanupCause=Cause $_.Exception}}}
    if($workerSelf){try{$workerSelf.Dispose();$workerSelfDisposed=$true}catch{$controllerUnknown=$true}}
}
try {
    Check;if($controllerUnknown -or -not $managedDisposed -or -not $jobFinished){$candidate=$false;if(-not $failure){$failure='cleanup_unknown'}}
    $nativeGuardCloses=0;foreach($c in $parentCases){$nativeGuardCloses+=$c.guard_native_closed}
    $report=[ordered]@{classification='cold-production-control-c2-t01';attempt=$attempt;allocation_sha256=$AllocationSha256;result=$(if($candidate){'observed'}else{'failed_or_unknown'})
        failure=$failure;failure_cause=$failureCause;cleanup_cause=$cleanupCause;controller_unknown=$controllerUnknown;whole_attempt_verdict='not_established'
        child_pid=$workerPid;worker_started_utc=$workerStartedUtc;started_supervisor_pid=$(if($claim){$claim.supervisor_pid}else{0});start_timestamp=$StartTimestamp;stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency
        source_before=$sourceBefore;source_after=$sourceAfter;mapping=$mapping.ToArray();parent_cases=$parentCases.ToArray();protocol_cases=$protocolCases.ToArray()
        initialization_calls=$initializeCalls;diagnostics=$diagnostics.ToArray();compiler_error=$compilerError;diagnostic_cap=$diagnosticCap
        protocol_function_test_only=$true;production_entrypoint_invoked=$false;core_type_absent=($null -eq ('Kilogram.S53.ColdA.Core' -as [type]))
        capture_invoked=$false;inventory_invoked=$false;receipt_invoked=$false;private_dataset_access=$false;fixture_retained=$true
        managed_inputs_disposed=$managedDisposed;public_inputs_before_close=$publicInputsBeforeClose;worker_self_disposed=$workerSelfDisposed;job_finished=$jobFinished;owned_children_after=$childrenAfter
        native_guard_closes=$nativeGuardCloses;owner_lifecycles=$parentCases.Count;explicit_helper_peak_bound=11;combined_controlled_bound=32
        work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1;observed_seconds=(Age)}
    if($candidate){Assert-WorkerReport $report $binding.Pins $workerPid $workerStartedUtc $claim.supervisor_pid}
    if($authorized){$file=New-PublicOutput 'cases.json';$safe=$file.SafeFileHandle
        try{Write-Public $file $report}finally{$file.Dispose();$reportClosed=$safe.IsClosed}
        Need $reportClosed 'report_close'
    }
    Check;if($candidate -and $reportClosed){$result=0}
}catch{$result=1}
if($result -ne 0){[Console]::Error.WriteLine('{"classification":"cold-production-control-worker-failed","result":"failed_or_unknown"}')}
exit $result
