# Definitions only. r44: no import, compilation, private I/O or automatic invocation.
# Initialize explicitly under a later reviewed guardian; factories do not activate a run.
function Initialize-S53ColdPreservationCore {
    [CmdletBinding()]
    param()
    if ('Kilogram.S53.ColdA.Core' -as [type]) { return }
    if (-not $IsWindows) { throw 'ColdA requires Windows.' }
    $source = @'
using System;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Buffers;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Microsoft.Win32.SafeHandles;

namespace Kilogram.S53.ColdA {
public sealed class Observation {
    public string Property = "bounded_payload_equality_cooperative_owner";
    public string WholeAttemptVerdict = "not_established";
    public string ContinuousNamespaceAbsence = "not_established";
    public bool Completed, ExpectedSetCapturedBeforeHash, ObservedEntrySetsEqual;
    public bool InputHandlesReleased, CleanupUnknown, ReceiptWriteUnknown, ReceiptsCompleted;
    public string FailureCode, FailurePhase, FailureRole, CauseKind;
    public int NativeError, ErrorHResult, CleanupNativeError, ExpectedFiles, LockCount, HashPasses;
    public long ExpectedBytes, HashBytes, CaptureCompleteTimestamp;
    public long PeakOwnedHandles, RemainingOwnedHandles, PeakPathBytes, PeakRecordBytes;
    public long PeakInventoryBytes, ScratchBytes, ReceiptBytesCharged, ReceiptBytesKnownWritten;
    public string HistoricalTupleDigest,SBeforeTupleDigest,BkpBeforeTupleDigest,SAfterTupleDigest,BkpAfterTupleDigest;
    public Dictionary<string,double> PhaseSeconds = new Dictionary<string,double>(StringComparer.Ordinal);
    public Dictionary<string,string> PrivateReceiptDigests = new Dictionary<string,string>(StringComparer.Ordinal);
    internal Observation Copy() {
        var copy=(Observation)MemberwiseClone();
        copy.PhaseSeconds=new Dictionary<string,double>(PhaseSeconds,StringComparer.Ordinal);
        copy.PrivateReceiptDigests=new Dictionary<string,string>(PrivateReceiptDigests,StringComparer.Ordinal);
        return copy;
    }
}
internal sealed class Limits {
    internal const int Inventory = 16*1024*1024, PathArena = 96*1024*1024;
    internal const int RecordArena = 32*1024*1024, Scratch = 512*1024;
    internal int Rows=8000, Entries=20000, Handles=40126;
    internal const long Payload = 64L*1024*1024;
    internal long Output=64L*1024*1024;
    internal static Limits Fixture(string id) {
        var c=new Limits();
        if(id=="cap-rows") c.Rows=2;
        if(id=="cap-directories") c.Entries=7;
        if(id=="cap-handles") c.Handles=32;
        if(id=="cap-output") c.Output=512;
        return c;
    }
}
internal sealed class Spec {
    internal const string W=@"E:\AI-Projects\kilogram-p2p-messenger";
    internal const string ProductionAttempt="P2-s53-exhausted-cold-a-01-attempt-01";
    internal string O,S,B,E,Attempt,InventoryHash;
    internal long InventoryBytes, ExpectedBytes;
    internal int ExpectedFiles;
    internal bool IsFixture;
    internal Limits Caps;
    internal static Spec Production() {
        return new Spec {
            O=W+@"\.tmp\migration-s5-l1\F-exhausted-origin-01",
            S=W+@"\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb",
            B=W+@"\.tmp\migration-s5-preserved\F-exhausted-preservation-01",
            E=W+@"\.tmp\migration-s5-preparation-evidence\"+ProductionAttempt,
            Attempt=ProductionAttempt,InventoryBytes=6765525,ExpectedFiles=6956,ExpectedBytes=45597399,
            InventoryHash="c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3",Caps=new Limits()
        };
    }
    internal static Spec Fixture(string id,long bytes,string hash,int files,long payload) {
        string[] allowed={"baseline-small","baseline-small-attempt-02","baseline-real","missing-s","missing-bkp","extra-s","extra-bkp",
            "changed-byte","invalid-path","invalid-schema","invalid-provenance","alias","ads-file","ads-directory",
            "streams-error","cap-rows","cap-directories","cap-handles","cap-output","partial-acquire",
            "writer-before","writer-after","persistent-extra","deadline","output-failure",
            "negative-a-missing-s","negative-a-missing-bkp","negative-a-extra-s","negative-a-extra-bkp","negative-a-changed-byte-s",
            "negative-b-binding-source","negative-b-triplet-digest","negative-b-forged-provenance","negative-b-duplicate-property","negative-b-escaping-path",
            "limits-file-bytes","limits-total-bytes","limits-integer-range",
            "locks-missing-first","locks-nonempty-first","locks-busy-first",
            "payload-writer-before","payload-writer-after",
            "streams-file-payload","streams-directory-payload",
            "aliases-directory-reparse","aliases-payload-hardlink",
            "namespace-extra-backup-directory","namespace-persistent-extra-state"};
        if(Array.IndexOf(allowed,id)<0 || bytes<=0 || bytes>Limits.Inventory ||
           !Core.IsHash(hash) || files<2 || files>8000 || payload<0 || payload>Limits.Payload)
            throw new ArgumentException("Invalid closed ColdA fixture binding.");
        bool freshSmall=id=="baseline-small-attempt-02";
        string root=freshSmall?W+@"\.tmp\C-s53-cold-core-01-attempt-02\baseline-small":W+@"\.tmp\C-s53-cold-core-01-attempt-01\"+id;
        string negativeLeaf=null;
        switch(id) {
            case "negative-a-missing-s": negativeLeaf="missing-s";break;
            case "negative-a-missing-bkp": negativeLeaf="missing-bkp";break;
            case "negative-a-extra-s": negativeLeaf="extra-s";break;
            case "negative-a-extra-bkp": negativeLeaf="extra-bkp";break;
            case "negative-a-changed-byte-s": negativeLeaf="changed-byte-s";break;
        }
        string negativeAttempt=negativeLeaf==null?null:"C-s53-cold-core-negative-a-01-attempt-01/"+negativeLeaf;
        if(negativeLeaf!=null)root=W+@"\.tmp\C-s53-cold-core-negative-a-01-attempt-01\"+negativeLeaf;
        string parserLeaf=null;
        switch(id) {
            case "negative-b-binding-source": parserLeaf="binding-source";break;
            case "negative-b-triplet-digest": parserLeaf="triplet-digest";break;
            case "negative-b-forged-provenance": parserLeaf="forged-provenance";break;
            case "negative-b-duplicate-property": parserLeaf="duplicate-property";break;
            case "negative-b-escaping-path": parserLeaf="escaping-path";break;
        }
        if(parserLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-negative-b-01-attempt-01\"+parserLeaf;
            negativeAttempt="C-s53-cold-core-negative-b-01-attempt-01/"+parserLeaf;
        }
        string limitLeaf=null;
        switch(id) {
            case "limits-file-bytes": limitLeaf="file-bytes";break;
            case "limits-total-bytes": limitLeaf="total-bytes";break;
            case "limits-integer-range": limitLeaf="integer-range";break;
        }
        if(limitLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-payload-limits-01-attempt-01\"+limitLeaf;
            negativeAttempt="C-s53-cold-core-payload-limits-01-attempt-01/"+limitLeaf;
        }
        string lockLeaf=null;
        switch(id) {
            case "locks-missing-first": lockLeaf="missing-lock";break;
            case "locks-nonempty-first": lockLeaf="nonempty-lock";break;
            case "locks-busy-first": lockLeaf="busy-lock";break;
        }
        if(lockLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-lock-preconditions-01-attempt-01\"+lockLeaf;
            negativeAttempt="C-s53-cold-core-lock-preconditions-01-attempt-01/"+lockLeaf;
        }
        string writerLeaf=null;
        switch(id) {
            case "payload-writer-before": writerLeaf="writer-before";break;
            case "payload-writer-after": writerLeaf="writer-after";break;
        }
        if(writerLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-payload-writers-01-attempt-01\"+writerLeaf;
            negativeAttempt="C-s53-cold-core-payload-writers-01-attempt-01/"+writerLeaf;
        }
        string streamLeaf=null;
        switch(id) {
            case "streams-file-payload": streamLeaf="file-stream";break;
            case "streams-directory-payload": streamLeaf="directory-stream";break;
        }
        if(streamLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-named-streams-01-attempt-02\"+streamLeaf;
            negativeAttempt="C-s53-cold-core-named-streams-01-attempt-02/"+streamLeaf;
        }
        string aliasLeaf=null;
        switch(id) {
            case "aliases-directory-reparse": aliasLeaf="directory-reparse";break;
            case "aliases-payload-hardlink": aliasLeaf="payload-hardlink";break;
        }
        if(aliasLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-filesystem-aliases-01-attempt-01\"+aliasLeaf;
            negativeAttempt="C-s53-cold-core-filesystem-aliases-01-attempt-01/"+aliasLeaf;
        }
        string namespaceLeaf=null;
        switch(id) {
            case "namespace-extra-backup-directory": namespaceLeaf="extra-backup-directory";break;
            case "namespace-persistent-extra-state": namespaceLeaf="persistent-extra-state";break;
        }
        if(namespaceLeaf!=null) {
            root=W+@"\.tmp\C-s53-cold-core-namespace-observations-01-attempt-01\"+namespaceLeaf;
            negativeAttempt="C-s53-cold-core-namespace-observations-01-attempt-01/"+namespaceLeaf;
        }
        return new Spec {O=root+@"\origin",S=root+@"\state",B=root+@"\backup",E=root+@"\proof",
            Attempt=negativeAttempt??(freshSmall?"C-s53-cold-core-01-attempt-02/baseline-small":"C-s53-cold-core-01-attempt-01/"+id),InventoryBytes=bytes,InventoryHash=hash,
            ExpectedFiles=files,ExpectedBytes=payload,IsFixture=true,Caps=Limits.Fixture(id)};
    }
}
internal sealed class Accounting {
    internal readonly Limits Caps;
    internal long Paths,Records,Inventory,Handles,Output;
    internal readonly Observation Facts;
    internal Accounting(Limits caps,Observation facts) {Caps=caps;Facts=facts;Facts.ScratchBytes=Limits.Scratch;}
    internal void Path(long n) { Add(ref Paths,n,Limits.PathArena,"path_arena_cap"); Facts.PeakPathBytes=Math.Max(Paths,Facts.PeakPathBytes); }
    internal void Record(long n) { Add(ref Records,n,Limits.RecordArena,"record_arena_cap"); Facts.PeakRecordBytes=Math.Max(Records,Facts.PeakRecordBytes); }
    internal void InventoryTake(long n) {Add(ref Inventory,n,Limits.Inventory,"inventory_cap");Facts.PeakInventoryBytes=Math.Max(Inventory,Facts.PeakInventoryBytes);}
    internal void HandleTake() {Add(ref Handles,1,Caps.Handles,"handle_cap");Facts.PeakOwnedHandles=Math.Max(Handles,Facts.PeakOwnedHandles);}
    internal void HandleRelease() {if(Handles<=0) Core.Refuse("ownership_accounting"); Handles--;}
    internal void OutputTake(long n) {Add(ref Output,n,Caps.Output,"output_cap");Facts.ReceiptBytesCharged=Output;}
    static void Add(ref long value,long n,long cap,string code) {
        if(n<0 || value<0 || value>cap || n>cap-value) Core.Refuse(code);
        value+=n;
    }
}
[StructLayout(LayoutKind.Sequential)]
internal struct FileInfo {
    internal uint Attributes;
    internal System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
    internal uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    internal long Length {get{return ((long)SizeHigh<<32)|SizeLow;}}
    internal Identity Id {get{return new Identity(Volume,((ulong)IndexHigh<<32)|IndexLow);}}
    internal bool Directory {get{return (Attributes&0x10u)!=0;}}
    internal bool Same(FileInfo b) {
        return Attributes==b.Attributes && Length==b.Length && Links==b.Links && Id.Equals(b.Id) &&
            Created.dwHighDateTime==b.Created.dwHighDateTime && Created.dwLowDateTime==b.Created.dwLowDateTime &&
            Written.dwHighDateTime==b.Written.dwHighDateTime && Written.dwLowDateTime==b.Written.dwLowDateTime;
    }
}
internal struct Identity : IEquatable<Identity> {
    internal uint Volume; internal ulong Index;
    internal Identity(uint v,ulong i) {Volume=v;Index=i;}
    public bool Equals(Identity b) {return Volume==b.Volume && Index==b.Index;}
    public override bool Equals(object b) {return b is Identity && Equals((Identity)b);}
    public override int GetHashCode() {return unchecked((int)Volume*397)^Index.GetHashCode();}
}
[StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]
internal struct FindData {
    internal uint Attributes;
    internal System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
    internal uint SizeHigh,SizeLow,Reserved0,Reserved1;
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=260)] internal string Name;
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=14)] internal string Alternate;
}
[StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]
internal struct StreamData {
    internal long Size;
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=296)] internal string Name;
}
internal static class Native {
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern SafeFileHandle CreateFileW(string p,uint access,uint share,IntPtr sa,uint mode,uint flags,IntPtr template);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool GetFileInformationByHandle(SafeFileHandle h,out FileInfo info);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern uint GetFileType(SafeFileHandle h);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint flags);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool SetFilePointerEx(SafeFileHandle h,long offset,out long actual,uint from);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool ReadFile(SafeFileHandle h,[Out] byte[] bytes,uint count,out uint read,IntPtr overlapped);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool WriteFile(SafeFileHandle h,IntPtr bytes,uint count,out uint written,IntPtr overlapped);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool FlushFileBuffers(SafeFileHandle h);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern IntPtr FindFirstFileW(string path,out FindData data);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern bool FindNextFileW(IntPtr h,out FindData data);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern IntPtr FindFirstStreamW(string path,int level,out StreamData data,uint flags);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    internal static extern bool FindNextStreamW(IntPtr h,out StreamData data);
    [DllImport("kernel32.dll",SetLastError=true)]
    internal static extern bool FindClose(IntPtr h);
}
internal sealed class Row {
    internal string Relative;
    internal long Bytes;
    internal byte[] Hash;
    internal int Index;
}
internal sealed class Owned {
    internal SafeFileHandle Handle;
    internal FileInfo Initial,Final;
    internal string FixedPath,Relative;
    internal int Area;
    internal bool Directory,Lock,EmptyVerified,CloseAttempted,SeenBefore,SeenAfter;
    internal byte[] BeforeHash,AfterHash;
}
internal sealed class Tree {
    internal readonly int Area;
    internal readonly string Root;
    internal readonly Owned[] Files;
    internal readonly Dictionary<string,Owned> Dirs;
    internal int EntriesBefore,EntriesAfter;
    internal bool BeforeComplete,AfterComplete;
    internal Tree(int area,string root,int rows,int entries) {
        Area=area;Root=root;Files=new Owned[rows];Dirs=new Dictionary<string,Owned>(entries,StringComparer.OrdinalIgnoreCase);
    }
}
internal sealed class FixedRecord : IBufferWriter<byte> {
    internal readonly byte[] Bytes=new byte[32768];
    internal int Used;
    public void Advance(int n) {if(n<0||n>Bytes.Length-Used) Core.Refuse("receipt_record_cap");Used+=n;}
    public Memory<byte> GetMemory(int n=0) {Check(n);return new Memory<byte>(Bytes,Used,Bytes.Length-Used);}
    public Span<byte> GetSpan(int n=0) {Check(n);return new Span<byte>(Bytes,Used,Bytes.Length-Used);}
    void Check(int n) {if(n<0||Math.Max(n,1)>Bytes.Length-Used) Core.Refuse("receipt_record_cap");}
    internal void Reset() {Array.Clear(Bytes,0,Used);Used=0;}
}
public sealed class Core : IDisposable {
    const string Actual="actual file byte read";
    const string Derived="held-lock length checked; empty-byte digest derived";
    const string OsRule="Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.";
    readonly Spec spec;
    readonly long deadline,start;
    readonly Accounting budget;
    readonly Observation facts=new Observation();
    readonly byte[] readBuffer;
    readonly StringBuilder finalPath;
    readonly FixedRecord record;
    readonly IncrementalHash payloadHash;
    readonly byte[] tupleHeader;
    readonly Owned[] live;
    int liveCount;
    readonly Dictionary<Identity,Owned> identities;
    readonly Dictionary<string,Owned> fixedOwned;
    readonly Dictionary<string,string> pool;
    readonly Dictionary<string,Row> expected;
    readonly Dictionary<string,int> parentNames;
    readonly Row[] rows;
    readonly string[] parents;
    readonly bool[] seen;
    readonly Owned[] directoryQueue;
    int rowCount,parentCount;
    Tree s,b;
    Owned inventory;
    readonly Owned[] locks=new Owned[6];
    bool captureStarted,captureFinished,disposed,inputsClosed,inventoryValidated,phaseActive,outputBoundOnce,receiptsStarted,hashClosed;
    string phase="created";
    string role="none";
    long phaseStart;
    Action<string> fixtureHook;
    SafeFileHandle outputDirectory;
    Func<string,SafeFileHandle> createRestricted;
    Action<string> assertPrivateAcl;
    string decisionHash,coreHash;
    bool outputSlot;
    Core(Spec binding,long workDeadline) {
        if(workDeadline<=Stopwatch.GetTimestamp()) throw new ArgumentException("Future monotonic work deadline required.");
        spec=binding;deadline=workDeadline;start=phaseStart=Stopwatch.GetTimestamp();
        budget=new Accounting(spec.Caps,facts);
        // Precharged, conservative table/array storage; no growth beyond fixed capacities.
        budget.Record(checked(60000L*64+spec.Caps.Handles*72L+spec.Caps.Rows*104L+spec.Caps.Entries*224L+32768));
        readBuffer=new byte[65536];finalPath=new StringBuilder(32768);record=new FixedRecord();
        tupleHeader=new byte[12];
        live=new Owned[spec.Caps.Handles];identities=new Dictionary<Identity,Owned>(spec.Caps.Handles);
        fixedOwned=new Dictionary<string,Owned>(128,StringComparer.OrdinalIgnoreCase);
        pool=new Dictionary<string,string>(60000,StringComparer.Ordinal);
        expected=new Dictionary<string,Row>(spec.Caps.Rows,StringComparer.OrdinalIgnoreCase);
        parentNames=new Dictionary<string,int>(spec.Caps.Entries,StringComparer.OrdinalIgnoreCase);
        rows=new Row[spec.Caps.Rows];parents=new string[spec.Caps.Entries];seen=new bool[spec.Caps.Rows];
        directoryQueue=new Owned[spec.Caps.Entries];
        s=new Tree(1,spec.S,spec.Caps.Rows,spec.Caps.Entries);b=new Tree(2,spec.B,spec.Caps.Rows,spec.Caps.Entries);
        facts.ExpectedFiles=spec.ExpectedFiles;facts.ExpectedBytes=spec.ExpectedBytes;
        payloadHash=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        try{facts.PhaseSeconds["allocation"]=(Stopwatch.GetTimestamp()-start)/(double)Stopwatch.Frequency;}
        catch{payloadHash.Dispose();throw;}
    }
    public static Core Production(long workDeadline) {return new Core(Spec.Production(),workDeadline);}
    public static Core Fixture(string caseId,long inventoryBytes,string inventorySha256,int files,long payloadBytes,long workDeadline) {
        return new Core(Spec.Fixture(caseId,inventoryBytes,inventorySha256,files,payloadBytes),workDeadline);
    }
    public void SetFixtureHook(Action<string> hook) {
        if(!spec.IsFixture||captureStarted) Refuse("fixture_scope");
        fixtureHook=hook;
    }
    public Observation GetObservation() {UpdateFacts();return facts.Copy();}
    public static bool IsHash(string v) {
        if(v==null||v.Length!=64)return false;
        foreach(char c in v)if(!((c>='0'&&c<='9')||(c>='a'&&c<='f')))return false;
        return true;
    }
    internal static void Refuse(string code) {
        var e=new InvalidOperationException("ColdA bounded refusal");e.Data["colda_code"]=code;throw e;
    }
    void Fail(string code) {Refuse(code);}
    void NativeFailure(int error,string special=null) {
        string code=special ?? (error==2||error==3?"input_missing":error==32||error==33?"input_busy":error==5?"unreadable":"native_error");
        var e=new InvalidOperationException("ColdA bounded native refusal",new Win32Exception(error));
        e.Data["colda_code"]=code;e.Data["colda_native_error"]=error;throw e;
    }
    void Work() {if(disposed)Fail("disposed");if(facts.CleanupUnknown)Fail("cleanup_error");if(Stopwatch.GetTimestamp()>=deadline)Fail("work_expired");}
    void Phase(string name) {
        StopPhase();phase=name;phaseStart=Stopwatch.GetTimestamp();phaseActive=true;Work();
        if(spec.IsFixture && fixtureHook!=null) fixtureHook(name);
        Work();
    }
    void StopPhase() {
        if(!phaseActive)return;
        facts.PhaseSeconds[phase]=(Stopwatch.GetTimestamp()-phaseStart)/(double)Stopwatch.Frequency;
        phaseActive=false;
    }
    string Intern(string value) {
        string got;if(pool.TryGetValue(value,out got))return got;
        if(pool.Count>=60000)Fail("path_arena_cap");
        budget.Path(checked((32L+2L*value.Length+7L)&~7L));
        pool.Add(value,value);return value;
    }
    string Absolute(Owned e) {
        return e.FixedPath ?? Join(e.Area==1?spec.S:spec.B,e.Relative);
    }
    // Only validated drive-absolute literals/contained paths reach Win32. No OS long-path setting change.
    static string IoPath(string absolute) {return @"\\?\"+absolute;}
    static string Join(string root,string rel) {return rel.Length==0?root:root+"\\"+rel.Replace('/','\\');}
    static int Depth(string value) {int n=value.Length==0?0:1;foreach(char c in value)if(c=='/')n++;return n;}
    void ValidComponent(string part) {
        if(String.IsNullOrEmpty(part)||part.Length>255)Fail("inventory_path");
        foreach(char c in part)
            if(Char.IsControl(c)||c=='/'||c=='\\'||c==':'||c=='*'||c=='?'||c=='"'||c=='<'||c=='>'||c=='|')Fail("inventory_path");
        if(part=="."||part==".."||part.EndsWith(".")||part.EndsWith(" "))Fail("inventory_path");
        int dot=part.IndexOf('.');string stem=(dot<0?part:part.Substring(0,dot)).ToUpperInvariant();
        if(stem=="CON"||stem=="PRN"||stem=="AUX"||stem=="NUL"||
           (stem.Length==4&&(stem.StartsWith("COM")||stem.StartsWith("LPT"))&&
            ((stem[3]>='1'&&stem[3]<='9')||stem[3]=='\u00b9'||stem[3]=='\u00b2'||stem[3]=='\u00b3')))Fail("inventory_path");
    }
    void ValidRelative(string value) {
        if(String.IsNullOrEmpty(value)||value.Length>1024||Depth(value)>32||value.Equals("preservation-inventory.json",StringComparison.OrdinalIgnoreCase))Fail("inventory_path");
        int begin=0;
        for(int i=0;i<=value.Length;i++) {
            if(i<value.Length&&value[i]!='/')continue;
            int n=i-begin;
            if(n==0||n>255)Fail("inventory_path");
            ValidComponent(value.Substring(begin,n));
            begin=i+1;
        }
        foreach(string root in new[]{spec.S,spec.B}) {
            string full=Join(root,value);
            if(full.Length>32760||!String.Equals(Path.GetFullPath(full),full,StringComparison.OrdinalIgnoreCase)||
               !full.StartsWith(root+"\\",StringComparison.OrdinalIgnoreCase))Fail("inventory_path");
        }
    }
    void Literal(string actual,string wanted) {
        if(actual==null||actual.IndexOf('\0')>=0)Fail("inventory_binding");
        string v=actual.Replace('/','\\');
        if(!String.Equals(v,wanted,StringComparison.OrdinalIgnoreCase)||
           !String.Equals(Path.GetFullPath(v),wanted,StringComparison.OrdinalIgnoreCase))Fail("inventory_binding");
    }
    static int Field(ref Utf8JsonReader r,string[] fields) {
        if(r.TokenType!=JsonTokenType.PropertyName)Refuse("inventory_schema");
        for(int i=0;i<fields.Length;i++)if(r.ValueTextEquals(fields[i]))return i;
        Refuse("inventory_schema");return -1;
    }
    static string StringValue(ref Utf8JsonReader r,int max) {
        if(r.TokenType!=JsonTokenType.String || r.ValueSpan.Length>max*6)Refuse("inventory_schema");
        ValidateEscapedUnicode(r.ValueSpan);
        // This bounded temporary is within the 512KiB scratch allowance, not a whole UTF-16 document.
        string value=r.GetString();
        if(value==null||value.Length>max)Refuse("inventory_schema");
        return value;
    }
    static int HexUnit(ReadOnlySpan<byte> value,int offset) {
        if(offset<0||offset>value.Length-4)Refuse("inventory_schema");
        int code=0;
        for(int n=0;n<4;n++){
            byte c=value[offset+n];int digit=c>=48&&c<=57?c-48:c>=65&&c<=70?c-55:c>=97&&c<=102?c-87:-1;
            if(digit<0)Refuse("inventory_schema");code=(code<<4)|digit;
        }
        return code;
    }
    static void ValidateEscapedUnicode(ReadOnlySpan<byte> value) {
        for(int n=0;n<value.Length;n++) {
            if(value[n]!=92)continue;
            if(++n>=value.Length)Refuse("inventory_schema");
            if(value[n]!=117)continue;
            int unit=HexUnit(value,n+1);n+=4;
            if(unit>=0xd800&&unit<=0xdbff) {
                if(n+2>=value.Length||value[n+1]!=92||value[n+2]!=117)Refuse("inventory_schema");
                int low=HexUnit(value,n+3);if(low<0xdc00||low>0xdfff)Refuse("inventory_schema");n+=6;
            } else if(unit>=0xdc00&&unit<=0xdfff)Refuse("inventory_schema");
        }
    }
    static long Number(ref Utf8JsonReader r) {
        ulong n=0;
        if(r.TokenType!=JsonTokenType.Number)Refuse("inventory_schema");
        foreach(byte digit in r.ValueSpan)if(digit<48||digit>57)Refuse("inventory_schema");
        if(!r.TryGetUInt64(out n)||n>long.MaxValue)Refuse("inventory_schema");
        return (long)n;
    }
    void ParseInventory(byte[] raw) {
        Work();new UTF8Encoding(false,true).GetCharCount(raw);Work();
        if(raw.Length>=3&&raw[0]==239&&raw[1]==187&&raw[2]==191)Fail("inventory_schema");
        var options=new JsonReaderOptions{MaxDepth=8,CommentHandling=JsonCommentHandling.Disallow,AllowTrailingCommas=false};
        var r=new Utf8JsonReader(raw,options);
        string[] names={"source","preserved","classification","held_lock_paths","os_lock_rule","source_files","source_after_files","destination_files"};
        int mask=0;int[] offsets=new int[3],lengths=new int[3];
        if(!r.Read()||r.TokenType!=JsonTokenType.StartObject)Fail("inventory_schema");
        while(r.Read()&&r.TokenType!=JsonTokenType.EndObject) {
            Work();int key=Field(ref r,names);int bit=1<<key;
            if((mask&bit)!=0)Fail("inventory_schema");mask|=bit;
            if(!r.Read())Fail("inventory_schema");
            if(key==0)Literal(StringValue(ref r,1024),spec.S);
            else if(key==1)Literal(StringValue(ref r,1024),spec.B);
            else if(key==2){if(StringValue(ref r,64)!="private-stopped-copy-never-opened")Fail("inventory_binding");}
            else if(key==3) {
                if(r.TokenType!=JsonTokenType.StartArray)Fail("inventory_schema");
                if(!r.Read())Fail("inventory_schema");Literal(StringValue(ref r,1024),spec.S+@"\one\state\.kilogram-state.lock");
                if(!r.Read())Fail("inventory_schema");Literal(StringValue(ref r,1024),spec.S+@"\two\state\.kilogram-state.lock");
                if(!r.Read()||r.TokenType!=JsonTokenType.EndArray)Fail("inventory_schema");
            } else if(key==4){if(StringValue(ref r,512)!=OsRule)Fail("inventory_provenance");}
            else {
                if(r.TokenType!=JsonTokenType.StartArray)Fail("inventory_schema");
                offsets[key-5]=checked((int)r.TokenStartIndex);r.Skip();
                lengths[key-5]=checked((int)r.BytesConsumed-offsets[key-5]);
            }
        }
        if(mask!=255||r.TokenType!=JsonTokenType.EndObject||r.Read())Fail("inventory_schema");
        for(int a=0;a<3;a++)ReadArray(raw,offsets[a],lengths[a],a,options);
        if(rowCount!=spec.ExpectedFiles||!expected.ContainsKey("one/state/.kilogram-state.lock")||
           !expected.ContainsKey("two/state/.kilogram-state.lock"))Fail("inventory_binding");
        long total=0;foreach(var row in expected.Values){if(row.Bytes>Limits.Payload-total)Fail("payload_bytes_cap");total+=row.Bytes;}
        if(total!=spec.ExpectedBytes)Fail("inventory_binding");
        Array.Sort(rows,0,rowCount,Comparer<Row>.Create((x,y)=>StringComparer.Ordinal.Compare(x.Relative,y.Relative)));
        for(int i=0;i<rowCount;i++)rows[i].Index=i;
        AddParent("");
        for(int i=0;i<rowCount;i++) {
            string path=rows[i].Relative;
            for(int at=path.LastIndexOf('/');at>=0;at=path.LastIndexOf('/',at-1)) {AddParent(path.Substring(0,at));if(at==0)break;}
        }
        Array.Sort(parents,0,parentCount,StringComparer.Ordinal);
        if((long)rowCount+parentCount>spec.Caps.Entries)Fail("entry_count_cap");
    }
    void AddParent(string value) {
        int prior;
        if(parentNames.TryGetValue(value,out prior)){
            if(!String.Equals(parents[prior],value,StringComparison.Ordinal))Fail("inventory_alias");
            return;
        }
        if(value.Length>0&&expected.ContainsKey(value))Fail("inventory_path");
        if(parentCount>=spec.Caps.Entries)Fail("directory_count_cap");
        string v=Intern(value);parentNames.Add(v,parentCount);parents[parentCount++]=v;
    }
    void ReadArray(byte[] raw,int offset,int length,int which,JsonReaderOptions options) {
        Array.Clear(seen,0,seen.Length);
        var r=new Utf8JsonReader(new ReadOnlySpan<byte>(raw,offset,length),options);
        if(!r.Read()||r.TokenType!=JsonTokenType.StartArray)Fail("inventory_schema");
        string[] fields={"path","bytes","sha256","digest_provenance"};
        int count=0;
        while(r.Read()&&r.TokenType!=JsonTokenType.EndArray) {
            Work();if(++count>spec.Caps.Rows)Fail("row_count_cap");
            if(r.TokenType!=JsonTokenType.StartObject)Fail("inventory_schema");
            int mask=0;string path=null,hash=null,provenance=null;long bytes=-1;
            while(r.Read()&&r.TokenType!=JsonTokenType.EndObject) {
                int k=Field(ref r,fields),bit=1<<k;if((mask&bit)!=0)Fail("inventory_schema");mask|=bit;
                if(!r.Read())Fail("inventory_schema");
                if(k==0)path=StringValue(ref r,1024);
                else if(k==1)bytes=Number(ref r);
                else if(k==2)hash=StringValue(ref r,64);
                else provenance=StringValue(ref r,64);
            }
            if(mask!=15||r.TokenType!=JsonTokenType.EndObject||!IsHash(hash)||bytes<0)Fail("inventory_schema");
            ValidRelative(path);if(bytes>Limits.Payload)Fail("payload_bytes_cap");
            bool isLock=path=="one/state/.kilogram-state.lock"||path=="two/state/.kilogram-state.lock";
            if((path.Equals(".kilogram-state.lock",StringComparison.OrdinalIgnoreCase)||
                path.EndsWith("/.kilogram-state.lock",StringComparison.OrdinalIgnoreCase))&&!isLock)Fail("inventory_provenance");
            if(provenance!=(isLock&&which<2?Derived:Actual))Fail("inventory_provenance");
            if(isLock&&(bytes!=0||hash!=EmptyHash))Fail("inventory_provenance");
            Row row;
            if(which==0) {
                if(expected.ContainsKey(path))Fail("inventory_alias");
                budget.Record(160);
                row=new Row{Relative=Intern(path),Bytes=bytes,Hash=Convert.FromHexString(hash),Index=rowCount};
                expected.Add(row.Relative,row);rows[rowCount++]=row;
            } else {
                if(!expected.TryGetValue(path,out row)||!String.Equals(row.Relative,path,StringComparison.Ordinal)||
                   row.Bytes!=bytes||Hex(row.Hash)!=hash)Fail("inventory_triplet");
                if(seen[row.Index])Fail("inventory_alias");seen[row.Index]=true;
            }
        }
        if(r.TokenType!=JsonTokenType.EndArray||r.Read()||(which>0&&count!=rowCount))Fail("inventory_triplet");
    }
    static readonly string EmptyHash="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    static string Hex(byte[] b) {return Convert.ToHexString(b).ToLowerInvariant();}
    static readonly HashSet<string> FailureCodes=new HashSet<string>(new[]{
        "ancestry_cap","arithmetic_overflow","cleanup_error","directory_changed","directory_count_cap",
        "directory_depth_cap","directory_extra","directory_enumeration","disposed","entry_alias","entry_count_cap",
        "entry_set_changed","file_alias","fixture_scope","handle_cap","handle_path","input_busy","input_missing",
        "inventory_alias","inventory_binding","inventory_cap","inventory_changed","inventory_digest","inventory_path",
        "inventory_provenance","inventory_schema","inventory_triplet","lock_changed","lock_missing","lock_nonempty",
        "named_stream","native_error","not_regular","object_changed","observation_unknown","observed_path","one_use",
        "output_acl","output_boundary","output_cap","output_collision","ownership_accounting","path_arena_cap",
        "payload_bytes_cap","payload_digest","payload_extra","payload_length","payload_missing","read_result",
        "receipt_file_cap","receipt_record_cap","record_arena_cap","reparse","row_count_cap","seek_result",
        "stream_changed","stream_enumeration_error","streams_unsupported","unreadable","work_expired","wrong_kind",
        "injected_failure","input_io","output_io","write_result"},StringComparer.Ordinal);
    void RememberFailure(Exception error) {
        if(facts.FailureCode!=null)return;
        Exception at=error,marked=null;
        for(int depth=0;at!=null&&depth<16;depth++,at=at.InnerException){
            string code=at.Data["colda_code"] as string;
            if(code!=null&&FailureCodes.Contains(code)){marked=at;break;}
        }
        Exception native=error;for(int depth=0;native.InnerException!=null&&depth<16;depth++)native=native.InnerException;
        var win=native as Win32Exception;
        bool output=role=="output"||role=="receipts";
        string mapped=null;
        if(marked==null&&win!=null) {
            int n=win.NativeErrorCode;
            mapped=output&&(n==80||n==183)?"output_collision":n==32||n==33?"input_busy":n==5?"unreadable":"native_error";
            facts.NativeError=n;
        }
        if(marked==null&&win==null&&native is IOException)mapped=output?"output_io":"input_io";
        if(marked==null&&native.Data.Contains("rootcopies_code")&&output&&
           (string)native.Data["rootcopies_code"]=="private_acl")mapped="output_acl";
        facts.FailureCode=marked==null?(mapped??(error is JsonException||error is DecoderFallbackException?"inventory_schema":error is OverflowException?"arithmetic_overflow":"observation_unknown")):(string)marked.Data["colda_code"];
        facts.FailurePhase=phase;facts.FailureRole=role;
        facts.ErrorHResult=native.HResult;
        facts.CauseKind=marked!=null?"bounded_refusal":error is JsonException?"json":error is OverflowException?"overflow":"unexpected";
        if(win!=null)facts.CauseKind="win32";
        else if(native is IOException)facts.CauseKind="io";
        if(marked!=null&&marked.Data["colda_native_error"] is int) {
            facts.NativeError=(int)marked.Data["colda_native_error"];facts.CauseKind="win32";
        }
    }
    Exception Normalize(Exception original) {
        var e=new InvalidOperationException("ColdA failed; inspect fixed classification.",original);
        e.Data["colda_code"]=facts.FailureCode;e.Data["colda_phase"]=facts.FailurePhase;e.Data["colda_role"]=facts.FailureRole;
        if(facts.CauseKind=="win32")e.Data["colda_native_error"]=facts.NativeError;
        return e;
    }
    void UpdateFacts() {
        facts.RemainingOwnedHandles=budget.Handles;facts.ReceiptBytesCharged=budget.Output;
    }
    bool Bad(IntPtr value) {return value==IntPtr.Zero||value==new IntPtr(-1);}
    bool CloseFile(SafeFileHandle h) {
        if(h==null)return false;
        if(h.IsClosed){facts.CleanupUnknown=true;return false;}
        if(h.IsInvalid){h.Dispose();budget.HandleRelease();return true;}
        // One explicit close makes the close result observable; never retry a recycled handle.
        bool ok=Native.CloseHandle(h.DangerousGetHandle());
        int error=ok?0:Marshal.GetLastWin32Error();
        h.SetHandleAsInvalid();h.Dispose();
        if(ok)budget.HandleRelease();
        else {facts.CleanupUnknown=true;if(facts.CleanupNativeError==0)facts.CleanupNativeError=error;}
        return ok;
    }
    void CloseFind(IntPtr h) {
        bool ok=Native.FindClose(h);int error=ok?0:Marshal.GetLastWin32Error();
        if(ok)budget.HandleRelease();
        else {facts.CleanupUnknown=true;if(facts.CleanupNativeError==0)facts.CleanupNativeError=error;}
    }
    FileInfo Info(SafeFileHandle h,string absolute,int kind) {
        Work();FileInfo info;
        bool ok=Native.GetFileInformationByHandle(h,out info);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)NativeFailure(error);Work();
        if((info.Attributes&0x400u)!=0)Fail("reparse");
        if((info.Attributes&0x40u)!=0)Fail("not_regular");
        if(kind>=0 && info.Directory!=(kind==1))Fail("wrong_kind");
        if(!info.Directory&&info.Links!=1)Fail("file_alias");
        uint type=Native.GetFileType(h);error=type==0?Marshal.GetLastWin32Error():0;
        if(type!=1){if(error!=0)NativeFailure(error,"not_regular");Fail("not_regular");}
        Work();finalPath.Clear();
        uint n=Native.GetFinalPathNameByHandleW(h,finalPath,32768,0);
        error=n==0?Marshal.GetLastWin32Error():0;
        if(n==0)NativeFailure(error);
        if(n>=32768)Fail("handle_path");
        // At most 64KiB bounded temporary; the 32Ki-character scratch is reused.
        string actual=finalPath.ToString();
        if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual.Substring(4);
        if(!String.Equals(actual.TrimEnd('\\'),absolute.TrimEnd('\\'),StringComparison.OrdinalIgnoreCase))Fail("handle_path");
        Work();return info;
    }
    void Streams(SafeFileHandle held,string absolute,FileInfo known) {
        FileInfo before=Info(held,absolute,known.Directory?1:0);
        if(!before.Same(known))Fail(known.Directory?"directory_changed":"object_changed");
        budget.HandleTake();IntPtr find=new IntPtr(-1);bool slot=true;
        try {
            Work();StreamData data;
            find=Native.FindFirstStreamW(IoPath(absolute),0,out data,0);
            int error=Bad(find)?Marshal.GetLastWin32Error():0;
            if(Bad(find)) {
                budget.HandleRelease();slot=false;
                if(!(error==38&&known.Directory))NativeFailure(error,error==87?"streams_unsupported":"stream_enumeration_error");
            } else {
                Work();
                if(known.Directory || data.Name!="::$DATA")Fail("named_stream");
                if(data.Size!=known.Length)Fail("stream_changed");
                Work();bool next=Native.FindNextStreamW(find,out data);error=next?0:Marshal.GetLastWin32Error();
                if(next)Fail("named_stream");
                if(error!=38)NativeFailure(error,"stream_enumeration_error");
                Work();
            }
        } finally {if(slot){if(!Bad(find))CloseFind(find);else budget.HandleRelease();}}
        FileInfo after=Info(held,absolute,known.Directory?1:0);
        if(!before.Same(after))Fail("object_changed");
    }
    Owned Acquire(string absolute,int area,string relative,bool directory,bool exclusive,bool isLock,bool fixedPath) {
        Work();budget.Record(384);budget.HandleTake();
        SafeFileHandle h=null;bool transferred=false;
        try {
            uint flags=0x00200000u|(directory?0x02000000u:0u);
            h=Native.CreateFileW(IoPath(absolute),directory?0x80u:0x80000000u,directory?3u:exclusive?0u:1u,IntPtr.Zero,3,flags,IntPtr.Zero);
            int error=h.IsInvalid?Marshal.GetLastWin32Error():0;
            if(h.IsInvalid)NativeFailure(error,isLock&&(error==2||error==3)?"lock_missing":null);
            Work();FileInfo info=Info(h,absolute,directory?1:0);
            if(!directory && (info.Length<0||info.Length>Limits.Payload))Fail("payload_bytes_cap");
            Owned alias;if(identities.TryGetValue(info.Id,out alias))Fail("file_alias");
            if(StreamScope(directory,absolute))Streams(h,absolute,info);
            var result=new Owned{Handle=h,Area=area,Relative=relative,FixedPath=fixedPath?absolute:null,
                Directory=directory,Lock=isLock,Initial=info,Final=info};
            if(liveCount>=live.Length)Fail("handle_cap");
            identities.Add(info.Id,result);live[liveCount++]=result;h=null;transferred=true;
            return result;
        } finally {if(h!=null)CloseFile(h);else if(!transferred)budget.HandleRelease();}
    }
    bool StreamScope(bool directory,string absolute) {
        return !directory||absolute==spec.S||absolute==spec.B||
            absolute.StartsWith(spec.S+"\\",StringComparison.OrdinalIgnoreCase)||
            absolute.StartsWith(spec.B+"\\",StringComparison.OrdinalIgnoreCase);
    }
    Owned Fixed(string absolute,bool directory,bool isLock=false) {
        Owned found;
        if(fixedOwned.TryGetValue(absolute,out found))return found;
        string stored=Intern(absolute);
        Owned item=Acquire(stored,0,null,directory,isLock,isLock,true);
        fixedOwned.Add(stored,item);return item;
    }
    void Ancestors(string target,bool includeTarget) {
        var chain=new string[64];int count=0;
        string at=includeTarget?target:Path.GetDirectoryName(target);
        while(!String.IsNullOrEmpty(at)) {
            if(count==chain.Length)Fail("ancestry_cap");
            chain[count++]=at;at=Path.GetDirectoryName(at);
        }
        for(int n=count-1;n>=0;n--)Fixed(chain[n],true);
    }
    void Seek(SafeFileHandle h) {
        Work();long actual;bool ok=Native.SetFilePointerEx(h,0,out actual,0);int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)NativeFailure(error);if(actual!=0)Fail("seek_result");Work();
    }
    uint Read(SafeFileHandle h,uint count) {
        Work();uint n;bool ok=Native.ReadFile(h,readBuffer,count,out n,IntPtr.Zero);
        int error=ok?0:Marshal.GetLastWin32Error();
        if(!ok)NativeFailure(error);if(n>count)Fail("read_result");Work();return n;
    }
    void Empty(Owned item) {
        string path=Absolute(item);FileInfo before=Info(item.Handle,path,0);
        if(before.Length!=0)Fail("lock_nonempty");
        Seek(item.Handle);if(Read(item.Handle,1)!=0)Fail("lock_nonempty");
        if(!before.Same(Info(item.Handle,path,0)))Fail("lock_changed");
        item.EmptyVerified=true;
    }
    byte[] ReadInventory() {
        FileInfo info=Info(inventory.Handle,Absolute(inventory),0);
        if(info.Length!=spec.InventoryBytes||info.Length>Limits.Inventory)Fail("inventory_binding");
        budget.InventoryTake(info.Length);
        byte[] bytes=new byte[(int)info.Length];int offset=0;
        Seek(inventory.Handle);
        while(offset<bytes.Length) {
            uint n=Read(inventory.Handle,(uint)Math.Min(readBuffer.Length,bytes.Length-offset));
            if(n==0)Fail("inventory_changed");
            Buffer.BlockCopy(readBuffer,0,bytes,offset,(int)n);offset+=(int)n;
        }
        if(Read(inventory.Handle,1)!=0||!info.Same(Info(inventory.Handle,Absolute(inventory),0)))Fail("inventory_changed");
        if(Hex(SHA256.HashData(bytes))!=spec.InventoryHash)Fail("inventory_digest");
        return bytes;
    }
    void CaptureLocks() {
        string[] paths={spec.O+@"\one.state\.kilogram-state.lock",spec.O+@"\two.state\.kilogram-state.lock",
            spec.S+@"\one\state\.kilogram-state.lock",spec.S+@"\two\state\.kilogram-state.lock",
            spec.B+@"\one\state\.kilogram-state.lock",spec.B+@"\two\state\.kilogram-state.lock"};
        string[] roles={"o_one_lock","o_two_lock","s_one_lock","s_two_lock","bkp_one_lock","bkp_two_lock"};
        for(int i=0;i<paths.Length;i++){role=roles[i];Ancestors(paths[i],false);locks[i]=Fixed(paths[i],false,true);Empty(locks[i]);facts.LockCount++;}
    }
    void CaptureTree(Tree tree) {
        role=tree.Area==1?"s":"bkp";
        for(int i=0;i<parentCount;i++) {
            Work();string rel=parents[i],absolute=Join(tree.Root,rel);Owned dir;
            if(!fixedOwned.TryGetValue(absolute,out dir))dir=Acquire(absolute,tree.Area,rel,true,false,false,false);
            if(!dir.Directory)Fail("wrong_kind");
            dir.Relative=rel;
            tree.Dirs.Add(rel,dir);
        }
        for(int i=0;i<rowCount;i++) {
            Work();Row row=rows[i];string absolute=Join(tree.Root,row.Relative);Owned item;
            if(!fixedOwned.TryGetValue(absolute,out item))item=Acquire(absolute,tree.Area,row.Relative,false,false,false,false);
            if(item.Directory||item.Initial.Length!=row.Bytes)Fail("payload_length");
            tree.Files[i]=item;
            if(spec.IsFixture&&fixtureHook!=null)fixtureHook("expected_file_acquired");
            Work();
        }
    }
    FileInfo Metadata(string absolute) {
        Work();budget.HandleTake();SafeFileHandle h=null;
        try {
            // Metadata-only reopening is solely an observed-name/ID check, never a hashing substitute.
            h=Native.CreateFileW(IoPath(absolute),0,7,IntPtr.Zero,3,0x02200000,IntPtr.Zero);
            int error=h.IsInvalid?Marshal.GetLastWin32Error():0;
            if(h.IsInvalid)NativeFailure(error);
            return Info(h,absolute,-1);
        } finally {if(h!=null)CloseFile(h);else budget.HandleRelease();}
    }
    void ValidateObservedName(string name) {
        // A component has no root-control policy: nested inventory-like names are ordinary payload.
        ValidComponent(name);
    }
    void Visit(Tree tree,string parent,string name,bool after,ref int entryCount,ref int queueTail) {
        Work();if(++entryCount>spec.Caps.Entries)Fail("entry_count_cap");
        ValidateObservedName(name);
        string rel=parent.Length==0?name:parent+"/"+name;
        if(rel.Length>1024||Depth(rel)>32)Fail("directory_depth_cap");
        string absolute=Join(tree.Root,rel);FileInfo current=Metadata(absolute);
        if(tree.Area==2&&rel=="preservation-inventory.json") {
            if(current.Directory||!current.Id.Equals(inventory.Initial.Id))Fail("inventory_changed");
            return;
        }
        if(current.Directory) {
            Owned dir;
            if(!tree.Dirs.TryGetValue(rel,out dir)) {
                if(tree.Area!=1)Fail("directory_extra");
                if(after)Fail("entry_set_changed");
                if(tree.Dirs.Count>=spec.Caps.Entries)Fail("directory_count_cap");
                string stored=Intern(rel);
                dir=Acquire(absolute,tree.Area,stored,true,false,false,false);
                if(!current.Id.Equals(dir.Initial.Id))Fail("directory_changed");
                tree.Dirs.Add(stored,dir);
            }
            if(!String.Equals(dir.Relative??(dir.FixedPath==tree.Root?"":rel),rel,StringComparison.Ordinal)||
               !current.Id.Equals(dir.Initial.Id))Fail("directory_changed");
            if(after?dir.SeenAfter:dir.SeenBefore)Fail("entry_alias");
            if(after)dir.SeenAfter=true;else dir.SeenBefore=true;
            if(queueTail>=directoryQueue.Length)Fail("directory_count_cap");
            directoryQueue[queueTail++]=dir;
        } else {
            Row row;
            if(!expected.TryGetValue(rel,out row))Fail(after?"entry_set_changed":"payload_extra");
            if(!String.Equals(row.Relative,rel,StringComparison.Ordinal))Fail("observed_path");
            Owned item=tree.Files[row.Index];
            if(item==null||!current.Id.Equals(item.Initial.Id)||!current.Same(item.Initial))Fail("object_changed");
            if(after?item.SeenAfter:item.SeenBefore)Fail("entry_alias");
            if(after)item.SeenAfter=true;else item.SeenBefore=true;
        }
    }
    void Scan(Tree tree,bool after) {
        role=tree.Area==1?"s":"bkp";
        Array.Clear(directoryQueue,0,directoryQueue.Length);
        foreach(Owned dir in tree.Dirs.Values){if(after)dir.SeenAfter=false;else dir.SeenBefore=false;}
        for(int i=0;i<rowCount;i++){if(after)tree.Files[i].SeenAfter=false;else tree.Files[i].SeenBefore=false;}
        Owned root=tree.Dirs[""];if(after)root.SeenAfter=true;else root.SeenBefore=true;
        directoryQueue[0]=root;int head=0,tail=1,count=0;
        while(head<tail) {
            Work();Owned dir=directoryQueue[head++];
            string relative=dir.FixedPath==tree.Root?"":dir.Relative;
            string absolute=Join(tree.Root,relative);
            FileInfo begin=Info(dir.Handle,absolute,1);
            if(!begin.Id.Equals(dir.Initial.Id))Fail("directory_changed");
            budget.HandleTake();IntPtr find=new IntPtr(-1);bool slot=true;
            try {
                FindData data;find=Native.FindFirstFileW(IoPath(absolute)+"\\*",out data);
                int error=Bad(find)?Marshal.GetLastWin32Error():0;
                if(Bad(find)) {
                    budget.HandleRelease();slot=false;
                    if(error!=2)NativeFailure(error,"directory_enumeration");
                } else {
                    while(true) {
                        Work();
                        if(data.Name!="."&&data.Name!="..")Visit(tree,relative,data.Name,after,ref count,ref tail);
                        Work();bool more=Native.FindNextFileW(find,out data);error=more?0:Marshal.GetLastWin32Error();
                        if(!more){if(error!=18)NativeFailure(error,"directory_enumeration");break;}
                    }
                }
            } finally {if(slot){if(!Bad(find))CloseFind(find);else budget.HandleRelease();}}
            FileInfo end=Info(dir.Handle,absolute,1);
            if(!begin.Same(end)||!dir.Initial.Same(end))Fail("directory_changed");
            dir.Final=end;
        }
        foreach(Owned dir in tree.Dirs.Values)if(!(after?dir.SeenAfter:dir.SeenBefore))Fail("entry_set_changed");
        for(int i=0;i<rowCount;i++)if(!(after?tree.Files[i].SeenAfter:tree.Files[i].SeenBefore))Fail("payload_missing");
        if(after){tree.EntriesAfter=count;if(tree.EntriesBefore!=count)Fail("entry_set_changed");}
        else tree.EntriesBefore=count;
        if(after)tree.AfterComplete=true;else tree.BeforeComplete=true;
    }
    byte[] Hash(Owned item,long length) {
        FileInfo begin=Info(item.Handle,Absolute(item),0);
        if(begin.Length!=length||!begin.Same(item.Initial))Fail("object_changed");
        Seek(item.Handle);
        {
            long bytes=0;
            while(bytes<length) {
                uint n=Read(item.Handle,(uint)Math.Min(readBuffer.Length,length-bytes));
                if(n==0)Fail("payload_length");
                payloadHash.AppendData(readBuffer,0,(int)n);bytes+=n;
            }
            if(Read(item.Handle,1)!=0)Fail("payload_length");
            FileInfo end=Info(item.Handle,Absolute(item),0);
            if(!begin.Same(end))Fail("object_changed");
            item.Final=end;return payloadHash.GetHashAndReset();
        }
    }
    void HashTree(Tree tree,bool after) {
        role=tree.Area==1?"s":"bkp";
        using(var tuple=BeginTuple()) {
            for(int i=0;i<rowCount;i++) {
                Work();Owned item=tree.Files[i];byte[] hash=Hash(item,rows[i].Bytes);
                if(!CryptographicOperations.FixedTimeEquals(hash,rows[i].Hash))Fail("payload_digest");
                if(after)item.AfterHash=hash;else item.BeforeHash=hash;
                facts.HashBytes=checked(facts.HashBytes+rows[i].Bytes);
                AppendTuple(tuple,rows[i],hash);
            }
            string digest=Hex(tuple.GetHashAndReset());
            if(tree.Area==1){if(after)facts.SAfterTupleDigest=digest;else facts.SBeforeTupleDigest=digest;}
            else {if(after)facts.BkpAfterTupleDigest=digest;else facts.BkpBeforeTupleDigest=digest;}
        }
        facts.HashPasses++;
    }
    IncrementalHash BeginTuple() {
        var hash=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.ASCII.GetBytes("ColdA-v1\0"));return hash;
    }
    void AppendTuple(IncrementalHash hash,Row row,byte[] digest) {
        Work();int n=Encoding.UTF8.GetByteCount(row.Relative);
        if(n>readBuffer.Length)Fail("path_arena_cap");
        uint count=(uint)n;ulong length=(ulong)row.Bytes;
        for(int i=0;i<4;i++)tupleHeader[i]=(byte)(count>>(i*8));
        for(int i=0;i<8;i++)tupleHeader[4+i]=(byte)(length>>(i*8));
        hash.AppendData(tupleHeader,0,4);
        Encoding.UTF8.GetBytes(row.Relative,0,row.Relative.Length,readBuffer,0);hash.AppendData(readBuffer,0,n);
        hash.AppendData(tupleHeader,4,8);hash.AppendData(digest);
    }
    string ExpectedDigest() {
        using(var tuple=BeginTuple()){
            for(int i=0;i<rowCount;i++)AppendTuple(tuple,rows[i],rows[i].Hash);
            return Hex(tuple.GetHashAndReset());
        }
    }
    void FinalInputs() {
        for(int i=0;i<liveCount;i++) {
            Owned item=live[i];string absolute=Absolute(item);role=InputRole(absolute);
            Work();FileInfo now=Info(item.Handle,absolute,item.Directory?1:0);
            bool payloadDirectory=item.Directory&&StreamScope(true,absolute);
            if(!now.Id.Equals(item.Initial.Id))Fail(item.Directory?"directory_changed":"object_changed");
            if((!item.Directory||payloadDirectory)&&!now.Same(item.Initial))Fail(item.Directory?"directory_changed":"object_changed");
            if(StreamScope(item.Directory,absolute))Streams(item.Handle,absolute,now);
            item.Final=now;
        }
        for(int i=0;i<locks.Length;i++){role=InputRole(Absolute(locks[i]));Empty(locks[i]);}
        role="inventory";
        if(Hex(Hash(inventory,spec.InventoryBytes))!=spec.InventoryHash)Fail("inventory_digest");
    }
    string InputRole(string absolute) {
        if(absolute.Equals(spec.S,StringComparison.OrdinalIgnoreCase)||absolute.StartsWith(spec.S+"\\",StringComparison.OrdinalIgnoreCase))return "s";
        if(absolute.Equals(spec.B,StringComparison.OrdinalIgnoreCase)||absolute.StartsWith(spec.B+"\\",StringComparison.OrdinalIgnoreCase))return "bkp";
        if(absolute.StartsWith(spec.O+"\\",StringComparison.OrdinalIgnoreCase))return "origin";
        return "ancestors";
    }
    void CloseInputs() {
        if(inputsClosed)return;
        for(int i=liveCount-1;i>=0;i--) {
            Owned item=live[i];
            if(item.CloseAttempted)continue;
            item.CloseAttempted=true;
            try{CloseFile(item.Handle);}catch{facts.CleanupUnknown=true;}
        }
        inputsClosed=true;facts.InputHandlesReleased=!facts.CleanupUnknown&&budget.Handles==(outputSlot?1:0);
    }
    void CloseHashState() {
        if(!hashClosed){try{payloadHash.Dispose();}catch{facts.CleanupUnknown=true;}hashClosed=true;}
        Array.Clear(readBuffer,0,readBuffer.Length);
    }
    public void CaptureAndCompare() {
        if(captureStarted||facts.FailureCode!=null)Fail("one_use");
        captureStarted=true;Exception failure=null;byte[] raw=null;
        try {
            Phase("locks");CaptureLocks();
            Phase("inventory");role="inventory";Ancestors(spec.S,true);Ancestors(spec.B,true);
            inventory=Fixed(spec.B+@"\preservation-inventory.json",false);
            raw=ReadInventory();ParseInventory(raw);inventoryValidated=true;
            facts.HistoricalTupleDigest=ExpectedDigest();Array.Clear(raw,0,raw.Length);raw=null;
            Phase("capture_s");CaptureTree(s);
            Phase("capture_bkp");CaptureTree(b);
            facts.ExpectedSetCapturedBeforeHash=true;facts.CaptureCompleteTimestamp=Stopwatch.GetTimestamp();
            Phase("scan_before");Scan(s,false);Scan(b,false);
            Phase("hash_s_before");HashTree(s,false);
            Phase("hash_bkp_before");HashTree(b,false);
            Phase("hash_s_after");HashTree(s,true);
            Phase("hash_bkp_after");HashTree(b,true);
            Phase("scan_after");Scan(s,true);Scan(b,true);
            Phase("final_inputs");FinalInputs();facts.ObservedEntrySetsEqual=true;Work();
        } catch(Exception e){RememberFailure(e);failure=Normalize(e);}
        finally {
            if(raw!=null)Array.Clear(raw,0,raw.Length);
            CloseInputs();CloseHashState();captureFinished=true;
            StopPhase();
            UpdateFacts();
        }
        if(failure!=null)ExceptionDispatchInfo.Capture(failure).Throw();
        if(facts.CleanupUnknown||facts.FailureCode!=null) {
            try{Fail("cleanup_error");}catch(Exception e){RememberFailure(e);throw Normalize(e);}
        }
        facts.Completed=true;
    }
    public void BindOwnedOutput(SafeFileHandle ownedNewDirectory,Func<string,SafeFileHandle> creator,Action<string> aclVerifier,string approvedDecisionHash,string approvedCoreHash) {
        // Consumes the caller's directory handle even on refusal. Only the future launcher proves CreateNew E ownership.
        // Creator transfers sole ownership of a fresh synchronous read/write SafeFileHandle:
        // restricted CREATE_NEW E/name, no FileStream or other concurrent/finalizing handle owner.
        bool reserved=false,transferred=false;
        try {
            role="output";Phase("output_binding");
            if(outputBoundOnce||outputDirectory!=null||disposed||ownedNewDirectory==null||creator==null||aclVerifier==null||
               !IsHash(approvedDecisionHash)||!IsHash(approvedCoreHash))Fail("output_boundary");
            outputBoundOnce=true;
            budget.HandleTake();reserved=true;
            Info(ownedNewDirectory,spec.E,1);aclVerifier(spec.E);
            outputDirectory=ownedNewDirectory;createRestricted=creator;assertPrivateAcl=aclVerifier;
            decisionHash=approvedDecisionHash;coreHash=approvedCoreHash;outputSlot=true;transferred=true;
        } catch(Exception e){RememberFailure(e);throw Normalize(e);}
        finally {
            if(!transferred&&ownedNewDirectory!=null) {
                if(reserved)CloseFile(ownedNewDirectory);
                else ownedNewDirectory.Dispose();
            }
            StopPhase();
        }
    }
    void StampJson(Utf8JsonWriter w,string name,FileInfo info) {
        w.WriteStartObject(name);
        w.WriteNumber("volume",info.Volume);w.WriteString("file_id",info.Id.Index.ToString("x16"));
        w.WriteNumber("attributes",info.Attributes);w.WriteNumber("bytes",info.Length);w.WriteNumber("links",info.Links);
        w.WriteNumber("created_high",info.Created.dwHighDateTime);w.WriteNumber("created_low",info.Created.dwLowDateTime);
        w.WriteNumber("written_high",info.Written.dwHighDateTime);w.WriteNumber("written_low",info.Written.dwLowDateTime);
        w.WriteEndObject();
    }
    void ObservationJson(Utf8JsonWriter w) {
        UpdateFacts();w.WriteStartObject();
        w.WriteString("property",facts.Property);w.WriteString("whole_attempt_verdict","not_established");
        w.WriteString("continuous_namespace_absence","not_established");
        w.WriteBoolean("completed",facts.Completed);
        w.WriteBoolean("expected_set_captured_before_hash",facts.ExpectedSetCapturedBeforeHash);
        w.WriteBoolean("observed_entry_sets_equal",facts.ObservedEntrySetsEqual);
        w.WriteBoolean("input_handles_released",facts.InputHandlesReleased);w.WriteBoolean("cleanup_unknown",facts.CleanupUnknown);
        w.WriteBoolean("receipts_completed",facts.ReceiptsCompleted);w.WriteNumber("cleanup_native_error",facts.CleanupNativeError);
        w.WriteString("failure_code",facts.FailureCode);w.WriteString("failure_phase",facts.FailurePhase);
        w.WriteString("failure_role",facts.FailureRole);
        w.WriteString("cause_kind",facts.CauseKind);w.WriteNumber("native_error",facts.NativeError);
        w.WriteNumber("error_hresult",facts.ErrorHResult);
        w.WriteNumber("expected_files",facts.ExpectedFiles);w.WriteNumber("expected_bytes",facts.ExpectedBytes);
        w.WriteNumber("locks",facts.LockCount);w.WriteNumber("hash_passes",facts.HashPasses);w.WriteNumber("hash_bytes",facts.HashBytes);
        w.WriteNumber("capture_complete_timestamp",facts.CaptureCompleteTimestamp);
        w.WriteString("historical_tuple_digest",facts.HistoricalTupleDigest);
        w.WriteString("s_before_tuple_digest",facts.SBeforeTupleDigest);w.WriteString("bkp_before_tuple_digest",facts.BkpBeforeTupleDigest);
        w.WriteString("s_after_tuple_digest",facts.SAfterTupleDigest);w.WriteString("bkp_after_tuple_digest",facts.BkpAfterTupleDigest);
        w.WriteNumber("peak_owned_handles",facts.PeakOwnedHandles);w.WriteNumber("remaining_owned_handles",facts.RemainingOwnedHandles);
        w.WriteNumber("peak_path_bytes",facts.PeakPathBytes);w.WriteNumber("peak_record_bytes",facts.PeakRecordBytes);
        w.WriteNumber("peak_inventory_bytes",facts.PeakInventoryBytes);w.WriteNumber("scratch_reserved_bytes",facts.ScratchBytes);
        w.WriteNumber("receipt_bytes_charged",facts.ReceiptBytesCharged);w.WriteNumber("receipt_bytes_known_written",facts.ReceiptBytesKnownWritten);
        w.WriteBoolean("receipt_write_unknown",facts.ReceiptWriteUnknown);
        w.WriteStartObject("phase_seconds");
        foreach(var phaseTime in facts.PhaseSeconds)w.WriteNumber(phaseTime.Key,phaseTime.Value);
        w.WriteEndObject();
        w.WriteStartObject("private_receipt_digests");
        foreach(var digest in facts.PrivateReceiptDigests)w.WriteString(digest.Key,digest.Value);
        w.WriteEndObject();w.WriteEndObject();
    }
    public byte[] PublicObservationUtf8() {
        // Fixed aggregate-only serialization; never includes an input name/ID/hash/absolute path.
        record.Reset();
        using(var writer=new Utf8JsonWriter(record)){ObservationJson(writer);writer.Flush();}
        byte[] result=new byte[record.Used];Buffer.BlockCopy(record.Bytes,0,result,0,result.Length);return result;
    }
    sealed class Receipt : IDisposable {
        readonly Core owner;
        readonly string name;
        readonly long cap;
        SafeFileHandle handle;
        GCHandle pinnedRecord;
        IncrementalHash digest;
        long charged;
        bool slot,complete,closed;
        internal Receipt(Core owner,string name,long cap) {
            this.owner=owner;this.name=name;this.cap=cap;
            try {
                owner.Work();owner.budget.HandleTake();slot=true;
                digest=IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
                string absolute=owner.spec.E+"\\"+name;
                handle=owner.createRestricted(absolute);
                if(handle==null||handle.IsInvalid||handle.IsClosed)owner.Fail("output_boundary");
                FileInfo info=owner.Info(handle,absolute,0);
                if(info.Length!=0)owner.Fail("output_boundary");
                owner.assertPrivateAcl(absolute);owner.Seek(handle);
                // Pin only the already bounded shared record buffer; synchronous writes finish before reuse/free.
                pinnedRecord=GCHandle.Alloc(owner.record.Bytes,GCHandleType.Pinned);
            } catch {Close();throw;}
        }
        void WriteBytes(int offset,int count) {
            while(count>0) {
                owner.Work();uint written;
                bool ok=Native.WriteFile(handle,IntPtr.Add(pinnedRecord.AddrOfPinnedObject(),offset),(uint)count,out written,IntPtr.Zero);
                int error=ok?0:Marshal.GetLastWin32Error();
                if(!ok){owner.facts.ReceiptWriteUnknown=true;owner.NativeFailure(error,"output_io");}
                if(written==0||written>(uint)count){owner.facts.ReceiptWriteUnknown=true;owner.Fail("write_result");}
                owner.facts.ReceiptBytesKnownWritten+=written;
                offset+=(int)written;count-=(int)written;owner.Work();
            }
        }
        internal void Record(Action<Utf8JsonWriter> build) {
            owner.Work();owner.record.Reset();
            using(var w=new Utf8JsonWriter(owner.record)){build(w);w.Flush();}
            long size=(long)owner.record.Used+1;
            if(size>cap-charged)owner.Fail("receipt_file_cap");
            owner.budget.OutputTake(size);charged+=size;
            try {
                WriteBytes(0,owner.record.Used);
                digest.AppendData(owner.record.Bytes,0,owner.record.Used);
                owner.record.Bytes[0]=10;WriteBytes(0,1);digest.AppendData(owner.record.Bytes,0,1);
            } catch {owner.facts.ReceiptWriteUnknown=true;throw;}
            owner.Work();
        }
        internal void Complete() {
            owner.Work();bool ok=Native.FlushFileBuffers(handle);int error=ok?0:Marshal.GetLastWin32Error();
            if(!ok)owner.NativeFailure(error,"output_io");
            owner.Work();
            if(owner.Info(handle,owner.spec.E+"\\"+name,0).Length!=charged)owner.Fail("write_result");
            complete=true;
        }
        void Close() {
            if(closed)return;closed=true;bool nativeClosed=false;
            try {
                if(handle!=null) {
                    nativeClosed=owner.CloseFile(handle);
                    if(nativeClosed)slot=false; // CloseFile releases the slot only on its observed success.
                } else if(slot) {owner.budget.HandleRelease();slot=false;}
            } catch {owner.facts.CleanupUnknown=true;}
            finally {
                if(slot)owner.facts.CleanupUnknown=true;
                try{if(pinnedRecord.IsAllocated)pinnedRecord.Free();}catch{owner.facts.CleanupUnknown=true;}
                try{if(complete&&nativeClosed)owner.facts.PrivateReceiptDigests[name]=Hex(digest.GetHashAndReset());}
                catch{owner.facts.CleanupUnknown=true;}
                finally{if(digest!=null){try{digest.Dispose();}catch{owner.facts.CleanupUnknown=true;}}}
            }
        }
        public void Dispose(){Close();}
    }
    void WriteOwner() {
        using(var output=new Receipt(this,"owner.private.json",65536)) {
            output.Record(w=>{
                w.WriteStartObject();w.WriteString("classification","cold-a-private-owner-r44");
                w.WriteString("attempt",spec.Attempt);w.WriteString("decision_sha256",decisionHash);w.WriteString("core_sha256",coreHash);
                w.WriteBoolean("activation_and_new_e_ownership_verified_by_core",false);
                w.WriteBoolean("external_guardian_required",true);
                w.WriteNumber("core_start_timestamp",start);w.WriteNumber("work_deadline_timestamp",deadline);
                w.WriteString("source",spec.S);w.WriteString("preserved",spec.B);w.WriteString("output",spec.E);
                w.WriteNumber("inventory_bytes",spec.InventoryBytes);w.WriteString("inventory_sha256",spec.InventoryHash);
                w.WriteBoolean("historical_inventory_validated",inventoryValidated);
                w.WriteNumber("expected_files",spec.ExpectedFiles);w.WriteNumber("expected_bytes",spec.ExpectedBytes);
                w.WriteStartArray("locks");
                foreach(Owned item in locks)if(item!=null) {
                    w.WriteStartObject();w.WriteString("path",Absolute(item));
                    w.WriteBoolean("verified_empty",item.EmptyVerified);
                    w.WriteString("provenance",item.EmptyVerified?"verified-empty-held-handle":"not-established");w.WriteNumber("read_bytes",0);
                    w.WriteBoolean("empty_hash_derived",item.EmptyVerified);StampJson(w,"initial",item.Initial);w.WriteEndObject();
                }
                w.WriteEndArray();w.WriteEndObject();
            });
            output.Complete();
        }
    }
    void WriteComparison() {
        using(var output=new Receipt(this,"comparison.private.jsonl",32L*1024*1024)) {
            for(int i=0;i<rowCount;i++) {
                Work();Row row=rows[i];Owned sa=s.Files[i],ba=b.Files[i];
                output.Record(w=>{
                    w.WriteStartObject();w.WriteString("path",row.Relative);w.WriteNumber("bytes",row.Bytes);
                    w.WriteString("historical_sha256",Hex(row.Hash));
                    w.WriteBoolean("historical_triplet_validated",inventoryValidated);
                    bool isLock=row.Relative=="one/state/.kilogram-state.lock"||row.Relative=="two/state/.kilogram-state.lock";
                    w.WriteString("source_provenance",isLock?Derived:Actual);
                    w.WriteString("source_after_provenance",inventoryValidated?(isLock?Derived:Actual):null);
                    w.WriteString("destination_provenance",inventoryValidated?Actual:null);
                    WriteObjectEvidence(w,"s",sa);WriteObjectEvidence(w,"bkp",ba);w.WriteEndObject();
                });
            }
            output.Complete();
        }
    }
    void WriteObjectEvidence(Utf8JsonWriter w,string name,Owned item) {
        if(item==null){w.WriteNull(name);return;}
        w.WriteStartObject(name);StampJson(w,"initial",item.Initial);StampJson(w,"final",item.Final);
        w.WriteString("before_sha256",item.BeforeHash==null?null:Hex(item.BeforeHash));
        w.WriteString("after_sha256",item.AfterHash==null?null:Hex(item.AfterHash));
        w.WriteBoolean("scan_before",item.SeenBefore);w.WriteBoolean("scan_after",item.SeenAfter);w.WriteEndObject();
    }
    void WriteNamespace() {
        using(var output=new Receipt(this,"namespace.private.jsonl",16L*1024*1024)) {
            foreach(Tree tree in new[]{s,b}) {
                foreach(var pair in tree.Dirs) {
                    Work();Owned dir=pair.Value;string relative=pair.Key;
                    output.Record(w=>{
                        w.WriteStartObject();w.WriteString("role",tree.Area==1?"s":"bkp");w.WriteString("path",relative);
                        w.WriteBoolean("required_parent",parentNames.ContainsKey(relative));
                        w.WriteBoolean("extra_s_directory_candidate",tree.Area==1&&!parentNames.ContainsKey(relative));
                        w.WriteBoolean("safe_empty_s_directory",tree.Area==1&&!parentNames.ContainsKey(relative)&&tree.BeforeComplete&&tree.AfterComplete);
                        w.WriteBoolean("scan_before",dir.SeenBefore);w.WriteBoolean("scan_after",dir.SeenAfter);
                        StampJson(w,"initial",dir.Initial);StampJson(w,"final",dir.Final);w.WriteEndObject();
                    });
                }
            }
            output.Complete();
        }
    }
    public void WriteReceipts() {
        if(receiptsStarted||!captureFinished||disposed||outputDirectory==null||createRestricted==null||assertPrivateAcl==null)Fail("output_boundary");
        receiptsStarted=true;
        try {
            role="receipts";Phase("receipts");Info(outputDirectory,spec.E,1);assertPrivateAcl(spec.E);
            WriteOwner();WriteComparison();WriteNamespace();
            using(var output=new Receipt(this,"result.private.json",65536)) {
                output.Record(ObservationJson);output.Complete();
            }
            facts.ReceiptsCompleted=true;
        } catch(Exception e) {
            RememberFailure(e);throw Normalize(e);
        } finally {
            if(outputDirectory!=null){CloseFile(outputDirectory);outputDirectory=null;outputSlot=false;}
            createRestricted=null;assertPrivateAcl=null;
            StopPhase();
            UpdateFacts();
        }
        if(facts.CleanupUnknown){facts.ReceiptsCompleted=false;try{Fail("cleanup_error");}catch(Exception e){RememberFailure(e);throw Normalize(e);}}
    }
    public void Dispose() {
        if(disposed)return;
        CloseInputs();
        if(outputDirectory!=null){CloseFile(outputDirectory);outputDirectory=null;outputSlot=false;}
        createRestricted=null;assertPrivateAcl=null;
        Array.Clear(readBuffer,0,readBuffer.Length);record.Reset();finalPath.Clear();
        for(int i=0;i<rowCount;i++)if(rows[i]!=null&&rows[i].Hash!=null)Array.Clear(rows[i].Hash,0,rows[i].Hash.Length);
        for(int i=0;i<liveCount;i++) {
            if(live[i].BeforeHash!=null)Array.Clear(live[i].BeforeHash,0,live[i].BeforeHash.Length);
            if(live[i].AfterHash!=null)Array.Clear(live[i].AfterHash,0,live[i].AfterHash.Length);
        }
        CloseHashState();disposed=true;UpdateFacts();
    }
}
}
'@
    # Explicit future compilation uses the fixed shell's supported adjacent ref directory.
    # Omit ReferencedAssemblies so Add-Type selects its complete default reference set.
    Add-Type -TypeDefinition $source -Language CSharp
}

