[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('\AC-s53-cold-b0b-[a-z0-9-]{1,64}\z')][string]$Attempt,
    [switch]$SyntheticEarlyEof
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
# Synthetic EOF fault happens before any native handle acquisition.
if($SyntheticEarlyEof){exit 23}
$sourceHash=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
Add-Type -Language CSharp -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.ComponentModel;

public static partial class S53TypedLockWorker {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes,CreateLow,CreateHigh,AccessLow,AccessHigh,WriteLow,WriteHigh,Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Entry {
        public uint Size,Usage,Pid; public UIntPtr Heap; public uint Module,Threads,Parent; public int Priority; public uint Flags;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=260)] public string Exe;
    }
    [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags,uint pid);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Process32FirstW(IntPtr snapshot,ref Entry entry);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Process32NextW(IntPtr snapshot,ref Entry entry);
    [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetProcessTimes(IntPtr process,out long create,out long exit,out long kernel,out long user);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool DuplicateHandle(IntPtr sourceProcess,IntPtr source,IntPtr targetProcess,out IntPtr target,uint access,bool inherit,uint options);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(IntPtr handle,out Info info);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint GetFileType(IntPtr handle);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetFinalPathNameByHandleW(IntPtr handle,StringBuilder path,uint size,uint flags);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetHandleInformation(IntPtr handle,out uint flags);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadFile(IntPtr handle,byte[] buffer,uint bytes,out uint read,IntPtr overlapped);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool WriteFile(IntPtr handle,byte[] buffer,uint bytes,out uint written,IntPtr overlapped);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr handle);

    sealed class Refusal : Exception {
        internal readonly string Code;
        internal readonly int Native;
        internal Refusal(string code,int native=0){Code=code;Native=native;}
    }
    sealed partial class Session {
        readonly string source,attempt;
        readonly Stream input=Console.OpenStandardInput(),output=Console.OpenStandardOutput();
        readonly IntPtr[] held=new IntPtr[2];
        readonly byte[] scratch=new byte[4096];
        int frames,created,closed,reads,writes;
        string nonce="",phase="bind";
        bool cleanup=true,snapshotClosed=true;
        IntPtr parent;
        uint parentPid;
        readonly string[] boundPaths=new string[2];
        JsonElement preservation;
        internal Session(string hash,string name){source=hash;attempt=name;}
        static void Need(bool ok,string code){if(!ok)throw new Refusal(code);}
        static void Native(bool ok){if(!ok)throw new Refusal("native",Marshal.GetLastWin32Error());}
        static bool Hex(string s,int n){
            if(s==null||s.Length!=n)return false;
            foreach(char c in s)if(!((c>='0'&&c<='9')||(c>='a'&&c<='f')))return false;
            return true;
        }
        static void Fields(JsonElement obj,params string[] names){
            Need(obj.ValueKind==JsonValueKind.Object,"protocol_schema");
            var seen=new HashSet<string>(StringComparer.Ordinal);
            foreach(var item in obj.EnumerateObject())Need(seen.Add(item.Name),"protocol_schema");
            Need(seen.SetEquals(names),"protocol_schema");
        }
        JsonDocument Read(){
            Need(++frames<=16,"frame_count");
            int n=0;
            while(true){
                int value=input.ReadByte();
                if(value<0)throw new Refusal("input_eof");
                Need(n<4096,"frame_bytes");
                if(value==10)break;
                scratch[n++]=(byte)value;
            }
            var text=new UTF8Encoding(false,true).GetString(scratch,0,n);
            return JsonDocument.Parse(text,new JsonDocumentOptions{MaxDepth=8});
        }
        void Send(object obj){
            Need(++frames<=16,"frame_count");
            byte[] bytes=JsonSerializer.SerializeToUtf8Bytes(obj);
            Need(bytes.Length+1<=4096,"frame_bytes");
            output.Write(bytes,0,bytes.Length);output.WriteByte(10);output.Flush();
        }
        void CheckCommon(JsonElement root,string kind){
            Need(root.GetProperty("kind").GetString()==kind,"protocol_state");
            Need(root.GetProperty("version").GetInt32()==1
                && root.GetProperty("nonce").GetString()==nonce
                && root.GetProperty("source_sha256").GetString()==source
                && root.GetProperty("attempt").GetString()==attempt,"binding");
        }
        void Close(ref IntPtr handle){
            if(handle==IntPtr.Zero)return;
            IntPtr value=handle;
            if(CloseHandle(value)){handle=IntPtr.Zero;closed++;}
            else cleanup=false;
        }
        uint ActualParent(){
            IntPtr snapshot=CreateToolhelp32Snapshot(2,0);
            Native(snapshot!=new IntPtr(-1));snapshotClosed=false;
            try{
                var entry=new Entry{Size=(uint)Marshal.SizeOf(typeof(Entry))};
                Native(Process32FirstW(snapshot,ref entry));
                uint self=(uint)Environment.ProcessId;
                for(int n=0;n<65536;n++){
                    if(entry.Pid==self)return entry.Parent;
                    if(!Process32NextW(snapshot,ref entry)){
                        int error=Marshal.GetLastWin32Error();
                        Need(error==18,"parent_snapshot");break;
                    }
                }
                throw new Refusal("parent_binding");
            }finally{
                snapshotClosed=CloseHandle(snapshot);
                if(!snapshotClosed)cleanup=false;
            }
        }
        void LiveParent(){
            uint code;Native(GetExitCodeProcess(parent,out code));Need(code==259,"parent_exited");
        }
        object Inspect(IntPtr handle,JsonElement expected){
            Info info;Native(GetFileInformationByHandle(handle,out info));
            uint flags;Native(GetHandleInformation(handle,out flags));
            var name=new StringBuilder(32768);
            uint count=GetFinalPathNameByHandleW(handle,name,(uint)name.Capacity,0);
            Native(count!=0);Need(count<name.Capacity,"path_bytes");
            ulong index=((ulong)info.IndexHigh<<32)|info.IndexLow;
            string path=expected.GetProperty("path").GetString();
            Need(path!=null && path.EndsWith(@"\.kilogram-state.lock",StringComparison.Ordinal)
                && name.ToString()==path && GetFileType(handle)==1
                && (info.Attributes&0x410)==0 && info.Links==1
                && info.SizeHigh==0 && info.SizeLow==0
                && (flags&1)==0
                && info.Volume==expected.GetProperty("volume").GetUInt64()
                && index==expected.GetProperty("index").GetUInt64(),"identity");
            return new {path,volume=(ulong)info.Volume,index,links=info.Links,bytes=0,noninheritable=true};
        }
        object[] BindInputs(bool preservationMode){
            object[] witnesses=new object[2];
                using(var doc=Read()){
                    var root=doc.RootElement;
                    if(preservationMode){Fields(root,"kind","version","nonce","source_sha256","attempt","parent_pid","locks","preservation");preservation=root.GetProperty("preservation").Clone();}
                    else Fields(root,"kind","version","nonce","source_sha256","attempt","parent_pid","locks");
                    nonce=root.GetProperty("nonce").GetString();
                    Need(Hex(nonce,32),"binding");CheckCommon(root,"bind");
                    parentPid=ActualParent();
                    Need(parentPid!=0 && root.GetProperty("parent_pid").GetUInt32()==parentPid,"parent_binding");
                    parent=OpenProcess(0x1040,false,parentPid);
                    Native(parent!=IntPtr.Zero);LiveParent();
                    long born,unused,unused2,unused3,selfBorn;
                    Native(GetProcessTimes(parent,out born,out unused,out unused2,out unused3));
                    Native(GetProcessTimes(GetCurrentProcess(),out selfBorn,out unused,out unused2,out unused3));
                    Need(born>0 && born<=selfBorn,"parent_binding");
                    var locks=root.GetProperty("locks");Need(locks.ValueKind==JsonValueKind.Array&&locks.GetArrayLength()==2,"protocol_schema");
                    phase="duplicate";
                    for(int n=0;n<2;n++){
                        var item=locks[n];Fields(item,"handle","path","volume","index");
                        ulong raw=item.GetProperty("handle").GetUInt64();Need(raw>0&&raw<=(ulong)long.MaxValue,"handle_binding");
                        IntPtr copy;
                        Native(DuplicateHandle(parent,new IntPtr((long)raw),GetCurrentProcess(),out copy,0,false,2));
                        held[n]=copy;created++;
                        witnesses[n]=Inspect(copy,item);boundPaths[n]=item.GetProperty("path").GetString();
                    }
                    LiveParent();Close(ref parent);Need(cleanup&&parent==IntPtr.Zero,"cleanup");
                }
            return witnesses;
        }
        internal int Run(){
            string failure=null;int native=0;object[] witnesses=new object[2];
            try{
                witnesses=BindInputs(false);
                Send(new {kind="duplicated",version=1,nonce,source_sha256=source,attempt,parent_pid=parentPid,locks=witnesses,read_attempts=reads,owned_handles=2,parent_closed=true,snapshot_closed=snapshotClosed});
                phase="release";
                using(var doc=Read()){
                    Fields(doc.RootElement,"kind","version","nonce","source_sha256","attempt");
                    CheckCommon(doc.RootElement,"released");
                }
                phase="read";
                for(int n=0;n<2;n++){
                    // This rights probe is only reached AFTER the real parent's
                    // checked release; Rust-origin files are read-only.
                    uint written;writes++;
                    bool writable=WriteFile(held[n],new byte[]{0x78},1,out written,IntPtr.Zero);
                    int error=writable?0:Marshal.GetLastWin32Error();
                    Need(!writable&&error==5,"write_access");
                    byte[] one=new byte[1];uint read;reads++;
                    Native(ReadFile(held[n],one,1,out read,IntPtr.Zero));
                    Need(read==0,"not_empty");
                }
                Send(new {kind="read",version=1,nonce,source_sha256=source,attempt,read_attempts=reads,read_bytes=0,write_attempts=writes,write_denied_native=5,owned_handles=2});
                phase="finish";
                using(var doc=Read()){
                    Fields(doc.RootElement,"kind","version","nonce","source_sha256","attempt");
                    CheckCommon(doc.RootElement,"finish");
                }
            }catch(Refusal error){failure=error.Code;native=error.Native;}
             catch(Exception){failure="internal";}
            finally{
                for(int n=1;n>=0;n--)Close(ref held[n]);
                Close(ref parent);
            }
            int remaining=(held[0]!=IntPtr.Zero?1:0)+(held[1]!=IntPtr.Zero?1:0)+(parent!=IntPtr.Zero?1:0);
            bool clean=cleanup&&snapshotClosed&&remaining==0;
            if(!clean)failure="cleanup";
            try{
                Send(new {kind=failure==null?"done":"refused",version=1,nonce,source_sha256=source,attempt,phase,code=failure,native_error=native,duplicates_created=created,closed_handles=closed,handles_after=remaining,parent_closed=parent==IntPtr.Zero,snapshot_closed=snapshotClosed,cleanup_ok=clean,read_attempts=reads});
            }catch{return 42;}
            return failure==null?0:40;
        }
    }
    public static int Run(string hash,string attempt){return new Session(hash,attempt).Run();}
}
'@
exit ([S53TypedLockWorker]::Run($sourceHash,$Attempt))

