//! Fresh in-memory synthetic credentials only. No files, clocks or network.
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
const SLOTS: [ExpectedContext; 2] = [
    ExpectedContext {
        context: [8; 32],
        consent_ref: [9; 32],
        consent_interval: (900, 1200),
    },
    ExpectedContext {
        context: [2; 32],
        consent_ref: [3; 32],
        consent_interval: (900, 1200),
    },
];
fn scope(actors: &Actors, sender: usize, now: u64) -> Scope<'_> {
    Scope {
        sender: &actors.credentials[sender],
        recipient: &actors.credentials[1 - sender],
        slots: SLOTS,
        sender_ipv4: IP,
        now,
    }
}
fn locators(actors: &Actors, sender: usize) -> TestResult<[ParsedRuntimeLocator2; 2]> {
    let sign = |slot: ExpectedContext| -> TestResult<_> {
        Ok(ParsedRuntimeLocator2::sign(
            wire::RuntimeLocatorFields2 {
                c: slot.context,
                cert: actors.credentials[sender].cert_ref(),
                peer_cert: actors.credentials[1 - sender].cert_ref(),
                endpoint: *actors.devices[sender].device_id().as_bytes(),
                ips: vec![SocketAddr::from((IP, 42000))],
                relay: None,
                revision: 7,
                not_before: 900,
                expires: 1200,
            },
            &actors.devices[sender],
        )?)
    };
    Ok([sign(SLOTS[0])?, sign(SLOTS[1])?])
}
fn check_rejected(raw: &[u8], scope: &Scope<'_>, key: &DeviceEncryptionIdentity, want: Rejection) {
    let plain = open(raw, scope, key).err().expect("must reject");
    let mut reasons = Vec::new();
    let observed = open_observed(raw, scope, key, |reason| reasons.push(reason))
        .err()
        .expect("same rejection");
    assert_eq!(reasons, [want]);
    assert_eq!(
        std::mem::discriminant(&plain),
        std::mem::discriminant(&observed)
    );
    assert_eq!(plain.to_string(), observed.to_string());
}
fn forged(actors: &Actors, body: Value, signer: usize) -> TestResult<Vec<u8>> {
    let signature = actors.devices[signer].sign(&signing_bytes(SIGN, &body, BODY_BYTES)?);
    let signed = encode(&a([body, b(signature)]), SIGNED_BYTES)?;
    let mut plain = [0; PLAIN_BYTES];
    plain[..2].copy_from_slice(&(signed.len() as u16).to_be_bytes());
    plain[2..2 + signed.len()].copy_from_slice(&signed);
    let sealed = actors.credentials[1]
        .certificate()
        .encryption_public_key()
        .seal(&plain, INFO, &aad()?)?;
    Ok(encode(
        &a([u(1), b(sealed.encapsulated_key), b(sealed.ciphertext)]),
        DATAGRAM_BYTES,
    )?)
}

