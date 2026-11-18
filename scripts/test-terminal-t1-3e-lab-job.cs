using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;

// Session containment only. Each attempt still uses the existing independent deadline Job.
// Children inherit this non-breakaway Job at creation, including before worker admission.
public sealed class KilogramLabJob : IDisposable
{
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
    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    static extern IntPtr CreateJobObject(IntPtr a, string name);
    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    static extern IntPtr OpenJobObject(uint access, bool inherit, string name);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool AssignProcessToJobObject(IntPtr j, IntPtr p);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool SetInformationJobObject(IntPtr j,int c,ref Extended i,uint n);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool QueryInformationJobObject(IntPtr j,int c,IntPtr p,uint n,out uint used);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool IsProcessInJob(IntPtr p,IntPtr j,out bool member);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern ulong GetTickCount64();
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool DuplicateHandle(IntPtr source,IntPtr handle,IntPtr target,out IntPtr copy,uint access,bool inherit,uint options);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateProcess(IntPtr process,uint code);
    IntPtr job;
    readonly int owner = Process.GetCurrentProcess().Id;
    public KilogramLabJob(string name) {
        job = CreateJobObject(IntPtr.Zero, name);
        int error = Marshal.GetLastWin32Error();
        if (job == IntPtr.Zero) throw new Win32Exception(error);
        if (error == 183) { CloseHandle(job); job=IntPtr.Zero; throw new InvalidOperationException("Job name collision"); }
        var limits = new Extended(); limits.Basic.Flags = 0x2000;
        if (!SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(Extended))) ||
            !AssignProcessToJobObject(job,Process.GetCurrentProcess().Handle)) {
            error=Marshal.GetLastWin32Error(); CloseHandle(job); job=IntPtr.Zero; throw new Win32Exception(error);
        }
    }
    static int[] Members(IntPtr j) {
        IntPtr memory = Marshal.AllocHGlobal(65536);
        try {
            uint used;
            if (!QueryInformationJobObject(j,3,memory,65536,out used)) throw new Win32Exception();
            int count=Marshal.ReadInt32(memory,4);
            if (count<0 || count>1024) throw new InvalidOperationException("Job process bound");
            var result = new List<int>();
            for (int n=0;n<count;n++) result.Add(checked((int)(IntPtr.Size==8
                ? Marshal.ReadInt64(memory,8+n*8) : Marshal.ReadInt32(memory,8+n*4))));
            return result.ToArray();
        } finally { Marshal.FreeHGlobal(memory); }
    }
    public static int[] Inspect(string name) {
        IntPtr j = OpenJobObject(4,false,name); // QUERY only; no broad process selection.
        if (j==IntPtr.Zero) {
            int error=Marshal.GetLastWin32Error();
            if (error==2) return new int[0];
            throw new Win32Exception(error);
        }
        try { return Members(j); } finally { CloseHandle(j); }
    }
    public int[] Children() { return Array.FindAll(Members(job), id => id!=owner); }
    public void CleanupChildren(int milliseconds) {
        var timer=Stopwatch.StartNew();
        while(Children().Length!=0 && timer.ElapsedMilliseconds<milliseconds) {
            foreach(int id in Children()) {
                try {
                    using(var p=Process.GetProcessById(id)) {
                        bool member;
                        if(!IsProcessInJob(p.Handle,job,out member)) throw new Win32Exception();
                        if(member && !TerminateProcess(p.Handle,124) && !p.HasExited) throw new Win32Exception();
                    }
                } catch(ArgumentException) {} catch(InvalidOperationException) {}
            }
            Thread.Sleep(20);
        }
        if(Children().Length!=0) throw new TimeoutException("Partial launch cleanup unconfirmed");
    }
    public static long TickNow() { return checked((long)GetTickCount64()); }
    public sealed class Guard : IDisposable {
        readonly IntPtr handle, job;
        readonly Timer timer;
        public volatile bool Expired;
        internal Guard(Process child,IntPtr job,int milliseconds) {
            this.job=job;
            if(!DuplicateHandle(GetCurrentProcess(),child.Handle,GetCurrentProcess(),out handle,0,false,2)) throw new Win32Exception();
            timer=new Timer(_=>{
                bool member;
                if(IsProcessInJob(handle,job,out member) && member) { Expired=true; TerminateProcess(handle,124); }
            },null,milliseconds,Timeout.Infinite);
        }
        public void Dispose() {
            using(var finished=new ManualResetEvent(false)) {
                if(!timer.Dispose(finished) || !finished.WaitOne(2000)) throw new TimeoutException("Worker guard drain");
            }
            CloseHandle(handle);
        }
    }
    public Guard Bound(Process child,int milliseconds) { Verify(child); return new Guard(child,job,milliseconds); }
    public void Verify(Process child) {
        bool member;
        if (!IsProcessInJob(child.Handle,job,out member) || !member || child.Id==owner)
            throw new InvalidOperationException("Child escaped session Job");
    }
    public void Dispose() {
        if (job==IntPtr.Zero) return;
        if (Children().Length!=0) throw new InvalidOperationException("Session Job not empty");
        var limits = new Extended();
        if (!SetInformationJobObject(job,9,ref limits,(uint)Marshal.SizeOf(typeof(Extended)))) throw new Win32Exception();
        CloseHandle(job); job=IntPtr.Zero;
    }
}
