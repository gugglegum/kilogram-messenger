use super::worker_tests::{Pair, START, STEP, TestResult, Worker};
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;

const ACTUAL: &str = "actual file byte read";
const DERIVED: &str = "held-lock length checked; empty-byte digest derived";
const OS_RULE: &str = "Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.";

struct Fixture {
    root: PathBuf,
    pair: Pair,
    config: Value,
    inventory: Vec<u8>,
    backup: Vec<(String, Vec<u8>)>,
}
impl Fixture {
    fn new(fault: &str) -> TestResult<Self> {
        let base =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/C-s53-cold-b0c-synthetic");
        fs::create_dir_all(&base)?;
        let root = tempfile::Builder::new()
            .prefix("b0c-")
            .tempdir_in(fs::canonicalize(base)?)?
            .keep();
        println!("synthetic_fixture={} fault={fault}", root.display());
        for role in ["one", "two"] {
            let state = root.join("state").join(role).join("state");
            fs::create_dir_all(&state)?;
            fs::write(state.join(LOCK_FILE), [])?;
            fs::write(state.join("state-vault.redb"), format!("db-{role}"))?;
            fs::write(state.join("state-vault.key"), format!("key-{role}"))?;
            let origin = root.join("origin").join(format!("{role}.state"));
            fs::create_dir_all(&origin)?;
            fs::write(origin.join(LOCK_FILE), [])?;
        }
        fs::write(root.join("state/one/state/unchanged.dat"), b"fixed")?;
        let names = [
            "one/state/.kilogram-state.lock",
            "one/state/state-vault.key",
            "one/state/state-vault.redb",
            "one/state/unchanged.dat",
            "two/state/.kilogram-state.lock",
            "two/state/state-vault.key",
            "two/state/state-vault.redb",
        ];
        let mut source = Vec::new();
        let mut destination = Vec::new();
        let mut backup = Vec::new();
        let mut total = 0u64;
        for name in names {
            let bytes = fs::read(root.join("state").join(name))?;
            let path = root.join("backup").join(name);
            fs::create_dir_all(path.parent().ok_or("backup parent")?)?;
            fs::write(&path, &bytes)?;
            let sha = format!("{:x}", Sha256::digest(&bytes));
            let derived = name.ends_with("/.kilogram-state.lock");
            source.push(json!({"path":name,"bytes":bytes.len(),"sha256":sha,"digest_provenance":if derived {DERIVED}else{ACTUAL}}));
            destination.push(
                json!({"path":name,"bytes":bytes.len(),"sha256":sha,"digest_provenance":ACTUAL}),
            );
            total += bytes.len() as u64;
            backup.push((name.to_owned(), bytes));
        }
        let state = literal(&root.join("state"))?;
        let bkp = literal(&root.join("backup"))?;
        let inventory = serde_json::to_vec(
            &json!({"source":state,"preserved":bkp,"classification":"private-stopped-copy-never-opened",
            "held_lock_paths":[state.join("one").join("state").join(LOCK_FILE),state.join("two").join("state").join(LOCK_FILE)],
            "os_lock_rule":OS_RULE,"source_files":source,"source_after_files":source,"destination_files":destination}),
        )?;
        fs::write(root.join("backup/preservation-inventory.json"), &inventory)?;
        let config = json!({"inventory_bytes":inventory.len(),"inventory_sha256":format!("{:x}",Sha256::digest(&inventory)),
            "files":names.len(),"bytes":total,"fault":fault});
        let pair = Pair::from_roots(vec![
            root.join("state/two/state"),
            root.join("state/one/state"),
        ])?;
        Ok(Self {
            root,
            pair,
            config,
            inventory,
            backup,
        })
    }
    fn bind(&self, worker: &Worker) -> Value {
        let mut frame = worker.bind(&self.pair);
        frame["preservation"] = self.config.clone();
        frame
    }
    fn bkp_unchanged(&self) -> TestResult {
        for (name, bytes) in &self.backup {
            assert_eq!(&fs::read(self.root.join("backup").join(name))?, bytes);
        }
        assert_eq!(
            fs::read(self.root.join("backup/preservation-inventory.json"))?,
            self.inventory
        );
        Ok(())
    }
    fn mutate_allowed(&self) -> TestResult {
        for role in ["one", "two"] {
            let parent = self.root.join("state").join(role).join("state");
            let mut db = OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(true)
                .open(parent.join("state-vault.redb"))?;
            db.try_lock()?;
            db.write_all(format!("changed database {role}").as_bytes())?;
            db.sync_all()?;
            db.unlock()?;
            drop(db);
            let temporary = parent.join("loader-owned.tmp");
            fs::write(&temporary, format!("replacement key {role}"))?;
            fs::rename(temporary, parent.join("state-vault.key"))?;
        }
        Ok(())
    }
    fn same_metadata_bytes(&self) -> TestResult {
        let path = self.root.join("state/one/state/state-vault.redb");
        let stamp = fs::metadata(&path)?.modified()?;
        let mut file = OpenOptions::new().write(true).open(&path)?;
        file.write_all(b"DB-ONE")?;
        file.sync_all()?;
        file.set_modified(stamp)?;
        Ok(())
    }
    fn probe_slots(&self) -> TestResult {
        for role in ["one", "two"] {
            for name in ["state-vault.redb", "state-vault.key"] {
                drop(
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .share_mode(7)
                        .open(self.root.join("state").join(role).join("state").join(name))?,
                );
            }
        }
        Ok(())
    }
}
fn baseline(frame: &Value) {
    assert_eq!(frame["kind"], "baseline");
    assert_eq!(frame["core"]["baseline_completed"], true);
    assert_eq!(frame["core"]["hash_passes"], 4);
    assert_eq!(frame["core"]["slots_released"], 0);
    assert_eq!(
        frame["core"]["historical_rows_unchanged_before_cleanup"],
        true
    );
    assert_eq!(frame["core"]["typed_lock_read_bytes"], 0);
    assert_eq!(
        frame["core"]["historical_tuple_digest"],
        frame["core"]["bkp_before"]
    );
    assert_eq!(frame["core"]["bkp_before"], frame["core"]["bkp_after"]);
    assert_eq!(frame["composition_sha256"].as_str().map(str::len), Some(64));
}
fn cleanup(frame: &Value) {
    assert_eq!(frame["transport_handles_after"], 0);
    assert_eq!(frame["transport_cleanup_ok"], true);
    assert_eq!(frame["snapshot_closed"], true);
    assert_eq!(frame["core"]["input_handles_released"], true);
    assert_eq!(frame["core"]["remaining_owned_handles"], 0);
    assert_eq!(frame["core"]["cleanup_unknown"], false);
    assert_eq!(
        frame["core"]["historical_rows_unchanged_before_cleanup"],
        true
    );
}
fn ready_to_mutate(fixture: &Fixture, worker: &mut Worker) -> TestResult<Value> {
    worker.send(&fixture.bind(worker))?;
    let before = worker.receive(START)?;
    baseline(&before);
    // The two real leases and typed origins have not been released.
    fixture.pair.excluded()?;
    worker.send(&worker.common("release-slots"))?;
    let slots = worker.receive(STEP)?;
    assert_eq!(slots["kind"], "slots");
    assert_eq!(slots["core"]["slots_released"], 4);
    assert_eq!(slots["core"]["slots_recaptured"], 0);
    Ok(before)
}
fn finish(fixture: &mut Fixture, worker: &mut Worker) -> TestResult<Value> {
    worker.send(&worker.common("finish"))?;
    let done = worker.receive(STEP)?;
    assert_eq!(done["kind"], "done");
    cleanup(&done);
    assert_eq!(worker.reap()?.code(), Some(0));
    fixture.pair.excluded()?; // Parent leases outlive the final raw audit/child.
    fixture.bkp_unchanged()?;
    fixture.probe_slots()?;
    fixture.pair.release()?;
    fixture.pair.released()?;
    Ok(done)
}

