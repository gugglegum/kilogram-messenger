[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$AllocationSha256,
    [Parameter(Mandatory)][long]$StartTimestamp
)
# C2-L receipt native close: source/static only; execution requires a separate root allocation.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$attempt='C-s53-cold-core-receipt-close-01-attempt-01'
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
function Stop-C2L([string]$Code) {
    $err=[InvalidOperationException]::new('C2L bounded refusal');$err.Data['c2l_code']=$Code;throw $err
}
function Check {
    $age=Age
    if($age -lt 0 -or $age -ge 20){Stop-C2L 'work_expired'}
    if($guard){$guard.CheckWork()}
}
function Failure-Code($Exception) {
    $depth=0
    for($at=$Exception;$null -ne $at -and $depth -lt 16;$at=$at.InnerException){
        $depth++
        $value=[string]$at.Data['c2l_code']
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
       -not [string]::Equals([IO.Path]::GetFullPath($Path),$Path,[StringComparison]::OrdinalIgnoreCase)){Stop-C2L 'public_input'}
    $at=[IO.Path]::GetDirectoryName($Path)
    while($at){
        Check;$attrs=[IO.File]::GetAttributes($at)
        if(($attrs -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attrs -band [IO.FileAttributes]::Directory) -eq 0){Stop-C2L 'public_input'}
        $at=[IO.Path]::GetDirectoryName($at)
    }
    if(([IO.File]::GetAttributes($Path) -band ([IO.FileAttributes]::Directory -bor [IO.FileAttributes]::ReparsePoint)) -ne 0){Stop-C2L 'public_input'}
    $f=$null
    try{
        $f=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        if($f.Length -lt 0 -or $f.Length -gt $Maximum){Stop-C2L 'public_input'}
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
    if($Stream.Length -gt $Maximum -or $Stream.Length -lt 0){Stop-C2L 'public_input'}
    $raw=[byte[]]::new([int]$Stream.Length);$Stream.Position=0;$offset=0
    while($offset -lt $raw.Length){Check;$n=$Stream.Read($raw,$offset,[Math]::Min(16384,$raw.Length-$offset));if($n -eq 0){Stop-C2L 'public_input'};$offset+=$n}
    if($Stream.ReadByte() -ne -1 -or $Stream.Length -ne $raw.Length){Stop-C2L 'public_input'}
    $Stream.Position=0;Check;return ,$raw
}
function Pin([string]$Path,[long]$Length,[string]$Sha256) {
    $f=Open-Public $Path 524288
    if($f.Length -ne $Length -or (Hash-Stream $f) -cne $Sha256){Stop-C2L 'source_pin'}
    return $f
}
function Validate-Allocation {
    Check
    if(-not $IsWindows -or [Environment]::ProcessPath -cne $shell){Stop-C2L 'allocation_scope'}
    $af=Open-Public $allocation 65536
    if((Hash-Stream $af) -cne $AllocationSha256){Stop-C2L 'allocation_pin'}
    $raw=Bytes $af 65536
    $text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
    if($text.Length -eq 0 -or $text[0] -eq [char]0xfeff){Stop-C2L 'allocation_schema'}
    $options=[Text.Json.JsonDocumentOptions]::new();$options.MaxDepth=6
    $doc=[Text.Json.JsonDocument]::Parse($text,$options)
    try {
        $fields=@('schema_version','classification','root_thread_id','activated','attempt','operation_set','workspace','fixture_parent','fixture_cases',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256',
            'work_seconds','whole_seconds','retry_count','lane_count','outputs','public_report_bytes','public_total_bytes')
        if($doc.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object){Stop-C2L 'allocation_schema'}
        $map=[Collections.Generic.Dictionary[string,Text.Json.JsonElement]]::new([StringComparer]::Ordinal)
        foreach($property in $doc.RootElement.EnumerateObject()){
            if($property.Name -cnotin $fields -or -not $map.TryAdd($property.Name,$property.Value)){Stop-C2L 'allocation_schema'}
        }
        if($map.Count -ne 22){Stop-C2L 'allocation_schema'}
        foreach($name in @('classification','root_thread_id','attempt','operation_set','workspace','fixture_parent',
            'preflight_sha256','watchdog_sha256','core_sha256','root_native_sha256','deadline_sha256','shell_sha256')){
            if($map[$name].ValueKind -ne [Text.Json.JsonValueKind]::String){Stop-C2L 'allocation_schema'}
        }
        if($map['schema_version'].GetInt32() -ne 1 -or $map['classification'].GetString() -cne 'root-approved-cold-core-receipt-close-c2-r01' -or
           $map['root_thread_id'].GetString() -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or
           $map['activated'].ValueKind -ne [Text.Json.JsonValueKind]::True -or $map['attempt'].GetString() -cne $attempt -or
           $map['operation_set'].GetString() -cne 'cold-receipt-close-one-only-r01' -or $map['workspace'].GetString() -cne $w -or
           $map['fixture_parent'].GetString() -cne ($w+'\.tmp\C-s53-cold-core-receipt-close-01-attempt-01') -or
           $map['work_seconds'].GetInt32() -ne 20 -or $map['whole_seconds'].GetInt32() -ne 30 -or
           $map['retry_count'].GetInt32() -ne 0 -or $map['lane_count'].GetInt32() -ne 1 -or
           $map['public_report_bytes'].GetInt32() -ne 65536 -or $map['public_total_bytes'].GetInt32() -ne 196608){Stop-C2L 'allocation_scope'}
        $cases=$map['fixture_cases'];$fixed=@((Case-Specs).Selector)
        if($cases.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $cases.GetArrayLength() -ne 1){Stop-C2L 'allocation_scope'}
        for($n=0;$n -lt 1;$n++){
            if($cases[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $cases[$n].GetString() -cne $fixed[$n]){Stop-C2L 'allocation_scope'}
        }
        $outputs=$map['outputs']
        if($outputs.ValueKind -ne [Text.Json.JsonValueKind]::Array -or $outputs.GetArrayLength() -ne 3){Stop-C2L 'allocation_schema'}
        $names=@('started.json','cases.json','watchdog.json')
        for($n=0;$n -lt 3;$n++){if($outputs[$n].ValueKind -ne [Text.Json.JsonValueKind]::String -or $outputs[$n].GetString() -cne $names[$n]){Stop-C2L 'allocation_scope'}}
        $pins=[ordered]@{}
        foreach($name in @('preflight','watchdog','core','root_native','deadline','shell')){
            $value=$map[$name+'_sha256'].GetString()
            if($value -cnotmatch '\A[0-9a-f]{64}\z'){Stop-C2L 'allocation_schema'};$pins[$name]=$value
        }
        if($pins.core -cne '7fe9ad4f60fe8f4e5b2ae6b191db183968e19084e0f4e39f8f23d87e2ba759ca' -or
           $pins.root_native -cne 'a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382' -or
           $pins.deadline -cne '0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45' -or
           $pins.shell -cne '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'){Stop-C2L 'source_pin'}
        $pf=Open-Public $preflight 131072;$wf=Open-Public $watchdog 131072
        if((Hash-Stream $pf) -cne $pins.preflight -or (Hash-Stream $wf) -cne $pins.watchdog){Stop-C2L 'source_pin'}
        $sourceFiles=[ordered]@{}
        $sourceFiles.core=Pin $core 78215 $pins.core
        $sourceFiles.root_native=Pin $rootNative 25596 $pins.root_native
        $sourceFiles.deadline=Pin $deadline 6038 $pins.deadline
        $null=Pin $shell 301368 $pins.shell
        return [pscustomobject]@{Pins=$pins;SourceFiles=$sourceFiles}
    }finally{$doc.Dispose()}
}
function New-PublicOutput([string]$Suffix) {
    if($Suffix -cnotin @('started.json','cases.json','watchdog.json')){Stop-C2L 'output_cap'}
    return [IO.File]::Open("$e\$attempt.$Suffix",[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
}
function Write-Public([IO.FileStream]$Stream,$Value) {
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
    if($raw.Length -gt 65536 -or $Stream.Length -ne 0){Stop-C2L 'output_cap'}
    $Stream.Write($raw,0,$raw.Length);$Stream.Flush($true)
}
function Close-PublicInputs {
    $ok=$true
    foreach($h in $held){try{$h.Dispose()}catch{$ok=$false}}
    $held.Clear();return $ok
}

function Need([bool]$Condition,[string]$Code='assertion') {if(-not $Condition){Stop-C2L $Code}}
function Case-Specs {
    return @([pscustomobject]@{Name='receipt-close';Selector='receipt-native-close';InventoryBytes=3586;InventoryHash='299750f5179b2e2905175b53212c454cd3a0602f5654f30cfc2bcb2533d01112';FailureCode='cleanup_error';FailurePhase='receipts';FailureRole='receipts';Acquired=10;FileCount=13;H='54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5';Phases=@('locks','inventory','capture_s','capture_bkp','scan_before','hash_s_before','hash_bkp_before','hash_s_after','hash_bkp_after','scan_after','final_inputs','output_binding','receipts')})
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
        foreach($key in @('colda_code','colda_phase','colda_role','rootcopies_code','c2l_code','c2l_fixture_code','c2l_fixture_operation')){
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
    Need ($Name -ceq 'receipt-close') 'fixture_scope'
    return @'
{"source":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-receipt-close-01-attempt-01\\receipt-close\\state","preserved":"E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-receipt-close-01-attempt-01\\receipt-close\\backup","classification":"private-stopped-copy-never-opened","held_lock_paths":["E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-receipt-close-01-attempt-01\\receipt-close\\state\\one\\state\\.kilogram-state.lock","E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\C-s53-cold-core-receipt-close-01-attempt-01\\receipt-close\\state\\two\\state\\.kilogram-state.lock"],"os_lock_rule":"Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.","source_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"source_after_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"held-lock length checked; empty-byte digest derived"}],"destination_files":[{"path":"one/Preservation-Inventory.json","bytes":23,"sha256":"6a25d6acd669a3abf168a4b2ecd29e1f67b86656b4db00990000e210ec5f16c3","digest_provenance":"actual file byte read"},{"path":"one/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"},{"path":"one/state/payload.bin","bytes":6,"sha256":"ba97f000c06284911e574645aa29b7b6890957382f1f7708309d443a380c40a4","digest_provenance":"actual file byte read"},{"path":"two/preservation-inventory.json","bytes":23,"sha256":"76a57d125d83975fdabd69b1b16b93b5267f282acd17834172f43710641d95db","digest_provenance":"actual file byte read"},{"path":"two/state/.kilogram-state.lock","bytes":0,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","digest_provenance":"actual file byte read"}]}
'@
}
function Initialize-C2LPremises {
    if(('S53C2LStreamsR01' -as [type]) -or ('Kilogram.S53.ColdReceiptFault.ReceiptCloseOwner' -as [type])){Stop-C2L 'core_type_cached'}
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
public sealed class S53C2LStreamsR01 {
    const string Parent=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-receipt-close-01-attempt-01";
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
    public S53C2LStreamsR01(string name,Action check) {
        string[] names={"family","receipt-close"};
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


namespace Kilogram.S53.ColdReceiptFault {
public sealed class ReceiptCloseOwner : IDisposable {
    public const string Proof = @"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-receipt-close-01-attempt-01\receipt-close\proof";
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

    [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetHandleInformation(SafeFileHandle h,uint mask,uint flags);
    [DllImport("kernel32.dll",SetLastError=true,EntryPoint="SetHandleInformation")] static extern bool SetRawFlags(IntPtr h,uint mask,uint flags);
    [DllImport("kernel32.dll",SetLastError=true,EntryPoint="GetHandleInformation")] static extern bool GetRawFlags(IntPtr h,out uint flags);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetFilePointerEx(IntPtr h,long offset,out long result,uint origin);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadFile(IntPtr h,byte[] b,uint count,out uint read,IntPtr overlapped);
    IntPtr token=IntPtr.Zero;WeakReference<SafeFileHandle> wrapperToken;Info receiptIdentity;
    bool protectedVerified,reclaimStarted,reclaimed,unprotected,tokenClosed,tokenOwned,wrapperClosedOrFinalized,ownerJsonVerified;
    int factoryCalls,protectSetCalls,protectVerifyCalls,reclaimVerifyCalls,clearCalls,clearVerifyCalls,tokenCloseCalls,tokenCloseError;
    long transferQpc,reclaimQpc,releaseQpc,receiptLength,receiptReadBytes;
    string decision,corePin;
    static bool Hash(string x){if(x==null||x.Length!=64)return false;foreach(char c in x)if(!((c>='0'&&c<='9')||(c>='a'&&c<='f')))return false;return true;}

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
    ReceiptCloseOwner(long absoluteWorkDeadline) {
        if(absoluteWorkDeadline<=Stopwatch.GetTimestamp())throw Refusal("deadline");
        deadline=absoluteWorkDeadline;
        using(var identity=WindowsIdentity.GetCurrent()) {
            if(identity.User==null)throw Refusal("owner_sid");
            user=identity.User.Value;
        }
        if(user=="S-1-5-18"||user=="S-1-5-32-544")throw Refusal("owner_sid");
    }
    // The sole binding has no path or command argument and performs no filesystem work here.
    public static ReceiptCloseOwner Create(long absoluteWorkDeadline,string decision,string core) {var o=new ReceiptCloseOwner(absoluteWorkDeadline);if(!Hash(decision)||!Hash(core))throw Refusal("binding_hash");o.decision=decision;o.corePin=core;return o;}
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
        bool ok=CloseHandle(h.DangerousGetHandle());int error=ok?0:Marshal.GetLastWin32Error();
        h.SetHandleAsInvalid();h.Dispose(); // never retry a possibly recycled native value
        if(ok)ownedHandles--;
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
        directory=OpenDirectory(Proof);AssertPrivate(Proof,true);RecheckAncestry();prepared=true;
    }
    // Caller becomes the sole owner; BindOwnedOutput consumes it on success AND refusal.
    public SafeFileHandle TakeDirectory() {
        Work();if(!prepared||taken||directory==null)throw Refusal("ownership");
        SafeFileHandle answer=directory;directory=null;taken=true;ownedHandles--;return answer;
    }
    SafeFileHandle CreateReceipt(string path) {
        Work();factoryCalls++;int role=Role(path);if(role!=0||factoryCalls!=1||!prepared||!taken)throw Refusal("path_scope");
        SafeFileHandle h=null;IntPtr sd=IntPtr.Zero;
        try {
            var sa=Security(out sd);Work();
            h=CreateSecureFile(IoPath(path),0xC0000000,0,ref sa,1,0x00200000,IntPtr.Zero);
            int error=h.IsInvalid?Marshal.GetLastWin32Error():0;
            if(h.IsInvalid)BadNative(error);ownedHandles++;
            receiptMask|=1<<role; // preserve truthful partial ownership even if validation fails
            Info info=Check(h,path,false);if(info.SizeHigh!=0||info.SizeLow!=0)throw Refusal("not_empty");
            AssertPrivate(path,false);Work();
            receiptIdentity=info;protectSetCalls++;
            bool ok=SetHandleInformation(h,2u,2u);int native=ok?0:Marshal.GetLastWin32Error();if(!ok)BadNative(native);
            uint flags;protectVerifyCalls++;ok=GetHandleInformation(h,out flags);native=ok?0:Marshal.GetLastWin32Error();
            if(!ok)BadNative(native);if(flags!=2u)throw Refusal("protected_noninherit");
            Check(h,path,false);AssertPrivate(path,false);protectedVerified=true;
            token=h.DangerousGetHandle();wrapperToken=new WeakReference<SafeFileHandle>(h,true);
            // Return is the only ownership transfer. The token/long weak reference cannot close the handle.
            SafeFileHandle answer=h;h=null;ownedHandles--;transferredReceipts++;transferQpc=Stopwatch.GetTimestamp();return answer;
        }finally{
            if(h!=null){
                // Still factory-owned on this path; remove protection before ordinary owned cleanup.
                uint f;if(!h.IsInvalid&&GetHandleInformation(h,out f)&&(f&2u)!=0){
                    bool clear=SetHandleInformation(h,2u,0u);int n=clear?0:Marshal.GetLastWin32Error();if(!clear){CleanupUnknown=true;CleanupNativeError=n;}
                }
                CloseOwned(h);
            }
            FreeDescriptor(sd);
        }
    }
    void VerifyExactAcl(string path) {
        Work();int role=Role(path);if(!DirectoryCreated)throw Refusal("ownership");AssertPrivate(path,role<0);
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

    static void Shape(System.Text.Json.JsonElement e,string[] fields){
        if(e.ValueKind!=System.Text.Json.JsonValueKind.Object)throw Refusal("owner_schema");
        var seen=new HashSet<string>(StringComparer.Ordinal);
        foreach(var p in e.EnumerateObject())if(Array.IndexOf(fields,p.Name)<0||!seen.Add(p.Name))throw Refusal("owner_schema");
        if(seen.Count!=fields.Length)throw Refusal("owner_schema");
    }
    void VerifyOwner(byte[] raw){
        if(raw.Length<2||raw[0]!=123||raw[raw.Length-1]!=10||raw[raw.Length-2]!=125)throw Refusal("owner_frame");
        for(int i=0;i<raw.Length-1;i++)if(raw[i]==10||raw[i]==13)throw Refusal("owner_frame");
        using(var doc=System.Text.Json.JsonDocument.Parse(new ReadOnlyMemory<byte>(raw,0,raw.Length-1))){
            var e=doc.RootElement;
            Shape(e,new[]{"classification","attempt","decision_sha256","core_sha256","activation_and_new_e_ownership_verified_by_core","external_guardian_required",
                "core_start_timestamp","work_deadline_timestamp","source","preserved","output","inventory_bytes","inventory_sha256","historical_inventory_validated","expected_files","expected_bytes","locks"});
            const string root=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-receipt-close-01-attempt-01\receipt-close";
            if(e.GetProperty("classification").GetString()!="cold-a-private-owner-r44"||e.GetProperty("attempt").GetString()!="C-s53-cold-core-receipt-close-01-attempt-01/receipt-close"||
                e.GetProperty("decision_sha256").GetString()!=decision||e.GetProperty("core_sha256").GetString()!=corePin||
                e.GetProperty("activation_and_new_e_ownership_verified_by_core").GetBoolean()||!e.GetProperty("external_guardian_required").GetBoolean()||
                e.GetProperty("core_start_timestamp").GetInt64()<=0||e.GetProperty("core_start_timestamp").GetInt64()>=deadline||
                e.GetProperty("work_deadline_timestamp").GetInt64()!=deadline||e.GetProperty("source").GetString()!=root+@"\state"||
                e.GetProperty("preserved").GetString()!=root+@"\backup"||e.GetProperty("output").GetString()!=Proof||
                e.GetProperty("inventory_bytes").GetInt64()!=3586||e.GetProperty("inventory_sha256").GetString()!="299750f5179b2e2905175b53212c454cd3a0602f5654f30cfc2bcb2533d01112"||
                !e.GetProperty("historical_inventory_validated").GetBoolean()||e.GetProperty("expected_files").GetInt32()!=5||e.GetProperty("expected_bytes").GetInt64()!=52)throw Refusal("owner_binding");
            string[] names={@"\origin\one.state\.kilogram-state.lock",@"\origin\two.state\.kilogram-state.lock",@"\state\one\state\.kilogram-state.lock",
                @"\state\two\state\.kilogram-state.lock",@"\backup\one\state\.kilogram-state.lock",@"\backup\two\state\.kilogram-state.lock"};
            var locks=e.GetProperty("locks");if(locks.GetArrayLength()!=6)throw Refusal("owner_locks");
            for(int i=0;i<6;i++){var l=locks[i];Shape(l,new[]{"path","verified_empty","provenance","read_bytes","empty_hash_derived","initial"});
                if(l.GetProperty("path").GetString()!=root+names[i]||!l.GetProperty("verified_empty").GetBoolean()||
                    l.GetProperty("provenance").GetString()!="verified-empty-held-handle"||l.GetProperty("read_bytes").GetInt64()!=0||!l.GetProperty("empty_hash_derived").GetBoolean())throw Refusal("owner_locks");
                var st=l.GetProperty("initial");Shape(st,new[]{"volume","file_id","attributes","bytes","links","created_high","created_low","written_high","written_low"});
                uint attr=st.GetProperty("attributes").GetUInt32();if((attr&(16u|64u|1024u))!=0||st.GetProperty("bytes").GetInt64()!=0||st.GetProperty("links").GetUInt32()!=1)throw Refusal("owner_stamp");
                st.GetProperty("volume").GetUInt32();string id=st.GetProperty("file_id").GetString();if(id==null||id.Length!=16)throw Refusal("owner_stamp");
                foreach(char c in id)if(!((c>='0'&&c<='9')||(c>='a'&&c<='f')))throw Refusal("owner_stamp");
                foreach(string key in new[]{"created_high","created_low","written_high","written_low"})st.GetProperty(key).GetInt32();
            }
        }
        ownerJsonVerified=true;
    }
    void ReleaseToken(){
        if(!tokenOwned||tokenClosed)return;
        if(clearCalls!=0||tokenCloseCalls!=0){CleanupUnknown=true;return;}
        clearCalls++;bool ok=SetRawFlags(token,2u,0u);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok){CleanupUnknown=true;CleanupNativeError=error;return;}
        uint f;clearVerifyCalls++;ok=GetRawFlags(token,out f);error=ok?0:Marshal.GetLastWin32Error();
        if(!ok||f!=0u){CleanupUnknown=true;CleanupNativeError=error;return;}unprotected=true;
        tokenCloseCalls++;ok=CloseHandle(token);error=ok?0:Marshal.GetLastWin32Error();tokenCloseError=error;
        if(ok){tokenClosed=true;tokenOwned=false;ownedHandles--;releaseQpc=Stopwatch.GetTimestamp();token=IntPtr.Zero;}
        else{CleanupUnknown=true;CleanupNativeError=error;}
    }
    // Caller must have validated the exact Core fault, recorded snapshots, and returned from Core.Dispose.
    public void ReclaimAfterCoreDisposed(long expectedBytes){
        Work();if(reclaimStarted||!protectedVerified||transferredReceipts!=1||token==IntPtr.Zero||expectedBytes<=0||expectedBytes>65536)throw Refusal("reclaim_boundary");
        reclaimStarted=true;reclaimQpc=Stopwatch.GetTimestamp();
        SafeFileHandle oldWrapper;
        // A long weak token does not own/dispose the SafeHandle; null means finalized/collected, not pending finalization.
        if(wrapperToken.TryGetTarget(out oldWrapper)){if(!oldWrapper.IsClosed)throw Refusal("core_wrapper_still_open");}
        wrapperClosedOrFinalized=true;oldWrapper=null;
        uint flags;reclaimVerifyCalls++;bool ok=GetRawFlags(token,out flags);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)BadNative(error);if(flags!=2u)throw Refusal("protection_lifetime");
        using(var view=new SafeFileHandle(token,false)){
            Info now=Check(view,Proof+@"\owner.private.json",false);AssertPrivate(Proof+@"\owner.private.json",false);
            if(now.Volume!=receiptIdentity.Volume||now.IndexHigh!=receiptIdentity.IndexHigh||now.IndexLow!=receiptIdentity.IndexLow||
                now.SizeHigh!=0||now.SizeLow!=expectedBytes)throw Refusal("receipt_identity_or_length");
        }
        // Core no longer has a live owning wrapper. The still-protected same object is explicitly reclaimed.
        reclaimed=true;tokenOwned=true;ownedHandles++;receiptLength=expectedBytes;
        try{
            long position;ok=SetFilePointerEx(token,0,out position,0);error=ok?0:Marshal.GetLastWin32Error();if(!ok)BadNative(error);if(position!=0)throw Refusal("receipt_seek");
            byte[] raw=new byte[(int)expectedBytes],chunk=new byte[(int)expectedBytes];int offset=0;
            while(offset<raw.Length){Work();uint n;ok=ReadFile(token,chunk,(uint)(raw.Length-offset),out n,IntPtr.Zero);error=ok?0:Marshal.GetLastWin32Error();
                if(!ok)BadNative(error);if(n==0||n>raw.Length-offset)throw Refusal("receipt_read");
                Buffer.BlockCopy(chunk,0,raw,offset,(int)n);offset+=(int)n;receiptReadBytes+=n;}
            byte[] eof=new byte[1];uint extra;ok=ReadFile(token,eof,1,out extra,IntPtr.Zero);error=ok?0:Marshal.GetLastWin32Error();
            if(!ok)BadNative(error);if(extra!=0)throw Refusal("receipt_eof");VerifyOwner(raw);
        }finally{ReleaseToken();}
        if(!tokenClosed||CleanupUnknown)throw Refusal("fixture_cleanup");
        RecheckAncestry();
    }
    public Dictionary<string,object> Snapshot(){return new Dictionary<string,object>{
        {"directory_created",DirectoryCreated},{"directory_transferred",taken},{"ancestry_count",ancestors.Count},{"factory_calls",factoryCalls},
        {"receipt_created_mask",receiptMask},{"receipts_transferred",transferredReceipts},{"protect_set_calls",protectSetCalls},{"protect_verify_calls",protectVerifyCalls},
        {"protected_verified",protectedVerified},{"wrapper_closed_or_finalized",wrapperClosedOrFinalized},{"reclaim_started",reclaimStarted},{"reclaimed",reclaimed},
        {"reclaim_verify_calls",reclaimVerifyCalls},{"clear_calls",clearCalls},{"clear_verify_calls",clearVerifyCalls},{"unprotected",unprotected},
        {"token_close_calls",tokenCloseCalls},{"token_close_error",tokenCloseError},{"token_closed",tokenClosed},{"token_owned",tokenOwned},
        {"owner_json_verified",ownerJsonVerified},{"receipt_length",receiptLength},{"receipt_read_bytes",receiptReadBytes},
        {"transfer_qpc",transferQpc},{"reclaim_qpc",reclaimQpc},{"release_qpc",releaseQpc},{"remaining_owned_handles",ownedHandles},
        {"cleanup_unknown",CleanupUnknown},{"cleanup_native_error",CleanupNativeError},{"disposed",disposed}
    };}

    public void Dispose() {
        if(disposed)return;
        if(tokenOwned&&!tokenClosed)ReleaseToken();
        if(token!=IntPtr.Zero&&!tokenClosed)CleanupUnknown=true;
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
    $chain=[Collections.Generic.List[object]]::new();$at=$Exception
    while($null -ne $at){
        Need ($chain.Count -lt 16) 'typed_refusal'
        foreach($key in @('c2l_code','c2l_fixture_code','c2l_premise_code','rootcopies_code','colda_native_error')){
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
function Assert-CoreSnapshot($o,$s,[string]$Stage,[long]$Bytes=0,[int]$CleanupError=0) {
    Need ($Stage -cin @('capture','bound','fault','disposed'))
    $fault=$Stage -cin @('fault','disposed');$bound=$Stage -ceq 'bound'
    $expected=[ordered]@{property='bounded_payload_equality_cooperative_owner';whole_attempt_verdict='not_established';continuous_namespace_absence='not_established'
        completed=$true;expected_set_captured_before_hash=$true;observed_entry_sets_equal=$true;input_handles_released=$true
        cleanup_unknown=$fault;receipts_completed=$false;cleanup_native_error=if($fault){$CleanupError}else{0}
        failure_code=if($fault){'cleanup_error'}else{$null};failure_phase=if($fault){'receipts'}else{$null};failure_role=if($fault){'receipts'}else{$null}
        cause_kind=if($fault){'bounded_refusal'}else{$null};native_error=0;error_hresult=if($fault){-2146233079}else{0}
        expected_files=5;expected_bytes=52;locks=6;hash_passes=4;hash_bytes=208
        historical_tuple_digest=$s.H;s_before_tuple_digest=$s.H;bkp_before_tuple_digest=$s.H;s_after_tuple_digest=$s.H;bkp_after_tuple_digest=$s.H
        peak_owned_handles=35;remaining_owned_handles=if($fault -or $bound){1}else{0}
        peak_path_bytes=7080;peak_record_bytes=12087312;peak_inventory_bytes=$s.InventoryBytes;scratch_reserved_bytes=524288
        receipt_bytes_charged=if($fault){$Bytes}else{0};receipt_bytes_known_written=if($fault){$Bytes}else{0};receipt_write_unknown=$false}
    Assert-Fields $o $expected $false;Need (@(Object-Keys $o).Count -eq ($expected.Count+3))
    $capture=Get-Value $o 'capture_complete_timestamp';Need (($capture -is [int] -or $capture -is [long]) -and $capture -gt 0)
    $keys=@('allocation')+@($s.Phases[0..10])
    if($bound -or $fault){$keys+=@('output_binding')};if($fault){$keys+=@('receipts');Need ($Bytes -gt 0 -and $Bytes -le 65536 -and $CleanupError -ge 0)}
    $actualKeys=@(Object-Keys (Get-Value $o 'phase_seconds'));Need ($actualKeys.Count -eq $keys.Count -and ($actualKeys -join '|') -ceq ($keys -join '|'))
    foreach($key in $keys){$v=Get-Value (Get-Value $o 'phase_seconds') $key;Need (($v -is [double] -or $v -is [long] -or $v -is [int] -or $v -is [decimal]) -and [double]::IsFinite([double]$v) -and $v -ge 0 -and $v -lt 20)}
    Need (@(Object-Keys (Get-Value $o 'private_receipt_digests')).Count -eq 0)
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


function Assert-CaseRecord($r,$s,[long]$Deadline) {
    Assert-Fields $r ([ordered]@{name=$s.Name;selector=$s.Selector;result='observed';failure=$null;failure_cause=$null;failure_stage=$null;cleanup_first_cause=$null
        core_factory_invoked=$true;core_capture_invoked=$true;core_dispose_attempted=$true;core_disposed=$true;core_success_returned=$true;core_exception_observed=$true
        output_bound=$true;output_write_invoked=$true;output_success_returned=$false;expected_core_cleanup_unknown=$true;fixture_cleanup_known=$true;core_history_unchanged_after_fixture_cleanup=$true
        controller_directories_closed=$true;remaining_controller_directories=0;unreleased_current_directories=0;controller_cleanup_unknown=$false;acquired_files=10
        hook_failure=$null;proof_created=$true;output_owner_bound=$true;receipt_count=1}) $false
    Assert-SetupSnapshot $r.setup $s
    Assert-PremiseSnapshot $r.premises.baseline $s $false;Assert-PremiseSnapshot $r.premises.prepared $s $true
    Assert-Fields $r.premises ([ordered]@{ready=$true;directories=15;files=13;physical_file_bytes=(104+$s.InventoryBytes);named_stream_bytes=0;streams_verified=56}) $false
    Assert-StreamSnapshot $r.baseline_stream_check 15 13;Assert-StreamSnapshot $r.prepared_strict_stream_check 15 13;Assert-GuardHandoff $r.guard_handoff $s
    $n=$r.receipt_bytes;$errorCode=$r.cleanup_native_error
    Need (($n -is [int] -or $n -is [long]) -and $n -gt 0 -and $n -le 65536 -and ($errorCode -is [int] -or $errorCode -is [long]) -and $errorCode -ge 0)
    Assert-CoreSnapshot $r.capture_observation $s 'capture'
    Assert-CoreSnapshot $r.before_write_observation $s 'bound'
    Assert-CoreSnapshot $r.core_observation $s 'fault' $n $errorCode
    Assert-CoreSnapshot $r.after_dispose_observation $s 'disposed' $n $errorCode
    Validate-CoreError $r.core_refusal $r.primary_core_cause $s
    Assert-OutputSnapshot $r.output_prepared 0 'prepared';Assert-OutputSnapshot $r.output_bound_snapshot 0 'bound'
    Assert-OutputSnapshot $r.output_at_fault 0 'fault';Assert-OutputSnapshot $r.output_after_core_dispose 0 'disposed'
    Assert-OutputSnapshot $r.output_helper $n
    Need ($r.output_at_fault.transfer_qpc -eq $r.output_helper.transfer_qpc -and $r.output_after_core_dispose.transfer_qpc -eq $r.output_helper.transfer_qpc)
    $capture=$r.capture_observation.capture_complete_timestamp
    Need ($capture -gt $r.factory_after_qpc -and $capture -le $r.capture_return_qpc -and $capture -eq $r.capture_qpc -and $capture -eq $r.before_write_observation.capture_complete_timestamp -and
        $capture -eq $r.core_observation.capture_complete_timestamp -and $capture -eq $r.after_dispose_observation.capture_complete_timestamp)
    $times=@($r.setup_started_qpc,$r.baseline_complete_qpc,$r.prepared_complete_qpc,$r.guards_closed_qpc,$r.factory_before_qpc,$r.factory_after_qpc,
        $r.capture_return_qpc,$r.before_write_qpc,$r.output_helper.transfer_qpc,$r.core_failure_qpc,$r.core_disposed_qpc,$r.output_helper.reclaim_qpc,$r.output_helper.release_qpc,$r.fixture_disposed_qpc)
    $last=$StartTimestamp;foreach($time in $times){Need (($time -is [long] -or $time -is [int]) -and $time -ge $last -and $time -lt $Deadline);$last=$time}
    Need ($r.hook_events.Count -eq $s.Phases.Count);for($i=0;$i -lt $s.Phases.Count;$i++){Need ($r.hook_events[$i].name -ceq $s.Phases[$i]);$q=$r.hook_events[$i].qpc;Need (($q -is [long] -or $q -is [int]) -and $q -ge $r.factory_after_qpc -and $q -le $r.core_failure_qpc);if($i -gt 0){Need ($q -ge $r.hook_events[$i-1].qpc)}}
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



function Bind-ReceiptFaultOutput {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][object]$Core,
        [Parameter(Mandatory)][object]$Owner,
        [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$DecisionSha256,
        [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$CoreSha256
    )
    if($Core.GetType().FullName -cne 'Kilogram.S53.ColdA.Core' -or
       $Owner.GetType().FullName -cne 'Kilogram.S53.ColdReceiptFault.ReceiptCloseOwner') { throw 'Cold output binding types.' }
    $method=$Core.GetType().GetMethod('BindOwnedOutput',[Reflection.BindingFlags]'Public,Instance')
    if(-not $method -or $method.GetParameters().Count -ne 5) { throw 'Cold output binding signature.' }
    $creator=$Owner.Creator
    $verifier=$Owner.AclVerifier
    $parameters=$method.GetParameters()
    if($parameters[0].ParameterType -ne [Microsoft.Win32.SafeHandles.SafeFileHandle] -or
       $parameters[1].ParameterType -ne $creator.GetType() -or $parameters[2].ParameterType -ne $verifier.GetType() -or
       $parameters[3].ParameterType -ne [string] -or $parameters[4].ParameterType -ne [string]) { throw 'Cold output binding signature.' }
    $pending=$Owner.TakeDirectory()
    $consumed=$false
    try {
        $arguments=[object[]]@($pending,$creator,$verifier,$DecisionSha256,$CoreSha256)
        try {
            $null=$method.Invoke($Core,$arguments)
            $consumed=$true
        } catch {
            # PowerShell can wrap MethodInfo.Invoke's TargetInvocationException.
            for($at=$_.Exception;$null -ne $at;$at=$at.InnerException) {
                if($at -is [Reflection.TargetInvocationException]) { $consumed=$true;break }
            }
            throw
        }
    } finally {
        # Reflection/binding failure before method entry leaves ownership with this scope.
        # Such a failure is never a passing case; disposal here is best-effort managed cleanup.
        if(-not $consumed -and $pending) { $pending.Dispose() }
        $pending=$null
    }
}

function Assert-OutputSnapshot($h,[long]$Bytes=0,[string]$Stage='final') {
    Need ($Stage -cin @('prepared','bound','fault','disposed','final'))
    $final=$Stage -ceq 'final';$receipt=$Stage -cin @('fault','disposed','final');$taken=$Stage -cne 'prepared'
    Assert-Fields $h ([ordered]@{directory_created=$true;directory_transferred=$taken;ancestry_count=6;factory_calls=if($receipt){1}else{0}
        receipt_created_mask=if($receipt){1}else{0};receipts_transferred=if($receipt){1}else{0}
        protect_set_calls=if($receipt){1}else{0};protect_verify_calls=if($receipt){1}else{0};protected_verified=$receipt
        wrapper_closed_or_finalized=$final;reclaim_started=$final;reclaimed=$final;reclaim_verify_calls=if($final){1}else{0}
        clear_calls=if($final){1}else{0};clear_verify_calls=if($final){1}else{0};unprotected=$final
        token_close_calls=if($final){1}else{0};token_close_error=0;token_closed=$final;token_owned=$false
        owner_json_verified=$final;receipt_length=if($final){$Bytes}else{0};receipt_read_bytes=if($final){$Bytes}else{0}
        remaining_owned_handles=if($final){0}elseif($taken){6}else{7};cleanup_unknown=$false;cleanup_native_error=0;disposed=$final}) $false
    Need (@(Object-Keys $h).Count -eq 30)
    foreach($key in @('transfer_qpc','reclaim_qpc','release_qpc')){
        $v=Get-Value $h $key;Need (($v -is [long] -or $v -is [int]) -and $v -ge 0)
        if(($key -ceq 'transfer_qpc' -and $receipt) -or $final){Need ($v -gt 0)}else{Need ($v -eq 0)}
    }
    if($final){Need ($h.transfer_qpc -le $h.reclaim_qpc -and $h.reclaim_qpc -le $h.release_qpc)}
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
    $outputOwner=$null;$outputDisposed=$false;$outputDisposeAttempted=$false;$outputBound=$false;$writeInvoked=$false;$writeReturned=$false
    $capturePublic=$null;$boundPublic=$null;$faultPublic=$null;$disposedPublic=$null;$disposedRaw=$null;$outputFacts=$null
    $outputPrepared=$null;$outputBoundFacts=$null;$outputFault=$null;$outputCoreDisposed=$null
    $receiptBytes=0L;$cleanupError=0;$captureReturn=0L;$beforeWrite=0L;$coreFailure=0L;$coreDisposedQpc=0L;$fixtureDisposedQpc=0L
    $caseFailure=$null;$caseCause=$null;$primaryCoreCause=$null;$typed=$null;$corePublic=$null;$stage='setup';$recordComplete=$false
    $baseline=$null;$prepared=$null;$premises=$null;$guardHandoff=$null;$cleanupOk=$false;$cleanupFirstCause=$null
    $caughtCore=$null;$factoryBefore=0L;$factoryAfter=0L;$captureInvoked=$false;$coreReturned=$false
    $setupStart=[Diagnostics.Stopwatch]::GetTimestamp();$baselineDone=0L;$preparedDone=0L;$guardsDone=0L
    try{
        Check;$Record.result='started';$root=$syntheticParent+'\'+$Spec.Name
        $preStreams=[S53C2LStreamsR01]::new($Spec.Name,[Action]{Check})
        $postStreams=[S53C2LStreamsR01]::new($Spec.Name,[Action]{Check})
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
        $stage='output_prepare'
        $outputOwner=[Kilogram.S53.ColdReceiptFault.ReceiptCloseOwner]::Create($absoluteWorkDeadline,$AllocationSha256,$binding.Pins.core)
        $outputOwner.PrepareDirectory();$outputPrepared=$outputOwner.Snapshot();Assert-OutputSnapshot $outputPrepared 0 'prepared'
        $stage='output_bind'
        Bind-ReceiptFaultOutput -Core $coreObject -Owner $outputOwner -DecisionSha256 $AllocationSha256 -CoreSha256 $binding.Pins.core
        $outputBound=$true;$outputBoundFacts=$outputOwner.Snapshot();Assert-OutputSnapshot $outputBoundFacts 0 'bound'
        $boundPublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        Assert-CoreSnapshot $boundPublic $Spec 'bound'
        $stage='receipt_write';$beforeWrite=[Diagnostics.Stopwatch]::GetTimestamp();$writeInvoked=$true
        try{$coreObject.WriteReceipts();$writeReturned=$true}
        catch{$caughtCore=$_.Exception;$primaryCoreCause=Cause $_.Exception;$coreFailure=[Diagnostics.Stopwatch]::GetTimestamp()}
        $stage='core_result'
        Need (-not $writeReturned -and $null -ne $caughtCore -and $null -eq $hookFailure -and
            $caseAcquired -eq $Spec.Acquired -and $caseHookIndex -eq $Spec.Phases.Count) 'typed_refusal'
        $typed=Core-Error $caughtCore $Spec;Validate-CoreError $typed $primaryCoreCause $Spec
        $faultPublic=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())|ConvertFrom-Json
        $receiptBytes=[long]$faultPublic.receipt_bytes_charged;$cleanupError=[int]$faultPublic.cleanup_native_error
        Assert-CoreSnapshot $faultPublic $Spec 'fault' $receiptBytes $cleanupError
        $outputFault=$outputOwner.Snapshot();Assert-OutputSnapshot $outputFault 0 'fault'
        $stage='core_dispose';$coreDisposeAttempted=$true;$coreObject.Dispose();$coreDisposed=$true
        $coreDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()
        $disposedRaw=[Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8())
        $disposedPublic=$disposedRaw|ConvertFrom-Json
        Assert-CoreSnapshot $disposedPublic $Spec 'disposed' $receiptBytes $cleanupError
        $outputCoreDisposed=$outputOwner.Snapshot();Assert-OutputSnapshot $outputCoreDisposed 0 'disposed'
        $expectedFault=$true
        # This is the sole reclaim callsite: the precise refusal is established and Core.Dispose has returned.
        $stage='fixture_reclaim';$outputOwner.ReclaimAfterCoreDisposed($receiptBytes)
        $outputDisposeAttempted=$true;$outputOwner.Dispose();$outputDisposed=$true
        $fixtureDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp();$outputFacts=$outputOwner.Snapshot()
        Assert-OutputSnapshot $outputFacts $receiptBytes
        Need ([Text.Encoding]::UTF8.GetString($coreObject.PublicObservationUtf8()) -ceq $disposedRaw)
        $coreHistoryUnchanged=$true;$fixtureKnown=$true
        Check;$recordComplete=$true
    }catch{$caseFailure=Failure-Code $_.Exception;$caseCause=Cause $_.Exception}
    finally{
        if($coreObject -and -not $coreDisposeAttempted){
            $coreDisposeAttempted=$true
            try{$coreObject.Dispose();$coreDisposed=$true;$coreDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
            catch{$coreUnknown=$true;$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        if($outputOwner -and -not $outputDisposeAttempted){
            $outputDisposeAttempted=$true
            try{$outputOwner.Dispose();$outputDisposed=$true;$fixtureDisposedQpc=[Diagnostics.Stopwatch]::GetTimestamp()}
            catch{$script:controllerCleanupUnknown=$true;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        if($outputOwner){
            try{
                $outputFacts=$outputOwner.Snapshot()
                if(-not $outputDisposed -or $outputOwner.CleanupUnknown -or $outputOwner.RemainingOwnedHandles -ne 0){
                    $script:controllerCleanupUnknown=$true;Retain-UnknownOwner $outputOwner
                }
            }catch{$script:controllerCleanupUnknown=$true;Retain-UnknownOwner $outputOwner;if(-not $cleanupFirstCause){$cleanupFirstCause=Cause $_.Exception}}
        }
        if($coreObject){
            try{
                $facts=$coreObject.GetObservation()
                # The historical Core slot is accepted only with this fully validated fault and separately known fixture cleanup.
                $allowHistorical=$expectedFault -and $fixtureKnown -and $coreHistoryUnchanged -and $coreDisposed -and
                    $facts.CleanupUnknown -and $facts.RemainingOwnedHandles -eq 1 -and $facts.InputHandlesReleased
                if(($facts.CleanupUnknown -or $facts.RemainingOwnedHandles -ne 0 -or -not $facts.InputHandlesReleased) -and -not $allowHistorical){
                    $coreUnknown=$true;$script:controllerCleanupUnknown=$true
                }
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
        $Record.core_exception_observed=($null -ne $caughtCore);$Record.core_success_returned=$coreReturned
        $Record.capture_observation=$capturePublic;$Record.before_write_observation=$boundPublic
        $Record.core_observation=$faultPublic;$Record.after_dispose_observation=$disposedPublic
        $Record.expected_core_cleanup_unknown=$expectedFault;$Record.fixture_cleanup_known=$fixtureKnown
        $Record.core_history_unchanged_after_fixture_cleanup=$coreHistoryUnchanged
        $Record.output_helper=$outputFacts;$Record.output_prepared=$outputPrepared;$Record.output_bound_snapshot=$outputBoundFacts
        $Record.output_at_fault=$outputFault;$Record.output_after_core_dispose=$outputCoreDisposed;$Record.receipt_bytes=$receiptBytes;$Record.cleanup_native_error=$cleanupError
        $Record.output_bound=$outputBound;$Record.output_write_invoked=$writeInvoked;$Record.output_success_returned=$writeReturned
        $Record.capture_return_qpc=$captureReturn;$Record.before_write_qpc=$beforeWrite;$Record.core_failure_qpc=$coreFailure
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
        $Record.output_owner_bound=$outputBound;$Record.proof_created=($null -ne $outputFacts -and $outputFacts.directory_created)
        $Record.receipt_count=if($outputFacts){$outputFacts.receipts_transferred}else{0}
        $Record.result=if($recordComplete -and -not $caseFailure -and -not $controllerCleanupUnknown -and $cleanupOk -and $coreDisposed -and $expectedFault -and $fixtureKnown -and $coreHistoryUnchanged){'observed'}else{'failed_or_unknown'}
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
$syntheticParent=$w+'\.tmp\C-s53-cold-core-receipt-close-01-attempt-01';$syntheticParentCreated=$false
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
    if($diagnostics.Count -ge 16){$script:diagnosticCap=$true;Stop-C2L 'diagnostic_cap'}
    $available=[Math]::Max(0,6144-$script:diagnosticChars);$take=[Math]::Min(2048,[Math]::Min($available,$message.Length))
    $diagnostics.Add([ordered]@{kind=$Kind;message=$message.Substring(0,$take);truncated=($take -ne $message.Length)})
    $script:diagnosticChars+=$take
    if($take -ne $message.Length){$script:diagnosticCap=$true;Stop-C2L 'diagnostic_cap'}
}

try {
    [Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
    if($StartTimestamp -le 0 -or $PSCommandPath -cne $preflight){Stop-C2L 'allocation_scope'}
    $binding=Validate-Allocation
    $started=Open-Public "$e\$attempt.started.json" 65536
    $claim=[Text.UTF8Encoding]::new($false,$true).GetString((Bytes $started 65536))|ConvertFrom-Json
    if($claim.classification -cne 'cold-core-receipt-close-c2-start-r01' -or $claim.attempt -cne $attempt -or
        $claim.allocation_sha256 -cne $AllocationSha256 -or $claim.start_timestamp -ne $StartTimestamp -or
        $claim.stopwatch_frequency -ne [Diagnostics.Stopwatch]::Frequency -or $claim.permission_consumed -ne $true -or
        $claim.work_seconds -ne 20 -or $claim.whole_seconds -ne 30 -or $claim.retry_count -ne 0 -or $claim.lane_count -ne 1){Stop-C2L 'start_claim'}
    $reportFile=New-PublicOutput 'cases.json'
    if((Age) -ge 18){Stop-C2L 'bootstrap_reserve'}
    Set-Phase 'job_setup'
    $typeAbsent.deadline=$null -eq ('KilogramS5Deadline' -as [type])
    Need $typeAbsent.deadline 'core_type_cached'
    . $deadline
    Check;$guard=[KilogramS5Deadline]::new((20-(Age)),(27-(Age)));Check
    Set-Phase 'definitions'
    $tokens=$null;$parseErrors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($core,[ref]$tokens,[ref]$parseErrors)
    $top=@($ast.EndBlock.Statements)
    if($parseErrors.Count -ne 0 -or $top.Count -ne 3 -or
        @($top|Where-Object{$_ -isnot [Management.Automation.Language.FunctionDefinitionAst]}).Count -ne 0){Stop-C2L 'definitions_shape'}
    foreach($key in @('core','root_native','deadline')){$sourceBefore[$key]=Hash-Stream $binding.SourceFiles[$key]}
    $typeNames=[ordered]@{core='Kilogram.S53.ColdA.Core';root_native='S53RootNativeR38';premise_streams='S53C2LStreamsR01';output_fault_owner='Kilogram.S53.ColdReceiptFault.ReceiptCloseOwner'}
    foreach($key in $typeNames.Keys){$typeAbsent[$key]=$null -eq ($typeNames[$key] -as [type]);Need $typeAbsent[$key] 'core_type_cached'}
    . $core
    . $rootNative
    Set-Phase 'compilation'
    foreach($initializer in @('Initialize-S53ColdPreservationCore','Initialize-S53RootNative','Initialize-C2LPremises')){
        $initializeCalls[$initializer]=1
        try {
            & $initializer *>&1 | ForEach-Object {
                Check
                if($_ -is [Management.Automation.ErrorRecord]){$script:compilerError=$true;Diagnostic $_ 'error'}
                elseif($_ -is [Management.Automation.WarningRecord]){Diagnostic $_ 'warning'}
                else{Diagnostic $_ 'output'}
            }
        }catch{$compilerError=$true;if(-not $diagnosticCap){Diagnostic $_ 'terminating_error'};Stop-C2L 'compiler_error'}
        if($compilerError){Stop-C2L 'compiler_error'}
    }
    foreach($key in $typeNames.Keys){Need ($null -ne ($typeNames[$key] -as [type])) 'signature_mismatch'}
    Set-Phase 'family_setup';$fixtureAccess=$true
    $familyStreams=[S53C2LStreamsR01]::new('family',[Action]{Check})
    New-Family
    for($caseIndex=0;$caseIndex -lt 1;$caseIndex++){
        Set-Phase $caseSpecs[$caseIndex].Name
        Run-Case $caseSpecs[$caseIndex] $records[$caseIndex]
    }
    $geometry=[ordered]@{directories=1;files=0;stream_checks=1;logical_file_bytes=0;named_stream_bytes=0;all_logical_stream_bytes=0}
    foreach($record in $records){
        $geometry.directories+=($record.premises.directories+1);$geometry.files+=($record.premises.files+1)
        $geometry.stream_checks+=$record.premises.streams_verified;$geometry.logical_file_bytes+=($record.premises.physical_file_bytes+$record.receipt_bytes)
        $geometry.named_stream_bytes+=$record.premises.named_stream_bytes
    }
    $geometry.all_logical_stream_bytes=$geometry.logical_file_bytes+$geometry.named_stream_bytes
    Need ($geometry.directories -eq 17 -and $geometry.files -eq 14 -and
        $geometry.stream_checks -eq 57 -and $geometry.logical_file_bytes -eq (3690+$records[0].receipt_bytes) -and $geometry.named_stream_bytes -eq 0 -and $geometry.all_logical_stream_bytes -eq (3690+$records[0].receipt_bytes))
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
    if((Age) -ge 25){Stop-C2L 'work_expired'}
    if($reportFile){
        $phaseTimes[$phase]=([Diagnostics.Stopwatch]::GetTimestamp()-$phaseStart)/[double][Diagnostics.Stopwatch]::Frequency
        Write-Public $reportFile ([ordered]@{
            classification='cold-core-receipt-close-c2-r01';attempt=$attempt;allocation_sha256=$AllocationSha256
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
            output_owner_bound=$records[0].output_owner_bound;proof_created=$records[0].proof_created;receipt_count=$records[0].receipt_count;receipt_bytes=$records[0].receipt_bytes;receipt_digests=@{}
            owned_children_after=$childrenAfter;managed_inputs_disposed=$managedDisposed;job_finish_requires_actual_exit_zero=$true
            observed_seconds=(Age);work_seconds=20;whole_seconds=30;retry_count=0;lane_count=1
            synthetic_fixture_access=$fixtureAccess;private_dataset_access=$false;native_false_close_path_tested=($records[0].result -ceq 'observed')
        })
        $reportFile.Dispose();$reportFile=$null
    }
    if((Age) -ge 20){$result=1}
    if($guard){$guard.Finish();$jobFinished=$true}
    if(-not $jobFinished){$result=1}
}catch{$result=1}
finally{if($reportFile){try{$reportFile.Dispose()}catch{$result=1}}}
exit $result
