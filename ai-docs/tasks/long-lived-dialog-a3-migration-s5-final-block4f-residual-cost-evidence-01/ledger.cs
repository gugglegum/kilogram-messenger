using System;
using System.Collections.Generic;
using System.Diagnostics;

public sealed class S54ResidualStats {
    public string Key;
    public long Calls, Ticks, PathCalls, PathTicks, DirectPathTicks, ReadPathTicks;
    public long ReadCalls, ReadTicks, ReadBytes, ShaCalls, ShaTicks, ShaBytes;
}
public static class S54ResidualLedger {
    public static string Current = "driver";
    public static int ReadDepth, MaxReadDepth;
    static readonly Dictionary<string,S54ResidualStats> Rows = new Dictionary<string,S54ResidualStats>(StringComparer.Ordinal);
    static S54ResidualStats Row(string key) {
        S54ResidualStats r;
        if (!Rows.TryGetValue(key,out r)) { r=new S54ResidualStats {Key=key}; Rows.Add(key,r); }
        return r;
    }
    public static long Begin(string key) { Current=key; Row(key); return Stopwatch.GetTimestamp(); }
    public static void End(string key,string previous,long start) {
        var r=Row(key); r.Calls++; r.Ticks+=Stopwatch.GetTimestamp()-start; Current=previous;
    }
    public static void Path(long start) {
        long ticks=Stopwatch.GetTimestamp()-start;
        var r=Row(Current); r.PathCalls++; r.PathTicks+=ticks;
        if(ReadDepth>0) r.ReadPathTicks+=ticks; else r.DirectPathTicks+=ticks;
    }
    public static void EnterRead() { ReadDepth++; if(ReadDepth>MaxReadDepth) MaxReadDepth=ReadDepth; }
    public static void Read(long start,long bytes) {
        var r=Row(Current); r.ReadCalls++; r.ReadTicks+=Stopwatch.GetTimestamp()-start; r.ReadBytes+=bytes; ReadDepth--;
    }
    public static void Sha(long start,long bytes) {
        var r=Row(Current); r.ShaCalls++; r.ShaTicks+=Stopwatch.GetTimestamp()-start; r.ShaBytes+=bytes;
    }
    public static S54ResidualStats[] Snapshot() { var a=new S54ResidualStats[Rows.Count]; Rows.Values.CopyTo(a,0); return a; }
}