#[test]
fn unchanged_and_allowed_slot_delta() -> TestResult {
    for case in ["unchanged", "allowed"] {
        let mut fixture = Fixture::new("none")?;
        let mut worker = Worker::spawn_held(case)?;
        let before = ready_to_mutate(&fixture, &mut worker)?;
        if case == "allowed" {
            let db = fixture.root.join("state/one/state/state-vault.redb");
            let error = match fs::remove_file(&db) {
                Err(error) => error,
                Ok(()) => return Err("database delete guard lost".into()),
            };
            assert_eq!(error.raw_os_error(), Some(32));
            fixture.mutate_allowed()?;
        }
        worker.send(&worker.common("audit"))?;
        let final_frame = worker.receive(START)?;
        assert_eq!(final_frame["kind"], "final", "{final_frame}");
        assert_eq!(
            final_frame["composition_sha256"],
            before["composition_sha256"]
        );
        assert_eq!(final_frame["core"]["final_verified"], true);
        assert_eq!(final_frame["core"]["hash_passes"], 8);
        assert_eq!(final_frame["core"]["slots_recaptured"], 4);
        assert_eq!(
            final_frame["core"]["historical_tuple_digest"],
            before["core"]["historical_tuple_digest"]
        );
        assert_eq!(
            final_frame["core"]["final_s_before"],
            final_frame["core"]["final_s_after"]
        );
        assert_eq!(
            final_frame["core"]["bkp_after"],
            before["core"]["historical_tuple_digest"]
        );
        let changes = final_frame["core"]["changed_slots"]
            .as_array()
            .ok_or("changes")?;
        assert_eq!(changes.len(), if case == "allowed" { 4 } else { 0 });
        if case == "allowed" {
            assert_ne!(
                final_frame["core"]["final_s_before"],
                before["core"]["historical_tuple_digest"]
            );
            for row in changes {
                assert_ne!(row["historical_sha256"], row["final_sha256"]);
                if row["path"].as_str().ok_or("delta path")?.ends_with(".redb") {
                    assert_eq!(row["historical_id"], row["final_id"]);
                } else {
                    assert_ne!(row["historical_id"], row["final_id"]);
                }
            }
        } else {
            assert_eq!(
                final_frame["core"]["final_s_before"],
                before["core"]["historical_tuple_digest"]
            );
        }
        let done = finish(&mut fixture, &mut worker)?;
        println!("held_case={case} final={final_frame} cleanup={done}");
    }
    Ok(())
}

