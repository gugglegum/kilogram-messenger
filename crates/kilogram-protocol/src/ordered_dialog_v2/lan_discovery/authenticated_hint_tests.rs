//! Actual codec/HPKE/signature tests with fresh in-memory public test identities.
use super::*;
use crate::ordered_dialog_v2 as wire;
#[allow(dead_code)]
mod fixtures {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/ordered_dialog_v2.rs"
    ));
}
use fixtures::{Actors, TestResult};

const IP: Ipv4Addr = Ipv4Addr::new(192, 168, 44, 3);
const CONTEXTS: [HintContext; 2] = [
    HintContext {
        context: [8; 32],
        consent_ref: [9; 32],
    },
    HintContext {
        context: [2; 32],
        consent_ref: [3; 32],
    },
];
fn bindings(actors: &Actors, sender: usize) -> HintScope<'_> {
    HintScope {
        sender: &actors.credentials[sender],
        recipient: &actors.credentials[1 - sender],
        slots: CONTEXTS,
        sender_ipv4: IP,
    }
}
fn strict(actors: &Actors, sender: usize, now: u64, calendar: (u64, u64)) -> Scope<'_> {
    Scope {
        sender: &actors.credentials[sender],
        recipient: &actors.credentials[1 - sender],
        slots: CONTEXTS.map(|c| ExpectedContext {
            context: c.context,
            consent_ref: c.consent_ref,
            consent_interval: calendar,
        }),
        sender_ipv4: IP,
        now,
    }
}
fn key(sender: usize) -> DeviceEncryptionIdentity {
    DeviceEncryptionIdentity::from_secret_bytes([31 + (1 - sender) as u8; 32])
}
fn fields(
    actors: &Actors,
    sender: usize,
    slot: usize,
    nb: u64,
    end: u64,
) -> wire::RuntimeLocatorFields2 {
    wire::RuntimeLocatorFields2 {
        c: CONTEXTS[slot].context,
        cert: actors.credentials[sender].cert_ref(),
        peer_cert: actors.credentials[1 - sender].cert_ref(),
        endpoint: *actors.devices[sender].device_id().as_bytes(),
        ips: vec![SocketAddr::from((IP, 42000))],
        relay: None,
        revision: 7,
        not_before: nb,
        expires: end,
    }
}
fn locators(actors: &Actors, sender: usize, nb: u64, end: u64) -> TestResult<[Vec<u8>; 2]> {
    let sign = |slot| -> TestResult<_> {
        Ok(ParsedRuntimeLocator2::sign(
            fields(actors, sender, slot, nb, end),
            &actors.devices[sender],
        )?
        .raw()
        .to_vec())
    };
    Ok([sign(0)?, sign(1)?])
}
fn body(issued: u64, expires: u64, locators: &[Vec<u8>; 2]) -> Value {
    a([
        u(VERSION),
        u(issued),
        u(expires),
        a([1, 0].map(|i| a([b(CONTEXTS[i].consent_ref), b(&locators[i])]))),
    ])
}
fn envelope(
    actors: &Actors,
    recipient: usize,
    body: Value,
    signer: usize,
    padding: u8,
) -> TestResult<Vec<u8>> {
    let signature = actors.devices[signer].sign(&signing_bytes(SIGN, &body, BODY_BYTES)?);
    let signed = encode(&a([body, b(signature)]), SIGNED_BYTES)?;
    let mut plain = [0; PLAIN_BYTES];
    plain[..2].copy_from_slice(&(signed.len() as u16).to_be_bytes());
    plain[2..2 + signed.len()].copy_from_slice(&signed);
    plain[PLAIN_BYTES - 1] = padding;
    let sealed = actors.credentials[recipient]
        .certificate()
        .encryption_public_key()
        .seal(&plain, INFO, &aad()?)?;
    Ok(encode(
        &a([u(VERSION), b(sealed.encapsulated_key), b(sealed.ciphertext)]),
        DATAGRAM_BYTES,
    )?)
}
fn reject(
    raw: &[u8],
    scope: &HintScope<'_>,
    key: &DeviceEncryptionIdentity,
    expected: Rejection,
) -> String {
    let mut reasons = Vec::new();
    let error = open_authenticated_hint_observed(raw, scope, key, |r| reasons.push(r))
        .err()
        .expect("hint rejected");
    assert_eq!(reasons, [expected]);
    let no_observer = open_authenticated_hint(raw, scope, key)
        .err()
        .expect("same hint rejection");
    assert_eq!(
        std::mem::discriminant(&error),
        std::mem::discriminant(&no_observer)
    );
    assert_eq!(error.to_string(), no_observer.to_string());
    error.to_string()
}

