[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,
    [Parameter(Mandatory)][long]$StartTimestamp
)
# C2-O actual output ACL verifier refusal: source/static only; no execution allocation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-output-acl-01-attempt-01'
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
function Stop-C2O([string]$Code) {
    $err=[InvalidOperationException]::new('C2O bounded refusal');$err.Data['c2o_code']=$Code;throw $err
}
function Check {
    $age=Age
    if($age -lt 0 -or $age -ge 20){Stop-C2O 'work_expired'}
    if($guard){$guard.CheckWork()}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        $value=[string]$at.Data['c2o_code']
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
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2O 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2O 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2O 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2O 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2O 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2O 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2O 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2O 'source_pin'}
    return $f
}
function Validate-Allocation {
    Check
    if(-not $IsWindows -or [Environment]::ProcessPath -cne $shell){Stop-C2O 'allocation_scope'}
    $af=Open-Public $allocation 65536
    if((Hash-Stream $af) -cne $AllocationSha256){Stop-C2O 'allocation_pin'}
    $raw=Bytes $af 65536
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    if($text.Length -eq 0 -or $text[0] -eq [char]0xfeff){Stop-C2O 'allocation_schema'}
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        if($doc.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object){Stop-C2O 'allocation_schema'}
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($property in $doc.RootElement.EnumerateObject()){
            if($property.Name -cnotin $fields -or -not $map.TryAdd($property.Name,$property.Value)){Stop-C2O 'allocation_schema'}
        }
        if($map.Count -ne 22){Stop-C2O 'allocation_schema'}
        foreach($name in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256')){
            if($map[$name].ValueKind -ne [Text.Json.JsonValueKind]::String){Stop-C2O 'allocation_schema'}
        }
        if($map['schema_version'].GetInt32() -ne 1 -or $map['classification'].GetString() -cne 'root-approved-cold-core-output-acl-c2-r01' -or
           $map['root_thread_id'].GetString() -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or
           $map['activated'].ValueKind -ne [Text.Json.JsonValueKind]::True -or $map['attempt'].GetString() -cne $attempt -or
           $map['operation_set'].GetString() -cne 'cold-output-acl-one-only-r01' -or $map['workspace'].GetString() -cne $w -or
           $map['fixture_parent'].GetString() -cne ($w+'\.tmp\C-s53-cold-core-output-acl-01-attempt-01') -or
           $map['work_seconds'].GetInt32() -ne 20 -or $map['whole_seconds'].GetInt32() -ne 30 -or
           $map['retry_count'].GetInt32() -ne 0 -or $map['lane_count'].GetInt32() -ne 1 -or
           $map['public_report_bytes'].GetInt32() -ne 65536 -or $map['public_total_bytes'].GetInt32() -ne 196608){Stop-C2O 'allocation_scope'}
        $cases=$map['fixture_cases'];$fixed=@((Case-Specs).Selector)
        if($cases.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $cases.GetArrayLength() -ne 1){Stop-C2O 'allocation_scope'}
        for($n=0;$n -lt 1;$n++){
            if($cases[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $cases[$n].GetString() -cne $fixed[$n]){Stop-C2O 'allocation_scope'}
        }
        $outputs=$map['outputs']
        if($outputs.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $outputs.GetArrayLength() -ne 3){Stop-C2O 'allocation_schema'}
        $names=@('started.json','cases.json','watchdog.json')
        for($n=0;$n -lt 3;$n++){if($outputs[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $outputs[$n].GetString() -cne $names[$n]){Stop-C2O 'allocation_scope'}}
        $pins=[ordered]@{}
        foreach($name in @('preflight','watchdog','core','root_native','deadline','shell')){
            $value=$map[$name+'_sha256'].GetString()
            if($value -cnotmatch '\A[0-9a-f]{64}\z'){Stop-C2O 'allocation_schema'};$pins[$name]=$value
        }
        if($pins.core -cne 'ec3cd2190c4b67c201c02eeda6c68e5832e95ac114a2e570954e4a25a05f3bb9' -or
           $pins.root_native -cne 'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382' -or
           $pins.deadline -cne '0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45' -or
           $pins.shell -cne '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'){Stop-C2O 'source_pin'}
        $pf=Open-Public $preflight 131072;$wf=Open-Public $watchdog 131072
        if((Hash-Stream $pf) -cne $pins.preflight -or (Hash-Stream $wf) -cne $pins.watchdog){Stop-C2O 'source_pin'}
        $sourceFiles=[ordered]@{}
        $sourceFiles.core=Pin $core 79709 $pins.core
        $sourceFiles.root_native=Pin $rootNative 25596 $pins.root_native
        $sourceFiles.deadline=Pin $deadline 6038 $pins.deadline
        $null=Pin $shell 301368 $pins.shell
        return [pscustomobject]@{Pins=$pins;SourceFiles=$sourceFiles}
    }finally{$doc.Dispose()}
}
function New-PublicOutput([string]$Suffix) {
    if($Suffix -cnotin @('started.json','cases.json','watchdog.json')){Stop-C2O 'output_cap'}
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2O 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}
function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}

