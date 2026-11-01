//! Real signed synthetic records through the terminal helper, no state/process fixture.
use super::*;
#[path = "migration_s54_records_fixture.rs"]
mod fixture;
use fixture::{Fixture, TestResult};

#[test]
fn s54_p11a3_signed_text_receipts_dispatch_independently_across_both_context_cuts() -> TestResult {
    let before = kilogram_ratchet::ordered::native_call_counts();
    for index in 0..2 {
        let f = Fixture::new(index)?;
        for role in 0..2 {
            for n in if index == 0 {
                vec![1, 24, 25, 640]
            } else {
                vec![1, 32]
            } {
                for encoding in if index == 0 && n >= 25 {
                    vec![2]
                } else {
                    vec![1, 2]
                } {
                    let case = f.case(role, n, encoding)?;
                    let record = f.check(&case)?;
                    assert!(
                        record.format == if index == 0 && n >= 25 { 2 } else { 1 },
                        "actual selected source encoding"
                    );
                    assert!(
                        record.receipt_format == encoding
                            && record.event == case.id
                            && record.request == case.core.request
                            && record.role == role
                            && record.ordinal == u64::from(n)
                            && record.source_hash == digest(&case.raw)
                            && record.receipt_hash == digest(&case.receipt),
                        "actual independent receipt/source commitments"
                    );
                }
            }
        }
    }
    assert!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "signed fixture/helper performs no Native constructors/packet crypto"
    );
    Ok(())
}
#[test]
fn s54_p11a3_signed_wrong_event_encoding_or_selected_cuts_cannot_override_domain() -> TestResult {
    for (index, n, wrong) in [(0, 24, 2), (0, 25, 1), (1, 32, 2)] {
        let f = Fixture::new(index)?;
        let req = oracle::Request::new(index, 0, n)?;
        let case = f.record(req, f.core(0, n)?, wrong, 2, &f.normal)?;
        assert!(
            f.check(&case).is_err(),
            "signed wrong boundary encoding is rejected"
        );
    }
    let f = Fixture::new(0)?;
    let case = f.case(0, 24, 2)?;
    let other = f.actors.existing(
        f.selected.d(),
        f.selected.e(),
        f.selected.sid().ok_or("SID")?,
        f.genesis.clone(),
        [24, 24],
        [[81; 32], [82; 32]],
    )?;
    let other =
        v2::StaticSelection2::new(other.proof, Some(f.selected.sid().ok_or("SID")?.into()))?;
    let mut expected = f.expected(&case);
    expected.selection = other.proof().q();
    assert!(
        verify(
            &expected,
            &f.projection(&case)?,
            &other,
            Credentials {
                author: &case.author,
                receiver: &case.receiver
            },
            &case.raw,
            &case.receipt
        )
        .is_err(),
        "authenticated different selected cuts still cannot replace original25/25"
    );
    Ok(())
}
#[test]
fn s54_p11a3_original_expected_scope_id_request_and_actual_projection_must_match() -> TestResult {
    let f = Fixture::new(0)?;
    let case = f.case(0, 25, 2)?;
    for mutation in 0..8 {
        let mut expected = f.expected(&case);
        match mutation {
            0 => expected.event = [99; 32],
            1 => expected.request.number = 26,
            2 => expected.request.role = 1,
            3 => expected.request.context = 1,
            4 => expected.d = [99; 32],
            5 => expected.e = [99; 32],
            6 => expected.sid = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            _ => expected.selection = [99; 32],
        }
        assert!(
            verify(
                &expected,
                &f.projection(&case)?,
                &f.selected,
                Credentials {
                    author: &case.author,
                    receiver: &case.receiver
                },
                &case.raw,
                &case.receipt
            )
            .is_err(),
            "original expected association {mutation}"
        );
    }
    for mutation in 0..3 {
        let mut actual = f.projection(&case)?;
        match mutation {
            0 => actual.event = [99; 32],
            1 => actual.request = [99; 16],
            _ => actual.text = "foreign body",
        }
        assert!(
            verify(
                &f.expected(&case),
                &actual,
                &f.selected,
                Credentials {
                    author: &case.author,
                    receiver: &case.receiver
                },
                &case.raw,
                &case.receipt
            )
            .is_err(),
            "actual projection association {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a3_actual_protocol_rejects_foreign_source_scope_author_selection_and_signature()
-> TestResult {
    let f = Fixture::new(0)?;
    for n in [24, 25] {
        let format = if n == 24 { 1 } else { 2 };
        let expected = oracle::Request::new(0, 0, n)?;
        for mutation in 0..7 {
            let mut core = f.core(0, n)?;
            match mutation {
                0 => core.c = f.contexts[1],
                1 => core.d = [99; 32],
                2 => core.e = [99; 32],
                3 => core.sid = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
                4 => core.author_auth = f.actors.credentials[1].reference(),
                5 => core.q = [99; 32],
                _ => core.request = [99; 16],
            }
            if n == 24 && mutation == 5 {
                continue;
            } // Event1 contains no signed Q.
            let case = f.record(expected, core, format, 2, &f.normal)?;
            assert!(
                f.check(&case).is_err(),
                "actual signature with foreign signed binding {n}/{mutation}"
            );
        }
        let mut case = f.case(0, n, 2)?;
        *case.raw.last_mut().ok_or("signature")? ^= 1;
        assert!(f.check(&case).is_err(), "source signature corruption");
        let mut case = f.case(0, n, 2)?;
        case.author = f.actors.credentials[1].clone();
        assert!(f.check(&case).is_err(), "wrong actual author credential");
        case.author = f.actors.renewal(0, 3)?;
        assert!(
            f.check(&case).is_err(),
            "same certificate but wrong source AuthRecords ref"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a3_receipt_own_encoding_source_receiver_signature_and_q_are_verified() -> TestResult {
    let f = Fixture::new(0)?;
    let mut current = f.case(0, 25, 1)?;
    assert!(
        f.check(&current).is_err(),
        "Receipt1 is not allowed to acknowledge Event2"
    );
    current = f.case(0, 25, 2)?;
    let other = f.case(0, 26, 2)?;
    current.receipt = other.receipt;
    assert!(
        f.check(&current).is_err(),
        "valid receipt for another source"
    );
    let mut case = f.case(0, 24, 2)?;
    case.receiver = case.author.clone();
    assert!(
        f.check(&case).is_err(),
        "receiver must be opposite-role peer"
    );
    for encoding in [1, 2] {
        let mut case = f.case(0, 24, encoding)?;
        *case.receipt.last_mut().ok_or("receipt signature")? ^= 1;
        assert!(
            f.check(&case).is_err(),
            "receipt signature corruption, no fallback"
        );
    }
    for mutation in 0..4 {
        let mut case = f.case(0, 24, 2)?;
        let mut fields = v2::ParsedReceipt2::parse(&case.receipt)?.fields().clone();
        match mutation {
            0 => fields.q = [99; 32],
            1 => fields.format = 2,
            2 => fields.receiver_auth = case.author.reference(),
            _ => fields.lane = 1,
        }
        case.receipt = v2::ParsedReceipt2::sign(fields, &f.actors.devices[1])?
            .raw()
            .to_vec();
        assert!(
            f.check(&case).is_err(),
            "validly signed wrong receipt source/receiver/Q {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a3_actual_normalized_facts_preserve_signed_parents_sequence_and_ciphertext() -> TestResult
{
    let f = Fixture::new(0)?;
    let case = f.case(0, 25, 2)?;
    let before = f.check(&case)?;
    assert!(
        before.previous == case.core.previous
            && before.parents == case.core.parents
            && before.sequence == case.core.sequence,
        "actual signed graph facts retained"
    );
    before.same(&f.check(&case)?)?;
    let mut core = case.core.clone();
    core.parents.push([90; 32]);
    core.sequence = 700;
    let changed = f.record(case.expected, core, 2, 2, &f.normal)?;
    let after = f.check(&changed)?;
    assert!(
        after.parents == vec![[80; 32], [90; 32]]
            && after.sequence == 700
            && before.same(&after).is_err(),
        "new signed graph facts survive; actual parent existence is a later graph obligation"
    );
    let mut packet = f.normal.clone();
    *packet.bytes.last_mut().ok_or("opaque MAC")? ^= 1;
    let changed = f.record(case.expected, case.core.clone(), 2, 2, &packet)?;
    let after = f.check(&changed)?;
    assert!(
        after.source_hash != before.source_hash
            && after.ciphertext_hash != before.ciphertext_hash
            && after.body_hash == before.body_hash
            && before.same(&after).is_err(),
        "actual opaque ciphertext/raw changes despite matching supplied projection"
    );
    let peer = f.check(&f.case(1, 25, 2)?)?;
    assert!(
        peer.sequence == before.sequence,
        "author sequence is not a shared cross-author clock"
    );
    Ok(())
}
#[test]
fn s54_p11a3_valid_new_receipt_bytes_remain_distinct_from_same_signed_source() -> TestResult {
    let f = Fixture::new(0)?;
    for encoding in [1, 2] {
        let mut case = f.case(0, 24, encoding)?;
        let before = f.check(&case)?;
        case.receiver = f.actors.renewal(1, 3)?;
        case.receipt = f.receipt(&case.core, 1, case.id, encoding, &case.receiver)?;
        let after = f.check(&case)?;
        assert!(
            after.source_hash == before.source_hash
                && after.ciphertext_hash == before.ciphertext_hash
                && after.receipt_hash != before.receipt_hash
                && after.receiver_auth != before.receiver_auth
                && before.same(&after).is_err(),
            "actual accepted new receiver authority produces different signed receipt bytes"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a3_bounded_malformed_encodings_and_genesis_never_become_text() -> TestResult {
    let f = Fixture::new(0)?;
    let mut case = f.case(0, 24, 1)?;
    case.raw = vec![0; v2::MAX_EVENT + 1];
    assert!(f.check(&case).is_err(), "source cap");
    let mut case = f.case(0, 24, 1)?;
    case.receipt = vec![0; v2::MAX_RECEIPT + 1];
    assert!(f.check(&case).is_err(), "receipt cap");
    let mut case = f.case(0, 24, 1)?;
    case.body = "x".repeat(v2::MAX_TEXT + 1);
    assert!(f.check(&case).is_err(), "projection cap before copying");
    let mut case = f.case(0, 1, 1)?;
    case.raw = f.genesis[0].clone();
    case.id = old::hash(old::EVENT_ID, &case.raw);
    assert!(
        f.check(&case).is_err(),
        "valid signed genesis cannot be assigned a Text request/receipt"
    );
    for mutation in 0..3 {
        let mut case = f.case(0, 24, 1)?;
        if mutation == 0 {
            case.raw.clear();
        } else {
            let mut fields = old::decode(&case.receipt, v2::MAX_RECEIPT)?
                .fields(2)?
                .to_vec();
            let mut body = fields[0].fields(8)?.to_vec();
            body[0] = old::u(if mutation == 1 { 2 } else { 3 });
            fields[0] = old::a(body);
            case.receipt = old::encode(&old::a(fields), v2::MAX_RECEIPT)?;
        }
        assert!(
            f.check(&case).is_err(),
            "malformed declared version never retries another receipt encoding"
        );
    }
    let mut case = f.case(0, 25, 2)?;
    let mut fields = old::decode(&case.raw, v2::MAX_EVENT)?.fields(5)?.to_vec();
    let mut core = fields[1].fields(15)?.to_vec();
    core[11] = old::a([old::b([80; 32]), old::b([90; 32]), old::b([99; 32])]);
    fields[1] = old::a(core);
    fields[4] = old::b(f.actors.devices[0].sign(&old::signing_bytes(
        v2::EVENT_SIGN,
        &old::a(fields[..4].to_vec()),
        v2::MAX_EVENT,
    )?));
    case.raw = old::encode(&old::a(fields), v2::MAX_EVENT)?;
    case.id = old::hash(v2::EVENT_ID, &case.raw);
    assert!(
        f.check(&case).is_err(),
        "actual parser rejects a validly signed source with more than two parents"
    );
    Ok(())
}
