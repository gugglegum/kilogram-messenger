//! Durable possession admission and bounded, typed ready-prefix drain.
use super::ratchet::Failure as GuardFailure;
use super::{
    admin,
    engine::{Changes, Engine, event_path, ordinal_path, pending_path, prefix},
    handshake::Credentials,
    outbox::receipt_path,
};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Id, a, b, u};
use kilogram_ratchet::ordered::{NativeFailure, Packet};
use std::collections::{BTreeMap, BTreeSet};

const ITEM: usize = 64 * 1024;
pub fn pending_state(c: Id, n: u64) -> String {
    format!("{}pending-state/{n:020}", prefix(c))
}
pub fn ack_intent(c: Id, lane: u64, n: u64) -> String {
    format!("{}ack-intent/{lane}/{n:020}", prefix(c))
}

pub struct Batch {
    pub ids: Vec<Id>,
    pub error: Option<String>,
    pub readable: usize,
    pub invalid: bool,
}
pub struct Drain {
    pub readable: usize,
    pub invalid: bool,
}
enum TrialFailure {
    Input(&'static str),
    Operational(anyhow::Error),
}

impl Engine {
    pub fn pending_stats(&self, c: Id) -> Result<(usize, usize, bool)> {
        let mut slots = 0;
        let mut bytes = 0;
        let mut head = false;
        for path in self.store.paths(&format!("{}pending-state/", prefix(c))) {
            let raw = admin::required(&self.store, &path)?;
            let value = wire::decode(&raw, 2048)?;
            let f = value.fields(8)?;
            ensure!(
                f[0].uint()? == 1 && f[2].uint()? <= 1,
                "pending checkpoint shape"
            );
            let charge = usize::try_from(f[7].uint()?)?;
            ensure!(charge <= ITEM, "pending item byte cap");
            if f[2].uint()? == 1 {
                ensure!(!head, "multiple head reservations");
                head = true;
            } else {
                slots += 1;
                bytes += charge;
            }
        }
        ensure!(
            slots <= 64 && bytes <= 64 * ITEM,
            "pending checkpoint budget"
        );
        Ok((slots, bytes, head))
    }
    pub fn receive_batch(
        &mut self,
        c: Id,
        raw_events: &[Vec<u8>],
        auth_records: &BTreeMap<Id, Vec<u8>>,
        binding: Id,
    ) -> Result<Batch> {
        self.check_lease(c, binding)?;
        ensure!(self.active(c)?, "NeedHandshake");
        ensure!(
            !raw_events.is_empty() && raw_events.len() <= 32 && auth_records.len() <= 32,
            "batch count cap"
        );
        for (id, raw) in auth_records {
            ensure!(wire::hash(b"", raw) == *id, "auth catalog hash");
            self.credential(raw)?;
        }
        let mut batch = Batch {
            ids: Vec::new(),
            error: None,
            readable: 0,
            invalid: false,
        };
        for raw in raw_events {
            match self.admit(c, raw, auth_records, binding) {
                Ok(id) => batch.ids.push(id),
                Err(error) => {
                    batch.invalid = error.downcast_ref::<wire::Error>().is_some();
                    batch.error = Some(error.to_string());
                    break;
                }
            }
        }
        let drained = self.drain(c, binding)?;
        batch.readable = drained.readable;
        batch.invalid |= drained.invalid;
        if batch.invalid {
            batch.error = Some("InvalidPrefix".to_owned());
        }
        Ok(batch)
    }
    fn peer_current(&self, c: Id) -> Result<Credentials> {
        let role = self.role(c)?;
        let cert = self.spec(c)?.peers[1 - role as usize].cert.clone();
        let snapshot = admin::required(
            &self.store,
            &format!("peer-authority/{}.snapshot", cert.account_id()),
        )?;
        self.credential(&wire::encode(
            &a([u(1), b(cert.encode()?), b(snapshot)]),
            wire::MAX_AUTH,
        )?)
    }
    fn admit(
        &mut self,
        c: Id,
        raw: &[u8],
        auth_records: &BTreeMap<Id, Vec<u8>>,
        binding: Id,
    ) -> Result<Id> {
        self.check_lease(c, binding)?;
        let mut head = self.head(c)?;
        let peer = 1 - head.role;
        let event = Event::parse(raw, self.spec(c)?.peers[peer as usize].cert.device_id())?;
        self.check_scope(&head, &event, true)?;
        ensure!(
            event.core.role == peer && event.core.kind == 3,
            "data lane/kind"
        );
        let id = event.id()?;
        let n = event.core.n;
        if let Some(existing) = self.store.get(&ordinal_path(c, peer, n))? {
            if existing != id {
                let old_id: Id = existing.as_slice().try_into()?;
                let old = self.find_event_raw(old_id)?;
                return self.signed_fork(c, "ordinal", &old, raw).and(Ok(id));
            }
            return Ok(id);
        }
        if let Some(existing) = self.store.get(&pending_path(c, n))? {
            if existing != raw {
                return self
                    .signed_fork(c, "pending ordinal", &existing, raw)
                    .and(Ok(id));
            }
            return Ok(id);
        }
        let native = self.native(&head)?;
        let next = native.guard.as_ref().context("guard")?.peer.next;
        ensure!(n >= next, "local committed prefix/index inconsistency");
        let auth = match auth_records.get(&event.core.author_auth) {
            Some(v) => v.clone(),
            None => self
                .store
                .get(&admin::auth_path(event.core.author_auth))?
                .ok_or(wire::Error::Invalid("original credential missing"))?,
        };
        let author = self.credential(&auth)?;
        ensure!(
            author.reference() == event.core.author_auth
                && author.cert_ref()? == self.spec(c)?.peers[peer as usize].cert_ref()?,
            "event original authority binding"
        );
        let current = self.peer_current(c)?;
        let (slots, bytes, head_used) = self.pending_stats(c)?;
        let charge = raw
            .len()
            .checked_add(auth.len())
            .and_then(|v| v.checked_add(current.raw.len()))
            .and_then(|v| v.checked_add(1024))
            .context("pending charge overflow")?;
        ensure!(
            charge <= ITEM,
            "UpgradeRequired: full stored admission item"
        );
        let slot = if n == next && !head_used {
            1
        } else {
            ensure!(
                slots < 64 && bytes + charge <= 64 * ITEM,
                "StorageBlocked: ordinary pending; head reserve preserved"
            );
            0
        };
        let signer = self.spec(c)?.peers[peer as usize].cert.device_id();
        let seq_path = format!(
            "runtime/od1/authored/{}/{:020}",
            signer, event.core.sequence
        );
        if let Some(existing) = self.store.get(&seq_path)?
            && existing != id
        {
            let old_id: Id = existing.as_slice().try_into()?;
            let old = self.find_event_raw(old_id)?;
            return self
                .signed_fork(c, "author sequence", &old, raw)
                .and(Ok(id));
        }
        let mut changes = Changes::default();
        let original = self.add_auth(&mut changes, &auth)?;
        let pinned = self.add_auth(&mut changes, &current.raw)?;
        changes.put(seq_path, id.to_vec());
        changes.put(pending_path(c, n), raw.to_vec());
        changes.put(
            pending_state(c, n),
            wire::encode(
                &a([
                    u(1),
                    b(id),
                    u(slot),
                    u(0),
                    b(original),
                    b(pinned),
                    b([]),
                    u(charge as u64),
                ]),
                2048,
            )?,
        );
        head.bump(false)?;
        self.check_lease(c, binding)?;
        self.save(&mut head, None, changes)?;
        super::test_support::checkpoint("after-pending")?;
        Ok(id)
    }
    /// Returns one committed batch of at most eight. A caller schedules another
    /// fair actor turn if a further head exists; it never waits for a new packet.
    pub fn drain(&mut self, c: Id, binding: Id) -> Result<Drain> {
        self.check_lease(c, binding)?;
        ensure!(self.active(c)?, "NeedHandshake");
        let mut head = self.head(c)?;
        let mut working = self.native(&head)?;
        let mut changes = Changes::default();
        let mut done = 0;
        let mut invalid = false;
        let mut known = BTreeSet::new();
        for _ in 0..8 {
            let next = working.guard.as_ref().context("guard")?.peer.next;
            let Some(raw) = self.store.get(&pending_path(c, next))? else {
                break;
            };
            let state_raw = admin::required(&self.store, &pending_state(c, next))?;
            let state = wire::decode(&state_raw, 2048)?;
            let fields = state.fields(8)?;
            if fields[3].uint()? == 1 {
                invalid = true;
                break;
            }
            let event = Event::parse(
                &raw,
                self.spec(c)?.peers[1 - head.role as usize].cert.device_id(),
            )
            .context("corrupt accepted outer record")?;
            self.check_scope(&head, &event, true)
                .context("accepted scope checkpoint")?;
            ensure!(
                event.id()? == fields[1].fixed::<32>()?,
                "accepted pending ID mismatch"
            );
            let mut parents_valid = true;
            for parent in &event.core.parents {
                if !known.contains(parent) {
                    if let Some(raw) = self.store.get(&event_path(c, *parent))? {
                        let value = wire::decode(&raw, wire::MAX_EVENT)?;
                        let core = wire::Core::parse(&value.fields(5)?[1])?;
                        parents_valid &= core.kind == 3 && core.c == c && core.e == event.core.e;
                    } else {
                        parents_valid = false;
                    }
                }
            }
            let mut trial = working.fork();
            let result = (|| -> std::result::Result<String, TrialFailure> {
                if !parents_valid {
                    return Err(TrialFailure::Input("unknown causal parent"));
                }
                trial
                    .guard
                    .as_ref()
                    .ok_or_else(|| TrialFailure::Operational(anyhow::anyhow!("guard absent")))?
                    .check_receive(&event)
                    .map_err(|e| match e {
                        GuardFailure::Input(reason) => TrialFailure::Input(reason),
                        other => TrialFailure::Operational(other.into()),
                    })?;
                let session = trial
                    .session
                    .as_mut()
                    .ok_or_else(|| TrialFailure::Operational(anyhow::anyhow!("session absent")))?;
                let plain = session
                    .decrypt(&Packet {
                        kind: 1,
                        bytes: event.ciphertext.clone(),
                    })
                    .map_err(|e| match e {
                        NativeFailure::Input(reason) => TrialFailure::Input(reason),
                        other => TrialFailure::Operational(other.into()),
                    })?;
                let payload = event
                    .verify_inner(&plain)
                    .map_err(|_| TrialFailure::Input("inner/outer binding"))?;
                let text = payload
                    .fields(1)
                    .and_then(|v| v[0].text())
                    .map_err(|_| TrialFailure::Input("Text payload encoding"))?;
                if text.len() > 4096 {
                    return Err(TrialFailure::Input("Text body cap"));
                }
                Ok(text.to_owned())
            })();
            match result {
                Ok(text) => {
                    trial
                        .guard
                        .as_mut()
                        .context("guard")?
                        .commit_receive(&event, event.id()?)?;
                    self.record_event(&event, &mut changes)?;
                    self.project(&event, &text, &mut changes)?;
                    // Admission authority + final authority + receipt authority are
                    // all captured. Their byte bounds are part of the frame charge.
                    self.add_auth(&mut changes, &self.peer_current(c)?.raw)?;
                    let receipt = self.make_receipt(&event, &mut changes)?;
                    changes.put(receipt_path(c, event.core.role, next), receipt.clone());
                    changes.put(
                        ack_intent(c, event.core.role, next),
                        wire::encode(
                            &a([u(1), b(wire::hash(wire::RECEIPT_ID, &receipt)), u(0)]),
                            1024,
                        )?,
                    );
                    changes.remove(pending_path(c, next));
                    changes.remove(pending_state(c, next));
                    known.insert(event.id()?);
                    working = trial;
                    done += 1;
                }
                Err(TrialFailure::Input(reason)) => {
                    let mut value = state.list()?.to_vec();
                    value[3] = u(1);
                    value[6] = b(reason.as_bytes());
                    changes.put(pending_state(c, next), wire::encode(&a(value), 2048)?);
                    invalid = true;
                    break;
                }
                Err(TrialFailure::Operational(error)) => {
                    return Err(error.context("OD1 drain rolled back entire current workspace"));
                }
            }
        }
        if done > 0 || invalid {
            // An existing quarantine is a stable blocked head, not a repeated write.
            if done == 0 && invalid {
                let n = working.guard.as_ref().context("guard")?.peer.next;
                let state =
                    wire::decode(&admin::required(&self.store, &pending_state(c, n))?, 2048)?;
                if state.fields(8)?[3].uint()? == 1 {
                    return Ok(Drain {
                        readable: 0,
                        invalid: true,
                    });
                }
            }
            head.bump(done > 0)?;
            self.check_lease(c, binding)?;
            if done > 0 {
                super::test_support::checkpoint("before-readable-commit")?;
            }
            self.save(
                &mut head,
                if done > 0 { Some(&working) } else { None },
                changes,
            )?;
            if done > 0 {
                super::test_support::checkpoint("after-readable-commit")?;
            }
        }
        Ok(Drain {
            readable: done,
            invalid,
        })
    }
}
