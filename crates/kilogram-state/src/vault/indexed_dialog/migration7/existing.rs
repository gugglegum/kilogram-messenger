//! Closed existing-control purposes over the same Ready Root7. Signatures and
//! Native semantics belong to the owner; this layer independently binds every
//! structural field, stage, source frontier, CAS and paid retained record.
use super::*;

const STATE_CAP: usize = 24 * 1024;
const CONTROL_CAP: usize = 32 * 1024;
const CONTROL_ID: &[u8] = b"kg:od2:control-id\0";
const CONSENT_ID: &[u8] = b"kg:od2:consent\0";
const MODE_ID: &[u8] = b"kg:od2:mode-certificate\0";
pub const MIXED_NATIVE_ID: &[u8] = b"kg:od2:migrated-native-state\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExistingMode {
    #[default]
    L1,
    Offering,
    Prepared,
    Selected,
    Active,
}
impl ExistingMode {
    pub fn wire(self) -> u64 {
        match self {
            Self::L1 => 1,
            Self::Offering => 2,
            Self::Prepared => 3,
            Self::Selected => 4,
            Self::Active => 5,
        }
    }
    pub fn selected(self) -> bool {
        matches!(self, Self::Selected | Self::Active)
    }
    pub fn frozen(self) -> bool {
        matches!(self, Self::Offering | Self::Prepared | Self::Selected)
    }
    pub fn allows_l1_transport(self) -> bool {
        self == Self::L1
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpgradeIntent7 {
    pub request: [u8; 16],
    pub consent: Id,
    pub body_digest: Id,
    pub cancelled: bool,
    pub notification_sent_unconfirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerUpgradeIntent7 {
    pub request: [u8; 16],
    pub body_digest: Id,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExistingState7 {
    pub mode: ExistingMode,
    pub current_consent: Option<Id>,
    pub local_intent: Option<UpgradeIntent7>,
    pub local_outcomes: Vec<UpgradeIntent7>,
    pub peer_intent: Option<PeerUpgradeIntent7>,
    pub selected_consent: Option<Id>,
    pub propose: Option<Id>,
    pub accept: Option<Id>,
    pub select: Option<Id>,
    pub committed: Option<Id>,
    pub q: Option<Id>,
    pub cuts: [Option<u64>; 2],
    pub previous: [Option<Id>; 2],
    pub peer_q_hint: Option<Id>,
    pub control_debt: bool,
    pub locator_revision: [u64; 2],
}
impl ExistingState7 {
    pub fn triggered(&self) -> bool {
        self.local_intent.as_ref().is_some_and(|i| !i.cancelled) || self.peer_intent.is_some()
    }
    fn check(&self, role: u8) -> Result<()> {
        let mut requests = std::collections::BTreeSet::new();
        if self.local_outcomes.len() > 128
            || self
                .local_outcomes
                .iter()
                .any(|i| !requests.insert(i.request))
            || self.local_outcomes.iter().filter(|i| !i.cancelled).count() > 1
            || self
                .local_intent
                .as_ref()
                .is_some_and(|i| !self.local_outcomes.contains(i))
            || self
                .local_outcomes
                .iter()
                .any(|i| !i.cancelled && self.local_intent.as_ref() != Some(i))
        {
            return Err(Error::Integrity("bounded local upgrade outcomes"));
        }
        let cut = |i: usize| {
            self.cuts[i].is_some_and(|n| (1..=512).contains(&n)) && self.previous[i].is_some()
        };
        let shape = match self.mode {
            ExistingMode::L1 => {
                self.q.is_none()
                    && self.cuts == [None, None]
                    && self.previous == [None, None]
                    && self.propose.is_none()
                    && self.accept.is_none()
                    && self.select.is_none()
                    && self.committed.is_none()
                    && self.selected_consent.is_none()
            }
            ExistingMode::Offering => {
                role == 0
                    && cut(0)
                    && self.cuts[1].is_none()
                    && self.previous[1].is_none()
                    && self.q.is_none()
                    && self.propose.is_some()
                    && self.accept.is_none()
                    && self.select.is_none()
                    && self.committed.is_none()
            }
            ExistingMode::Prepared => {
                role == 1
                    && cut(0)
                    && cut(1)
                    && self.q.is_none()
                    && self.propose.is_some()
                    && self.accept.is_some()
                    && self.select.is_none()
                    && self.committed.is_none()
            }
            ExistingMode::Selected => {
                role == 0
                    && cut(0)
                    && cut(1)
                    && self.q.is_some()
                    && self.propose.is_some()
                    && self.accept.is_some()
                    && self.select.is_some()
                    && self.committed.is_none()
            }
            ExistingMode::Active => {
                cut(0)
                    && cut(1)
                    && self.q.is_some()
                    && self.propose.is_some()
                    && self.accept.is_some()
                    && self.select.is_some()
                    && self.committed.is_some()
            }
        };
        if !shape
            || self.peer_q_hint.is_some()
                && (role != 1
                    || !matches!(self.mode, ExistingMode::Prepared | ExistingMode::Active)
                    || self.mode == ExistingMode::Active && self.peer_q_hint != self.q)
            || self.mode != ExistingMode::L1 && self.selected_consent.is_none()
            || role != 0 && self.peer_intent.is_some()
            || self.mode == ExistingMode::L1 && self.control_debt
            || matches!(
                self.mode,
                ExistingMode::Offering | ExistingMode::Prepared | ExistingMode::Selected
            ) && !self.control_debt
            || self.mode == ExistingMode::Active && role == 0 && self.control_debt
        {
            return Err(Error::Integrity("existing stage/role/optional shape"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
pub struct ExistingNative7 {
    pub head: Vec<u8>,
    pub native: Vec<u8>,
    pub seen: Vec<(u8, Id, u64, Id)>,
}
#[derive(Clone, Serialize)]
pub enum ExistingAction7 {
    Consent {
        expected: Option<Id>,
        raw: Vec<u8>,
        historical: bool,
    },
    Intent {
        request: [u8; 16],
    },
    Cancel {
        request: [u8; 16],
    },
    NotificationSent {
        request: [u8; 16],
    },
    PeerIntent {
        request: [u8; 16],
        body: Vec<u8>,
    },
    Propose {
        raw: Vec<u8>,
        consent: Vec<u8>,
    },
    Prepare {
        propose: Vec<u8>,
        accept: Vec<u8>,
        consent: Vec<u8>,
    },
    Select {
        proof: Vec<u8>,
        converted: ExistingNative7,
    },
    CommitB {
        proof: Vec<u8>,
        converted: ExistingNative7,
        committed: Vec<u8>,
    },
    Activate {
        committed: Vec<u8>,
    },
    Locator {
        local: bool,
        revision: u64,
        raw: Vec<u8>,
    },
    PeerSelected {
        q: Id,
        peer_stage: u8,
    },
    PeerHint {
        q: Id,
        cuts: [Option<u64>; 2],
        stage: u8,
    },
    Data(ExistingData7),
    Block {
        previous: Vec<u8>,
        conflict: Vec<u8>,
    },
}
#[derive(Clone, Serialize)]
pub struct ExistingOperation7 {
    pub context: u8,
    pub expected_root: u64,
    pub expected_head: Id,
    pub policy_generation: u64,
    pub wall_time: u64,
    pub action: ExistingAction7,
}
impl ExistingOperation7 {
    fn bound(&self) -> Result<()> {
        if self.context > 1 {
            return Err(Error::Bound("existing occupied slot"));
        }
        if let ExistingAction7::Data(data) = &self.action {
            data.bound(self.context)?;
        }
        let bytes =
            postcard::to_allocvec(self).map_err(|_| Error::Bound("existing operation codec"))?;
        if bytes.len() > 512 * 1024 {
            return Err(Error::Bound("existing operation bytes"));
        }
        if let ExistingAction7::Select { converted, .. }
        | ExistingAction7::CommitB { converted, .. } = &self.action
            && (converted.native.len() > 128 * 1024
                || converted.head.len() > 4096
                || converted.seen.len() > 1024)
        {
            return Err(Error::Bound("existing conversion bytes/seen"));
        }
        Ok(())
    }
    fn live(&self) -> bool {
        !matches!(
            self.action,
            ExistingAction7::Consent { .. }
                | ExistingAction7::Intent { .. }
                | ExistingAction7::Cancel { .. }
        )
    }
}
fn state_key(c: u8) -> Result<Key> {
    key(c, b"existing")
}
fn control_key(c: u8, id: Id) -> Result<Key> {
    Key::hash(c, Namespace::Control, id)
}
fn control(raw: &[u8], kind: u64) -> Result<(Id, Value)> {
    let v = codec::decode(raw, CONTROL_CAP)?;
    let f = v.fields(4)?;
    let n = match kind {
        18 => 8,
        19 => 4,
        20 => 3,
        21 => 2,
        _ => return Err(Error::Bound("existing control purpose")),
    };
    if f[0].uint()? != 2 || f[1].uint()? != kind || f[3].bytes()?.len() != 64 {
        return Err(Error::Integrity("existing signed control structure"));
    }
    f[2].fields(n)?;
    Ok((hash(CONTROL_ID, raw), f[2].clone()))
}

impl L1Repository {
    /// The same bounded debug-only backend fault hook used by offline migration.
    #[cfg(any(test, debug_assertions))]
    pub fn verification_existing_io_fault(&self, point: &str, after: u64) -> Result<()> {
        let point = match point {
            "before_write" => "before_write",
            "after_write" => "after_write",
            "before_sync" => "before_sync",
            "after_sync" => "after_sync",
            "storage_full" => "storage_full",
            _ => return Err(Error::Bound("existing fault point")),
        };
        self.core.backend.fail(point, after)
    }
    /// Read-only cut preflight. The writer repeats it under the commit CAS.
    pub fn check_existing_cuts(&mut self, c: u8, cuts: [u64; 2], previous: [Id; 2]) -> Result<()> {
        self.occupied(c)?;
        let tx = self.core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.core.crypto, self.core.root.clone());
        for r in 0..2 {
            w.existing_cut7(c, r, cuts[r], previous[r])?;
        }
        Ok(())
    }
    pub fn existing_state(&mut self, c: u8) -> Result<ExistingState7> {
        self.occupied(c)?;
        let state = self
            .core
            .get(state_key(c)?)?
            .map(|raw| unpack(&raw))
            .transpose()?
            .unwrap_or_default();
        let state: ExistingState7 = state;
        state.check(self.profile.contexts[usize::from(c)].1)?;
        Ok(state)
    }
    pub fn existing_control(&mut self, c: u8, id: Id) -> Result<Vec<u8>> {
        self.occupied(c)?;
        self.core
            .get(control_key(c, id)?)?
            .ok_or(Error::Integrity("existing immutable control absent"))
    }
    pub fn existing_locator(&mut self, c: u8, local: bool) -> Result<Option<Vec<u8>>> {
        self.occupied(c)?;
        self.core.get(key(
            c,
            if local {
                b"locator2-own"
            } else {
                b"locator2-peer"
            },
        )?)
    }
    pub fn commit_existing(
        &mut self,
        operation: ExistingOperation7,
        mut gate: Option<&mut dyn FnMut() -> Result<()>>,
    ) -> Result<CommitReport> {
        if self.core.backend.stats()?.physical_quota != self.config.physical_capacity {
            return Err(Error::Bound("CapacityCASRequiredBeforeWriter"));
        }
        operation.bound()?;
        if operation.live() && gate.is_none() {
            return Err(Error::PolicyRejected("existing requires final live gate"));
        }
        if let Some(check) = gate.as_mut() {
            check()?;
        }
        let raw = Zeroizing::new(
            postcard::to_allocvec(&operation).map_err(|_| Error::Bound("existing codec"))?,
        );
        let digest = self.core.crypto.hash(0, b"existing-commit/v7", &raw);
        let mut id = digest;
        id[0] |= 1;
        self.core.commit_with_gate(
            operation.expected_root,
            id,
            digest,
            CommitInput::Existing7(Box::new(operation)),
            raw.len() * 3 + 16384,
            gate,
        )
    }
}

impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(in crate::vault::indexed_dialog) fn existing_state7(
        &mut self,
        c: u8,
    ) -> Result<ExistingState7> {
        let state: ExistingState7 = if let Some(raw) = self.get(state_key(c)?)? {
            if raw.len() > STATE_CAP {
                return Err(Error::Bound("existing paid state cap"));
            }
            self.migration_leaf_check(state_key(c)?, &raw, (STATE_CAP - raw.len()) as u64)?;
            unpack(&raw)?
        } else {
            ExistingState7::default()
        };
        let p = self.migration_profile()?;
        state.check(
            p.contexts
                .get(usize::from(c))
                .ok_or(Error::Bound("existing slot"))?
                .1,
        )?;
        Ok(state)
    }
    fn existing_store(&mut self, c: u8, id: Id, raw: &[u8], cap: usize) -> Result<()> {
        if raw.len() > cap {
            return Err(Error::Bound("existing immutable record cap"));
        }
        self.put(control_key(c, id)?, raw, 0, false)
    }
    fn existing_consent(&mut self, c: u8, raw: &[u8], wall: Option<u64>) -> Result<Id> {
        let v = codec::decode(raw, 4096)?;
        let f = v.fields(3)?;
        let b = f[0].fields(10)?;
        let origin = b[6].fields(4)?;
        let p = self.migration_profile()?;
        let head_raw = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("existing Head"))?;
        let head = codec::read_record(&head_raw, 10, 8192)?;
        let inner = codec::decode(head.fields(9)?[4].bytes()?, 4096)?;
        let h = inner.fields(13)?;
        let certs = self.l1_cert_refs(&p, c)?;
        if b[0].uint()? != 2
            || b[1].fixed::<32>()? != p.contexts[usize::from(c)].0
            || b[2].fixed::<32>()? != certs[0]
            || b[3].fixed::<32>()? != certs[1]
            || b[4].uint()? != 1
            || b[5].uint()? != 2
            || b[7].uint()? != 1
            || b[8].uint()? >= b[9].uint()?
            || origin[0].uint()? != 1
            || Some(origin[1].fixed::<32>()?) != h[9].optional()?.map(Value::fixed).transpose()?
            || Some(origin[2].fixed::<32>()?) != h[10].optional()?.map(Value::fixed).transpose()?
            || Some(origin[3].bytes()?) != h[11].optional()?.map(Value::bytes).transpose()?
            || f[1].bytes()?.len() != 64
            || f[2].bytes()?.len() != 64
        {
            return Err(Error::Integrity("existing Consent2 exact scope"));
        }
        if wall.is_some_and(|now| {
            b[8].uint().map_or(true, |n| n > now) || b[9].uint().map_or(true, |n| now >= n)
        }) {
            return Err(Error::PolicyRejected("existing Consent2 interval"));
        }
        Ok(hash(CONSENT_ID, raw))
    }
    fn existing_ready7(&mut self, c: u8) -> Result<()> {
        let p = self.migration_profile()?;
        let raw = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("existing Head"))?;
        let wrapper = codec::read_record(&raw, 10, 8192)?;
        let inner = codec::decode(wrapper.fields(9)?[4].bytes()?, 4096)?;
        let h = inner.fields(13)?;
        if h[5].uint()? != 3
            || h[11].optional()?.is_none()
            || self.l1_meta(c)?.next.contains(&0)
            || self.l1_control(&p, c, L1Control::Ready(0))?.is_none()
            || self.l1_control(&p, c, L1Control::Ready(1))?.is_none()
        {
            return Err(Error::PolicyRejected("NeedHandshake"));
        }
        Ok(())
    }
    pub(in crate::vault::indexed_dialog) fn existing_check7(
        &mut self,
        op: &ExistingOperation7,
        duplicate: bool,
    ) -> Result<()> {
        op.bound()?;
        let p = self.migration_profile()?;
        if self.migration_progress(p.mid)?.stage != super::super::MigrationStage::Ready
            || usize::from(op.context) >= p.contexts.len()
        {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        self.existing_ready7(op.context)?;
        let state = self.existing_state7(op.context)?;
        if !duplicate {
            let raw = self
                .get(head_key(op.context)?)?
                .ok_or(Error::Integrity("existing Head"))?;
            let value = codec::read_record(&raw, 10, 8192)?;
            let meta = self.l1_meta(op.context)?;
            if hash(b"", value.fields(9)?[4].bytes()?) != op.expected_head
                || meta.policy_generation != op.policy_generation
            {
                return Err(Error::StaleGeneration);
            }
            if op.wall_time < meta.wall_high_water {
                return Err(Error::PolicyRejected("clock rollback"));
            }
            if !matches!(
                op.action,
                ExistingAction7::Consent { .. } | ExistingAction7::Cancel { .. }
            ) {
                if self
                    .l1_control(&p, op.context, L1Control::BlockedFork)?
                    .is_some()
                {
                    return Err(Error::PolicyRejected("signed fork blocked"));
                }
                let current = state
                    .current_consent
                    .ok_or(Error::PolicyRejected("existing Consent2 absent"))?;
                let raw = self
                    .get(control_key(op.context, current)?)?
                    .ok_or(Error::Integrity("current Consent2 bytes"))?;
                self.existing_consent(op.context, &raw, Some(op.wall_time))?;
            }
        }
        Ok(())
    }
    fn existing_cut7(&mut self, c: u8, role: usize, cut: u64, previous: Id) -> Result<()> {
        if !(1..=512).contains(&cut) || role > 1 {
            return Err(Error::Bound("existing cut"));
        }
        let p = self.migration_profile()?;
        let meta = self.l1_meta(c)?;
        let own = usize::from(p.contexts[usize::from(c)].1);
        if role == own {
            if meta.next[role] != cut || meta.tails[role] != Some(previous) {
                return Err(Error::Conflict);
            }
        } else {
            if cut < meta.next[role]
                || cut < meta.peer_source_next
                || cut == meta.next[role] && meta.tails[role] != Some(previous)
                || cut == meta.peer_source_next
                    && meta.peer_source_last.is_some_and(|id| id != previous)
            {
                return Err(Error::Conflict);
            }
            let slot = usize::from(c) * 24 + Namespace::Pending as usize;
            let rows = self
                .tree
                .range(slot as u8, self.root.roots[slot], [0; 32], 66)?;
            if rows.len() as u64 != self.root.roots[slot].count {
                return Err(Error::Integrity("pending cut count"));
            }
            for row in rows {
                let raw = self
                    .get(Key {
                        context: c,
                        namespace: Namespace::Pending,
                        bytes: row.prefix,
                    })?
                    .ok_or(Error::Integrity("pending cut row"))?;
                if codec::read_record(&raw, 13, 32 * 1024)?.fields(10)?[5].uint()? >= cut {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }
    fn existing_p7(&mut self, c: u8, raw: &[u8], consent: &[u8]) -> Result<(Id, u64, Id)> {
        let (id, body) = control(raw, 18)?;
        let b = body.fields(8)?;
        let p = self.migration_profile()?;
        let h = self.get(head_key(c)?)?.ok_or(Error::Integrity("P Head"))?;
        let v = codec::read_record(&h, 10, 8192)?;
        let h = codec::decode(v.fields(9)?[4].bytes()?, 4096)?;
        let h = h.fields(13)?;
        let consent_id = self.existing_consent(c, consent, None)?;
        if b[0].fixed::<32>()? != p.contexts[usize::from(c)].0
            || Some(b[1].fixed::<32>()?) != h[9].optional()?.map(Value::fixed).transpose()?
            || Some(b[2].fixed::<32>()?) != h[10].optional()?.map(Value::fixed).transpose()?
            || Some(b[3].bytes()?) != h[11].optional()?.map(Value::bytes).transpose()?
            || b[4].fixed::<32>()? != consent_id
        {
            return Err(Error::Integrity("Propose exact scope/consent"));
        }
        b[5].fixed::<32>()?;
        let cut = b[6].uint()?;
        let prev = b[7].fixed()?;
        self.existing_cut7(c, 0, cut, prev)?;
        Ok((id, cut, prev))
    }
    fn existing_a7(&mut self, c: u8, raw: &[u8], p_id: Id) -> Result<(Id, u64, Id)> {
        let (id, body) = control(raw, 19)?;
        let b = body.fields(4)?;
        let p = self.migration_profile()?;
        if b[0].fixed::<32>()? != p.contexts[usize::from(c)].0 || b[1].fixed::<32>()? != p_id {
            return Err(Error::Conflict);
        }
        let cut = b[2].uint()?;
        let prev = b[3].fixed()?;
        self.existing_cut7(c, 1, cut, prev)?;
        Ok((id, cut, prev))
    }
    fn existing_convert7(&mut self, c: u8, value: &ExistingNative7) -> Result<()> {
        let p = self.migration_profile()?;
        let wrapper = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("conversion Head"))?;
        let v = codec::read_record(&wrapper, 10, 8192)?;
        let old = super::ops::own_head(v.fields(9)?[4].bytes()?)?;
        let next = super::ops::own_head(&value.head)?;
        let o = old.fields(13)?;
        let n = next.fields(13)?;
        for i in [0, 1, 2, 3, 5, 7, 8, 9, 10, 11] {
            if o[i] != n[i] {
                return Err(Error::Integrity("same-Native immutable Head fields"));
            }
        }
        if n[4].uint()?
            != o[4]
                .uint()?
                .checked_add(1)
                .ok_or(Error::Bound("Head generation"))?
            || n[6].uint()?
                != o[6]
                    .uint()?
                    .checked_add(1)
                    .ok_or(Error::Bound("Native generation"))?
            || n[12].optional()?.map(Value::fixed::<32>).transpose()?
                != Some(hash(MIXED_NATIVE_ID, &value.native))
        {
            return Err(Error::Integrity("Mixed7 encoding-only conversion"));
        }
        let mut counts = [0u64; 2];
        let mut keys = std::collections::BTreeSet::new();
        for (role, id, generation, first) in &value.seen {
            if *role > 1 || !keys.insert((*role, *id)) {
                return Err(Error::Integrity("conversion Seen shape"));
            }
            let ns = if *role == 0 {
                Namespace::SeenA
            } else {
                Namespace::SeenB
            };
            let raw = self
                .get(Key::hash(c, ns, *id)?)?
                .ok_or(Error::Integrity("conversion current Seen missing"))?;
            if raw.len() != 40 || raw[..8] != generation.to_be_bytes() || raw[8..] != *first {
                return Err(Error::Integrity("conversion current Seen correspondence"));
            }
            counts[usize::from(*role)] += 1;
        }
        for (r, count) in counts.iter().enumerate() {
            if *count != self.root.roots[usize::from(c) * 24 + 10 + r].count {
                return Err(Error::Integrity("conversion complete Seen mirror"));
            }
        }
        if p.contexts[usize::from(c)].1 > 1 {
            return Err(Error::OwnerMismatch);
        }
        self.put(
            Key::hash(c, Namespace::Native, [0; 32])?,
            &value.native,
            (128 * 1024 - value.native.len()) as u64,
            true,
        )?;
        Ok(())
    }
    pub(in crate::vault::indexed_dialog) fn existing_apply7(
        &mut self,
        op: ExistingOperation7,
    ) -> Result<()> {
        let c = op.context;
        let p = self.migration_profile()?;
        let role = p.contexts[usize::from(c)].1;
        let mut state = self.existing_state7(c)?;
        let mut converted_head: Option<Vec<u8>> = None;
        match op.action {
            ExistingAction7::Consent {
                expected,
                raw,
                historical,
            } => {
                let id = self.existing_consent(c, &raw, (!historical).then_some(op.wall_time))?;
                self.existing_store(c, id, &raw, 4096)?;
                if !historical {
                    if state.current_consent != expected {
                        return Err(Error::Conflict);
                    }
                    if state.current_consent != Some(id) {
                        state.current_consent = Some(id);
                        let mut meta = self.l1_meta(c)?;
                        meta.policy_generation = meta
                            .policy_generation
                            .checked_add(1)
                            .ok_or(Error::Bound("policy generation"))?;
                        self.l1_meta_put(c, &meta)?;
                    }
                }
            }
            ExistingAction7::Intent { request } => {
                if state.local_outcomes.iter().any(|i| i.request == request) {
                    return Ok(());
                }
                if state.mode != ExistingMode::L1 {
                    return Err(Error::PolicyRejected("TooLate"));
                }
                if let Some(old) = &state.local_intent {
                    if old.request != request && !old.cancelled {
                        return Err(Error::PolicyRejected("IntentAlreadyPending"));
                    }
                    if old.request == request {
                        return Ok(());
                    }
                }
                if state.local_outcomes.len() == 128 {
                    return Err(Error::Bound("local upgrade outcome count"));
                }
                let consent = state
                    .current_consent
                    .ok_or(Error::PolicyRejected("current Consent2 absent"))?;
                let body = codec::encode(&a([b(p.contexts[usize::from(c)].0), b(request)]), 128)?;
                let intent = UpgradeIntent7 {
                    request,
                    consent,
                    body_digest: hash(b"kg:od2:upgrade-request/v7\0", &body),
                    cancelled: false,
                    notification_sent_unconfirmed: false,
                };
                state.local_outcomes.push(intent.clone());
                state.local_intent = Some(intent);
            }
            ExistingAction7::Cancel { request } => {
                if state
                    .local_outcomes
                    .iter()
                    .any(|i| i.request == request && i.cancelled)
                {
                    return Ok(());
                }
                let old = state
                    .local_intent
                    .as_mut()
                    .ok_or(Error::PolicyRejected("NoLocalIntent"))?;
                if old.request != request {
                    return Err(Error::Conflict);
                }
                if old.cancelled {
                    return Ok(());
                }
                if state.mode != ExistingMode::L1 {
                    return Err(Error::PolicyRejected("TooLate"));
                }
                old.cancelled = true;
                state
                    .local_outcomes
                    .iter_mut()
                    .find(|i| i.request == request)
                    .ok_or(Error::Integrity("local cancellation outcome"))?
                    .cancelled = true;
            }
            ExistingAction7::NotificationSent { request } => {
                if role != 1 || state.mode != ExistingMode::L1 {
                    return Err(Error::PolicyRejected("notification role/stage"));
                }
                let intent = state
                    .local_intent
                    .as_mut()
                    .ok_or(Error::PolicyRejected("NoLocalIntent"))?;
                if intent.cancelled || intent.request != request {
                    return Err(Error::Conflict);
                }
                intent.notification_sent_unconfirmed = true;
                state
                    .local_outcomes
                    .iter_mut()
                    .find(|i| i.request == request)
                    .ok_or(Error::Integrity("notification saved request"))?
                    .notification_sent_unconfirmed = true;
            }
            ExistingAction7::PeerIntent { request, body } => {
                if role != 0 {
                    return Err(Error::OwnerMismatch);
                }
                let v = codec::decode(&body, 512)?;
                let b = v.fields(6)?;
                let head = self
                    .get(head_key(c)?)?
                    .ok_or(Error::Integrity("notification Head"))?;
                let wrapper = codec::read_record(&head, 10, 8192)?;
                let inner = codec::decode(wrapper.fields(9)?[4].bytes()?, 4096)?;
                let h = inner.fields(13)?;
                if b[0].fixed::<32>()? != p.contexts[usize::from(c)].0
                    || b[1].uint()? != 3
                    || b[2] != h[7]
                    || b[3] != h[8]
                {
                    return Err(Error::Integrity("peer intent original scope"));
                }
                for r in 0..2 {
                    let id = self
                        .get(super::super::super::history::lane(c, r as u8, 0)?)?
                        .ok_or(Error::Integrity("notification genesis"))?;
                    if b[4 + r].optional()?.map(Value::fixed::<32>).transpose()?
                        != Some(
                            id.as_slice()
                                .try_into()
                                .map_err(|_| Error::Integrity("genesis ID"))?,
                        )
                    {
                        return Err(Error::Conflict);
                    }
                }
                let digest = hash(b"kg:od2:peer-upgrade-intent/v7\0", &body);
                if let Some(old) = &state.peer_intent {
                    if old.request == request && old.body_digest != digest {
                        return Err(Error::Conflict);
                    }
                } else if state.mode == ExistingMode::L1 {
                    state.peer_intent = Some(PeerUpgradeIntent7 {
                        request,
                        body_digest: digest,
                    });
                }
            }
            ExistingAction7::Propose { raw, consent } => {
                if role != 0 || state.mode != ExistingMode::L1 || !state.triggered() {
                    return Err(Error::PolicyRejected("NoTrigger/TooLate"));
                }
                let (id, cut, prev) = self.existing_p7(c, &raw, &consent)?;
                let consent_id = self.existing_consent(c, &consent, Some(op.wall_time))?;
                if state.current_consent != Some(consent_id) {
                    return Err(Error::Conflict);
                }
                self.existing_store(c, consent_id, &consent, 4096)?;
                self.existing_store(c, id, &raw, CONTROL_CAP)?;
                state.propose = Some(id);
                state.selected_consent = Some(consent_id);
                state.cuts[0] = Some(cut);
                state.previous[0] = Some(prev);
                state.mode = ExistingMode::Offering;
                state.control_debt = true;
            }
            ExistingAction7::Prepare {
                propose,
                accept,
                consent,
            } => {
                if role != 1 || state.mode != ExistingMode::L1 {
                    return Err(Error::PolicyRejected("Prepare role/stage"));
                }
                let (id, cut, prev) = self.existing_p7(c, &propose, &consent)?;
                let consent_id = self.existing_consent(c, &consent, None)?;
                let (a_id, b_cut, b_prev) = self.existing_a7(c, &accept, id)?;
                self.existing_store(c, consent_id, &consent, 4096)?;
                self.existing_store(c, id, &propose, CONTROL_CAP)?;
                self.existing_store(c, a_id, &accept, CONTROL_CAP)?;
                state.propose = Some(id);
                state.accept = Some(a_id);
                state.selected_consent = Some(consent_id);
                state.cuts = [Some(cut), Some(b_cut)];
                state.previous = [Some(prev), Some(b_prev)];
                state.mode = ExistingMode::Prepared;
                state.control_debt = true;
            }
            ExistingAction7::Select { proof, converted } => {
                if role != 0 || state.mode != ExistingMode::Offering {
                    return Err(Error::PolicyRejected("Select role/stage"));
                }
                state = self.existing_select7(c, state, &proof, &converted, None)?;
                converted_head = Some(converted.head);
            }
            ExistingAction7::CommitB {
                proof,
                converted,
                committed,
            } => {
                if role != 1 || state.mode != ExistingMode::Prepared {
                    return Err(Error::PolicyRejected("CommitB role/stage"));
                }
                state = self.existing_select7(c, state, &proof, &converted, Some(&committed))?;
                converted_head = Some(converted.head);
            }
            ExistingAction7::Activate { committed } => {
                if role != 0 || state.mode != ExistingMode::Selected {
                    return Err(Error::PolicyRejected("Activate role/stage"));
                }
                let (id, body) = control(&committed, 21)?;
                let f = body.fields(2)?;
                if f[0].fixed::<32>()? != p.contexts[usize::from(c)].0
                    || Some(f[1].fixed()?) != state.q
                {
                    return Err(Error::Conflict);
                }
                self.existing_store(c, id, &committed, CONTROL_CAP)?;
                state.committed = Some(id);
                state.mode = ExistingMode::Active;
                state.control_debt = false;
            }
            ExistingAction7::Locator {
                local,
                revision,
                raw,
            } => {
                let v = codec::decode(&raw, 8192)?;
                let f = v.fields(2)?;
                let b = f[0].fields(10)?;
                let certs = self.l1_cert_refs(&p, c)?;
                let issuer = if local { role } else { 1 - role };
                if b[0].uint()? != 2
                    || b[1].fixed::<32>()? != p.contexts[usize::from(c)].0
                    || b[2].fixed::<32>()? != certs[usize::from(issuer)]
                    || b[3].fixed::<32>()? != certs[usize::from(1 - issuer)]
                    || b[7].uint()? != revision
                    || revision == 0
                    || b[9].uint()? <= op.wall_time
                    || f[1].bytes()?.len() != 64
                {
                    return Err(Error::Integrity("existing Locator2 shape"));
                }
                let idx = usize::from(!local);
                let k = key(
                    c,
                    if local {
                        b"locator2-own"
                    } else {
                        b"locator2-peer"
                    },
                )?;
                if revision < state.locator_revision[idx]
                    || revision == state.locator_revision[idx]
                        && self.get(k)?.as_deref() != Some(raw.as_slice())
                {
                    return Err(Error::Conflict);
                }
                self.put(k, &raw, (8192 - raw.len()) as u64, true)?;
                state.locator_revision[idx] = revision;
            }
            ExistingAction7::PeerSelected { q, peer_stage } => {
                if state.q != Some(q)
                    || role != 1
                    || state.mode != ExistingMode::Active
                    || peer_stage != 5
                {
                    return Err(Error::Conflict);
                }
                state.control_debt = false;
            }
            ExistingAction7::Data(data) => self.existing_data7(&p, c, data)?,
            ExistingAction7::PeerHint { q, cuts, stage } => {
                if role != 1
                    || state.mode != ExistingMode::Prepared
                    || !matches!(stage, 4 | 5)
                    || cuts != state.cuts
                    || state.peer_q_hint.is_some_and(|old| old != q)
                {
                    return Err(Error::Conflict);
                }
                state.peer_q_hint = Some(q);
            }
            ExistingAction7::Block { previous, conflict } => {
                let old = codec::decode(&previous, CONTROL_CAP)?;
                let kind = old.fields(4)?[1].uint()?;
                let (old_id, old_body) = control(&previous, kind)?;
                let (new_id, new_body) = control(&conflict, kind)?;
                let saved = match kind {
                    18 => state.propose,
                    19 => state.accept,
                    20 => state.select,
                    21 => state.committed,
                    _ => return Err(Error::Integrity("existing fork kind")),
                };
                if saved != Some(old_id)
                    || old_id == new_id
                    || old_body.list()?[0] != new_body.list()?[0]
                    || old_body.list()?[0] != b(p.contexts[usize::from(c)].0)
                    || self.get(control_key(c, old_id)?)?.as_deref() != Some(previous.as_slice())
                {
                    return Err(Error::Conflict);
                }
                let proof =
                    codec::encode(&a([u(7), u(kind), b(previous), b(conflict)]), 64 * 1024)?;
                self.put(
                    key(c, b"blocked-fork")?,
                    &proof,
                    (64 * 1024 - proof.len()) as u64,
                    true,
                )?;
                let mut meta = self.l1_meta(c)?;
                meta.policy_generation = meta
                    .policy_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("fork policy generation"))?;
                self.l1_meta_put(c, &meta)?;
            }
        }
        state.check(role)?;
        let raw = packed(&state)?;
        if raw.len() > STATE_CAP {
            return Err(Error::Bound("paid existing state"));
        }
        self.put(state_key(c)?, &raw, (STATE_CAP - raw.len()) as u64, true)?;
        let head_raw = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("existing Head"))?;
        let v = codec::read_record(&head_raw, 10, 8192)?;
        let mut h = v.fields(9)?.to_vec();
        if let Some(head) = converted_head.take() {
            let inner = codec::decode(&head, 4096)?;
            h[5] = inner.fields(13)?[12].clone();
            h[4] = b(head);
        }
        h[6] = u(state.mode.wire());
        h[7] = opt(state.q.map(b));
        h[8] = a(state.cuts.map(|n| opt(n.map(u))));
        let raw = codec::record(10, &a(h), 8192)?;
        self.put(head_key(c)?, &raw, (8192 - raw.len()) as u64, true)?;
        let mut meta = self.l1_meta(c)?;
        meta.wall_high_water = meta.wall_high_water.max(op.wall_time);
        self.l1_meta_put(c, &meta)?;
        self.migration_budget_put(&p, super::super::MigrationStage::Ready)
    }

    fn existing_select7(
        &mut self,
        c: u8,
        mut state: ExistingState7,
        raw: &[u8],
        converted: &ExistingNative7,
        committed: Option<&[u8]>,
    ) -> Result<ExistingState7> {
        let value = codec::decode(raw, 16 * 1024)?;
        let f = value.fields(6)?;
        if f[0].uint()? != 2 || f[1].uint()? != 1 {
            return Err(Error::Integrity("existing selection proof shape"));
        }
        let p_raw = f[2].bytes()?;
        let a_raw = f[3].bytes()?;
        let k_raw = f[4].bytes()?;
        let consent = f[5].bytes()?;
        let (p_id, a_cut, a_prev) = self.existing_p7(c, p_raw, consent)?;
        let (a_id, b_cut, b_prev) = self.existing_a7(c, a_raw, p_id)?;
        let (k_id, body) = control(k_raw, 20)?;
        let k = body.fields(3)?;
        let profile = self.migration_profile()?;
        let consent_id = self.existing_consent(c, consent, None)?;
        let q = hash(MODE_ID, raw);
        if state.propose != Some(p_id)
            || state.accept.is_some_and(|old| old != a_id)
            || state.selected_consent != Some(consent_id)
            || k[0].fixed::<32>()? != profile.contexts[usize::from(c)].0
            || k[1].fixed::<32>()? != p_id
            || k[2].fixed::<32>()? != a_id
            || state.peer_q_hint.is_some_and(|hint| hint != q)
        {
            return Err(Error::Conflict);
        }
        // Handoff reuses this real view. It never creates/replaces a fresh view.
        let identity = self.l1_history_view(c)?;
        let head = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("selection current Head"))?;
        let wrapper = codec::read_record(&head, 10, 8192)?;
        let inner = codec::decode(wrapper.fields(9)?[4].bytes()?, 4096)?;
        let h = inner.fields(13)?;
        if identity.init.scope.context != profile.contexts[usize::from(c)].0
            || Some(identity.init.scope.dialog) != h[9].optional()?.map(Value::fixed).transpose()?
            || Some(identity.init.scope.epoch) != h[10].optional()?.map(Value::fixed).transpose()?
            || Some(identity.init.scope.session_id.as_bytes())
                != h[11].optional()?.map(Value::bytes).transpose()?
        {
            return Err(Error::Integrity("selection immutable history view"));
        }
        self.existing_convert7(c, converted)?;
        self.existing_store(c, p_id, p_raw, CONTROL_CAP)?;
        self.existing_store(c, a_id, a_raw, CONTROL_CAP)?;
        self.existing_store(c, k_id, k_raw, CONTROL_CAP)?;
        self.existing_store(c, q, raw, 16 * 1024)?;
        state.accept = Some(a_id);
        state.select = Some(k_id);
        state.q = Some(q);
        state.cuts = [Some(a_cut), Some(b_cut)];
        state.previous = [Some(a_prev), Some(b_prev)];
        state.mode = ExistingMode::Selected;
        state.control_debt = true;
        if let Some(raw) = committed {
            let (id, body) = control(raw, 21)?;
            let f = body.fields(2)?;
            if f[0].fixed::<32>()? != profile.contexts[usize::from(c)].0 || f[1].fixed::<32>()? != q
            {
                return Err(Error::Conflict);
            }
            self.existing_store(c, id, raw, CONTROL_CAP)?;
            state.committed = Some(id);
            state.mode = ExistingMode::Active;
        }
        Ok(state)
    }

    pub(in crate::vault::indexed_dialog) fn existing_head_check7(
        &mut self,
        c: u8,
        head: &Value,
    ) -> Result<ExistingState7> {
        let state = self.existing_state7(c)?;
        let h = head.fields(9)?;
        if h[6].uint()? != state.mode.wire()
            || h[7].optional()?.map(Value::fixed::<32>).transpose()? != state.q
            || h[8] != a(state.cuts.map(|n| opt(n.map(u))))
        {
            return Err(Error::Integrity("Head7/existing mode correspondence"));
        }
        if let Some(q) = state.q {
            let raw = self
                .get(control_key(c, q)?)?
                .ok_or(Error::Integrity("selected proof absent"))?;
            let proof = codec::decode(&raw, 16 * 1024)?;
            let f = proof.fields(6)?;
            if hash(MODE_ID, &raw) != q
                || f[0].uint()? != 2
                || f[1].uint()? != 1
                || Some(control(f[2].bytes()?, 18)?.0) != state.propose
                || Some(control(f[3].bytes()?, 19)?.0) != state.accept
                || Some(control(f[4].bytes()?, 20)?.0) != state.select
                || Some(self.existing_consent(c, f[5].bytes()?, None)?) != state.selected_consent
            {
                return Err(Error::Integrity("selected proof retained correspondence"));
            }
        }
        for id in [state.current_consent, state.selected_consent]
            .into_iter()
            .flatten()
        {
            let raw = self
                .get(control_key(c, id)?)?
                .ok_or(Error::Integrity("existing consent absent"))?;
            if self.existing_consent(c, &raw, None)? != id {
                return Err(Error::Integrity("existing consent ref"));
            }
        }
        let profile = self.migration_profile()?;
        for outcome in &state.local_outcomes {
            let body = codec::encode(
                &a([b(profile.contexts[usize::from(c)].0), b(outcome.request)]),
                128,
            )?;
            let raw = self
                .get(control_key(c, outcome.consent)?)?
                .ok_or(Error::Integrity("intent creation consent"))?;
            if outcome.body_digest != hash(b"kg:od2:upgrade-request/v7\0", &body)
                || self.existing_consent(c, &raw, None)? != outcome.consent
            {
                return Err(Error::Integrity("intent durable body/consent"));
            }
        }
        for (id, kind) in [
            (state.propose, 18),
            (state.accept, 19),
            (state.select, 20),
            (state.committed, 21),
        ] {
            if let Some(id) = id {
                let raw = self
                    .get(control_key(c, id)?)?
                    .ok_or(Error::Integrity("existing control absent"))?;
                let (actual, body) = control(&raw, kind)?;
                let f = body.list()?;
                let profile = self.migration_profile()?;
                if actual != id || f[0].fixed::<32>()? != profile.contexts[usize::from(c)].0 {
                    return Err(Error::Integrity("existing control hash/scope"));
                }
                match kind {
                    18 => {
                        let inner = codec::decode(h[4].bytes()?, 4096)?;
                        let old = inner.fields(13)?;
                        if Some(f[1].fixed::<32>()?)
                            != old[9].optional()?.map(Value::fixed).transpose()?
                            || Some(f[2].fixed::<32>()?)
                                != old[10].optional()?.map(Value::fixed).transpose()?
                            || Some(f[3].bytes()?)
                                != old[11].optional()?.map(Value::bytes).transpose()?
                            || Some(f[4].fixed::<32>()?) != state.selected_consent
                            || Some(f[6].uint()?) != state.cuts[0]
                            || Some(f[7].fixed::<32>()?) != state.previous[0]
                        {
                            return Err(Error::Integrity("retained P exact Head/cut"));
                        }
                        f[5].fixed::<32>()?;
                    }
                    19 => {
                        if Some(f[1].fixed::<32>()?) != state.propose
                            || Some(f[2].uint()?) != state.cuts[1]
                            || Some(f[3].fixed::<32>()?) != state.previous[1]
                        {
                            return Err(Error::Integrity("retained A chain/cut"));
                        }
                    }
                    20 => {
                        if Some(f[1].fixed::<32>()?) != state.propose
                            || Some(f[2].fixed::<32>()?) != state.accept
                        {
                            return Err(Error::Integrity("retained K chain"));
                        }
                    }
                    21 => {
                        if Some(f[1].fixed::<32>()?) != state.q {
                            return Err(Error::Integrity("retained Committed Q"));
                        }
                    }
                    _ => return Err(Error::Integrity("retained control purpose")),
                }
            }
        }
        Ok(state)
    }
}