#[test]
fn restored_timestamp_byte_delta_is_reported() -> TestResult {
    let mut fixture = Fixture::new("none")?;
    let mut worker = Worker::spawn_held("same-stamp")?;
    ready_to_mutate(&fixture, &mut worker)?;
    fixture.same_metadata_bytes()?;
    worker.send(&worker.common("audit"))?;
    let final_frame = worker.receive(START)?;
    assert_eq!(final_frame["kind"], "final", "{final_frame}");
    let changes = final_frame["core"]["changed_slots"]
        .as_array()
        .ok_or("changes")?;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["metadata_equal"], true);
    assert_ne!(changes[0]["historical_sha256"], changes[0]["final_sha256"]);
    finish(&mut fixture, &mut worker)?;
    println!("same_stamp_delta={final_frame}");
    Ok(())
}

#[test]
fn disallowed_payload_and_namespace() -> TestResult {
    for case in ["payload-writer", "extra-file", "extra-directory"] {
        let mut fixture = Fixture::new("none")?;
        let mut worker = Worker::spawn_held(case)?;
        ready_to_mutate(&fixture, &mut worker)?;
        if case == "payload-writer" {
            let path = fixture.root.join("state/one/state/unchanged.dat");
            let error = match OpenOptions::new().write(true).open(path) {
                Err(error) => error,
                Ok(_) => return Err("unapproved payload writer admitted".into()),
            };
            assert_eq!(error.raw_os_error(), Some(32));
        } else {
            // Explained key replacement in the SAME parent isolates namespace
            // refusal from the directory-write-time exception.
            fixture.mutate_allowed()?;
            let extra = fixture.root.join("state/one/state/unlisted");
            if case == "extra-file" {
                fs::write(extra, b"bad")?;
            } else {
                fs::create_dir(extra)?;
            }
        }
        worker.send(&worker.common("audit"))?;
        let result = worker.receive(START)?;
        if case == "payload-writer" {
            assert_eq!(result["kind"], "final");
            finish(&mut fixture, &mut worker)?;
        } else {
            assert_eq!(result["kind"], "refused", "{result}");
            assert_eq!(result["code"], "entry_set_changed", "{result}");
            assert_eq!(result["core"]["failure_code"], "entry_set_changed");
            assert_eq!(result["core"]["failure_phase"], "held_final_scan");
            cleanup(&result);
            assert_eq!(worker.reap()?.code(), Some(40));
            fixture.bkp_unchanged()?;
            fixture.probe_slots()?;
            fixture.pair.release()?;
            fixture.pair.released()?;
        }
        println!("held_refusal_case={case} actual={result}");
    }
    Ok(())
}

#[test]
fn partial_release_recapture_and_eof_cleanup() -> TestResult {
    for case in ["release-after-one", "recapture-after-one", "early-eof"] {
        let mut fixture = Fixture::new(if case == "early-eof" { "none" } else { case })?;
        let mut worker = Worker::spawn_held(case)?;
        worker.send(&fixture.bind(&worker))?;
        baseline(&worker.receive(START)?);
        if case == "early-eof" {
            worker.close_input();
        } else {
            worker.send(&worker.common("release-slots"))?;
            if case == "recapture-after-one" {
                let slots = worker.receive(STEP)?;
                assert_eq!(slots["kind"], "slots");
                fixture.mutate_allowed()?;
                worker.send(&worker.common("audit"))?;
            }
        }
        let refused = worker.receive(START)?;
        assert_eq!(refused["kind"], "refused", "{refused}");
        assert_eq!(
            refused["code"],
            match case {
                "release-after-one" => "held_release_injected",
                "recapture-after-one" => "held_recapture_injected",
                _ => "input_eof",
            }
        );
        assert_eq!(
            refused["core"]["slots_released"],
            if case == "early-eof" {
                0
            } else if case == "release-after-one" {
                1
            } else {
                4
            }
        );
        assert_eq!(
            refused["core"]["slots_recaptured"],
            if case == "recapture-after-one" { 1 } else { 0 }
        );
        assert_eq!(refused["core"]["final_verified"], false);
        if case != "early-eof" {
            assert_eq!(refused["core"]["failure_code"], refused["code"]);
            assert_eq!(refused["core"]["failure_cause"], "bounded_refusal");
            assert_eq!(
                refused["core"]["failure_phase"],
                if case == "release-after-one" {
                    "held_selective_release"
                } else {
                    "held_recapture"
                }
            );
        }
        cleanup(&refused);
        assert_eq!(worker.reap()?.code(), Some(40));
        fixture.bkp_unchanged()?;
        fixture.probe_slots()?;
        fixture.pair.release()?;
        fixture.pair.released()?;
        println!("held_partial_case={case} actual={refused}");
    }
    Ok(())
}