function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2O $Code}}
function Case-Specs {
    return @([pscustomobject]@{Name='output-acl';Selector='boundary-output-acl';InventoryBytes=3562;InventoryHash='1daa4c64c070458b8b0c494131b7f31eaa8058947c85fd906ef671b2c3a3d579';FailureCode='output_acl';FailurePhase='output_binding';FailureRole='output';Acquired=10;FileCount=13;PathPeak=6800;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';Phases=@('locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before','hash_bkp_before','hash_s_after','hash_bkp_after','scan_after','final_inputs','output_binding')})
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
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2o_code','c2o_fixture_code','c2o_fixture_operation')){
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
    Need ($Name -ceq 'output-acl') 'fixture_setup'
    return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-output-acl-01-attempt-01\\output-acl\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-output-acl-01-attempt-01\\output-acl\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-output-acl-01-attempt-01\\output-acl\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-output-acl-01-attempt-01\\output-acl\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@
}
function Initialize-C2OPremises {
    if(('S53C2OStreamsR01' -as [type]) -or ('Kilogram.S53.ColdOutputAcl.OutputAclOwner' -as [type])){Stop-C2O 'core_type_cached'}
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;
using Microsoft.Win32.SafeHandles;
using System.Runtime.ExceptionServices;
public sealed class S53C2OStreamsR01 {
    const string Parent=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-output-acl-01-attempt-01";
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
    public S53C2OStreamsR01(string name,Action check) {
        string[] names={"family","output-acl"};
        if(Array.IndexOf(names,name)<0||check==null)throw new ArgumentException("Closed C2L premise scope.");
        family=name=="family";root=family?Parent:Parent+"\\"+name;work=check;
    }
    Exception Refuse(string code,int native=0) {
        if(FailureCode==null){FailureCode=code;NativeError=native;}
        var e=native==0?new InvalidOperationException("C2L premise refusal"):
            new InvalidOperationException("C2L premise native refusal",new Win32Exception(native));
        e.Data["c2l_premise_code"]=code;return e;
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


namespace Kilogram.S53.ColdOutputAcl {
public sealed class OutputAclOwner : IDisposable {
    public const string Proof = @"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-output-acl-01-attempt-01\output-acl\proof";
    static readonly string[] Names={"owner.private.json","comparison.private.jsonl","namespace.private.jsonl","result.private.json"};
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    }
    [StructLayout(LayoutKind.Sequential)] struct SA {
        public int Length;public IntPtr Descriptor;[MarshalAs(UnmanagedType.Bool)] public bool Inherit;
    }
    sealed class Ancestor {internal string Path,Acl;internal Info Initial;internal SafeFileHandle Handle;}
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern SafeFileHandle CreateFileW(string p,uint a,uint s,IntPtr sa,uint c,uint f,IntPtr t);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true,EntryPoint="CreateFileW")]
    static extern SafeFileHandle CreateSecureFile(string p,uint a,uint s,ref SA sa,uint c,uint f,IntPtr t);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern bool CreateDirectoryW(string p,ref SA sa);
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(string s,uint revision,out IntPtr sd,out uint bytes);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr p);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint flags);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetHandleInformation(SafeFileHandle h,out uint flags);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint GetFileType(SafeFileHandle h);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr h);
    SafeFileHandle editor;Info eIdentity;string groupBefore;byte[] systemAce,userAce;
    int closeCalls,closedCalls,verifierCalls,factoryCalls,editorOpens,setCalls,pinCalls,pinFrees,daclQueries,validAclCalls;
    int emptyChecks,findOpened,findClosed;IntPtr search=new IntPtr(-1);
    bool narrowStarted,narrowed,postVerified,pinOwned,daclPresent,daclNonNull,daclValid,aceBytesPreserved,ownerGroupPreserved,emptyVerified;
    uint setResult;long narrowBefore,narrowAfter,postQpc;
    readonly long deadline;
    readonly string user;
    readonly List<Ancestor> ancestors=new List<Ancestor>(16);
    readonly StringBuilder finalPath=new StringBuilder(32768);
    SafeFileHandle directory;
    bool prepared,taken,disposed,prepareStarted;
    int receiptMask,transferredReceipts,ownedHandles;
    public bool DirectoryCreated {get;private set;}
    public bool DirectoryTransferred {get{return taken;}}
    public bool CleanupUnknown {get;private set;}
    public int CleanupNativeError {get;private set;}
    public int RemainingOwnedHandles {get{return ownedHandles;}}
    public int ReceiptCreatedMask {get{return receiptMask;}}
    public int ReceiptsTransferred {get{return transferredReceipts;}}
    public int AncestryCount {get{return ancestors.Count;}}
    OutputAclOwner(long absoluteWorkDeadline) {
        if(absoluteWorkDeadline<=Stopwatch.GetTimestamp())throw Refusal("deadline");
        deadline=absoluteWorkDeadline;
        using(var identity=WindowsIdentity.GetCurrent()) {
            if(identity.User==null)throw Refusal("owner_sid");
            user=identity.User.Value;
        }
        if(user=="S-1-5-18"||user=="S-1-5-32-544")throw Refusal("owner_sid");
    }
    // The sole binding has no path or command argument and performs no filesystem work here.
    public static OutputAclOwner Create(long absoluteWorkDeadline) {return new OutputAclOwner(absoluteWorkDeadline);}
    static Exception Refusal(string code) {
        var e=new InvalidOperationException("Cold output bounded refusal");e.Data["cold_output_code"]=code;
        if(code=="private_acl")e.Data["rootcopies_code"]="private_acl";
        return e;
    }
    void Work() {
        if(disposed)throw Refusal("disposed");
        if(CleanupUnknown)throw Refusal("cleanup_unknown");
        if(Stopwatch.GetTimestamp()>=deadline)throw Refusal("deadline");
    }
    static string IoPath(string path) {return @"\\?\"+path;}
    void BadNative(int error) {throw new Win32Exception(error);}
    bool CloseOwned(SafeFileHandle h) {
        if(h==null)return true;
        if(h.IsClosed){CleanupUnknown=true;return false;}
        if(h.IsInvalid){h.Dispose();return true;}
        closeCalls++;bool ok=CloseHandle(h.DangerousGetHandle());int error=ok?0:Marshal.GetLastWin32Error();
        h.SetHandleAsInvalid();h.Dispose(); // never retry a possibly recycled native value
        if(ok){ownedHandles--;closedCalls++;}
        else {CleanupUnknown=true;if(CleanupNativeError==0)CleanupNativeError=error;}
        return ok;
    }
    void FreeDescriptor(IntPtr sd) {
        if(sd!=IntPtr.Zero&&LocalFree(sd)!=IntPtr.Zero)CleanupUnknown=true;
    }
    SA Security(out IntPtr sd) {
        sd=IntPtr.Zero;uint bytes;
        string s="O:"+user+"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;"+user+")";
        if(!ConvertStringSecurityDescriptorToSecurityDescriptorW(s,1,out sd,out bytes))BadNative(Marshal.GetLastWin32Error());
        return new SA {Length=Marshal.SizeOf(typeof(SA)),Descriptor=sd,Inherit=false};
    }
    Info Check(SafeFileHandle h,string path,bool isDirectory) {
        Work();if(h==null||h.IsClosed||h.IsInvalid)throw Refusal("handle_shape");
        Info i;bool ok=GetFileInformationByHandle(h,out i);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)BadNative(error);
        if((i.Attributes&(0x400u|0x40u))!=0||((i.Attributes&0x10u)!=0)!=isDirectory||(!isDirectory&&i.Links!=1))
            throw Refusal("handle_shape");
        uint type=GetFileType(h);error=type==0?Marshal.GetLastWin32Error():0;
        if(type!=1){if(error!=0)BadNative(error);throw Refusal("handle_type");}
        uint flags;ok=GetHandleInformation(h,out flags);error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)BadNative(error);if((flags&1u)!=0)throw Refusal("inheritable");
        finalPath.Clear();uint n=GetFinalPathNameByHandleW(h,finalPath,(uint)finalPath.Capacity,0);
        error=n==0?Marshal.GetLastWin32Error():0;if(n==0)BadNative(error);
        if(n>=finalPath.Capacity)throw Refusal("handle_path");
        string actual=finalPath.ToString();if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual.Substring(4);
        if(!String.Equals(actual.TrimEnd('\\'),path.TrimEnd('\\'),StringComparison.OrdinalIgnoreCase))throw Refusal("handle_path");
        Work();return i;
    }
    SafeFileHandle OpenDirectory(string path) {
        Work();SafeFileHandle h=null;
        try {
            h=CreateFileW(IoPath(path),0x80,3,IntPtr.Zero,3,0x02200000,IntPtr.Zero);
            int error=h.IsInvalid?Marshal.GetLastWin32Error():0;
            if(h.IsInvalid)BadNative(error);ownedHandles++;
            Check(h,path,true);var answer=h;h=null;return answer;
        }finally{if(h!=null)CloseOwned(h);}
    }
    FileSystemSecurity Acl(string path,bool isDirectory) {
        Work();
        FileSystemSecurity acl=isDirectory?
            (FileSystemSecurity)FileSystemAclExtensions.GetAccessControl(new DirectoryInfo(path)):
            (FileSystemSecurity)FileSystemAclExtensions.GetAccessControl(new FileInfo(path));
        Work();return acl;
    }
    string AclSnapshot(string path) {
        string s=Acl(path,true).GetSecurityDescriptorSddlForm(AccessControlSections.Owner|AccessControlSections.Group|AccessControlSections.Access);
        if(s.Length>16384)throw Refusal("ancestry_acl_cap");return s;
    }
    void AssertPrivate(string path,bool isDirectory) {
        var acl=Acl(path,isDirectory);
        if(acl.GetOwner(typeof(SecurityIdentifier)).Value!=user||!acl.AreAccessRulesProtected)throw Refusal("private_acl");
        var seen=new HashSet<string>(StringComparer.Ordinal);
        foreach(FileSystemAccessRule rule in acl.GetAccessRules(true,true,typeof(SecurityIdentifier))) {
            string sid=rule.IdentityReference.Value;
            if((sid!=user&&sid!="S-1-5-18"&&sid!="S-1-5-32-544")||rule.IsInherited||
               rule.AccessControlType!=AccessControlType.Allow||
               (rule.FileSystemRights&FileSystemRights.FullControl)!=FileSystemRights.FullControl||!seen.Add(sid))
                throw Refusal("private_acl");
        }
        if(seen.Count!=3||!seen.Contains(user)||!seen.Contains("S-1-5-18")||!seen.Contains("S-1-5-32-544"))
            throw Refusal("private_acl");
    }
    int Role(string path) {
        if(String.Equals(path,Proof,StringComparison.Ordinal))return -1;
        for(int i=0;i<Names.Length;i++)if(String.Equals(path,Proof+"\\"+Names[i],StringComparison.Ordinal))return i;
        throw Refusal("path_scope");
    }
    public void PrepareDirectory() {
        Work();if(prepareStarted)throw Refusal("one_use");prepareStarted=true;
        var chain=new Stack<string>();string at=Path.GetDirectoryName(Proof);
        while(!String.IsNullOrEmpty(at)) {if(chain.Count>=16)throw Refusal("ancestry_cap");chain.Push(at);at=Path.GetDirectoryName(at);}
        foreach(string path in chain) {
            SafeFileHandle h=OpenDirectory(path);
            try {
                var record=new Ancestor {Path=path,Handle=h,Initial=Check(h,path,true),Acl=AclSnapshot(path)};
                ancestors.Add(record);h=null;
            }finally{if(h!=null)CloseOwned(h);}
        }
        IntPtr sd=IntPtr.Zero;
        try {
            Work();var sa=Security(out sd);
            bool ok=CreateDirectoryW(IoPath(Proof),ref sa);int error=ok?0:Marshal.GetLastWin32Error();
            if(!ok)BadNative(error);
            DirectoryCreated=true; // immediate ownership, before any subsequent validation or open
        }finally{FreeDescriptor(sd);}
        directory=OpenDirectory(Proof);AssertPrivate(Proof,true);eIdentity=Check(directory,Proof,true);RecheckAncestry();prepared=true;
    }
    // Caller becomes the sole owner; BindOwnedOutput consumes it on success AND refusal.
    public SafeFileHandle TakeDirectory() {
        Work();if(!prepared||taken||directory==null)throw Refusal("ownership");
        SafeFileHandle answer=directory;directory=null;taken=true;ownedHandles--;return answer;
    }
    SafeFileHandle CreateReceipt(string path) {
        Work();factoryCalls++;int role=Role(path);if(role<0||!prepared||!taken)throw Refusal("path_scope");
        SafeFileHandle h=null;IntPtr sd=IntPtr.Zero;
        try {
            var sa=Security(out sd);Work();
            h=CreateSecureFile(IoPath(path),0xC0000000,0,ref sa,1,0x00200000,IntPtr.Zero);
            int error=h.IsInvalid?Marshal.GetLastWin32Error():0;
            if(h.IsInvalid)BadNative(error);ownedHandles++;
            receiptMask|=1<<role; // preserve truthful partial ownership even if validation fails
            Info info=Check(h,path,false);if(info.SizeHigh!=0||info.SizeLow!=0)throw Refusal("not_empty");
            AssertPrivate(path,false);Work();
            SafeFileHandle answer=h;h=null;ownedHandles--;transferredReceipts++;return answer;
        }finally{if(h!=null)CloseOwned(h);FreeDescriptor(sd);}
    }
    void VerifyExactAcl(string path) {
        verifierCalls++;Work();int role=Role(path);if(!DirectoryCreated)throw Refusal("ownership");AssertPrivate(path,role<0);
    }
    public Func<string,SafeFileHandle> Creator {get{return CreateReceipt;}}
    public Action<string> AclVerifier {get{return VerifyExactAcl;}}
    public void RecheckAncestry() {
        Work();
        foreach(var a in ancestors) {
            Info now=Check(a.Handle,a.Path,true);
            if(now.Volume!=a.Initial.Volume||now.IndexHigh!=a.Initial.IndexHigh||now.IndexLow!=a.Initial.IndexLow||
               AclSnapshot(a.Path)!=a.Acl)throw Refusal("ancestry_changed");
        }
    }

    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct FindData {
        public uint Attributes;public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint SizeHigh,SizeLow,Reserved0,Reserved1;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=260)] public string Name;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=14)] public string AlternateName;
    }
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr FindFirstFileW(string p,out FindData data);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool FindNextFileW(IntPtr h,out FindData data);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool FindClose(IntPtr h);
    [DllImport("advapi32.dll",SetLastError=true)] static extern bool GetSecurityDescriptorDacl(IntPtr sd,[MarshalAs(UnmanagedType.Bool)] out bool present,out IntPtr dacl,[MarshalAs(UnmanagedType.Bool)] out bool defaulted);
    [DllImport("advapi32.dll")] static extern bool IsValidAcl(IntPtr dacl);
    [DllImport("advapi32.dll")] static extern uint SetSecurityInfo(SafeFileHandle h,int type,uint flags,IntPtr owner,IntPtr group,IntPtr dacl,IntPtr sacl);
    static bool SameId(Info a,Info b){return a.Volume==b.Volume&&a.IndexHigh==b.IndexHigh&&a.IndexLow==b.IndexLow;}
    static byte[] AceBytes(GenericAce a){byte[] b=new byte[a.BinaryLength];a.GetBinaryForm(b,0);return b;}
    static bool EqualBytes(byte[] a,byte[] b){if(a==null||b==null||a.Length!=b.Length)return false;for(int i=0;i<a.Length;i++)if(a[i]!=b[i])return false;return true;}
    RawSecurityDescriptor ReadSecurity(){
        var acl=Acl(Proof,true);byte[] raw=acl.GetSecurityDescriptorBinaryForm();
        if(raw.Length<=0||raw.Length>16384)throw Refusal("descriptor_cap");
        return new RawSecurityDescriptor(raw,0);
    }
    void VerifyIdentity(SafeFileHandle h){if(!SameId(eIdentity,Check(h,Proof,true)))throw Refusal("e_identity");}
    void EmptyDirectory(){
        Work();emptyChecks++;if(emptyChecks!=1||search!=new IntPtr(-1))throw Refusal("one_use");
        VerifyIdentity(directory);FindData data;
        try{
            search=FindFirstFileW(IoPath(Proof)+@"\*",out data);int error=search==new IntPtr(-1)?Marshal.GetLastWin32Error():0;
            if(search==IntPtr.Zero)throw Refusal("search_handle");
            if(search==new IntPtr(-1)){if(error!=2)BadNative(error);}
            else{
                findOpened++;ownedHandles++;int dotMask=0;
                for(int i=0;;i++){
                    Work();if(i>=2)throw Refusal("e_not_empty");
                    int bit=data.Name=="."?1:data.Name==".."?2:0;
                    if(bit==0||(dotMask&bit)!=0||(data.Attributes&16u)==0||(data.Attributes&1024u)!=0)throw Refusal("e_not_empty");
                    dotMask|=bit;bool next=FindNextFileW(search,out data);error=next?0:Marshal.GetLastWin32Error();
                    if(!next){if(error!=18)BadNative(error);break;}
                }
            }
        }finally{
            if(search!=new IntPtr(-1)&&search!=IntPtr.Zero){
                bool ok=FindClose(search);int error=ok?0:Marshal.GetLastWin32Error();
                search=new IntPtr(-1);if(ok){findClosed++;ownedHandles--;}else{CleanupUnknown=true;if(CleanupNativeError==0)CleanupNativeError=error;}
            }
        }
        if(CleanupUnknown)throw Refusal("empty_cleanup");VerifyIdentity(directory);emptyVerified=true;
    }
    void VerifyRestricted(){
        VerifyIdentity(editor);var sd=ReadSecurity();
        if(sd.Owner==null||sd.Owner.Value!=user||sd.Group==null||sd.Group.Value!=groupBefore||
           (sd.ControlFlags&ControlFlags.DiscretionaryAclProtected)==0||(sd.ControlFlags&ControlFlags.DiscretionaryAclPresent)==0||
           sd.DiscretionaryAcl==null||sd.DiscretionaryAcl.Count!=2)throw Refusal("narrowed_acl");
        bool sy=false,us=false;
        for(int i=0;i<2;i++){
            var a=sd.DiscretionaryAcl[i] as CommonAce;if(a==null||a.AceQualifier!=AceQualifier.AccessAllowed||a.IsCallback)throw Refusal("narrowed_acl");
            if(a.SecurityIdentifier.Value=="S-1-5-18"&&!sy&&EqualBytes(AceBytes(a),systemAce))sy=true;
            else if(a.SecurityIdentifier.Value==user&&!us&&EqualBytes(AceBytes(a),userAce))us=true;
            else throw Refusal("narrowed_acl");
        }
        if(!sy||!us)throw Refusal("narrowed_acl");aceBytesPreserved=true;ownerGroupPreserved=true;
    }
    public void NarrowNewEmptyE(){
        Work();if(!prepared||taken||directory==null||narrowStarted||receiptMask!=0||factoryCalls!=0)throw Refusal("narrow_boundary");
        narrowStarted=true;narrowBefore=Stopwatch.GetTimestamp();EmptyDirectory();AssertPrivate(Proof,true);
        SafeFileHandle current=null;
        try{
            editorOpens++;current=CreateFileW(IoPath(Proof),0x00060080u,3,IntPtr.Zero,3,0x02200000u,IntPtr.Zero);
            int error=current.IsInvalid?Marshal.GetLastWin32Error():0;if(current.IsInvalid)BadNative(error);ownedHandles++;
            VerifyIdentity(current);editor=current;current=null;
        }finally{if(current!=null)CloseOwned(current);}
        var sd=ReadSecurity();
        if(sd.Owner==null||sd.Owner.Value!=user||sd.Group==null||sd.DiscretionaryAcl==null||sd.DiscretionaryAcl.Count!=3||
           (sd.ControlFlags&ControlFlags.DiscretionaryAclProtected)==0||(sd.ControlFlags&ControlFlags.DiscretionaryAclPresent)==0)throw Refusal("before_acl");
        groupBefore=sd.Group.Value;int ba=-1;
        for(int i=0;i<3;i++){
            var a=sd.DiscretionaryAcl[i] as CommonAce;if(a==null||a.AceQualifier!=AceQualifier.AccessAllowed||a.IsCallback)throw Refusal("before_acl");
            string sid=a.SecurityIdentifier.Value;
            if(sid=="S-1-5-32-544"&&ba<0)ba=i;
            else if(sid=="S-1-5-18"&&systemAce==null)systemAce=AceBytes(a);
            else if(sid==user&&userAce==null)userAce=AceBytes(a);
            else throw Refusal("before_acl");
        }
        if(ba<0||systemAce==null||userAce==null)throw Refusal("before_acl");
        sd.DiscretionaryAcl.RemoveAce(ba);
        if(sd.DiscretionaryAcl.Count!=2||sd.BinaryLength<=0||sd.BinaryLength>16384)throw Refusal("descriptor_cap");
        byte[] descriptor=new byte[sd.BinaryLength];sd.GetBinaryForm(descriptor,0);GCHandle pin=default(GCHandle);
        try{
            pin=GCHandle.Alloc(descriptor,GCHandleType.Pinned);pinOwned=true;pinCalls++;
            IntPtr acl;bool present,defaulted;daclQueries++;
            bool ok=GetSecurityDescriptorDacl(pin.AddrOfPinnedObject(),out present,out acl,out defaulted);int error=ok?0:Marshal.GetLastWin32Error();
            if(!ok)BadNative(error);daclPresent=present;daclNonNull=acl!=IntPtr.Zero;
            if(!daclPresent||!daclNonNull)throw Refusal("null_dacl");
            validAclCalls++;daclValid=IsValidAcl(acl);if(!daclValid)throw Refusal("invalid_dacl");
            Work();setCalls++;setResult=SetSecurityInfo(editor,1,0x80000004u,IntPtr.Zero,IntPtr.Zero,acl,IntPtr.Zero);
            if(setResult!=0)BadNative(checked((int)setResult));
        }finally{
            if(pin.IsAllocated){try{pin.Free();pinOwned=false;pinFrees++;}catch{CleanupUnknown=true;}}
        }
        if(pinOwned||CleanupUnknown)throw Refusal("descriptor_cleanup");VerifyRestricted();RecheckAncestry();
        narrowed=true;narrowAfter=Stopwatch.GetTimestamp();
    }
    public void VerifyAfterCoreDisposed(){
        Work();if(!narrowed||!taken||editor==null||postVerified)throw Refusal("post_boundary");
        VerifyRestricted();RecheckAncestry();postVerified=true;postQpc=Stopwatch.GetTimestamp();
    }
    public Dictionary<string,object> Snapshot(){return new Dictionary<string,object>{
        {"directory_created",DirectoryCreated},{"directory_transferred",taken},{"ancestry_count",ancestors.Count},{"factory_calls",factoryCalls},
        {"receipt_created_mask",receiptMask},{"receipts_transferred",transferredReceipts},{"verifier_calls",verifierCalls},
        {"empty_checks",emptyChecks},{"empty_verified",emptyVerified},{"find_opened",findOpened},{"find_closed",findClosed},{"search_owned",search!=new IntPtr(-1)&&search!=IntPtr.Zero},
        {"editor_opens",editorOpens},{"editor_owned",editor!=null},{"set_calls",setCalls},{"set_result",setResult},
        {"pin_calls",pinCalls},{"pin_frees",pinFrees},{"pin_owned",pinOwned},{"dacl_queries",daclQueries},{"valid_acl_calls",validAclCalls},
        {"dacl_present",daclPresent},{"dacl_nonnull",daclNonNull},{"dacl_valid",daclValid},{"ace_bytes_preserved",aceBytesPreserved},{"owner_group_preserved",ownerGroupPreserved},
        {"narrowed",narrowed},{"post_verified",postVerified},{"close_calls",closeCalls},{"closed_calls",closedCalls},
        {"remaining_owned_handles",ownedHandles},{"cleanup_unknown",CleanupUnknown},{"cleanup_native_error",CleanupNativeError},{"disposed",disposed},
        {"narrow_before_qpc",narrowBefore},{"narrow_after_qpc",narrowAfter},{"post_qpc",postQpc}
    };}

    public void Dispose() {
        if(disposed)return;
        if(editor!=null){try{CloseOwned(editor);}catch{CleanupUnknown=true;}finally{editor=null;}}
        if(directory!=null){try{CloseOwned(directory);}catch{CleanupUnknown=true;}finally{directory=null;}}
        for(int i=ancestors.Count-1;i>=0;i--) {
            if(ancestors[i].Handle!=null){
                try{CloseOwned(ancestors[i].Handle);}catch{CleanupUnknown=true;}finally{ancestors[i].Handle=null;}
            }
        }
        disposed=true;
        if(ownedHandles!=0)CleanupUnknown=true;
    }
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
    Check;Need ($List.Count -lt $Cap -and $directoryOwners.Count -lt 20 -and $null -eq $directoryCurrent) 'fixture_setup'
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

