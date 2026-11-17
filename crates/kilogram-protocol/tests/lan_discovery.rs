//! Synthetic signed locators and HPKE. No profile state, network or key files.
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_identity::DeviceIdentity;
use kilogram_protocol::ordered_dialog_v2::{self as wire, a, b, decode, encode, signing_bytes, u};
use std::net::{Ipv4Addr, SocketAddr};
use wire::{
    ParsedRuntimeLocator2, RuntimeLocatorFields2, Value,
    lan_discovery::{self as codec, DATAGRAM_BYTES, ExpectedContext, GROUP, PORT, Scope},
};

#[path = "support/ordered_dialog_v2.rs"]
pub mod fixtures;
use fixtures::{Actors, TestResult};

const NOW: u64 = 1000;
const SIGN: &[u8] = b"kg:od2:lan-discovery-sign:v1\0";
const INFO: &[u8] = b"kg:od2:lan-discovery-hpke:v1\0";
const PEER: Ipv4Addr = Ipv4Addr::new(192, 168, 7, 5);
const SLOTS: [ExpectedContext; 2] = [
    ExpectedContext {
        context: [8; 32],
        consent_ref: [9; 32],
        consent_interval: (900, 1_200),
    },
    ExpectedContext {
        context: [2; 32],
        consent_ref: [3; 32],
        consent_interval: (950, 1_150),
    },
];

fn scope(actors: &Actors) -> Scope<'_> {
    Scope {
        sender: &actors.credentials[0],
        recipient: &actors.credentials[1],
        slots: SLOTS,
        sender_ipv4: PEER,
        now: NOW,
    }
}

fn locator(actors: &Actors, at: usize) -> RuntimeLocatorFields2 {
    RuntimeLocatorFields2 {
        c: SLOTS[at].context,
        cert: actors.credentials[0].cert_ref(),
        peer_cert: actors.credentials[1].cert_ref(),
        endpoint: *actors.devices[0].device_id().as_bytes(),
        ips: vec![SocketAddr::from((PEER, u16::MAX))],
        relay: None,
        revision: 7,
        not_before: 950,
        expires: 1_100,
    }
}

fn sign_locators(
    actors: &Actors,
    fields: [RuntimeLocatorFields2; 2],
) -> TestResult<[ParsedRuntimeLocator2; 2]> {
    let [first, second] = fields;
    Ok([
        ParsedRuntimeLocator2::sign(first, &actors.devices[0])?,
        ParsedRuntimeLocator2::sign(second, &actors.devices[0])?,
    ])
}

fn credential_key() -> DeviceEncryptionIdentity {
    // The existing fixture uses seed 31 + role. Here the recipient is role 1.
    DeviceEncryptionIdentity::from_secret_bytes([32; 32])
}

fn aad(port: u16) -> TestResult<Vec<u8>> {
    Ok(encode(
        &a([
            b(b"kg:od2:lan-discovery-aad:v1\0"),
            u(1),
            b(GROUP.octets()),
            u(u64::from(port)),
        ]),
        64,
    )?)
}

fn body(
    locators: &[ParsedRuntimeLocator2; 2],
    order: [usize; 2],
    issued: u64,
    expires: u64,
) -> Value {
    a([
        u(1),
        u(issued),
        u(expires),
        a(order.map(|i| a([b(SLOTS[i].consent_ref), b(locators[i].raw())]))),
    ])
}

fn signed_bytes(body: &Value, signer: &DeviceIdentity, domain: &[u8]) -> TestResult<Vec<u8>> {
    let signature = signer.sign(&signing_bytes(domain, body, 583)?);
    Ok(encode(&a([body.clone(), b(signature)]), 650)?)
}

fn padding(signed: &[u8]) -> [u8; 704] {
    let mut plaintext = [0_u8; 704];
    plaintext[..2].copy_from_slice(&(signed.len() as u16).to_be_bytes());
    plaintext[2..2 + signed.len()].copy_from_slice(signed);
    plaintext
}

fn envelope(actors: &Actors, plain: &[u8; 704], info: &[u8], port: u16) -> TestResult<Vec<u8>> {
    let enc = actors.credentials[1]
        .certificate()
        .encryption_public_key()
        .seal(plain, info, &aad(port)?)?;
    Ok(encode(
        &a([u(1), b(enc.encapsulated_key), b(enc.ciphertext)]),
        DATAGRAM_BYTES,
    )?)
}

fn forged(
    actors: &Actors,
    body: &Value,
    signer: &DeviceIdentity,
    domain: &[u8],
) -> TestResult<Vec<u8>> {
    envelope(
        actors,
        &padding(&signed_bytes(body, signer, domain)?),
        INFO,
        PORT,
    )
}

