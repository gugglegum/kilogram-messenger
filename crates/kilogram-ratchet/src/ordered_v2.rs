//! Bounded ordering/counter component. Authentication and native trials belong to the owner.
//! A parsed checkpoint or EventMeta is never evidence of authentication by itself.
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Id = [u8; 32];
pub const MAX_GUARD: usize = 1024;
pub const MAX_CHECKPOINT: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Counter {
    Ordinal,
    Author,
    RootGeneration,
    NativeGeneration,
    LaneGeneration,
    Symmetric,
    Turn,
}
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum Error {
    #[error("CounterExhausted({0:?})")]
    CounterExhausted(Counter),
    #[error("ordered2 input: {0}")]
    Input(&'static str),
    #[error("ordered2 checkpoint: {0}")]
    Checkpoint(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn successor(n: u64, kind: Counter) -> Result<u64> {
    n.checked_add(1).ok_or(Error::CounterExhausted(kind))
}
pub fn consume_index(j: u64) -> Result<u64> {
    successor(j, Counter::Symmetric)
}
pub fn continuation_index(last: u64) -> Result<(u64, u64)> {
    let j = successor(last, Counter::Symmetric)?;
    Ok((j, consume_index(j)?))
}
pub fn turn(role: u8, g: u64) -> Result<u64> {
    if role > 1 {
        return Err(Error::Input("role"));
    }
    g.checked_mul(2)
        .and_then(|v| v.checked_add(u64::from(role)))
        .ok_or(Error::CounterExhausted(Counter::Turn))
}
pub fn dh_step(pre: u64, role: u8, g: u64) -> Result<u64> {
    let post = turn(role, g)?;
    if successor(pre, Counter::Turn)? != post {
        return Err(Error::Input("DH turn/phase"));
    }
    Ok(post)
}
pub fn check_generations(root: u64, native: u64) -> Result<(u64, u64)> {
    Ok((
        successor(root, Counter::RootGeneration)?,
        successor(native, Counter::NativeGeneration)?,
    ))
}
pub fn check_checkpoint_size(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_CHECKPOINT {
        return Err(Error::Checkpoint("combined sealed native/guard cap"));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClosedTail {
    pub g: u64,
    pub key: Id,
    pub index: u64,
    pub last: Id,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    pub g: u64,
    pub index: u64,
    pub parent: Option<Id>,
    pub tail: Option<ClosedTail>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Lane {
    pub next: u64,
    pub last: Option<Id>,
    pub g: u64,
    pub key: Option<Id>,
    pub index: u64,
    pub first: Option<Id>,
    pub parent: Option<Id>,
    pub sequence: Option<u64>,
}
impl Lane {
    fn validate(&self, role: u8) -> Result<()> {
        let empty = self.next == 0;
        if empty != self.last.is_none()
            || empty != self.key.is_none()
            || empty != self.first.is_none()
            || empty != self.sequence.is_none()
            || empty && (self.g != 0 || self.index != 0 || self.parent.is_some())
        {
            return Err(Error::Checkpoint("lane shape"));
        }
        if !empty {
            turn(role, self.g)?;
            consume_index(self.index)?;
            if self.g >= self.next || self.index >= self.next {
                return Err(Error::Checkpoint("lane counters"));
            }
        }
        Ok(())
    }
    pub fn tail(&self) -> Result<ClosedTail> {
        Ok(ClosedTail {
            g: self.g,
            key: self.key.ok_or(Error::Checkpoint("tail key"))?,
            index: self.index,
            last: self.last.ok_or(Error::Checkpoint("tail last"))?,
        })
    }
    fn record(&mut self, event: &EventMeta, id: Id, next: u64) {
        if self.key != Some(event.key) {
            self.first = Some(id);
        }
        self.next = next;
        self.last = Some(id);
        self.g = event.expected.g;
        self.key = Some(event.key);
        self.index = event.index;
        self.parent = event.expected.parent;
        self.sequence = Some(event.sequence);
    }
}
/// Normalized owner input, not a signature/MAC/policy proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventMeta {
    pub role: u8,
    pub n: u64,
    pub previous: Option<Id>,
    pub sequence: u64,
    pub kind: u8,
    pub native_kind: u8,
    pub key: Id,
    pub index: u64,
    pub expected: Expected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seen {
    pub generation: u64,
    pub first: Id,
}
/// Must be obtained from the authenticated index at the same root generation as the checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeenObservation {
    Absent,
    Present(Seen),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Guard2 {
    version: u8,
    role: u8,
    own: Lane,
    peer: Lane,
    rotate_due: bool,
    received_new_first: Option<Id>,
    used_own_first: Option<Id>,
}
/// Private fields bind a single transition to the exact before checkpoint and metadata.
pub struct PreparedStep {
    before: Guard2,
    event: EventMeta,
    receive: bool,
    next: u64,
    pub native_post_index: u64,
    pub native_post_turn: u64,
}
impl Guard2 {
    pub fn new(role: u8) -> Result<Self> {
        if role > 1 {
            return Err(Error::Input("role"));
        }
        Ok(Self {
            version: 2,
            role,
            own: Lane::default(),
            peer: Lane::default(),
            rotate_due: false,
            received_new_first: None,
            used_own_first: None,
        })
    }
    pub fn role(&self) -> u8 {
        self.role
    }
    pub fn own(&self) -> &Lane {
        &self.own
    }
    pub fn peer(&self) -> &Lane {
        &self.peer
    }
    pub fn rotate_due(&self) -> bool {
        self.rotate_due
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 2 || self.role > 1 {
            return Err(Error::Checkpoint("version/role"));
        }
        self.own.validate(self.role)?;
        self.peer.validate(1 - self.role)?;
        let inactive = self.peer.first.is_some() && self.used_own_first == self.own.first;
        if self.rotate_due != inactive || self.peer.first != self.received_new_first {
            return Err(Error::Checkpoint("native Active/Inactive provenance"));
        }
        if self.peer.next != 0 {
            let peer_turn = turn(1 - self.role, self.peer.g)?;
            if self.own.next == 0 {
                if self.role != 1 || peer_turn != 0 || !self.rotate_due {
                    return Err(Error::Checkpoint("inbound genesis phase"));
                }
            } else {
                let own_turn = turn(self.role, self.own.g)?;
                let valid = if self.rotate_due {
                    own_turn.checked_add(1) == Some(peer_turn)
                } else {
                    peer_turn.checked_add(1) == Some(own_turn)
                };
                if !valid {
                    return Err(Error::Checkpoint("two-lane turn provenance"));
                }
            }
        } else if self.own.next != 0 && (self.role != 0 || self.own.g != 0) {
            return Err(Error::Checkpoint("outbound genesis phase"));
        }
        if self.rotate_due
            && (self.peer.first != self.received_new_first
                || self.peer.first.is_none()
                || self.used_own_first != self.own.first)
            || self.received_new_first.is_some() && self.received_new_first != self.peer.first
        {
            return Err(Error::Checkpoint("rotation cause"));
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self).map_err(|_| Error::Checkpoint("codec"))?;
        if bytes.len() > MAX_GUARD {
            return Err(Error::Checkpoint("guard cap"));
        }
        Ok(bytes)
    }
    /// Caller authenticates envelope/scope/provenance before restoring this bounded summary.
    pub fn restore(bytes: &[u8], role: u8) -> Result<Self> {
        if bytes.len() > MAX_GUARD {
            return Err(Error::Checkpoint("guard cap"));
        }
        let (g, rest): (Self, _) =
            postcard::take_from_bytes(bytes).map_err(|_| Error::Checkpoint("codec"))?;
        if !rest.is_empty() || g.role != role {
            return Err(Error::Checkpoint("role/trailing"));
        }
        g.validate()?;
        Ok(g)
    }
    fn current_turn(&self) -> Result<u64> {
        if self.rotate_due {
            turn(1 - self.role, self.peer.g)
        } else if self.own.next == 0 {
            Ok(0)
        } else {
            turn(self.role, self.own.g)
        }
    }
    pub fn expected_send(&self) -> Result<Expected> {
        self.validate()?;
        if self.own.next == 0 {
            return Ok(Expected {
                g: 0,
                index: 0,
                parent: if self.role == 0 {
                    None
                } else {
                    Some(self.peer.first.ok_or(Error::Input("Confirm before Init"))?)
                },
                tail: None,
            });
        }
        if self.rotate_due {
            Ok(Expected {
                g: successor(self.own.g, Counter::LaneGeneration)?,
                index: 0,
                parent: self.received_new_first,
                tail: Some(self.own.tail()?),
            })
        } else {
            Ok(Expected {
                g: self.own.g,
                index: continuation_index(self.own.index)?.0,
                parent: self.own.parent,
                tail: None,
            })
        }
    }
    pub fn preflight_send(
        &self,
        event: &EventMeta,
        root: u64,
        native: u64,
    ) -> Result<PreparedStep> {
        self.preflight(event, false, None, root, native)
    }
    pub fn preflight_receive(
        &self,
        event: &EventMeta,
        seen: SeenObservation,
        root: u64,
        native: u64,
    ) -> Result<PreparedStep> {
        self.preflight(event, true, Some(seen), root, native)
    }
    fn preflight(
        &self,
        event: &EventMeta,
        receive: bool,
        seen: Option<SeenObservation>,
        root: u64,
        native: u64,
    ) -> Result<PreparedStep> {
        self.validate()?;
        check_generations(root, native)?;
        let lane = if receive { &self.peer } else { &self.own };
        let role = if receive { 1 - self.role } else { self.role };
        let next = successor(event.n, Counter::Ordinal)?;
        successor(event.sequence, Counter::Author)?;
        let post_index = consume_index(event.index)?;
        if event.role != role
            || event.n != lane.next
            || event.previous != lane.last
            || lane.sequence.is_some_and(|s| event.sequence <= s)
            || event.index != event.expected.index
            || event.kind != if event.n == 0 { role + 1 } else { 3 }
            || (event.kind == 1) != (event.native_kind == 0)
            || event.native_kind > 1
        {
            return Err(Error::Input("cursor/kind/header/sequence"));
        }
        let new = lane.key != Some(event.key);
        let expected = if !receive {
            self.expected_send()?
        } else if lane.next == 0 {
            Expected {
                g: 0,
                index: 0,
                parent: if role == 0 {
                    None
                } else {
                    Some(self.own.first.ok_or(Error::Input("Confirm before Init"))?)
                },
                tail: None,
            }
        } else if !new {
            Expected {
                g: lane.g,
                index: continuation_index(lane.index)?.0,
                parent: lane.parent,
                tail: None,
            }
        } else {
            if self.rotate_due {
                return Err(Error::Input("new peer DH in Inactive"));
            }
            let first = self
                .own
                .first
                .ok_or(Error::Input("peer DH without own first"))?;
            if self.used_own_first == Some(first) {
                return Err(Error::Input("reused own DH cause"));
            }
            Expected {
                g: successor(lane.g, Counter::LaneGeneration)?,
                index: 0,
                parent: Some(first),
                tail: Some(lane.tail()?),
            }
        };
        if event.expected != expected {
            return Err(Error::Input("expected native/ClosedTail"));
        }
        if receive {
            check_seen(
                lane,
                event.key,
                event.expected.g,
                seen.ok_or(Error::Input("seen observation"))?,
            )?;
        }
        let pre = self.current_turn()?;
        let post_turn = if receive && new {
            if role == 0 && event.n == 0 {
                if pre != 0 || self.own.next != 0 {
                    return Err(Error::Input("inbound constructor"));
                }
                0
            } else {
                dh_step(pre, role, event.expected.g)?
            }
        } else if !receive && (self.rotate_due || self.role == 1 && lane.next == 0) {
            dh_step(pre, role, event.expected.g)?
        } else {
            pre
        };
        Ok(PreparedStep {
            before: self.clone(),
            event: event.clone(),
            receive,
            next,
            native_post_index: post_index,
            native_post_turn: post_turn,
        })
    }
    /// Apply only after the successful native/header/inner trial. Own key is known only now.
    pub fn apply(
        &mut self,
        step: PreparedStep,
        actual: &EventMeta,
        id: Id,
        seen: SeenObservation,
    ) -> Result<()> {
        let mut prepared = step.event.clone();
        if !step.receive {
            prepared.key = actual.key;
        }
        if *self != step.before || prepared != *actual {
            return Err(Error::Input("stale prepared step"));
        }
        if !step.receive
            && self.own.next != 0
            && !self.rotate_due
            && self.own.key != Some(actual.key)
        {
            return Err(Error::Input("own continuation key"));
        }
        let lane = if step.receive { &self.peer } else { &self.own };
        check_seen(lane, actual.key, actual.expected.g, seen)?;
        let mut trial = self.clone();
        if step.receive {
            if self.peer.key != Some(actual.key) {
                trial.rotate_due = true;
                trial.received_new_first = Some(id);
                trial.used_own_first = self.own.first;
            }
            trial.peer.record(actual, id, step.next);
        } else {
            trial.own.record(actual, id, step.next);
            trial.rotate_due = false;
        }
        trial.validate()?;
        *self = trial;
        Ok(())
    }
}
fn check_seen(lane: &Lane, key: Id, g: u64, seen: SeenObservation) -> Result<()> {
    match (lane.key == Some(key), seen) {
        (true, SeenObservation::Present(s)) if s.generation == g && Some(s.first) == lane.first => {
            Ok(())
        }
        (false, SeenObservation::Absent) => Ok(()),
        _ => Err(Error::Input("seen replay/missing/first mismatch")),
    }
}

pub struct ImportedL1 {
    pub guard: Guard2,
    pub seen: Vec<(u8, Id, Seen)>,
}
/// Public OD1 summary fields, copied only after owner-side Head/AEAD/origin checks.
/// This DTO carries no native checkpoint and confers no authentication by itself.
pub struct L1CheckpointFacts {
    pub role: u8,
    pub own: Lane,
    pub peer: Lane,
    pub rotate_due: bool,
    pub received_new_first: Option<Id>,
    pub used_own_first: Option<Id>,
}
pub fn compact_l1_checkpoint(facts: L1CheckpointFacts) -> Result<Vec<u8>> {
    for lane in [&facts.own, &facts.peer] {
        if lane.next > 512 || lane.g >= 512 || lane.index >= 512 {
            return Err(Error::Checkpoint("L1 summary bounds"));
        }
    }
    Guard2 {
        version: 2,
        role: facts.role,
        own: facts.own,
        peer: facts.peer,
        rotate_due: facts.rotate_due,
        received_new_first: facts.received_new_first,
        used_own_first: facts.used_own_first,
    }
    .encode()
}
/// Import-only bounded fold. Owner must first authenticate OD1 Head/AEAD/session config/SID,
/// exact legacy sources/signatures/headers and constructor origin. This function proves no crypto.
pub fn import_l1(
    role: u8,
    steps: &[(EventMeta, Id)],
    expected_checkpoint: &[u8],
) -> Result<ImportedL1> {
    if steps.len() > 1024 {
        return Err(Error::Checkpoint("L1 source count"));
    }
    let mut g = Guard2::new(role)?;
    let mut seen = std::collections::BTreeMap::<(u8, Id), Seen>::new();
    for (meta, id) in steps {
        if meta.n >= 512 || meta.expected.g >= 512 || meta.index >= 512 {
            return Err(Error::Checkpoint("L1 bounds"));
        }
        let observation = seen
            .get(&(meta.role, meta.key))
            .copied()
            .map_or(SeenObservation::Absent, SeenObservation::Present);
        let step = if meta.role == role {
            g.preflight_send(meta, 0, 0)?
        } else {
            g.preflight_receive(meta, observation, 0, 0)?
        };
        g.apply(step, meta, *id, observation)?;
        seen.entry((meta.role, meta.key)).or_insert(Seen {
            generation: meta.expected.g,
            first: *id,
        });
    }
    let expected = Guard2::restore(expected_checkpoint, role)?;
    if expected != g {
        return Err(Error::Checkpoint(
            "L1 accepted checkpoint/source correspondence",
        ));
    }
    Ok(ImportedL1 {
        guard: g,
        seen: seen.into_iter().map(|((r, k), v)| (r, k, v)).collect(),
    })
}

#[cfg(test)]
#[path = "ordered_v2/tests.rs"]
mod tests;
