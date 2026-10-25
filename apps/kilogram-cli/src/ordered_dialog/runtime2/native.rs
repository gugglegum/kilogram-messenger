use super::*;
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession},
    ordered_v2::{self as guard, Guard2},
};
use zeroize::Zeroizing;

pub(super) struct Native2 {
    pub key: Zeroizing<Id>,
    pub account: OrderedAccount,
    pub session: Option<OrderedSession>,
    pub guard: Option<Guard2>,
    pub reservation: Option<(Id, Id, Id, u8)>,
}
impl Native2 {
    pub fn fresh(receiver: bool) -> Result<(Self, Option<Id>)> {
        let (account, otk) = OrderedAccount::fresh(receiver);
        ensure!(!receiver || otk.is_some(), "OTK generation");
        Ok((
            Self {
                key: Zeroizing::new(shared_admin::random()?),
                account,
                session: None,
                guard: None,
                reservation: None,
            },
            otk,
        ))
    }
    pub fn fork(&self) -> Self {
        let _timing = timing::span(timing::Metric::NativeFork);
        Self {
            key: Zeroizing::new(*self.key),
            account: self.account.fork(),
            session: self.session.as_ref().map(OrderedSession::fork),
            guard: self.guard.clone(),
            reservation: self.reservation,
        }
    }
    pub fn guard(&self) -> Result<&Guard2> {
        self.guard.as_ref().context("NeedHandshake")
    }
    pub fn guard_mut(&mut self) -> Result<&mut Guard2> {
        self.guard.as_mut().context("NeedHandshake")
    }
    fn aad(profile: &db::RuntimeProfile, head: &db::RuntimeHead) -> Result<Vec<u8>> {
        Ok(wire::encode(
            &a([
                u(2),
                b(profile.incarnation),
                b(profile.claim),
                b(head.context),
                b(wire::hash(b"", &profile.enrollment.certificate)),
                u(u64::from(head.role)),
                u(u64::from(head.phase)),
                u(head.native_generation),
                opt(head.offer.map(b)),
                opt(head.grant.map(b)),
                opt(head.dialog.map(b)),
                opt(head.epoch.map(b)),
                opt(head.sid.as_ref().map(|s| b(s.as_bytes()))),
                opt(head.mode_ref.map(b)),
            ]),
            2048,
        )?)
    }
    fn payload(&self, head: &db::RuntimeHead) -> Result<Value> {
        let reservation = |r: (Id, Id, Id, u8)| a([b(r.0), b(r.1), b(r.2), u(u64::from(r.3))]);
        let account = b(self.account.snapshot(&self.key));
        Ok(match head.phase {
            1 => {
                ensure!(
                    self.session.is_none() && self.guard.is_none() && self.reservation.is_none(),
                    "OfferedA shape"
                );
                a([u(2), u(1), b(&self.key[..]), account])
            }
            2 => {
                ensure!(
                    self.session.is_none() && self.guard.is_none(),
                    "GrantedB shape"
                );
                let r = self.reservation.context("OTK reservation")?;
                ensure!(r.3 == 0, "OTK used");
                a([u(2), u(2), b(&self.key[..]), account, reservation(r)])
            }
            3 => a([
                u(2),
                u(3),
                b(&self.key[..]),
                account,
                b(self
                    .session
                    .as_ref()
                    .context("session")?
                    .snapshot(&self.key)),
                b(self.guard()?.encode()?),
                opt(self.reservation.map(reservation)),
            ]),
            _ => anyhow::bail!("native phase"),
        })
    }
    pub fn seal(
        &self,
        profile: &db::RuntimeProfile,
        head: &db::RuntimeHead,
        device: &DeviceEncryptionIdentity,
    ) -> Result<Vec<u8>> {
        let _timing = timing::span(timing::Metric::NativeSeal);
        let payload = Zeroizing::new(self.payload(head)?);
        let plain = Zeroizing::new(wire::encode(&payload, guard::MAX_CHECKPOINT)?);
        let sealed = device.public_key().seal(
            &plain,
            b"kg:od2:private-state\0",
            &Self::aad(profile, head)?,
        )?;
        let raw = wire::encode(
            &a([
                u(2),
                u(u64::from(head.phase)),
                u(head.native_generation),
                b(sealed.encapsulated_key),
                b(sealed.ciphertext),
            ]),
            guard::MAX_CHECKPOINT,
        )?;
        guard::check_checkpoint_size(&raw)?;
        Ok(raw)
    }
    pub fn open(
        profile: &db::RuntimeProfile,
        head: &db::RuntimeHead,
        device: &DeviceEncryptionIdentity,
        raw: &[u8],
    ) -> Result<Self> {
        guard::check_checkpoint_size(raw)?;
        ensure!(
            head.native_hash == Some(wire::hash(b"", raw)),
            "native Head hash"
        );
        let value = wire::decode(raw, guard::MAX_CHECKPOINT)?;
        let f = value.fields(5)?;
        ensure!(
            f[0].uint()? == 2
                && f[1].uint()? == u64::from(head.phase)
                && f[2].uint()? == head.native_generation,
            "native phase/generation"
        );
        let plain = Zeroizing::new(device.open(
            &SealedMessage {
                encapsulated_key: f[3].fixed::<32>()?.to_vec(),
                ciphertext: f[4].bytes()?.to_vec(),
            },
            b"kg:od2:private-state\0",
            &Self::aad(profile, head)?,
        )?);
        let payload = Zeroizing::new(wire::decode(&plain, guard::MAX_CHECKPOINT)?);
        let f = payload.fields(match head.phase {
            1 => 4,
            2 => 5,
            3 => 7,
            _ => anyhow::bail!("native phase"),
        })?;
        ensure!(
            f[0].uint()? == 2 && f[1].uint()? == u64::from(head.phase),
            "native payload version"
        );
        let key = Zeroizing::new(f[2].fixed::<32>()?);
        let account = OrderedAccount::restore(f[3].bytes()?, &key)?;
        let reserved = |v: &Value| -> Result<(Id, Id, Id, u8)> {
            let r = v.fields(4)?;
            ensure!(r[3].uint()? <= 1, "reservation used");
            Ok((
                r[0].fixed()?,
                r[1].fixed()?,
                r[2].fixed()?,
                r[3].uint()? as u8,
            ))
        };
        let (session, guard, reservation) = match head.phase {
            1 => (None, None, None),
            2 => (None, None, Some(reserved(&f[4])?)),
            3 => {
                let session = OrderedSession::restore(f[4].bytes()?, &key)?;
                ensure!(
                    Some(session.sid()).as_ref() == head.sid.as_ref(),
                    "native SID"
                );
                (
                    Some(session),
                    Some(Guard2::restore(f[5].bytes()?, head.role)?),
                    f[6].option()?.map(reserved).transpose()?,
                )
            }
            _ => anyhow::bail!("native phase"),
        };
        if let Some(r) = reservation {
            ensure!(
                head.role == 1
                    && head.offer == Some(r.0)
                    && head.grant == Some(r.1)
                    && r.3 == u8::from(head.phase == 3),
                "reservation Head binding"
            );
        } else {
            ensure!(head.role == 0, "missing B reservation");
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
