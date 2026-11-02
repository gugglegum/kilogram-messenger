//! Immutable byte comparison seam only, not actual storage or scope authentication.
use super::*;
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
