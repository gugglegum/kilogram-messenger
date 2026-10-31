# Definitions only. C2-S source submission; no compilation, I/O or automatic activation.
function Initialize-S53WrapperFixtureSupportR01 {
    [CmdletBinding()]
    param()
    if ('Kilogram.S53.WrapperR01.FixtureSupport' -as [type]) { return }
    if (-not $IsWindows) { throw 'C2S requires Windows.' }
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
namespace Kilogram.S53.WrapperR01 {
public sealed class FixtureSupport : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Info {
        public uint Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME Created,Accessed,Written;
        public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
    }
    [StructLayout(LayoutKind.Sequential)] struct SA {
        public int Length;public IntPtr Descriptor;[MarshalAs(UnmanagedType.Bool)] public bool Inherit;
    }
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern SafeFileHandle CreateFileW(string p,uint a,uint s,IntPtr sa,uint c,uint f,IntPtr t);
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
    public const string Family=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-output-wrapper-01-attempt-01";
    readonly string[] roots={@"E:\",@"E:\AI-Projects",@"E:\AI-Projects\kilogram-p2p-messenger",
        @"E:\AI-Projects\kilogram-p2p-messenger\.tmp",Family,Family+@"\healthy",Family+@"\output-acl"};
    readonly SafeFileHandle[] guards=new SafeFileHandle[7];
    readonly Info[] ids=new Info[7];
    readonly string[] acls=new string[7];
    readonly long deadline;
    readonly string user;
    readonly StringBuilder finalPath=new StringBuilder(32768);
    string activeProof,groupBefore,beforeAcl,afterAcl,firstIdentity,secondIdentity;
    SafeFileHandle editor;
    Info eIdentity;
    byte[] userAce,systemAce;
    IntPtr search=new IntPtr(-1);
    int ownedHandles,peakHandles,createdDirectories,closeCalls,closedCalls,emptyChecks,findOpened,findClosed,editorOpens;
    int pinCalls,pinFrees,daclQueries,validAclCalls,setCalls,nextCase;
    long setResult=-1; // preserves the uint API result and the pre-call sentinel
    bool disposed,prepared,caseActive,narrowStarted,narrowed,postVerified,pinOwned,daclPresent,daclNonNull,daclValid;
    bool emptyVerified,aceBytesPreserved,ownerGroupPreserved;
    long narrowBefore,narrowAfter,postQpc;
    public bool CleanupUnknown {get;private set;}
    public int CleanupNativeError {get;private set;}
    public int RemainingOwnedHandles {get{return ownedHandles;}}
    public FixtureSupport(long absoluteWorkDeadline) {
        if(absoluteWorkDeadline<=Stopwatch.GetTimestamp())throw Refusal("deadline");
        deadline=absoluteWorkDeadline;
        using(var identity=WindowsIdentity.GetCurrent()) {if(identity.User==null)throw Refusal("owner_sid");user=identity.User.Value;}
        if(user=="S-1-5-18"||user=="S-1-5-32-544")throw Refusal("owner_sid");
    }
    static Exception Refusal(string code) {
        var e=new InvalidOperationException("Cold output bounded refusal");e.Data["c2s_support_code"]=code;
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
            if(h.IsInvalid)BadNative(error);ownedHandles++;peakHandles=Math.Max(peakHandles,ownedHandles);
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
    public void PrepareRoots() {
        Work();if(prepared)throw Refusal("one_use");
        prepared=true; // no retry even on a partial CreateNew failure
        for(int i=0;i<roots.Length;i++){
            Work();
            if(i>=4) {
                IntPtr sd=IntPtr.Zero;
                try {
                    var sa=Security(out sd);bool ok=CreateDirectoryW(IoPath(roots[i]),ref sa);
                    int error=ok?0:Marshal.GetLastWin32Error();if(!ok)BadNative(error);createdDirectories++;
                } finally {FreeDescriptor(sd);}
            }
            guards[i]=OpenDirectory(roots[i]);ids[i]=Check(guards[i],roots[i],true);acls[i]=AclSnapshot(roots[i]);
            if(i>=4)AssertPrivate(roots[i],true);
        }
        RecheckAncestry();
    }
    public void RecheckAncestry() {
        Work();for(int i=0;i<roots.Length;i++)
            if(guards[i]==null||!SameId(ids[i],Check(guards[i],roots[i],true))||acls[i]!=AclSnapshot(roots[i]))throw Refusal("ancestry_changed");
    }
    static string Identity(Info i){return i.Volume.ToString("x8")+":"+i.IndexHigh.ToString("x8")+i.IndexLow.ToString("x8");}
    static string Digest(string s){return Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(s))).ToLowerInvariant();}
    void OpenEditor(bool write) {
        Work();if(editor!=null)throw Refusal("editor_owned");
        SafeFileHandle current=null;
        try {
            editorOpens++;current=CreateFileW(IoPath(activeProof),write?0x00060080u:0x80u,3,IntPtr.Zero,3,0x02200000u,IntPtr.Zero);
            int error=current.IsInvalid?Marshal.GetLastWin32Error():0;if(current.IsInvalid)BadNative(error);
            ownedHandles++;peakHandles=Math.Max(peakHandles,ownedHandles);
            Info now=Check(current,activeProof,true);
            if(caseActive&&!SameId(now,eIdentity))throw Refusal("e_identity");
            editor=current;current=null;eIdentity=now;
        }finally{if(current!=null)CloseOwned(current);}
    }
    public void BeginCase(string name) {
        Work();if(!prepared||caseActive||nextCase>=2||name!=(nextCase==0?"healthy":"output-acl"))throw Refusal("case_order");
        activeProof=Family+"\\"+name+@"\proof";OpenEditor(name=="output-acl");caseActive=true;
        if(name=="healthy")firstIdentity=Identity(eIdentity);else secondIdentity=Identity(eIdentity);
        AssertPrivate(activeProof,true);EmptyDirectory();beforeAcl=Digest(AclSnapshot(activeProof));
        if(name=="healthy"){if(!CloseOwned(editor))throw Refusal("editor_cleanup");editor=null;}
        else NarrowNewEmptyE();
        RecheckAncestry();
    }
    public void EndCase() {
        Work();if(!caseActive)throw Refusal("case_order");
        if(nextCase==0){OpenEditor(false);AssertPrivate(activeProof,true);}else VerifyRestricted();
        EmptyDirectory();afterAcl=Digest(AclSnapshot(activeProof));RecheckAncestry();
        postVerified=true;postQpc=Stopwatch.GetTimestamp();
        if(!CloseOwned(editor))throw Refusal("editor_cleanup");editor=null;
        caseActive=false;nextCase++;
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
        var acl=Acl(activeProof,true);byte[] raw=acl.GetSecurityDescriptorBinaryForm();
        if(raw.Length<=0||raw.Length>16384)throw Refusal("descriptor_cap");
        return new RawSecurityDescriptor(raw,0);
    }
    void VerifyIdentity(SafeFileHandle h){if(!SameId(eIdentity,Check(h,activeProof,true)))throw Refusal("e_identity");}
    void EmptyDirectory(){
        Work();emptyChecks++;if(emptyChecks>4||search!=new IntPtr(-1))throw Refusal("one_use");
        VerifyIdentity(editor);FindData data;
        try{
            search=FindFirstFileW(IoPath(activeProof)+@"\*",out data);int error=search==new IntPtr(-1)?Marshal.GetLastWin32Error():0;
            if(search==IntPtr.Zero)throw Refusal("search_handle");
            if(search==new IntPtr(-1)){if(error!=2)BadNative(error);}
            else{
                findOpened++;ownedHandles++;peakHandles=Math.Max(peakHandles,ownedHandles);int dotMask=0;
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
        if(CleanupUnknown)throw Refusal("empty_cleanup");VerifyIdentity(editor);emptyVerified=true;
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
        Work();if(!caseActive||nextCase!=1||editor==null||narrowStarted)throw Refusal("narrow_boundary");
        narrowStarted=true;narrowBefore=Stopwatch.GetTimestamp();AssertPrivate(activeProof,true);
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
    public Dictionary<string,object> Snapshot(){return new Dictionary<string,object>{
        {"created_directories",createdDirectories},{"root_guards",guards.Length},{"remaining_owned_handles",ownedHandles},{"peak_owned_handles",peakHandles},
        {"close_calls",closeCalls},{"closed_calls",closedCalls},{"empty_checks",emptyChecks},{"empty_verified",emptyVerified},
        {"find_opened",findOpened},{"find_closed",findClosed},{"search_owned",search!=new IntPtr(-1)&&search!=IntPtr.Zero},
        {"editor_opens",editorOpens},{"editor_owned",editor!=null},{"pin_calls",pinCalls},{"pin_frees",pinFrees},{"pin_owned",pinOwned},
        {"dacl_queries",daclQueries},{"valid_acl_calls",validAclCalls},{"set_calls",setCalls},{"set_result",setResult},
        {"dacl_present",daclPresent},{"dacl_nonnull",daclNonNull},{"dacl_valid",daclValid},
        {"ace_bytes_preserved",aceBytesPreserved},{"owner_group_preserved",ownerGroupPreserved},
        {"user_ace_hex",userAce==null?null:Convert.ToHexString(userAce)},{"system_ace_hex",systemAce==null?null:Convert.ToHexString(systemAce)},
        {"before_acl_sha256",beforeAcl},{"after_acl_sha256",afterAcl},{"healthy_identity",firstIdentity},{"acl_identity",secondIdentity},
        {"narrowed",narrowed},{"post_verified",postVerified},{"completed_cases",nextCase},
        {"narrow_before_qpc",narrowBefore},{"narrow_after_qpc",narrowAfter},{"post_qpc",postQpc},
        {"cleanup_unknown",CleanupUnknown},{"cleanup_native_error",CleanupNativeError},{"disposed",disposed}
    };}
    public void Dispose() {
        if(disposed)return;
        if(editor!=null){try{CloseOwned(editor);}catch{CleanupUnknown=true;}finally{editor=null;}}
        for(int i=guards.Length-1;i>=0;i--)if(guards[i]!=null){
            try{CloseOwned(guards[i]);}catch{CleanupUnknown=true;}finally{guards[i]=null;}
        }
        disposed=true;if(ownedHandles!=0||pinOwned||search!=new IntPtr(-1))CleanupUnknown=true;
    }
}
}
'@
    Add-Type -TypeDefinition $source -Language CSharp
}
