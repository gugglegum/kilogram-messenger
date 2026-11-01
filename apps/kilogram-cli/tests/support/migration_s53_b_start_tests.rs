//! Evidence association tests only; no real process/network claim.
use super::super::tests::{contexts, directed, item, status_fixture};
use super::*;

fn pending_reply() -> Value {
    json!({"version":3,"control_debt":false,"context":contexts()[0],"slot":1,"mode":"L1","status":"NotificationPending","lease_current":false,"policy_error":null,"q":null,"cuts":[null,null],"freeze":false,"controls":{"p":null,"a":null,"k":null,"committed":null}})
}
#[test]
fn b_start_serialized_causal_gate_negatives() -> Result<()> {
    let maps = oracle::mappings(&status_fixture(), &contexts())?;
    let m = mapping(&maps, 1, &contexts()[0])?;
    let v = pending_reply();
    pending(&v, m)?;
    for (k, bad) in [
        ("lease_current", json!(true)),
        ("status", json!("LocalIntentPending")),
        ("context", json!(contexts()[1])),
        ("slot", json!(0)),
        ("q", json!("33".repeat(32))),
    ] {
        let mut changed = v.clone();
        changed[k] = bad;
        assert!(pending(&changed, m).is_err(), "{k}");
    }
    let mut wrong = m.clone();
    wrong.owner = 0;
    assert!(pending(&v, &wrong).is_err());
    let raw = serde_json::to_vec(&v)?;
    let encoded = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        decode_upgrade_reply(&serde_json::to_string(&encoded)?, m)?,
        v
    );
    assert!(decode_upgrade_reply("\"a\"", m).is_err());
    Ok(())
}

#[test]
fn b_start_mixed_slot_trace_and_notification_association() -> Result<()> {
    let maps = oracle::mappings(&status_fixture(), &contexts())?;
    let raw=b"s53_rpc slot=0 kind=Propose\ns53_rpc slot=1 kind=OpenIntent\ns53_scheduler slot=0 phase=probe-start monotonic_ms=10\ns53_scheduler slot=0 phase=probe-resolved mode=L1 monotonic_ms=55\ns53_scheduler slot=0 phase=probe-start monotonic_ms=61010\npartial";
    let x = parse_trace(raw, 1, Path::new("owned-X.stderr"), &maps)?;
    assert_eq!(x.partial_tail_bytes, 7);
    assert_eq!(x.records[0]["context"], contexts()[1]);
    assert_eq!(x.records[1]["context"], contexts()[0]);
    let y = parse_trace(
        b"s53_scheduler slot=0 phase=probe-start monotonic_ms=3\n",
        0,
        Path::new("owned-Y.stderr"),
        &maps,
    )?;
    let ts = vec![y, x];
    notification(&ts, &contexts()[0], 0, 1)?;
    assert!(notification(&ts, &contexts()[1], 0, 1).is_err());
    assert!(notification(&ts, &contexts()[0], 1, 1).is_err());
    assert!(notification(&ts[..1], &contexts()[0], 0, 1).is_err());
    assert!(
        parse_trace(
            b"s53_rpc slot=1 kind=OpenIntent\n",
            0,
            Path::new("wrong-owner"),
            &maps
        )
        .is_err()
    );
    assert!(parse_trace(b"s53_scheduler slot=0 phase=probe-start monotonic_ms=3\ns53_scheduler slot=0 phase=probe-start monotonic_ms=2\n",0,Path::new("reset-epoch"),&maps).is_err());
    Ok(())
}

#[test]
fn b_start_selected_identity_mismatch_is_not_readiness() -> Result<()> {
    let mut s = status_fixture();
    for full in &mut s {
        for row in full["contexts"]
            .as_array_mut()
            .context("fixture contexts")?
        {
            row["controls"] = json!({"p":"a","a":"b","k":"c","committed":"d"});
            row["control_debt"] = json!(false);
        }
    }
    let maps = oracle::mappings(&s, &contexts())?;
    let full = s.clone().map(Some);
    selected(&full, &maps, &contexts()[0])?;
    let mut bad = full;
    bad[1].as_mut().context("fixture X")?["contexts"][0]["q"] = json!("different-selection");
    assert!(selected(&bad, &maps, &contexts()[0]).is_err());
    Ok(())
}

#[test]
fn b_start_exact_message_and_cross_context_negatives() -> Result<()> {
    let mut d = directed(1, None);
    d.context = contexts()[0].clone();
    d.author = 1;
    d.id = 601;
    let one =
        oracle::History::from([(d.saved.event.clone().context("fixture EventId")?, item(&d))]);
    d.history(&oracle::History::new(), &one)?;
    for field in ["request", "event", "duplicate"] {
        let mut bad = one.clone();
        if field == "request" {
            bad.values_mut().next().context("fixture row")?.request = oracle::request_id(602);
        }
        if field == "event" {
            let row = bad.pop_first().context("fixture history")?.1;
            bad.insert("wrong-id".into(), row);
        }
        if field == "duplicate" {
            bad.insert("extra-event".into(), item(&d));
        }
        assert!(d.history(&oracle::History::new(), &bad).is_err(), "{field}");
    }
    let mut wrong = d.saved.clone();
    wrong.queue += 1;
    assert!(oracle::request_stable(&d.saved, &wrong).is_err());
    let s = status_fixture();
    let maps = oracle::mappings(&s, &contexts())?;
    let history = maps
        .iter()
        .map(|m| (key(m.owner, &m.context), oracle::History::new()))
        .collect();
    let a = Live {
        status: s.clone().map(Some),
        history,
    };
    let mut b = Live {
        status: s.map(Some),
        history: a.history.clone(),
    };
    unchanged_c2(&a, &b, &maps, &contexts()[1])?;
    b.history.insert(key(1, &contexts()[1]), one);
    assert!(unchanged_c2(&a, &b, &maps, &contexts()[1]).is_err());
    Ok(())
}

