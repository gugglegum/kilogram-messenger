[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,
    [Parameter(Mandatory)][long]$StartTimestamp
)
# C2-G filesystem aliases: dormant source; no runtime allocation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-filesystem-aliases-01-attempt-01'
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
function Stop-C2G([string]$Code) {
    $err=[InvalidOperationException]::new('C2G bounded refusal');$err.Data['c2g_code']=$Code;throw $err
}
function Check {
    $age=Age
    if($age -lt 0 -or $age -ge 20){Stop-C2G 'work_expired'}
    if($guard){$guard.CheckWork()}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        $value=[string]$at.Data['c2g_code']
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
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2G 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2G 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2G 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2G 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2G 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2G 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2G 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2G 'source_pin'}
    return $f
}
function Validate-Allocation {
    Check
    if(-not $IsWindows -or [Environment]::ProcessPath -cne $shell){Stop-C2G 'allocation_scope'}
    $af=Open-Public $allocation 65536
    if((Hash-Stream $af) -cne $AllocationSha256){Stop-C2G 'allocation_pin'}
    $raw=Bytes $af 65536
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    if($text.Length -eq 0 -or $text[0] -eq [char]0xfeff){Stop-C2G 'allocation_schema'}
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        if($doc.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object){Stop-C2G 'allocation_schema'}
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($property in $doc.RootElement.EnumerateObject()){
            if($property.Name -cnotin $fields -or -not $map.TryAdd($property.Name,$property.Value)){Stop-C2G 'allocation_schema'}
        }
        if($map.Count -ne 22){Stop-C2G 'allocation_schema'}
        foreach($name in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256')){
            if($map[$name].ValueKind -ne [Text.Json.JsonValueKind]::String){Stop-C2G 'allocation_schema'}
        }
        if($map['schema_version'].GetInt32() -ne 1 -or $map['classification'].GetString() -cne 'root-approved-cold-core-filesystem-aliases-c2-r01' -or
           $map['root_thread_id'].GetString() -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or
           $map['activated'].ValueKind -ne [Text.Json.JsonValueKind]::True -or $map['attempt'].GetString() -cne $attempt -or
           $map['operation_set'].GetString() -cne 'cold-filesystem-aliases-two-only-r01' -or $map['workspace'].GetString() -cne $w -or
           $map['fixture_parent'].GetString() -cne ($w+'\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01') -or
           $map['work_seconds'].GetInt32() -ne 20 -or $map['whole_seconds'].GetInt32() -ne 30 -or
           $map['retry_count'].GetInt32() -ne 0 -or $map['lane_count'].GetInt32() -ne 1 -or
           $map['public_report_bytes'].GetInt32() -ne 65536 -or $map['public_total_bytes'].GetInt32() -ne 196608){Stop-C2G 'allocation_scope'}
        $cases=$map['fixture_cases'];$fixed=@((Case-Specs).Selector)
        if($cases.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $cases.GetArrayLength() -ne 2){Stop-C2G 'allocation_scope'}
        for($n=0;$n -lt 2;$n++){
            if($cases[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $cases[$n].GetString() -cne $fixed[$n]){Stop-C2G 'allocation_scope'}
        }
        $outputs=$map['outputs']
        if($outputs.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $outputs.GetArrayLength() -ne 3){Stop-C2G 'allocation_schema'}
        $names=@('started.json','cases.json','watchdog.json')
        for($n=0;$n -lt 3;$n++){if($outputs[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $outputs[$n].GetString() -cne $names[$n]){Stop-C2G 'allocation_scope'}}
        $pins=[ordered]@{}
        foreach($name in @('preflight','watchdog','core','root_native','deadline','shell')){
            $value=$map[$name+'_sha256'].GetString()
            if($value -cnotmatch '\A[0-9a-f]{64}\z'){Stop-C2G 'allocation_schema'};$pins[$name]=$value
        }
        if($pins.core -cne 'afe18a0f4ef306f0c59e941445cdc43da0c7eb28071230a42ffbb6001b05866e' -or
           $pins.root_native -cne 'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382' -or
           $pins.deadline -cne '0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45' -or
           $pins.shell -cne '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'){Stop-C2G 'source_pin'}
        $pf=Open-Public $preflight 131072;$wf=Open-Public $watchdog 131072
        if((Hash-Stream $pf) -cne $pins.preflight -or (Hash-Stream $wf) -cne $pins.watchdog){Stop-C2G 'source_pin'}
        $sourceFiles=[ordered]@{}
        $sourceFiles.core=Pin $core 74989 $pins.core
        $sourceFiles.root_native=Pin $rootNative 25596 $pins.root_native
        $sourceFiles.deadline=Pin $deadline 6038 $pins.deadline
        $null=Pin $shell 301368 $pins.shell
        return [pscustomobject]@{Pins=$pins;SourceFiles=$sourceFiles}
    }finally{$doc.Dispose()}
}
function New-PublicOutput([string]$Suffix) {
    if($Suffix -cnotin @('started.json','cases.json','watchdog.json')){Stop-C2G 'output_cap'}
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2G 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}
function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}

function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2G $Code}}
function Case-Specs {
    return @(
        [pscustomobject]@{Name='payload-hardlink';Selector='aliases-payload-hardlink';Directory=$false;InventoryBytes=3618;InventoryHash='c0ab7a5a81251abc9612a48be20f557679f01bbb72de2d8aa1f5b72835862ad7';PreHook='capture_s';FailureCode='file_alias';FailurePhase='capture_s';FailureRole='s';CoreLocks=6;Acquired=2;PreLocks=6;PreOwned=26;PrePeak=27;PreRecord=12084624;PrePath=7368;PeakHandles=28;PeakRecord=12085392;PeakPath=7368;InventoryPeak=3618;PreInventoryPeak=3618;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';SFiles=5;SBytes=52;BFiles=5;BBytes=52;FileCount=13;TargetRelative='state/one/state/payload.bin';Phases=@('locks','inventory','capture_s');PreKeys=@('allocation','locks','inventory')},
        [pscustomobject]@{Name='directory-reparse';Selector='aliases-directory-reparse';Directory=$true;InventoryBytes=3622;InventoryHash='2201cf6fa12d08b624c830fc84986bf3f9baebceb2009056cc46e7a45589eeee';PreHook='locks';FailureCode='reparse';FailurePhase='locks';FailureRole='s_one_lock';CoreLocks=2;Acquired=0;PreLocks=0;PreOwned=0;PrePeak=0;PreRecord=12073840;PrePath=0;PeakHandles=14;PeakRecord=12079216;PeakPath=3128;InventoryPeak=0;PreInventoryPeak=0;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';SFiles=5;SBytes=52;BFiles=5;BBytes=52;FileCount=13;TargetRelative='state/one/state';Phases=@('locks');PreKeys=@('allocation')}
    )
}
function Version-Text([string]$Value) {
    Need ($Value -cmatch '\A[A-Za-z0-9._+-]{1,64}\z') 'signature_mismatch'
    return $Value
}
function Cause($Exception) {
    $last=$Exception;$native=0;$marks=[ordered]@{};$depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++;$last=$at
        if($at -is [ComponentModel.Win32Exception]){$native=$at.NativeErrorCode}
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2g_code','c2g_fixture_code','c2g_fixture_operation')){
            $value=[string]$at.Data[$key]
            if($value -cmatch '\A[a-z0-9_]{1,64}\z'){$marks[$key]=$value}
        }
    }
    $type=$last.GetType().FullName
    if($type -cnotmatch '\A[A-Za-z0-9_.+]{1,128}\z'){$type='bounded_unknown'}
    return [ordered]@{type=$type;hresult=$last.HResult;native_error=$native;marks=$marks}
}
function Payloads {
    return @(
        [pscustomobject]@{Path='one/Preservation-Inventory.json';Bytes=23;Hash='6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3';Hex='7b2264756d6d79223a226e65737465642d6f6e65227d0a'},
        [pscustomobject]@{Path='one/state/.kilogram-state.lock';Bytes=0;Hash='e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';Hex=''},
        [pscustomobject]@{Path='one/state/payload.bin';Bytes=6;Hash='ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4';Hex='00017f80ff0a'},
        [pscustomobject]@{Path='two/preservation-inventory.json';Bytes=23;Hash='76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db';Hex='7b2264756d6d79223a226e65737465642d74776f227d0a'},
        [pscustomobject]@{Path='two/state/.kilogram-state.lock';Bytes=0;Hash='e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';Hex=''}
    )
}
function Inventory-Literal([string]$Name) {
    switch -CaseSensitive ($Name) {
        'payload-hardlink' { return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\payload-hardlink\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\payload-hardlink\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\payload-hardlink\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\payload-hardlink\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@
        }
        'directory-reparse' { return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\directory-reparse\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\directory-reparse\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\directory-reparse\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-filesystem-aliases-01-attempt-01\\directory-reparse\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@
        }
        default {Stop-C2G 'fixture_setup'}
    }
}
function Initialize-C2GPremises {
    if(('S53C2GStreamsR01' -as [type]) -or ('S53C2GAliasR01' -as [type])){Stop-C2G 'core_type_cached'}
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public sealed class S53C2GStreamsR01 {
    const string Parent=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01";
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
        public long Length {get{return ((long)SizeHigh<<32)|SizeLow;}}
        public bool Same(Info b) {
            return Attributes==b.Attributes&&Volume==b.Volume&&SizeHigh==b.SizeHigh&&SizeLow==b.SizeLow&&Links==b.Links&&
                IndexHigh==b.IndexHigh&&IndexLow==b.IndexLow&&Created.dwHighDateTime==b.Created.dwHighDateTime&&
                Created.dwLowDateTime==b.Created.dwLowDateTime&&Written.dwHighDateTime==b.Written.dwHighDateTime&&
                Written.dwLowDateTime==b.Written.dwLowDateTime;
        }
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StreamInfo {
        public long Size;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=296)] public string Name;
    }
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint GetFileType(SafeFileHandle h);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern IntPtr FindFirstStreamW(string p,int level,out StreamInfo data,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern bool FindNextStreamW(IntPtr h,out StreamInfo data);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool FindClose(IntPtr h);
    readonly string root; readonly bool family; readonly Action work;
    readonly StringBuilder finalPath=new StringBuilder(512);
    public int FirstCalls {get;private set;}
    public int NextCalls {get;private set;}
    public int OpenedHandles {get;private set;}
    public int ClosedHandles {get;private set;}
    public int RemainingHandles {get;private set;}
    public int PeakHandles {get;private set;}
    public int ChecksCompleted {get;private set;}
    public int DirectoriesVerified {get;private set;}
    public int FilesVerified {get;private set;}
    public bool CleanupUnknown {get;private set;}
    public int CleanupNativeError {get;private set;}
    public string FailureCode {get;private set;}
    public int NativeError {get;private set;}
    public S53C2GStreamsR01(string name,Action check) {
        string[] names={"family","payload-hardlink","directory-reparse"};
        if(Array.IndexOf(names,name)<0||check==null)throw new ArgumentException("Closed C2G premise scope.");
        family=name=="family";root=family?Parent:Parent+"\\"+name;work=check;
    }
    Exception Refuse(string code,int native=0) {
        if(FailureCode==null){FailureCode=code;NativeError=native;}
        var e=native==0?new InvalidOperationException("C2G premise refusal"):
            new InvalidOperationException("C2G premise native refusal",new Win32Exception(native));
        e.Data["c2g_premise_code"]=code;return e;
    }
    Info HeldInfo(SafeFileHandle held,string path,bool directory,long length) {
        if(held==null||held.IsInvalid||held.IsClosed)throw Refuse("premise_handle");
        Info i;bool ok=GetFileInformationByHandle(held,out i);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)throw Refuse("premise_info",error);
        uint type=GetFileType(held);error=type==0?Marshal.GetLastWin32Error():0;
        if(type!=1)throw Refuse("premise_type",error);
        if((i.Attributes&(0x400u|0x40u))!=0||((i.Attributes&0x10u)!=0)!=directory||
            (!directory&&(i.Links!=1||i.Length!=length)))throw Refuse("premise_shape");
        finalPath.Clear();uint n=GetFinalPathNameByHandleW(held,finalPath,512,0);error=n==0?Marshal.GetLastWin32Error():0;
        if(n==0)throw Refuse("premise_path",error);
        if(n>=512)throw Refuse("premise_path");
        string actual=finalPath.ToString();
        if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual.Substring(4);
        if(!String.Equals(actual,path,StringComparison.OrdinalIgnoreCase))throw Refuse("premise_identity_path");
        return i;
    }
    public void Verify(string path,SafeFileHandle held,bool directory,long length) {
        if(path==null||path.Length>480||length<0||length>4096||
            !String.Equals(Path.GetFullPath(path),path,StringComparison.Ordinal)||
            !(String.Equals(path,root,StringComparison.Ordinal)||(!family&&path.StartsWith(root+"\\",StringComparison.Ordinal))))
            throw Refuse("premise_scope");
        if(CleanupUnknown||RemainingHandles!=0||FirstCalls>=32)throw Refuse("premise_cap_or_cleanup");
        Exception primary=null;IntPtr find=new IntPtr(-1);bool owned=false;
        try {
            work();Info before=HeldInfo(held,path,directory,length);work();
            StreamInfo data;FirstCalls++;find=FindFirstStreamW(path,0,out data,0);
            bool invalid=find==IntPtr.Zero||find==new IntPtr(-1);
            int error=invalid?Marshal.GetLastWin32Error():0;
            // Own current successful handle BEFORE callback/work/shape validation.
            if(!invalid){owned=true;RemainingHandles=1;PeakHandles=1;OpenedHandles++;}
            if(invalid) {
                if(!directory||error!=38)throw Refuse(error==87?"streams_unsupported":"stream_enumeration_error",error);
            } else {
                work();
                if(directory||!String.Equals(data.Name,"::$DATA",StringComparison.Ordinal)||data.Size!=length)
                    throw Refuse("named_or_changed_stream");
                work();NextCalls++;bool next=FindNextStreamW(find,out data);error=next?0:Marshal.GetLastWin32Error();
                if(next)throw Refuse("extra_stream");
                if(error!=38)throw Refuse("stream_enumeration_error",error);
            }
            work();Info after=HeldInfo(held,path,directory,length);
            if(!before.Same(after))throw Refuse("premise_identity_changed");
            work();
        } catch(Exception e) {primary=e;if(FailureCode==null)FailureCode="callback_or_unknown";}
        finally {
            if(owned){
                try {
                    bool ok=FindClose(find);int error=ok?0:Marshal.GetLastWin32Error();
                    if(ok){RemainingHandles=0;ClosedHandles++;}
                    else{CleanupUnknown=true;CleanupNativeError=error;}
                } catch {CleanupUnknown=true;} // exactly one attempt; no retry/no false release
            }
        }
        if(primary!=null)ExceptionDispatchInfo.Capture(primary).Throw();
        if(CleanupUnknown)throw Refuse("stream_cleanup",CleanupNativeError);
        ChecksCompleted++;if(directory)DirectoriesVerified++;else FilesVerified++;
    }
}

