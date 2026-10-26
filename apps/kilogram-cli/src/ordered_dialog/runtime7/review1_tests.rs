//! Component/negative parser checks only. Genuine owner/Native lineage is a
//! separately budgeted field regression, never inferred from these Head values.
use super::*;

#[test]
fn indexed_l1_review1_materialization_eligibility_is_before_native() -> Result<()> {
    assert_eq!(data::materialize_limit(4, 127)?, 1);
    assert_eq!(data::materialize_limit(3, 128)?, 0);
    assert_eq!(data::materialize_limit(0, 127)?, 0);
    assert_eq!(data::materialize_limit(128, 0)?, 4);
    assert!(data::materialize_limit(4, 129).is_err());
    Ok(())
}

#[test]
fn indexed_l1_review1_ready_scope_shape_precedes_genesis_classification() -> Result<()> {
    for own_role in 0..2 {
        let mut head = Head::absent([1; 32], [2; 32], own_role);
        head.d = Some([3; 32]);
        head.e = Some([4; 32]);
        head.sid = Some("x".repeat(43));
        head.phase = 3;
        let peer = 1 - own_role;
        let fields = vec![
            b([3; 32]),
            b([4; 32]),
            b("x".repeat(43)),
            b([5; 32]),
            b([6; 32]),
            u(peer),
            u(1),
            u(1),
        ];
        assert_eq!(
            bootstrap::ready_claims(&head, peer, &a(fields.clone()))?,
            [[5; 32], [6; 32]]
        );
        for index in 0..8 {
            let mut wrong = fields.clone();
            wrong[index] = match index {
                0 | 1 => b([9; 32]),
                2 => b("y".repeat(43)),
                3 | 4 => b([9; 31]),
                5 => u(own_role),
                _ => u(2),
            };
            assert!(
                bootstrap::ready_claims(&head, peer, &a(wrong)).is_err(),
                "malformed/scope field{index}"
            );
        }
        let mut short = fields.clone();
        short.pop();
        assert!(bootstrap::ready_claims(&head, peer, &a(short)).is_err());
        let mut conflict = fields;
        conflict[3] = b([9; 32]);
        assert_eq!(
            bootstrap::ready_claims(&head, peer, &a(conflict))?,
            [[9; 32], [6; 32]],
            "valid-scope differing claims reach the owner fork comparison"
        );
    }
    Ok(())
}
