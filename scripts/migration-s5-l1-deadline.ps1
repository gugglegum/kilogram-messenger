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
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    readonly IntPtr job; readonly int owner;
    readonly ManualResetEvent stop = new ManualResetEvent(false);
    readonly Thread workThread, wholeThread;
    volatile bool expired; bool finished;
    public readonly double WorkSeconds, WholeSeconds;
    public bool WorkExpired { get { return expired; } }
    public KilogramS5Deadline(double workSeconds,double wholeSeconds) {
        if(workSeconds<=0 || wholeSeconds<=workSeconds) throw new ArgumentException("deadline order");
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
            if(!stop.WaitOne(TimeSpan.FromSeconds(workSeconds))) {
                expired=true;
                while(!stop.WaitOne(50)) { try { KillChildren(); } catch {} }
            }
        });
        wholeThread=new Thread(()=>{
            if(!stop.WaitOne(TimeSpan.FromSeconds(wholeSeconds))) TerminateJobObject(job,124);
        });
        workThread.IsBackground=true; wholeThread.IsBackground=true;
        wholeThread.Start(); workThread.Start();
    }
    public void CheckWork() { if(expired) throw new TimeoutException("whole-attempt work cutoff; cleanup reserve active"); }
    public void VerifyChild(Process p) {
        bool member;
        if(p.Id==owner || !IsProcessInJob(p.Handle,job,out member) || !member)
            throw new InvalidOperationException("spawned process escaped owned deadline job");
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
        while(Children().Length!=0 && watch.ElapsedMilliseconds<milliseconds) { KillChildren(); Thread.Sleep(20); }
        if(Children().Length!=0) throw new TimeoutException("owned child cleanup did not finish");
    }
    public void Finish() {
        if(finished) return;
        if(Children().Length!=0) throw new InvalidOperationException("owned children still live");
        // The independent whole deadline stays armed through case/ledger writes.
        stop.Set(); workThread.Join(); wholeThread.Join();
        var limits=new Extended();
        if(!SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(Extended)))) throw new Win32Exception();
        CloseHandle(job); stop.Dispose(); finished=true;
    }
}
'@
}
