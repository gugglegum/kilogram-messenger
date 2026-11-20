# Dedicated invocation only: the current PowerShell process and its descendants
# enter a non-breakaway Windows job. No process is selected by name or reused PID.
Set-StrictMode -Version Latest
if(-not ('KilogramS5Deadline' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public sealed class KilogramS5Deadline {
    [StructLayout(LayoutKind.Sequential)] struct Basic {
        public long ProcessTime, JobTime; public uint Flags;
        public UIntPtr MinWorking, MaxWorking; public uint ActiveLimit;
        public UIntPtr Affinity; public uint Priority, Scheduling;
    }
    [StructLayout(LayoutKind.Sequential)] struct Io {
        public ulong ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes;
    }
    [StructLayout(LayoutKind.Sequential)] struct Extended {
        public Basic Basic; public Io Io;
        public UIntPtr ProcessMemory, JobMemory, PeakProcess, PeakJob;
    }
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr CreateJobObject(IntPtr a,string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr j,int c,ref Extended i,uint n);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr j,IntPtr p);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool IsProcessInJob(IntPtr p,IntPtr j,out bool member);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr j,int c,IntPtr p,uint n,out uint used);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateJobObject(IntPtr j,uint code);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateProcess(IntPtr p,uint code);
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool DuplicateHandle(IntPtr source,IntPtr handle,IntPtr target,out IntPtr copy,uint access,bool inherit,uint options);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetProcessTimes(IntPtr p,out long created,out long exited,out long kernel,out long user);
    [DllImport("kernel32.dll", SetLastError=true,CharSet=CharSet.Unicode)] static extern bool QueryFullProcessImageName(IntPtr p,uint flags,StringBuilder name,ref int size);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint WaitForSingleObject(IntPtr p,uint milliseconds);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr p,out uint code);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern ulong GetTickCount64();
    readonly IntPtr job; readonly int owner;
    readonly ManualResetEvent stop = new ManualResetEvent(false);
    readonly Thread workThread, wholeThread;
    volatile bool expired; bool finished;
    readonly bool original;
    public readonly double WorkSeconds, WholeSeconds;
    public readonly long OriginalTick, WorkTick, WholeTick;
    public static long TickNow() { return checked((long)GetTickCount64()); }
    int Remaining(long cutoff) { return (int)Math.Min(Int32.MaxValue,Math.Max(0,cutoff-TickNow())); }
    public int RemainingWholeMilliseconds { get { return Remaining(WholeTick); } }
    public bool WorkExpired { get { return expired || (original && Remaining(WorkTick)==0); } }
    public KilogramS5Deadline(double workSeconds,double wholeSeconds)
        : this(workSeconds,wholeSeconds,TickNow(),false) {}
    public KilogramS5Deadline(double workSeconds,double wholeSeconds,long originalTick)
        : this(workSeconds,wholeSeconds,originalTick,true) {}
    KilogramS5Deadline(double workSeconds,double wholeSeconds,long originalTick,bool useOriginal) {
        if(workSeconds<=0 || wholeSeconds<=workSeconds) throw new ArgumentException("deadline order");
        original=useOriginal; OriginalTick=originalTick;
        WorkTick=checked(originalTick+(long)(workSeconds*1000));
        WholeTick=checked(originalTick+(long)(wholeSeconds*1000));
        if(original && (originalTick<=0 || originalTick>TickNow() || Remaining(WorkTick)==0))
            throw new TimeoutException("invalid/expired original deadline");
        WorkSeconds=workSeconds; WholeSeconds=wholeSeconds; owner=Process.GetCurrentProcess().Id;
        job=CreateJobObject(IntPtr.Zero,null);
        if(job==IntPtr.Zero) throw new Win32Exception();
        var limits=new Extended(); limits.Basic.Flags=0x2000; // KILL_ON_JOB_CLOSE, no breakaway
        if(!SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(Extended)))) {
            int e=Marshal.GetLastWin32Error(); CloseHandle(job); throw new Win32Exception(e);
        }
        if(!AssignProcessToJobObject(job,Process.GetCurrentProcess().Handle)) {
            int e=Marshal.GetLastWin32Error(); CloseHandle(job); throw new Win32Exception(e);
        }
        workThread=new Thread(()=>{
            if(!stop.WaitOne(original?TimeSpan.FromMilliseconds(Remaining(WorkTick)):TimeSpan.FromSeconds(workSeconds))) {
                expired=true;
                while(!stop.WaitOne(50)) { try { KillChildren(); } catch {} }
            }
        });
        wholeThread=new Thread(()=>{
            if(!stop.WaitOne(original?TimeSpan.FromMilliseconds(Remaining(WholeTick)):TimeSpan.FromSeconds(wholeSeconds))) TerminateJobObject(job,124);
        });
        workThread.IsBackground=true; wholeThread.IsBackground=true;
        wholeThread.Start(); workThread.Start();
    }
    public void CheckWork() { if(expired || (original && Remaining(WorkTick)==0)) throw new TimeoutException("whole-attempt work cutoff; cleanup reserve active"); }
    public void CheckWhole() { if(original && Remaining(WholeTick)==0) throw new TimeoutException("original outer cutoff"); }
    public void VerifyChild(Process p) {
        bool member;
        if(p.Id==owner || !IsProcessInJob(p.Handle,job,out member) || !member)
            throw new InvalidOperationException("spawned process escaped owned deadline job");
    }
    // Opt-in laboratory fault. The retained handle never follows a replacement PID.
    public sealed class CrashTarget : IDisposable {
        readonly KilogramS5Deadline owner; readonly IntPtr handle;
        public readonly int Pid; public readonly long Started; public readonly string Image;
        readonly object gate=new object(); string request; bool claimed,completed,disposed; uint exit;
        internal CrashTarget(KilogramS5Deadline owner,Process p,long started,string image,int frontend) {
            owner.CheckWork(); owner.VerifyChild(p);
            if(p.Id==frontend) throw new InvalidOperationException("runtime is frontend");
            this.owner=owner; Pid=p.Id; Started=started; Image=image;
            if(!DuplicateHandle(GetCurrentProcess(),p.Handle,GetCurrentProcess(),out handle,0,false,2)) throw new Win32Exception();
            try { Validate(Pid,Started,Image); }
            catch { CloseHandle(handle); throw; }
        }
        void Validate(int pid,long started,string image) {
            owner.CheckWork(); bool member; long created,ended,kernel,user;
            var name=new StringBuilder(32768);int size=name.Capacity;
            if(pid!=Pid || started!=Started || !String.Equals(image,Image,StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("runtime fault request identity");
            if(!IsProcessInJob(handle,owner.job,out member) || !member) throw new InvalidOperationException("runtime fault Job membership");
            if(!GetProcessTimes(handle,out created,out ended,out kernel,out user) || created!=Started) throw new InvalidOperationException("runtime fault creation time");
            if(!QueryFullProcessImageName(handle,0,name,ref size) || !String.Equals(name.ToString(),Image,StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("runtime fault image: "+name.ToString()+" != "+Image);
            if(WaitForSingleObject(handle,0)!=258) throw new InvalidOperationException("runtime fault process not live");
        }
        public uint Execute(string digest,int pid,long started,string image) {
            lock(gate) {
                if(disposed) throw new ObjectDisposedException("CrashTarget");
                if(digest==null || digest.Length!=64 || Array.Exists(digest.ToCharArray(),c=>!((c>='0'&&c<='9')||(c>='a'&&c<='f')))) throw new ArgumentException("fault digest");
                if(claimed) {
                    if(request!=digest || pid!=Pid || started!=Started || !String.Equals(image,Image,StringComparison.OrdinalIgnoreCase) || !completed)
                        throw new InvalidOperationException("fault consumed or outcome unknown");
                    return exit;
                }
                Validate(pid,started,image);
                // Claim BEFORE termination. Unknown outcomes are never retried as a new kill.
                request=digest; claimed=true;
                if(!TerminateProcess(handle,137)) throw new Win32Exception();
                if(WaitForSingleObject(handle,5000)!=0 || !GetExitCodeProcess(handle,out exit) || exit!=137)
                    throw new InvalidOperationException("runtime fault exit unconfirmed");
                completed=true; return exit;
            }
        }
        public void Dispose() { lock(gate) { if(!disposed) { disposed=true; CloseHandle(handle); } } }
    }
    public CrashTarget RetainCrashTarget(Process p,long started,string image,int frontend) {
        return new CrashTarget(this,p,started,image,frontend);
    }
    public int[] Children() {
        IntPtr memory=Marshal.AllocHGlobal(65536);
        try {
            uint used;
            if(!QueryInformationJobObject(job,3,memory,65536,out used)) throw new Win32Exception();
            uint count=(uint)Marshal.ReadInt32(memory,4);
            if(count>1024) throw new InvalidOperationException("owned process count bound");
            var ids=new List<int>();
            for(int n=0;n<count;n++) {
                long id=IntPtr.Size==8?Marshal.ReadInt64(memory,8+n*8):Marshal.ReadInt32(memory,8+n*4);
                if(id!=owner) ids.Add(checked((int)id));
            }
            return ids.ToArray();
        } finally { Marshal.FreeHGlobal(memory); }
    }
    void KillChildren() {
        foreach(int id in Children()) {
            try {
                using(var p=Process.GetProcessById(id)) {
                    bool member;
                    // A process handle plus exact job membership defeats PID reuse.
                    if(IsProcessInJob(p.Handle,job,out member) && member) TerminateProcess(p.Handle,124);
                }
            } catch(ArgumentException) {} catch(InvalidOperationException) {}
        }
    }
    public void CleanupChildren(int milliseconds) {
        var watch=Stopwatch.StartNew();
        while(Children().Length!=0 && watch.ElapsedMilliseconds<milliseconds && (!original || Remaining(WholeTick)>0)) { KillChildren(); Thread.Sleep(20); }
        if(Children().Length!=0) throw new TimeoutException("owned child cleanup did not finish");
    }
    public void Finish() {
        if(finished) return;
        CheckWhole();
        if(Children().Length!=0) throw new InvalidOperationException("owned children still live");
        // The independent whole deadline stays armed through case/ledger writes.
        stop.Set();
        if(original) {
            if(!workThread.Join(Remaining(WholeTick)) || !wholeThread.Join(Remaining(WholeTick)))
                throw new TimeoutException("original watchdog join cutoff");
            CheckWhole();
        } else { workThread.Join(); wholeThread.Join(); }
        var limits=new Extended();
        if(!SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(Extended)))) throw new Win32Exception();
        CloseHandle(job); stop.Dispose(); finished=true;
    }
}
'@
}