#[test]
fn t13reject_both_directions_slot_order_and_independent_clocks() -> TestResult {
    let actors = Actors::new([47; 32])?;
    let mut cases = 0;
    // Roles stay identical while the faster host changes. Failure follows the
    // sender being ahead, not proposer/cosigner identity or local slot order.
    for clocks in [[1000, 1000], [1000, 1003], [1003, 1000]] {
        for sender in 0..2 {
            let source = scope(&actors, sender, clocks[sender]);
            let locators = locators(&actors, sender)?;
            let raw = seal(
                &source,
                &actors.devices[sender],
                clocks[sender],
                clocks[sender] + 45,
                locators.each_ref().map(|l| l.raw()),
            )?;
            let key = DeviceEncryptionIdentity::from_secret_bytes([31 + (1 - sender) as u8; 32]);
            for swapped in [false, true] {
                let mut receiver = scope(&actors, sender, clocks[1 - sender]);
                if swapped {
                    receiver.slots.swap(0, 1);
                }
                if clocks[sender] > clocks[1 - sender] {
                    check_rejected(&raw, &receiver, &key, Rejection::AnnouncementFuture);
                } else {
                    let candidate = open_observed(&raw, &receiver, &key, |_| {
                        panic!("success emitted rejection")
                    })?;
                    let legacy = open(&raw, &receiver, &key)?;
                    for i in 0..2 {
                        assert_eq!(candidate.locators[i].raw(), legacy.locators[i].raw());
                        assert_eq!(candidate.locators[i].fields().c, receiver.slots[i].context);
                    }
                    assert_eq!(candidate.socket, legacy.socket);
                }
                println!(
                    "T13REJECT_CLOCK sender={sender} sender_now={} receiver_now={} swapped={swapped} accepted={}",
                    clocks[sender],
                    clocks[1 - sender],
                    clocks[sender] <= clocks[1 - sender]
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 12);
    Ok(())
}

#[test]
fn t13reject_fixed_failure_sites_preserve_original_errors() -> TestResult {
    let actors = Actors::new([48; 32])?;
    let source = scope(&actors, 0, 1000);
    let key = DeviceEncryptionIdentity::from_secret_bytes([32; 32]);
    let locators = locators(&actors, 0)?;
    let raw = seal(
        &source,
        &actors.devices[0],
        1000,
        1045,
        locators.each_ref().map(|l| l.raw()),
    )?;
    check_rejected(&raw[..758], &source, &key, Rejection::FrameSize);
    check_rejected(
        &raw,
        &source,
        &DeviceEncryptionIdentity::from_secret_bytes([44; 32]),
        Rejection::RecipientKey,
    );
    let mut changed = raw;
    changed[758] ^= 1;
    check_rejected(&changed, &source, &key, Rejection::Decryption);
    let mut wrong = scope(&actors, 0, 1000);
    wrong.slots[1] = wrong.slots[0];
    check_rejected(&raw, &wrong, &key, Rejection::Context);
    let mut wrong = scope(&actors, 0, 1000);
    wrong.slots[0].consent_ref[0] ^= 1;
    check_rejected(&raw, &wrong, &key, Rejection::Consent);
    let mut wrong = scope(&actors, 0, 1000);
    wrong.slots[0].context = [7; 32];
    check_rejected(&raw, &wrong, &key, Rejection::LocatorVerification);
    let mut wrong = scope(&actors, 0, 1000);
    wrong.sender_ipv4 = Ipv4Addr::new(192, 168, 44, 4);
    check_rejected(&raw, &wrong, &key, Rejection::Route);
    for (now, reason) in [
        (999, Rejection::AnnouncementFuture),
        (1045, Rejection::AnnouncementExpired),
    ] {
        check_rejected(&raw, &scope(&actors, 0, now), &key, reason);
    }
    for now in [1000, 1044] {
        assert!(open(&raw, &scope(&actors, 0, now), &key).is_ok());
    }
    let body = |expires, bytes: [&[u8]; 2]| {
        a([
            u(1),
            u(1000),
            u(expires),
            a([1, 0].map(|i| a([b(SLOTS[i].consent_ref), b(bytes[i])]))),
        ])
    };
    let bytes = locators.each_ref().map(|l| l.raw());
    let bad_signature = forged(&actors, body(1045, bytes), 1)?;
    check_rejected(&bad_signature, &source, &key, Rejection::Signature);
    let too_long = forged(&actors, body(1061, bytes), 0)?;
    check_rejected(&too_long, &source, &key, Rejection::AnnouncementDuration);
    let malformed_locator = forged(&actors, body(1045, [&[0], bytes[1]]), 0)?;
    check_rejected(&malformed_locator, &source, &key, Rejection::LocatorParse);
    let mut fields = locators[0].fields().clone();
    fields.expires = 1044;
    let short = ParsedRuntimeLocator2::sign(fields, &actors.devices[0])?;
    let short = forged(&actors, body(1045, [short.raw(), bytes[1]]), 0)?;
    check_rejected(&short, &source, &key, Rejection::LocatorInterval);
    println!("T13REJECT_SITES failures=13 exact_error_equivalence=true decrypt_once_per_call=true");
    Ok(())
}
