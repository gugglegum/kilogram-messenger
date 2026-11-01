use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
use receipts::AckContents;
fn signer() -> kilogram_identity::DeviceIdentity {
    kilogram_identity::DeviceIdentity::from_secret_bytes([7; 32])
}
pub(super) fn r1(n: u64, event: Id, lane: u8) -> Result<Vec<u8>> {
    let body = wire::a([
        wire::u(1),
        wire::b([2; 32]),
        wire::b([3; 32]),
        wire::u(lane.into()),
        wire::u(n),
        wire::b(event),
        wire::b([7; 32]),
        wire::b([8; 32]),
    ]);
    let sig = signer().sign(&wire::signing_bytes(wire::RECEIPT_SIGN, &body, 1024)?);
    Ok(wire::encode(&wire::a([body, wire::b(sig)]), 1024)?)
}
fn r2(n: u64, event: Id, lane: u8, source_format: u8) -> Result<Vec<u8>> {
    Ok(v2::ParsedReceipt2::sign(
        v2::ReceiptFields2 {
            c: [1; 32],
            d: [2; 32],
            e: [3; 32],
            sid: "A".repeat(43),
            q: [4; 32],
            format: source_format,
            lane,
            n,
            event,
            receiver_cert: [7; 32],
            receiver_auth: [8; 32],
        },
        &signer(),
    )?
    .raw()
    .to_vec())
}
fn body(items: &[(u8, Vec<u8>)], lane: u8) -> wire::Value {
    wire::a([
        wire::b([1; 32]),
        wire::b([2; 32]),
        wire::b([3; 32]),
        wire::b("A".repeat(43).as_bytes()),
        wire::u(lane.into()),
        wire::a([]),
        wire::a(
            items
                .iter()
                .map(|(tag, raw)| wire::a([wire::u((*tag).into()), wire::b(raw)])),
        ),
        wire::a([]),
    ])
}
#[test]
fn s54_p9a_receipt_parser_distinguishes_receipt_encoding_from_source_format() -> Result<()> {
    let items = vec![
        (1, r1(24, [24; 32], 0)?),
        (2, r2(25, [25; 32], 0, 1)?),
        (2, r2(385, [85; 32], 0, 2)?),
    ];
    let ack = AckContents::parse(v2::FrameKind::AckBatch, &body(&items, 0))?
        .context("actual nonempty mixed batch")?;
    ensure!(
        ack.tuples()
            .iter()
            .map(|r| (r.0, r.1, r.2, r.3))
            .collect::<Vec<_>>()
            == [(1, 1, 0, 24), (2, 1, 0, 25), (2, 2, 0, 385)],
        "actual parsed encoding/source/lane/ordinal"
    );
    ensure!(
        ack.receipts[2].digest == *blake3::hash(&items[2].1).as_bytes(),
        "digest exact saved receipt bytes"
    );
    ensure!(
        ack.matches(None, 0, true)
            && ack.matches(Some(385), 1, false)
            && !ack.matches(Some(386), 1, false)
            && !ack.matches(Some(385), 1, true)
            && !ack.matches(Some(385), 0, false),
        "default and targeted actual push gates"
    );
    let old = AckContents::parse(v2::FrameKind::AckBatch, &body(&items[..2], 0))?
        .context("old duplicates")?;
    ensure!(
        !old.matches(Some(385), 1, false),
        "old-only pushes advance normally"
    );
    let other = AckContents::parse(
        v2::FrameKind::AckBatch,
        &body(&[(2, r2(640, [6; 32], 1, 2)?)], 1),
    )?
    .context("B640 opposite lane")?;
    ensure!(
        !other.matches(Some(385), 1, false),
        "B640 is not required A receipt push"
    );
    Ok(())
}
#[test]
fn s54_p9a_receipt_parser_rejects_tag_scope_lane_ordinal_duplicates_and_malformed() -> Result<()> {
    for items in [
        vec![(1, r2(385, [1; 32], 0, 2)?)],
        vec![(2, r1(24, [1; 32], 0)?)],
        vec![(2, r2(385, [1; 32], 1, 2)?)],
        vec![(2, r2(4097, [1; 32], 0, 2)?)],
        vec![(2, r2(385, [1; 32], 0, 2)?), (2, r2(385, [2; 32], 0, 2)?)],
        vec![(2, r2(385, [1; 32], 0, 2)?), (2, r2(386, [1; 32], 0, 2)?)],
        vec![(2, vec![0; 16])],
    ] {
        ensure!(
            AckContents::parse(v2::FrameKind::AckBatch, &body(&items, 0)).is_err(),
            "actual parser malformed/conflicting receipt"
        );
    }
    let mut bad = body(&[(2, r2(385, [1; 32], 0, 2)?)], 0);
    if let wire::Value::A(ref mut f) = bad {
        f[0] = wire::b([9; 32]);
    }
    ensure!(
        AckContents::parse(v2::FrameKind::AckBatch, &bad).is_err(),
        "actual signed receipt scope differs from batch"
    );
    ensure!(
        AckContents::parse(v2::FrameKind::AckBatch, &body(&[], 0))?.is_none()
            && AckContents::parse(v2::FrameKind::AckQuery, &wire::a([]))?.is_none(),
        "empty/query is not saved ACK"
    );
    Ok(())
}
#[test]
fn s54_p9a_ack_selector_defaults_schema_and_atomic_release_are_closed() -> Result<()> {
    let mut plan = json!({"context":state::hex(&[1;32]),"lane":1,"phase":"catch_up","hold_ack":true,"max_pause_ms":120000});
    for value in [None, Some(Json::Null)] {
        if let Some(v) = value {
            plan["hold_ack_from"] = v;
        };
        ensure!(
            Schedule::parse(&serde_json::to_vec(&plan)?, true)?
                .hold_ack_from
                .is_none(),
            "absent/null default"
        );
    }
    plan["hold_ack_from"] = 385.into();
    Schedule::parse(&serde_json::to_vec(&plan)?, true)?;
    for (key, value) in [
        ("lane", json!(0)),
        ("phase", json!("pressure")),
        ("hold_ack", json!(false)),
        ("hold_ack_from", json!(0)),
        ("hold_ack_from", json!(4097)),
        ("hold_ack_from", json!(u64::MAX)),
    ] {
        let mut bad = plan.clone();
        bad[key] = value;
        ensure!(
            Schedule::parse(&serde_json::to_vec(&bad)?, true).is_err(),
            "reject {key}"
        );
    }
    ensure!(
        Schedule::parse(&serde_json::to_vec(&plan)?, false).is_err(),
        "release refused"
    );
    plan["hold_ack"] = false.into();
    ensure!(
        Schedule::parse(&serde_json::to_vec(&plan)?, true).is_err(),
        "partial release invalid"
    );
    plan["hold_ack_from"] = Json::Null;
    Schedule::parse(&serde_json::to_vec(&plan)?, true)?;
    Ok(())
}
#[test]
fn s54_p9a_max16_actual_receipt_facts_fit_complete4096_marker() -> Result<()> {
    let items = (4081..=4096)
        .map(|n| {
            let mut id = [0xff; 32];
            id[..8].copy_from_slice(&u64::to_be_bytes(n));
            Ok((2, r2(n, id, 0, 2)?))
        })
        .collect::<Result<Vec<_>>>()?;
    let ack = AckContents::parse(v2::FrameKind::AckBatch, &body(&items, 0))?
        .context("max16 actual parsed receipts")?;
    let base = tempfile::tempdir()?;
    let root = base.path().join("state");
    std::fs::create_dir(&root)?;
    let file = base.path().join("control.json");
    std::fs::write(
        &file,
        serde_json::to_vec(
            &json!({"context":state::hex(&[1;32]),"lane":1,"phase":"catch_up","hold_ack":true,"hold_ack_from":385,"max_pause_ms":120000}),
        )?,
    )?;
    let observer = Observer {
        location: Some(Location::new(&file, &root, true)?),
        incarnation: [0xff; 32],
        owner: [0xff; 32],
        sequence: 4096,
        pages: [0; 2],
        last_page: [None; 2],
        heads: [None; 2],
    };
    let facts = ack.facts([1; 32], [0xff; 32], u64::MAX, [0xff; 16], false, Some(385));
    let pause = observer
        .pause([1; 32], 1, PauseKind::Ack, facts.clone())?
        .context("actual ACK pause")?;
    let mut maximal = pause.event.clone();
    maximal["pid"] = u32::MAX.into();
    ensure!(
        serde_json::to_vec(&maximal)?.len() <= 4096,
        "full envelope, max counters/IDs and16 actual receipts within original marker cap"
    );
    observer
        .location
        .as_ref()
        .context("pinned location")?
        .marker("ack", &maximal)?;
    let actual = std::fs::read(file.with_extension("s54-ack.json"))?;
    ensure!(
        actual.len() <= 4096 && serde_json::from_slice::<Json>(&actual)? == maximal,
        "real marker writer enforces unchanged cap"
    );
    let mut over = items;
    over.push((2, r2(1, [1; 32], 0, 2)?));
    ensure!(
        AckContents::parse(v2::FrameKind::AckBatch, &body(&over, 0)).is_err(),
        "17 never retained"
    );
    Ok(())
}
