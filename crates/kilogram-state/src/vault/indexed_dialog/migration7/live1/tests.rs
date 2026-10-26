//! Structural negatives and credit accounting; no synthetic L1 origin claim.
use super::*;

#[test]
fn indexed_l1_review1_captures_preserve_applicable_f_and_nonapplicable_roles() -> Result<()> {
    // Applicability/codec samples, no synthetic Native or Event origin.
    for text in [false, true] {
        check_capture_shape(true, text, [false, true, false, false])?;
        assert!(check_capture_shape(true, text, [false, false, false, false]).is_err());
        assert!(check_capture_shape(true, text, [true, true, false, false]).is_err());
        assert!(check_capture_shape(true, text, [false, true, true, false]).is_err());
    }
    check_capture_shape(false, false, [true, true, false, false])?;
    check_capture_shape(false, true, [true, true, true, true])?;
    for missing in 0..4 {
        let mut fields = [true; 4];
        fields[missing] = false;
        assert!(check_capture_shape(false, true, fields).is_err());
    }
    Ok(())
}

#[test]
fn indexed_l1_review1_closed_operation_rejects_missing_final_capture() -> Result<()> {
    let raw = vec![1];
    let auth = HistoryAuth {
        id: hash(b"", &raw),
        bytes: raw,
    };
    let mut row = L1Readable {
        author: [2; 32],
        source: vec![],
        text: Some("opaque envelope sample".into()),
        original: auth.clone(),
        admission: None,
        final_auth: None,
        receiver: None,
        receipt: None,
        queue: Some(1),
    };
    let operation = |row| L1Operation::Materialize {
        change: L1NativeChange {
            context: 0,
            expected_head: [0; 32],
            policy_generation: 1,
            wall_time: 1,
            head: vec![],
            native: Some(vec![1]),
        },
        rows: vec![row],
    };
    assert!(matches!(
        operation(row.clone()).bound(),
        Err(Error::Integrity("post-Ready final capture missing"))
    ));
    row.final_auth = Some(auth);
    operation(row).bound()?; // Envelope bounds only, not valid Event/Native authorization.
    Ok(())
}

#[test]
fn indexed_l1_wrong_purpose_precedes_generation_and_exact_retry() -> Result<()> {
    let config = Config::default();
    for fresh_runtime in [false, true] {
        let dir = tempfile::tempdir()?;
        let claim = CreationClaim {
            account: [1; 32],
            claim: [2; 32],
            signing_secret: Zeroizing::new([3; 32]),
            encryption_secret: Zeroizing::new([4; 32]),
            contexts: [[5; 32], [6; 32]],
        };
        let mut core = if fresh_runtime {
            let mut repo = RuntimeRepository::create(
                StateDirectoryLock::acquire(dir.path())?,
                claim,
                RuntimeEnrollment {
                    device: [7; 32],
                    certificate: vec![8; 32],
                    authority: vec![9; 32],
                    authority_revision: 1,
                },
                config,
            )?;
            repo.commit(
                repo.generation(),
                [10; 32],
                RuntimeOperation::PinAuthority {
                    account: [1; 32],
                    revision: 2,
                    raw: vec![21; 64],
                    expected_policy: 1,
                },
                &mut || Ok(()),
            )?;
            repo.store
        } else {
            let mut core = IndexedDialogVault::create(
                StateDirectoryLock::acquire(dir.path())?,
                claim,
                config,
            )?;
            core.commit(
                core.generation(),
                [10; 32],
                vec![Mutation::Put {
                    key: Key::hash(0, Namespace::Event, [11; 32])?,
                    value: vec![12],
                }],
                vec![],
            )?;
            core
        };
        let before = core.root.encode();
        for id in [core.root.commit_id, [13; 32]] {
            assert_ne!(id, [0; 32]);
            let operation = L1Operation::Queue {
                context: 0,
                request: [14; 16],
                text_hash: [15; 32],
                sealed: vec![],
                now: 0,
            };
            assert!(matches!(
                core.commit_with(
                    u64::MAX,
                    id,
                    core.root.request_digest,
                    CommitInput::Legacy1(Box::new(operation)),
                    1
                ),
                Err(Error::RuntimeWriterRequired)
            ));
            assert_eq!(core.root.encode(), before);
        }
    }
    Ok(())
}

#[test]
fn indexed_l1_held64_separate_head_no_refund_or_cross_context() -> Result<()> {
    let mut one = credits::Ledger::migration_unfunded(1024 * 1024 * 1024);
    let mut two = one.clone();
    one.migration_ready(0, None, true)?;
    two.migration_ready(0, None, true)?;
    let other = two.clone();
    for n in 2..66 {
        one.legacy1_admit_pending(n, false)?;
    }
    let full = one.clone();
    assert!(matches!(
        one.legacy1_admit_pending(66, false),
        Err(Error::Quota)
    ));
    assert!(one == full && two == other);
    one.legacy1_admit_pending(1, true)?;
    assert!(one.legacy1_admit_pending(0, true).is_err());
    assert!(one.legacy1_retire_pending(2, true).is_err());
    one.legacy1_retire_pending(1, true)?;
    for n in 2..66 {
        one.legacy1_retire_pending(n, false)?;
    }
    assert!(one.legacy1_retire_pending(66, false).is_err());
    assert!(one == other && two == other);
    assert!(one.migration_ready(0, None, true).is_err());
    assert!(one.legacy1_admit_pending(512, false).is_err());
    Ok(())
}

#[test]
fn indexed_l1_closed_metadata_envelope_and_input_caps() -> Result<()> {
    let request = L1Request {
        queue_sequence: Some(u64::MAX),
        event: None,
        text_hash: [3; 32],
    };
    let mut raw = packed(&request)?;
    assert_eq!(unpack::<L1Request>(&raw)?.queue_sequence, Some(u64::MAX));
    raw.push(0);
    assert!(unpack::<L1Request>(&raw).is_err());
    assert!(
        L1Operation::Queue {
            context: 2,
            request: [1; 16],
            text_hash: [2; 32],
            sealed: vec![],
            now: 0
        }
        .bound()
        .is_err()
    );
    assert!(
        L1Operation::Queue {
            context: 0,
            request: [1; 16],
            text_hash: [2; 32],
            sealed: vec![0; 8193],
            now: 0
        }
        .bound()
        .is_err()
    );
    assert!(
        L1Operation::AdmissionPair {
            context: 0,
            pair: vec![0; 192 * 1024 + 1]
        }
        .bound()
        .is_err()
    );
    Ok(())
}