function Verify-Premises($Spec,[string]$Root,$Files,$Streams,[bool]$Prepared) {
    $expected=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($d in $caseDirectories){$expected.Add($d,[pscustomobject]@{Directory=$true;Row=$null})}
    foreach($row in $Files){$expected.Add($row.Path,[pscustomobject]@{Directory=$false;Row=$row})}
    Need ($expected.Count -eq 27) 'premise_failure'
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $queue=[string[]]::new(16);$queue[0]='';$head=0;$tail=1;$count=0;$filesRead=0;$locks=0
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
                Need ($isDir -eq $entry.Directory -and ($item.Attributes -band ([IO.FileAttributes]::ReparsePoint -bor [IO.FileAttributes]::Device)) -eq 0) 'premise_failure'
                if($isDir){Need ($tail -lt 16) 'premise_failure';$queue[$tail++]=$relative;continue}
                $row=$entry.Row;$path=$Root+'\'+$relative.Replace('/','\');$owner=New-TrackedFile 'read'
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
    $strict=Stream-Facts $Streams;Assert-StreamSnapshot $strict 15 13
    $wantedReads=if($Prepared){26}else{13}
    Need ($seen.Count -eq $expected.Count -and $count -eq $expected.Count -and $tail -eq 15 -and
        $filesRead -eq 13 -and $locks -eq 6 -and $healthyInventory -and $caseState.read_streams_closed -eq $wantedReads -and
        $caseState.read_streams_open -eq 0 -and $sFiles -eq 5 -and $sBytes -eq 52 -and $bFiles -eq 5 -and $bBytes -eq 52 -and
        -not $controllerCleanupUnknown) 'premise_failure'
    return [ordered]@{ready=$true;healthy_physical_layout_verified=$true;bounded_entries=$count
        directories=15;files=$filesRead;physical_file_bytes=$physicalBytes
        s_payload_files=5;s_payload_bytes=52;bkp_payload_files=5;bkp_payload_bytes=52;empty_closed_locks=6
        target_object_verified=$true;canonical_names=$true;protected_exact_acl=$true;normal_objects_strict=$true
        streams_verified=$Streams.ChecksCompleted;strict_streams_verified=$Streams.ChecksCompleted
        named_stream_bytes=0;streams_unsupported_accepted=$false;setup_writers_closed=$true
        expected_files_spec=5;expected_bytes_spec=52;healthy_dummy_inventory_premises_verified=$healthyInventory
        core_inventory_not_executed_during_premises=$true;inventory_bytes=$Spec.InventoryBytes;inventory_sha256=$Spec.InventoryHash}
}