#[test]
fn a1hint_both_directions_dates_and_slots_are_lossless() -> TestResult {
    let actors = Actors::new([61; 32])?;
    let mut cases = 0;
    for offset in [
        0_i64, 1, -1, 3, -3, 5, -5, 10, -10, 60, -60, 3600, -3600, 86400, -86400,
    ] {
        let issued = 100_000u64
            .checked_add_signed(offset)
            .expect("synthetic range");
        for sender in 0..2 {
            let locators = locators(&actors, sender, issued - 1, issued + 100)?;
            // Sender uses its own calendar when sealing. HintScope below has no
            // receiver wall/calendar; it cannot substitute a fake local now.
            let raw = seal(
                &strict(&actors, sender, issued, (issued - 10, issued + 200)),
                &actors.devices[sender],
                issued,
                issued + 45,
                locators.each_ref().map(Vec::as_slice),
            )?;
            for reversed in [false, true] {
                let mut scope = bindings(&actors, sender);
                if reversed {
                    scope.slots.swap(0, 1);
                }
                let hint = open_authenticated_hint_observed(&raw, &scope, &key(sender), |_| {
                    panic!("success emitted rejection")
                })?;
                assert_eq!(hint.datagram(), &raw);
                assert_eq!(hint.signed_announcement_interval(), (issued, issued + 45));
                assert_eq!(hint.socket(), SocketAddr::from((IP, 42000)));
                for slot in 0..2 {
                    let original = if reversed { 1 - slot } else { slot };
                    assert_eq!(hint.locators()[slot].raw(), locators[original]);
                    assert_eq!(hint.locators()[slot].fields().c, scope.slots[slot].context);
                    assert_eq!(hint.locators()[slot].fields().not_before, issued - 1);
                    assert_eq!(hint.locators()[slot].fields().expires, issued + 100);
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 60);
    println!("A1HINT actual_crypto_cases=60 receiver_clock_input=absent bytes_preserved=true");
    Ok(())
}

#[test]
fn a1hint_legacy_calendar_boundaries_remain_strict() -> TestResult {
    let actors = Actors::new([62; 32])?;
    let locators = locators(&actors, 0, 999, 1200)?;
    let raw = seal(
        &strict(&actors, 0, 1000, (900, 1200)),
        &actors.devices[0],
        1000,
        1045,
        locators.each_ref().map(Vec::as_slice),
    )?;
    let hint = open_authenticated_hint(&raw, &bindings(&actors, 0), &key(0))?;
    for now in [1000, 1044] {
        let old = open(&raw, &strict(&actors, 0, now, (900, 1200)), &key(0))?;
        assert_eq!(
            old.announcement_interval,
            hint.signed_announcement_interval()
        );
        for slot in 0..2 {
            assert_eq!(old.locators[slot].raw(), hint.locators()[slot].raw());
        }
    }
    for (now, reason) in [
        (999, Rejection::AnnouncementFuture),
        (1045, Rejection::AnnouncementExpired),
        (0, Rejection::AnnouncementFuture),
        (u64::MAX, Rejection::AnnouncementExpired),
    ] {
        let mut rejected = Vec::new();
        let error = open_observed(&raw, &strict(&actors, 0, now, (900, 1200)), &key(0), |r| {
            rejected.push(r)
        })
        .err()
        .expect("legacy time refusal");
        assert_eq!(rejected, [reason]);
        assert_eq!(error.to_string(), "OD2 discovery announcement interval");
        assert!(open_authenticated_hint(&raw, &bindings(&actors, 0), &key(0)).is_ok());
    }
    for calendar in [(0, 1), (1001, 1200), (900, 1044)] {
        let error = open(&raw, &strict(&actors, 0, 1000, calendar), &key(0))
            .err()
            .expect("legacy calendar containment");
        assert_eq!(error.to_string(), "OD2 discovery consent scope or interval");
        assert!(open_authenticated_hint(&raw, &bindings(&actors, 0), &key(0)).is_ok());
    }
    println!("A1HINT legacy_wall_and_calendar_refusals_preserved=true hint_not_authorization=true");
    Ok(())
}

#[test]
fn a1hint_envelope_recipient_identity_and_signature() -> TestResult {
    let actors = Actors::new([63; 32])?;
    let scope = bindings(&actors, 0);
    let locators = locators(&actors, 0, 999, 1200)?;
    let raw = envelope(&actors, 1, body(1000, 1045, &locators), 0, 0)?;
    reject(&raw[..758], &scope, &key(0), Rejection::FrameSize);
    reject(
        &raw,
        &scope,
        &DeviceEncryptionIdentity::from_secret_bytes([44; 32]),
        Rejection::RecipientKey,
    );
    let wrong_recipient = envelope(&actors, 0, body(1000, 1045, &locators), 0, 0)?;
    let error = reject(&wrong_recipient, &scope, &key(0), Rejection::Decryption);
    assert_eq!(
        open(
            &wrong_recipient,
            &strict(&actors, 0, 1000, (900, 1200)),
            &key(0)
        )
        .err()
        .expect("strict decrypt")
        .to_string(),
        error
    );
    let mut altered = raw.clone();
    altered[758] ^= 1;
    reject(&altered, &scope, &key(0), Rejection::Decryption);
    let wrong_signer = envelope(&actors, 1, body(1000, 1045, &locators), 1, 0)?;
    reject(&wrong_signer, &scope, &key(0), Rejection::Signature);
    let mut same_peer = bindings(&actors, 0);
    same_peer.sender = same_peer.recipient;
    reject(&raw, &same_peer, &key(0), Rejection::PeerScope);
    let mut same_context = bindings(&actors, 0);
    same_context.slots[1] = same_context.slots[0];
    reject(&raw, &same_context, &key(0), Rejection::Context);
    let mut outer = decode(&raw, DATAGRAM_BYTES)?.fields(3)?.to_vec();
    outer[0] = u(2);
    reject(
        &encode(&a(outer), DATAGRAM_BYTES)?,
        &scope,
        &key(0),
        Rejection::Envelope,
    );
    let padding = envelope(&actors, 1, body(1000, 1045, &locators), 0, 1)?;
    reject(&padding, &scope, &key(0), Rejection::SignedBody);
    println!("A1HINT envelope_identity_failures=9 observations_single=true");
    Ok(())
}

#[test]
fn a1hint_valid_signature_still_rejects_later_body_locator_and_route() -> TestResult {
    let actors = Actors::new([64; 32])?;
    let scope = bindings(&actors, 0);
    let locators = locators(&actors, 0, 999, 1200)?;
    let raw = envelope(&actors, 1, body(1000, 1045, &locators), 0, 0)?;
    let mut wrong = bindings(&actors, 0);
    wrong.slots[0].consent_ref[0] ^= 1;
    reject(&raw, &wrong, &key(0), Rejection::Consent);
    let mut wrong = bindings(&actors, 0);
    wrong.slots[0].context = [7; 32];
    reject(&raw, &wrong, &key(0), Rejection::LocatorVerification);
    let mut wrong = bindings(&actors, 0);
    wrong.sender_ipv4 = Ipv4Addr::new(192, 168, 44, 4);
    reject(&raw, &wrong, &key(0), Rejection::Route);
    let mut bad = locators.clone();
    bad[0] = vec![0];
    reject(
        &envelope(&actors, 1, body(1000, 1045, &bad), 0, 0)?,
        &scope,
        &key(0),
        Rejection::LocatorParse,
    );
    let mut bad = locators.clone();
    bad[0] = vec![0; LOCATOR_BYTES + 1];
    reject(
        &envelope(&actors, 1, body(1000, 1045, &bad), 0, 0)?,
        &scope,
        &key(0),
        Rejection::LocatorParse,
    );
    let mut bad = locators.clone();
    bad[0] = ParsedRuntimeLocator2::sign(fields(&actors, 0, 0, 999, 1200), &actors.devices[1])?
        .raw()
        .to_vec();
    reject(
        &envelope(&actors, 1, body(1000, 1045, &bad), 0, 0)?,
        &scope,
        &key(0),
        Rejection::LocatorVerification,
    );
    for (nb, end) in [(1001, 1200), (999, 1044)] {
        let bad = locators_for_interval(&actors, nb, end)?;
        reject(
            &envelope(&actors, 1, body(1000, 1045, &bad), 0, 0)?,
            &scope,
            &key(0),
            Rejection::LocatorInterval,
        );
    }
    let mut bad = locators.clone();
    let mut different = fields(&actors, 0, 0, 999, 1200);
    different.ips = vec![SocketAddr::from((IP, 42001))];
    bad[0] = ParsedRuntimeLocator2::sign(different, &actors.devices[0])?
        .raw()
        .to_vec();
    reject(
        &envelope(&actors, 1, body(1000, 1045, &bad), 0, 0)?,
        &scope,
        &key(0),
        Rejection::LocatorPair,
    );
    let malformed = a([u(VERSION), u(1000), u(1045)]);
    reject(
        &envelope(&actors, 1, malformed, 0, 0)?,
        &scope,
        &key(0),
        Rejection::SignedBody,
    );
    println!("A1HINT valid_outer_signature_later_failures=10");
    Ok(())
}
fn locators_for_interval(actors: &Actors, nb: u64, end: u64) -> TestResult<[Vec<u8>; 2]> {
    locators(actors, 0, nb, end)
}

#[test]
fn a1hint_signed_intervals_boundaries_and_unsigned_overflow() -> TestResult {
    let actors = Actors::new([65; 32])?;
    let scope = bindings(&actors, 0);
    for (issued, expires) in [
        (1000, 1000),
        (1000, 999),
        (1000, 1061),
        (u64::MAX, 0),
        (0, u64::MAX),
    ] {
        let locators = locators(&actors, 0, 0, u64::MAX)?;
        let error = reject(
            &envelope(&actors, 1, body(issued, expires, &locators), 0, 0)?,
            &scope,
            &key(0),
            Rejection::AnnouncementDuration,
        );
        assert_eq!(error, "OD2 discovery announcement interval");
    }
    for (issued, expires) in [(0, 1), (1000, 1060), (u64::MAX - 60, u64::MAX)] {
        let locators = locators(&actors, 0, issued.saturating_sub(1), expires)?;
        let raw = envelope(&actors, 1, body(issued, expires, &locators), 0, 0)?;
        let hint = open_authenticated_hint(&raw, &scope, &key(0))?;
        assert_eq!(hint.signed_announcement_interval(), (issued, expires));
        assert_eq!(hint.datagram().as_slice(), raw);
    }
    println!("A1HINT interval_failures=5 interval_edges_accepted=3 signed_dates_preserved=true");
    Ok(())
}
