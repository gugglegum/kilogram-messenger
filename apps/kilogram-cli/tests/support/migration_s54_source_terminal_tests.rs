//! Immutable byte comparison seam only, not actual storage or scope authentication.
use super::*;
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
