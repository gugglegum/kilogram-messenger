//! C1 phase-tagged device-local private state. The vault Head is authoritative.
use super::ratchet::Guard;
use anyhow::{Context, Result, ensure};
use kilogram_crypto::{DeviceEncryptionIdentity, EncryptionPublicKey, SealedMessage};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Id, Value, a, b, opt, optional_id, u};
use kilogram_ratchet::ordered::{OrderedAccount, OrderedSession};
use zeroize::Zeroizing;

pub const MAX_NATIVE: usize = 128 * 1024;
pub const NATIVE_TAG: &[u8] = b"kg:od1:native-state\0";

#[derive(Clone, Debug)]
pub struct Head {
    pub c: Id,
    pub local_cert: Id,
    pub role: u64,
    pub generation: u64,
    pub phase: u64,
    pub native_generation: u64,
    pub offer: Option<Id>,
    pub grant: Option<Id>,
    pub d: Option<Id>,
    pub e: Option<Id>,
    pub sid: Option<String>,
    pub blob: Option<Id>,
}
impl Head {
    pub fn absent(c: Id, local_cert: Id, role: u64) -> Self {
        Self {
            c,
            local_cert,
            role,
            generation: 0,
            phase: 0,
            native_generation: 0,
            offer: None,
            grant: None,
            d: None,
            e: None,
            sid: None,
            blob: None,
        }
    }
    pub fn value(&self) -> Value {
        a([
            u(1),
            b(self.c),
            b(self.local_cert),
            u(self.role),
            u(self.generation),
            u(self.phase),
            u(self.native_generation),
            opt(self.offer.map(b)),
            opt(self.grant.map(b)),
            opt(self.d.map(b)),
            opt(self.e.map(b)),
            opt(self.sid.as_ref().map(|s| b(s.as_bytes()))),
            opt(self.blob.map(b)),
        ])
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(wire::encode(&self.value(), 2048)?)
    }
    pub fn parse(raw: &[u8], c: Id, cert: Id, role: u64) -> Result<Self> {
        let v = wire::decode(raw, 2048)?;
        let f = v.fields(13)?;
        ensure!(f[0].uint()? == 1, "OD1 Head version");
        let h = Self {
            c: f[1].fixed()?,
            local_cert: f[2].fixed()?,
            role: f[3].uint()?,
            generation: f[4].uint()?,
            phase: f[5].uint()?,
            native_generation: f[6].uint()?,
            offer: optional_id(&f[7])?,
            grant: optional_id(&f[8])?,
            d: optional_id(&f[9])?,
            e: optional_id(&f[10])?,
            sid: f[11]
                .option()?
                .map(|v| v.text().map(ToOwned::to_owned))
                .transpose()?,
            blob: optional_id(&f[12])?,
        };
        ensure!(
            h.c == c && h.local_cert == cert && h.role == role && role <= 1,
            "OD1 Head scope"
        );
        h.validate()?;
        Ok(h)
    }
    pub fn validate(&self) -> Result<()> {
        let no_epoch =
            self.grant.is_none() && self.d.is_none() && self.e.is_none() && self.sid.is_none();
        let epoch =
            self.offer.is_some() && self.grant.is_some() && self.d.is_some() && self.e.is_some();
        let valid = match self.phase {
            0 => {
                self.native_generation == 0
                    && self.offer.is_none()
                    && no_epoch
                    && self.blob.is_none()
            }
            1 => {
                self.role == 0
                    && self.native_generation == 1
                    && self.offer.is_some()
                    && no_epoch
                    && self.blob.is_some()
            }
            2 => {
                self.role == 1
                    && self.native_generation == 1
                    && epoch
                    && self.sid.is_none()
                    && self.blob.is_some()
            }
            3 => {
                self.native_generation >= 2
                    && epoch
                    && self
                        .sid
                        .as_ref()
                        .is_some_and(|s| s.len() == 43 && s.is_ascii())
                    && self.blob.is_some()
            }
            _ => false,
        };
        ensure!(valid, "OD1 private phase availability");
        Ok(())
    }
    pub fn aad(&self, profile_nonce: Id) -> Result<Vec<u8>> {
        Ok(wire::encode(
            &a([
                u(1),
                b(profile_nonce),
                b(self.c),
                b(self.local_cert),
                u(self.role),
                u(self.phase),
                u(self.native_generation),
                opt(self.offer.map(b)),
                opt(self.grant.map(b)),
                opt(self.d.map(b)),
                opt(self.e.map(b)),
                opt(self.sid.as_ref().map(|s| b(s.as_bytes()))),
            ]),
            2048,
        )?)
    }
    pub fn bump(&mut self, native: bool) -> Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .context("OD1 context generation exhausted")?;
        if native {
            self.native_generation = self
                .native_generation
                .checked_add(1)
                .context("OD1 native generation exhausted")?;
        }
        Ok(())
    }
}

