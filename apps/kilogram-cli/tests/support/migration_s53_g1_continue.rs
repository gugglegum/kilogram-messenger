//! Attempt02 only: historical failed prefix + independently checked Active A600.
use super::*;
use kilogram_cli::ordered_dialog::{handshake::Spec, runtime7::Profile3, state};
use kilogram_state::indexed_dialog::migration7::live1 as store;
use sha2::{Digest as _, Sha256};
use std::io::Write;

const CASE: &str = "F-s53-exhausted-integrated-01";
const ATTEMPT: &str = "F-s53-exhausted-integrated-01-attempt-02";
const OLD_RUN: &str = ".tmp/migration-s53-integrated/F-s53-exhausted-integrated-01-attempt-01";
const EVIDENCE: &str = "ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence";
const Q: &str = "be44fd6493e281ae10aeadd075af2b7777cc694aaed1d3e247bb9f235e0c70fa";
fn sha(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}
fn pinned(name: &str, bytes: usize, digest: &str) -> Result<Vec<u8>> {
    let raw = kilogram_cli::ordered_dialog::admin::read_public(
        &workspace().join(EVIDENCE).join(name),
        bytes,
    )?;
    ensure!(
        raw.len() == bytes && sha(&raw) == digest,
        "C2 fixed historical public input"
    );
    Ok(raw)
}
struct Historical {
    full: [Value; 2],
    rows: [Value; 2],
    requests: [Value; 2],
    originals: [g1_replay::Original; 2],
    prefix: Value,
}
impl Historical {
    fn load() -> Result<Self> {
        let mut documents = Vec::new();
        for (name, bytes, digest) in [
            (
                "F-s53-exhausted-integrated-01-attempt-01.handoff.json",
                20566,
                "f9eb64e0ddac9eaa6b233b52979465d573c36d71fe6c22441c06ee84e1d465c8",
            ),
            (
                "F-s53-exhausted-integrated-01-attempt-01.root-failure-review.json",
                2838,
                "aa1c3efb12f1c3d58a1241cfcc3356f9d50edd110778b1e27699d467204a4185",
            ),
            (
                "P2-s53-exhausted-cold-b-01-attempt-02.aggregate.json",
                17077,
                "a0efc865407cf0202644b7fa8ce584d564f16b0747ba8b2a31290f15d4996b06",
            ),
            (
                "P2-s53-exhausted-M1.candidate.json",
                23116,
                "788076595dbebdfc7157731609d4a1c1ce613e78821c4d866bded41f16e50c77",
            ),
            (
                "P2-s53-exhausted-M1.D2-root-accepted.md",
                5661,
                "9ea318a5531dc9a5b5ff605eae2bcf4328772db9cea7e84fe383baf4bbcc33a9",
            ),
        ] {
            pinned(name, bytes, digest)?;
            documents.push(json!({"name":name,"sha256":digest}));
        }
        let manifest = pinned(
            "F-s53-exhausted-integrated-01-attempt-01.public-command-files.json",
            509647,
            "aea46b7e73c8c44163945ae576fd0ae8079498a1fcae38c16e18288e137c24c3",
        )?;
        let manifest: Value = serde_json::from_slice(&manifest)?;
        let mut witnesses = Vec::new();
        let mut read = |n: usize, stderr: bool| -> Result<Vec<u8>> {
            let relative = format!(
                "{OLD_RUN}/command-{n:04}.{}.log",
                if stderr { "stderr" } else { "stdout" }
            );
            let rows: Vec<_> = manifest["files"]
                .as_array()
                .context("C2 public file pins")?
                .iter()
                .filter(|r| r["path"] == relative)
                .collect();
            ensure!(rows.len() == 1, "C2 unique historical command pin");
            let bytes = rows[0]["bytes"].as_u64().context("C2 command bytes")?;
            ensure!(bytes <= 48 * 1024, "C2 public command cap");
            let raw = kilogram_cli::ordered_dialog::admin::read_public(
                &workspace().join(&relative),
                48 * 1024,
            )?;
            ensure!(
                raw.len() as u64 == bytes && rows[0]["sha256"] == sha(&raw),
                "C2 historical command SHA"
            );
            witnesses.push(rows[0].clone());
            Ok(raw)
        };
        // All original public refusal/conversion brackets remain historical.
        for n in [0, 1, 495, 496, 497, 498, 500, 501, 502, 503, 517, 518, 525] {
            read(n, false)?;
        }
        ensure!(
            String::from_utf8(read(499, true)?)?.trim()
                == "Error: UpgradeRequired: send lane lifetime capacity",
            "C2 historical exact refusal"
        );
        let full: [Value; 2] = [
            serde_json::from_slice(&read(1114, false)?)?,
            serde_json::from_slice(&read(1115, false)?)?,
        ];
        let rows = [
            full[0]["contexts"][0].clone(),
            full[1]["contexts"][0].clone(),
        ];
        let requests = [
            serde_json::from_slice(&read(1116, false)?)?,
            serde_json::from_slice(&read(1117, false)?)?,
        ];
        for (role, row) in rows.iter().enumerate() {
            start_fields(row, row, role)?;
        }
        ensure!(
            requests[0]
                == json!([[
                    [1564],
                    ["36c4b673903a55449df7e4dfc28d4de8d36bc0299b2ef4814dd4a91dd835f800"],
                    1
                ]])
                && requests[1] == json!([]),
            "C2 exact historical A request/B absence"
        );
        let originals = g1_replay::public_originals()?;
        Ok(Self {
            full,
            rows,
            requests,
            originals,
            prefix: json!({"status":"FAILED","historical_prefix":"root-accepted inference from pinned actual executed sequential fallible prefix plus public brackets","lost_ephemeral_values_recovered":false,"documents":documents,"public_commands":witnesses}),
        })
    }
}
fn start_fields(now: &Value, saved: &Value, role: usize) -> Result<()> {
    ensure!(
        role < 2
            && now["role"] == role
            && now["phase"] == 3
            && now["mode"] == "Active"
            && now["q"] == Q
            && now["cuts"] == json!([512, 18])
            && now["next"] == json!([513, 18])
            && now["native_encoding"] == "Mixed7"
            && now["native_generation"] == [519, 90][role]
            && now["native_hash"]
                == [
                    "2e3bc9c9b05236b93f3b32c509bc7c6ce27536d46d3a1bdd27061a64d6eab9e5",
                    "17c7aef1addcb3b64364e2ae5d555ca5cf98dd1bd38c61edf4b254aac6f0643c"
                ][role],
        "C2 exact saved Active A state"
    );
    ensure!(
        now["counts"] == json!({"sources":531,"receipts":[512,17],"queued":0,"pending":0,"debt":0})
            && now["control_debt"] == false
            && now["freeze"] == false
            && now.get("head_pending").is_some_and(Value::is_null),
        "C2 saved quiescent counts"
    );
    for f in [
        "context",
        "d",
        "e",
        "sid",
        "current_consent2",
        "controls",
        "native_hash",
        "native_generation",
    ] {
        ensure!(
            saved.get(f).is_some() && now.get(f) == saved.get(f),
            "C2 saved identity/control {f}"
        );
    }
    Ok(())
}
fn stored_gate(pair: &Pair, h: &Historical) -> Result<Value> {
    ensure!(
        pair.children.iter().all(Option::is_none),
        "C2 stopped start owners"
    );
    let mut observations = Vec::new();
    for local in 0..2 {
        let node = &pair.nodes[local];
        let profile = Profile3::load(&node.profile)?;
        let mut repo = profile.open(&node.profile)?;
        ensure!(
            repo.contexts() == [(state::parse_id(&pair.c)?, local as u8)]
                && state::hex(&repo.source_id()) == profile.source_id
                && state::hex(&repo.migration_id()) == profile.migration_id
                && h.full[local]["source_id"] == profile.source_id
                && repo.config_revision() == profile.config_revision,
            "C2 stored source/migration/role/config identity"
        );
        let spec = Spec::parse(&repo.spec(0)?)?;
        let head = state::Head::parse(
            &repo.head(0)?,
            state::parse_id(&pair.c)?,
            spec.cert_refs()?[local],
            local as u64,
        )?;
        let existing = repo.existing_state(0)?;
        let meta = repo.metadata(0)?;
        ensure!(
            meta.request_index_ready && repo.initialized()? && repo.history_initialized(0)?,
            "C2 saved initialized request/history indices"
        );
        let repair = repo.repair_state(0)?;
        ensure!(
            existing.current_consent == existing.selected_consent
                && existing
                    .local_intent
                    .as_ref()
                    .is_none_or(|i| Some(i.consent) == existing.selected_consent),
            "C2 no replacement Consent/intent"
        );
        let row = json!({"context":state::hex(&head.c),"d":head.d.map(|i|state::hex(&i)),"e":head.e.map(|i|state::hex(&i)),"sid":head.sid,
            "role":head.role,"phase":head.phase,"mode":format!("{:?}",existing.mode),"q":existing.q.map(|i|state::hex(&i)),"cuts":existing.cuts,
            "next":meta.next,"counts":repo.counts(0)?,"control_debt":existing.control_debt,"freeze":existing.mode.frozen(),"head_pending":repair.head,
            "native_generation":head.native_generation,"native_hash":head.blob.map(|i|state::hex(&i)),"native_encoding":"Mixed7",
            "current_consent2":existing.current_consent.map(|i|state::hex(&i)),"controls":{"p":existing.propose.map(|i|state::hex(&i)),"a":existing.accept.map(|i|state::hex(&i)),"k":existing.select.map(|i|state::hex(&i)),"committed":existing.committed.map(|i|state::hex(&i))}});
        start_fields(&row, &h.rows[local], local)?;
        let snapshot = repo.snapshot(0)?;
        let raw = snapshot.native.context("C2 saved opaque Native")?;
        ensure!(
            snapshot.head == repo.head(0)?
                && head.blob
                    == Some(kilogram_protocol::ordered_dialog_v1::hash(
                        store::MIXED_NATIVE_ID,
                        &raw
                    )),
            "C2 authenticated opaque Native pointer/hash"
        );
        let decoded = kilogram_protocol::ordered_dialog_v1::decode(&raw, state::MAX_NATIVE)?;
        let f = decoded.fields(5)?;
        ensure!(
            f[0].uint()? == 7 && f[1].uint()? == 3 && f[2].uint()? == head.native_generation,
            "C2 actual Mixed7 envelope"
        );
        ensure!(
            repo.request(0, request_bytes(601))?.is_none()
                && repo.event_at(0, 1, 18)?.is_none()
                && repo.receipt(0, 1, 18)?.is_none(),
            "C2 successful B request/source/receipt absence"
        );
        observations.push(json!({"local":local,"state":row,"native_public_keys":"bound by exact saved opaque hash; independently compared through fresh live status below","opaque_hash_domain":"MIXED_NATIVE_ID"}));
    }
    Ok(json!(observations))
}
fn request_bytes(n: u64) -> [u8; 16] {
    let mut id = [0; 16];
    id[0] = 0xfa;
    id[8..].copy_from_slice(&n.to_be_bytes());
    id
}
fn replay(pair: &Pair, h: &Historical) -> Result<Value> {
    ensure!(
        pair.children.iter().all(Option::is_none),
        "C2 original replay stopped owners"
    );
    let until = Instant::now() + Duration::from_secs(180);
    let mut reports = Vec::new();
    for local in 0..2 {
        let profile = Profile3::load(&pair.nodes[local].profile)?;
        let mut repo = profile.open(&pair.nodes[local].profile)?;
        reports.push(h.originals[local].replay(&mut repo, until)?);
    }
    Ok(serde_json::to_value(reports)?)
}
fn admitted(full: &Value, row: &Value, saved: &Value, quiet: bool) -> Result<()> {
    same_origin(saved, row)?;
    ensure!(
        row["mode"] == "Active"
            && row["q"] == saved["q"]
            && row["cuts"] == saved["cuts"]
            && row["current_consent2"] == saved["current_consent2"]
            && row["lease_family"] == 2
            && row["lease_current"] == true
            && row["lease_generation"].as_u64().is_some()
            && row["publication_ready"] == true
            && row["existing_origin_ready"] == true,
        "C2 fresh existing Consent admission"
    );
    for f in [
        "policy_error",
        "handoff_policy_error",
        "publication_error",
        "origin_publication_error",
    ] {
        ensure!(row.get(f).is_some_and(Value::is_null), "C2 live {f}");
    }
    for f in [
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(full["stats"][f].as_u64() == Some(0), "C2 no process {f}");
    }
    if quiet {
        for f in ["materialized", "readable"] {
            ensure!(full["stats"][f].as_u64() == Some(0), "C2 no repeated {f}");
        }
    }
    Ok(())
}
fn bind_live(pair: &Pair, h: &Historical, d: &g1_data::Delivery, quiet: bool) -> Result<()> {
    let rows = d.rows();
    for (local, row) in rows.iter().enumerate() {
        admitted(&d.status()[local], row, &h.rows[local], quiet)?;
        h.originals[local].compare_original_map(&d.histories()[local])?;
    }
    ensure!(
        pair.children.iter().all(Option::is_some),
        "C2 live owned pair"
    );
    Ok(())
}
fn fresh_start(pair: &mut Pair, h: &Historical) -> Result<()> {
    for local in 0..2 {
        for suffix in ["stdout.log", "stderr.log"] {
            let path = pair.nodes[local].branch.base.join(format!(
                "live-{local}-{}.{}",
                pair.starts[local] + 1,
                suffix
            ));
            match std::fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
                Ok(_) => anyhow::bail!("C2 fresh log collision; preserve prior evidence"),
            }
        }
    }
    pair.start(0, None)?;
    pair.start(1, None)?;
    pair.exchange2()?;
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        s53_deadline::check()?;
        ensure!(Instant::now() < until, "C2 fresh admission deadline");
        for child in pair.children.iter_mut().flatten() {
            child.alive()?;
        }
        let full = [pair.nodes[0].status()?, pair.nodes[1].status()?];
        let rows = [
            pair.nodes[0].context(&full[0])?,
            pair.nodes[1].context(&full[1])?,
        ];
        // Only connection establishment is pending here. Malformed states or
        // policy failures are not swallowed as a successful permission check.
        if rows.iter().all(|r| r["lease_current"] == true) {
            for local in 0..2 {
                admitted(&full[local], rows[local], &h.rows[local], true)?;
            }
            return Ok(());
        }
        for row in rows {
            ensure!(
                row.get("policy_error").is_some_and(Value::is_null)
                    && row.get("handoff_policy_error").is_some_and(Value::is_null),
                "C2 pending admission policy"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn phase(base: &Path, name: &str, mut value: Value) -> Result<Value> {
    s53_deadline::check()?;
    public_fingerprints(&mut value)?;
    let raw = serde_json::to_vec(
        &json!({"phase":name,"classification":"completed continuation phase; not whole-case PASS","observations":value}),
    )?;
    ensure!(raw.len() <= 256 * 1024, "C2 phase evidence byte cap");
    let path = base.join(format!("continuation-{name}.json"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(&raw)?;
    file.sync_all()?;
    Ok(json!({"path":path,"bytes":raw.len(),"sha256":sha(&raw)}))
}
pub(super) fn run() -> Result<()> {
    let managed = Managed::new(CASE, 540)?;
    ensure!(
        managed.run.file_name().and_then(|s| s.to_str()) == Some(ATTEMPT),
        "C2 fresh attempt02 only"
    );
    let h = Historical::load()?; // All fixed public historical guards precede S.
    let prepared = g1_prerequisite::Prepared::load(&managed)?;
    let (nodes, roots, c) = prepared.nodes()?;
    // Old executed selector used starts1/2. Reserve201/202 to preserve its logs.
    let mut pair = Pair {
        nodes,
        roots,
        c,
        base: managed.run.clone(),
        children: [None, None],
        starts: [200, 200],
    };
    let mut origin = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(pair.base.join("exhausted.origin.cbor"))?;
    origin.write_all(h.originals[0].origin())?;
    origin.sync_all()?;
    drop(origin);
    let start = stored_gate(&pair, &h)?;
    let original_a = replay(&pair, &h)?;
    let stopped_a = g1_data::stopped_a(&pair, h.originals[0].origin(), &h.rows, &h.requests)?;
    let initial = phase(
        &pair.base,
        "stopped-A",
        json!({"historical":h.prefix,"start":start,"original_B1":original_a,"raw_A":stopped_a.report()}),
    )?;
    fresh_start(&mut pair, &h)?;
    let a = g1_data::wait_delivery(&mut pair, &h.rows, 1)?;
    stopped_a.bind(&a)?;
    bind_live(&pair, &h, &a, true)?;
    let a_gate = phase(
        &pair.base,
        "A-gate",
        json!({"stopped_phase":initial,"A":a.report()?,"raw_A":stopped_a.report(),"B_send_not_started":true}),
    )?;
    pair.nodes[1].send(601, g1_admission::FIRST_TEXT)?;
    let b = g1_data::wait_delivery(&mut pair, &h.rows, 2)?;
    g1_data::a_before_b(&a, &b)?;
    bind_live(&pair, &h, &b, false)?;
    let first_stop = g1_data::stop(&mut pair)?;
    let originals_b = replay(&pair, &h)?;
    let raw_b = g1_data::stopped_delivery(&pair, &b)?;
    stopped_a.preserve_prefix(&raw_b)?;
    let checkpoint = phase(
        &pair.base,
        "B-before-restart",
        json!({"A_gate":a_gate,"B":b.report()?,"raw_AB":raw_b.report(),"original_B1":originals_b,"stop":first_stop}),
    )?;
    fresh_start(&mut pair, &h)?;
    let reopened = g1_data::wait_delivery(&mut pair, &h.rows, 2)?;
    bind_live(&pair, &h, &reopened, true)?;
    g1_data::compare_delivery(&b, &reopened)?;
    for role in 0..2 {
        ensure!(
            b.rows()[role]["repair_free"] == reopened.rows()[role]["repair_free"],
            "C2 actual spent pool retained"
        );
    }
    let final_stop = g1_data::stop(&mut pair)?;
    let final_originals = replay(&pair, &h)?;
    let final_raw = g1_data::stopped_delivery(&pair, &reopened)?;
    ensure!(
        raw_b.report() == final_raw.report(),
        "C2 exact new stopped records after restart"
    );
    let restart = phase(
        &pair.base,
        "restart",
        json!({"before_restart":checkpoint,"reopened":reopened.report()?,"raw_AB":final_raw.report(),"original_B1":final_originals,"stop":final_stop}),
    )?;
    drop(pair);
    managed.finish(json!({"classification":"continuation of preserved Active+A600; failed historical attempt remains FAILED","historical_prefix":h.prefix,"stopped_A":initial,"A_gate":a_gate,"pre_restart":checkpoint,"restart":restart,"no_A_resend_upgrade_or_root_ceremony":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn g1c2_captured_start_and_fresh_admission_negatives() -> Result<()> {
        let h = Historical::load()?;
        for local in 0..2 {
            let saved = &h.rows[local];
            start_fields(saved, saved, local)?;
            for (field, value) in [
                ("q", json!("0".repeat(64))),
                ("native_hash", json!("0".repeat(64))),
                ("native_generation", json!(518)),
                ("next", json!([512, 18])),
                ("counts", json!({})),
                ("current_consent2", json!("0".repeat(64))),
                ("controls", json!({})),
            ] {
                let mut bad = saved.clone();
                bad[field] = value;
                ensure!(
                    start_fields(&bad, saved, local).is_err(),
                    "C2 negative start {field}"
                );
            }
            let mut fresh = h.full[local].clone();
            fresh["stats"]["materialized"] = json!(0);
            fresh["stats"]["readable"] = json!(0);
            admitted(&fresh, saved, saved, true)?;
            for (field, value) in [
                ("lease_current", json!(false)),
                ("lease_family", json!(1)),
                ("policy_error", json!("denied")),
            ] {
                let mut bad = saved.clone();
                bad[field] = value;
                ensure!(
                    admitted(&fresh, &bad, saved, true).is_err(),
                    "C2 negative admission {field}"
                );
            }
            fresh["stats"]["readable"] = json!(1);
            ensure!(
                admitted(&fresh, saved, saved, true).is_err(),
                "C2 repeated readable refused"
            );
        }
        Ok(())
    }
}
