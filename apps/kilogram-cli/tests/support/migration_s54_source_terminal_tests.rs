//! Public source-witness helper regressions; no historical storage or private data.
use super::*;
fn queue_positive(purpose: Purpose, numbers: [u16; 2]) -> Result<()> {
    let mut requests = QueuedRequests::new(purpose);
    ensure!(
        requests.seen.is_empty(),
        "empty initial queue remains valid"
    );
    for n in numbers {
        let expected = oracle::Request::new(0, 0, n)?;
        let name = request_id(expected);
        let (id, request) = requests.matching(0, 0, &name)?;
        ensure!(
            id == parse_request_id(&name)? && request == expected,
            "actual matcher preserves exact canonical request and body constructor"
        );
    }
    ensure!(requests.seen.len() == 2, "both exact requests matched once");
    Ok(())
}
#[test]
fn s54_source_queue_whole_p_retains_exact17_18_and_empty_initial_queue() -> Result<()> {
    queue_positive(Purpose::WholeP, [17, 18])?;
    println!("s54_source_queue_P17_18=accepted;empty_allowed=true");
    Ok(())
}
#[test]
fn s54_source_queue_feasibility_f_accepts_exact3_4_and_empty_initial_queue() -> Result<()> {
    queue_positive(Purpose::FeasibilityF, [3, 4])?;
    println!("s54_source_queue_F3_4=accepted;empty_allowed=true");
    Ok(())
}
#[test]
fn s54_source_queue_refuses_cross_purpose_scope_ids_duplicates_and_third_entry() -> Result<()> {
    let mut checked = 0;
    for (purpose, own, foreign) in [
        (Purpose::FeasibilityF, [3, 4], [17, 18]),
        (Purpose::WholeP, [17, 18], [3, 4]),
    ] {
        for n in foreign.into_iter().chain([1, 2, 5, 16, 19]) {
            let id = request_id(oracle::Request::new(0, 0, n)?);
            ensure!(
                QueuedRequests::new(purpose).matching(0, 0, &id).is_err(),
                "cross-purpose and unrecognized ordinal refused"
            );
            checked += 1;
        }
        let id = request_id(oracle::Request::new(0, 0, own[0])?);
        for (context, role) in [(1, 0), (0, 1)] {
            ensure!(
                QueuedRequests::new(purpose)
                    .matching(context, role, &id)
                    .is_err(),
                "only actual A C1 queue"
            );
            checked += 1;
        }
        for (context, role) in [(1, 0), (0, 1)] {
            let wrong = request_id(oracle::Request::new(context, role, own[0])?);
            ensure!(
                QueuedRequests::new(purpose).matching(0, 0, &wrong).is_err(),
                "request itself binds original A C1 scope"
            );
            checked += 1;
        }
        for wrong in [
            id.to_uppercase(),
            id[..31].to_owned(),
            format!("g{}", &id[1..]),
            format!("fb{}", &id[2..]),
        ] {
            ensure!(
                QueuedRequests::new(purpose).matching(0, 0, &wrong).is_err(),
                "canonical exact request ID required"
            );
            checked += 1;
        }
        let mut duplicate = QueuedRequests::new(purpose);
        duplicate.matching(0, 0, &id)?;
        ensure!(
            duplicate.matching(0, 0, &id).is_err(),
            "duplicate request refused"
        );
        checked += 1;
        let mut full = QueuedRequests::new(purpose);
        for n in own {
            full.matching(0, 0, &request_id(oracle::Request::new(0, 0, n)?))?;
        }
        ensure!(
            full.matching(0, 0, &id).is_err(),
            "third queue entry refused"
        );
        checked += 1;
    }
    ensure!(checked == 34, "all exact negative cases executed");
    println!("s54_source_queue_negative_cases={checked}");
    Ok(())
}
#[test]
fn s54_p11a6_legacy_native_identity_checks_original_sid_and_all_three_session_keys() -> Result<()> {
    let sid = "S".repeat(43);
    let keys = [[1; 32], [2; 32], [3; 32]];
    let mut head = state::Head::absent([4; 32], [5; 32], 0);
    head.sid = Some(sid.clone());
    head.phase = 3;
    head.native_generation = 2;
    head.offer = Some([9; 32]);
    head.grant = Some([10; 32]);
    head.d = Some([11; 32]);
    head.e = Some([12; 32]);
    head.blob = Some([13; 32]);
    head.validate()?;
    let original = Legacy {
        state: "synthetic-only".into(),
        signing: Zeroizing::new([6; 32]),
        encryption: Zeroizing::new([7; 32]),
        source_id: [8; 32],
        contexts: vec![SavedContext {
            c: [4; 32],
            role: 0,
            spec: vec![],
            head: head.encode()?,
            native: vec![],
            keys: keys.iter().map(|key| state::hex(key)).collect(),
            events: BTreeMap::new(),
            history: BTreeMap::new(),
            queue: BTreeMap::new(),
            receipts: BTreeMap::new(),
        }],
    };
    original.terminal_native(0, &sid, &keys)?;
    ensure!(
        original.terminal_native(0, &"X".repeat(43), &keys).is_err()
            && original.terminal_native(1, &sid, &keys).is_err(),
        "original SID/logical context"
    );
    for index in 0..3 {
        let mut bad = keys;
        bad[index] = [99; 32];
        ensure!(
            original.terminal_native(0, &sid, &bad).is_err(),
            "all original keys including base key"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a5_original_legacy_sources_and_saved_receipts_are_immutable() -> Result<()> {
    let original = Legacy {
        state: "synthetic-only".into(),
        signing: Zeroizing::new([1; 32]),
        encryption: Zeroizing::new([2; 32]),
        source_id: [3; 32],
        contexts: vec![SavedContext {
            c: [4; 32],
            role: 0,
            spec: vec![],
            head: vec![],
            native: vec![],
            keys: vec![],
            events: BTreeMap::from([((0, 0), vec![7]), ((0, 24), vec![8])]),
            history: BTreeMap::new(),
            queue: BTreeMap::new(),
            receipts: BTreeMap::from([((0, 24), vec![9])]),
        }],
    };
    original.terminal_source(0, 0, 0, &[7], None)?;
    original.terminal_source(0, 0, 24, &[8], Some(&[9]))?;
    ensure!(
        original
            .terminal_source(0, 0, 24, &[99], Some(&[9]))
            .is_err()
            && original
                .terminal_source(0, 0, 24, &[8], Some(&[99]))
                .is_err()
            && original.terminal_source(0, 0, 24, &[8], None).is_err()
            && original
                .terminal_source(1, 0, 24, &[8], Some(&[9]))
                .is_err(),
        "original source and exact saved receipt bytes, bounded context"
    );
    Ok(())
}
