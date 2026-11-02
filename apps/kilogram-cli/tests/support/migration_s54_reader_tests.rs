//! Synthetic validation gates, signed A3 seam; no actual repository/profile access.
use super::super::records_fixture::Fixture;
use super::*;
fn mode() -> store::ExistingState7 {
    store::ExistingState7 {
        mode: store::ExistingMode::Active,
        current_consent: Some([4; 32]),
        selected_consent: Some([4; 32]),
        q: Some([5; 32]),
        cuts: [Some(25); 2],
        propose: Some([6; 32]),
        accept: Some([7; 32]),
        select: Some([8; 32]),
        committed: Some([9; 32]),
        ..Default::default()
    }
}
fn meta(c: usize) -> store::L1Metadata {
    store::L1Metadata {
        policy_generation: 1,
        wall_high_water: 1,
        next: [limit(c) + 1; 2],
        tails: [Some([1; 32]); 2],
        next_queue: 1,
        request_index_after: Some(2),
        request_index_ready: true,
        ack_next: 1,
        peer_source_next: 1,
        peer_source_last: None,
    }
}
fn repair() -> db::RuntimeRepairState {
    db::RuntimeRepairState {
        repair_free: 1,
        outstanding: 0,
        pending: 0,
        head: None,
        grant: None,
    }
}
fn counts(c: usize) -> (db::RuntimeCounts, store::L1SourceCounts) {
    (
        db::RuntimeCounts {
            sources: limit(c) * 2 + 2,
            queued: 0,
            debt: 0,
            pending: 0,
            receipts: [limit(c); 2],
        },
        store::L1SourceCounts {
            lanes: [limit(c) + 1; 2],
            projections: limit(c) * 2,
            requests: limit(c),
        },
    )
}
#[test]
fn s54_p11a5_original_ready_slot_mapping_rejects_foreign_and_duplicate_owner() -> Result<()> {
    let ids = [[1; 32], [2; 32]];
    let ready =
        json!({"contexts":[{"context":state::hex(&ids[1])},{"context":state::hex(&ids[0])}]});
    ensure!(
        gates::mapping(1, &[(ids[1], 1), (ids[0], 1)], ids, &ready)? == [1, 0],
        "logical mapping independent of physical order"
    );
    for rows in [
        vec![(ids[0], 1), (ids[1], 1)],
        vec![(ids[1], 0), (ids[0], 1)],
        vec![(ids[1], 1), ([99; 32], 1)],
        vec![(ids[1], 1)],
    ] {
        ensure!(
            gates::mapping(1, &rows, ids, &ready).is_err(),
            "foreign Ready/role/context"
        );
    }
    ensure!(
        gates::mapping(2, &[(ids[1], 1), (ids[0], 1)], ids, &ready).is_err()
            && gates::mapping(1, &[(ids[1], 1), (ids[0], 1)], [ids[0]; 2], &ready).is_err(),
        "bounded distinct context mapping"
    );
    Ok(())
}
#[test]
fn s54_p11a5_exact_root_counts_exclude_all_relevant_distant_extras() -> Result<()> {
    for c in 0..2 {
        let (r, n) = counts(c);
        gates::counts(c, &r, &n, &meta(c), &repair(), &mode())?;
        for change in 0..9 {
            let (mut r, mut n) = counts(c);
            match change {
                0 => r.sources += 1,
                1 => n.lanes[0] += 1,
                2 => n.lanes[1] -= 1,
                3 => n.projections += 1,
                4 => n.requests += 1,
                5 => r.receipts[0] += 1,
                6 => r.receipts[1] -= 1,
                7 => r.debt = 1,
                _ => r.pending = 1,
            }
            ensure!(
                gates::counts(c, &r, &n, &meta(c), &repair(), &mode()).is_err(),
                "exact authenticated namespace count"
            );
        }
        let (mut r, n) = counts(c);
        r.queued = 1;
        ensure!(
            gates::counts(c, &r, &n, &meta(c), &repair(), &mode()).is_err(),
            "empty queue count"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a5_terminal_next_and_repair_obligations_cannot_be_hidden() -> Result<()> {
    let (r, n) = counts(0);
    // A completed queue-index preparation retains its after watermark.
    gates::counts(0, &r, &n, &meta(0), &repair(), &mode())?;
    let mut empty_grant = repair();
    empty_grant.grant = Some(([1; 32], 385, 640, [0; 32]));
    gates::counts(0, &r, &n, &meta(0), &empty_grant, &mode())?;
    empty_grant.grant.as_mut().context("synthetic grant")?.3[0] = 1;
    ensure!(
        gates::counts(0, &r, &n, &meta(0), &empty_grant, &mode()).is_err(),
        "remaining funded grant obligation"
    );
    for i in 0..7 {
        let mut m = meta(0);
        let mut p = repair();
        let mut state = mode();
        match i {
            0 => m.next[1] = 640,
            1 => m.tails[0] = None,
            2 => m.request_index_ready = false,
            3 => p.head = Some(640),
            4 => p.pending = 1,
            5 => p.outstanding = 1,
            _ => state.control_debt = true,
        }
        ensure!(
            gates::counts(0, &r, &n, &m, &p, &state).is_err(),
            "no terminal repair/control/prefix obligation"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a5_both_active_original_control_scope_is_required() -> Result<()> {
    for c in 0..2 {
        let scope = gates::Scope {
            c: [1; 32],
            d: [2; 32],
            e: [3; 32],
            sid: "S",
        };
        let mut state = mode();
        state.cuts = [Some(if c == 0 { 25 } else { 33 }); 2];
        let actual = json!({"context":state::hex(&scope.c),"role":1,"d":state::hex(&scope.d),"e":state::hex(&scope.e),"sid":scope.sid,"mode":"Active","q":state::hex(&[5;32]),"cuts":state.cuts,"current_consent2":state::hex(&[4;32]),"controls":{"p":state::hex(&[6;32]),"a":state::hex(&[7;32]),"k":state::hex(&[8;32]),"committed":state::hex(&[9;32])}});
        gates::active(c, 1, &scope, &state, &actual)?;
        for field in [
            "context",
            "role",
            "d",
            "e",
            "sid",
            "mode",
            "q",
            "cuts",
            "current_consent2",
            "controls",
        ] {
            let mut bad = actual.clone();
            bad[field] = Value::Null;
            ensure!(
                gates::active(c, 1, &scope, &state, &bad).is_err(),
                "original Active scope field {field}"
            );
        }
        let mut bad = state.clone();
        bad.mode = store::ExistingMode::Selected;
        ensure!(
            gates::active(c, 1, &scope, &bad, &actual).is_err(),
            "stored Selected cannot replace Active"
        );
        bad = state;
        bad.selected_consent = Some([99; 32]);
        ensure!(
            gates::active(c, 1, &scope, &bad, &actual).is_err(),
            "selected and current consent association"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a5_actual_a3_projection_request_and_b640_gates()
-> super::super::records_fixture::TestResult {
    let f = Fixture::new(0)?;
    let case = f.case(1, 640, 2)?;
    let row = f.check(&case)?;
    let request = store::L1Request {
        queue_sequence: Some(1),
        event: Some(row.event),
        text_hash: row.body_hash,
    };
    gates::own_request(1, &row, Some(&request))?;
    gates::own_request(0, &row, None)?;
    assert!(
        gates::own_request(1, &row, None).is_err()
            && gates::own_request(0, &row, Some(&request)).is_err()
    );
    let mut bad = request.clone();
    bad.event = Some([99; 32]);
    assert!(gates::own_request(1, &row, Some(&bad)).is_err());
    bad = request;
    bad.text_hash = [99; 32];
    assert!(gates::own_request(1, &row, Some(&bad)).is_err());
    let mid = [1; 32];
    let source = [2; 32];
    let projection = db::ProjectionValue {
        text: case.body.clone(),
        binding: old::encode(
            &old::a([
                old::u(7),
                old::b(mid),
                old::b(source),
                old::b([3; 32]),
                old::b(row.request),
            ]),
            2304,
        )?,
    };
    assert_eq!(gates::projection(&projection, mid, source)?, row.request);
    assert!(
        gates::projection(&projection, [99; 32], source).is_err()
            && gates::projection(&projection, mid, [99; 32]).is_err()
    );
    let held = json!({"request":state::hex(&row.request),"id":state::hex(&row.event),"source_blake3":state::hex(&row.source_hash),"ciphertext_blake3":state::hex(&row.ciphertext_hash)});
    gates::held(&row, &held)?;
    for field in ["request", "id", "source_blake3", "ciphertext_blake3"] {
        let mut bad = held.clone();
        bad[field] = state::hex(&[99; 32]).into();
        assert!(gates::held(&row, &bad).is_err());
    }
    Ok(())
}
#[test]
fn s54_p11a5_history_must_match_original_projection_and_verified_receipt() -> Result<()> {
    let original = json!({"format":2,"id":state::hex(&[1;32]),"role":0,"ordinal":25,"request":source::request_id(oracle::Request::new(0,0,25)?),"text":oracle::Request::new(0,0,25)?.body()});
    let mut actual = original.clone();
    actual["delivered"] = true.into();
    ensure!(
        gates::history_row(&actual, &original)? == [1; 32],
        "actual original history association"
    );
    for field in [
        "format",
        "id",
        "role",
        "ordinal",
        "request",
        "text",
        "delivered",
    ] {
        let mut bad = actual.clone();
        bad[field] = Value::Null;
        ensure!(
            gates::history_row(&bad, &original).is_err(),
            "wrong actual history association"
        );
    }
    Ok(())
}