#[test]
fn lan_discovery_round_trip_slot_mapping_and_replay() -> TestResult {
    let actors = Actors::new([2; 32])?;
    let loc = sign_locators(&actors, [locator(&actors, 0), locator(&actors, 1)])?;
    let sender = scope(&actors);
    let bytes = codec::seal(
        &sender,
        &actors.devices[0],
        NOW,
        NOW + 60,
        [loc[0].raw(), loc[1].raw()],
    )?;
    assert_eq!(bytes.len(), 759);
    let receiver = Scope {
        slots: [SLOTS[1], SLOTS[0]],
        ..sender
    };
    for attempt in 0..2 {
        let candidate = codec::open(&bytes, &receiver, &credential_key())?;
        assert!(
            candidate.locators[0].raw() == loc[1].raw()
                && candidate.locators[1].raw() == loc[0].raw(),
            "wrong local locator mapping at attempt {attempt}"
        );
        assert_eq!(candidate.socket, SocketAddr::from((PEER, u16::MAX)));
        assert_eq!(candidate.announcement_interval, (NOW, NOW + 60));
    }
    Ok(())
}

#[test]
fn lan_discovery_maximal_integer_wire_budget() -> TestResult {
    let actors = Actors::new([2; 32])?;
    let max = u64::MAX;
    let fields = [0, 1].map(|i| RuntimeLocatorFields2 {
        revision: max,
        not_before: max - 100,
        expires: max,
        ..locator(&actors, i)
    });
    let loc = sign_locators(&actors, fields)?;
    assert_eq!([loc[0].raw().len(), loc[1].raw().len()], [244, 244]);
    let scoped = Scope {
        now: max - 1,
        slots: SLOTS.map(|slot| ExpectedContext {
            consent_interval: (max - 100, max),
            ..slot
        }),
        ..scope(&actors)
    };
    let frame = codec::seal(
        &scoped,
        &actors.devices[0],
        max - 60,
        max,
        [loc[0].raw(), loc[1].raw()],
    )?;
    let outer = decode(&frame, DATAGRAM_BYTES)?;
    let f = outer.fields(3)?;
    assert_eq!(f[1].bytes()?.len(), 32);
    assert_eq!(f[2].bytes()?.len(), 720);
    let plain = credential_key().open(
        &SealedMessage {
            encapsulated_key: f[1].bytes()?.to_vec(),
            ciphertext: f[2].bytes()?.to_vec(),
        },
        INFO,
        &aad(PORT)?,
    )?;
    let canonical_body = body(&loc, [1, 0], max - 60, max);
    assert_eq!(encode(&canonical_body, 583)?.len(), 583);
    let signed = signed_bytes(&canonical_body, &actors.devices[0], SIGN)?;
    assert_eq!(signed.len(), 650);
    assert!(plain == padding(&signed), "canonical padded frame mismatch");
    assert_eq!(
        codec::open(&frame, &scoped, &credential_key())?.locators[0]
            .fields()
            .revision,
        max
    );
    assert!(
        codec::seal(
            &scoped,
            &actors.devices[0],
            max - 61,
            max,
            [loc[0].raw(), loc[1].raw()]
        )
        .is_err()
    );
    assert!(codec::open(&frame, &Scope { now: max, ..scoped }, &credential_key()).is_err());
    Ok(())
}

#[test]
fn lan_discovery_rejects_wrong_key_ciphertext_signature_and_domains() -> TestResult {
    let actors = Actors::new([2; 32])?;
    let ctx = scope(&actors);
    let loc = sign_locators(&actors, [locator(&actors, 0), locator(&actors, 1)])?;
    let b = body(&loc, [1, 0], NOW, NOW + 60);
    let signed = signed_bytes(&b, &actors.devices[0], SIGN)?;
    let pad = padding(&signed);
    let frame = envelope(&actors, &pad, INFO, PORT)?;
    assert!(
        codec::open(
            &frame,
            &ctx,
            &DeviceEncryptionIdentity::from_secret_bytes([79; 32])
        )
        .is_err()
    );
    let mut ciphertext = frame;
    ciphertext[DATAGRAM_BYTES - 1] ^= 1;
    assert!(codec::open(&ciphertext, &ctx, &credential_key()).is_err());
    let mut signature = signed.clone();
    let signature_byte = signature.len() - 1;
    signature[signature_byte] ^= 1;
    let wrong_recipient = actors.credentials[0]
        .certificate()
        .encryption_public_key()
        .seal(&pad, INFO, &aad(PORT)?)?;
    for bad in [
        forged(&actors, &b, &actors.devices[1], SIGN)?,
        forged(&actors, &b, &actors.devices[0], b"wrong signing domain")?,
        envelope(&actors, &pad, b"wrong hpke info", PORT)?,
        envelope(&actors, &pad, INFO, PORT + 1)?,
        envelope(&actors, &padding(&signature), INFO, PORT)?,
        encode(
            &a([
                u(1),
                wire::b(wrong_recipient.encapsulated_key),
                wire::b(wrong_recipient.ciphertext),
            ]),
            DATAGRAM_BYTES,
        )?,
    ] {
        assert!(codec::open(&bad, &ctx, &credential_key()).is_err());
    }
    Ok(())
}

