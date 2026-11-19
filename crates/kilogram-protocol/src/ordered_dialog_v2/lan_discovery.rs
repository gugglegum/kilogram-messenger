//! OD2 LAN discovery v1. Verification yields a signed locator candidate.
//! Durable revision high-water, current policy and admission belong to the owner.

use super::{
    Error, Id, ParsedRuntimeLocator2, Result, RootVerifiedCredential, Value, a, b, decode, encode,
    require, signing_bytes, u,
};
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_identity::DeviceIdentity;
use std::cell::Cell;
use std::net::{Ipv4Addr, SocketAddr};
use zeroize::Zeroizing;

pub const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 75, 72);
pub const PORT: u16 = 50_372;
pub const DATAGRAM_BYTES: usize = 759;

/// Fixed failure sites only; never carries payload, plaintext or error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    Unknown,
    FrameSize,
    RecipientKey,
    Context,
    PeerScope,
    Envelope,
    Decryption,
    SignedBody,
    Signature,
    AnnouncementFuture,
    AnnouncementExpired,
    AnnouncementDuration,
    Consent,
    LocatorParse,
    LocatorVerification,
    LocatorInterval,
    Route,
    LocatorPair,
}

const VERSION: u64 = 1;
const LOCATOR_BYTES: usize = 244;
const BODY_BYTES: usize = 583;
const SIGNED_BYTES: usize = 650;
const PLAIN_BYTES: usize = 704;
const CIPHERTEXT_BYTES: usize = 720;
const MAX_ANNOUNCEMENT_SECONDS: u64 = 60;
const SIGN: &[u8] = b"kg:od2:lan-discovery-sign:v1\0";
const INFO: &[u8] = b"kg:od2:lan-discovery-hpke:v1\0";
const AAD: &[u8] = b"kg:od2:lan-discovery-aad:v1\0";

/// The local view of one installed context and its expected consent.
#[derive(Clone, Copy)]
pub struct ExpectedContext {
    pub context: Id,
    pub consent_ref: Id,
    pub consent_interval: (u64, u64),
}

/// The caller supplies the installed credential pair and its current clock.
/// For an incoming announcement, sender_ipv4 is the configured peer address.
pub struct Scope<'a> {
    pub sender: &'a RootVerifiedCredential,
    pub recipient: &'a RootVerifiedCredential,
    pub slots: [ExpectedContext; 2],
    pub sender_ipv4: Ipv4Addr,
    pub now: u64,
}

pub struct Candidate {
    /// Locators are in the caller's local slot order.
    pub locators: [ParsedRuntimeLocator2; 2],
    pub socket: SocketAddr,
    /// The import window. OD2 uses the locator/consent/lease deadlines after import.
    pub announcement_interval: (u64, u64),
}

fn aad() -> Result<Vec<u8>> {
    Ok(encode(
        &a([b(AAD), u(VERSION), b(GROUP.octets()), u(u64::from(PORT))]),
        64,
    )?)
}

fn wire_order(scope: &Scope<'_>, rejection: &Cell<Rejection>) -> Result<[usize; 2]> {
    rejection.set(Rejection::Context);
    require(
        scope.slots[0].context != scope.slots[1].context,
        "discovery distinct contexts",
    )?;
    let sender = scope.sender.certificate();
    let recipient = scope.recipient.certificate();
    rejection.set(Rejection::PeerScope);
    require(
        sender.account_id() != recipient.account_id()
            && sender.device_id() != recipient.device_id()
            && scope.sender_ipv4.is_private(),
        "discovery peer scope",
    )?;
    Ok(if scope.slots[0].context < scope.slots[1].context {
        [0, 1]
    } else {
        [1, 0]
    })
}

fn validate_body(
    body: &Value,
    scope: &Scope<'_>,
    order: [usize; 2],
    rejection: &Cell<Rejection>,
) -> Result<Candidate> {
    rejection.set(Rejection::SignedBody);
    let fields = body.fields(4)?;
    require(fields[0].uint()? == VERSION, "discovery body version")?;
    let issued = fields[1].uint()?;
    let expires = fields[2].uint()?;
    rejection.set(Rejection::AnnouncementFuture);
    require(
        issued <= scope.now
            && {
                rejection.set(Rejection::AnnouncementExpired);
                scope.now < expires
            }
            && {
                rejection.set(Rejection::AnnouncementDuration);
                expires
                    .checked_sub(issued)
                    .is_some_and(|length| (1..=MAX_ANNOUNCEMENT_SECONDS).contains(&length))
            },
        "discovery announcement interval",
    )?;
    rejection.set(Rejection::SignedBody);
    let entries = fields[3].fields(2)?;
    let parse = |position: usize| -> Result<(ParsedRuntimeLocator2, SocketAddr)> {
        let want = scope.slots[order[position]];
        rejection.set(Rejection::SignedBody);
        let entry = entries[position].fields(2)?;
        rejection.set(Rejection::Consent);
        require(
            entry[0].fixed::<32>()? == want.consent_ref
                && want.consent_interval.0 <= issued
                && expires <= want.consent_interval.1,
            "discovery consent scope or interval",
        )?;
        rejection.set(Rejection::LocatorParse);
        let raw = entry[1].bytes()?;
        require(raw.len() <= LOCATOR_BYTES, "discovery locator size")?;
        let locator = ParsedRuntimeLocator2::parse(raw)?;
        rejection.set(Rejection::LocatorVerification);
        locator.verify(want.context, scope.sender, scope.recipient)?;
        rejection.set(Rejection::LocatorInterval);
        require(
            locator.fields().not_before <= issued && expires <= locator.fields().expires,
            "discovery locator interval",
        )?;
        rejection.set(Rejection::Route);
        let socket = locator.direct_lan_socket(scope.sender_ipv4)?;
        Ok((locator, socket))
    };
    let (first, socket) = parse(0)?;
    let (second, other) = parse(1)?;
    rejection.set(Rejection::LocatorPair);
    require(
        first.fields().endpoint == second.fields().endpoint && socket == other,
        "discovery locator pair disagrees",
    )?;
    Ok(Candidate {
        locators: if order[0] == 0 {
            [first, second]
        } else {
            [second, first]
        },
        socket,
        announcement_interval: (issued, expires),
    })
}