// Public CLI premise: attempt01 command-0065.stdout.log (427 bytes), SHA256
// dffa39ac571ddca57f1372f3cef66cac089a48b2dae028229aa20962883568e3.
// No private fixture is read by these pure tests.
const ACTUAL_UPGRADE_STATUS: &str = r#"{"context":"e0cb00a4683caa4b83e49aaad583bb5f80d1447a036315902f2e2e1c0a1b4811","control_debt":false,"controls":{"a":null,"committed":null,"k":null,"p":null},"coordinator":true,"cuts":[null,null],"freeze":false,"lease_current":false,"mode":"L1","native_encoding":"Legacy7","policy_error":null,"policy_paused":false,"q":null,"receive_event2":false,"send_event2":false,"slot":0,"status":"NoLocalIntent","version":3,"wire_stage":1}"#;
fn actual_y_mapping() -> oracle::Mapping {
    oracle::Mapping {
        owner: 0,
        role: 0,
        slot: 0,
        context: "e0cb00a4683caa4b83e49aaad583bb5f80d1447a036315902f2e2e1c0a1b4811".into(),
    }
}
#[test]
fn b_start_actual_upgrade_status_object_and_distinct_upgrade_hex() -> Result<()> {
    let y = actual_y_mapping();
    let object = parse_upgrade_status(ACTUAL_UPGRADE_STATUS, &y)?;
    assert_eq!(object["status"], "NoLocalIntent");
    assert!(decode_upgrade_reply(ACTUAL_UPGRADE_STATUS, &y).is_err());
    let maps = oracle::mappings(&status_fixture(), &contexts())?;
    let b = mapping(&maps, 1, &contexts()[0])?;
    let pending = pending_reply();
    let bytes = serde_json::to_vec(&pending)?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let encoded = serde_json::to_string(&hex)?;
    assert_eq!(decode_upgrade_reply(&encoded, b)?, pending);
    assert!(parse_upgrade_status(&encoded, &y).is_err());
    for shape in ["null", "[]", "3", "true", "\"status\""] {
        assert!(parse_upgrade_status(shape, &y).is_err());
        assert!(decode_upgrade_reply(shape, b).is_err());
        let hex = shape
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(decode_upgrade_reply(&serde_json::to_string(&hex)?, b).is_err());
    }
    assert!(parse_upgrade_status(&" ".repeat(STATUS_CAP + 1), &y).is_err());
    assert!(decode_upgrade_reply(&" ".repeat(2 * STATUS_CAP + 4), b).is_err());
    assert!(decode_upgrade_reply("\"gg\"", b).is_err());
    Ok(())
}
#[test]
fn b_start_status_version_mapping_and_premise_negatives() -> Result<()> {
    let y = actual_y_mapping();
    let good = parse_upgrade_status(ACTUAL_UPGRADE_STATUS, &y)?;
    for (key, value) in [
        ("version", json!(2)),
        ("version", json!("3")),
        ("context", json!(contexts()[1])),
        ("slot", json!(1)),
        ("mode", json!("Active")),
        ("status", json!("LocalIntentPending")),
        ("lease_current", json!(true)),
        ("control_debt", json!(true)),
        ("q", json!("selected")),
        ("policy_error", json!("blocked")),
    ] {
        let mut bad = good.clone();
        bad[key] = value;
        assert!(
            parse_upgrade_status(&serde_json::to_string(&bad)?, &y).is_err(),
            "{key}"
        );
    }
    for key in ["version", "context", "slot", "policy_error"] {
        let mut bad = good.clone();
        bad.as_object_mut().context("fixture object")?.remove(key);
        assert!(
            parse_upgrade_status(&serde_json::to_string(&bad)?, &y).is_err(),
            "missing {key}"
        );
    }
    let maps = oracle::mappings(&status_fixture(), &contexts())?;
    let b = mapping(&maps, 1, &contexts()[0])?;
    for (key, value) in [
        ("version", json!(2)),
        ("context", json!(contexts()[1])),
        ("slot", json!(0)),
    ] {
        let mut bad = pending_reply();
        bad[key] = value;
        let hex = serde_json::to_vec(&bad)?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(
            decode_upgrade_reply(&serde_json::to_string(&hex)?, b).is_err(),
            "Upgrade {key}"
        );
        assert!(pending(&bad, b).is_err(), "B gate {key}");
    }
    let mut wrong = y.clone();
    wrong.owner = 1;
    assert!(parse_upgrade_status(ACTUAL_UPGRADE_STATUS, &wrong).is_err());
    Ok(())
}
