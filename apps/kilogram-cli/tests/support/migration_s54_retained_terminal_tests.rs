//! Actual P7 immutable-row bridge over labelled synthetic normalized metadata.
use super::*;
#[test]
fn s54_p11a5_terminal_p7_bridge_rejects_changed_anchor_and_retained_commitments() -> Result<()> {
    let mut row = records::Record {
        format: 2,
        event: [1; 32],
        context: [2; 32],
        d: [3; 32],
        e: [4; 32],
        sid: "S".repeat(43),
        selection: [5; 32],
        role: 0,
        ordinal: 385,
        request: [6; 16],
        previous: Some([7; 32]),
        parents: vec![[7; 32]],
        sequence: 8,
        author_auth: [9; 32],
        source_hash: [10; 32],
        ciphertext_hash: [11; 32],
        body_hash: [12; 32],
        receipt_format: 2,
        receipt_hash: [13; 32],
        receiver_auth: [14; 32],
    };
    let original = Immutable {
        state: "synthetic".into(),
        scope: Scope {
            source: [15; 32],
            migration: [16; 32],
            context: row.context,
            q: row.selection,
            d: row.d,
            e: row.e,
            sid: row.sid.clone(),
            identity: [17; 32],
        },
        anchor: [7; 32],
        sources: vec![Source {
            ordinal: 385,
            request: row.request,
            id: row.event,
            previous: [7; 32],
            source_hash: row.source_hash,
            ciphertext_hash: row.ciphertext_hash,
            text_hash: row.body_hash,
        }],
    };
    original.terminal_record(&row)?;
    for i in 0..6 {
        let mut bad = row.clone();
        match i {
            0 => bad.request = [99; 16],
            1 => bad.event = [99; 32],
            2 => bad.previous = Some([99; 32]),
            3 => bad.source_hash = [99; 32],
            4 => bad.ciphertext_hash = [99; 32],
            _ => bad.body_hash = [99; 32],
        };
        ensure!(
            original.terminal_record(&bad).is_err(),
            "retained actual immutable P7 field"
        );
    }
    row.ordinal = 384;
    row.event = [7; 32];
    original.terminal_record(&row)?;
    row.event = [99; 32];
    ensure!(
        original.terminal_record(&row).is_err(),
        "original anchor384"
    );
    Ok(())
}