function Core-Error($Exception,$Spec) {
    Need ($Exception.GetType() -eq [Management.Automation.MethodInvocationException] -and $Exception.HResult -eq -2146233087) 'typed_refusal'
    $normal=$Exception.InnerException;$original=$normal.InnerException
    Need ($normal.GetType() -eq [InvalidOperationException] -and $normal.HResult -eq -2146233079 -and
        $original.GetType() -eq [InvalidOperationException] -and $original.HResult -eq -2146233079 -and $null -eq $original.InnerException) 'typed_refusal'
    Need ([string]$normal.Data['colda_code'] -ceq 'output_acl' -and [string]$normal.Data['colda_phase'] -ceq 'output_binding' -and [string]$normal.Data['colda_role'] -ceq 'output')
    Need ([string]$original.Data['rootcopies_code'] -ceq 'private_acl' -and [string]$original.Data['cold_output_code'] -ceq 'private_acl')
    Need ($normal.Data.Count -eq 3 -and $original.Data.Count -eq 2)
    return [ordered]@{matched=$true;code='output_acl';phase='output_binding';role='output';native_error=0;cause_kind='unexpected'
        normalized_marks=[ordered]@{colda_code=[string]$normal.Data['colda_code'];colda_phase=[string]$normal.Data['colda_phase'];colda_role=[string]$normal.Data['colda_role']}
        original_marks=[ordered]@{rootcopies_code=[string]$original.Data['rootcopies_code'];cold_output_code=[string]$original.Data['cold_output_code']}
        chain=@([ordered]@{type=$Exception.GetType().FullName;hresult=$Exception.HResult},[ordered]@{type=$normal.GetType().FullName;hresult=$normal.HResult},[ordered]@{type=$original.GetType().FullName;hresult=$original.HResult})}
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
function Assert-CoreSnapshot($o,$s,[string]$Stage) {
    Need ($Stage -cin @('capture','refusal','disposed'));$fault=$Stage -cne 'capture'
    $expected=[ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$true;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$true;input_handles_released=$true
        cleanup_unknown=$false;receipts_completed=$false;cleanup_native_error=0
        failure_code=if($fault){'output_acl'}else{$null};failure_phase=if($fault){'output_binding'}else{$null};failure_role=if($fault){'output'}else{$null}
        cause_kind=if($fault){'unexpected'}else{$null};native_error=0;error_hresult=if($fault){-2146233079}else{0}
        expected_files=5;expected_bytes=52;locks=6;hash_passes=4;hash_bytes=208
        historical_tuple_digest=$s.H;s_before_tuple_digest=$s.H;bkp_before_tuple_digest=$s.H;s_after_tuple_digest=$s.H;bkp_after_tuple_digest=$s.H
        peak_owned_handles=35;remaining_owned_handles=0;peak_path_bytes=6800;peak_record_bytes=12087312;peak_inventory_bytes=3562;scratch_reserved_bytes=524288
        receipt_bytes_charged=0;receipt_bytes_known_written=0;receipt_write_unknown=$false}
    Assert-Fields $o $expected $false;Need (@(Object-Keys $o).Count -eq 38)
    $capture=Get-Value $o 'capture_complete_timestamp';Need (($capture -is [int] -or $capture -is [long]) -and $capture -gt 0)
    $keys=@('allocation')+@($s.Phases[0..10]);if($fault){$keys+=@('output_binding')}
    $ak=@(Object-Keys $o.phase_seconds);Need ($ak.Count -eq $keys.Count -and ($ak -join '|') -ceq ($keys -join '|'))
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
    Assert-Fields $t ([ordered]@{matched=$true;code='output_acl';phase='output_binding';role='output';native_error=0;cause_kind='unexpected'}) $false
    Assert-Fields $t.normalized_marks ([ordered]@{colda_code='output_acl';colda_phase='output_binding';colda_role='output'}) $true
    Assert-Fields $t.original_marks ([ordered]@{rootcopies_code='private_acl';cold_output_code='private_acl'}) $true
    Need ($t.chain.Count -eq 3)
    Assert-Fields $t.chain[0] ([ordered]@{type='System.Management.Automation.MethodInvocationException';hresult=-2146233087}) $true
    foreach($n in $t.chain[1..2]){Assert-Fields $n ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079}) $true}
    Assert-Fields $cause ([ordered]@{type='System.InvalidOperationException';hresult=-2146233079;native_error=0}) $false
    Assert-Fields $cause.marks ([ordered]@{rootcopies_code='private_acl';colda_code='output_acl';colda_phase='output_binding';colda_role='output'}) $true
}