function New-S53ColdAProductionCore {
    [CmdletBinding()]
    param([Parameter(Mandatory)][long]$WorkDeadlineTimestamp)
    [Kilogram.S53.ColdA.Core]::Production($WorkDeadlineTimestamp)
}

function New-S53ColdAFixtureCore {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateSet('baseline-small','baseline-small-attempt-02','baseline-real','missing-s','missing-bkp','extra-s','extra-bkp',
            'changed-byte','invalid-path','invalid-schema','invalid-provenance','alias','ads-file','ads-directory',
            'streams-error','cap-rows','cap-directories','cap-handles','cap-output','partial-acquire',
            'writer-before','writer-after','persistent-extra','deadline','output-failure',
            'negative-a-missing-s','negative-a-missing-bkp','negative-a-extra-s','negative-a-extra-bkp','negative-a-changed-byte-s',
            'negative-b-binding-source','negative-b-triplet-digest','negative-b-forged-provenance','negative-b-duplicate-property','negative-b-escaping-path',
            'limits-file-bytes','limits-total-bytes','limits-integer-range',
            'locks-missing-first','locks-nonempty-first','locks-busy-first',
            'payload-writer-before','payload-writer-after',
            'streams-file-payload','streams-directory-payload',
            'aliases-directory-reparse','aliases-payload-hardlink',
            'namespace-extra-backup-directory','namespace-persistent-extra-state')]
        [string]$Case,
        [Parameter(Mandatory)][long]$InventoryBytes,
        [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$InventorySha256,
        [Parameter(Mandatory)][int]$ExpectedFiles,
        [Parameter(Mandatory)][long]$ExpectedPayloadBytes,
        [Parameter(Mandatory)][long]$WorkDeadlineTimestamp
    )
    [Kilogram.S53.ColdA.Core]::Fixture($Case,$InventoryBytes,$InventorySha256,$ExpectedFiles,$ExpectedPayloadBytes,$WorkDeadlineTimestamp)
}
