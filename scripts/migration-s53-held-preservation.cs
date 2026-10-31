// Added held-session lifecycle. Frozen parser/native reader/scanner stay in the
// pinned Core; this partial never replaces historical rows with final rows.
namespace Kilogram.S53.ColdA {
public sealed partial class Core {
    HashSet<Owned> heldTyped;
    List<SafeFileHandle> heldDbGuards;
    Owned[] heldOldSlots;
    Dictionary<Owned,FileInfo> heldOldDirectories;
    HashSet<string> heldExplainedParents;
    Row[] heldFinalRows;
    string heldFault,heldHistorical,heldFinalBefore,heldFinalAfter,heldDeltaStampDigest;
    bool heldBaseline,heldReleased,heldFinal,heldDisposed,heldHistoricalStable;
    int heldReleaseCount,heldRecaptureCount,heldTypedMetadata;
    List<object> heldChanges;
    List<object> heldDirectoryDeltas;

    public static Core HeldFixture(string root,long inventoryBytes,string inventoryHash,int files,long bytes,string fault,long deadline) {
        string prefix=Spec.W+@"\.tmp\C-s53-cold-b0c-synthetic\";
        if(root==null||!root.StartsWith(prefix,StringComparison.Ordinal)||root.Substring(prefix.Length).IndexOf('\\')>=0||
           !root.Substring(prefix.Length).StartsWith("b0c-",StringComparison.Ordinal)||
           Path.GetFullPath(root)!=root||inventoryBytes<1||inventoryBytes>Limits.Inventory||
           !IsHash(inventoryHash)||files<6||files>8000||bytes<0||bytes>Limits.Payload||
           !(fault=="none"||fault=="release-after-one"||fault=="recapture-after-one")) Refuse("held_fixture_binding");
        var spec=new Spec{O=root+@"\origin",S=root+@"\state",B=root+@"\backup",E=root+@"\proof",
            Attempt="C-s53-cold-b0c-synthetic",InventoryBytes=inventoryBytes,InventoryHash=inventoryHash,
            ExpectedFiles=files,ExpectedBytes=bytes,IsFixture=true,Caps=new Limits{Handles=Limits.CombinedHandles-Limits.ProductionExternalReserve}};
        var core=new Core(spec,deadline);
        core.budget.Record(8192); // Fixed held lifecycle collections/records, before allocation.
        core.heldTyped=new HashSet<Owned>(2);core.heldDbGuards=new List<SafeFileHandle>(2);core.heldOldSlots=new Owned[4];
        core.heldOldDirectories=new Dictionary<Owned,FileInfo>(2);core.heldExplainedParents=new HashSet<string>(2,StringComparer.Ordinal);
        core.heldChanges=new List<object>(4);core.heldDirectoryDeltas=new List<object>(2);
        core.heldFault=fault;return core;
    }
    string[] HeldSlotPaths() {return new[]{
        spec.S+@"\one\state\state-vault.redb",spec.S+@"\one\state\state-vault.key",
        spec.S+@"\two\state\state-vault.redb",spec.S+@"\two\state\state-vault.key"};}
    bool IsHeldTyped(Owned item) {return heldTyped!=null&&heldTyped.Contains(item);}
    void HeldEmpty(Owned item) {
        if(!item.Lock||item.Directory||item.CloseAttempted||
           !(Absolute(item)==spec.S+@"\one\state\.kilogram-state.lock"||Absolute(item)==spec.S+@"\two\state\.kilogram-state.lock")) Fail("held_lock_binding");
        FileInfo now=Info(item.Handle,Absolute(item),0);
        if(now.Length!=0||!now.Same(item.Initial))Fail("lock_changed");
        item.Final=now;item.EmptyVerified=true;heldTypedMetadata++;
    }
    byte[] HeldHash(Owned item,long length) {
        if(length!=0)Fail("held_lock_length");
        HeldEmpty(item);
        return SHA256.HashData(Array.Empty<byte>());
    }
    // Consumes raw even on failure; the transport relinquishes its slot first.
    public void AttachTypedLock(string path,IntPtr raw) {
        SafeFileHandle handle=new SafeFileHandle(raw,true);bool slot=false,transferred=false;
        try {
            if(captureStarted||heldTyped.Count>=2||
               !(path==spec.S+@"\one\state\.kilogram-state.lock"||path==spec.S+@"\two\state\.kilogram-state.lock"))Fail("held_lock_binding");
            budget.HandleTake();slot=true;Ancestors(path,false);
            FileInfo info=Info(handle,path,0);
            if(info.Length!=0||fixedOwned.ContainsKey(path)||identities.ContainsKey(info.Id))Fail("held_lock_binding");
            Streams(handle,path,info);
            var item=new Owned{Handle=handle,Initial=info,Final=info,FixedPath=Intern(path),Lock=true};
            if(liveCount>=live.Length)Fail("handle_cap");
            identities.Add(info.Id,item);fixedOwned.Add(path,item);live[liveCount++]=item;heldTyped.Add(item);
            transferred=true;handle=null;
        } finally {
            if(!transferred&&handle!=null){
                if(slot)CloseFile(handle);
                else {bool ok=Native.CloseHandle(handle.DangerousGetHandle());handle.SetHandleAsInvalid();handle.Dispose();if(!ok)facts.CleanupUnknown=true;}
            }
        }
    }
    public void CaptureHeldBaseline() {
        if(captureStarted||heldTyped.Count!=2)Fail("held_state");
        captureStarted=true;byte[] raw=null;
        try {
            Phase("held_locks");CaptureLocks();
            Phase("held_inventory");role="inventory";Ancestors(spec.S,true);Ancestors(spec.B,true);
            inventory=Fixed(spec.B+@"\preservation-inventory.json",false);
            raw=ReadInventory();ParseInventory(raw);inventoryValidated=true;heldHistorical=ExpectedDigest();
            Array.Clear(raw,0,raw.Length);raw=null;
            Phase("held_capture");CaptureTree(s);CaptureTree(b);
            facts.ExpectedSetCapturedBeforeHash=true;facts.CaptureCompleteTimestamp=Stopwatch.GetTimestamp();
            Scan(s,false);Scan(b,false);
            Phase("held_baseline_hash");HashTree(s,false);HashTree(b,false);HashTree(s,true);HashTree(b,true);
            Scan(s,true);Scan(b,true);FinalInputs();facts.ObservedEntrySetsEqual=true;
            foreach(string path in HeldSlotPaths()){
                string rel=path.Substring(spec.S.Length+1).Replace('\\','/');
                Row row;if(!expected.TryGetValue(rel,out row))Fail("held_slot_missing");
            }
            foreach(string name in new[]{"one/state","two/state"}){
                Owned dir;if(!s.Dirs.TryGetValue(name,out dir))Fail("held_slot_parent");
                heldOldDirectories.Add(dir,dir.Initial);
            }
            heldHistoricalStable=ExpectedDigest()==heldHistorical;heldBaseline=true;
        }finally{if(raw!=null)Array.Clear(raw,0,raw.Length);StopPhase();UpdateFacts();}
    }
    SafeFileHandle HeldDatabaseGuard(Owned item) {
        Work();budget.HandleTake();SafeFileHandle handle=null;bool transfer=false;
        try {
            // Zero-access metadata opens do not enforce a no-delete share in
            // this Windows path. A real READ handle + share READ|WRITE does.
            // It performs metadata checks only and holds no byte-range lock.
            handle=Native.CreateFileW(IoPath(Absolute(item)),0x80000000u,3,IntPtr.Zero,3,0x00200000,IntPtr.Zero);
            if(handle.IsInvalid)NativeFailure(Marshal.GetLastWin32Error());
            if(!Info(handle,Absolute(item),0).Id.Equals(item.Initial.Id))Fail("held_database_identity");
            heldDbGuards.Add(handle);transfer=true;return handle;
        }finally{if(!transfer){if(handle!=null)CloseFile(handle);else budget.HandleRelease();}}
    }
    public void ReleaseHeldSlots() {
        if(!heldBaseline||heldReleased||heldReleaseCount!=0)Fail("held_state");
        Phase("held_selective_release");
        string[] paths=HeldSlotPaths();
        for(int n=0;n<4;n++){
            string rel=paths[n].Substring(spec.S.Length+1).Replace('\\','/');
            Owned item=s.Files[expected[rel].Index];heldOldSlots[n]=item;
            if(n%2==0)HeldDatabaseGuard(item);
            if(item.CloseAttempted)Fail("held_slot_ownership");
            item.CloseAttempted=true;
            if(!CloseFile(item.Handle))Fail("cleanup_error");
            if(!identities.Remove(item.Initial.Id))Fail("held_slot_ownership");
            heldReleaseCount++;
            if(heldFault=="release-after-one"&&heldReleaseCount==1)Fail("held_release_injected");
        }
        heldReleased=true;StopPhase();UpdateFacts();
    }
    static bool OnlyDirectoryWrite(FileInfo before,FileInfo after) {
        after.Written=before.Written;return before.Same(after);
    }
    bool MutableRow(int index) {
        string rel=rows[index].Relative;
        return rel=="one/state/state-vault.redb"||rel=="one/state/state-vault.key"||
               rel=="two/state/state-vault.redb"||rel=="two/state/state-vault.key";
    }
    void HeldFinalHash(bool after) {
        role="s";
        using(var tuple=BeginTuple()){
            long total=0;
            for(int i=0;i<rowCount;i++){
                Work();Owned item=s.Files[i];long bytes=MutableRow(i)?item.Initial.Length:rows[i].Bytes;
                total=checked(total+bytes);if(total>Limits.Payload)Fail("payload_bytes_cap");
                byte[] hash=Hash(item,bytes);
                if(!MutableRow(i)&&!CryptographicOperations.FixedTimeEquals(hash,rows[i].Hash))Fail("payload_digest");
                if(after){
                    if(bytes!=heldFinalRows[i].Bytes||!CryptographicOperations.FixedTimeEquals(hash,heldFinalRows[i].Hash))Fail("held_final_drift");
                }else heldFinalRows[i]=new Row{Relative=rows[i].Relative,Index=i,Bytes=bytes,Hash=hash};
                AppendTuple(tuple,heldFinalRows[i],hash);
                facts.HashBytes=checked(facts.HashBytes+bytes);
            }
            if(after)heldFinalAfter=Hex(tuple.GetHashAndReset());else heldFinalBefore=Hex(tuple.GetHashAndReset());
        }
        facts.HashPasses++;
    }
    public void AuditHeldFinal() {
        if(!heldReleased||heldFinal||heldRecaptureCount!=0)Fail("held_state");
        Phase("held_recapture");
        budget.Record(checked((long)rowCount*160)); // refs + retained Row/hash objects, before allocation.
        heldFinalRows=new Row[rowCount];
        string[] paths=HeldSlotPaths();
        for(int n=0;n<4;n++){
            string rel=paths[n].Substring(spec.S.Length+1).Replace('\\','/');int index=expected[rel].Index;
            Owned item=Acquire(paths[n],1,rel,false,false,false,false);s.Files[index]=item;
            if(n%2==0&&!item.Initial.Id.Equals(heldOldSlots[n].Initial.Id))Fail("held_database_identity");
            heldRecaptureCount++;
            if(heldFault=="recapture-after-one"&&heldRecaptureCount==1)Fail("held_recapture_injected");
        }
        // All expected objects are held again. Hash S first so byte changes
        // explain allowed parent activity even if a writer restored timestamps.
        Phase("held_final_hash");HeldFinalHash(false);BuildHeldDelta();
        // Historical stamps remain in heldOldDirectories. Scan's Initial is an
        // explicitly distinct final-phase view for only explained state parents.
        foreach(var entry in heldOldDirectories){
            Owned dir=entry.Key;FileInfo now=Info(dir.Handle,Absolute(dir),1);
            if(!entry.Value.Same(now)){
                if(!OnlyDirectoryWrite(entry.Value,now)||!heldExplainedParents.Contains(dir.Relative))Fail("directory_changed");
                heldDirectoryDeltas.Add(new {path=dir.Relative,historical_written=new[]{entry.Value.Written.dwHighDateTime,entry.Value.Written.dwLowDateTime},
                    final_written=new[]{now.Written.dwHighDateTime,now.Written.dwLowDateTime},volume=now.Volume,index=now.Id.Index});
                dir.Initial=now;
            }
        }
        // after=true retains the baseline namespace policy: it refuses even a
        // new empty S directory rather than treating it as a fresh baseline.
        Phase("held_final_scan");Scan(s,true);Scan(b,true);
        Phase("held_final_hash");HashTree(b,false);HeldFinalHash(true);HashTree(b,true);
        Scan(s,true);Scan(b,true);
        Phase("held_final_metadata");
        for(int i=0;i<liveCount;i++){
            Owned item=live[i];if(item.CloseAttempted)continue;
            FileInfo now=Info(item.Handle,Absolute(item),item.Directory?1:0);
            if(!now.Id.Equals(item.Initial.Id)||((!item.Directory||StreamScope(true,Absolute(item)))&&!now.Same(item.Initial)))Fail(item.Directory?"directory_changed":"object_changed");
            if(StreamScope(item.Directory,Absolute(item)))Streams(item.Handle,Absolute(item),now);
        }
        for(int n=0;n<heldDbGuards.Count;n++){
            if(!Info(heldDbGuards[n],paths[n*2],0).Id.Equals(heldOldSlots[n*2].Initial.Id))Fail("held_database_identity");
        }
        for(int i=0;i<locks.Length;i++)Empty(locks[i]);
        if(Hex(Hash(inventory,spec.InventoryBytes))!=spec.InventoryHash||ExpectedDigest()!=heldHistorical)Fail("held_historical_drift");
        if(heldFinalBefore!=heldFinalAfter)Fail("held_final_drift");
        heldHistoricalStable=true;heldFinal=true;StopPhase();UpdateFacts();
    }
    static object HeldStamp(FileInfo info){return new {attributes=info.Attributes,bytes=info.Length,links=info.Links,volume=info.Volume,index=info.Id.Index,
        created_high=info.Created.dwHighDateTime,created_low=info.Created.dwLowDateTime,written_high=info.Written.dwHighDateTime,written_low=info.Written.dwLowDateTime};}
    void BuildHeldDelta(){
        var stamps=new List<object>();string[] paths=HeldSlotPaths();
        for(int n=0;n<4;n++){
            string rel=paths[n].Substring(spec.S.Length+1).Replace('\\','/');int i=expected[rel].Index;
            Owned item=s.Files[i];Row final=heldFinalRows[i];bool metadataEqual=heldOldSlots[n].Initial.Same(item.Initial);
            bool bytesEqual=final.Bytes==rows[i].Bytes&&CryptographicOperations.FixedTimeEquals(final.Hash,rows[i].Hash);
            if(metadataEqual&&bytesEqual)continue;
            heldExplainedParents.Add(rel.Substring(0,rel.LastIndexOf('/')));
            heldChanges.Add(new {path=rel,write_class=n%2==0?"database-same-id":"key-rewrap-or-replacement",metadata_equal=metadataEqual,
                historical_bytes=rows[i].Bytes,historical_sha256=Hex(rows[i].Hash),final_bytes=final.Bytes,final_sha256=Hex(final.Hash),
                historical_id=new ulong[]{heldOldSlots[n].Initial.Volume,heldOldSlots[n].Initial.Id.Index},final_id=new ulong[]{item.Initial.Volume,item.Initial.Id.Index}});
            stamps.Add(new {path=rel,historical_stamp=HeldStamp(heldOldSlots[n].Initial),final_stamp=HeldStamp(item.Initial)});
        }
        heldDeltaStampDigest=Hex(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(stamps)));
    }
    public void HeldFailure(Exception error){
        RememberFailure(error);
        string code=error.Data["colda_code"] as string;
        // Frozen classification stays frozen; only our own literal codes extend it.
        switch(code){
            case "held_fixture_binding":case "held_lock_binding":case "held_lock_length":case "held_state":
            case "held_slot_missing":case "held_slot_parent":case "held_database_identity":case "held_slot_ownership":
            case "held_release_injected":case "held_recapture_injected":case "held_final_drift":case "held_historical_drift":
                facts.FailureCode=code;facts.CauseKind="bounded_refusal";break;
        }
        StopPhase();UpdateFacts();
    }
    public int HeldNativeError(){return facts.NativeError;}
    public object HeldReport() {
        UpdateFacts();
        return new {baseline_completed=heldBaseline,final_verified=heldFinal,slots_released=heldReleaseCount,slots_recaptured=heldRecaptureCount,
            historical_tuple_digest=heldHistorical,historical_rows_unchanged_before_cleanup=heldHistoricalStable,
            historical_checked_phase=heldFinal?"final":heldBaseline?"baseline":"not-established",
            final_s_before=heldFinalBefore,final_s_after=heldFinalAfter,
            bkp_before=facts.BkpBeforeTupleDigest,bkp_after=facts.BkpAfterTupleDigest,hash_passes=facts.HashPasses,
            typed_metadata_checks=heldTypedMetadata,typed_lock_read_bytes=0,typed_lock_hash_provenance="verified-empty-metadata; empty hash derived",
            changed_slots=heldChanges.ToArray(),delta_complete=heldDeltaStampDigest!=null,delta_stamp_sha256=heldDeltaStampDigest,directory_deltas=heldDirectoryDeltas.ToArray(),
            input_handles_released=facts.InputHandlesReleased,remaining_owned_handles=budget.Handles,
            cleanup_unknown=facts.CleanupUnknown,failure_code=facts.FailureCode,failure_phase=facts.FailurePhase,
            native_error=facts.NativeError,cleanup_native_error=facts.CleanupNativeError,failure_cause=facts.CauseKind};
    }
    public void HeldDispose() {
        if(heldDisposed)return;heldDisposed=true;
        try{foreach(var handle in heldDbGuards)try{CloseFile(handle);}catch{facts.CleanupUnknown=true;}}
        finally{heldDbGuards.Clear();Dispose();}
    }
    public bool HeldCleanupOk(){return heldDisposed&&facts.InputHandlesReleased&&!facts.CleanupUnknown&&budget.Handles==0;}
}
}
public static partial class S53TypedLockWorker {
    sealed partial class Session {
        internal int RunHeld(string composition) {
            Kilogram.S53.ColdA.Core core=null;string failure=null;int native=0;
            try{
                BindInputs(true);
                Fields(preservation,"inventory_bytes","inventory_sha256","files","bytes","fault");
                string suffix=@"\state\two\state\.kilogram-state.lock";
                Need(boundPaths[0].EndsWith(suffix,StringComparison.Ordinal),"held_fixture_binding");
                string root=boundPaths[0].Substring(0,boundPaths[0].Length-suffix.Length);
                Need(boundPaths[1]==root+@"\state\one\state\.kilogram-state.lock","held_fixture_binding");
                if(root.StartsWith(@"\\?\",StringComparison.Ordinal))root=root.Substring(4);
                core=Kilogram.S53.ColdA.Core.HeldFixture(root,preservation.GetProperty("inventory_bytes").GetInt64(),
                    preservation.GetProperty("inventory_sha256").GetString(),preservation.GetProperty("files").GetInt32(),
                    preservation.GetProperty("bytes").GetInt64(),preservation.GetProperty("fault").GetString(),
                    Stopwatch.GetTimestamp()+60L*Stopwatch.Frequency);
                for(int n=0;n<2;n++){
                    IntPtr raw=held[n];held[n]=IntPtr.Zero;
                    core.AttachTypedLock(root+(n==0?@"\state\two\state\.kilogram-state.lock":@"\state\one\state\.kilogram-state.lock"),raw);
                }
                phase="baseline";core.CaptureHeldBaseline();
                Send(new {kind="baseline",version=1,nonce,source_sha256=source,attempt,composition_sha256=composition,core=core.HeldReport()});
                phase="release";
                using(var doc=Read()){
                    Fields(doc.RootElement,"kind","version","nonce","source_sha256","attempt");
                    CheckCommon(doc.RootElement,"release-slots");
                }
                core.ReleaseHeldSlots();
                Send(new {kind="slots",version=1,nonce,source_sha256=source,attempt,core=core.HeldReport()});
                phase="audit";
                using(var doc=Read()){
                    Fields(doc.RootElement,"kind","version","nonce","source_sha256","attempt");
                    CheckCommon(doc.RootElement,"audit");
                }
                core.AuditHeldFinal();
                Send(new {kind="final",version=1,nonce,source_sha256=source,attempt,composition_sha256=composition,core=core.HeldReport()});
                phase="finish";
                using(var doc=Read()){
                    Fields(doc.RootElement,"kind","version","nonce","source_sha256","attempt");
                    CheckCommon(doc.RootElement,"finish");
                }
            }catch(Refusal error){failure=error.Code;native=error.Native;}
             catch(Exception error){
                failure=error.Data["colda_code"] as string??"internal";
                if(core!=null){core.HeldFailure(error);native=core.HeldNativeError();}
             }
            finally{
                if(core!=null)try{core.HeldDispose();}catch{failure="cleanup";}
                for(int n=1;n>=0;n--)Close(ref held[n]);
                Close(ref parent);
            }
            bool clean=cleanup&&snapshotClosed&&parent==IntPtr.Zero&&held[0]==IntPtr.Zero&&held[1]==IntPtr.Zero&&(core==null||core.HeldCleanupOk());
            if(!clean)failure="cleanup";
            object report=core==null?null:core.HeldReport();
            // Core's actual cleanup counters are checked by the Rust controller.
            try{Send(new {kind=failure==null?"done":"refused",version=1,nonce,source_sha256=source,attempt,
                phase,code=failure,native_error=native,transport_handles_after=(parent==IntPtr.Zero?0:1)+(held[0]==IntPtr.Zero?0:1)+(held[1]==IntPtr.Zero?0:1),
                transport_cleanup_ok=clean,snapshot_closed=snapshotClosed,core=report});}
            catch{return 42;}
            return failure==null&&clean?0:40;
        }
    }
    public static int RunHeld(string hash,string attempt,string composition){return new Session(hash,attempt).RunHeld(composition);}
}