function Assert-CaseRecord($r,$s,[long]$Deadline) {
    Assert-Fields $r ([ordered]@{name=$s.Name;selector=$s.Selector;result='observed';failure=$null;failure_cause=$null;failure_stage=$null;cleanup_first_cause=$null
        core_factory_invoked=$true;core_capture_invoked=$true;core_dispose_attempted=$true;core_disposed=$true;core_success_returned=$true;core_exception_observed=$true
        output_bound=$false;output_write_invoked=$false;output_success_returned=$false;output_prepare_returned=$true
        bind_attempted=$true;bind_returned=$false;incoming_closed_at_bind_return=$true;incoming_closed_after_cleanup=$true;consumption_proved_before_dispose=$true
        expected_core_cleanup_unknown=$false;fixture_cleanup_known=$true;core_history_unchanged_after_fixture_cleanup=$true
        controller_directories_closed=$true;remaining_controller_directories=0;unreleased_current_directories=0
        controller_cleanup_unknown=$false;acquired_files=10;hook_failure=$null;proof_created=$true;positive_proof=$false;output_owner_bound=$false;receipt_count=0;receipt_bytes=0}) $false
    Assert-SetupSnapshot $r.setup $s;Assert-GuardHandoff $r.guard_handoff $s
    Assert-PremiseSnapshot $r.premises.baseline $s $false;Assert-PremiseSnapshot $r.premises.prepared $s $true
    Assert-Fields $r.premises ([ordered]@{ready=$true;directories=15;files=13;physical_file_bytes=(104+$s.InventoryBytes);named_stream_bytes=0;streams_verified=56}) $false
    Assert-StreamSnapshot $r.baseline_stream_check 15 13;Assert-StreamSnapshot $r.prepared_strict_stream_check 15 13
    Assert-CoreSnapshot $r.capture_observation $s 'capture';Assert-CoreSnapshot $r.core_observation $s 'refusal';Assert-CoreSnapshot $r.after_dispose_observation $s 'disposed'
    Validate-CoreError $r.core_refusal $r.primary_core_cause $s
    Assert-AclOwner $r.output_prepared 'prepared' $Deadline;Assert-AclOwner $r.output_restricted 'restricted' $Deadline
    Assert-AclOwner $r.output_at_fault 'refusal' $Deadline;Assert-AclOwner $r.output_helper 'final' $Deadline
    Need ($r.capture_qpc -gt $r.factory_after_qpc -and $r.capture_qpc -le $r.capture_return_qpc)
    foreach($o in @($r.capture_observation,$r.core_observation,$r.after_dispose_observation)){Need ($o.capture_complete_timestamp -eq $r.capture_qpc)}
    foreach($h in @($r.output_at_fault,$r.output_helper)){
        Need ($h.narrow_before_qpc -eq $r.output_restricted.narrow_before_qpc -and $h.narrow_after_qpc -eq $r.output_restricted.narrow_after_qpc -and
            $h.find_opened -eq $r.output_restricted.find_opened -and $h.find_closed -eq $r.output_restricted.find_closed)
    }
    $times=@($r.setup_started_qpc,$r.baseline_complete_qpc,$r.prepared_complete_qpc,$r.guards_closed_qpc,$r.factory_before_qpc,$r.factory_after_qpc,
        $r.capture_return_qpc,$r.output_prepare_before_qpc,$r.output_prepare_after_qpc,$r.output_restricted.narrow_before_qpc,$r.output_restricted.narrow_after_qpc,$r.bind_before_qpc,$r.bind_return_qpc,
        $r.consumption_proof_qpc,$r.core_dispose_before_qpc,$r.core_disposed_qpc,$r.output_helper.post_qpc,$r.fixture_disposed_qpc)
    $last=$StartTimestamp;foreach($time in $times){Need (($time -is [long] -or $time -is [int]) -and $time -ge $last -and $time -lt $Deadline);$last=$time}
    Need ($r.hook_events.Count -eq $s.Phases.Count)
    $last=$r.factory_after_qpc;foreach($i in 0..($s.Phases.Count-1)){
        $e=$r.hook_events[$i];Need (@(Object-Keys $e).Count -eq 2 -and $e.name -ceq $s.Phases[$i] -and
            ($e.qpc -is [long] -or $e.qpc -is [int]) -and $e.qpc -ge $last -and $e.qpc -lt $r.bind_return_qpc);$last=$e.qpc
    }
    Need ($r.hook_events[11].qpc -ge $r.bind_before_qpc)
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



function Invoke-AclBinding($Core,$Owner,[string]$Decision,[string]$CorePin) {
    Need ($Core.GetType().FullName -ceq 'Kilogram.S53.ColdA.Core' -and $Owner.GetType().FullName -ceq 'Kilogram.S53.ColdOutputAcl.OutputAclOwner')
    $m=$Core.GetType().GetMethod('BindOwnedOutput',[Reflection.BindingFlags]'Public,Instance')
    $creator=$Owner.Creator;$verifier=$Owner.AclVerifier;$parameters=$m.GetParameters()
    Need ($parameters.Count -eq 5 -and $parameters[0].ParameterType -eq [Microsoft.Win32.SafeHandles.SafeFileHandle] -and
        $parameters[1].ParameterType -eq $creator.GetType() -and $parameters[2].ParameterType -eq $verifier.GetType() -and
        $parameters[3].ParameterType -eq [string] -and $parameters[4].ParameterType -eq [string])
    $box=[pscustomobject]@{Pending=$null;Attempted=$false;Returned=$false;Error=$null;ReturnQpc=0L;ClosedAtReturn=$false}
    $box.Pending=$Owner.TakeDirectory()
    try{$box.Attempted=$true;$null=$m.Invoke($Core,[object[]]@($box.Pending,$creator,$verifier,$Decision,$CorePin));$box.Returned=$true}
    catch{$box.Error=$_.Exception}
    finally{$box.ReturnQpc=[Diagnostics.Stopwatch]::GetTimestamp();$box.ClosedAtReturn=$box.Pending.IsClosed}
    # Observe the original error/wrapper. Never infer method entry from TargetInvocationException or close an uncertain handle here.
    return $box
}


function Assert-AclOwner($h,[string]$Stage,[long]$Deadline) {
    Need ($Stage -cin @('prepared','restricted','refusal','final'))
    $changed=$Stage -cne 'prepared';$taken=$Stage -cin @('refusal','final');$final=$Stage -ceq 'final'
    Assert-Fields $h ([ordered]@{directory_created=$true;directory_transferred=$taken;ancestry_count=6;factory_calls=0
        receipt_created_mask=0;receipts_transferred=0;verifier_calls=if($taken){1}else{0}
        empty_checks=if($changed){1}else{0};empty_verified=$changed;search_owned=$false
        editor_opens=if($changed){1}else{0};editor_owned=($changed -and -not $final)
        set_calls=if($changed){1}else{0};set_result=0;pin_calls=if($changed){1}else{0};pin_frees=if($changed){1}else{0};pin_owned=$false
        dacl_queries=if($changed){1}else{0};valid_acl_calls=if($changed){1}else{0}
        dacl_present=$changed;dacl_nonnull=$changed;dacl_valid=$changed;ace_bytes_preserved=$changed;owner_group_preserved=$changed
        narrowed=$changed;post_verified=$final;close_calls=if($final){7}else{0};closed_calls=if($final){7}else{0}
        remaining_owned_handles=if($final){0}elseif($taken){7}elseif($changed){8}else{7}
        cleanup_unknown=$false;cleanup_native_error=0;disposed=$final}) $false
    Need (@(Object-Keys $h).Count -eq 37)
    Need ($h.find_opened -in @(0,1) -and $h.find_closed -eq $h.find_opened)
    if(-not $changed){Need ($h.find_opened -eq 0 -and $h.narrow_before_qpc -eq 0 -and $h.narrow_after_qpc -eq 0)}
    else{Need ($h.narrow_before_qpc -gt $StartTimestamp -and $h.narrow_after_qpc -ge $h.narrow_before_qpc -and $h.narrow_after_qpc -lt $Deadline)}
    if($final){Need ($h.post_qpc -ge $h.narrow_after_qpc -and $h.post_qpc -lt $Deadline)}
    else{Need ($h.post_qpc -eq 0)}
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
    $script:hookFailure=$null
    $beforeGuards=$directoryReturned;$beforeAttempts=$directoryCloseAttempts;$beforeClosed=$directoryClosed
    $coreObject=$null;$coreDisposed=$false;$coreDisposeAttempted=$false;$preStreams=$null;$postStreams=$null
    $coreUnknown=$false;$expectedFault=$false;$fixtureKnown=$false;$coreHistoryUnchanged=$false
    $outputOwner=$null;$outputDisposed=$false;$outputDisposeAttempted=$false;$outputPrepareReturned=$false
    $capturePublic=$null;$faultPublic=$null;$disposedPublic=$null;$disposedRaw=$null;$outputFacts=$null
    $outputPrepared=$null;$outputRestricted=$null;$outputFault=$null;$bindingResult=$null
    $captureReturn=0L;$prepareBefore=0L;$prepareAfter=0L;$bindBefore=0L;$consumptionQpc=0L;$disposeBefore=0L;$coreDisposedQpc=0L;$fixtureDisposedQpc=0L;$consumptionProved=$false
    $caseFailure=$null;$caseCause=$null;$primaryCoreCause=$null;$typed=$null;$stage='setup';$recordComplete=$false
    $baseline=$null;$prepared=$null;$premises=$null;$guardHandoff=$null;$cleanupOk=$false;$cleanupFirstCause=$null
    $factoryBefore=0L;$factoryAfter=0L;$captureInvoked=$false;$coreReturned=$false
    $setupStart=[Diagnostics.Stopwatch]::GetTimestamp();$baselineDone=0L;$preparedDone=0L;$guardsDone=0L
    try{
        Check;$Record.result='started';$root=$syntheticParent+'\'+$Spec.Name
        $preStreams=[S53C2OStreamsR01]::new($Spec.Name,[Action]{Check})
        $postStreams=[S53C2OStreamsR01]::new($Spec.Name,[Action]{Check})
        $files=@(Actual-Files $Spec);New-CaseFixture $Spec $root $files
        $stage='baseline_premises'
        $baseline=Verify-Premises $Spec $root $files $preStreams $false
        Assert-PremiseSnapshot $baseline $Spec $false;$baselineDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $stage='prepared_premises'
        $prepared=Verify-Premises $Spec $root $files $postStreams $true
        Assert-PremiseSnapshot $prepared $Spec $true;$preparedDone=[Diagnostics.Stopwatch]::GetTimestamp()
        $premises=[ordered]@{ready=$true;directories=$prepared.directories;files=$prepared.files;physical_file_bytes=$prepared.physical_file_bytes
            named_stream_bytes=0;streams_verified=($baseline.streams_verified+$prepared.streams_verified);baseline=$baseline;prepared=$prepared}
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
            }catch{$script:hookFailure=Cause $_.Exception;throw}
        }
        $coreObject.SetFixtureHook($hook)
        $stage='capture';$captureInvoked=$true
        $coreObject.CaptureAndCompare();$coreReturned=$true;$captureReturn=[Diagnostics.Stopwatch]::GetTimestamp()
        Need ($null -eq $hookFailure -and $caseAcquired -eq $Spec.Acquired -and $caseHookIndex -eq 11)
        $capturePublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        Assert-CoreSnapshot $capturePublic $Spec 'capture'
        $stage='output_prepare';$prepareBefore=[Diagnostics.Stopwatch]::GetTimestamp()
        $outputOwner=[Kilogram.S53.ColdOutputAcl.OutputAclOwner]::Create($absoluteWorkDeadline)
        $outputOwner.PrepareDirectory();$outputPrepareReturned=$true;$prepareAfter=[Diagnostics.Stopwatch]::GetTimestamp();$outputPrepared=$outputOwner.Snapshot()
        Assert-AclOwner $outputPrepared 'prepared' $absoluteWorkDeadline
        $stage='output_narrow';$outputOwner.NarrowNewEmptyE();$outputRestricted=$outputOwner.Snapshot()
        Assert-AclOwner $outputRestricted 'restricted' $absoluteWorkDeadline
        $stage='output_bind';$bindBefore=[Diagnostics.Stopwatch]::GetTimestamp()
        $bindingResult=Invoke-AclBinding $coreObject $outputOwner $AllocationSha256 $binding.Pins.core
        # This barrier must finish before any Core.Dispose. It observes the original wrapper and real verifier.
        Need ($bindingResult.Attempted -and -not $bindingResult.Returned -and $null -ne $bindingResult.Error) 'typed_refusal'
        $primaryCoreCause=Cause $bindingResult.Error
        $typed=Core-Error $bindingResult.Error $Spec;Validate-CoreError $typed $primaryCoreCause $Spec
        $faultPublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        Assert-CoreSnapshot $faultPublic $Spec 'refusal'
        $outputFault=$outputOwner.Snapshot();Assert-AclOwner $outputFault 'refusal' $absoluteWorkDeadline
        Need ($bindingResult.ClosedAtReturn -and $bindingResult.Pending.IsClosed -and
            $faultPublic.remaining_owned_handles -eq 0 -and -not $faultPublic.cleanup_unknown -and
            $faultPublic.cleanup_native_error -eq 0 -and $outputFault.verifier_calls -eq 1) 'typed_refusal'
        Need ($null -eq $hookFailure -and $caseAcquired -eq 10 -and $caseHookIndex -eq $Spec.Phases.Count)
        $consumptionQpc=[Diagnostics.Stopwatch]::GetTimestamp();$consumptionProved=$true
        $stage='core_dispose';$disposeBefore=[Diagnostics.Stopwatch]::GetTimestamp();$coreDisposeAttempted=$true
        $coreObject.Dispose();$coreDisposed=$true;$coreDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()
        $disposedRaw=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8());$disposedPublic=$disposedRaw|ConvertFrom-Json
        Assert-CoreSnapshot $disposedPublic $Spec 'disposed';$expectedFault=$true
        $stage='output_verify';$outputOwner.VerifyAfterCoreDisposed()
        $outputDisposeAttempted=$true;$outputOwner.Dispose();$outputDisposed=$true
        $fixtureDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp();$outputFacts=$outputOwner.Snapshot()
        Assert-AclOwner $outputFacts 'final' $absoluteWorkDeadline
        Need ([Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8()) -ceq $disposedRaw)
        $coreHistoryUnchanged=$true;$fixtureKnown=$true
        Check;$recordComplete=$true
    }catch{$caseFailure=Failure-Code $_.Exception;$caseCause=Cause $_.Exception}
    finally{
        if($coreObject -and -not $coreDisposeAttempted){
            $coreDisposeAttempted=$true;$disposeBefore=[Diagnostics.Stopwatch]::GetTimestamp()
            try{$coreObject.Dispose();$coreDisposed=$true;$coreDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
            catch{$coreUnknown=$true;$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        # Never retry-close an uncertain incoming handle, including a pre-entry reflection failure.
        if($bindingResult -and $bindingResult.Pending -and -not $bindingResult.Pending.IsClosed){
            $script:controllerCleanupUnknown=$true;Retain-UnknownOwner $bindingResult.Pending
        }
        if($outputOwner -and -not $outputDisposeAttempted){
            $outputDisposeAttempted=$true
            try{$outputOwner.Dispose();$outputDisposed=$true;$fixtureDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
            catch{$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        if($outputOwner){
            try{$outputFacts=$outputOwner.Snapshot();if(-not $outputDisposed -or $outputOwner.CleanupUnknown -or $outputOwner.RemainingOwnedHandles -ne 0){
                $script:controllerCleanupUnknown=$true;Retain-UnknownOwner $outputOwner}}
            catch{$script:controllerCleanupUnknown=$true;Retain-UnknownOwner $outputOwner;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        if($coreObject){
            try{
                $facts=$coreObject.GetObservation()
                if($facts.CleanupUnknown -or $facts.RemainingOwnedHandles -ne 0 -or -not $facts.InputHandlesReleased){$coreUnknown=$true;$script:controllerCleanupUnknown=$true}
            }catch{$coreUnknown=$true;$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
            if(-not $coreDisposed){$coreUnknown=$true;$script:controllerCleanupUnknown=$true}
            if($coreUnknown){Retain-UnknownOwner $coreObject}
        }
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
        $Record.setup=$caseState;$Record.premises=$premises;$Record.guard_handoff=$guardHandoff
        $Record.baseline_stream_check=Stream-Facts $preStreams;$Record.prepared_strict_stream_check=Stream-Facts $postStreams
        $Record.core_refusal=$typed;$Record.primary_core_cause=$primaryCoreCause;$Record.hook_failure=$hookFailure
        $Record.core_exception_observed=($null -ne $bindingResult -and $null -ne $bindingResult.Error);$Record.core_success_returned=$coreReturned
        $Record.capture_observation=$capturePublic;$Record.core_observation=$faultPublic;$Record.after_dispose_observation=$disposedPublic
        $Record.expected_core_cleanup_unknown=$false;$Record.fixture_cleanup_known=$fixtureKnown
        $Record.core_history_unchanged_after_fixture_cleanup=$coreHistoryUnchanged
        $Record.output_helper=$outputFacts;$Record.output_prepared=$outputPrepared;$Record.output_restricted=$outputRestricted;$Record.output_at_fault=$outputFault
        $Record.output_prepare_returned=$outputPrepareReturned;$Record.output_bound=($null -ne $bindingResult -and $bindingResult.Returned);$Record.output_write_invoked=$false;$Record.output_success_returned=$false
        $Record.bind_attempted=($null -ne $bindingResult -and $bindingResult.Attempted);$Record.bind_returned=($null -ne $bindingResult -and $bindingResult.Returned)
        $Record.incoming_closed_at_bind_return=($null -ne $bindingResult -and $bindingResult.ClosedAtReturn)
        $Record.incoming_closed_after_cleanup=($null -ne $bindingResult -and $bindingResult.Pending.IsClosed)
        $Record.consumption_proved_before_dispose=$consumptionProved;$Record.consumption_proof_qpc=$consumptionQpc
        $Record.output_prepare_before_qpc=$prepareBefore;$Record.output_prepare_after_qpc=$prepareAfter
        $Record.bind_before_qpc=$bindBefore;$Record.bind_return_qpc=if($bindingResult){$bindingResult.ReturnQpc}else{0L}
        $Record.capture_return_qpc=$captureReturn;$Record.core_dispose_before_qpc=$disposeBefore
        $Record.core_disposed_qpc=$coreDisposedQpc;$Record.fixture_disposed_qpc=$fixtureDisposedQpc
        $Record.failure=$caseFailure;$Record.failure_cause=$caseCause;$Record.failure_stage=if($caseFailure){$stage}else{$null}
        $Record.cleanup_first_cause=$cleanupFirstCause
        $Record.setup_started_qpc=$setupStart;$Record.baseline_complete_qpc=$baselineDone;$Record.prepared_complete_qpc=$preparedDone
        $Record.guards_closed_qpc=$guardsDone;$Record.factory_before_qpc=$factoryBefore;$Record.factory_after_qpc=$factoryAfter
        $Record.core_factory_invoked=($factoryBefore -ne 0);$Record.core_capture_invoked=$captureInvoked
        $Record.core_dispose_attempted=$coreDisposeAttempted;$Record.core_disposed=$coreDisposed
        $Record.controller_directories_closed=$cleanupOk;$Record.remaining_controller_directories=$caseHeld.Count
        $Record.unreleased_current_directories=$unreleasedCurrentDirectories;$Record.controller_cleanup_unknown=$controllerCleanupUnknown
        $Record.acquired_files=$caseAcquired;$Record.hook_events=$caseHooks.ToArray();$Record.capture_qpc=if($capturePublic){$capturePublic.capture_complete_timestamp}else{0L}
        $Record.output_owner_bound=($null -ne $bindingResult -and $bindingResult.Returned);$Record.proof_created=($null -ne $outputFacts -and $outputFacts.directory_created)
        $Record.positive_proof=$false;$Record.receipt_count=if($outputFacts){$outputFacts.receipts_transferred}else{0};$Record.receipt_bytes=0
        $Record.result=if($recordComplete -and -not $caseFailure -and -not $controllerCleanupUnknown -and $cleanupOk -and $coreDisposed -and $expectedFault -and
            $fixtureKnown -and $coreHistoryUnchanged -and $consumptionProved){'observed'}else{'failed_or_unknown'}
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
$syntheticParent=$w+'\.tmp\C-s53-cold-core-output-acl-01-attempt-01';$syntheticParentCreated=$false
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
foreach($spec in $caseSpecs){$records.Add([ordered]@{name=$spec.Name;selector=$spec.Selector;result='not_run';output_owner_bound=$false;proof_created=$false;receipt_count=0;receipt_bytes=0})}
function Set-Phase([string]$Name) {
    $now=[Diagnostics.Stopwatch]::GetTimestamp()
    $phaseTimes[$script:phase]=($now-$script:phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
    $script:phase=$Name;$script:phaseStart=$now;Check
}
function Diagnostic($Item,[string]$Kind) {
    $message=[string]$(if($Item -is [Management.Automation.ErrorRecord]){$Item.Exception.Message}else{[string]$Item})
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2O 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2O 'diagnostic_cap'}
}

try {
    [Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    if($StartTimestamp -le 0 -or $PSCommandPath -cne $preflight){Stop-C2O 'allocation_scope'}
    $binding=Validate-Allocation
    $started=Open-Public "$e\$attempt.started.json" 65536
    $claim=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $started 65536))|ConvertFrom-Json
    if($claim.classification -cne 'cold-core-output-acl-c2-start-r01' -or $claim.attempt -cne $attempt -or
        $claim.allocation_sha256 -cne $AllocationSha256 -or $claim.start_timestamp -ne $StartTimestamp -or
        $claim.stopwatch_frequency -ne [Diagnostics.Stopwatch]::Frequency -or $claim.permission_consumed -ne $true -or
        $claim.work_seconds -ne 20 -or $claim.whole_seconds -ne 30 -or $claim.retry_count -ne 0 -or $claim.lane_count -ne 1){Stop-C2O 'start_claim'}
    $reportFile=New-PublicOutput 'cases.json'
    if((Age) -ge 18){Stop-C2O 'bootstrap_reserve'}
    Set-Phase 'job_setup'
    $typeAbsent.deadline=$null -eq ('KilogramS5Deadline' -as [type])
    Need $typeAbsent.deadline 'core_type_cached'
    . $deadline
    Check;$guard=[KilogramS5Deadline]::new((20-(Age)),(27-(Age)));Check
    Set-Phase 'definitions'
    $tokens=$null;$parseErrors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($core,[ref]$tokens,[ref]$parseErrors)
    $top=@($ast.EndBlock.Statements)
    if($parseErrors.Count -ne 0 -or $top.Count -ne 3 -or
        @($top|Where-Object{$_ -isnot [Management.Automation.Language.FunctionDefinitionAst]}).Count -ne 0){Stop-C2O 'definitions_shape'}
    foreach($key in @('core','root_native','deadline')){$sourceBefore[$key]=Hash-Stream $binding.SourceFiles[$key]}
    $typeNames=[ordered]@{core='Kilogram.S53.ColdA.Core';root_native='S53RootNativeR38';premise_streams='S53C2OStreamsR01';output_fault_owner='Kilogram.S53.ColdOutputAcl.OutputAclOwner'}
    foreach($key in $typeNames.Keys){$typeAbsent[$key]=$null -eq ($typeNames[$key] -as [type]);Need $typeAbsent[$key] 'core_type_cached'}
    . $core
    . $rootNative
    Set-Phase 'compilation'
    foreach($initializer in @('Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-C2OPremises')){
        $initializeCalls[$initializer]=1
        try {
            & $initializer *>&1 | ForEach-Object {
                Check
                if($_ -is [Management.Automation.ErrorRecord]){$script:compilerError=$true;Diagnostic $_ 'error'}
                elseif($_ -is [Management.Automation.WarningRecord]){Diagnostic $_ 'warning'}
                else{Diagnostic $_ 'output'}
            }
        }catch{$compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2O 'compiler_error'}
        if($compilerError){Stop-C2O 'compiler_error'}
    }
    foreach($key in $typeNames.Keys){Need ($null -ne ($typeNames[$key] -as [type])) 'signature_mismatch'}
    Set-Phase 'family_setup';$fixtureAccess=$true
    $familyStreams=[S53C2OStreamsR01]::new('family',[Action]{Check})
    New-Family
    for($caseIndex=0;$caseIndex -lt 1;$caseIndex++){
        Set-Phase $caseSpecs[$caseIndex].Name
        Run-Case $caseSpecs[$caseIndex] $records[$caseIndex]
    }
    $geometry=[ordered]@{directories=1;files=0;stream_checks=1;logical_file_bytes=0;named_stream_bytes=0;all_logical_stream_bytes=0}
    foreach($record in $records){
        $geometry.directories+=($record.premises.directories+1);$geometry.files+=$record.premises.files
        $geometry.stream_checks+=$record.premises.streams_verified;$geometry.logical_file_bytes+=($record.premises.physical_file_bytes+$record.receipt_bytes)
        $geometry.named_stream_bytes+=$record.premises.named_stream_bytes
    }
    $geometry.all_logical_stream_bytes=$geometry.logical_file_bytes+$geometry.named_stream_bytes
    Need ($geometry.directories -eq 17 -and $geometry.files -eq 13 -and
        $geometry.stream_checks -eq 57 -and $geometry.logical_file_bytes -eq 3666 -and $geometry.named_stream_bytes -eq 0 -and $geometry.all_logical_stream_bytes -eq 3666)
    Set-Phase 'source_recheck'
    foreach($key in @('core','root_native','deadline')){
        $sourceAfter[$key]=Hash-Stream $binding.SourceFiles[$key]
        Need ($sourceAfter[$key] -ceq $sourceBefore[$key] -and $sourceAfter[$key] -ceq $binding.Pins[$key]) 'source_pin'
    }
    Need (@($records|Where-Object {$_.result -ceq 'observed'}).Count -eq 1)
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
    if((Age) -ge 25){Stop-C2O 'work_expired'}
    if($reportFile){
        $phaseTimes[$phase]=([Diagnostics.Stopwatch]::GetTimestamp()-$phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
        Write-Public $reportFile ([ordered]@{
            classification='cold-core-output-acl-c2-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
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
            case_order=@($caseSpecs.Name);case_count=1;cases_observed=@($records|Where-Object {$_.result -ceq 'observed'}).Count
            cases=$records.ToArray();validated_geometry=$geometry;boundary_semantics='observed Core phase hooks on one absolute QPC; not exact phase starts'
            synthetic_parent_created=$syntheticParentCreated;family_directories_created=if($syntheticParentCreated){1}else{0}
            family_stream_check=(Stream-Facts $familyStreams);family_ancestry_disposed=$familyClosed;family_remaining_handles=$familyHeld.Count
            controller_cleanup_unknown=$controllerCleanupUnknown;unreleased_current_directories=$unreleasedCurrentDirectories;fixture_retained_if_created=$syntheticParentCreated
            directory_guard_ledger=[ordered]@{returned=$directoryReturned;close_attempts=$directoryCloseAttempts;closed=$directoryClosed
                remaining=@($directoryOwners.Values|Where-Object {-not $_.Closed}).Count;current_slots=if($null -eq $directoryCurrent){0}else{1};cleanup_unknown=$controllerCleanupUnknown}
            output_owner_bound=$records[0].output_owner_bound;proof_created=$records[0].proof_created;positive_proof=$false;receipt_count=$records[0].receipt_count;receipt_bytes=$records[0].receipt_bytes;receipt_digests=@{}
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