public sealed class S53C2GAliasR01 : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes; public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint Volume,High,Low,Links,IndexHigh,IndexLow;
        public long Length {get{return ((long)High<<32)|Low;}}
        public bool Directory {get{return (Attributes&16)!=0;}}
        public bool SameId(Info b){return Volume==b.Volume&&IndexHigh==b.IndexHigh&&IndexLow==b.IndexLow;}
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StreamData {
        public long Size; [MarshalAs(UnmanagedType.ByValTStr,SizeConst=296)] public string Name;
    }
    sealed class Slot {public Microsoft.Win32.SafeHandles.SafeFileHandle H;public bool Attempted,Closed;}
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern Microsoft.Win32.SafeHandles.SafeFileHandle CreateFileW(string p,uint a,uint s,IntPtr security,uint creation,uint flags,IntPtr template);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(Microsoft.Win32.SafeHandles.SafeFileHandle h,out Info info);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint GetFileType(Microsoft.Win32.SafeHandles.SafeFileHandle h);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetFinalPathNameByHandleW(Microsoft.Win32.SafeHandles.SafeFileHandle h,System.Text.StringBuilder s,uint n,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool MoveFileExW(string a,string b,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateHardLinkW(string newName,string existing,IntPtr security);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool DeviceIoControl(Microsoft.Win32.SafeHandles.SafeFileHandle h,uint code,byte[] input,uint inputLength,byte[] output,uint outputLength,out uint returned,IntPtr overlapped);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetFilePointerEx(Microsoft.Win32.SafeHandles.SafeFileHandle h,long offset,out long actual,uint origin);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadFile(Microsoft.Win32.SafeHandles.SafeFileHandle h,byte[] b,uint n,out uint read,IntPtr overlapped);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr FindFirstStreamW(string path,int level,out StreamData data,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool FindNextStreamW(IntPtr find,out StreamData data);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool FindClose(IntPtr find);
    const string Parent=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01";
    readonly bool directory;readonly string target,extra;readonly Action check;
    readonly System.Collections.Generic.List<Slot> slots=new System.Collections.Generic.List<Slot>(8);
    readonly byte[] six=new byte[6],expected={0,1,127,128,255,10};
    readonly System.Text.StringBuilder finalName=new System.Text.StringBuilder(512);
    Info original,junctionIdentity;bool haveJunction,captured,prepared,verified,disposed,unknown,partial,moved,created;
    IntPtr currentFind=new IntPtr(-1);bool findAttempted;
    int prepareCalls,verifyCalls,disposeCalls,opens,returns,closed,closeAttempts,peak,current,finds,findClosed,readBytes,reads,moves,links,sets,gets;
    int nativeError,cleanupNativeError;string operation="",failureType="",cleanupOperation="",cleanupType="";int failureHr,cleanupHr;
    public S53C2GAliasR01(string name,Action callback){
        if(name!="directory-reparse"&&name!="payload-hardlink"||callback==null)throw new ArgumentException("closed alias case");
        directory=name=="directory-reparse";check=callback;
        string root=Parent+@"\"+name;
        target=root+(directory?@"\state\one\state":@"\state\one\state\payload.bin");
        extra=root+(directory?@"\alias-target":@"\alias-link");
        if(target.Length>480||extra.Length>480||System.IO.Path.GetFullPath(target)!=target||System.IO.Path.GetFullPath(extra)!=extra||
            !target.StartsWith(root+@"\",StringComparison.Ordinal)||!extra.StartsWith(root+@"\",StringComparison.Ordinal))throw new ArgumentException("path bound");
    }
    Exception Fail(string op,int error=0) {
        if(operation==""){operation=op;nativeError=error;}
        var e=error==0?new InvalidOperationException("C2G fixture refusal"):new InvalidOperationException("C2G fixture refusal",new System.ComponentModel.Win32Exception(error));
        e.Data["c2g_fixture_code"]="alias_setup";e.Data["c2g_fixture_operation"]=op;e.Data["c2g_fixture_native_error"]=error;return e;
    }
    void Save(Exception e){if(failureType==""){failureType=e.GetType().FullName;failureHr=e.HResult;}}
    void SaveCleanup(Exception e){unknown=true;if(cleanupOperation==""){cleanupOperation="file_close";cleanupType=e.GetType().FullName;cleanupHr=e.HResult;}}
    Slot Open(string path,bool dir,uint access=0x80000000u,uint share=1){
        check();if(slots.Count>=8)throw Fail("owner_cap");
        var s=new Slot();slots.Add(s);current++;peak=Math.Max(peak,current);opens++;
        try{
            s.H=CreateFileW(@"\\?\"+path,access,share,IntPtr.Zero,3,dir?0x02200000u:0x00200000u,IntPtr.Zero);
            int error=s.H.IsInvalid?Marshal.GetLastWin32Error():0;returns++;
            if(s.H.IsInvalid)throw Fail("open",error);
            check();return s;
        }catch{if(s.H==null)partial=true;throw;}
    }
    Info Metadata(Microsoft.Win32.SafeHandles.SafeFileHandle h,bool dir,bool reparse=false){
        check();Info i;bool ok=GetFileInformationByHandle(h,out i);int e=ok?0:Marshal.GetLastWin32Error();
        if(!ok)throw Fail("metadata",e);
        if(i.Directory!=dir||(i.Attributes&64)!=0||((i.Attributes&1024)!=0)!=reparse)throw Fail("kind");
        uint type=GetFileType(h);e=type==0?Marshal.GetLastWin32Error():0;if(type!=1)throw Fail("disk",e);
        check();return i;
    }
    void Path(Microsoft.Win32.SafeHandles.SafeFileHandle h,string path){
        check();finalName.Clear();uint n=GetFinalPathNameByHandleW(h,finalName,512,0);int e=n==0?Marshal.GetLastWin32Error():0;
        if(n==0||n>=512)throw Fail("final_path",e);
        string got=finalName.ToString();if(got.StartsWith(@"\\?\",StringComparison.Ordinal))got=got.Substring(4);
        if(!String.Equals(got,path,StringComparison.OrdinalIgnoreCase))throw Fail("final_path");check();
    }
    void Close(Slot s){
        if(s==null||s.Attempted)return;s.Attempted=true;closeAttempts++;
        if(s.H==null){unknown=true;return;}
        try{s.H.Dispose();if(!s.H.IsClosed){unknown=true;return;}s.Closed=true;closed++;current--;}catch(Exception e){SaveCleanup(e);}
    }
    void DefaultBytes(Slot s){
        check();long at;if(!SetFilePointerEx(s.H,0,out at,0))throw Fail("seek",Marshal.GetLastWin32Error());
        if(at!=0)throw Fail("seek_result");
        uint n;bool ok=ReadFile(s.H,six,6,out n,IntPtr.Zero);int e=ok?0:Marshal.GetLastWin32Error();reads++;
        if(!ok)throw Fail("read",e);if(n!=6)throw Fail("read_count");readBytes+=6;
        for(int i=0;i<6;i++)if(six[i]!=expected[i])throw Fail("read_bytes");
        ok=ReadFile(s.H,six,1,out n,IntPtr.Zero);e=ok?0:Marshal.GetLastWin32Error();reads++;
        if(!ok)throw Fail("eof",e);if(n!=0)throw Fail("eof");
        check();
    }
    void Streams(string path){
        if(finds>=4||currentFind!=new IntPtr(-1))throw Fail("find_cap");
        check();findAttempted=false;StreamData d;
        currentFind=FindFirstStreamW(@"\\?\"+path,0,out d,0);int e=(currentFind==new IntPtr(-1)||currentFind==IntPtr.Zero)?Marshal.GetLastWin32Error():0;
        if(currentFind==new IntPtr(-1)||currentFind==IntPtr.Zero){currentFind=new IntPtr(-1);throw Fail("stream_first",e);}
        finds++;
        try{
            if(d.Name!="::$DATA"||d.Size!=6)throw Fail("stream_set");
            bool more=FindNextStreamW(currentFind,out d);e=more?0:Marshal.GetLastWin32Error();
            if(more||e!=38)throw Fail("stream_eof",e);check();
        }finally{
            findAttempted=true;bool ok=FindClose(currentFind);e=ok?0:Marshal.GetLastWin32Error();
            if(ok){findClosed++;currentFind=new IntPtr(-1);}else{unknown=true;if(cleanupOperation==""){cleanupOperation="find_close";cleanupNativeError=e;}}
        }
    }
    byte[] MountBuffer(){
        byte[] sub=System.Text.Encoding.Unicode.GetBytes(@"\??\"+extra),print=System.Text.Encoding.Unicode.GetBytes(extra);
        int n=20+sub.Length+print.Length;if(n>4096)throw Fail("reparse_cap");
        byte[] b=new byte[n];
        Array.Copy(BitConverter.GetBytes(0xA0000003u),0,b,0,4);
        Array.Copy(BitConverter.GetBytes(checked((ushort)(n-8))),0,b,4,2);
        Array.Copy(BitConverter.GetBytes(checked((ushort)sub.Length)),0,b,10,2);
        Array.Copy(BitConverter.GetBytes(checked((ushort)(sub.Length+2))),0,b,12,2);
        Array.Copy(BitConverter.GetBytes(checked((ushort)print.Length)),0,b,14,2);
        Array.Copy(sub,0,b,16,sub.Length);Array.Copy(print,0,b,18+sub.Length,print.Length);return b;
    }
    void Junction(Slot junction){
        Info identity=Metadata(junction.H,true,true);
        if(haveJunction&&!identity.SameId(junctionIdentity))throw Fail("junction_changed");
        byte[] expected=MountBuffer(),buffer=new byte[4096];uint n;
        bool ok=DeviceIoControl(junction.H,0x000900A8,null,0,buffer,4096,out n,IntPtr.Zero);int e=ok?0:Marshal.GetLastWin32Error();gets++;
        if(!ok)throw Fail("reparse_get",e);
        if(n!=expected.Length)throw Fail("reparse_length");
        for(int i=0;i<expected.Length;i++)if(buffer[i]!=expected[i])throw Fail("reparse_target");
        if(!identity.SameId(Metadata(junction.H,true,true)))throw Fail("junction_changed");
        junctionIdentity=identity;haveJunction=true;
    }
    public void CaptureBaseline(Microsoft.Win32.SafeHandles.SafeFileHandle borrowed){
        if(captured||prepared||disposed)throw Fail("baseline_scope");
        if(directory){
            if(borrowed==null||borrowed.IsClosed||borrowed.IsInvalid)throw Fail("borrowed");
            original=Metadata(borrowed,true);Path(borrowed,target);
        }
        captured=true;
    }
    public void Prepare(Action createRestrictedDirectory){
        if(!captured||prepared||disposed||prepareCalls!=0)throw Fail("prepare_scope");prepareCalls++;
        try{
            if(directory){
                check();bool ok=MoveFileExW(@"\\?\"+target,@"\\?\"+extra,0);int e=ok?0:Marshal.GetLastWin32Error();moves++;
                if(!ok)throw Fail("move",e);moved=true;check();
                if(createRestrictedDirectory==null)throw Fail("directory_creator");createRestrictedDirectory();created=true;check();
                Slot mount=Open(target,true,0xC0000000u,0),dest=null;
                try{
                    Info baseInfo=Metadata(mount.H,true);byte[] b=MountBuffer();uint n;
                    ok=DeviceIoControl(mount.H,0x000900A4,b,(uint)b.Length,null,0,out n,IntPtr.Zero);e=ok?0:Marshal.GetLastWin32Error();sets++;
                    if(!ok)throw Fail("reparse_set",e);
                    Junction(mount);if(!baseInfo.SameId(Metadata(mount.H,true,true))||baseInfo.SameId(original))throw Fail("junction_identity");dest=Open(extra,true,0x80u,3);Info now=Metadata(dest.H,true);Path(dest.H,extra);
                    if(!original.SameId(now))throw Fail("moved_identity");
                }finally{Close(dest);Close(mount);}
            }else{
                Slot source=Open(target,false,0x80000000u,7),alias=null;
                try{
                    original=Metadata(source.H,false);Path(source.H,target);
                    if(original.Links!=1||original.Length!=6)throw Fail("baseline_links");DefaultBytes(source);
                    check();bool ok=CreateHardLinkW(@"\\?\"+extra,@"\\?\"+target,IntPtr.Zero);int e=ok?0:Marshal.GetLastWin32Error();links++;
                    if(!ok)throw Fail("hardlink_create",e);created=true;check();
                    Info now=Metadata(source.H,false);alias=Open(extra,false,0x80000000u,7);Info twin=Metadata(alias.H,false);
                    if(!original.SameId(now)||!now.SameId(twin)||now.Links!=2||twin.Links!=2||now.Length!=6||twin.Length!=6)throw Fail("hardlink_identity");
                    DefaultBytes(alias);Streams(target);Streams(extra);
                }finally{Close(alias);Close(source);}
            }
            if(unknown||partial||current!=0||findClosed!=finds)throw Fail("prepare_cleanup");
            prepared=true;
        }catch(Exception e){Save(e);throw;}
    }
    public void VerifyPreparedTarget(){
        if(!prepared||disposed||verifyCalls!=0)throw Fail("verify_scope");verifyCalls++;
        try{
            if(directory){
                Slot mount=Open(target,true,0x80u,3),dest=null;
                try{Junction(mount);dest=Open(extra,true,0x80u,3);Info now=Metadata(dest.H,true);Path(dest.H,extra);if(!original.SameId(now))throw Fail("target_identity");}
                finally{Close(dest);Close(mount);}
            }else{
                Slot source=Open(target,false),alias=null;
                try{
                    alias=Open(extra,false);Info a=Metadata(source.H,false),b=Metadata(alias.H,false);
                    if(!a.SameId(original)||!a.SameId(b)||a.Links!=2||b.Links!=2||a.Length!=6||b.Length!=6)throw Fail("prepared_links");
                    DefaultBytes(source);DefaultBytes(alias);Streams(target);Streams(extra);
                    Info afterA=Metadata(source.H,false),afterB=Metadata(alias.H,false);
                    if(!a.SameId(afterA)||!b.SameId(afterB)||afterA.Links!=2||afterB.Links!=2||afterA.Length!=6||afterB.Length!=6)throw Fail("prepared_stability");
                }finally{Close(alias);Close(source);}
            }
            if(unknown||partial||current!=0||findClosed!=finds)throw Fail("verify_cleanup");verified=true;
        }catch(Exception e){Save(e);throw;}
    }
    public System.Collections.Generic.Dictionary<string,object> Snapshot(){
        return new System.Collections.Generic.Dictionary<string,object>{
            {"prepare_calls",prepareCalls},{"verify_calls",verifyCalls},{"dispose_calls",disposeCalls},
            {"prepared",prepared},{"verified",verified},{"disposed",disposed},{"created",created},{"moved",moved},
            {"file_open_calls",opens},{"file_returns",returns},{"file_closed",closed},{"close_attempts",closeAttempts},{"current_file_slots",current},
            {"retained_file_owners",current},{"peak_file_results",peak},{"find_returns",finds},{"find_closed",findClosed},
            {"current_find_slots",currentFind==new IntPtr(-1)?0:1},{"retained_find_owners",currentFind==new IntPtr(-1)?0:1},
            {"read_bytes",readBytes},{"read_calls",reads},{"move_calls",moves},{"link_calls",links},{"set_calls",sets},{"get_calls",gets},
            {"cleanup_unknown",unknown},{"partial_native_return_unknown",partial},{"failure_operation",operation},
            {"failure_type",failureType},{"failure_hresult",failureHr},{"failure_native_error",nativeError},
            {"cleanup_operation",cleanupOperation},{"cleanup_type",cleanupType},{"cleanup_hresult",cleanupHr},{"cleanup_native_error",cleanupNativeError}
        };
    }
    public void Dispose(){
        if(disposed)return;disposed=true;disposeCalls++;
        if(currentFind!=new IntPtr(-1)&&!findAttempted){findAttempted=true;bool ok=FindClose(currentFind);int error=ok?0:Marshal.GetLastWin32Error();if(ok){findClosed++;currentFind=new IntPtr(-1);}else {unknown=true;if(cleanupOperation==""){cleanupOperation="find_close";cleanupNativeError=error;}}}
        foreach(Slot s in slots)Close(s);Array.Clear(six,0,six.Length);
    }
}

'@
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
function Close-DirectoryOwner($Owner) {
    if($Owner.CloseAttempted){return [bool]$Owner.Closed}
    if(-not $Owner.Returned -or $null -eq $Owner.Handle){
        $script:controllerCleanupUnknown=$true
        return $false
    }
    $Owner.CloseAttempted=$true;$script:directoryCloseAttempts++
    try {
        $Owner.Handle.Dispose()
        if($Owner.Handle.IsClosed){$Owner.Closed=$true;$script:directoryClosed++;return $true}
    }catch{if(-not $script:directoryCleanupFirstCause){$script:directoryCleanupFirstCause=Cause $_.Exception}}
    $script:controllerCleanupUnknown=$true
    return $false
}
function Retain-Directory([string]$Path,$List,[int]$Cap) {
    Check;Need ($List.Count -lt $Cap -and $directoryOwners.Count -lt 48 -and $null -eq $directoryCurrent) 'fixture_setup'
    $owner=[pscustomobject]@{Handle=$null;Returned=$false;CloseAttempted=$false;Closed=$false;Retained=$false}
    $script:directoryCurrent=$owner
    try{
        # The current owner box is reachable before the factory/getter/list insertion.
        $owner.Handle=[S53RootNativeR38]::DirectoryHandle($Path)
        $owner.Returned=$true;$script:directoryReturned++
        Need ($null -ne $owner.Handle -and -not $owner.Handle.IsInvalid -and -not $owner.Handle.IsClosed) 'fixture_setup'
        $directoryOwners.Add($owner.Handle,$owner);$List.Add($owner.Handle)
        $owner.Retained=$true;$script:directoryCurrent=$null
    }finally{
        if($null -ne $script:directoryCurrent){
            if($owner.Returned -and $null -ne $owner.Handle){
                $closed=Close-DirectoryOwner $owner
                if($closed){$script:directoryCurrent=$null}
            }
            if($null -ne $script:directoryCurrent){$script:controllerCleanupUnknown=$true;$script:unreleasedCurrentDirectories=1}
        }
    }
}
function Close-Directories($List) {
    $ok=$true
    for($i=$List.Count-1;$i -ge 0;$i--){
        $h=$List[$i]
        if(-not $directoryOwners.ContainsKey($h)){$script:controllerCleanupUnknown=$true;$ok=$false;continue}
        $owner=$directoryOwners[$h]
        # Previously attempted uncertain closes remain reachable and are never retried.
        $closed=Close-DirectoryOwner $owner
        if($closed){$List.RemoveAt($i)}else{$ok=$false}
    }
    if(-not $ok){$script:controllerCleanupUnknown=$true}
    return $ok
}
function New-Family {
    $chain=[Collections.Generic.Stack[string]]::new();$at=[IO.Path]::GetDirectoryName($syntheticParent)
    while($at){Need ($chain.Count -lt 15) 'fixture_setup';$chain.Push($at);$at=[IO.Path]::GetDirectoryName($at)}
    foreach($directory in $chain){Retain-Directory $directory $familyHeld 16}
    Need (-not [IO.File]::Exists($syntheticParent) -and -not [IO.Directory]::Exists($syntheticParent)) 'fixture_collision'
    New-S53RootDirectory $syntheticParent {$script:syntheticParentCreated=$true}
    Retain-Directory $syntheticParent $familyHeld 16
    Assert-FixtureAcl $syntheticParent $true
    $familyStreams.Verify($syntheticParent,$familyHeld[$familyHeld.Count-1],$true,0)
    Need (-not $familyStreams.CleanupUnknown -and $familyStreams.RemainingHandles -eq 0) 'case_cleanup'
}
function Actual-Files($Spec) {
    $files=[Collections.Generic.List[object]]::new()
    foreach($name in @('one.state','two.state')){
        $files.Add([pscustomobject]@{Path=('origin/'+$name+'/.kilogram-state.lock');Bytes=0;Hash=$emptyHash;Hex='';Control=$false})
    }
    foreach($area in @('state','backup')){
        foreach($row in (Payloads)){
            $files.Add([pscustomobject]@{Path=($area+'/'+$row.Path);Bytes=$row.Bytes;Hash=$row.Hash;Hex=$row.Hex;Control=$false})
        }
    }
    $files.Add([pscustomobject]@{Path='backup/preservation-inventory.json';Bytes=$Spec.InventoryBytes;Hash=$Spec.InventoryHash;Hex=$null;Control=$true})
    Need ($files.Count -eq 13 -and $Spec.FileCount -eq 13) 'fixture_setup'
    return $files.ToArray()
}
function New-TrackedEnumerator {
    Need ($caseEnumerators.Count -lt 32) 'premise_failure'
    $owner=[pscustomobject]@{Enumerator=$null;Returned=$false;CloseAttempted=$false;Disposed=$false}
    $caseEnumerators.Add($owner);$caseState.namespace_enumerator_results++
    return $owner
}
function Close-TrackedEnumerator($Owner) {
    if($Owner.CloseAttempted){return}
    if(-not $Owner.Returned -or $null -eq $Owner.Enumerator){
        $caseState.partial_namespace_return_unknown=$true;$script:controllerCleanupUnknown=$true;return
    }
    $Owner.CloseAttempted=$true;$caseState.namespace_enumerator_close_attempts++
    try{
        $Owner.Enumerator.Dispose();$Owner.Disposed=$true;$caseState.namespace_enumerators_disposed++
    }catch{
        $script:controllerCleanupUnknown=$true
        if(-not $script:caseFileCleanupFirstCause){$script:caseFileCleanupFirstCause=Cause $_.Exception}
    }
}
function New-TrackedFile([string]$Kind) {
    Need ($Kind -cin @('create','read') -and $caseFileOwners.Count -lt 48) 'fixture_setup'
    $owner=[pscustomobject]@{Kind=$Kind;Stream=$null;Safe=$null;Returned=$false;CloseAttempted=$false;Closed=$false}
    $caseFileOwners.Add($owner);$caseState.tracked_file_results++
    return $owner
}
function Close-TrackedFile($Owner) {
    if($Owner.CloseAttempted){return}
    if(-not $Owner.Returned -or $null -eq $Owner.Stream){
        $caseState.partial_file_return_unknown=$true;$script:controllerCleanupUnknown=$true
        return
    }
    $Owner.CloseAttempted=$true;$caseState.tracked_file_close_attempts++
    try{
        $Owner.Stream.Dispose()
        if($null -ne $Owner.Safe -and $Owner.Safe.IsClosed){
            $Owner.Closed=$true;$caseState.tracked_file_results_closed++
            if($Owner.Kind -ceq 'create'){$caseState.file_streams_open--;$caseState.file_streams_closed++}
            else{$caseState.read_streams_open--;$caseState.read_streams_closed++}
        }else{$script:controllerCleanupUnknown=$true}
    }catch{
        $script:controllerCleanupUnknown=$true
        if(-not $script:caseFileCleanupFirstCause){$script:caseFileCleanupFirstCause=Cause $_.Exception}
    }
}
function Write-Fixture([string]$Path,[AllowEmptyCollection()][byte[]]$Raw,[string]$Hash) {
    Check;$caseState.file_create_attempts++;$owner=New-TrackedFile 'create'
    try{
        $owner.Stream=[S53RootNativeR38]::NewFile($Path)
        $owner.Returned=$true;$caseState.tracked_file_returns++;$caseState.file_create_returned++;$caseState.file_streams_open++
        $owner.Safe=$owner.Stream.SafeFileHandle
        Assert-FixtureAcl $Path $false
        $owner.Stream.Write($Raw,0,$Raw.Length);$owner.Stream.Flush($true);Check
        Need ($owner.Stream.Length -eq $Raw.Length) 'fixture_hash'
        $owner.Stream.Position=0;$got=[byte[]]::new($Raw.Length);$offset=0
        while($offset -lt $got.Length){Check;$n=$owner.Stream.Read($got,$offset,$got.Length-$offset);Need ($n -gt 0) 'fixture_hash';$offset+=$n}
        Need ($owner.Stream.ReadByte() -eq -1 -and (Get-S53RootDigest $got) -ceq $Hash) 'fixture_hash'
        $caseState.files_validated++
    }finally{
        if(-not $owner.Returned){$caseState.partial_file_creation_unknown=$true}
        Close-TrackedFile $owner
    }
}
function New-CaseFixture($Spec,[string]$Root,$Files) {
    Check;Need (-not [IO.File]::Exists($Root) -and -not [IO.Directory]::Exists($Root)) 'fixture_collision'
    New-S53RootDirectory $Root {$script:caseState.case_created=$true;$script:caseState.directories_created++}
    Retain-Directory $Root $caseHeld 32
    $caseHandleMap.Add($Root,$caseHeld[$caseHeld.Count-1])
    Assert-FixtureAcl $Root $true
    foreach($relative in $caseDirectories){
        Check;$path=$Root+'\'+$relative.Replace('/','\')
        New-S53RootDirectory $path {$script:caseState.directories_created++}
        Retain-Directory $path $caseHeld 32
        $caseHandleMap.Add($path,$caseHeld[$caseHeld.Count-1])
        Assert-FixtureAcl $path $true
    }
    foreach($row in $Files){
        [byte[]]$raw=$null
        if($row.Control){$raw=[Text.Encoding]::UTF8.GetBytes((Inventory-Literal $Spec.Name)+[char]10)}
        else{$raw=[Convert]::FromHexString($row.Hex)}
        Need ($raw.Length -eq $row.Bytes -and (Get-S53RootDigest $raw) -ceq $row.Hash) 'fixture_hash'
        Write-Fixture ($Root+'\'+$row.Path.Replace('/','\')) $raw $row.Hash
    }
    Need (-not $controllerCleanupUnknown -and $caseState.case_created -and $caseState.directories_created -eq 15 -and $caseHeld.Count -eq 15 -and
        $caseState.file_create_attempts -eq $Spec.FileCount -and $caseState.file_create_returned -eq $Spec.FileCount -and
        $caseState.files_validated -eq $Spec.FileCount -and $caseState.file_streams_closed -eq $Spec.FileCount -and
        $caseState.file_streams_open -eq 0 -and -not $caseState.partial_file_creation_unknown) 'fixture_setup'
}
function Verify-HealthyInventory([byte[]]$Raw,[string]$Root,$Spec) {
    Need ($Raw.Length -eq $Spec.InventoryBytes -and $Raw.Length -le 4096 -and
        $Raw[-1] -eq 10 -and $Raw[-2] -eq 125 -and (Get-S53RootDigest $Raw) -ceq $Spec.InventoryHash) 'premise_failure'
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($Raw)
    Need ($text[0] -ne [char]0xfeff) 'premise_failure'
    $doc=$text|ConvertFrom-Json -AsHashtable -Depth 8 -ErrorAction Stop
    $fields=@('source','preserved','classification','held_lock_paths','os_lock_rule','source_files','source_after_files','destination_files')
    Need ($doc -is [Collections.IDictionary] -and $doc.Count -eq 8) 'premise_failure'
    foreach($field in $doc.Keys){Need ($field -cin $fields) 'premise_failure'}
    Need ($doc.source -ceq ($Root+'\state') -and $doc.preserved -ceq ($Root+'\backup') -and
        $doc.classification -ceq 'private-stopped-copy-never-opened' -and $doc.held_lock_paths.Count -eq 2 -and
        $doc.held_lock_paths[0] -ceq ($Root+'\state\one\state\.kilogram-state.lock') -and
        $doc.held_lock_paths[1] -ceq ($Root+'\state\two\state\.kilogram-state.lock') -and
        $doc.os_lock_rule -ceq 'Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.') 'premise_failure'
    $payloads=@(Payloads)
    foreach($name in @('source_files','source_after_files','destination_files')){
        $rows=$doc[$name];Need ($rows.Count -eq 5) 'premise_failure';$total=0L
        for($i=0;$i -lt 5;$i++){
            $row=$rows[$i];$expected=$payloads[$i]
            Need ($row -is [Collections.IDictionary] -and $row.Count -eq 4) 'premise_failure'
            foreach($field in $row.Keys){Need ($field -cin @('path','bytes','sha256','digest_provenance')) 'premise_failure'}
            $provenance=if($name -cne 'destination_files' -and $expected.Path.EndsWith('/.kilogram-state.lock',[StringComparison]::Ordinal)){
                'held-lock length checked; empty-byte digest derived'
            }else{'actual file byte read'}
            Need (($row.bytes -is [int] -or $row.bytes -is [long]) -and $row.path -ceq $expected.Path -and
                $row.bytes -eq $expected.Bytes -and $row.sha256 -ceq $expected.Hash -and
                $row.digest_provenance -ceq $provenance) 'premise_failure'
            $total+=$row.bytes
        }
        Need ($total -eq 52) 'premise_failure'
    }
    return $true
}

function Verify-Premises($Spec,[string]$Root,$Files,$Streams,[bool]$Prepared,$Alias) {
    $expected=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($d in $caseDirectories){$expected.Add($d,[pscustomobject]@{Directory=$true;Row=$null;Special=($Prepared -and $Spec.Directory -and $d -ceq $Spec.TargetRelative)})}
    foreach($row in $Files){
        $physical=$row.Path
        if($Prepared -and $Spec.Directory -and $physical.StartsWith('state/one/state/',[StringComparison]::Ordinal)){
            $physical='alias-target/'+$physical.Substring('state/one/state/'.Length)
        }
        $expected.Add($physical,[pscustomobject]@{Directory=$false;Row=$row;Special=($Prepared -and -not $Spec.Directory -and $physical -ceq $Spec.TargetRelative)})
    }
    if($Prepared){
        Need ($null -ne $Alias) 'premise_failure';$Alias.VerifyPreparedTarget()
        if($Spec.Directory){
            $expected.Add('alias-target',[pscustomobject]@{Directory=$true;Row=$null;Special=$false})
        }else{
            $expected.Add('alias-link',[pscustomobject]@{Directory=$false;Special=$true;Row=[pscustomobject]@{Path='alias-link';Bytes=6;Hash='ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4';Control=$false}})
        }
    }
    Need ($expected.Count -eq $(if($Prepared){28}else{27})) 'premise_failure'
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $queue=[string[]]::new(16);$queue[0]='';$head=0;$tail=1;$count=0;$filesRead=0;$locks=0;$special=0
    $sFiles=0;$sBytes=0L;$bFiles=0;$bBytes=0L;$healthyInventory=$false;$physicalBytes=0L
    while($head -lt $tail){
        Check;$parent=$queue[$head++];$directory=if($parent -ceq ''){$Root}else{$Root+'\'+$parent.Replace('/','\')}
        Assert-FixtureAcl $directory $true
        $Streams.Verify($directory,$caseHandleMap[$directory],$true,0)
        $en=New-TrackedEnumerator
        try{
            $en.Enumerator=[IO.DirectoryInfo]::new($directory).EnumerateFileSystemInfos().GetEnumerator()
            $en.Returned=$true;$caseState.namespace_enumerator_returns++
            while($true){
                Check;if(-not $en.Enumerator.MoveNext()){break};Check;$item=$en.Enumerator.Current;$count++;Need ($count -le 28) 'premise_failure'
                $relative=if($parent -ceq ''){$item.Name}else{$parent+'/'+$item.Name}
                Need ($seen.Add($relative) -and $expected.ContainsKey($relative)) 'premise_failure'
                $entry=$expected[$relative];$isDir=($item.Attributes -band [IO.FileAttributes]::Directory) -ne 0
                $isReparse=($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0
                Need ($isDir -eq $entry.Directory -and ($item.Attributes -band [IO.FileAttributes]::Device) -eq 0) 'premise_failure'
                if($entry.Special -and $Spec.Directory){
                    Need ($isReparse -and $isDir -and $relative -ceq 'state/one/state') 'premise_failure'
                    $special++;continue # NEVER descend the junction.
                }
                Need (-not $isReparse) 'premise_failure'
                if($isDir){Need ($tail -lt 16) 'premise_failure';$queue[$tail++]=$relative;continue}
                $row=$entry.Row
                if($entry.Special){
                    Need ($Prepared -and -not $Spec.Directory -and $relative -cin @('state/one/state/payload.bin','alias-link') -and $row.Bytes -eq 6) 'premise_failure'
                    $special++;$filesRead++;$physicalBytes+=6
                    if($relative -ceq $Spec.TargetRelative){$sFiles++;$sBytes+=6}
                    continue # Both fixed names/IDs/links/default bytes/streams were checked by Alias.
                }
                $path=$Root+'\'+$relative.Replace('/','\');$owner=New-TrackedFile 'read'
                try{
                    $owner.Stream=[S53RootNativeR38]::Read($path,$false)
                    $owner.Returned=$true;$caseState.tracked_file_returns++;$caseState.read_streams_open++
                    $owner.Safe=$owner.Stream.SafeFileHandle;$stamp=[S53RootNativeR38]::Stamp($owner.Stream,$path)
                    $parts=$stamp.Split(':');Need ($parts.Count -eq 10 -and $ids.Add(($parts[0..2] -join ':'))) 'premise_failure'
                    Assert-FixtureAcl $path $false
                    Need ($owner.Stream.Length -eq $row.Bytes -and $row.Bytes -le 4096) 'premise_failure'
                    $Streams.Verify($path,$owner.Safe,$false,$row.Bytes)
                    [byte[]]$raw=Bytes $owner.Stream 4096
                    Need ($raw.Length -eq $row.Bytes -and (Get-S53RootDigest $raw) -ceq $row.Hash -and [S53RootNativeR38]::Stamp($owner.Stream,$path) -ceq $stamp) 'premise_failure'
                    if($row.Path.EndsWith('/.kilogram-state.lock',[StringComparison]::Ordinal)){Need ($row.Bytes -eq 0) 'premise_failure';$locks++}
                    if($row.Control){$healthyInventory=Verify-HealthyInventory $raw $Root $Spec}
                    if($row.Path.StartsWith('state/',[StringComparison]::Ordinal)){$sFiles++;$sBytes+=$raw.Length}
                    elseif($row.Path.StartsWith('backup/',[StringComparison]::Ordinal) -and -not $row.Control){$bFiles++;$bBytes+=$raw.Length}
                    $filesRead++;$physicalBytes+=$raw.Length
                }finally{Close-TrackedFile $owner}
            }
        }finally{Close-TrackedEnumerator $en}
    }
    $extra=[int]($Prepared -and -not $Spec.Directory);$specialWant=if(-not $Prepared){0}elseif($Spec.Directory){1}else{2}
    $strictFiles=if($Prepared -and -not $Spec.Directory){12}else{13}
    $strict=Stream-Facts $Streams;Assert-StreamSnapshot $strict 15 $strictFiles
    $wantedReads=if(-not $Prepared){13}elseif($Spec.Directory){26}else{25}
    Need ($seen.Count -eq $expected.Count -and $count -eq $expected.Count -and $tail -eq 15 -and $special -eq $specialWant -and
        $filesRead -eq 13+$extra -and $locks -eq 6 -and $healthyInventory -and $caseState.read_streams_closed -eq $wantedReads -and
        $caseState.read_streams_open -eq 0 -and $sFiles -eq 5 -and $sBytes -eq 52 -and $bFiles -eq 5 -and $bBytes -eq 52 -and
        -not $controllerCleanupUnknown) 'premise_failure'
    return [ordered]@{ready=$true;healthy_physical_layout_verified=(-not $Prepared);bounded_entries=$count
        directories=(15+[int]($Prepared -and $Spec.Directory));files=$filesRead;physical_file_bytes=$physicalBytes
        s_payload_files=5;s_payload_bytes=52;bkp_payload_files=5;bkp_payload_bytes=52;empty_closed_locks=6
        target_object_verified=$true;canonical_names=$true;protected_exact_acl=$true;normal_objects_strict=$true
        streams_verified=($Streams.ChecksCompleted+($special*[int](-not $Spec.Directory)));strict_streams_verified=$Streams.ChecksCompleted;deliberate_targets_verified=$special
        deliberate_alias_verified=$Prepared;alias_objects=[int]$Prepared;named_stream_bytes=0;streams_unsupported_accepted=$false;setup_writers_closed=$true
        expected_files_spec=5;expected_bytes_spec=52;healthy_dummy_inventory_premises_verified=$healthyInventory
        core_inventory_not_executed_during_premises=$true;inventory_bytes=$Spec.InventoryBytes;inventory_sha256=$Spec.InventoryHash}
}


function Core-Error($Exception,$Spec) {
    $chain=[Collections.Generic.List[object]]::new();$at=$Exception
    while($null -ne $at){
        Need ($chain.Count -lt 16) 'typed_refusal'
        foreach($key in @('c2g_code','c2g_fixture_code','c2g_premise_code','rootcopies_code','colda_native_error')){
            Need (-not $at.Data.Contains($key)) 'typed_refusal'
        }
        Need ($at -isnot [ComponentModel.Win32Exception]) 'typed_refusal'
        $chain.Add([ordered]@{type=$at.GetType().FullName;hresult=$at.HResult;native_error=0})
        $at=$at.InnerException
    }
    Need ($chain.Count -eq 3 -and $Exception.GetType() -eq [Management.Automation.MethodInvocationException] -and $Exception.HResult -eq -2146233087) 'typed_refusal'
    $normal=$Exception.InnerException
    Need ($normal.GetType() -eq [InvalidOperationException] -and $normal.HResult -eq -2146233079 -and
        [string]$normal.Data['colda_code'] -ceq $Spec.FailureCode -and [string]$normal.Data['colda_phase'] -ceq $Spec.FailurePhase -and
        [string]$normal.Data['colda_role'] -ceq $Spec.FailureRole) 'typed_refusal'
    $original=$normal.InnerException
    Need ($original.GetType() -eq [InvalidOperationException] -and $original.HResult -eq -2146233079 -and
        [string]$original.Data['colda_code'] -ceq $Spec.FailureCode -and $null -eq $original.InnerException -and
        -not $original.Data.Contains('colda_phase') -and -not $original.Data.Contains('colda_role')) 'typed_refusal'
    return [ordered]@{matched=$true;code=$Spec.FailureCode;phase=$Spec.FailurePhase;role=$Spec.FailureRole;native_error=0;cause_kind='bounded_refusal'
        normalized_type=$normal.GetType().FullName;normalized_hresult=$normal.HResult;normalized_marks=@{colda_code=$Spec.FailureCode;colda_phase=$Spec.FailurePhase;colda_role=$Spec.FailureRole}
        original_marked_exception=@{type=$original.GetType().FullName;hresult=$original.HResult;marks=@{colda_code=$Spec.FailureCode};inner_type=$null;inner_hresult=0;inner_native_error=0;inner_has_deeper_cause=$false}
        deepest_type=$original.GetType().FullName;error_hresult=$original.HResult;native_marks_absent=$true;chain=$chain.ToArray()}
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
    $hist=if($s.Directory){$null}else{$s.H}
    $expected=[ordered]@{
        property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$false;expected_set_captured_before_hash=$false;observed_entry_sets_equal=$false
        input_handles_released=(-not $Pre);cleanup_unknown=$false;receipts_completed=$false;cleanup_native_error=0
        failure_code=if($Pre){$null}else{$s.FailureCode};failure_phase=if($Pre){$null}else{$s.FailurePhase}
        failure_role=if($Pre){$null}else{$s.FailureRole};cause_kind=if($Pre){$null}else{'bounded_refusal'}
        native_error=0;error_hresult=if($Pre){0}else{-2146233079};expected_files=5;expected_bytes=52
        locks=if($Pre){$s.PreLocks}else{$s.CoreLocks};hash_passes=0;hash_bytes=0;capture_complete_timestamp=0
        historical_tuple_digest=$hist;s_before_tuple_digest=$null;bkp_before_tuple_digest=$null;s_after_tuple_digest=$null;bkp_after_tuple_digest=$null
        peak_owned_handles=if($Pre){$s.PrePeak}else{$s.PeakHandles};remaining_owned_handles=if($Pre){$s.PreOwned}else{0}
        peak_path_bytes=if($Pre){$s.PrePath}else{$s.PeakPath};peak_record_bytes=if($Pre){$s.PreRecord}else{$s.PeakRecord}
        peak_inventory_bytes=if($Pre){$s.PreInventoryPeak}else{$s.InventoryPeak};scratch_reserved_bytes=524288
        receipt_bytes_charged=0;receipt_bytes_known_written=0;receipt_write_unknown=$false
    }
    Assert-Fields $o $expected $false
    Need (@(Object-Keys $o).Count -eq ($expected.Count+2))
    $ph=Get-Value $o 'phase_seconds';$receipts=Get-Value $o 'private_receipt_digests'
    Need (@(Object-Keys $receipts).Count -eq 0)
    $keys=@(if($Pre){@($s.PreKeys)}else{@('allocation')+@($s.Phases)})
    $actualKeys=@(Object-Keys $ph);Need ($actualKeys.Count -eq $keys.Count -and ($actualKeys -join '|') -ceq ($keys -join '|'))
    foreach($key in $keys){
        $value=Get-Value $ph $key
        Need ($value -is [double] -or $value -is [float] -or $value -is [decimal] -or $value -is [int] -or $value -is [long])
        Need ([double]::IsFinite([double]$value) -and $value -ge 0 -and $value -lt 20)
    }
}
function Assert-StreamSnapshot($v,[int]$Directories,[int]$Files) {
    $count=$Directories+$Files
    $expected=[ordered]@{first_calls=$count;next_calls=$Files;opened=$Files;closed=$Files;remaining=0;peak=if($Files -gt 0){1}else{0};
        checks=$count;directories=$Directories;files=$Files;failure_code=$null;native_error=0;cleanup_unknown=$false;cleanup_native_error=0}
    Assert-Fields $v $expected $true
}
function Assert-PremiseSnapshot($p,$s,[bool]$Prepared) {
    $extra=[int]($Prepared -and -not $s.Directory)
    $special=if(-not $Prepared){0}elseif($s.Directory){1}else{2}
    $expected=[ordered]@{ready=$true;healthy_physical_layout_verified=(-not $Prepared);bounded_entries=(27+[int]$Prepared)
        directories=(15+[int]($Prepared -and $s.Directory));files=(13+$extra);physical_file_bytes=(104+$s.InventoryBytes+6*$extra)
        s_payload_files=5;s_payload_bytes=52;bkp_payload_files=5;bkp_payload_bytes=52;empty_closed_locks=6
        target_object_verified=$true;canonical_names=$true;protected_exact_acl=$true;normal_objects_strict=$true
        streams_verified=(28+[int]($Prepared -and -not $s.Directory));strict_streams_verified=(28-[int]($Prepared -and -not $s.Directory));deliberate_targets_verified=$special
        deliberate_alias_verified=$Prepared;alias_objects=[int]$Prepared;named_stream_bytes=0;streams_unsupported_accepted=$false;setup_writers_closed=$true
        expected_files_spec=5;expected_bytes_spec=52;healthy_dummy_inventory_premises_verified=$true;core_inventory_not_executed_during_premises=$true
        inventory_bytes=$s.InventoryBytes;inventory_sha256=$s.InventoryHash}
    Assert-Fields $p $expected $true
}
function Assert-SetupSnapshot($a,$s) {
    $reads=if($s.Directory){26}else{25};$files=13+$reads
    $expected=[ordered]@{case_created=$true;directories_created=(15+[int]$s.Directory);file_create_attempts=13;file_create_returned=13;files_validated=13
        file_streams_open=0;file_streams_closed=13;read_streams_open=0;read_streams_closed=$reads;partial_file_creation_unknown=$false
        tracked_file_results=$files;tracked_file_returns=$files;tracked_file_close_attempts=$files;tracked_file_results_closed=$files
        tracked_file_results_remaining=0;partial_file_return_unknown=$false
        namespace_enumerator_results=30;namespace_enumerator_returns=30;namespace_enumerator_close_attempts=30;namespace_enumerators_disposed=30
        namespace_enumerators_remaining=0;partial_namespace_return_unknown=$false}
    Assert-Fields $a $expected $true
}
function Assert-GuardHandoff($o,$s) {
    $n=15+[int]$s.Directory
    Assert-Fields $o ([ordered]@{returned=$n;close_attempts=$n;closed=$n;remaining=0;current_slots=0;cleanup_unknown=$false;before_factory=$true}) $true
}
function Validate-CoreError($t,$cause,$s) {
    Assert-Fields $t ([ordered]@{matched=$true;code=$s.FailureCode;phase=$s.FailurePhase;role=$s.FailureRole;native_error=0;cause_kind='bounded_refusal';
        normalized_type='System.InvalidOperationException';normalized_hresult=-2146233079;deepest_type='System.InvalidOperationException';error_hresult=-2146233079;
        native_marks_absent=$true}) $false
    $n=Get-Value $t 'normalized_marks'
    Assert-Fields $n ([ordered]@{colda_code=$s.FailureCode;colda_phase=$s.FailurePhase;colda_role=$s.FailureRole}) $true
    $original=Get-Value $t 'original_marked_exception'
    Assert-Fields $original ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;inner_type=$null;inner_hresult=0;inner_native_error=0;inner_has_deeper_cause=$false}) $false
    Assert-Fields (Get-Value $original 'marks') ([ordered]@{colda_code=$s.FailureCode}) $true
    $chain=@(Get-Value $t 'chain');Need ($chain.Count -eq 3)
    Assert-Fields $chain[0] ([ordered]@{type='System.Management.Automation.MethodInvocationException';hresult=-2146233087;native_error=0}) $true
    foreach($node in $chain[1..2]){Assert-Fields $node ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0}) $true}
    Assert-Fields $cause ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0}) $false
    Assert-Fields (Get-Value $cause 'marks') ([ordered]@{colda_code=$s.FailureCode;colda_phase=$s.FailurePhase;colda_role=$s.FailureRole}) $true
}

function Assert-AliasSnapshot($o,$s) {
    $n=if($s.Directory){0}else{4}
    $expected=[ordered]@{prepare_calls=1;verify_calls=1;dispose_calls=1;prepared=$true;verified=$true;disposed=$true;created=$true;moved=$s.Directory
        file_open_calls=4;file_returns=4;file_closed=4;close_attempts=4;current_file_slots=0;retained_file_owners=0;peak_file_results=2
        find_returns=$n;find_closed=$n;current_find_slots=0;retained_find_owners=0
        read_bytes=if($s.Directory){0}else{24};read_calls=if($s.Directory){0}else{8}
        move_calls=[int]$s.Directory;link_calls=[int](-not $s.Directory);set_calls=[int]$s.Directory;get_calls=if($s.Directory){2}else{0}
        cleanup_unknown=$false;partial_native_return_unknown=$false;failure_operation='';failure_type='';failure_hresult=0;failure_native_error=0
        cleanup_operation='';cleanup_type='';cleanup_hresult=0;cleanup_native_error=0}
    Assert-Fields $o $expected $true
}
function Assert-CaseRecord($r,$s,[long]$Deadline) {
    Assert-Fields $r ([ordered]@{name=$s.Name;selector=$s.Selector;result='observed';failure=$null;failure_cause=$null;failure_stage=$null;cleanup_first_cause=$null
        alias_prepare_returned=$true;alias_dispose_attempts=1;alias_disposed=$true
        core_factory_invoked=$true;core_capture_invoked=$true;core_dispose_attempted=$true;core_disposed=$true
        core_exception_observed=$true;core_success_returned=$false;pre_hook_seen=$true;pre_hook_acquired_events=0;hook_failure=$null
        controller_directories_closed=$true;remaining_controller_directories=0;unreleased_current_directories=0;controller_cleanup_unknown=$false
        acquired_files=$s.Acquired;capture_qpc=0;output_owner_bound=$false;proof_created=$false;receipt_count=0}) $false
    Assert-SetupSnapshot $r.setup $s
    Assert-Fields $r.premises ([ordered]@{ready=$true;directories=(15+[int]$s.Directory);files=(13+[int](-not $s.Directory));physical_file_bytes=(104+$s.InventoryBytes+6*[int](-not $s.Directory));named_stream_bytes=0;streams_verified=(57-[int]$s.Directory)}) $false
    Assert-PremiseSnapshot $r.premises.baseline $s $false
    Assert-PremiseSnapshot $r.premises.prepared $s $true
    Assert-StreamSnapshot $r.baseline_stream_check 15 13
    $postDirs=15;$postFiles=if($s.Directory){13}else{12}
    Assert-StreamSnapshot $r.prepared_strict_stream_check $postDirs $postFiles
    Assert-AliasSnapshot $r.alias_helper $s
    Assert-GuardHandoff $r.guard_handoff $s
    Assert-CoreSnapshot $r.pre_hook_core_observation $s $true
    Assert-CoreSnapshot $r.core_observation $s $false
    Validate-CoreError $r.core_refusal $r.primary_core_cause $s
    $times=@($r.setup_started_qpc,$r.baseline_complete_qpc,$r.alias_prepare_before_qpc,$r.alias_prepare_after_qpc,
        $r.prepared_complete_qpc,$r.alias_disposed_qpc,$r.guards_closed_qpc,$r.factory_before_qpc,$r.factory_after_qpc)
    $last=$StartTimestamp
    foreach($time in $times){
        Need ($time -is [long] -or $time -is [int])
        Need ($time -ge $last -and $time -gt $StartTimestamp -and $time -lt $Deadline);$last=$time
    }
    Need ($r.hook_events.Count -eq $s.Phases.Count)
    $selected=0L
    for($j=0;$j -lt $s.Phases.Count;$j++){
        $event=$r.hook_events[$j]
        Need (@(Object-Keys $event).Count -eq 2)
        Need ($event.name -ceq $s.Phases[$j] -and ($event.qpc -is [long] -or $event.qpc -is [int]) -and
            $event.qpc -ge $last -and $event.qpc -lt $Deadline)
        $last=$event.qpc;if($event.name -ceq $s.PreHook){$selected=$event.qpc}
    }
    Need ($selected -gt 0 -and ($r.pre_hook_qpc -is [long] -or $r.pre_hook_qpc -is [int]) -and
        $r.pre_hook_qpc -ge $selected -and $r.pre_hook_qpc -lt $Deadline -and
        $r.capture_qpc -eq $r.core_observation.capture_complete_timestamp -and
        $r.capture_qpc -eq $r.pre_hook_core_observation.capture_complete_timestamp)
}



function Stream-Facts($Streams) {
    if($null -eq $Streams){return $null}
    return [ordered]@{first_calls=$Streams.FirstCalls;next_calls=$Streams.NextCalls;opened=$Streams.OpenedHandles;
        closed=$Streams.ClosedHandles;remaining=$Streams.RemainingHandles;peak=$Streams.PeakHandles;checks=$Streams.ChecksCompleted;
        directories=$Streams.DirectoriesVerified;files=$Streams.FilesVerified;failure_code=$Streams.FailureCode;native_error=$Streams.NativeError;
        cleanup_unknown=$Streams.CleanupUnknown;cleanup_native_error=$Streams.CleanupNativeError}
}


function Retain-UnknownOwner($Owner) {
    if($null -eq $Owner){return}
    foreach($known in $retainedUnknownOwners){if([object]::ReferenceEquals($known,$Owner)){return}}
    if($retainedUnknownOwners.Count -lt 4){$retainedUnknownOwners.Add($Owner)}
    $script:controllerCleanupUnknown=$true
}
function Run-Case($Spec,$Record) {
    $script:caseState=[ordered]@{case_created=$false;directories_created=0;file_create_attempts=0;file_create_returned=0
        files_validated=0;file_streams_open=0;file_streams_closed=0;read_streams_open=0;read_streams_closed=0;partial_file_creation_unknown=$false
        tracked_file_results=0;tracked_file_returns=0;tracked_file_close_attempts=0;tracked_file_results_closed=0;tracked_file_results_remaining=0;partial_file_return_unknown=$false
        namespace_enumerator_results=0;namespace_enumerator_returns=0;namespace_enumerator_close_attempts=0;namespace_enumerators_disposed=0
        namespace_enumerators_remaining=0;partial_namespace_return_unknown=$false}
    $script:caseFileOwners=[Collections.Generic.List[object]]::new();$script:caseEnumerators=[Collections.Generic.List[object]]::new()
    $script:caseFileCleanupFirstCause=$null
    $script:caseHeld=[Collections.Generic.List[Microsoft.Win32.SafeHandles.SafeFileHandle]]::new()
    $script:caseHandleMap=[Collections.Generic.Dictionary[string,Microsoft.Win32.SafeHandles.SafeFileHandle]]::new([StringComparer]::Ordinal)
    $script:caseHooks=[Collections.Generic.List[object]]::new();$script:caseHookIndex=0;$script:caseAcquired=0
    $script:preHookPublic=$null;$script:preHookAcquired=0;$script:preHookSeen=$false;$script:preHookQpc=0L;$script:hookFailure=$null
    $beforeGuards=$directoryReturned;$beforeAttempts=$directoryCloseAttempts;$beforeClosed=$directoryClosed
    $coreObject=$null;$coreDisposed=$false;$coreDisposeAttempted=$false;$preStreams=$null;$postStreams=$null
    $coreUnknown=$false;$aliasUnknown=$false
    $alias=$null;$aliasDisposeAttempted=$false;$aliasDisposeAttempts=0;$aliasDisposed=$false;$aliasSnapshot=$null;$aliasOk=$false
    $caseFailure=$null;$caseCause=$null;$primaryCoreCause=$null;$typed=$null;$corePublic=$null;$stage='setup';$recordComplete=$false
    $baseline=$null;$prepared=$null;$premises=$null;$guardHandoff=$null;$cleanupOk=$false;$cleanupFirstCause=$null
    $caughtCore=$null;$factoryBefore=0L;$factoryAfter=0L;$captureInvoked=$false;$coreReturned=$false;$aliasReturned=$false
    $setupStart=[Diagnostics.Stopwatch]::GetTimestamp();$baselineDone=0L;$prepareBefore=0L;$prepareAfter=0L;$preparedDone=0L;$aliasDone=0L;$guardsDone=0L
    try{
        Check;$Record.result='started';$root=$syntheticParent+'\'+$Spec.Name
        $preStreams=[S53C2GStreamsR01]::new($Spec.Name,[Action]{Check})
        $postStreams=[S53C2GStreamsR01]::new($Spec.Name,[Action]{Check})
        $files=@(Actual-Files $Spec);New-CaseFixture $Spec $root $files
        $stage='baseline_premises'
        $baseline=Verify-Premises $Spec $root $files $preStreams $false $null
        Assert-PremiseSnapshot $baseline $Spec $false;$baselineDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $stage='alias_prepare';$alias=[S53C2GAliasR01]::new($Spec.Name,[Action]{Check})
        $prepareBefore=[Diagnostics.Stopwatch]::GetTimestamp()
        $target=$root+'\'+$Spec.TargetRelative.Replace('/','\')
        $borrowed=if($Spec.Directory){$caseHandleMap[$target]}else{$null}
        $alias.CaptureBaseline($borrowed)
        if($Spec.Directory){
            Need ($directoryOwners.ContainsKey($borrowed) -and (Close-DirectoryOwner $directoryOwners[$borrowed])) 'case_cleanup'
            Need ($caseHeld.Remove($borrowed) -and $caseHandleMap.Remove($target)) 'case_cleanup'
        }
        $creator=[Action]{
            Check
            Need $Spec.Directory 'fixture_scope'
            New-S53RootDirectory $target {$caseState.directories_created++}
            Assert-FixtureAcl $target $true
        }
        $alias.Prepare($creator);$aliasReturned=$true;$prepareAfter=[Diagnostics.Stopwatch]::GetTimestamp()
        if($Spec.Directory){
            $moved=$root+'\alias-target';Retain-Directory $moved $caseHeld 32
            $caseHandleMap.Add($moved,$caseHeld[$caseHeld.Count-1])
        }
        $stage='prepared_premises'
        $prepared=Verify-Premises $Spec $root $files $postStreams $true $alias
        Assert-PremiseSnapshot $prepared $Spec $true;$preparedDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $premises=[ordered]@{ready=$true;directories=$prepared.directories;files=$prepared.files;physical_file_bytes=$prepared.physical_file_bytes
            named_stream_bytes=0;streams_verified=($baseline.streams_verified+$prepared.streams_verified);baseline=$baseline;prepared=$prepared}
        $stage='alias_dispose';$aliasDisposeAttempted=$true;$aliasDisposeAttempts++
        $alias.Dispose();$aliasDisposed=$true;$aliasDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $aliasSnapshot=$alias.Snapshot();Assert-AliasSnapshot $aliasSnapshot $Spec;$aliasOk=$true
        $caseState.tracked_file_results_remaining=@($caseFileOwners|Where-Object {-not $_.Closed}).Count
        $caseState.namespace_enumerators_remaining=@($caseEnumerators|Where-Object {-not $_.Disposed}).Count
        Assert-SetupSnapshot $caseState $Spec
        $stage='guard_handoff';$cleanupOk=Close-Directories $caseHeld
        Need ($cleanupOk -and $caseHeld.Count -eq 0 -and -not $controllerCleanupUnknown -and $null -eq $directoryCurrent) 'case_cleanup'
        $caseHandleMap.Clear();$guardsDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $guardHandoff=[ordered]@{returned=($directoryReturned-$beforeGuards);close_attempts=($directoryCloseAttempts-$beforeAttempts)
            closed=($directoryClosed-$beforeClosed);remaining=$caseHeld.Count;current_slots=if($null -eq $directoryCurrent){0}else{1}
            cleanup_unknown=$controllerCleanupUnknown;before_factory=$true}
        Assert-GuardHandoff $guardHandoff $Spec
        $stage='factory';Check;$factoryBefore=[Diagnostics.Stopwatch]::GetTimestamp()
        $coreObject=New-S53ColdAFixtureCore -Case $Spec.Selector -InventoryBytes $Spec.InventoryBytes -InventorySha256 $Spec.InventoryHash -ExpectedFiles 5 -ExpectedPayloadBytes 52 -WorkDeadlineTimestamp $absoluteWorkDeadline
        $factoryAfter=[Diagnostics.Stopwatch]::GetTimestamp()
        $hook=[Action[string]]{
            param($Name)
            try{
                Check
                if($Name -ceq 'expected_file_acquired'){$script:caseAcquired++;Need ($script:caseAcquired -le $Spec.Acquired);return}
                Need ($script:caseHookIndex -lt $Spec.Phases.Count -and $Name -ceq $Spec.Phases[$script:caseHookIndex])
                $script:caseHookIndex++;$script:caseHooks.Add([ordered]@{name=$Name;qpc=[Diagnostics.Stopwatch]::GetTimestamp()})
                if($Name -ceq $Spec.PreHook){
                    Need (-not $preHookSeen -and $caseHeld.Count -eq 0 -and $aliasDisposed -and $aliasOk -and
                        $caseState.file_streams_open -eq 0 -and $caseState.read_streams_open -eq 0 -and -not $controllerCleanupUnknown)
                    $script:preHookSeen=$true;$script:preHookAcquired=$caseAcquired
                    $script:preHookPublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
                    Assert-CoreSnapshot $preHookPublic $Spec $true
                    Need ($caseAcquired -eq 0)
                    $script:preHookQpc=[Diagnostics.Stopwatch]::GetTimestamp()
                }
            }catch{$script:hookFailure=Cause $_.Exception;throw}
        }
        $coreObject.SetFixtureHook($hook)
        $stage='capture';$captureInvoked=$true
        try{$coreObject.CaptureAndCompare();$coreReturned=$true}catch{$caughtCore=$_.Exception;$primaryCoreCause=Cause $_.Exception}
        $stage='core_result'
        Need (-not $coreReturned -and $null -ne $caughtCore -and $preHookSeen -and $null -eq $hookFailure -and
            $null -ne $preHookPublic -and $caseAcquired -eq $Spec.Acquired -and $caseHookIndex -eq $Spec.Phases.Count) 'typed_refusal'
        $typed=Core-Error $caughtCore $Spec
        Validate-CoreError $typed $primaryCoreCause $Spec
        $corePublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        Assert-CoreSnapshot $corePublic $Spec $false
        $stage='core_dispose';$coreDisposeAttempted=$true;$coreObject.Dispose();$coreDisposed=$true
        $corePublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        Assert-CoreSnapshot $corePublic $Spec $false
        Check;$recordComplete=$true
    }catch{$caseFailure=Failure-Code $_.Exception;$caseCause=Cause $_.Exception}
    finally{
        if($coreObject -and -not $coreDisposeAttempted){
            $coreDisposeAttempted=$true
            try{$coreObject.Dispose();$coreDisposed=$true}catch{
                $coreUnknown=$true;$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}
            }
        }
        if($coreObject){
            try{
                $facts=$coreObject.GetObservation()
                $corePublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
                if($facts.CleanupUnknown -or $facts.RemainingOwnedHandles -ne 0 -or -not $facts.InputHandlesReleased){$coreUnknown=$true;$script:controllerCleanupUnknown=$true}
            }catch{$coreUnknown=$true;$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
            if(-not $coreDisposed){$coreUnknown=$true}
            if($coreUnknown){Retain-UnknownOwner $coreObject}
        }
        if($alias -and -not $aliasDisposeAttempted){
            $aliasDisposeAttempted=$true;$aliasDisposeAttempts++
            try{$alias.Dispose();$aliasDisposed=$true}catch{
                $script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}
            }
        }
        if($alias){
            $aliasSnapshot=$null
            try{
                $aliasSnapshot=$alias.Snapshot()
                if($aliasSnapshot.cleanup_unknown -or $aliasSnapshot.partial_native_return_unknown -or
                    $aliasSnapshot.current_file_slots -ne 0 -or $aliasSnapshot.retained_file_owners -ne 0 -or
                    $aliasSnapshot.current_find_slots -ne 0 -or $aliasSnapshot.retained_find_owners -ne 0 -or
                    -not $aliasSnapshot.disposed){$aliasUnknown=$true;$script:controllerCleanupUnknown=$true}
                if($recordComplete){Assert-AliasSnapshot $aliasSnapshot $Spec;$aliasOk=$true}
            }catch{
                if($null -eq $aliasSnapshot){$aliasUnknown=$true;$script:controllerCleanupUnknown=$true}
                if(-not $caseFailure){$caseFailure='assertion';$caseCause=Cause $_.Exception;$stage='alias_assertions'}
            }
        }
        if($aliasUnknown){Retain-UnknownOwner $alias}
        foreach($owner in $caseFileOwners){if(-not $owner.CloseAttempted){Close-TrackedFile $owner}}
        foreach($owner in $caseEnumerators){if(-not $owner.CloseAttempted){Close-TrackedEnumerator $owner}}
        $caseState.tracked_file_results_remaining=@($caseFileOwners|Where-Object {-not $_.Closed}).Count
        $caseState.namespace_enumerators_remaining=@($caseEnumerators|Where-Object {-not $_.Disposed}).Count
        $cleanupOk=Close-Directories $caseHeld
        if($cleanupOk){$caseHandleMap.Clear()}
        if($caseState.file_streams_open -ne 0 -or $caseState.read_streams_open -ne 0 -or
            $caseState.tracked_file_results_remaining -ne 0 -or $caseState.namespace_enumerators_remaining -ne 0){$script:controllerCleanupUnknown=$true}
        foreach($streams in @($preStreams,$postStreams)){
            if($streams -and ($streams.CleanupUnknown -or $streams.RemainingHandles -ne 0)){Retain-UnknownOwner $streams}
        }
        if(-not $cleanupFirstCause){$cleanupFirstCause=$caseFileCleanupFirstCause}
        if(-not $cleanupFirstCause){$cleanupFirstCause=$directoryCleanupFirstCause}
        if((-not $cleanupOk -or $controllerCleanupUnknown) -and -not $caseFailure){$caseFailure='case_cleanup';$caseCause=$cleanupFirstCause;$stage='case_cleanup'}
        if(-not $recordComplete -and -not $caseFailure){$caseFailure='assertion';$stage='case_assertions'}
        $Record.setup=$caseState;$Record.premises=$premises
        $Record.baseline_stream_check=Stream-Facts $preStreams;$Record.prepared_strict_stream_check=Stream-Facts $postStreams
        $Record.alias_helper=$aliasSnapshot;$Record.alias_prepare_returned=$aliasReturned
        $Record.alias_dispose_attempts=$aliasDisposeAttempts;$Record.alias_disposed=$aliasDisposed;$Record.guard_handoff=$guardHandoff
        $Record.core_refusal=$typed;$Record.primary_core_cause=$primaryCoreCause;$Record.hook_failure=$hookFailure
        $Record.core_exception_observed=($null -ne $caughtCore);$Record.core_success_returned=$coreReturned
        $Record.pre_hook_core_observation=$preHookPublic;$Record.pre_hook_acquired_events=$preHookAcquired
        $Record.pre_hook_seen=$preHookSeen;$Record.pre_hook_qpc=$preHookQpc;$Record.core_observation=$corePublic
        $Record.failure=$caseFailure;$Record.failure_cause=$caseCause;$Record.failure_stage=if($caseFailure){$stage}else{$null}
        $Record.cleanup_first_cause=$cleanupFirstCause
        $Record.setup_started_qpc=$setupStart;$Record.baseline_complete_qpc=$baselineDone;$Record.alias_prepare_before_qpc=$prepareBefore
        $Record.alias_prepare_after_qpc=$prepareAfter;$Record.prepared_complete_qpc=$preparedDone;$Record.alias_disposed_qpc=$aliasDone;$Record.guards_closed_qpc=$guardsDone
        $Record.factory_before_qpc=$factoryBefore;$Record.factory_after_qpc=$factoryAfter
        $Record.core_factory_invoked=($factoryBefore -ne 0);$Record.core_capture_invoked=$captureInvoked
        $Record.core_dispose_attempted=$coreDisposeAttempted;$Record.core_disposed=$coreDisposed
        $Record.controller_directories_closed=$cleanupOk;$Record.remaining_controller_directories=$caseHeld.Count
        $Record.unreleased_current_directories=$unreleasedCurrentDirectories;$Record.controller_cleanup_unknown=$controllerCleanupUnknown
        $Record.acquired_files=$caseAcquired;$Record.hook_events=$caseHooks.ToArray();$Record.capture_qpc=if($corePublic){$corePublic.capture_complete_timestamp}else{0L}
        $Record.output_owner_bound=$false;$Record.proof_created=$false;$Record.receipt_count=0
        $Record.result=if($recordComplete -and -not $caseFailure -and -not $controllerCleanupUnknown -and $cleanupOk -and $aliasOk -and $aliasDisposed -and $coreDisposed){'observed'}else{'failed_or_unknown'}
    }
    if($Record.result -ceq 'observed'){
        try{Assert-CaseRecord $Record $Spec $absoluteWorkDeadline}
        catch{$Record.result='failed_or_unknown';$Record.failure=Failure-Code $_.Exception;$Record.failure_cause=Cause $_.Exception;$Record.failure_stage='record_assertions';throw}
    }
    Need ($Record.result -ceq 'observed')
    Check
}

$reportFile=$null;$result=1;$failure='not_started';$primaryFailure=$null;$failureCause=$null;$phase='bootstrap'
$geometry=$null;$binding=$null;$claim=$null;$guard=$null;$childrenAfter=$null;$managedDisposed=$false;$jobFinished=$false
$sourceBefore=[ordered]@{};$sourceAfter=[ordered]@{};$typeAbsent=[ordered]@{};$initializeCalls=[ordered]@{}
$diagnostics=[Collections.Generic.List[object]]::new();$diagnosticChars=0;$compilerError=$false;$diagnosticCap=$false
$phaseTimes=[ordered]@{};$phaseStart=[Diagnostics.Stopwatch]::GetTimestamp()
$syntheticParent=$w+'\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01';$syntheticParentCreated=$false
$familyHeld=[Collections.Generic.List[Microsoft.Win32.SafeHandles.SafeFileHandle]]::new()
$familyClosed=$false;$familyStreams=$null;$controllerCleanupUnknown=$false;$unreleasedCurrentDirectories=0;$fixtureAccess=$false
$directoryOwners=[Collections.Generic.Dictionary[Microsoft.Win32.SafeHandles.SafeFileHandle,object]]::new()
$directoryCurrent=$null;$directoryReturned=0;$directoryCloseAttempts=0;$directoryClosed=0;$directoryCleanupFirstCause=$null
$retainedUnknownOwners=[Collections.Generic.List[object]]::new()
$caseDirectories=@('origin','origin/one.state','origin/two.state','state','state/one','state/one/state',
    'state/two','state/two/state','state/safe-empty','backup','backup/one','backup/one/state','backup/two','backup/two/state')
$emptyHash='e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
$absoluteWorkDeadline=$StartTimestamp+20L*[Diagnostics.Stopwatch]::Frequency
$caseSpecs=@(Case-Specs);$records=[Collections.Generic.List[object]]::new()
foreach($spec in $caseSpecs){$records.Add([ordered]@{name=$spec.Name;selector=$spec.Selector;result='not_run'})}
function Set-Phase([string]$Name) {
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    $phaseTimes[$script:phase]=($now-$script:phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
    $script:phase=$Name;$script:phaseStart=$now;Check
}
function Diagnostic($Item,[string]$Kind) {
    $message=[string]$(if($Item -is [Management.Automation.ErrorRecord]){$Item.Exception.Message}else{[string]$Item})
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2G 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2G 'diagnostic_cap'}
}

try {
    [Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    if($StartTimestamp -le 0 -or $PSCommandPath -cne $preflight){Stop-C2G 'allocation_scope'}
    $binding=Validate-Allocation
    $started=Open-Public "$e\$attempt.started.json" 65536
    $claim=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $started 65536))|ConvertFrom-Json
    if($claim.classification -cne 'cold-core-filesystem-aliases-c2-start-r01' -or $claim.attempt -cne $attempt -or
        $claim.allocation_sha256 -cne $AllocationSha256 -or $claim.start_timestamp -ne $StartTimestamp -or
        $claim.stopwatch_frequency -ne [Diagnostics.Stopwatch]::Frequency -or $claim.permission_consumed -ne $true -or
        $claim.work_seconds -ne 20 -or $claim.whole_seconds -ne 30 -or $claim.retry_count -ne 0 -or $claim.lane_count -ne 1){Stop-C2G 'start_claim'}
    $reportFile=New-PublicOutput 'cases.json'
    if((Age) -ge 18){Stop-C2G 'bootstrap_reserve'}
    Set-Phase 'job_setup'
    $typeAbsent.deadline=$null -eq ('KilogramS5Deadline' -as [type])
    Need $typeAbsent.deadline 'core_type_cached'
    . $deadline
    Check;$guard=[KilogramS5Deadline]::new((20-(Age)),(27-(Age)));Check
    Set-Phase 'definitions'
    $tokens=$null;$parseErrors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($core,[ref]$tokens,[ref]$parseErrors)
    $top=@($ast.EndBlock.Statements)
    if($parseErrors.Count -ne 0 -or $top.Count -ne 3 -or
        @($top|Where-Object{$_ -isnot [Management.Automation.Language.FunctionDefinitionAst]}).Count -ne 0){Stop-C2G 'definitions_shape'}
    foreach($key in @('core','root_native','deadline')){$sourceBefore[$key]=Hash-Stream $binding.SourceFiles[$key]}
    $typeNames=[ordered]@{core='Kilogram.S53.ColdA.Core';root_native='S53RootNativeR38';premise_streams='S53C2GStreamsR01';alias_fixture='S53C2GAliasR01'}
    foreach($key in $typeNames.Keys){$typeAbsent[$key]=$null -eq ($typeNames[$key] -as [type]);Need $typeAbsent[$key] 'core_type_cached'}
    . $core
    . $rootNative
    Set-Phase 'compilation'
    foreach($initializer in @('Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-C2GPremises')){
        $initializeCalls[$initializer]=1
        try {
            & $initializer *>&1 | ForEach-Object {
                Check
                if($_ -is [Management.Automation.ErrorRecord]){$script:compilerError=$true;Diagnostic $_ 'error'}
                elseif($_ -is [Management.Automation.WarningRecord]){Diagnostic $_ 'warning'}
                else{Diagnostic $_ 'output'}
            }
        }catch{$compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2G 'compiler_error'}
        if($compilerError){Stop-C2G 'compiler_error'}
    }
    foreach($key in $typeNames.Keys){Need ($null -ne ($typeNames[$key] -as [type])) 'signature_mismatch'}
    Set-Phase 'family_setup';$fixtureAccess=$true
    $familyStreams=[S53C2GStreamsR01]::new('family',[Action]{Check})
    New-Family
    for($caseIndex=0;$caseIndex -lt 2;$caseIndex++){
        Set-Phase $caseSpecs[$caseIndex].Name
        Run-Case $caseSpecs[$caseIndex] $records[$caseIndex]
    }
    $geometry=[ordered]@{directories=1;files=0;stream_checks=1;logical_file_bytes=0;named_stream_bytes=0;all_logical_stream_bytes=0}
    foreach($record in $records){
        $geometry.directories+=$record.premises.directories;$geometry.files+=$record.premises.files
        $geometry.stream_checks+=$record.premises.streams_verified;$geometry.logical_file_bytes+=$record.premises.physical_file_bytes
        $geometry.named_stream_bytes+=$record.premises.named_stream_bytes
    }
    $geometry.all_logical_stream_bytes=$geometry.logical_file_bytes+$geometry.named_stream_bytes
    Need ($geometry.directories -eq 32 -and $geometry.files -eq 27 -and
        $geometry.stream_checks -eq 114 -and $geometry.logical_file_bytes -eq 7454 -and $geometry.named_stream_bytes -eq 0 -and $geometry.all_logical_stream_bytes -eq 7454)
    Set-Phase 'source_recheck'
    foreach($key in @('core','root_native','deadline')){
        $sourceAfter[$key]=Hash-Stream $binding.SourceFiles[$key]
        Need ($sourceAfter[$key] -ceq $sourceBefore[$key] -and $sourceAfter[$key] -ceq $binding.Pins[$key]) 'source_pin'
    }
    Need (@($records|Where-Object {$_.result -ceq 'observed'}).Count -eq 2)
    Check;$failure=$null;$result=0
}catch{$failure=Failure-Code $_.Exception;$primaryFailure=$failure;$failureCause=Cause $_.Exception}
finally {
    $familyClosed=Close-Directories $familyHeld
    if($familyStreams -and ($familyStreams.CleanupUnknown -or $familyStreams.RemainingHandles -ne 0)){$controllerCleanupUnknown=$true}
    if(-not $familyClosed -or $controllerCleanupUnknown){$result=1;$failure='cleanup_unknown'}
    try{if($guard){$guard.CleanupChildren([Math]::Max(1,[Math]::Min(1000,[int]((25-(Age))*1000))));$childrenAfter=$guard.Children().Length}}
    catch{$result=1;$failure='cleanup_unknown'}
    $managedDisposed=Close-PublicInputs
    if(-not $managedDisposed){$result=1;$failure='cleanup_unknown'}
}
try {
    if((Age) -ge 20 -or -not $guard -or $guard.WorkExpired -or $childrenAfter -ne 0){$result=1;if(-not $failure){$failure='work_expired'}}
    if((Age) -ge 25){Stop-C2G 'work_expired'}
    if($reportFile){
        $phaseTimes[$phase]=([Diagnostics.Stopwatch]::GetTimestamp()-$phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
        Write-Public $reportFile ([ordered]@{
            classification='cold-core-filesystem-aliases-c2-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
            child_pid=[Diagnostics.Process]::GetCurrentProcess().Id;start_timestamp=$StartTimestamp
            stopwatch_frequency=[Diagnostics.Stopwatch]::Frequency;started_supervisor_pid=$claim.supervisor_pid
            result=if($result -eq 0){'observed'}else{'failed_or_unknown'};failure=$failure;primary_failure=$primaryFailure
            failure_phase=if($failure){$phase}else{$null};failure_cause=$failureCause;whole_attempt_verdict='not_established'
            assertions_passed=($result -eq 0);source_pins_before=$sourceBefore;source_pins_after=$sourceAfter
            fresh_types_absent_before=$typeAbsent;initialization_calls=$initializeCalls;compiler_error=$compilerError
            diagnostic_cap=$diagnosticCap;diagnostics=$diagnostics.ToArray()
            powershell_version=(Version-Text ($PSVersionTable.PSVersion.ToString()));dotnet_version=(Version-Text ([Environment]::Version.ToString()))
            priority=[Diagnostics.Process]::GetCurrentProcess().PriorityClass.ToString()
            work_deadline_timestamp=$absoluteWorkDeadline;phase_seconds=$phaseTimes
            case_order=@($caseSpecs.Name);case_count=2;cases_observed=@($records|Where-Object {$_.result -ceq 'observed'}).Count
            cases=$records.ToArray();validated_geometry=$geometry;boundary_semantics='observed Core phase hooks on one absolute QPC; not exact phase starts'
            synthetic_parent_created=$syntheticParentCreated;family_directories_created=if($syntheticParentCreated){1}else{0}
            family_stream_check=(Stream-Facts $familyStreams);family_ancestry_disposed=$familyClosed;family_remaining_handles=$familyHeld.Count
            controller_cleanup_unknown=$controllerCleanupUnknown;unreleased_current_directories=$unreleasedCurrentDirectories;fixture_retained_if_created=$syntheticParentCreated
            directory_guard_ledger=[ordered]@{returned=$directoryReturned;close_attempts=$directoryCloseAttempts;closed=$directoryClosed
                remaining=@($directoryOwners.Values|Where-Object {-not $_.Closed}).Count;current_slots=if($null -eq $directoryCurrent){0}else{1};cleanup_unknown=$controllerCleanupUnknown}
            output_owner_bound=$false;proof_created=$false;receipt_count=0;receipt_bytes=0;receipt_digests=@{}
            owned_children_after=$childrenAfter;managed_inputs_disposed=$managedDisposed;job_finish_requires_actual_exit_zero=$true
            observed_seconds=(Age);work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1
            synthetic_fixture_access=$fixtureAccess;private_dataset_access=$false;native_false_close_path_tested=$false
        })
        $reportFile.Dispose();$reportFile=$null
    }
    if((Age) -ge 20){$result=1}
    if($guard){$guard.Finish();$jobFinished=$true}
    if(-not $jobFinished){$result=1}
}catch{$result=1}
finally{if($reportFile){try{$reportFile.Dispose()}catch{$result=1}}}
exit $result