/// Create an announcement from the two signed locator bytes in local slot order.
pub fn seal(
    scope: &Scope<'_>,
    signer: &DeviceIdentity,
    issued: u64,
    expires: u64,
    locators: [&[u8]; 2],
) -> Result<[u8; DATAGRAM_BYTES]> {
    require(
        signer.device_id() == scope.sender.certificate().device_id(),
        "discovery signer",
    )?;
    let rejection = Cell::new(Rejection::Unknown);
    let order = wire_order(scope, &rejection)?;
    require(
        locators.iter().all(|raw| raw.len() <= LOCATOR_BYTES),
        "discovery locator size",
    )?;
    let body = Zeroizing::new(a([
        u(VERSION),
        u(issued),
        u(expires),
        a(order.map(|slot| a([b(scope.slots[slot].consent_ref), b(locators[slot])]))),
    ]));
    validate_body(&body, scope, order, &rejection)?;
    let signature = signer.sign(&Zeroizing::new(signing_bytes(SIGN, &body, BODY_BYTES)?));
    let signed = Zeroizing::new(encode(
        &Zeroizing::new(a([(*body).clone(), b(signature)])),
        SIGNED_BYTES,
    )?);
    let mut plain = Zeroizing::new([0_u8; PLAIN_BYTES]);
    plain[..2].copy_from_slice(&(signed.len() as u16).to_be_bytes());
    plain[2..2 + signed.len()].copy_from_slice(&signed);
    let sealed = scope
        .recipient
        .certificate()
        .encryption_public_key()
        .seal(&plain[..], INFO, &aad()?)
        .map_err(|_| Error::Invalid("discovery sealing"))?;
    require(
        sealed.encapsulated_key.len() == 32 && sealed.ciphertext.len() == CIPHERTEXT_BYTES,
        "discovery HPKE size",
    )?;
    encode(
        &a([u(VERSION), b(sealed.encapsulated_key), b(sealed.ciphertext)]),
        DATAGRAM_BYTES,
    )?
    .try_into()
    .map_err(|_| Error::Invalid("discovery datagram size"))
}

/// Decrypt, verify and map the current announcement to the caller's local slots.
/// Replays within the window may yield the same candidate.
pub fn open(
    raw: &[u8],
    scope: &Scope<'_>,
    recipient: &DeviceEncryptionIdentity,
) -> Result<Candidate> {
    open_observed(raw, scope, recipient, |_| {})
}

/// The same single validation path as `open`. Reports exactly one fixed site on
/// failure and nothing on success. Return values and validation order are unchanged.
pub fn open_observed(
    raw: &[u8],
    scope: &Scope<'_>,
    recipient: &DeviceEncryptionIdentity,
    rejected: impl FnOnce(Rejection),
) -> Result<Candidate> {
    let rejection = Cell::new(Rejection::Unknown);
    let result = (|| {
        rejection.set(Rejection::FrameSize);
        require(raw.len() == DATAGRAM_BYTES, "discovery datagram size")?;
        rejection.set(Rejection::RecipientKey);
        require(
            recipient.public_key() == scope.recipient.certificate().encryption_public_key(),
            "discovery recipient key",
        )?;
        let order = wire_order(scope, &rejection)?;
        rejection.set(Rejection::Envelope);
        let outer = decode(raw, DATAGRAM_BYTES)?;
        let fields = outer.fields(3)?;
        require(fields[0].uint()? == VERSION, "discovery envelope version")?;
        let sealed = SealedMessage {
            encapsulated_key: fields[1].fixed::<32>()?.to_vec(),
            ciphertext: fields[2].fixed::<CIPHERTEXT_BYTES>()?.to_vec(),
        };
        rejection.set(Rejection::Decryption);
        let plain = Zeroizing::new(
            recipient
                .open(&sealed, INFO, &aad()?)
                .map_err(|_| Error::Invalid("discovery decryption"))?,
        );
        rejection.set(Rejection::SignedBody);
        require(plain.len() == PLAIN_BYTES, "discovery plaintext size")?;
        let signed_len = usize::from(u16::from_be_bytes([plain[0], plain[1]]));
        require(
            (1..=SIGNED_BYTES).contains(&signed_len),
            "discovery signed frame size",
        )?;
        let end = 2 + signed_len;
        require(
            plain[end..].iter().all(|&byte| byte == 0),
            "discovery padding",
        )?;
        let signed = Zeroizing::new(decode(&plain[2..end], SIGNED_BYTES)?);
        let fields = signed.fields(2)?;
        let signature = fields[1].fixed::<64>()?;
        let signed_bytes = Zeroizing::new(signing_bytes(SIGN, &fields[0], BODY_BYTES)?);
        rejection.set(Rejection::Signature);
        scope
            .sender
            .certificate()
            .device_id()
            .verify(&signed_bytes, &signature)?;
        validate_body(&fields[0], scope, order, &rejection)
    })();
    if result.is_err() {
        rejected(rejection.get());
    }
    result
}

#[cfg(test)]
mod rejection_tests;