pub struct PrivateState {
    pub key: Zeroizing<[u8; 32]>,
    pub account: OrderedAccount,
    pub session: Option<OrderedSession>,
    pub guard: Option<Guard>,
    pub reservation: Option<(Id, Id, Id, u64)>,
}
impl PrivateState {
    pub fn fresh(receiver: bool) -> Result<(Self, Option<Id>)> {
        let mut key = Zeroizing::new([0_u8; 32]);
        getrandom::fill(key.as_mut())?;
        let (account, otk) = OrderedAccount::fresh(receiver);
        ensure!(!receiver || otk.is_some(), "OD1 OTK generation");
        Ok((
            Self {
                key,
                account,
                session: None,
                guard: None,
                reservation: None,
            },
            otk,
        ))
    }
    pub fn fork(&self) -> Self {
        Self {
            key: Zeroizing::new(*self.key),
            account: self.account.fork(),
            session: self.session.as_ref().map(OrderedSession::fork),
            guard: self.guard.clone(),
            reservation: self.reservation,
        }
    }
    fn payload(&self, phase: u64) -> Result<Value> {
        let account = b(self.account.snapshot(&self.key));
        let reservation = |r: (Id, Id, Id, u64)| a([b(r.0), b(r.1), b(r.2), u(r.3)]);
        Ok(match phase {
            1 => {
                ensure!(
                    self.session.is_none() && self.reservation.is_none(),
                    "OfferedA payload shape"
                );
                a([u(1), u(1), b(&self.key[..]), account])
            }
            2 => {
                let r = self.reservation.context("GrantedB reservation missing")?;
                ensure!(r.3 == 0 && self.session.is_none(), "GrantedB payload shape");
                a([u(1), u(2), b(&self.key[..]), account, reservation(r)])
            }
            3 => {
                let session = self.session.as_ref().context("Selected session missing")?;
                let guard = self.guard.as_ref().context("Selected guard missing")?;
                a([
                    u(1),
                    u(3),
                    b(&self.key[..]),
                    account,
                    b(session.snapshot(&self.key)),
                    b(postcard::to_allocvec(guard)?),
                    opt(self.reservation.map(reservation)),
                ])
            }
            _ => anyhow::bail!("no private payload for phase"),
        })
    }
    /// The blob hash is deliberately excluded from AAD, avoiding a hash cycle.
    pub fn seal(
        &self,
        head: &mut Head,
        profile_nonce: Id,
        device: EncryptionPublicKey,
    ) -> Result<Vec<u8>> {
        let payload = Zeroizing::new(self.payload(head.phase)?);
        let plaintext = Zeroizing::new(wire::encode(&payload, MAX_NATIVE)?);
        let aad = head.aad(profile_nonce)?;
        let sealed = device.seal(&plaintext, info(head.phase)?, &aad)?;
        let blob = wire::encode(
            &a([
                u(1),
                u(head.phase),
                u(head.native_generation),
                a([b(&sealed.encapsulated_key), b(&sealed.ciphertext)]),
            ]),
            MAX_NATIVE,
        )?;
        head.blob = Some(wire::hash(NATIVE_TAG, &blob));
        head.validate()?;
        Ok(blob)
    }
    pub fn open(
        head: &Head,
        profile_nonce: Id,
        device: &DeviceEncryptionIdentity,
        raw: &[u8],
    ) -> Result<Self> {
        head.validate()?;
        ensure!(
            head.phase != 0 && head.blob == Some(wire::hash(NATIVE_TAG, raw)),
            "OD1 private blob hash"
        );
        let v = wire::decode(raw, MAX_NATIVE)?;
        let f = v.fields(4)?;
        let sealed = f[3].fields(2)?;
        ensure!(
            f[0].uint()? == 1
                && f[1].uint()? == head.phase
                && f[2].uint()? == head.native_generation,
            "OD1 private blob phase/generation"
        );
        let message = SealedMessage {
            encapsulated_key: sealed[0].fixed::<32>()?.to_vec(),
            ciphertext: sealed[1].bytes()?.to_vec(),
        };
        let plain =
            Zeroizing::new(device.open(&message, info(head.phase)?, &head.aad(profile_nonce)?)?);
        let payload = Zeroizing::new(wire::decode(&plain, MAX_NATIVE)?);
        let len = match head.phase {
            1 => 4,
            2 => 5,
            3 => 7,
            _ => anyhow::bail!("OD1 private phase"),
        };
        let f = payload.fields(len)?;
        ensure!(
            f[0].uint()? == 1 && f[1].uint()? == head.phase,
            "OD1 private payload version/phase"
        );
        let key = Zeroizing::new(f[2].fixed::<32>()?);
        let account = OrderedAccount::restore(f[3].bytes()?, &key)?;
        let reservation = |v: &Value| -> Result<(Id, Id, Id, u64)> {
            let r = v.fields(4)?;
            Ok((r[0].fixed()?, r[1].fixed()?, r[2].fixed()?, r[3].uint()?))
        };
        let (session, guard, reserved) = match head.phase {
            1 => (None, None, None),
            2 => (None, None, Some(reservation(&f[4])?)),
            3 => {
                let s = OrderedSession::restore(f[4].bytes()?, &key)?;
                ensure!(
                    Some(s.sid()).as_ref() == head.sid.as_ref(),
                    "OD1 native SID mismatch"
                );
                let g: Guard = postcard::from_bytes(f[5].bytes()?)?;
                g.validate(head.role)?;
                (
                    Some(s),
                    Some(g),
                    f[6].option()?.map(reservation).transpose()?,
                )
            }
            _ => anyhow::bail!("OD1 private payload phase"),
        };
        if let Some(r) = reserved {
            ensure!(
                Some(r.0) == head.offer
                    && Some(r.1) == head.grant
                    && head.role == 1
                    && r.3 == if head.phase == 2 { 0 } else { 1 },
                "OD1 reservation binding"
            );
        } else {
            ensure!(
                head.role == 0 || head.phase == 1,
                "OD1 missing B reservation"
            );
        }
        Ok(Self {
            key,
            account,
            session,
            guard,
            reservation: reserved,
        })
    }
}
fn info(phase: u64) -> Result<&'static [u8]> {
    Ok(match phase {
        1 => b"kg:od1:private-offered-a\0",
        2 => b"kg:od1:private-granted-b\0",
        3 => b"kg:od1:private-selected\0",
        _ => anyhow::bail!("OD1 private phase tag"),
    })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn parse_id(text: &str) -> Result<Id> {
    ensure!(
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "expected 64 lowercase hex characters"
    );
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)?;
    }
    Ok(out)
}
pub fn head_path(c: Id) -> String {
    format!("runtime/od1/contexts/{}/head", hex(&c))
}
pub fn native_path(c: Id) -> String {
    format!("ratchet/od1-contexts/{}/native", hex(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_pre_session_blobs_need_no_future_ids_and_reject_swaps() -> Result<()> {
        let device = DeviceEncryptionIdentity::from_secret_bytes([11; 32]);
        let (private, _) = PrivateState::fresh(false)?;
        let curve = private.account.curve_key();
        let mut head = Head::absent([1; 32], [2; 32], 0);
        head.phase = 1;
        head.offer = Some([3; 32]);
        head.bump(true)?;
        let raw = private.seal(&mut head, [4; 32], device.public_key())?;
        let parsed = Head::parse(&head.encode()?, head.c, head.local_cert, 0)?;
        let restored = PrivateState::open(&parsed, [4; 32], &device, &raw)?;
        assert_eq!(restored.account.curve_key(), curve);
        assert!(parsed.d.is_none() && parsed.e.is_none() && parsed.sid.is_none());
        assert!(PrivateState::open(&parsed, [5; 32], &device, &raw).is_err());
        let mut swapped = parsed.clone();
        swapped.c = [9; 32];
        assert!(PrivateState::open(&swapped, [4; 32], &device, &raw).is_err());
        let mut stale = parsed.clone();
        stale.native_generation += 1;
        assert!(PrivateState::open(&stale, [4; 32], &device, &raw).is_err());
        let (mut private, otk) = PrivateState::fresh(true)?;
        let otk = otk.context("test OTK")?;
        let mut head = Head::absent([6; 32], [7; 32], 1);
        head.phase = 2;
        head.offer = Some([8; 32]);
        head.grant = Some([9; 32]);
        head.d = Some([10; 32]);
        head.e = Some([11; 32]);
        head.bump(true)?;
        private.reservation = Some(([8; 32], [9; 32], otk, 0));
        let curve = private.account.curve_key();
        let raw = private.seal(&mut head, [4; 32], device.public_key())?;
        let restored = PrivateState::open(&head, [4; 32], &device, &raw)?;
        assert_eq!(restored.account.curve_key(), curve);
        assert_eq!(restored.reservation, private.reservation);
        assert!(head.sid.is_none());
        Ok(())
    }
}
