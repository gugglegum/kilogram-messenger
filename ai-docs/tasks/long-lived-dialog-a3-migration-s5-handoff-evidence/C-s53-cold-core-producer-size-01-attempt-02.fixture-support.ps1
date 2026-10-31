# R1 definitions only. No import, compilation, path probing or automatic invocation.
function Initialize-S53ProducerSizeFixtureSupportR02 {
    [CmdletBinding()]
    param()
    if ('Kilogram.S53.ProducerSizeR02.FixtureSupport' -as [type]) {
        throw 'Producer-size helper requires fresh types.'
    }
    if (-not $IsWindows) { throw 'Producer-size helper requires Windows.' }
    $source = @'
using System;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Buffers;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Security.Cryptography;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Kilogram.S53.ProducerSizeR02 {
// R2 must bind these synchronous delegates to the pinned accepted Root primitives:
// NewDirectory (raw fail-if-exists), DirectoryHandle, NewFile, Read(path,false),
// Assert-FixtureAcl (protected exact three-SID ACL). No delegate runs on a thread.
public sealed class Primitives {
    internal readonly Action<string> NewDirectory;
    internal readonly Func<string,SafeFileHandle> Directory;
    internal readonly Func<string,FileStream> NewFile,Read;
    internal readonly Action<string,bool> Acl;
    public Primitives(Action<string> newDirectory,Func<string,SafeFileHandle> directory,
        Func<string,FileStream> newFile,Func<string,FileStream> read,Action<string,bool> acl) {
        NewDirectory=newDirectory??throw new ArgumentNullException(nameof(newDirectory));
        Directory=directory??throw new ArgumentNullException(nameof(directory));
        NewFile=newFile??throw new ArgumentNullException(nameof(newFile));
        Read=read??throw new ArgumentNullException(nameof(read));
        Acl=acl??throw new ArgumentNullException(nameof(acl));
    }
}
public sealed class Summary {
    public string Stage;
    public int CreatedDirectories,CreateAttempts,CreateReturns,ReadAttempts,ReadReturns,FileCloseAttempts,FilesClosed;
    public int FileOwnersRemaining,PeakFileOwners,DirectoryOpenAttempts,DirectoryReturns,DirectoryCloseAttempts,DirectoriesClosed;
    public int DirectoryOwnersRemaining,PeakDirectoryOwners,EnumerationReturns,EnumerationsDisposed;
    public int EnumerationRemaining,PeakEnumerationOwners,StreamChecks,StreamSearchOpened,StreamSearchClosed,StreamSearchRemaining;
    public int BaselineFiles,PreparedFiles,SetupGuardsClosed;
    public int OracleRows,PeakIdentityEntries,PeakQueueEntries,PeakReceiptRecordBytes,PeakTokenWindowBytes;
    public bool ReadyForCore,ReceiptsVerified,Disposed,CleanupUnknown,PartialCreateUnknown,PartialReturnUnknown;
    public string CleanupFirstType;
    public int CleanupFirstHResult,CleanupNativeError;
}
public sealed class ReceiptReport {
    public bool Verified;
    public int Files,ComparisonRows,NamespaceRows;
    public long Bytes;
    // Fixed order: owner,comparison,namespace,result. No per-input evidence escapes.
    public long[] Sizes;
    public string[] Digests;
}
public sealed class FixtureSupport : IDisposable {
    public const string Family=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-producer-size-01-attempt-02";
    public const string Root=Family+@"\producer-size";
    public const string Origin=Root+@"\origin",State=Root+@"\state",Backup=Root+@"\backup",Proof=Root+@"\proof";
    public const string Attempt="C-s53-cold-core-producer-size-01-attempt-02/producer-size";
    public const int Rows=6956,PhysicalFiles=13915,InventoryBytes=3465145;
    public const long PayloadBytes=45597399,PhysicalBytes=94659943,HashPassBytes=182389596;
    public const string InventoryHash="6962b9986b7055d7bceb5e0ea15bfeb70ebec43c3db4d7de2cf0e5cbc693ea12";
    public const string TupleHash="2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf";
    const string EmptyHash="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const string Actual="actual file byte read",Derived="held-lock length checked; empty-byte digest derived";
    const string Rule="Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.";
    const int IdCap=13930,CaseDirectories=15,RecordCap=32768,ScanCap=13929;
    static readonly string[] DirectoryNames={"","origin","origin/one.state","origin/two.state",
        "state","state/one","state/one/state","state/two","state/two/state","state/safe-empty",
        "backup","backup/one","backup/one/state","backup/two","backup/two/state"};
    static readonly string[] ReceiptNames={"owner.private.json","comparison.private.jsonl","namespace.private.jsonl","result.private.json"};
    static readonly long[] ReceiptCaps={65536,33554432,16777216,65536};
    static readonly string[] StampFields={"volume","file_id","attributes","bytes","links","created_high","created_low","written_high","written_low"};
    static readonly string[] ObservationFields={"property","whole_attempt_verdict","continuous_namespace_absence","completed",
        "expected_set_captured_before_hash","observed_entry_sets_equal","input_handles_released","cleanup_unknown",
        "receipts_completed","cleanup_native_error","failure_code","failure_phase","failure_role","cause_kind","native_error",
        "error_hresult","expected_files","expected_bytes","locks","hash_passes","hash_bytes","capture_complete_timestamp",
        "historical_tuple_digest","s_before_tuple_digest","bkp_before_tuple_digest","s_after_tuple_digest","bkp_after_tuple_digest",
        "peak_owned_handles","remaining_owned_handles","peak_path_bytes","peak_record_bytes","peak_inventory_bytes",
        "scratch_reserved_bytes","receipt_bytes_charged","receipt_bytes_known_written","receipt_write_unknown",
        "phase_seconds","private_receipt_digests"};
    static readonly string[] PhaseNames={"allocation","output_binding","locks","inventory","capture_s","capture_bkp",
        "scan_before","hash_s_before","hash_bkp_before","hash_s_after","hash_bkp_after","scan_after","final_inputs","receipts"};
    sealed class Row { internal string Path,Hash;internal int Length,Index,Global;internal bool Lock; }
    sealed class FileOwner {
        internal FileStream Stream;internal SafeFileHandle Safe;internal bool CloseAttempted;
    }
    sealed class DirectoryOwner {
        internal string Path;internal SafeFileHandle Handle;internal bool CloseAttempted,Created;
        internal Info Initial;
    }
    struct Identity : IEquatable<Identity> {
        internal uint Volume;internal ulong Index;
        internal Identity(uint volume,ulong index){Volume=volume;Index=index;}
        public bool Equals(Identity x){return Volume==x.Volume&&Index==x.Index;}
        public override bool Equals(object x){return x is Identity&&Equals((Identity)x);}
        public override int GetHashCode(){return unchecked((int)Volume*397)^Index.GetHashCode();}
    }
    [StructLayout(LayoutKind.Sequential)] struct Info {
        internal uint Attributes;
        internal System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        internal uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
        internal long Length {get{return ((long)SizeHigh<<32)|SizeLow;}}
        internal Identity Id {get{return new Identity(Volume,((ulong)IndexHigh<<32)|IndexLow);}}
        internal bool Same(Info x){return Attributes==x.Attributes&&Volume==x.Volume&&SizeHigh==x.SizeHigh&&SizeLow==x.SizeLow&&
            Links==x.Links&&IndexHigh==x.IndexHigh&&IndexLow==x.IndexLow&&Created.dwHighDateTime==x.Created.dwHighDateTime&&
            Created.dwLowDateTime==x.Created.dwLowDateTime&&Written.dwHighDateTime==x.Written.dwHighDateTime&&Written.dwLowDateTime==x.Written.dwLowDateTime;}
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct StreamInfo {
        internal long Size;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=296)] internal string Name;
    }
    // Same accepted C2-I strict stream/held-info primitives, fixed R1 scope and bulk counts.
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
    [DllImport("kernel32.dll",SetLastError=true)] static extern uint GetFileType(SafeFileHandle h);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr FindFirstStreamW(string p,int level,out StreamInfo data,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool FindNextStreamW(IntPtr h,out StreamInfo data);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool FindClose(IntPtr h);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr h);
    readonly Primitives ops;
    readonly Action check;
    readonly long deadline;
    readonly Row[] rows=new Row[Rows];
    readonly Dictionary<string,Row> map=new Dictionary<string,Row>(Rows,StringComparer.Ordinal);
    readonly HashSet<Identity> identities=new HashSet<Identity>(IdCap);
    readonly bool[] seenFiles=new bool[PhysicalFiles],seenRows=new bool[Rows];
    readonly string[] queue=new string[CaseDirectories];
    readonly DirectoryOwner[] familyGuards=new DirectoryOwner[5],caseGuards=new DirectoryOwner[CaseDirectories];
    readonly FileOwner[] receiptOwners=new FileOwner[4];
    readonly byte[] io=new byte[65536],payload=new byte[6578],record=new byte[RecordCap],header=new byte[12];
    readonly StringBuilder finalPath=new StringBuilder(512);
    readonly UTF8Encoding utf8=new UTF8Encoding(false,true);
    readonly Summary facts=new Summary{Stage="oracle"};
    FileOwner currentFile;
    DirectoryOwner currentDirectory,proofGuard;
    IEnumerator<FileSystemInfo> currentEnumeration;
    bool enumerationCloseAttempted,prepareStarted,prepared,guardsReleased,receiptStarted,disposed;
    Exception cleanupFirst;
    IntPtr currentSearch=new IntPtr(-1);
    bool searchCloseAttempted;
    int familyCount,caseCount;
    long factoryLow,factoryHigh,startQpc,captureQpc;
    string decisionHash,coreHash;
    string[] lockStamps=new string[6];
    string[] receiptHashes=new string[4];
    long[] receiptSizes=new long[4];

    FixtureSupport(long absoluteWorkDeadline,Action work,Primitives primitives) {
        if(work==null||primitives==null||absoluteWorkDeadline<=0)throw new ArgumentException("Closed R1 factory.");
        check=work;ops=primitives;deadline=absoluteWorkDeadline;
        Work();BuildOracle();
    }
    public static FixtureSupport Create(long absoluteWorkDeadline,Action check,Primitives primitives) {
        return new FixtureSupport(absoluteWorkDeadline,check,primitives);
    }
    static void Need(bool condition,string code="schema"){if(!condition)throw Refusal(code);}
    static Exception Refusal(string code) {
        var e=new InvalidOperationException("Producer-size bounded refusal");e.Data["c2r_support_code"]=code;return e;
    }
    static string Hex(byte[] raw){return Convert.ToHexString(raw).ToLowerInvariant();}
    static bool IsHash(string s,int n=64){if(s==null||s.Length!=n)return false;foreach(char c in s)if(!((c>='0'&&c<='9')||(c>='a'&&c<='f')))return false;return true;}
    void Work() {Need(!disposed,"disposed");Need(!facts.CleanupUnknown,"cleanup_unknown");check();Need(Stopwatch.GetTimestamp()<deadline,"work_expired");}
    static string At(string relative){return relative.Length==0?Root:Root+"\\"+relative.Replace('/','\\');}
    static bool CaseScope(string p){return p==Root||p.StartsWith(Root+"\\",StringComparison.Ordinal);}
    static void Scope(string p) {
        Need(p!=null&&p.Length<=480&&(CaseScope(p)||p==Family||p==@"E:\"||p==@"E:\AI-Projects"||
            p==@"E:\AI-Projects\kilogram-p2p-messenger"||p==@"E:\AI-Projects\kilogram-p2p-messenger\.tmp")&&
            String.Equals(Path.GetFullPath(p),p,StringComparison.Ordinal),"scope");
    }
    void CleanupFailure(Exception e,int native=0) {
        facts.CleanupUnknown=true;
        if(native==0&&e is Win32Exception)native=((Win32Exception)e).NativeErrorCode;
        if(cleanupFirst==null){cleanupFirst=e;facts.CleanupFirstType=e.GetType().FullName;facts.CleanupFirstHResult=e.HResult;facts.CleanupNativeError=native;}
    }
    void PartialReturn(){facts.PartialReturnUnknown=true;}
    int FileOwners(){return facts.CreateReturns+facts.ReadReturns-facts.FilesClosed;}
    FileOwner OpenFile(string path,bool create) {
        Work();Scope(path);Need(CaseScope(path)&&currentFile==null&&FileOwners()<4,"file_owner_cap");
        var owner=new FileOwner();currentFile=owner; // reachable before factory/getter/validation
        if(create)facts.CreateAttempts++;else facts.ReadAttempts++;
        try {
            owner.Stream=create?ops.NewFile(path):ops.Read(path);
            if(create)facts.CreateReturns++;else facts.ReadReturns++;
            Need(owner.Stream!=null,"file_return");owner.Safe=owner.Stream.SafeFileHandle;
            Need(owner.Safe!=null&&!owner.Safe.IsInvalid&&!owner.Safe.IsClosed,"file_return");
            facts.PeakFileOwners=Math.Max(facts.PeakFileOwners,FileOwners());return owner;
        } catch {
            if(owner.Stream==null){PartialReturn();if(create)facts.PartialCreateUnknown=true;}
            throw;
        } finally {
            // A successful owner stays current until explicitly closed/transferred.
            if(owner.Stream==null){facts.CleanupUnknown=true;}
        }
    }
    void CloseFile(FileOwner owner) {
        if(owner==null||owner.CloseAttempted)return;
        owner.CloseAttempted=true;facts.FileCloseAttempts++;
        if(owner.Stream==null){CleanupFailure(Refusal("unknown_file_return"));return;}
        SafeFileHandle safe=owner.Safe;
        // Even a failing SafeFileHandle getter must not skip Stream.Dispose.
        if(safe==null){try{safe=owner.Stream.SafeFileHandle;}catch(Exception e){CleanupFailure(e);}}
        try {
            owner.Stream.Dispose();
            if(safe==null||!safe.IsClosed){CleanupFailure(Refusal("file_close"));return;}
            facts.FilesClosed++;owner.Stream=null;owner.Safe=null;
            if(ReferenceEquals(currentFile,owner))currentFile=null;
        } catch(Exception e){CleanupFailure(e);}
    }
    void CloseDirectory(DirectoryOwner owner) {
        if(owner==null||owner.CloseAttempted)return;
        owner.CloseAttempted=true;facts.DirectoryCloseAttempts++;
        if(owner.Handle==null){CleanupFailure(Refusal("unknown_directory_return"));return;}
        try {
            bool ok=CloseHandle(owner.Handle.DangerousGetHandle());int error=ok?0:Marshal.GetLastWin32Error();
            owner.Handle.SetHandleAsInvalid();owner.Handle.Dispose(); // exactly once, no recycled-value retry
            if(!ok){CleanupFailure(new Win32Exception(error),error);return;}
            facts.DirectoriesClosed++;owner.Handle=null;
            if(ReferenceEquals(currentDirectory,owner))currentDirectory=null;
        } catch(Exception e){CleanupFailure(e);}
    }
    DirectoryOwner OpenDirectory(string path,bool create) {
        Work();Scope(path);Need(currentDirectory==null&&facts.DirectoryReturns-facts.DirectoriesClosed<21,"directory_owner_cap");
        var owner=new DirectoryOwner{Path=path};currentDirectory=owner;
        try {
            if(create){try{ops.NewDirectory(path);owner.Created=true;facts.CreatedDirectories++;}
                catch{facts.PartialCreateUnknown=true;throw;}}
            facts.DirectoryOpenAttempts++;owner.Handle=ops.Directory(path);facts.DirectoryReturns++;
            Need(owner.Handle!=null&&!owner.Handle.IsClosed&&!owner.Handle.IsInvalid,"directory_return");
            owner.Initial=HeldInfo(owner.Handle,path,true,0);
            if(create)ops.Acl(path,true);
            facts.PeakDirectoryOwners=Math.Max(facts.PeakDirectoryOwners,facts.DirectoryReturns-facts.DirectoriesClosed);
            return owner;
        } catch {if(owner.Handle==null)PartialReturn();throw;}
    }
    void RegisterDirectory(DirectoryOwner[] slots,ref int count,DirectoryOwner owner) {
        Need(count<slots.Length&&ReferenceEquals(currentDirectory,owner),"directory_owner_cap");
        slots[count++]=owner;currentDirectory=null; // already owned if validation/insertion failed
    }
    Info HeldInfo(SafeFileHandle held,string path,bool directory,long length) {
        Scope(path);Need(held!=null&&!held.IsInvalid&&!held.IsClosed,"held_handle");Info i;
        bool ok=GetFileInformationByHandle(held,out i);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)throw new Win32Exception(error);
        uint type=GetFileType(held);error=type==0?Marshal.GetLastWin32Error():0;
        if(type!=1){if(error!=0)throw new Win32Exception(error);throw Refusal("held_type");}
        Need((i.Attributes&(0x400u|0x40u))==0&&((i.Attributes&0x10u)!=0)==directory&&
            (directory||(i.Links==1&&i.Length==length)),"held_shape");
        finalPath.Clear();uint n=GetFinalPathNameByHandleW(held,finalPath,512,0);error=n==0?Marshal.GetLastWin32Error():0;
        if(n==0)throw new Win32Exception(error);
        Need(n<512,"held_path");string actual=finalPath.ToString();
        if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual.Substring(4);
        Need(String.Equals(actual.TrimEnd('\\'),path.TrimEnd('\\'),StringComparison.OrdinalIgnoreCase),"held_path");
        return i;
    }
    void AddIdentity(Identity id) {
        Need(identities.Count<IdCap&&identities.Add(id),"identity_alias_or_cap");
        facts.PeakIdentityEntries=Math.Max(facts.PeakIdentityEntries,identities.Count);
    }
    void AddId(Info info) {AddIdentity(info.Id);}
    void StrictStreams(string path,SafeFileHandle held,bool directory,long length) {
        Work();Need(facts.StreamSearchRemaining==0,"stream_owner_cap");
        Info before=HeldInfo(held,path,directory,length);
        try {
            StreamInfo data;IntPtr find=FindFirstStreamW(path,0,out data,0);
            bool bad=find==IntPtr.Zero||find==new IntPtr(-1);int error=bad?Marshal.GetLastWin32Error():0;
            if(!bad){currentSearch=find;searchCloseAttempted=false;facts.StreamSearchOpened++;facts.StreamSearchRemaining=1;}
            if(bad){if(!(directory&&error==38)){if(error!=0)throw new Win32Exception(error);throw Refusal("stream_enumeration");}}
            else {
                Work();Need(!directory&&data.Name=="::$DATA"&&data.Size==length,"named_or_changed_stream");
                Work();bool next=FindNextStreamW(find,out data);error=next?0:Marshal.GetLastWin32Error();
                Need(!next,"extra_stream");if(error!=38)throw new Win32Exception(error);
            }
            Work();Need(before.Same(HeldInfo(held,path,directory,length)),"stream_stamp");
            facts.StreamChecks++;
        } finally {CloseSearch();}
        Work();
    }
    void CloseSearch() {
        if(facts.StreamSearchRemaining==0||searchCloseAttempted)return;
        searchCloseAttempted=true;
        try {
            bool ok=FindClose(currentSearch);int error=ok?0:Marshal.GetLastWin32Error();
            if(ok){facts.StreamSearchClosed++;facts.StreamSearchRemaining=0;currentSearch=new IntPtr(-1);}
            else CleanupFailure(new Win32Exception(error),error);
        } catch(Exception e){CleanupFailure(e);}
    }
    void BeginEnumeration(string path) {
        Work();Scope(path);Need(currentEnumeration==null,"enumerator_owner_cap");
        // The slot is reserved by the null check; no MoveNext before owner registration.
        try{currentEnumeration=new DirectoryInfo(path).EnumerateFileSystemInfos().GetEnumerator();
            facts.EnumerationReturns++;facts.PeakEnumerationOwners=1;enumerationCloseAttempted=false;}
        catch{PartialReturn();throw;}
    }
    void CloseEnumeration() {
        if(currentEnumeration==null||enumerationCloseAttempted)return;
        enumerationCloseAttempted=true;
        try{currentEnumeration.Dispose();facts.EnumerationsDisposed++;currentEnumeration=null;}
        catch(Exception e){CleanupFailure(e);}
    }
    void FillPayload(int global,int length) {
        Array.Clear(payload,0,payload.Length);
        if(global>=0)for(int k=0;k<4;k++)payload[k]=(byte)(global>>(8*k));
        Need(length>=0&&length<=payload.Length,"payload_buffer");
    }
    void BuildOracle() {
        int at=0;long total=0;
        using(var tuple=IncrementalHash.CreateHash(HashAlgorithmName.SHA256)){
            tuple.AppendData(Encoding.ASCII.GetBytes("ColdA-v1\0"));
            for(int side=0;side<2;side++)for(int local=-1;local<3477;local++){
                Work();Need(at<Rows,"oracle_cap");int global=local<0?-1:side*3477+local;
                int length=global<0?0:global==6953?6578:6557;
                string prefix=side==0?"one/state/":"two/state/";
                string path=prefix+(global<0?".kilogram-state.lock":"p"+global.ToString("D6",CultureInfo.InvariantCulture)+".bin");
                FillPayload(global,length);
                var row=new Row{Path=path,Length=length,Global=global,Lock=global<0,Index=at,Hash=Hex(SHA256.HashData(new ReadOnlySpan<byte>(payload,0,length)))};
                rows[at++]=row;map.Add(path,row);total+=length;facts.OracleRows=at;
                byte[] name=utf8.GetBytes(path);uint n=(uint)name.Length;ulong size=(ulong)length;
                for(int k=0;k<4;k++)header[k]=(byte)(n>>(8*k));
                for(int k=0;k<8;k++)header[k+4]=(byte)(size>>(8*k));
                tuple.AppendData(header,0,4);tuple.AppendData(name);tuple.AppendData(header,4,8);tuple.AppendData(Convert.FromHexString(row.Hash));
            }
            Need(at==Rows&&total==PayloadBytes&&Hex(tuple.GetHashAndReset())==TupleHash,"oracle_pin");
        }
        Array.Clear(payload,0,payload.Length);facts.Stage="dormant_ready";
    }
    sealed class CappedWrite : Stream {
        readonly FixtureSupport owner;readonly FileStream output;internal long Count;
        internal CappedWrite(FixtureSupport o,FileStream s){owner=o;output=s;}
        public override void Write(byte[] b,int offset,int count) {
            owner.Work();Need(count>=0&&count<=InventoryBytes-Count,"inventory_write_cap");
            output.Write(b,offset,count);Count+=count;owner.Work();
        }
        public override void Write(ReadOnlySpan<byte> b){owner.Work();Need(b.Length<=InventoryBytes-Count,"inventory_write_cap");output.Write(b);Count+=b.Length;owner.Work();}
        public override void Flush(){output.Flush();}
        public override bool CanRead{get{return false;}}public override bool CanSeek{get{return false;}}public override bool CanWrite{get{return true;}}
        public override long Length{get{return Count;}}public override long Position{get{return Count;}set{throw new NotSupportedException();}}
        public override int Read(byte[] b,int o,int n){throw new NotSupportedException();}public override long Seek(long n,SeekOrigin s){throw new NotSupportedException();}
        public override void SetLength(long n){throw new NotSupportedException();}
        // Never owns/disposes the underlying file owner.
    }
    void WriteInventory(FileStream stream) {
        var sink=new CappedWrite(this,stream);
        using(var w=new Utf8JsonWriter(sink)){
            w.WriteStartObject();w.WriteString("source",State);w.WriteString("preserved",Backup);
            w.WriteString("classification","private-stopped-copy-never-opened");
            w.WriteStartArray("held_lock_paths");w.WriteStringValue(State+@"\one\state\.kilogram-state.lock");w.WriteStringValue(State+@"\two\state\.kilogram-state.lock");w.WriteEndArray();
            w.WriteString("os_lock_rule",Rule);w.Flush();
            foreach(string array in new[]{"source_files","source_after_files","destination_files"}){
                w.WriteStartArray(array);
                foreach(Row row in rows){
                    Work();w.WriteStartObject();w.WriteString("path",row.Path);w.WriteNumber("bytes",row.Length);
                    w.WriteString("sha256",row.Hash);w.WriteString("digest_provenance",row.Lock&&array!="destination_files"?Derived:Actual);
                    w.WriteEndObject();w.Flush(); // bounded row; never buffer the entire inventory
                }
                w.WriteEndArray();w.Flush();
            }
            w.WriteEndObject();w.Flush();
        }
        sink.WriteByte(10);Need(sink.Count==InventoryBytes,"inventory_bytes");
    }
    string HashFile(FileStream stream,long bytes) {
        Work();Need(bytes>=0&&bytes<=67108864&&stream.Length==bytes,"file_bytes");stream.Position=0;
        using(var h=IncrementalHash.CreateHash(HashAlgorithmName.SHA256)){
            long total=0;while(total<bytes){Work();int n=stream.Read(io,0,(int)Math.Min(io.Length,bytes-total));Need(n>0,"short_read");h.AppendData(io,0,n);total+=n;Work();}
            Need(stream.ReadByte()==-1,"extra_bytes");return Hex(h.GetHashAndReset());
        }
    }
    void WriteFile(string path,Row row,bool inventory) {
        FileOwner owner=null;
        try {
            owner=OpenFile(path,true);ops.Acl(path,false);
            if(inventory)WriteInventory(owner.Stream);
            else{FillPayload(row.Global,row.Length);owner.Stream.Write(payload,0,row.Length);}
            owner.Stream.Flush(true);Work();
            long length=inventory?InventoryBytes:row.Length;
            Info before=HeldInfo(owner.Safe,path,false,length);StrictStreams(path,owner.Safe,false,length);
            Need(HashFile(owner.Stream,length)==(inventory?InventoryHash:row.Hash),"fixture_hash");
            Need(before.Same(HeldInfo(owner.Safe,path,false,length)),"fixture_stamp");ops.Acl(path,false);
        } finally{CloseFile(owner??currentFile);}
        Work();
    }
    Row EmptyRow(){return rows[0];}
    public void PrepareAndVerify() {
        Work();Need(!prepareStarted&&!receiptStarted,"one_use");prepareStarted=true;facts.Stage="create";
        try {
            foreach(string p in new[]{@"E:\",@"E:\AI-Projects",@"E:\AI-Projects\kilogram-p2p-messenger",@"E:\AI-Projects\kilogram-p2p-messenger\.tmp"}){
                Work();RegisterDirectory(familyGuards,ref familyCount,OpenDirectory(p,false));
            }
            RegisterDirectory(familyGuards,ref familyCount,OpenDirectory(Family,true));
            foreach(string relative in DirectoryNames)RegisterDirectory(caseGuards,ref caseCount,OpenDirectory(At(relative),true));
            WriteFile(Origin+@"\one.state\.kilogram-state.lock",EmptyRow(),false);
            WriteFile(Origin+@"\two.state\.kilogram-state.lock",EmptyRow(),false);
            foreach(string mirror in new[]{State,Backup})foreach(Row row in rows)WriteFile(mirror+"\\"+row.Path.Replace('/','\\'),row,false);
            WriteFile(Backup+@"\preservation-inventory.json",null,true);
            Need(facts.CreatedDirectories==16&&facts.CreateAttempts==PhysicalFiles&&facts.CreateReturns==PhysicalFiles&&facts.FilesClosed==PhysicalFiles,"setup_ledger");
            facts.Stage="baseline";Walk();facts.BaselineFiles=PhysicalFiles;
            facts.Stage="prepared";Walk();facts.PreparedFiles=PhysicalFiles;
            Need(facts.ReadAttempts==27830&&facts.ReadReturns==27830&&facts.FilesClosed==41745&&FileOwners()==0&&
                facts.StreamSearchRemaining==0&&currentEnumeration==null&&!facts.CleanupUnknown,"setup_ledger");
            prepared=true;facts.Stage="premises_verified";
        } finally{CloseFile(currentFile);CloseEnumeration();CloseSearch();if(currentDirectory!=null)CloseDirectory(currentDirectory);}
        Work();
    }
    void ResolveFile(string relative,out Row row,out bool inventory,out int index) {
        inventory=false;row=null;index=-1;
        if(relative=="origin/one.state/.kilogram-state.lock"){row=EmptyRow();index=0;return;}
        if(relative=="origin/two.state/.kilogram-state.lock"){row=EmptyRow();index=1;return;}
        if(relative=="backup/preservation-inventory.json"){inventory=true;index=PhysicalFiles-1;return;}
        bool s=relative.StartsWith("state/",StringComparison.Ordinal),b=relative.StartsWith("backup/",StringComparison.Ordinal);
        Need(s||b,"premise_extra_file");string name=relative.Substring(s?6:7);
        Need(map.TryGetValue(name,out row),"premise_extra_file");index=2+(s?0:Rows)+row.Index;
    }
    void Walk() {
        identities.Clear();Array.Clear(seenFiles,0,seenFiles.Length);Array.Clear(queue,0,queue.Length);
        queue[0]="";facts.PeakQueueEntries=Math.Max(facts.PeakQueueEntries,1);int head=0,tail=1,entries=0,files=0,locks=0;long bytes=0,sBytes=0,bBytes=0;
        foreach(DirectoryOwner d in caseGuards){Work();AddId(HeldInfo(d.Handle,d.Path,true,0));}
        while(head<tail){
            Work();string parent=queue[head++];string path=At(parent);int di=Array.IndexOf(DirectoryNames,parent);Need(di>=0,"premise_directory");
            DirectoryOwner held=caseGuards[di];ops.Acl(path,true);Info before=HeldInfo(held.Handle,path,true,0);StrictStreams(path,held.Handle,true,0);
            BeginEnumeration(path);
            try{
                while(true){
                    Work();if(!currentEnumeration.MoveNext())break;Work();Need(++entries<=ScanCap,"premise_entry_cap");
                    FileSystemInfo item=currentEnumeration.Current;string name=item.Name;
                    Need(name.Length>0&&name.Length<=255&&name.IndexOfAny(new[]{'/','\\',':'})<0,"premise_name");
                    string relative=parent.Length==0?name:parent+"/"+name;
                    bool directory=(item.Attributes&FileAttributes.Directory)!=0;
                    Need((item.Attributes&(FileAttributes.ReparsePoint|FileAttributes.Device))==0,"premise_kind");
                    if(directory){
                        int at=Array.IndexOf(DirectoryNames,relative);Need(at>0&&tail<CaseDirectories,"premise_extra_directory");
                        for(int k=0;k<tail;k++)Need(queue[k]!=relative,"premise_duplicate_directory");
                        queue[tail++]=relative;facts.PeakQueueEntries=Math.Max(facts.PeakQueueEntries,tail);continue;
                    }
                    Row row;bool inventory;int index;ResolveFile(relative,out row,out inventory,out index);
                    Need(index>=0&&index<PhysicalFiles&&!seenFiles[index],"premise_duplicate_file");seenFiles[index]=true;
                    FileOwner owner=null;long length=inventory?InventoryBytes:row.Length;string absolute=At(relative);
                    try{
                        owner=OpenFile(absolute,false);Info initial=HeldInfo(owner.Safe,absolute,false,length);AddId(initial);
                        ops.Acl(absolute,false);StrictStreams(absolute,owner.Safe,false,length);
                        Need(HashFile(owner.Stream,length)==(inventory?InventoryHash:row.Hash),"premise_digest");
                        if(inventory){owner.Stream.Position=0;ValidateInventory(owner.Stream);}
                        Need(initial.Same(HeldInfo(owner.Safe,absolute,false,length)),"premise_stamp");ops.Acl(absolute,false);
                    } finally {CloseFile(owner??currentFile);}
                    Work();files++;bytes+=length;if(!inventory&&row.Lock)locks++;
                    if(relative.StartsWith("state/",StringComparison.Ordinal))sBytes+=length;
                    if(relative.StartsWith("backup/",StringComparison.Ordinal)&&!inventory)bBytes+=length;
                }
            }finally{CloseEnumeration();}
            Work();Need(before.Same(HeldInfo(held.Handle,path,true,0)),"premise_directory_stamp");
        }
        Need(entries==ScanCap&&files==PhysicalFiles&&locks==6&&tail==CaseDirectories&&bytes==PhysicalBytes&&
            sBytes==PayloadBytes&&bBytes==PayloadBytes&&identities.Count==IdCap,"premise_totals");
        foreach(bool seen in seenFiles)Need(seen,"premise_missing");
    }
    public void ReleaseSetupGuards() {
        Work();Need(prepared&&!guardsReleased&&!receiptStarted,"handoff_order");
        for(int i=caseCount-1;i>=0;i--){CloseDirectory(caseGuards[i]);if(caseGuards[i].Handle==null)facts.SetupGuardsClosed++;}
        Need(facts.SetupGuardsClosed==CaseDirectories&&FileOwners()==0&&currentEnumeration==null&&!facts.CleanupUnknown,"handoff_cleanup");
        guardsReleased=true;facts.ReadyForCore=true;facts.Stage="released_for_core";
    }
    // Bounded token reader: one64KiB window, complete token<=4096 raw bytes, no full inventory text/DOM.
    sealed class Tokens : IDisposable {
        readonly FixtureSupport owner;readonly FileStream stream;readonly byte[] buffer=new byte[65536];
        JsonReaderState state=new JsonReaderState(new JsonReaderOptions{MaxDepth=8,CommentHandling=JsonCommentHandling.Disallow,AllowTrailingCommas=false});
        int begin,end,count;bool eof;long read;
        internal JsonTokenType Type;internal string Text;internal long Number;
        internal Tokens(FixtureSupport o,FileStream s){owner=o;stream=s;}
        internal bool Next() {
            while(true){
                owner.Work();var r=new Utf8JsonReader(new ReadOnlySpan<byte>(buffer,begin,end-begin),eof,state);
                if(r.Read()){
                    Need(++count<=250000,"inventory_token_cap");Type=r.TokenType;Text=null;Number=0;
                    if(Type==JsonTokenType.String||Type==JsonTokenType.PropertyName){
                        Need(r.ValueSpan.Length<=4096,"inventory_token_cap");
                        owner.utf8.GetCharCount(r.ValueSpan);Text=r.GetString();
                    }else if(Type==JsonTokenType.Number)Need(r.TryGetInt64(out Number),"inventory_integer");
                    begin+=checked((int)r.BytesConsumed);state=r.CurrentState;return true;
                }
                begin+=checked((int)r.BytesConsumed);state=r.CurrentState;
                if(eof){Need(begin==end&&read==InventoryBytes,"inventory_eof");return false;}
                if(begin>0){Buffer.BlockCopy(buffer,begin,buffer,0,end-begin);end-=begin;begin=0;}
                Need(end<buffer.Length,"inventory_token_cap");int n=stream.Read(buffer,end,buffer.Length-end);
                owner.Work();Need(n<=InventoryBytes-read,"inventory_bytes");read+=n;end+=n;owner.facts.PeakTokenWindowBytes=Math.Max(owner.facts.PeakTokenWindowBytes,end);eof=n==0;
            }
        }
        internal void Want(JsonTokenType t){Need(Next()&&Type==t,"inventory_schema");}
        internal string String(){Want(JsonTokenType.String);return Text;}
        public void Dispose(){Array.Clear(buffer,0,buffer.Length);Text=null;}
    }
    void ValidateInventory(FileStream stream) {
        Need(stream.Length==InventoryBytes,"inventory_bytes");
        using(var t=new Tokens(this,stream)){
            t.Want(JsonTokenType.StartObject);int mask=0;
            string[] fields={"source","preserved","classification","held_lock_paths","os_lock_rule","source_files","source_after_files","destination_files"};
            while(t.Next()&&t.Type!=JsonTokenType.EndObject){
                Need(t.Type==JsonTokenType.PropertyName,"inventory_schema");int key=Array.IndexOf(fields,t.Text);
                Need(key>=0&&(mask&(1<<key))==0,"inventory_fields");mask|=1<<key;
                if(key==0)Need(t.String()==State,"inventory_binding");
                else if(key==1)Need(t.String()==Backup,"inventory_binding");
                else if(key==2)Need(t.String()=="private-stopped-copy-never-opened","inventory_binding");
                else if(key==3){
                    t.Want(JsonTokenType.StartArray);Need(t.String()==State+@"\one\state\.kilogram-state.lock","inventory_binding");
                    Need(t.String()==State+@"\two\state\.kilogram-state.lock","inventory_binding");t.Want(JsonTokenType.EndArray);
                }else if(key==4)Need(t.String()==Rule,"inventory_provenance");
                else{
                    Array.Clear(seenRows,0,seenRows.Length);int rowsRead=0;long total=0;t.Want(JsonTokenType.StartArray);
                    while(t.Next()&&t.Type!=JsonTokenType.EndArray){
                        Work();Need(t.Type==JsonTokenType.StartObject&&++rowsRead<=Rows,"inventory_rows");
                        int rm=0;string path=null,hash=null,provenance=null;long size=-1;
                        string[] names={"path","bytes","sha256","digest_provenance"};
                        while(t.Next()&&t.Type!=JsonTokenType.EndObject){
                            Need(t.Type==JsonTokenType.PropertyName,"inventory_schema");int f=Array.IndexOf(names,t.Text);
                            Need(f>=0&&(rm&(1<<f))==0,"inventory_row_fields");rm|=1<<f;
                            if(f==1){t.Want(JsonTokenType.Number);size=t.Number;}
                            else{string v=t.String();if(f==0)path=v;else if(f==2)hash=v;else provenance=v;}
                        }
                        Need(t.Type==JsonTokenType.EndObject&&rm==15&&path!=null,"inventory_row_fields");Row row;
                        Need(map.TryGetValue(path,out row)&&!seenRows[row.Index],"inventory_map");seenRows[row.Index]=true;
                        Need(size==row.Length&&hash==row.Hash&&provenance==(row.Lock&&key<7?Derived:Actual),"inventory_triplet");
                        total=checked(total+size);
                    }
                    Need(t.Type==JsonTokenType.EndArray&&rowsRead==Rows&&total==PayloadBytes,"inventory_rows");
                    foreach(bool seen in seenRows)Need(seen,"inventory_missing");
                }
            }
            Need(t.Type==JsonTokenType.EndObject&&mask==255&&!t.Next(),"inventory_fields");
        }
    }
    static void Fields(JsonElement e,string[] names,int count=-1) {
        Need(e.ValueKind==JsonValueKind.Object,"receipt_schema");int max=count<0?names.Length:count;Need(max<=64,"field_cap");
        ulong seen=0;int n=0;
        foreach(var p in e.EnumerateObject()){
            int at=Array.IndexOf(names,p.Name,0,max);Need(at>=0&&(seen&(1UL<<at))==0,"receipt_fields");seen|=1UL<<at;n++;
        }
        Need(n==max,"receipt_fields");
    }
    static string Text(JsonElement e){Need(e.ValueKind==JsonValueKind.String,"receipt_type");return e.GetString();}
    static string T(JsonElement e,string k){return Text(e.GetProperty(k));}
    static long Number(JsonElement e){Need(e.ValueKind==JsonValueKind.Number,"receipt_type");long n;Need(e.TryGetInt64(out n),"receipt_integer");return n;}
    static long N(JsonElement e,string k){return Number(e.GetProperty(k));}
    static bool B(JsonElement e,string k){var v=e.GetProperty(k);Need(v.ValueKind==JsonValueKind.True||v.ValueKind==JsonValueKind.False,"receipt_type");return v.GetBoolean();}
    static string Stamp(JsonElement e,bool directory,long length,out Identity identity) {
        Fields(e,StampFields);
        foreach(string k in new[]{"volume","attributes","links"})Need(N(e,k)>=0&&N(e,k)<=uint.MaxValue,"stamp_range");
        foreach(string k in new[]{"created_high","created_low","written_high","written_low"})Need(N(e,k)>=int.MinValue&&N(e,k)<=int.MaxValue,"stamp_range");
        string id=T(e,"file_id");Need(IsHash(id,16),"stamp_id");long attrs=N(e,"attributes"),size=N(e,"bytes");
        Need((attrs&(0x400|0x40))==0&&((attrs&0x10)!=0)==directory&&size>=0&&
            (directory||(N(e,"links")==1&&size==length)),"stamp_shape");
        identity=new Identity((uint)N(e,"volume"),ulong.Parse(id,NumberStyles.HexNumber,CultureInfo.InvariantCulture));
        // Canonical values, not raw JSON property order/spacing, define stamp equality.
        var b=new StringBuilder(256);
        foreach(string k in StampFields){if(k=="file_id")b.Append(id);else b.Append(N(e,k).ToString(CultureInfo.InvariantCulture));b.Append(':');}
        return b.ToString();
    }
    string ValidateStamp(JsonElement e,bool directory,long length,bool add) {
        Identity id;string value=Stamp(e,directory,length,out id);
        if(add)AddIdentity(id);
        return value;
    }
    static JsonDocument Document(ReadOnlyMemory<byte> bytes) {
        return JsonDocument.Parse(bytes,new JsonDocumentOptions{MaxDepth=8,CommentHandling=JsonCommentHandling.Disallow,AllowTrailingCommas=false});
    }
    void EnumerateReceipts() {
        bool[] found=new bool[4];int count=0;BeginEnumeration(Proof);
        try{
            while(true){
                Work();if(!currentEnumeration.MoveNext())break;Work();Need(++count<=4,"receipt_set");
                FileSystemInfo item=currentEnumeration.Current;int at=Array.IndexOf(ReceiptNames,item.Name);
                Need(at>=0&&!found[at]&&(item.Attributes&(FileAttributes.Directory|FileAttributes.ReparsePoint|FileAttributes.Device))==0,"receipt_set");found[at]=true;
                // Bracket enumerated name by retained file ID, never use enumeration as a substitute for held input.
                if(receiptOwners[at]!=null)Need(HeldInfo(receiptOwners[at].Safe,Proof+"\\"+ReceiptNames[at],false,receiptSizes[at]).Same(receiptInitial[at]),"receipt_stamp");
            }
        }finally{CloseEnumeration();}
        Need(count==4,"receipt_set");
    }
    readonly Info[] receiptInitial=new Info[4];
    void ReadReceipt(int index,Action<JsonElement,int> validate,int wanted) {
        FileOwner owner=receiptOwners[index];long total=0;int used=0,lines=0;
        owner.Stream.Position=0;
        using(var hash=IncrementalHash.CreateHash(HashAlgorithmName.SHA256)){
            while(true){
                Work();int n=owner.Stream.Read(io,0,io.Length);Work();if(n==0)break;
                Need(n<=ReceiptCaps[index]-total,"receipt_file_cap");total+=n;hash.AppendData(io,0,n);
                for(int k=0;k<n;k++){
                    byte c=io[k];Need(c!=13,"receipt_cr");
                    if(c==10){
                        Need(used>=2&&record[0]==123&&record[used-1]==125&&lines<wanted,"receipt_frame");
                        facts.PeakReceiptRecordBytes=Math.Max(facts.PeakReceiptRecordBytes,used);
                        utf8.GetCharCount(record,0,used);
                        using(var doc=Document(new ReadOnlyMemory<byte>(record,0,used))){validate(doc.RootElement,lines);}
                        lines++;Array.Clear(record,0,used);used=0;Work();
                    }else{Need(used<RecordCap,"receipt_record_cap");record[used++]=c;}
                }
            }
            Need(used==0&&lines==wanted&&total==receiptSizes[index],"receipt_records");
            receiptHashes[index]=Hex(hash.GetHashAndReset());
        }
    }
    string[] Locks(){return new[]{Origin+@"\one.state\.kilogram-state.lock",Origin+@"\two.state\.kilogram-state.lock",
        State+@"\one\state\.kilogram-state.lock",State+@"\two\state\.kilogram-state.lock",Backup+@"\one\state\.kilogram-state.lock",Backup+@"\two\state\.kilogram-state.lock"};}
    void ValidateOwner(JsonElement e,int line) {
        Fields(e,new[]{"classification","attempt","decision_sha256","core_sha256","activation_and_new_e_ownership_verified_by_core",
            "external_guardian_required","core_start_timestamp","work_deadline_timestamp","source","preserved","output","inventory_bytes",
            "inventory_sha256","historical_inventory_validated","expected_files","expected_bytes","locks"});
        Need(T(e,"classification")=="cold-a-private-owner-r44"&&T(e,"attempt")==Attempt&&T(e,"decision_sha256")==decisionHash&&T(e,"core_sha256")==coreHash,"owner_binding");
        Need(!B(e,"activation_and_new_e_ownership_verified_by_core")&&B(e,"external_guardian_required")&&
            N(e,"core_start_timestamp")>=factoryLow&&N(e,"core_start_timestamp")<=factoryHigh&&N(e,"work_deadline_timestamp")==deadline,"owner_time");
        Need(T(e,"source")==State&&T(e,"preserved")==Backup&&T(e,"output")==Proof&&N(e,"inventory_bytes")==InventoryBytes&&
            T(e,"inventory_sha256")==InventoryHash&&B(e,"historical_inventory_validated")&&N(e,"expected_files")==Rows&&N(e,"expected_bytes")==PayloadBytes,"owner_binding");
        JsonElement locks=e.GetProperty("locks");Need(locks.ValueKind==JsonValueKind.Array&&locks.GetArrayLength()==6,"lock_set");
        int i=0;string[] names=Locks();var ownerIds=new HashSet<Identity>(6);
        foreach(var l in locks.EnumerateArray()){
            Fields(l,new[]{"path","verified_empty","provenance","read_bytes","empty_hash_derived","initial"});
            Need(T(l,"path")==names[i]&&B(l,"verified_empty")&&B(l,"empty_hash_derived")&&N(l,"read_bytes")==0&&T(l,"provenance")=="verified-empty-held-handle","lock_binding");
            Identity id;lockStamps[i]=Stamp(l.GetProperty("initial"),false,0,out id);Need(ownerIds.Count<6&&ownerIds.Add(id),"lock_alias");
            if(i<2)AddIdentity(id);i++;
        }
    }
    void ValidateComparison(JsonElement e,int index) {
        Row row=rows[index];
        Fields(e,new[]{"path","bytes","historical_sha256","historical_triplet_validated","source_provenance","source_after_provenance","destination_provenance","s","bkp"});
        Need(T(e,"path")==row.Path&&N(e,"bytes")==row.Length&&T(e,"historical_sha256")==row.Hash&&B(e,"historical_triplet_validated"),"comparison_map");
        string provenance=row.Lock?Derived:Actual;
        Need(T(e,"source_provenance")==provenance&&T(e,"source_after_provenance")==provenance&&T(e,"destination_provenance")==Actual,"comparison_provenance");
        foreach(string role in new[]{"s","bkp"}){
            JsonElement a=e.GetProperty(role);Fields(a,new[]{"initial","final","before_sha256","after_sha256","scan_before","scan_after"});
            string initial=ValidateStamp(a.GetProperty("initial"),false,row.Length,true);
            Need(initial==ValidateStamp(a.GetProperty("final"),false,row.Length,false)&&T(a,"before_sha256")==row.Hash&&T(a,"after_sha256")==row.Hash&&B(a,"scan_before")&&B(a,"scan_after"),"comparison_evidence");
            if(row.Lock){int side=row.Path.StartsWith("one/",StringComparison.Ordinal)?0:1;Need(initial==lockStamps[(role=="s"?2:4)+side],"lock_comparison");}
        }
    }
    readonly HashSet<string> namespaceNames=new HashSet<string>(11,StringComparer.Ordinal);
    void ValidateNamespace(JsonElement e,int index) {
        Fields(e,new[]{"role","path","required_parent","extra_s_directory_candidate","safe_empty_s_directory","scan_before","scan_after","initial","final"});
        string role=T(e,"role"),path=T(e,"path");bool extra=role=="s"&&path=="safe-empty";
        Need((role=="s"||role=="bkp")&&(extra||Array.IndexOf(new[]{"","one","one/state","two","two/state"},path)>=0)&&
            namespaceNames.Count<11&&namespaceNames.Add(role+":"+path),"namespace_set");
        Need(B(e,"required_parent")==!extra&&B(e,"extra_s_directory_candidate")==extra&&B(e,"safe_empty_s_directory")==extra&&B(e,"scan_before")&&B(e,"scan_after"),"namespace_evidence");
        Need(ValidateStamp(e.GetProperty("initial"),true,0,true)==ValidateStamp(e.GetProperty("final"),true,0,false),"namespace_stamp");
    }
    void ValidateObservation(JsonElement e,bool final,long expectedReceiptBytes) {
        Fields(e,ObservationFields);
        Need(T(e,"property")=="bounded_payload_equality_cooperative_owner"&&T(e,"whole_attempt_verdict")=="not_established"&&T(e,"continuous_namespace_absence")=="not_established","observation_property");
        foreach(string k in new[]{"completed","expected_set_captured_before_hash","observed_entry_sets_equal","input_handles_released"})Need(B(e,k),"observation_success");
        Need(!B(e,"cleanup_unknown")&&!B(e,"receipt_write_unknown")&&B(e,"receipts_completed")==final,"observation_cleanup");
        foreach(string k in new[]{"failure_code","failure_phase","failure_role","cause_kind"})Need(e.GetProperty(k).ValueKind==JsonValueKind.Null,"observation_failure");
        foreach(string k in new[]{"native_error","error_hresult","cleanup_native_error"})Need(N(e,k)==0,"observation_failure");
        Need(N(e,"expected_files")==Rows&&N(e,"expected_bytes")==PayloadBytes&&N(e,"locks")==6&&N(e,"hash_passes")==4&&N(e,"hash_bytes")==HashPassBytes,"observation_totals");
        foreach(string k in new[]{"historical_tuple_digest","s_before_tuple_digest","bkp_before_tuple_digest","s_after_tuple_digest","bkp_after_tuple_digest"})Need(T(e,k)==TupleHash,"observation_tuple");
        long captured=N(e,"capture_complete_timestamp");Need(captured>=factoryHigh&&captured>startQpc&&captured<deadline,"capture_time");
        if(final)captureQpc=captured;else Need(captured==captureQpc,"capture_time");
        Need(N(e,"peak_owned_handles")==13938&&N(e,"remaining_owned_handles")==(final?0:2)&&N(e,"peak_path_bytes")==563128&&
            N(e,"peak_record_bytes")==18537840&&N(e,"peak_inventory_bytes")==InventoryBytes&&N(e,"scratch_reserved_bytes")==524288,"observation_resources");
        Need(N(e,"receipt_bytes_charged")==expectedReceiptBytes&&N(e,"receipt_bytes_known_written")==expectedReceiptBytes,"observation_receipt_bytes");
        JsonElement phases=e.GetProperty("phase_seconds");Fields(phases,PhaseNames,final?14:13);
        foreach(var p in phases.EnumerateObject()){Need(p.Value.ValueKind==JsonValueKind.Number,"phase_type");double d=p.Value.GetDouble();Need(double.IsFinite(d)&&d>=0&&d<=480,"phase_range");}
        JsonElement digests=e.GetProperty("private_receipt_digests");Fields(digests,ReceiptNames,final?4:3);
        for(int i=0;i<(final?4:3);i++)Need(IsHash(T(digests,ReceiptNames[i])),"receipt_hash");
    }
    void ValidateResult(JsonElement e,int line) {
        long prior=receiptSizes[0]+receiptSizes[1]+receiptSizes[2];ValidateObservation(e,false,prior);
        JsonElement hashes=e.GetProperty("private_receipt_digests");
        for(int i=0;i<3;i++)Need(T(hashes,ReceiptNames[i])==receiptHashes[i],"receipt_hash");
    }
    public ReceiptReport VerifyReceipts(byte[] finalObservation,string approvedDecisionHash,string approvedCoreHash,
        long factoryBefore,long factoryAfter,long attemptStartTimestamp) {
        Work();Need(guardsReleased&&!receiptStarted&&finalObservation!=null&&finalObservation.Length<=RecordCap&&
            IsHash(approvedDecisionHash)&&IsHash(approvedCoreHash)&&attemptStartTimestamp>0&&
            factoryBefore>=attemptStartTimestamp&&factoryAfter>=factoryBefore&&factoryAfter<deadline,"receipt_order_or_binding");
        receiptStarted=true;facts.Stage="receipts";decisionHash=approvedDecisionHash;coreHash=approvedCoreHash;
        factoryLow=factoryBefore;factoryHigh=factoryAfter;startQpc=attemptStartTimestamp;
        identities.Clear();namespaceNames.Clear();long total=0;
        try {
            proofGuard=OpenDirectory(Proof,false);currentDirectory=null;ops.Acl(Proof,true);
            AddId(proofGuard.Initial);StrictStreams(Proof,proofGuard.Handle,true,0);EnumerateReceipts();
            for(int i=0;i<4;i++){
                string path=Proof+"\\"+ReceiptNames[i];FileOwner owner=OpenFile(path,false);
                receiptOwners[i]=owner;currentFile=null; // fixed preallocated transfer before further validation
                long size=owner.Stream.Length;Need(size>0&&size<=ReceiptCaps[i]&&size<=67108864-total,"receipt_file_cap");
                receiptSizes[i]=size;total+=size;receiptInitial[i]=HeldInfo(owner.Safe,path,false,size);AddId(receiptInitial[i]);
                ops.Acl(path,false);StrictStreams(path,owner.Safe,false,size);
            }
            utf8.GetCharCount(finalObservation);using(var final=Document(finalObservation)){ValidateObservation(final.RootElement,true,total);}
            ReadReceipt(0,ValidateOwner,1);ReadReceipt(1,ValidateComparison,Rows);ReadReceipt(2,ValidateNamespace,11);ReadReceipt(3,ValidateResult,1);
            Need(namespaceNames.Count==11&&namespaceNames.Contains("s:safe-empty")&&identities.Count==IdCap,"receipt_set");
            using(var final=Document(finalObservation)){
                var digests=final.RootElement.GetProperty("private_receipt_digests");
                for(int i=0;i<4;i++)Need(T(digests,ReceiptNames[i])==receiptHashes[i],"receipt_hash");
            }
            EnumerateReceipts();Need(proofGuard.Initial.Same(HeldInfo(proofGuard.Handle,Proof,true,0)),"receipt_directory_stamp");
            ops.Acl(Proof,true);
            for(int i=0;i<4;i++){
                string path=Proof+"\\"+ReceiptNames[i];ops.Acl(path,false);
                Need(receiptInitial[i].Same(HeldInfo(receiptOwners[i].Safe,path,false,receiptSizes[i])),"receipt_stamp");
                StrictStreams(path,receiptOwners[i].Safe,false,receiptSizes[i]);
                Need(HashFile(receiptOwners[i].Stream,receiptSizes[i])==receiptHashes[i],"receipt_final_hash");
            }
        } finally {
            CloseFile(currentFile);for(int i=3;i>=0;i--)CloseFile(receiptOwners[i]);
            CloseDirectory(currentDirectory);CloseDirectory(proofGuard);CloseEnumeration();CloseSearch();
        }
        Work();Need(FileOwners()==0&&facts.StreamSearchRemaining==0&&currentEnumeration==null,"receipt_cleanup");
        facts.ReceiptsVerified=true;facts.Stage="receipts_verified";
        return new ReceiptReport{Verified=true,Files=4,ComparisonRows=Rows,NamespaceRows=11,Bytes=total,Sizes=(long[])receiptSizes.Clone(),Digests=(string[])receiptHashes.Clone()};
    }
    public Summary GetSummary() {
        facts.FileOwnersRemaining=FileOwners();facts.DirectoryOwnersRemaining=facts.DirectoryReturns-facts.DirectoriesClosed;
        facts.EnumerationRemaining=facts.EnumerationReturns-facts.EnumerationsDisposed;facts.Disposed=disposed;
        return new Summary{Stage=facts.Stage,CreatedDirectories=facts.CreatedDirectories,CreateAttempts=facts.CreateAttempts,CreateReturns=facts.CreateReturns,
            ReadAttempts=facts.ReadAttempts,ReadReturns=facts.ReadReturns,FileCloseAttempts=facts.FileCloseAttempts,FilesClosed=facts.FilesClosed,
            FileOwnersRemaining=facts.FileOwnersRemaining,PeakFileOwners=facts.PeakFileOwners,DirectoryOpenAttempts=facts.DirectoryOpenAttempts,
            DirectoryReturns=facts.DirectoryReturns,DirectoryCloseAttempts=facts.DirectoryCloseAttempts,DirectoriesClosed=facts.DirectoriesClosed,
            DirectoryOwnersRemaining=facts.DirectoryOwnersRemaining,PeakDirectoryOwners=facts.PeakDirectoryOwners,EnumerationReturns=facts.EnumerationReturns,
            EnumerationsDisposed=facts.EnumerationsDisposed,EnumerationRemaining=facts.EnumerationRemaining,PeakEnumerationOwners=facts.PeakEnumerationOwners,
            StreamChecks=facts.StreamChecks,StreamSearchOpened=facts.StreamSearchOpened,StreamSearchClosed=facts.StreamSearchClosed,StreamSearchRemaining=facts.StreamSearchRemaining,
            BaselineFiles=facts.BaselineFiles,PreparedFiles=facts.PreparedFiles,SetupGuardsClosed=facts.SetupGuardsClosed,
            OracleRows=facts.OracleRows,PeakIdentityEntries=facts.PeakIdentityEntries,PeakQueueEntries=facts.PeakQueueEntries,
            PeakReceiptRecordBytes=facts.PeakReceiptRecordBytes,PeakTokenWindowBytes=facts.PeakTokenWindowBytes,ReadyForCore=facts.ReadyForCore,
            ReceiptsVerified=facts.ReceiptsVerified,Disposed=disposed,CleanupUnknown=facts.CleanupUnknown,PartialCreateUnknown=facts.PartialCreateUnknown,
            PartialReturnUnknown=facts.PartialReturnUnknown,CleanupFirstType=facts.CleanupFirstType,CleanupFirstHResult=facts.CleanupFirstHResult,CleanupNativeError=facts.CleanupNativeError};
    }
    public void Dispose() {
        if(disposed)return;
        CloseEnumeration();CloseSearch();CloseFile(currentFile);for(int i=3;i>=0;i--)CloseFile(receiptOwners[i]);
        CloseDirectory(currentDirectory);CloseDirectory(proofGuard);
        for(int i=caseCount-1;i>=0;i--)CloseDirectory(caseGuards[i]);
        for(int i=familyCount-1;i>=0;i--)CloseDirectory(familyGuards[i]);
        Array.Clear(io,0,io.Length);Array.Clear(payload,0,payload.Length);Array.Clear(record,0,record.Length);Array.Clear(header,0,header.Length);
        identities.Clear();map.Clear();Array.Clear(rows,0,rows.Length);Array.Clear(lockStamps,0,lockStamps.Length);
        disposed=true;facts.Stage="disposed";
    }
}
}
'@
    Add-Type -TypeDefinition $source -Language CSharp
}