#[test]
fn lan_discovery_rejects_noncanonical_lengths_and_padding() -> TestResult {
    let actors = Actors::new([2; 32])?;
    let ctx = scope(&actors);
    let loc = sign_locators(&actors, [locator(&actors, 0), locator(&actors, 1)])?;
    let signed = signed_bytes(&body(&loc, [1, 0], NOW, NOW + 60), &actors.devices[0], SIGN)?;
    let pad = padding(&signed);
    let valid = envelope(&actors, &pad, INFO, PORT)?;
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(codec::open(&trailing, &ctx, &credential_key()).is_err());
    assert!(codec::open(&valid[..DATAGRAM_BYTES - 1], &ctx, &credential_key()).is_err());
    let mut noncanonical_outer = valid;
    noncanonical_outer.splice(1..2, [0x18, 1]);
    noncanonical_outer.pop();
    assert!(codec::open(&noncanonical_outer, &ctx, &credential_key()).is_err());
    let mut bad_padding = pad;
    bad_padding[703] = 1;
    let mut bad_size = pad;
    bad_size[..2].copy_from_slice(&651_u16.to_be_bytes());
    let mut noncanonical = signed.clone();
    // The first inner uint is the body version. Encode 1 non-minimally.
    assert!(
        noncanonical[..3] == [0x82, 0x84, 1],
        "inner array/version marker"
    );
    noncanonical.splice(2..3, [0x18, 1]);
    let mut extra_cbor = signed;
    extra_cbor.push(0);
    for plain in [
        bad_padding,
        bad_size,
        padding(&noncanonical),
        padding(&extra_cbor),
    ] {
        let invalid = envelope(&actors, &plain, INFO, PORT)?;
        assert!(codec::open(&invalid, &ctx, &credential_key()).is_err());
    }
    Ok(())
}

#[test]
fn lan_discovery_rejects_authenticated_wrong_scope_socket_and_intervals() -> TestResult {
    let actors = Actors::new([2; 32])?;
    let ctx = scope(&actors);
    let base = locator(&actors, 1);
    let variants = [
        RuntimeLocatorFields2 {
            c: [99; 32],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            cert: [99; 32],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            peer_cert: [99; 32],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            endpoint: [99; 32],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            ips: vec![SocketAddr::from((Ipv4Addr::new(192, 168, 7, 6), 4444))],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            ips: vec![SocketAddr::from((PEER, 4444))],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            ips: vec![base.ips[0], SocketAddr::from((PEER, 4444))],
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            relay: Some("https://relay.example.org/".into()),
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            not_before: NOW + 1,
            ..base.clone()
        },
        RuntimeLocatorFields2 {
            expires: NOW + 59,
            ..base.clone()
        },
    ];
    for variant in variants {
        let loc = sign_locators(&actors, [locator(&actors, 0), variant])?;
        let wrong = forged(
            &actors,
            &body(&loc, [1, 0], NOW, NOW + 60),
            &actors.devices[0],
            SIGN,
        )?;
        assert!(codec::open(&wrong, &ctx, &credential_key()).is_err());
    }
    let loc = sign_locators(&actors, [locator(&actors, 0), base])?;
    for (order, issued, expires) in [
        ([0, 1], NOW, NOW + 60),
        ([0, 0], NOW, NOW + 60),
        ([1, 0], NOW + 1, NOW + 61),
        ([1, 0], NOW - 60, NOW),
        ([1, 0], NOW, NOW + 61),
        ([1, 0], NOW, NOW - 1),
    ] {
        let wrong = forged(
            &actors,
            &body(&loc, order, issued, expires),
            &actors.devices[0],
            SIGN,
        )?;
        assert!(codec::open(&wrong, &ctx, &credential_key()).is_err());
    }
    let frame = codec::seal(
        &ctx,
        &actors.devices[0],
        NOW,
        NOW + 60,
        [loc[0].raw(), loc[1].raw()],
    )?;
    for bad_slot in [
        ExpectedContext {
            consent_ref: [42; 32],
            ..SLOTS[0]
        },
        ExpectedContext {
            consent_interval: (NOW + 1, NOW + 60),
            ..SLOTS[0]
        },
        ExpectedContext {
            consent_interval: (NOW - 1, NOW + 59),
            ..SLOTS[0]
        },
    ] {
        let wrong = Scope {
            slots: [bad_slot, SLOTS[1]],
            ..scope(&actors)
        };
        assert!(codec::open(&frame, &wrong, &credential_key()).is_err());
    }
    Ok(())
}
