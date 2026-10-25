use super::*;
use crate::ordered_dialog_v2 as wire;
#[allow(dead_code)]
mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/ordered_dialog_v2.rs"
    ));
}
use support::TestResult;
mod golden;
mod matrix;
#[test]
fn a3_s3_root_selected_proof_and_authority() -> TestResult {
    let actors = support::Actors::new([5; 32])?;
    let fresh = actors.fresh([61; 32], [62; 32], [63; 32])?;
    assert_eq!(fresh.proof.q(), hash(MODE, &fresh.raw));
    for role in 0..2 {
        let renewed = actors.renewal(role, 3)?;
        assert_eq!(renewed.cert_ref(), actors.credentials[role].cert_ref());
        assert_ne!(renewed.reference(), actors.credentials[role].reference());
    }
    let mut bad = actors.credentials[0].raw().to_vec();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(ParsedCredentialBundle::parse(&bad)?.verify_root().is_err());
    let mut wrong = fresh.raw.clone();
    let last = wrong.len() - 1;
    wrong[last] ^= 1;
    let p = ParsedModeProof2::parse(&wrong)?;
    assert!(
        p.verify(&SelectionEvidence2 {
            peers: actors.credentials.clone(),
            membership: actors.membership.clone(),
            initial_pair: Some(fresh.pair),
            previous: None
        })
        .is_err()
    );
    Ok(())
}
