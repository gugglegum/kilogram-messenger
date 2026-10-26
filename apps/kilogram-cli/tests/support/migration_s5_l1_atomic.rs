//! Actual parent kills at the typed commit gate; cold reads never restore Native.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_protocol::ordered_dialog_v1 as wire;

fn cold(node: &Node, request: Option<u64>, receipt: Option<(u8, u64)>) -> Result<Value> {
    let profile = Profile3::load(&node.profile)?;
    let mut repository = profile.open(&node.profile)?;
    let snapshot = repository.snapshot(0)?;
    let value = wire::decode(&snapshot.head, 2048)?;
    let h = value.fields(13)?;
    let request=request.map(|id|->Result<Value> {
        let bytes=format!("fa00000000000000{id:016x}");
        let mut raw=[0;16];
        for (i,v) in raw.iter_mut().enumerate(){*v=u8::from_str_radix(&bytes[2*i..2*i+2],16)?;}
        Ok(match repository.request(0,raw)? {Some(v)=>json!({"queue":v.queue_sequence,"event":v.event.map(|id|state::hex(&id)),"text_hash":state::hex(&v.text_hash)}),None=>Value::Null})
    }).transpose()?;
    let receipt = receipt
        .map(|(role, n)| -> Result<Option<String>> {
            Ok(repository
                .receipt(0, role, n)?
                .map(|r| blake3::hash(&r).to_hex().to_string()))
        })
        .transpose()?;
    Ok(
        json!({"root_generation":repository.generation(),"head_generation":h[4].uint()?,"native_generation":h[6].uint()?,"native_hash":snapshot.native.as_ref().map(|v|state::hex(&wire::hash(state::NATIVE_TAG,v))),"next":snapshot.meta.next,"counts":repository.counts(0)?,"repair_free":repository.repair_state(0)?.repair_free,"request":request,"receipt_blake3":receipt}),
    )
}
fn request_status(node: &Node, id: u64) -> Result<Value> {
    json_cli(
        &current(),
        &[
            "ordered".into(),
            "request-status3".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            "0".into(),
            "--request-id".into(),
            format!("fa00000000000000{id:016x}"),
        ],
    )
}

