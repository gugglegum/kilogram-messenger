//! Real seal/restore over ephemeral crypto; synthetic scope/guard, never a real store/origin export.
use super::*;
#[path = "migration_s54_native_fixture.rs"]
mod fixture;
use fixture::Fixture;
fn replace(
    value: &old::Value,
    index: usize,
    replacement: old::Value,
) -> Result<Zeroizing<old::Value>> {
    let mut fields = Zeroizing::new(value.fields(7)?.to_vec());
    fields[index] = replacement;
    Ok(Zeroizing::new(old::Value::A(std::mem::take(&mut *fields))))
}
fn encoded(value: &old::Value, index: usize, replacement: old::Value) -> Result<Vec<u8>> {
    let mut fields = value.fields(5)?.to_vec();
    fields[index] = replacement;
    Ok(old::encode(&old::Value::A(fields), state::MAX_NATIVE)?)
}
#[test]
fn s54_p11a6_real_restore_both_roles_asymmetric_guard_and_zero_reader_calls() -> Result<()> {
    let f = Fixture::new()?; // All Native setup precedes this measured boundary.
    let before = kilogram_ratchet::ordered::native_call_counts();
    for role in 0..2 {
        let mut head = f.heads[role].clone();
        let raw = f.seal(role, &mut head, &f.payloads[role])?;
        let actual = f.check(role, &head, &raw)?;
        ensure!(
            actual.role == role as u8
                && actual.next == [6, 4]
                && actual.tails == f.metas[role].tails,
            "role-relative actual restored checkpoint"
        );
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "restore never constructs or calls packets"
    );
    Ok(())
}
#[test]
fn s54_p11a6_real_open_requires_b_consumed_original_tuple_and_a_none() -> Result<()> {
    let f = Fixture::new()?;
    let before = kilogram_ratchet::ordered::native_call_counts();
    for case in 0..5 {
        let tuple = if case == 0 {
            old::opt(None)
        } else {
            old::a([
                old::b(if case == 1 { [99; 32] } else { f.scope.offer }),
                old::b(if case == 2 { [99; 32] } else { f.scope.grant }),
                old::b(if case == 3 { [99; 32] } else { f.scope.otk }),
                old::u(if case == 4 { 0 } else { 1 }),
            ])
        };
        let payload = replace(&f.payloads[1], 6, tuple)?;
        let mut head = f.heads[1].clone();
        let raw = f.seal(1, &mut head, &payload)?;
        ensure!(
            f.check(1, &head, &raw).is_err(),
            "B missing/foreign/unconsumed tuple"
        );
    }
    let payload = replace(
        &f.payloads[0],
        6,
        old::a([
            old::b(f.scope.offer),
            old::b(f.scope.grant),
            old::b(f.scope.otk),
            old::u(1),
        ]),
    )?;
    let mut head = f.heads[0].clone();
    let raw = f.seal(0, &mut head, &payload)?;
    ensure!(
        f.check(0, &head, &raw).is_err(),
        "A cannot retain a reservation"
    );
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "zero error counters"
    );
    Ok(())
}
#[test]
fn s54_p11a6_real_restored_account_sid_and_all_public_keys_must_match() -> Result<()> {
    let f = Fixture::new()?;
    let foreign = Fixture::new()?;
    let before = kilogram_ratchet::ordered::native_call_counts();
    let pickle = Zeroizing::new([13; 32]);
    for (field, raw) in [
        (3, f.accounts[1].snapshot(&pickle)),
        (4, foreign.sessions[0].snapshot(&pickle)),
    ] {
        let payload = replace(&f.payloads[0], field, old::b(raw))?;
        let mut head = f.heads[0].clone();
        let raw = f.seal(0, &mut head, &payload)?;
        ensure!(
            f.check(0, &head, &raw).is_err(),
            "restored original account/transcript or SID mismatch"
        );
    }
    let live = json!({"sid":f.scope.sid,"keys":f.keys.iter().map(|key|state::hex(key)).collect::<Vec<_>>()});
    let mut head = f.heads[0].clone();
    let raw = f.seal(0, &mut head, &f.payloads[0])?;
    for case in 0..4 {
        let mut bad = live.clone();
        if case == 3 {
            bad["sid"] = "foreign".into();
        } else {
            bad["keys"][case] = state::hex(&[99; 32]).into();
        }
        let binding = Binding {
            material: &f.material,
            source: f.source,
            head: &head,
            q: f.q,
            scope: &f.scope,
            meta: &f.metas[0],
            raw: &raw,
        };
        ensure!(
            open(&binding, |sid, keys| public_identity(&bad, sid, keys)).is_err(),
            "actual restored identity including base key must equal retained live keys"
        );
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "zero mismatch counters"
    );
    Ok(())
}
#[test]
fn s54_p11a6_real_caps_pointer_full_aad_and_ciphertext_authentication_fail_closed() -> Result<()> {
    let mut f = Fixture::new()?;
    let mut head = f.heads[0].clone();
    let raw = f.seal(0, &mut head, &f.payloads[0])?;
    let before = kilogram_ratchet::ordered::native_call_counts();
    let mut wrong = head.clone();
    wrong.blob = Some([99; 32]);
    ensure!(f.check(0, &wrong, &raw).is_err(), "actual Head pointer");
    ensure!(
        f.check(0, &head, &vec![0; state::MAX_NATIVE + 1]).is_err(),
        "actual input cap"
    );
    let envelope = old::decode(&raw, state::MAX_NATIVE)?;
    let mut generation = head.clone();
    generation.native_generation += 1;
    let generation_raw = encoded(&envelope, 2, old::u(generation.native_generation))?;
    generation.blob = Some(old::hash(store::MIXED_NATIVE_ID, &generation_raw));
    ensure!(
        f.check(0, &generation, &generation_raw).is_err(),
        "matching envelope/Head generation still requires original authenticated generation AAD"
    );
    for (field, replacement) in [
        (0, old::u(1)),
        (1, old::u(2)),
        (2, old::u(head.native_generation + 1)),
        (3, old::b([99; 32])),
    ] {
        let changed = encoded(&envelope, field, replacement)?;
        let mut h = head.clone();
        h.blob = Some(old::hash(store::MIXED_NATIVE_ID, &changed));
        ensure!(
            f.check(0, &h, &changed).is_err(),
            "envelope discriminator/generation or authenticated key failure"
        );
    }
    let mut cipher = envelope.fields(5)?[4].bytes()?.to_vec();
    cipher[0] ^= 1;
    let changed = encoded(&envelope, 4, old::b(cipher))?;
    let mut h = head.clone();
    h.blob = Some(old::hash(store::MIXED_NATIVE_ID, &changed));
    ensure!(
        f.check(0, &h, &changed).is_err(),
        "altered ciphertext with updated pointer reaches authentication failure"
    );
    for field in 0..10 {
        let mut h = head.clone();
        match field {
            0 => h.c = [99; 32],
            1 => h.local_cert = [99; 32],
            2 => {
                h.role = 1;
                h.local_cert = f.scope.certs[1];
            }
            3 => h.phase = 2,
            4 => h.offer = Some([99; 32]),
            5 => h.grant = Some([99; 32]),
            6 => h.d = Some([99; 32]),
            7 => h.e = Some([99; 32]),
            8 => h.sid = Some("Z".repeat(43)),
            _ => h.native_generation += 1,
        }
        ensure!(
            f.check(0, &h, &raw).is_err(),
            "full current Head/AAD/original scope field"
        );
    }
    for field in 0..5 {
        let (source, q) = (f.source, f.q);
        let (nonce, incarnation) = (f.material.nonce, f.material.incarnation);
        let encryption = *f.material.encryption;
        match field {
            0 => f.source = [99; 32],
            1 => f.q = [99; 32],
            2 => f.material.nonce = [99; 32],
            3 => f.material.incarnation = [99; 32],
            _ => *f.material.encryption = [99; 32],
        }
        ensure!(
            f.check(0, &head, &raw).is_err(),
            "actual source/Q/root identity/device seal AAD"
        );
        f.source = source;
        f.q = q;
        f.material.nonce = nonce;
        f.material.incarnation = incarnation;
        *f.material.encryption = encryption;
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "zero authentication-error counters"
    );
    Ok(())
}
#[test]
fn s54_p11a6_real_private_discriminators_pickles_guard_role_and_lanes_are_checked() -> Result<()> {
    let f = Fixture::new()?;
    let before = kilogram_ratchet::ordered::native_call_counts();
    for (field, value) in [
        (0, old::u(1)),
        (1, old::u(2)),
        (2, old::b([0; 31])),
        (3, old::b(b"invalid-account")),
        (4, old::b(b"invalid-session")),
        (5, old::b(b"invalid-guard")),
        (
            5,
            old::b(vec![0; kilogram_ratchet::ordered_v2::MAX_GUARD + 1]),
        ),
        (5, old::b(fixture::guard(1)?)),
    ] {
        let payload = replace(&f.payloads[0], field, value)?;
        let mut head = f.heads[0].clone();
        let raw = f.seal(0, &mut head, &payload)?;
        ensure!(
            f.check(0, &head, &raw).is_err(),
            "authenticated malformed private/pickle/guard failure"
        );
    }
    let mut head = f.heads[1].clone();
    let raw = f.seal(1, &mut head, &f.payloads[1])?;
    for field in 0..2 {
        let mut meta = f.metas[1].clone();
        if field == 0 {
            meta.next.swap(0, 1);
        } else {
            meta.tails.swap(0, 1);
        }
        let binding = Binding {
            material: &f.material,
            source: f.source,
            head: &head,
            q: f.q,
            scope: &f.scope,
            meta: &meta,
            raw: &raw,
        };
        ensure!(
            open(&binding, |_, _| Ok(())).is_err(),
            "asymmetric B lane reversal cannot hide in symmetric terminal counts"
        );
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "zero malformed-restore error counters"
    );
    Ok(())
}
#[test]
fn s54_p11a6_cut_content_commits_stored_pickles_guard_and_reservation_not_outer_seal() -> Result<()>
{
    let f = Fixture::new()?;
    let before = kilogram_ratchet::ordered::native_call_counts();
    let mut h1 = f.heads[0].clone();
    let r1 = f.seal(0, &mut h1, &f.payloads[0])?;
    let mut h2 = h1.clone();
    h2.native_generation += 1;
    let r2 = f.seal(0, &mut h2, &f.payloads[0])?;
    let first = f.check(0, &h1, &r1)?;
    let second = f.check(0, &h2, &r2)?;
    ensure!(
        first.seal != second.seal && first.native_generation != second.native_generation,
        "real random outer seals/generations differ"
    );
    first.same_content(&second)?;
    // Explicitly change the stored pickle encryption key/bytes while keeping
    // the same restored account/session and all their public identities.
    let pickle = Zeroizing::new([14; 32]);
    let payload = replace(&f.payloads[0], 2, old::b(&pickle[..]))?;
    let payload = replace(&payload, 3, old::b(f.accounts[0].snapshot(&pickle)))?;
    let payload = replace(&payload, 4, old::b(f.sessions[0].snapshot(&pickle)))?;
    let raw = f.seal(0, &mut h2, &payload)?;
    let changed = f.check(0, &h2, &raw)?;
    ensure!(
        first.same_content(&changed).is_err() && first.public_identity == changed.public_identity,
        "same SID/keys cannot hide changed stored account/session pickle bytes"
    );
    let mut facts = kilogram_ratchet::ordered_v2::L1CheckpointFacts {
        role: 0,
        own: Guard2::restore(&fixture::guard(0)?, 0)?.own().clone(),
        peer: Guard2::restore(&fixture::guard(0)?, 0)?.peer().clone(),
        rotate_due: true,
        received_new_first: Some([45; 32]),
        used_own_first: Some([44; 32]),
    };
    facts.own.sequence = Some(99);
    let payload = replace(
        &f.payloads[0],
        5,
        old::b(kilogram_ratchet::ordered_v2::compact_l1_checkpoint(facts)?),
    )?;
    let raw = f.seal(0, &mut h2, &payload)?;
    let changed = f.check(0, &h2, &raw)?;
    ensure!(
        first.same_content(&changed).is_err() && first.guard != changed.guard,
        "full actual guard content comparison"
    );
    // Independent component comparisons are part of same_content even if other
    // fingerprints were accidentally retained by a later adapter change.
    for field in 0..4 {
        let mut changed = first.clone();
        match field {
            0 => changed.content = [99; 32],
            1 => changed.session = [99; 32],
            2 => changed.guard = [99; 32],
            _ => changed.reservation = [99; 32],
        };
        ensure!(
            first.same_content(&changed).is_err(),
            "complete private commitment equality"
        );
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "snapshot fixture edits and reader restore do not call constructors or packets"
    );
    Ok(())
}
#[test]
fn s54_p11a6_role_relative_guard_predicate_handles_asymmetric_frontiers() -> Result<()> {
    for role in 0..2 {
        let guard = Guard2::restore(&fixture::guard(role)?, role)?;
        let meta = store::L1Metadata {
            policy_generation: 1,
            wall_high_water: 1,
            next: [6, 4],
            tails: [Some([40; 32]), Some([41; 32])],
            next_queue: 1,
            request_index_after: None,
            request_index_ready: true,
            ack_next: 1,
            peer_source_next: 1,
            peer_source_last: None,
        };
        frontiers(&guard, role, &meta)?;
        ensure!(frontiers(&guard, 1 - role, &meta).is_err(), "wrong role");
        let mut wrong = meta.clone();
        wrong.next.swap(0, 1);
        ensure!(
            frontiers(&guard, role, &wrong).is_err(),
            "no global lane0 assumption"
        );
    }
    Ok(())
}
