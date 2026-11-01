//! The same native account/session under the distinct Mixed7 local envelope.
//! Conversion changes encoding, never creates or replays an Olm operation.
use super::*;
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession},
    ordered_v2::{self as g2, Guard2},
};
use zeroize::Zeroizing;

const INFO: &[u8] = b"kg:od2:migrated-private\0";
pub(super) struct MixedIdentity7 {
    pub incarnation: Id,
    pub claim: Id,
    pub source: Id,
}
pub(super) enum Native7 {
    Legacy(PrivateState),
    Mixed(MixedNative7),
}
pub(super) struct MixedNative7 {
    pub key: Zeroizing<Id>,
    pub account: OrderedAccount,
    pub session: OrderedSession,
    pub guard: Guard2,
    pub reservation: Option<(Id, Id, Id, u64)>,
}
type NativeConversion7 = (MixedNative7, Vec<(u8, Id, u64, Id)>);
impl MixedNative7 {
    pub fn fork(&self) -> Self {
        Self {
            key: Zeroizing::new(*self.key),
            account: self.account.fork(),
            session: self.session.fork(),
            guard: self.guard.clone(),
            reservation: self.reservation,
        }
    }
    pub fn from_legacy(
        legacy: PrivateState,
        imported: g2::ImportedL1,
        head: &Head,
    ) -> Result<NativeConversion7> {
        ensure!(head.phase == 3, "NeedHandshake");
        let old = legacy.guard.as_ref().context("current Guard1 missing")?;
        old.validate(head.role)?;
        let lane = |l: &super::super::ratchet::Lane| g2::Lane {
            next: l.next,
            last: l.last,
            g: l.g,
            key: l.key,
            index: l.index,
            first: l.first,
            parent: l.parent,
            sequence: l.sequence,
        };
        let expected = g2::compact_l1_checkpoint(g2::L1CheckpointFacts {
            role: head.role.try_into()?,
            own: lane(&old.own),
            peer: lane(&old.peer),
            rotate_due: old.rotate_due,
            received_new_first: old.received_new_first,
            used_own_first: old.used_own_first,
        })?;
        ensure!(
            imported.guard.encode()? == expected,
            "current Guard1/Guard2 exact conversion"
        );
        let mut seen = Vec::with_capacity(imported.seen.len());
        let mut keys = std::collections::BTreeSet::new();
        for (role, key, value) in imported.seen {
            ensure!(
                role < 2 && keys.insert((role, key)),
                "conversion Seen unique roles"
            );
            let lane = if u64::from(role) == head.role {
                &old.own
            } else {
                &old.peer
            };
            ensure!(lane.seen.contains(&key), "conversion Seen current key");
            seen.push((role, key, value.generation, value.first));
        }
        ensure!(
            seen.len() == old.own.seen.len() + old.peer.seen.len(),
            "conversion Seen complete"
        );
        let session = legacy.session.context("current native session")?;
        ensure!(
            head.sid.as_deref() == Some(session.sid().as_str()),
            "conversion SID unchanged"
        );
        Ok((
            Self {
                key: legacy.key,
                account: legacy.account,
                session,
                guard: imported.guard,
                reservation: legacy.reservation,
            },
            seen,
        ))
    }
    fn aad(identity: &MixedIdentity7, head: &Head, q: Id) -> Result<Vec<u8>> {
        ensure!(head.phase == 3, "Mixed7 phase");
        Ok(wire::encode(
            &a([
                u(7),
                b(identity.incarnation),
                b(identity.claim),
                b(identity.source),
                b(head.c),
                b(head.local_cert),
                u(head.role),
                u(head.phase),
                u(head.native_generation),
                b(head.offer.context("Mixed7 Offer1")?),
                b(head.grant.context("Mixed7 Grant1")?),
                b(head.d.context("Mixed7 D")?),
                b(head.e.context("Mixed7 E")?),
                b(head.sid.as_ref().context("Mixed7 SID")?.as_bytes()),
                b(q),
            ]),
            2048,
        )?)
    }
    pub fn seal(
        &self,
        identity: &MixedIdentity7,
        head: &mut Head,
        q: Id,
        device: &DeviceEncryptionIdentity,
    ) -> Result<Vec<u8>> {
        ensure!(
            head.sid.as_deref() == Some(self.session.sid().as_str())
                && self.guard.role() == head.role as u8,
            "Mixed7 session/role"
        );
        let reservation = self
            .reservation
            .map(|r| a([b(r.0), b(r.1), b(r.2), u(r.3)]));
        let payload = Zeroizing::new(a([
            u(7),
            u(3),
            b(&self.key[..]),
            b(self.account.snapshot(&self.key)),
            b(self.session.snapshot(&self.key)),
            b(self.guard.encode()?),
            opt(reservation),
        ]));
        let plain = Zeroizing::new(wire::encode(&payload, state::MAX_NATIVE)?);
        let sealed = device
            .public_key()
            .seal(&plain, INFO, &Self::aad(identity, head, q)?)?;
        let raw = wire::encode(
            &a([
                u(7),
                u(3),
                u(head.native_generation),
                b(sealed.encapsulated_key),
                b(sealed.ciphertext),
            ]),
            state::MAX_NATIVE,
        )?;
        head.blob = Some(wire::hash(store::MIXED_NATIVE_ID, &raw));
        head.validate()?;
        Ok(raw)
    }
    pub fn open(
        identity: &MixedIdentity7,
        head: &Head,
        q: Id,
        device: &DeviceEncryptionIdentity,
        raw: &[u8],
    ) -> Result<Self> {
        ensure!(
            raw.len() <= state::MAX_NATIVE
                && head.blob == Some(wire::hash(store::MIXED_NATIVE_ID, raw)),
            "Mixed7 current hash/cap"
        );
        let v = wire::decode(raw, state::MAX_NATIVE)?;
        let f = v.fields(5)?;
        ensure!(
            f[0].uint()? == 7 && f[1].uint()? == 3 && f[2].uint()? == head.native_generation,
            "Mixed7 envelope version/generation"
        );
        let plain = Zeroizing::new(device.open(
            &SealedMessage {
                encapsulated_key: f[3].bytes()?.to_vec(),
                ciphertext: f[4].bytes()?.to_vec(),
            },
            INFO,
            &Self::aad(identity, head, q)?,
        )?);
        let v = Zeroizing::new(wire::decode(&plain, state::MAX_NATIVE)?);
        let f = v.fields(7)?;
        ensure!(
            f[0].uint()? == 7 && f[1].uint()? == 3,
            "Mixed7 payload discriminator"
        );
        let key = Zeroizing::new(f[2].fixed::<32>()?);
        let account = OrderedAccount::restore(f[3].bytes()?, &key)?;
        let session = OrderedSession::restore(f[4].bytes()?, &key)?;
        ensure!(
            head.sid.as_deref() == Some(session.sid().as_str()),
            "Mixed7 restored SID"
        );
        let guard = Guard2::restore(f[5].bytes()?, head.role.try_into()?)?;
        let reservation = f[6]
            .option()?
            .map(|v| -> Result<_> {
                let r = v.fields(4)?;
                ensure!(r[3].uint()? == 1, "Mixed7 consumed OTK");
                Ok((
                    r[0].fixed::<32>()?,
                    r[1].fixed::<32>()?,
                    r[2].fixed::<32>()?,
                    r[3].uint()?,
                ))
            })
            .transpose()?;
        if let Some(r) = reservation {
            ensure!(
                head.role == 1 && head.offer == Some(r.0) && head.grant == Some(r.1),
                "Mixed7 reservation Head"
            );
        } else {
            ensure!(head.role == 0, "Mixed7 B reservation absent");
        }
        Ok(Self {
            key,
            account,
            session,
            guard,
            reservation,
        })
    }
}
