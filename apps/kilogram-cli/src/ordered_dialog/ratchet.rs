//! Public ordering guard around a jointly serialized native session.
use kilogram_protocol::ordered_dialog_v1::{ClosedTail, Event, Expected, Id, LIFETIME};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    Input(&'static str),
    Operational(&'static str),
    UpgradeRequired,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OD1 guard: {self:?}")
    }
}
impl std::error::Error for Failure {}
type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Lane {
    pub next: u64,
    pub last: Option<Id>,
    pub g: u64,
    pub key: Option<Id>,
    pub index: u64,
    pub first: Option<Id>,
    pub parent: Option<Id>,
    pub seen: BTreeSet<Id>,
    pub sequence: Option<u64>,
}
impl Lane {
    fn tail(&self) -> Result<ClosedTail> {
        Ok(ClosedTail {
            g: self.g,
            key: self.key.ok_or(Failure::Operational("tail key"))?,
            index: self.index,
            last: self.last.ok_or(Failure::Operational("tail last"))?,
        })
    }
    fn record(&mut self, event: &Event, id: Id) {
        if self.key != Some(event.key) {
            self.first = Some(id);
            self.seen.insert(event.key);
        }
        self.next = event.core.n + 1;
        self.last = Some(id);
        self.g = event.core.expected.g;
        self.key = Some(event.key);
        self.index = event.index;
        self.parent = event.core.expected.parent;
        self.sequence = Some(event.core.sequence);
    }
    fn validate(&self) -> Result<()> {
        let empty = self.next == 0;
        if self.next > LIFETIME
            || self.seen.len() > LIFETIME as usize
            || self.g >= LIFETIME
            || empty != self.last.is_none()
            || empty != self.first.is_none()
            || empty != self.key.is_none()
            || self.key.is_some_and(|k| !self.seen.contains(&k))
        {
            return Err(Failure::Operational("checkpoint lane shape"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guard {
    pub version: u64,
    pub role: u64,
    pub own: Lane,
    pub peer: Lane,
    pub rotate_due: bool,
    pub received_new_first: Option<Id>,
    pub used_own_first: Option<Id>,
}
impl Guard {
    pub fn new(role: u64) -> Self {
        Self {
            version: 1,
            role,
            own: Lane::default(),
            peer: Lane::default(),
            rotate_due: false,
            received_new_first: None,
            used_own_first: None,
        }
    }
    pub fn validate(&self, role: u64) -> Result<()> {
        if self.version != 1 || self.role != role || role > 1 {
            return Err(Failure::Operational("checkpoint role/version"));
        }
        self.own.validate()?;
        self.peer.validate()
    }
    pub fn expected_send(&self) -> Result<Expected> {
        self.validate(self.role)?;
        if self.own.next >= LIFETIME {
            return Err(Failure::UpgradeRequired);
        }
        if self.own.next == 0 {
            return Ok(Expected {
                g: 0,
                index: 0,
                parent: if self.role == 0 {
                    None
                } else {
                    Some(
                        self.peer
                            .first
                            .ok_or(Failure::Operational("Confirm before Init"))?,
                    )
                },
                tail: None,
            });
        }
        if self.rotate_due {
            Ok(Expected {
                g: self.own.g + 1,
                index: 0,
                parent: Some(
                    self.received_new_first
                        .ok_or(Failure::Operational("rotation cause"))?,
                ),
                tail: Some(self.own.tail()?),
            })
        } else {
            Ok(Expected {
                g: self.own.g,
                index: self.own.index + 1,
                parent: self.own.parent,
                tail: None,
            })
        }
    }
    pub fn commit_send(&mut self, event: &Event, id: Id) -> Result<()> {
        let core = &event.core;
        if core.role != self.role
            || core.n != self.own.next
            || core.previous != self.own.last
            || core.expected != self.expected_send()?
            || event.index != core.expected.index
            || self.own.sequence.is_some_and(|s| core.sequence <= s)
        {
            return Err(Failure::Operational("own output/cursor binding"));
        }
        let new = self.own.next == 0 || self.rotate_due;
        if (new && self.own.seen.contains(&event.key))
            || (!new && Some(event.key) != self.own.key)
            || (core.kind == 1) != (event.native_kind == 0)
        {
            return Err(Failure::Operational("own native transition"));
        }
        self.own.record(event, id);
        self.rotate_due = false;
        Ok(())
    }
    /// Must precede the native trial. Exact duplicates/forks are handled by ledger first.
    pub fn check_receive(&self, event: &Event) -> Result<()> {
        self.validate(self.role)?;
        let core = &event.core;
        if core.role != 1 - self.role
            || core.n != self.peer.next
            || core.previous != self.peer.last
            || core.n >= LIFETIME
            || event.index != core.expected.index
            || self.peer.sequence.is_some_and(|s| core.sequence <= s)
        {
            return Err(Failure::Input(
                "ordinal/predecessor/native index/author sequence",
            ));
        }
        if self.peer.next == 0 {
            let parent = if core.role == 0 {
                None
            } else {
                Some(
                    self.own
                        .first
                        .ok_or(Failure::Operational("Confirm without own Init"))?,
                )
            };
            if core.kind != core.role + 1
                || core.expected
                    != (Expected {
                        g: 0,
                        index: 0,
                        parent,
                        tail: None,
                    })
                || event.index != 0
            {
                return Err(Failure::Input("genesis"));
            }
        } else if self.peer.key == Some(event.key) {
            if core.kind != 3
                || core.expected
                    != (Expected {
                        g: self.peer.g,
                        index: self.peer.index + 1,
                        parent: self.peer.parent,
                        tail: None,
                    })
            {
                return Err(Failure::Input("known chain continuation"));
            }
        } else {
            let first = self
                .own
                .first
                .ok_or(Failure::Input("peer DH before own chain"))?;
            if self.peer.seen.contains(&event.key)
                || Some(first) == self.used_own_first
                || core.kind != 3
                || core.expected
                    != (Expected {
                        g: self.peer.g + 1,
                        index: 0,
                        parent: Some(first),
                        tail: Some(self.peer.tail()?),
                    })
            {
                return Err(Failure::Input("new chain tail/cause/index"));
            }
        }
        Ok(())
    }
    /// Assign only after successful native MAC, inner binding and payload validation.
    pub fn commit_receive(&mut self, event: &Event, id: Id) -> Result<()> {
        self.check_receive(event)?;
        if self.peer.key != Some(event.key) {
            self.rotate_due = true;
            self.received_new_first = Some(id);
            self.used_own_first = self.own.first;
        }
        self.peer.record(event, id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::DeviceIdentity;
    use kilogram_protocol::ordered_dialog_v1::{a, b};
    use kilogram_ratchet::ordered::{OrderedAccount, OrderedSession, Packet};

    fn send(
        g: &mut Guard,
        s: &mut OrderedSession,
        signer: &DeviceIdentity,
    ) -> anyhow::Result<Event> {
        send_body(g, s, signer, b"text")
    }
    fn send_body(
        g: &mut Guard,
        s: &mut OrderedSession,
        signer: &DeviceIdentity,
        body: &[u8],
    ) -> anyhow::Result<Event> {
        let kind = if g.own.next == 0 { g.role + 1 } else { 3 };
        let core = kilogram_protocol::ordered_dialog_v1::Core {
            c: [1; 32],
            d: [2; 32],
            e: [3; 32],
            sid: s.sid(),
            role: g.role,
            n: g.own.next,
            previous: g.own.last,
            sequence: g.own.next,
            kind,
            request: [0; 16],
            parents: vec![],
            expected: g.expected_send()?,
            author_auth: [4; 32],
        };
        let inner = Event::inner(&core, a([b(body)]))?;
        let packet = s.encrypt(&inner)?;
        let header = packet.header()?;
        let mut event = Event {
            core,
            native_kind: u64::from(packet.kind),
            key: header.key,
            index: header.index,
            ciphertext: packet.bytes,
            signature: Vec::new(),
        };
        event.sign(signer)?;
        g.commit_send(&event, event.id()?)?;
        Ok(event)
    }
    #[test]
    fn ordered_full_lifetime_native_snapshot_and_max_text_stay_within_encoded_caps()
    -> anyhow::Result<()> {
        use super::super::state::{Head, PrivateState};
        use kilogram_crypto::DeviceEncryptionIdentity;
        let ia = DeviceIdentity::from_secret_bytes([41; 32]);
        let ib = DeviceIdentity::from_secret_bytes([42; 32]);
        let (aa, _) = OrderedAccount::fresh(false);
        let (mut ab, otk) = OrderedAccount::fresh(true);
        let otk = otk.ok_or_else(|| anyhow::anyhow!("OTK"))?;
        let mut sa = aa.outbound(ab.curve_key(), otk)?;
        let mut ga = Guard::new(0);
        let mut gb = Guard::new(1);
        let a0 = send(&mut ga, &mut sa, &ia)?;
        let (mut sb, plain) = ab.inbound(
            aa.curve_key(),
            &Packet {
                kind: 0,
                bytes: a0.ciphertext.clone(),
            },
        )?;
        a0.verify_inner(&plain)?;
        gb.commit_receive(&a0, a0.id()?)?;
        let b0 = send(&mut gb, &mut sb, &ib)?;
        receive(&mut ga, &mut sa, &b0)?;
        let text = vec![b'x'; 4096];
        let mut max_event = 0;
        for _ in 1..512 {
            let event = send_body(&mut ga, &mut sa, &ia, &text)?;
            max_event = max_event.max(event.raw()?.len());
            receive(&mut gb, &mut sb, &event)?;
            let event = send_body(&mut gb, &mut sb, &ib, &text)?;
            max_event = max_event.max(event.raw()?.len());
            receive(&mut ga, &mut sa, &event)?;
        }
        assert!(ga.expected_send().is_err());
        assert!(gb.expected_send().is_err());
        assert_eq!(ga.own.next, 512);
        assert_eq!(gb.peer.next, 512);
        let device = DeviceEncryptionIdentity::from_secret_bytes([43; 32]);
        let mut largest = 0;
        for (role, account, session, guard) in [(0, aa, sa, ga), (1, ab, sb, gb)] {
            let mut head = Head::absent([1; 32], [4; 32], role);
            head.phase = 3;
            head.offer = Some([5; 32]);
            head.grant = Some([6; 32]);
            head.d = Some([2; 32]);
            head.e = Some([3; 32]);
            head.sid = Some(session.sid());
            head.generation = 1024;
            head.native_generation = 1024;
            let private = PrivateState {
                key: zeroize::Zeroizing::new([44; 32]),
                account,
                session: Some(session),
                guard: Some(guard),
                reservation: if role == 1 {
                    Some(([5; 32], [6; 32], otk, 1))
                } else {
                    None
                },
            };
            let blob = private.seal(&mut head, [45; 32], device.public_key())?;
            largest = largest.max(blob.len());
            let restored = PrivateState::open(&head, [45; 32], &device, &blob)?;
            assert_eq!(
                restored
                    .guard
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("guard"))?
                    .peer
                    .next,
                512
            );
        }
        println!(
            "OD1 maximal encoded event_bytes={max_event} selected_blob_bytes={largest} lifetime_frames_per_lane=512"
        );
        assert!(largest <= 128 * 1024 && max_event <= 16 * 1024);
        Ok(())
    }
    fn receive(g: &mut Guard, s: &mut OrderedSession, event: &Event) -> anyhow::Result<()> {
        g.check_receive(event)?;
        let mut trial = s.fork();
        let raw = trial.decrypt(&Packet {
            kind: u8::try_from(event.native_kind)?,
            bytes: event.ciphertext.clone(),
        })?;
        event.verify_inner(&raw)?;
        g.commit_receive(event, event.id()?)?;
        *s = trial;
        Ok(())
    }
    #[test]
    fn ordered_late_tail_64_gap_and_forged_transition_never_skip_native() -> anyhow::Result<()> {
        // Component test of the real native boundary, not process-auth evidence.
        let ia = DeviceIdentity::from_secret_bytes([31; 32]);
        let ib = DeviceIdentity::from_secret_bytes([32; 32]);
        let (aa, _) = OrderedAccount::fresh(false);
        let (mut ab, otk) = OrderedAccount::fresh(true);
        let mut sa = aa.outbound(
            ab.curve_key(),
            otk.ok_or_else(|| anyhow::anyhow!("test OTK"))?,
        )?;
        let mut ga = Guard::new(0);
        let mut gb = Guard::new(1);
        let a0 = send(&mut ga, &mut sa, &ia)?;
        gb.check_receive(&a0)?;
        let (mut sb, plain) = ab.inbound(
            aa.curve_key(),
            &Packet {
                kind: 0,
                bytes: a0.ciphertext.clone(),
            },
        )?;
        a0.verify_inner(&plain)?;
        gb.commit_receive(&a0, a0.id()?)?;
        let b0 = send(&mut gb, &mut sb, &ib)?;
        receive(&mut ga, &mut sa, &b0)?;
        let a1 = send(&mut ga, &mut sa, &ia)?;
        receive(&mut gb, &mut sb, &a1)?;
        let b1 = send(&mut gb, &mut sb, &ib)?;
        let tail = (2..=65)
            .map(|_| send(&mut ga, &mut sa, &ia))
            .collect::<anyhow::Result<Vec<_>>>()?;
        for late in &tail[1..] {
            assert!(gb.check_receive(late).is_err());
        }
        receive(&mut ga, &mut sa, &b1)?;
        let successor = send(&mut ga, &mut sa, &ia)?;
        assert_ne!(successor.key, a1.key);
        assert_eq!(successor.index, 0);
        assert!(gb.check_receive(&successor).is_err());
        for event in &tail {
            receive(&mut gb, &mut sb, event)?;
        }
        let stable = postcard::to_allocvec(&gb)?;
        for mutation in 0..4 {
            let mut forged = successor.clone();
            match mutation {
                0 => forged.core.expected.index = 1,
                1 => forged.core.expected.parent = Some([91; 32]),
                2 => {
                    forged
                        .core
                        .expected
                        .tail
                        .as_mut()
                        .ok_or_else(|| anyhow::anyhow!("tail"))?
                        .last = [92; 32]
                }
                _ => forged.core.expected.tail = None,
            }
            forged.sign(&ia)?;
            let forged = Event::parse(&forged.raw()?, ia.device_id())?;
            assert!(receive(&mut gb, &mut sb, &forged).is_err());
            assert_eq!(postcard::to_allocvec(&gb)?, stable);
        }
        receive(&mut gb, &mut sb, &successor)?;
        assert_eq!(gb.peer.next, 67);
        assert_eq!(gb.peer.g, 2);
        let good = send(&mut ga, &mut sa, &ia)?;
        let mut forged = good.clone();
        forged.core.expected.g += 1;
        forged.sign(&ia)?;
        let forged = Event::parse(&forged.raw()?, ia.device_id())?;
        assert!(gb.check_receive(&forged).is_err());
        // Signed outer mutation can also reach a MAC-valid but inner-invalid
        // trial. Discard it; the authentic event remains decryptable.
        let mut mismatched = good.clone();
        mismatched.core.author_auth = [9; 32];
        mismatched.sign(&ia)?;
        assert!(receive(&mut gb, &mut sb, &mismatched).is_err());
        assert_eq!(gb.peer.next, 67);
        let mut bad_mac = good.clone();
        let last = bad_mac
            .ciphertext
            .last_mut()
            .ok_or_else(|| anyhow::anyhow!("ciphertext"))?;
        *last ^= 1;
        bad_mac.sign(&ia)?;
        assert!(receive(&mut gb, &mut sb, &bad_mac).is_err());
        assert_eq!(gb.peer.next, 67);
        receive(&mut gb, &mut sb, &good)?;
        assert_eq!(gb.peer.next, 68);
        Ok(())
    }
}