#[test]
fn migration_s5_l1_atomic_parent_kills_and_escaped_history_budget() -> Result<()> {
    let seed =
        PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged origin")?);
    let origin: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let context = origin["context"].as_str().context("C")?;
    let base = run_dir("l1-atomic-")?;
    let mut a = Node::from_seed(
        &seed,
        origin["elected_a"]["side"].as_str().context("A")?,
        &base,
        context,
    )?;
    let mut b = Node::from_seed(
        &seed,
        origin["elected_b"]["side"].as_str().context("B")?,
        &base,
        context,
    )?;
    a.migrate()?;
    b.migrate()?;
    let schedule_a = base.join("a.schedule.json");
    let schedule_b = base.join("b.schedule.json");
    schedule_file(&schedule_a, &json!({}))?;
    schedule_file(&schedule_b, &json!({}))?;
    let mut ca = a.start_controlled("a-initial", Some(&schedule_a), None)?;
    let mut cb = b.start_controlled("b-initial", Some(&schedule_b), None)?;
    b.import(&a)?;
    a.import(&b)?;
    let mut outcomes = Vec::new();
    let mut old_cursor = None;
    for (index, point, receiver) in [
        (0, "l1-before-native-commit", false),
        (1, "l1-after-native-commit", false),
        (2, "l1-after-native-commit", true),
    ] {
        let own_before = 5 + index;
        let warm_a = a.wait(&mut ca, own_before, 5)?;
        let warm_b = b.wait(&mut cb, 5, own_before)?;
        let (target, marker) = if receiver {
            (&b, &schedule_b)
        } else {
            (&a, &schedule_a)
        };
        let hit = marker.with_extension("hit");
        if hit.exists() {
            std::fs::remove_file(&hit)?;
        }
        schedule_file(marker, &json!({"pause_point":point}))?;
        let body = if index < 2 {
            "\u{1f}".repeat(4096)
        } else {
            "readable with actual receipt after kill".into()
        };
        let id = 40 + index;
        a.send(id, &body)?;
        let victim = if receiver { &mut cb } else { &mut ca };
        wait_file(victim, &hit, 20)?;
        ensure!(
            std::fs::read_to_string(&hit)? == point,
            "exact typed checkpoint"
        );
        let pid = victim.0.id();
        victim.alive()?;
        victim.kill_wait()?;
        let saved = cold(
            target,
            if receiver { None } else { Some(id) },
            if receiver {
                Some((0, own_before))
            } else {
                None
            },
        )?;
        let warm = if receiver { &warm_b } else { &warm_a };
        if index == 0 {
            ensure!(
                saved["native_hash"] == warm["contexts"][0]["native_hash"]
                    && saved["request"]["event"].is_null()
                    && saved["request"]["queue"].is_u64()
                    && saved["counts"]["queued"] == 1,
                "before-commit old outcome/accepted queue"
            );
        } else {
            ensure!(
                saved["native_generation"].as_u64()
                    == Some(
                        warm["contexts"][0]["native_generation"]
                            .as_u64()
                            .context("Native generation")?
                            + 1
                    ),
                "durable Native advanced exactly once"
            );
            if receiver {
                ensure!(
                    saved["counts"]["pending"] == 0 && saved["receipt_blake3"].is_string(),
                    "readable/receipt/pending atomic outcome"
                );
            } else {
                ensure!(
                    saved["request"]["event"].is_string()
                        && saved["counts"]["queued"] == 0
                        && saved["counts"]["debt"] == 1,
                    "durable before dispatch/debt outcome"
                );
            }
        }
        schedule_file(marker, &json!({}))?;
        if receiver {
            cb = b.start_controlled("b-recovered", Some(&schedule_b), None)?;
        } else {
            ca = a.start_controlled(&format!("a-recovered-{index}"), Some(&schedule_a), None)?;
        }
        b.import(&a)?;
        a.import(&b)?;
        a.send(id, &body)?;
        let after_a = a.wait(&mut ca, own_before + 1, 5)?;
        let after_b = b.wait(&mut cb, 5, own_before + 1)?;
        let status = request_status(&a, id)?;
        ensure!(
            status[0][2] == 1,
            "actual receipt resolves request delivered"
        );
        if !receiver && index == 1 {
            ensure!(
                status[0][1][0] == saved["request"]["event"],
                "exact committed ID after reopen"
            );
        }
        let recovered = if receiver { &after_b } else { &after_a };
        ensure!(
            recovered["stats"]["account_constructors"] == 0
                && recovered["stats"]["outbound_constructors"] == 0
                && recovered["stats"]["inbound_constructors"] == 0,
            "recovery recreated existing objects"
        );
        if index > 0 {
            ensure!(
                recovered["stats"]["packet_attempts"] == 0,
                "durable outcome replayed Native"
            );
        }
        if index == 1 {
            let raw = cli(
                &current(),
                &[
                    "ordered".into(),
                    "history3".into(),
                    "--ipc-file".into(),
                    text(&a.ipc),
                    "--slot".into(),
                    "0".into(),
                    "--limit".into(),
                    "32".into(),
                ],
            )?;
            ensure!(
                raw.trim_end().len() <= 48 * 1024 - 64,
                "actual escaped JSON cap"
            );
            let page: Value = serde_json::from_str(&raw)?;
            ensure!(
                page["items"].as_array().context("escaped page")?.len() == 9
                    && page["finished"] == false,
                "must leave the second4096 escaped body unconsumed"
            );
            old_cursor = Some(
                page["cursor"]
                    .as_str()
                    .context("bounded cursor")?
                    .to_owned(),
            );
        }
        outcomes.push(json!({"point":point,"receiver":receiver,"pid":pid,"parent_verified_alive_then_killed_waited":true,"cold":saved,"a":after_a,"b":after_b,"delivered_request":status}));
    }
    ca.kill_wait()?;
    cb.kill_wait()?;
    let final_b = cold(&b, None, Some((0, 7)))?;
    ensure!(
        final_b["receipt_blake3"] == outcomes[2]["cold"]["receipt_blake3"],
        "same exact durable Receipt1 after recovery/ACK"
    );
    let profile = Profile3::load(&a.profile)?;
    let mut repository = profile.open(&a.profile)?;
    let cursor = kilogram_state::indexed_dialog::HistoryCursor::from_base64(
        old_cursor.as_deref().context("old cursor")?,
    )?;
    let suffix = repository.history(0, Some(&cursor), 32)?;
    let suffix: Value = serde_json::from_slice(&suffix)?;
    ensure!(
        suffix["items"].as_array().context("suffix")?.len() == 1 && suffix["finished"] == true,
        "cursor consumed exactly one remaining old-snapshot body after append/reopen"
    );
    drop(repository);
    std::fs::write(
        base.join("parent-l1-result.json"),
        serde_json::to_vec_pretty(
            &json!({"outcomes":outcomes,"same_receipt_after_reopen":final_b,"escaped_old_suffix":suffix,"cold_reader_method":"typed state point/history API; no ratchet/Native restore call in this controller"}),
        )?,
    )?;
    println!(
        "L1_ATOMIC PASS actual_parent_kills=3 old_or_new=true no_replay_after_durable=true receipt_exact=true escaped_history_cursor=true root={}",
        base.display()
    );
    Ok(())
}
