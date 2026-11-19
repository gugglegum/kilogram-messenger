using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;

// Read-only identity/network helpers. No process launch, policy write or privilege enabling.
public static class KilogramLabNative
{
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int id);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetProcessTimes(IntPtr p,out long created,out long exit,out long kernel,out long user);
    [DllImport("kernel32.dll", SetLastError=true,CharSet=CharSet.Unicode)] static extern bool QueryFullProcessImageName(IntPtr p,uint flags,StringBuilder name,ref int size);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr p,out uint code);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr p,uint access,out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool DuplicateTokenEx(IntPtr token,uint access,IntPtr attributes,int level,int type,out IntPtr copy);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token,int kind,IntPtr data,int size,out int needed);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool AccessCheck(byte[] descriptor,IntPtr token,uint access,ref Mapping mapping,IntPtr privileges,ref uint size,out uint granted,out bool allowed);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disabled,IntPtr disabledSids,uint deleted,IntPtr deletedPrivileges,uint restricted,IntPtr restrictedSids,out IntPtr result);
    [StructLayout(LayoutKind.Sequential)] struct Mapping { public uint Read,Write,Execute,All; }
    [StructLayout(LayoutKind.Sequential)] struct SidAttributes { public IntPtr Sid; public uint Attributes; }
    public sealed class Group { public string sid; public uint attributes; }
    static IntPtr Information(IntPtr token,int kind) {
        int needed; GetTokenInformation(token,kind,IntPtr.Zero,0,out needed);
        if(needed<=0 || needed>65536) throw new InvalidOperationException("Token information bound");
        IntPtr p=Marshal.AllocHGlobal(needed);
        if(!GetTokenInformation(token,kind,p,needed,out needed)) { int e=Marshal.GetLastWin32Error();Marshal.FreeHGlobal(p);throw new Win32Exception(e); }
        return p;
    }
    static int Integer(IntPtr token,int kind) { IntPtr p=Information(token,kind);try{return Marshal.ReadInt32(p);}finally{Marshal.FreeHGlobal(p);} }
    static Group[] Groups(IntPtr token,int kind) {
        IntPtr p=Information(token,kind);
        try {
            int count=Marshal.ReadInt32(p);if(count<0 || count>512)throw new InvalidOperationException("Group bound");
            var groups=new List<Group>();int start=IntPtr.Size==8?8:4;int size=Marshal.SizeOf(typeof(SidAttributes));
            for(int n=0;n<count;n++) {var item=(SidAttributes)Marshal.PtrToStructure(IntPtr.Add(p,start+n*size),typeof(SidAttributes));groups.Add(new Group{sid=new SecurityIdentifier(item.Sid).Value,attributes=item.Attributes});}
            return groups.ToArray();
        } finally{Marshal.FreeHGlobal(p);}
    }
    public sealed class Actor : IDisposable {
        IntPtr process,token;
        public readonly int pid;
        public readonly long started;
        public readonly string image,user;
        public readonly bool elevated,app_container;
        public readonly Group[] groups,restricting;
        internal Actor(int id,long creation,string path,string sid,bool ordinary,bool systemAllowed=false) {
            process=OpenProcess(0x1000,false,id);if(process==IntPtr.Zero)throw new Win32Exception();
            try {
                long end,k,u;if(!GetProcessTimes(process,out started,out end,out k,out u))throw new Win32Exception();
                var name=new StringBuilder(32768);int size=name.Capacity;
                if(!QueryFullProcessImageName(process,0,name,ref size))throw new Win32Exception();
                image=name.ToString();pid=id;
                if((creation!=-1 && started!=creation) || !String.Equals(image,path,StringComparison.OrdinalIgnoreCase))throw new InvalidOperationException("Actor incarnation/image mismatch");
                IntPtr original;
                if(!OpenProcessToken(process,0xA,out original))throw new Win32Exception();
                try {
                    using(var who=new WindowsIdentity(original)){user=who.User.Value;}
                    elevated=Integer(original,20)!=0;app_container=Integer(original,29)!=0;
                    groups=Groups(original,2);restricting=Groups(original,11);
                    bool system=systemAllowed && user=="S-1-5-18";
                    if((user!=sid && !system) || (ordinary && elevated && !system))throw new InvalidOperationException("Actor token mismatch");
                    if(!DuplicateTokenEx(original,0xE,IntPtr.Zero,2,2,out token))throw new Win32Exception();
                } finally{CloseHandle(original);}
                if(!Alive())throw new InvalidOperationException("Actor not alive");
            } catch{Dispose();throw;}
        }
        public bool Alive(){uint code;return process!=IntPtr.Zero && GetExitCodeProcess(process,out code) && code==259;}
        public int LocalUser(string sddl) {
            if(!Alive() || app_container)return 0;
            return Check(token,sddl);
        }
        // Focused native restriction tests: ephemeral copies only, never a process/account change.
        public int RestrictedLocalUser(string sddl,string restrictSid,string disableSid) {
            if(!Alive() || app_container)return 0;
            if(String.IsNullOrEmpty(restrictSid))restrictSid=null;
            if(String.IsNullOrEmpty(disableSid))disableSid=null;
            IntPtr sid=IntPtr.Zero,row=IntPtr.Zero,copy=IntPtr.Zero;
            try {
                string value=restrictSid??disableSid;var identity=new SecurityIdentifier(value);byte[] bytes=new byte[identity.BinaryLength];identity.GetBinaryForm(bytes,0);
                sid=Marshal.AllocHGlobal(bytes.Length);Marshal.Copy(bytes,0,sid,bytes.Length);
                row=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(SidAttributes)));Marshal.StructureToPtr(new SidAttributes{Sid=sid,Attributes=0},row,false);
                if(!CreateRestrictedToken(token,0,disableSid==null?0U:1U,disableSid==null?IntPtr.Zero:row,0,IntPtr.Zero,restrictSid==null?0U:1U,restrictSid==null?IntPtr.Zero:row,out copy))throw new Win32Exception();
                return Check(copy,sddl);
            } finally{if(copy!=IntPtr.Zero)CloseHandle(copy);if(row!=IntPtr.Zero)Marshal.FreeHGlobal(row);if(sid!=IntPtr.Zero)Marshal.FreeHGlobal(sid);}
        }
        public void Dispose(){if(token!=IntPtr.Zero){CloseHandle(token);token=IntPtr.Zero;}if(process!=IntPtr.Zero){CloseHandle(process);process=IntPtr.Zero;}}
    }
    public static Actor OpenActor(int pid,long started,string image,string sid,bool ordinary) {
        if(started<=0)throw new ArgumentException("Creation time required");
        return new Actor(pid,started,image,sid,ordinary);
    }
    // Metadata, image and token come from the same retained native handle. No managed
    // Process.StartTime access, PID reopen or collector-token substitution is needed.
    public static Actor OpenServerActor(int pid,string image,string ordinarySid,bool service) {
        return new Actor(pid,-1,image,service?"S-1-5-18":ordinarySid,!service,true);
    }
    public sealed class ServerActors : IDisposable {
        readonly Actor[] actors;
        public readonly int service_pid;
        public readonly string image,ordinary_sid;
        internal ServerActors(Actor[] members,int servicePid,string path,string sid) {
            if(members==null || members.Length<1 || members.Length>16)throw new ArgumentException("Server actor bound");
            actors=(Actor[])members.Clone();service_pid=servicePid;image=path;ordinary_sid=sid;
            var ids=new HashSet<int>();bool service=false;
            foreach(var a in actors) {
                if(!ids.Add(a.pid) || !a.Alive() || !String.Equals(a.image,path,StringComparison.OrdinalIgnoreCase) || a.app_container ||
                    (a.user!="S-1-5-18" && (a.user!=sid || a.elevated)))throw new InvalidOperationException("Server actor role mismatch");
                if(a.pid==servicePid){if(a.user!="S-1-5-18")throw new InvalidOperationException("Service token mismatch");service=true;}
            }
            if(!service)throw new InvalidOperationException("Service actor missing");
        }
        public Actor[] Members(){return (Actor[])actors.Clone();}
        public bool Alive(){foreach(var a in actors)if(!a.Alive())return false;return true;}
        public bool Contains(int pid){foreach(var a in actors)if(a.pid==pid)return true;return false;}
        public int LocalUser(string sddl) {
            bool unknown=false;
            foreach(var a in actors){int result=a.LocalUser(sddl);if(result==1)return 1;if(result==0)unknown=true;}
            return unknown?0:-1;
        }
        public void Dispose(){foreach(var a in actors)a.Dispose();}
    }
    public static ServerActors BindServerActors(Actor[] actors,int servicePid,string image,string ordinarySid) {
        return new ServerActors(actors,servicePid,image,ordinarySid);
    }
    // Supported subset: exactly one non-inherited, non-callback allow ACE granting only
    // FWP_ACTRL_MATCH_FILTER (1). Windows AccessCheck handles enabled/deny-only/restricted SIDs.
    static int Check(IntPtr token,string sddl) {
        try {
            var sd=new RawSecurityDescriptor(sddl);
            if(sd.Owner==null || sd.SystemAcl!=null || sd.DiscretionaryAcl==null || sd.DiscretionaryAcl.Count!=1)return 0;
            var ace=sd.DiscretionaryAcl[0] as CommonAce;
            if(ace==null || ace.IsCallback || ace.AceQualifier!=AceQualifier.AccessAllowed || ace.AceFlags!=AceFlags.None || ace.AccessMask!=1)return 0;
            // AccessCheck requires owner/group. Primary group does not affect the supported
            // explicit ACE or requested bit 1; fill only an absent group, never alter the DACL.
            if(sd.Group==null)sd.Group=new SecurityIdentifier(WellKnownSidType.LocalSystemSid,null);
            byte[] bytes=new byte[sd.BinaryLength];sd.GetBinaryForm(bytes,0);
            IntPtr privileges=Marshal.AllocHGlobal(4096);
            try {
                uint size=4096,granted;bool allowed;var map=new Mapping{Read=1,Write=1,Execute=1,All=1};
                if(!AccessCheck(bytes,token,1,ref map,privileges,ref size,out granted,out allowed))return 0;
                if(Marshal.ReadInt32(privileges)!=0)return 0;
                return allowed && (granted&1)!=0?1:-1;
            } finally{Marshal.FreeHGlobal(privileges);}
        } catch(ArgumentException){return 0;}
    }
    [ComImport,Guid("dcb00000-570f-4a9b-8d69-199fdba5723b"),InterfaceType(ComInterfaceType.InterfaceIsDual)]
    interface Manager {void Slot0();void Slot1();Connections GetNetworkConnections();}
    [ComImport,Guid("dcb00006-570f-4a9b-8d69-199fdba5723b"),InterfaceType(ComInterfaceType.InterfaceIsDual)]
    interface Connections {void Slot0();[PreserveSig]int Next(uint wanted,out Connection item,out uint returned);}
    [ComImport,Guid("dcb00005-570f-4a9b-8d69-199fdba5723b"),InterfaceType(ComInterfaceType.InterfaceIsDual)]
    interface Connection {Network GetNetwork();void Slot1();short IsConnected();void Slot3();void Slot4();Guid GetAdapterId();}
    [ComImport,Guid("dcb00002-570f-4a9b-8d69-199fdba5723b"),InterfaceType(ComInterfaceType.InterfaceIsDual)]
    interface Network {void Slot0();void Slot1();void Slot2();void Slot3();Guid GetNetworkId();void Slot5();void Slot6();void Slot7();void Slot8();void Slot9();void Slot10();int GetCategory();}
    public sealed class Binding {public string adapter,network;public int category;}
    public static Binding[] Networks() {
        var manager=(Manager)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("dcb00c01-570f-4a9b-8d69-199fdba5723b")));
        var connections=manager.GetNetworkConnections();var result=new List<Binding>();
        for(int i=0;i<128;i++){uint count;Connection item;Marshal.ThrowExceptionForHR(connections.Next(1,out item,out count));if(count==0)return result.ToArray();
            if(item.IsConnected()!=0){var network=item.GetNetwork();result.Add(new Binding{adapter=item.GetAdapterId().ToString("D"),network=network.GetNetworkId().ToString("D"),category=network.GetCategory()});}}
        throw new InvalidOperationException("Network enumeration bound");
    }
}
