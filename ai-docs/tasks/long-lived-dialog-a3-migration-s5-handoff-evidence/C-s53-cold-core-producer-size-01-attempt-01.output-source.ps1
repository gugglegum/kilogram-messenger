# Definitions only, r50. No import, compilation, path access or automatic invocation.
function Initialize-S53ColdOutputAdapter {
    [CmdletBinding()]
    param()
    if ('Kilogram.S53.ColdOutput.BaselineSmallOwner' -as [type]) { return }
    if (-not $IsWindows) { throw 'Cold output adapter requires Windows.' }
    $source = @'
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
namespace Kilogram.S53.ColdOutput {
public sealed class BaselineSmallOwner : IDisposable {
    public const string Proof = @"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-01-attempt-02\baseline-small\proof";
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
    readonly long deadline;
    readonly string activeProof;
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
    BaselineSmallOwner(long absoluteWorkDeadline) : this(absoluteWorkDeadline,false) {}
    BaselineSmallOwner(long absoluteWorkDeadline,bool producerSize) {
        activeProof=producerSize?@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-producer-size-01-attempt-01\producer-size\proof":Proof;
        if(absoluteWorkDeadline<=Stopwatch.GetTimestamp())throw Refusal("deadline");
        deadline=absoluteWorkDeadline;
        using(var identity=WindowsIdentity.GetCurrent()) {
            if(identity.User==null)throw Refusal("owner_sid");
            user=identity.User.Value;
        }
        if(user=="S-1-5-18"||user=="S-1-5-32-544")throw Refusal("owner_sid");
    }
    // The sole binding has no path or command argument and performs no filesystem work here.
    public static BaselineSmallOwner Create(long absoluteWorkDeadline) {return new BaselineSmallOwner(absoluteWorkDeadline);}
    public static BaselineSmallOwner CreateProducerSize(long absoluteWorkDeadline) {return new BaselineSmallOwner(absoluteWorkDeadline,true);}
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
        if(String.Equals(path,activeProof,StringComparison.Ordinal))return -1;
        for(int i=0;i<Names.Length;i++)if(String.Equals(path,activeProof+"\\"+Names[i],StringComparison.Ordinal))return i;
        throw Refusal("path_scope");
    }
    public void PrepareDirectory() {
        Work();if(prepareStarted)throw Refusal("one_use");prepareStarted=true;
        var chain=new Stack<string>();string at=Path.GetDirectoryName(activeProof);
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
            bool ok=CreateDirectoryW(IoPath(activeProof),ref sa);int error=ok?0:Marshal.GetLastWin32Error();
            if(!ok)BadNative(error);
            DirectoryCreated=true; // immediate ownership, before any subsequent validation or open
        }finally{FreeDescriptor(sd);}
        directory=OpenDirectory(activeProof);AssertPrivate(activeProof,true);RecheckAncestry();prepared=true;
    }
    // Caller becomes the sole owner; BindOwnedOutput consumes it on success AND refusal.
    public SafeFileHandle TakeDirectory() {
        Work();if(!prepared||taken||directory==null)throw Refusal("ownership");
        SafeFileHandle answer=directory;directory=null;taken=true;ownedHandles--;return answer;
    }
    SafeFileHandle CreateReceipt(string path) {
        Work();int role=Role(path);if(role<0||!prepared||!taken)throw Refusal("path_scope");
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
    public void Dispose() {
        if(disposed)return;
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
    Add-Type -TypeDefinition $source -Language CSharp
}

function New-S53ColdBaselineSmallOutput {
    [CmdletBinding()]
    param([Parameter(Mandatory)][long]$WorkDeadlineTimestamp)
    [Kilogram.S53.ColdOutput.BaselineSmallOwner]::Create($WorkDeadlineTimestamp)
}

function Bind-S53ColdBaselineSmallOutput {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][object]$Core,
        [Parameter(Mandatory)][object]$Owner,
        [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$DecisionSha256,
        [Parameter(Mandatory)][ValidatePattern('\A[0-9a-f]{64}\z')][string]$CoreSha256
    )
    if($Core.GetType().FullName -cne 'Kilogram.S53.ColdA.Core' -or
       $Owner.GetType().FullName -cne 'Kilogram.S53.ColdOutput.BaselineSmallOwner') { throw 'Cold output binding types.' }
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
