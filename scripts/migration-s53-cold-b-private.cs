// Fixed Cold-B wiring only. Frozen Core and synthetic factory are unchanged.
namespace Kilogram.S53.ColdA {
public sealed partial class Core {
    public static Core HeldPrivate(long absoluteDeadline) {
        if(absoluteDeadline<=Stopwatch.GetTimestamp())Refuse("deadline");
        var spec=Spec.Production();
        spec.Attempt="P2-s53-exhausted-cold-b-01-attempt-01";
        spec.E=Spec.W+@"\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-b-01-attempt-01";
        spec.Caps.Output=48L*1024*1024;
        var core=new Core(spec,absoluteDeadline);
        core.budget.Record(8192);
        core.heldTyped=new HashSet<Owned>(2);core.heldDbGuards=new List<SafeFileHandle>(2);core.heldOldSlots=new Owned[4];
        core.heldOldDirectories=new Dictionary<Owned,FileInfo>(2);core.heldExplainedParents=new HashSet<string>(2,StringComparer.Ordinal);
        core.heldChanges=new List<object>(4);core.heldDirectoryDeltas=new List<object>(2);core.heldFault="none";
        return core;
    }
    public void WriteHeldPrivateReport(bool typedOk,bool bothLeases,string b0,string b1) {
        if(receiptsStarted||disposed||outputDirectory==null||createRestricted==null||assertPrivateAcl==null)Fail("output_boundary");
        receiptsStarted=true;assertPrivateAcl(spec.E);
        using(var output=new Receipt(this,"comparison.private.jsonl",32L*1024*1024)) {
            output.Record(w=>{w.WriteStartObject();w.WriteString("classification","cold-b-held-physical-final-observation");w.WriteString("attempt",spec.Attempt);
                w.WriteBoolean("typed_ok",typedOk);w.WriteBoolean("both_actual_rust_leases_retained",bothLeases);
                w.WriteString("b0",b0);w.WriteString("b1",b1);w.WritePropertyName("aggregate");JsonSerializer.Serialize(w,HeldReport());w.WriteEndObject();});
            for(int i=0;i<rowCount;i++) {
                Work();int index=i;
                output.Record(w=>{var historical=rows[index];var current=heldFinalRows==null?null:heldFinalRows[index];
                    w.WriteStartObject();w.WriteString("path",historical.Relative);w.WriteNumber("historical_bytes",historical.Bytes);w.WriteString("historical_sha256",Hex(historical.Hash));
                    w.WritePropertyName("historical_s_stamp");Owned old=s.Files[index];for(int n=0;n<4;n++)if(heldOldSlots[n]!=null&&rows[index].Relative==HeldSlotPaths()[n].Substring(spec.S.Length+1).Replace('\\','/'))old=heldOldSlots[n];
                    if(old==null)w.WriteNullValue();else JsonSerializer.Serialize(w,HeldStamp(old.Initial));
                    w.WritePropertyName("final_s_stamp");if(s.Files[index]==null||s.Files[index].CloseAttempted)w.WriteNullValue();else JsonSerializer.Serialize(w,HeldStamp(s.Files[index].Initial));
                    if(current==null){w.WriteNull("final_s_sha256");w.WriteNull("final_s_bytes");}else{w.WriteString("final_s_sha256",Hex(current.Hash));w.WriteNumber("final_s_bytes",current.Bytes);}
                    w.WriteString("bkp_before_sha256",b.Files[index].BeforeHash==null?null:Hex(b.Files[index].BeforeHash));w.WriteString("bkp_after_sha256",b.Files[index].AfterHash==null?null:Hex(b.Files[index].AfterHash));w.WriteEndObject();});
            }
            // Persist full old/new slot stamps and directory stamps. The public
            // delta_stamp_sha256 is not the only remaining representation.
            for(int n=0;n<4;n++){int slot=n;output.Record(w=>{string path=HeldSlotPaths()[slot];string rel=path.Substring(spec.S.Length+1).Replace('\\','/');Row row;Owned final=null;if(expected.TryGetValue(rel,out row))final=s.Files[row.Index];
                w.WriteStartObject();w.WriteString("slot",rel);w.WritePropertyName("historical_stamp");if(heldOldSlots[slot]==null)w.WriteNullValue();else JsonSerializer.Serialize(w,HeldStamp(heldOldSlots[slot].Initial));
                w.WritePropertyName("final_stamp");if(final==null||final.CloseAttempted)w.WriteNullValue();else JsonSerializer.Serialize(w,HeldStamp(final.Initial));w.WriteEndObject();});}
            foreach(var directory in heldOldDirectories)output.Record(w=>{w.WriteStartObject();w.WriteString("directory",directory.Key.Relative);w.WritePropertyName("historical_stamp");JsonSerializer.Serialize(w,HeldStamp(directory.Value));w.WritePropertyName("final_stamp");JsonSerializer.Serialize(w,HeldStamp(directory.Key.Initial));w.WriteEndObject();});
            output.Complete();
        }
    }
    public object PrivateReceiptPins(){return facts.PrivateReceiptDigests;}
}
}
public static partial class S53TypedLockWorker {
    sealed partial class Session {
        internal int RunPrivate(long deadline,SafeFileHandle outputDirectory,Func<string,SafeFileHandle> creator,Action<string> verifier,string decision) {
            Kilogram.S53.ColdA.Core core=null;string failure=null,b0=null,b1=null;int native=0;bool typedOk=false,both=false,reportAttempted=false;
            try {
                Need(attempt=="P2-s53-exhausted-cold-b-01-attempt-01","binding");BindInputs(false);
                string root=@"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb";
                for(int n=0;n<2;n++){string actual=boundPaths[n];if(actual.StartsWith(@"\\?\",StringComparison.Ordinal))actual=actual.Substring(4);
                    Need(actual==root+(n==0?@"\two\state\.kilogram-state.lock":@"\one\state\.kilogram-state.lock"),"binding");}
                core=Kilogram.S53.ColdA.Core.HeldPrivate(deadline);
                var transferred=outputDirectory;outputDirectory=null;core.BindOwnedOutput(transferred,creator,verifier,decision,source);
                for(int n=0;n<2;n++){IntPtr raw=held[n];held[n]=IntPtr.Zero;core.AttachTypedLock(root+(n==0?@"\two\state\.kilogram-state.lock":@"\one\state\.kilogram-state.lock"),raw);}
                phase="baseline";core.CaptureHeldBaseline();Send(new{kind="baseline",version=1,nonce,source_sha256=source,attempt,core=core.HeldReport()});
                using(var d=Read()){Fields(d.RootElement,"kind","version","nonce","source_sha256","attempt");CheckCommon(d.RootElement,"release-slots");}
                phase="release";core.ReleaseHeldSlots();Send(new{kind="slots",version=1,nonce,source_sha256=source,attempt,core=core.HeldReport()});
                using(var d=Read()){var r=d.RootElement;Fields(r,"kind","version","nonce","source_sha256","attempt","typed_ok","leases_held","b0","b1");CheckCommon(r,"typed-complete");
                    typedOk=r.GetProperty("typed_ok").GetBoolean();both=r.GetProperty("leases_held").GetInt32()==2;b0=r.GetProperty("b0").GetString();b1=r.GetProperty("b1").GetString();
                    Need(!typedOk||(both&&Hex(b0,64)&&Hex(b1,64)),"binding");}
                if(!typedOk)failure=both?"typed_reader_failed":"typed_open_lease_consumed";
                phase="final";if(both)core.AuditHeldFinal();
                reportAttempted=true;core.WriteHeldPrivateReport(typedOk,both,b0,b1);
                Send(new{kind="final",version=1,nonce,source_sha256=source,attempt,typed_ok=typedOk,both_actual_rust_leases_retained=both,core=core.HeldReport(),private_receipts=core.PrivateReceiptPins()});
                using(var d=Read()){Fields(d.RootElement,"kind","version","nonce","source_sha256","attempt");CheckCommon(d.RootElement,"finish");}
            }catch(Refusal error){failure=error.Code;native=error.Native;}
             catch(Exception error){failure=error.Data["colda_code"] as string??"internal";if(core!=null){core.HeldFailure(error);native=core.HeldNativeError();}}
            finally {
                // Preserve bounded partial observations before clearing hashes
                // and historical stamps. A failed receipt never becomes PASS.
                if(core!=null&&!reportAttempted)try{core.WriteHeldPrivateReport(typedOk,both,b0,b1);}catch{}
                if(core!=null)try{core.HeldDispose();}catch{failure="cleanup";}
                if(outputDirectory!=null)outputDirectory.Dispose();
                for(int n=1;n>=0;n--)Close(ref held[n]);Close(ref parent);
            }
            bool clean=cleanup&&snapshotClosed&&parent==IntPtr.Zero&&held[0]==IntPtr.Zero&&held[1]==IntPtr.Zero&&(core==null||core.HeldCleanupOk());
            if(!clean)failure="cleanup";
            try{Send(new{kind=failure==null?"done":"refused",version=1,nonce,source_sha256=source,attempt,phase,code=failure,native_error=native,typed_ok=typedOk,both_actual_rust_leases_retained=both,transport_cleanup_ok=clean,transport_handles_after=(parent==IntPtr.Zero?0:1)+(held[0]==IntPtr.Zero?0:1)+(held[1]==IntPtr.Zero?0:1),core=core==null?null:core.HeldReport()});}catch{return 42;}
            return failure==null&&typedOk&&both&&clean?0:40;
        }
    }
    public static SafeFileHandle DuplicateOutput(SafeFileHandle input) {
        IntPtr duplicate;NativeOutput(DuplicateHandle(GetCurrentProcess(),input.DangerousGetHandle(),GetCurrentProcess(),out duplicate,0,false,2));return new SafeFileHandle(duplicate,true);
    }
    static void NativeOutput(bool ok){if(!ok)throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());}
    public static int RunPrivate(string source,string attempt,long deadline,SafeFileHandle directory,Func<string,SafeFileHandle> creator,Action<string> verifier,string decision){return new Session(source,attempt).RunPrivate(deadline,directory,creator,verifier,decision);}
}
