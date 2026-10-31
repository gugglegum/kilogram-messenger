// Explicit two-DB dirty-header continuation. Historical rows are immutable.
namespace Kilogram.S53.ColdA {
public sealed partial class Core {
    bool heldContinuation;
    Row[] heldEntryDbs;
    string heldEntryBefore,heldEntryAfter;
    long heldNormalizationBytes;
    object heldContinuationSnapshot;
    void EnableHeldContinuation() {
        if(captureStarted||heldContinuation)Fail("held_continuation_state");
        budget.Record(spec.IsFixture?8192:4096);heldEntryDbs=new Row[2];heldContinuation=true;
    }
    public static Core HeldContinuationFixture(string root,long inventoryBytes,string inventoryHash,int files,long bytes,string fault,long deadline) {
        var core=HeldFixture(root,inventoryBytes,inventoryHash,files,bytes,fault,deadline);
        core.EnableHeldContinuation();return core;
    }
    static bool ContinuationDb(string path) {return path=="one/state/state-vault.redb"||path=="two/state/state-vault.redb";}
    // Only this normalization differs from a raw hash, and only in memory.
    // Seek/Read/metadata/EOF/hash primitives and the fixed readBuffer are reused.
    byte[] DirtyHeaderHistoricalHash(Owned item,long length) {
        if(!ContinuationDb(item.Relative)||length<=9)Fail("held_continuation_delta");
        FileInfo before=Info(item.Handle,Absolute(item),0);
        if(before.Length!=length||!before.Same(item.Initial))Fail("object_changed");
        Seek(item.Handle);long bytes=0;bool normalized=false;
        while(bytes<length) {
            uint count=Read(item.Handle,(uint)Math.Min(readBuffer.Length,length-bytes));
            if(count==0)Fail("payload_length");
            if(bytes<=9&&bytes+count>9) {
                int offset=(int)(9-bytes);
                if((readBuffer[offset]&2)==0)Fail("held_continuation_delta");
                readBuffer[offset]=(byte)(readBuffer[offset]&~2);normalized=true;
            }
            payloadHash.AppendData(readBuffer,0,(int)count);bytes+=count;
            heldNormalizationBytes=checked(heldNormalizationBytes+count);
        }
        if(!normalized||Read(item.Handle,1)!=0)Fail("payload_length");
        if(!before.Same(Info(item.Handle,Absolute(item),0)))Fail("object_changed");
        return payloadHash.GetHashAndReset();
    }
    void HeldBaselineHashes() {
        if(!heldContinuation){HashTree(s,false);HashTree(b,false);HashTree(s,true);HashTree(b,true);return;}
        budget.Record(checked((long)rowCount*160));heldFinalRows=new Row[rowCount];
        HeldFinalHash(false);HashTree(b,false);
        int databases=0;
        for(int i=0;i<rowCount;i++) {
            Row actual=heldFinalRows[i],historical=rows[i];
            if(ContinuationDb(historical.Relative)) {
                if(databases>=2||actual.Bytes!=historical.Bytes||CryptographicOperations.FixedTimeEquals(actual.Hash,historical.Hash))Fail("held_continuation_delta");
                if(!CryptographicOperations.FixedTimeEquals(DirtyHeaderHistoricalHash(s.Files[i],historical.Bytes),historical.Hash))Fail("held_continuation_delta");
                heldEntryDbs[databases++]=actual;
            } else if(actual.Bytes!=historical.Bytes||!CryptographicOperations.FixedTimeEquals(actual.Hash,historical.Hash))Fail("payload_digest");
        }
        if(databases!=2)Fail("held_continuation_delta");
        HeldFinalHash(true);HashTree(b,true);
        heldEntryBefore=heldFinalBefore;heldEntryAfter=heldFinalAfter;
        if(heldEntryBefore!=heldEntryAfter)Fail("held_final_drift");
        // Retain only the separately identified two entry DB rows. The later
        // final audit allocates its own rows; it never overwrites historical rows.
        heldFinalRows=null;heldFinalBefore=null;heldFinalAfter=null;
    }
    Row HeldEntryRow(int index) {
        if(heldContinuation)foreach(Row row in heldEntryDbs)if(row!=null&&row.Relative==rows[index].Relative)return row;
        return rows[index];
    }
    object HeldContinuationSummary() {
        if(heldContinuationSnapshot!=null)return heldContinuationSnapshot;
        if(!heldContinuation)return null;
        var dbs=new List<object>(2);
        foreach(Row entry in heldEntryDbs)if(entry!=null){int i=entry.Index;Row final=heldFinal&&heldFinalRows!=null?heldFinalRows[i]:null;
            dbs.Add(new {path=entry.Relative,historical_sha256=Hex(rows[i].Hash),entry_raw_sha256=Hex(entry.Hash),
                final_raw_sha256=final==null?null:Hex(final.Hash),
                final_equals_historical=final==null?(bool?)null:CryptographicOperations.FixedTimeEquals(final.Hash,rows[i].Hash),
                final_differs_from_entry=final==null?(bool?)null:!CryptographicOperations.FixedTimeEquals(final.Hash,entry.Hash)});
        }
        return new {enabled=true,entry_baseline_verified=heldBaseline,original_raw_equal_claim=false,
            allowed_offset=9,allowed_mask=2,entry_s_before=heldEntryBefore,entry_s_after=heldEntryAfter,
            normalization_read_bytes=heldNormalizationBytes,databases=dbs.ToArray()};
    }
    public string HeldContinuationObservationDigest() {
        if(!heldContinuation||!spec.IsFixture)Refuse("held_fixture_binding");
        return Hex(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(HeldContinuationSummary())));
    }
    object HeldContinuationPipeSummary() {
        if(!heldContinuation)return null;
        int count=0;foreach(Row entry in heldEntryDbs)if(entry!=null)count++;
        return new {enabled=true,entry_baseline_verified=heldBaseline,original_raw_equal_claim=false,
            allowed_offset=9,allowed_mask=2,database_count=count,entry_s_before=heldEntryBefore,entry_s_after=heldEntryAfter,
            normalization_read_bytes=heldNormalizationBytes,
            final_equals_historical=heldFinal?(bool?)(heldFinalBefore==heldHistorical):null,
            final_differs_from_entry=heldFinal?(bool?)(heldFinalBefore!=heldEntryBefore):null};
    }
    public object HeldPipeReport() {
        UpdateFacts();
        return new {baseline_completed=heldBaseline,final_verified=heldFinal,slots_released=heldReleaseCount,slots_recaptured=heldRecaptureCount,
            historical_tuple_digest=heldHistorical,historical_rows_unchanged_before_cleanup=heldHistoricalStable,
            final_s_before=heldFinalBefore,final_s_after=heldFinalAfter,bkp_before=facts.BkpBeforeTupleDigest,bkp_after=facts.BkpAfterTupleDigest,
            hash_passes=facts.HashPasses,typed_lock_read_bytes=0,typed_metadata_checks=heldTypedMetadata,
            changed_slot_count=heldChanges.Count,delta_complete=heldDeltaStampDigest!=null,delta_stamp_sha256=heldDeltaStampDigest,
            continuation=HeldContinuationPipeSummary(),input_handles_released=facts.InputHandlesReleased,remaining_owned_handles=budget.Handles,
            cleanup_unknown=facts.CleanupUnknown,failure_code=facts.FailureCode,failure_phase=facts.FailurePhase,
            native_error=facts.NativeError,cleanup_native_error=facts.CleanupNativeError,failure_cause=facts.CauseKind};
    }
    public void WriteContinuationBaseline() {
        if(!heldContinuation||!heldBaseline||outputDirectory==null||createRestricted==null)Fail("output_boundary");
        using(var output=new Receipt(this,"continuation-baseline.private.json",65536)) {
            output.Record(w=>{w.WriteStartObject();w.WriteString("classification","explicit-two-db-dirty-header-entry");
                w.WriteString("attempt",spec.Attempt);w.WriteString("historical_tuple_digest",heldHistorical);
                w.WritePropertyName("continuation");JsonSerializer.Serialize(w,HeldContinuationSummary());
                w.WriteStartArray("entry_stamps");foreach(Row entry in heldEntryDbs){w.WriteStartObject();w.WriteString("path",entry.Relative);w.WritePropertyName("entry_stamp");JsonSerializer.Serialize(w,HeldStamp(s.Files[entry.Index].Initial));w.WriteEndObject();}w.WriteEndArray();
                w.WriteBoolean("historical_rows_unchanged",ExpectedDigest()==heldHistorical);w.WriteEndObject();});
            output.Complete();
        }
    }
}
}
