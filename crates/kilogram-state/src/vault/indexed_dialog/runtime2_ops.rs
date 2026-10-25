use super::*;
use crate::vault::indexed_dialog::history::protocol2::source::{
    read_event2, read_fresh_binding2, read_runtime_locator, read_runtime_offer, read_runtime_ready,
    runtime_credential_parts,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimePeerPin {
    pub account: Id,
    pub certificate: Id,
    pub revision: u64,
    pub authority: Vec<u8>,
}
#[derive(Clone, Serialize)]
pub struct RuntimeFence {
    pub context_generation: u64,
    pub policy_generation: u64,
    pub native_hash: Option<Id>,
    pub own_sequence: u64,
    pub wall: u64,
}
impl RuntimeSnapshot {
    pub fn fence(&self, wall: u64) -> RuntimeFence {
        RuntimeFence {
            context_generation: self.head.generation,
            policy_generation: self.profile.policy_generation,
            native_hash: self.head.native_hash,
            own_sequence: self.profile.next_author_sequence,
            wall,
        }
    }
}
#[derive(Clone, Serialize)]
pub struct RuntimeGenesisStep {
    pub row: HistoryReadable2,
    pub native: HistoryNativeTransition,
}
impl RuntimeOperation {
    pub fn captured_wall(&self) -> Option<u64> {
        self.context_fence().map(|(_, f)| f.wall)
    }
    pub(super) fn bound(&self) -> Result<()> {
        fn selection(s: &RuntimeSelection) -> Result<()> {
            if s.initial_pair.len() > 192 * 1024
                || s.selected_consent.len() > 4096
                || s.offer.len() > 1024
                || s.grant.len() > 1024
                || s.proof.len() > 16 * 1024
            {
                return Err(Error::Bound("runtime selection"));
            }
            Ok(())
        }
        fn row(r: &HistoryReadable2) -> Result<()> {
            if r.source.len() > 16 * 1024
                || r.text.len() > 4096
                || r.auth.len() > 2
                || r.receiver_receipt.as_ref().is_some_and(|v| v.len() > 1024)
            {
                return Err(Error::Bound("runtime native row"));
            }
            for auth in &r.auth {
                auth.check_bound()?;
            }
            Ok(())
        }
        match self {
            Self::InstallContext { spec, .. } => {
                packed(spec, SPEC_BYTES)?;
            }
            Self::PinAuthority { raw, .. } if raw.is_empty() || raw.len() > 16 * 1024 => {
                return Err(Error::Bound("authority"));
            }
            Self::RenewConsent { raw, .. } | Self::HistoricalConsent { raw, .. }
                if raw.is_empty() || raw.len() > 4096 =>
            {
                return Err(Error::Bound("consent"));
            }
            Self::Locator { raw, .. } if raw.len() > 8192 => return Err(Error::Bound("locator")),
            Self::PrepareOffer {
                selection: s,
                private,
                ..
            }
            | Self::ReserveGrant {
                selection: s,
                private,
                ..
            } => {
                selection(s)?;
                if private.is_empty() || private.len() > 128 * 1024 {
                    return Err(Error::Bound("pre-session native"));
                }
            }
            Self::Genesis {
                selection: s,
                steps,
                ..
            } => {
                selection(s)?;
                if steps.is_empty() || steps.len() > 2 {
                    return Err(Error::Bound("runtime genesis steps"));
                }
                for step in steps {
                    row(&step.row)?;
                    step.native.check_bound()?;
                }
            }
            Self::Ready { raw, .. } if raw.len() > 1024 => return Err(Error::Bound("Ready")),
            Self::Queue { text, .. } if text.len() > 4096 => return Err(Error::Bound("Text")),
            Self::Materialize { rows, native, .. } | Self::Drain { rows, native, .. } => {
                if rows.is_empty() || rows.len() > 4 {
                    return Err(Error::Bound("runtime native batch"));
                }
                for r in rows {
                    row(r)?;
                }
                native.check_bound()?;
            }
            Self::AcceptPending { source, auth, .. } => {
                if source.len() > 16 * 1024 || auth.len() > 2 {
                    return Err(Error::Bound("pending input"));
                }
                for a in auth {
                    a.check_bound()?;
                }
            }
            Self::Acknowledge { raw, auth, .. } => {
                if raw.len() > 1024 || auth.len() > 1 {
                    return Err(Error::Bound("ACK input"));
                }
                for a in auth {
                    a.check_bound()?;
                }
            }
            Self::AckProgress { ids, .. } if ids.is_empty() || ids.len() > 32 => {
                return Err(Error::Bound("ACK cursor batch"));
            }
            Self::Block {
                reason, evidence, ..
            } if reason.len() > 1024
                || evidence.is_empty()
                || evidence.len() > 4
                || evidence.iter().any(|v| v.is_empty() || v.len() > 16 * 1024) =>
            {
                return Err(Error::Bound("block evidence"));
            }
            _ => {}
        }
        Ok(())
    }
    fn context_fence(&self) -> Option<(u8, &RuntimeFence)> {
        match self {
            Self::InstallContext { .. } | Self::PinAuthority { .. } => None,
            Self::RenewConsent { context, fence, .. }
            | Self::Locator { context, fence, .. }
            | Self::PrepareOffer { context, fence, .. }
            | Self::ReserveGrant { context, fence, .. }
            | Self::HistoricalConsent { context, fence, .. }
            | Self::Genesis { context, fence, .. }
            | Self::Ready { context, fence, .. }
            | Self::Queue { context, fence, .. }
            | Self::Materialize { context, fence, .. }
            | Self::AcceptPending { context, fence, .. }
            | Self::Drain { context, fence, .. }
            | Self::Acknowledge { context, fence, .. }
            | Self::Repair { context, fence, .. }
            | Self::AckProgress { context, fence, .. }
            | Self::PeerHead { context, fence, .. }
            | Self::Block { context, fence, .. } => Some((*context, fence)),
        }
    }
}
impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    fn rt_read<T: DeserializeOwned>(&mut self, c: u8, name: &[u8], bytes: usize) -> Result<T> {
        let raw = self
            .get(key(c, name)?)?
            .ok_or(Error::Integrity("runtime record absent"))?;
        unpack(&raw, bytes)
    }
    fn rt_head(&mut self, c: u8) -> Result<RuntimeHead> {
        let h: RuntimeHead = self.rt_read(c, b"head", HEAD_BYTES)?;
        h.check()?;
        if c > 1 || h.context != self.root.contexts[usize::from(c)] || h.role > 1 {
            return Err(Error::Integrity("runtime Head scope"));
        }
        Ok(h)
    }
    fn rt_active(h: &RuntimeHead) -> Result<()> {
        if !h.active || h.phase != 3 || h.ready.iter().any(Option::is_none) || h.blocked.is_some() {
            return Err(Error::PolicyRejected("NeedHandshake/Blocked"));
        }
        Ok(())
    }
    fn rt_finish(
        &mut self,
        c: u8,
        mut h: RuntimeHead,
        p: &mut RuntimeProfile,
        fence: &RuntimeFence,
    ) -> Result<()> {
        h.generation = h
            .generation
            .checked_add(1)
            .ok_or(Error::Bound("context generation"))?;
        p.wall_high_water = p.wall_high_water.max(fence.wall);
        self.runtime_put(c, b"head", &h, HEAD_BYTES)?;
        self.runtime_put_key(profile_key()?, p, PROFILE_BYTES)
    }
    fn rt_selection_check(
        &mut self,
        c: u8,
        selection: &RuntimeSelection,
        spec: &RuntimeContextSpec,
    ) -> Result<crate::vault::indexed_dialog::history::protocol2::source::FreshFields> {
        let fields = read_fresh_binding2(&selection.proof)?;
        if fields.c != self.root.contexts[usize::from(c)]
            || fields.certs != spec.certs
            || fields.offer
                != history_types::hash_parts(b"kg:od2:control-id\0", &[&selection.offer])
            || fields.grant
                != history_types::hash_parts(b"kg:od2:control-id\0", &[&selection.grant])
        {
            return Err(Error::Conflict);
        }
        Ok(fields)
    }
    pub(super) fn runtime_install_peer(
        &mut self,
        p: &mut RuntimeProfile,
        spec: &RuntimeContextSpec,
        revision: u64,
    ) -> Result<()> {
        let role = usize::from(1 - spec.role);
        let (cert, authority) = runtime_credential_parts(&spec.credentials[role])?;
        if cert != spec.certs[role] {
            return Err(Error::Conflict);
        }
        if let Some(pin) = &p.peer {
            if pin.account != spec.accounts[role]
                || pin.certificate != cert
                || (pin.revision == revision && pin.authority != authority)
            {
                return Err(Error::Conflict);
            }
            if pin.revision >= revision {
                return Ok(());
            }
        }
        p.peer = Some(RuntimePeerPin {
            account: spec.accounts[role],
            certificate: cert,
            revision,
            authority: authority.to_vec(),
        });
        Ok(())
    }
    pub(super) fn runtime_check_operation(
        &mut self,
        op: &RuntimeOperation,
        retry: bool,
    ) -> Result<()> {
        op.bound()?;
        let p = self.runtime_profile()?;
        if let Some((c, f)) = op.context_fence() {
            let h = self.rt_head(c)?;
            if !retry
                && (h.generation != f.context_generation
                    || p.policy_generation != f.policy_generation
                    || h.native_hash != f.native_hash
                    || p.next_author_sequence != f.own_sequence)
            {
                return Err(Error::StaleGeneration);
            }
            if !retry && f.wall < p.wall_high_water {
                return Err(Error::PolicyRejected("clock rollback"));
            }
        }
        if retry {
            self.rt_retry(op)?;
        }
        Ok(())
    }
    fn rt_retry(&mut self, op: &RuntimeOperation) -> Result<()> {
        match op {
            RuntimeOperation::PinAuthority {
                account,
                revision,
                raw,
                ..
            } => {
                let p = self.runtime_profile()?;
                let matches = if *account == p.account {
                    p.enrollment.authority_revision == *revision && p.enrollment.authority == *raw
                } else {
                    p.peer.as_ref().is_some_and(|p| {
                        p.account == *account && p.revision == *revision && p.authority == *raw
                    })
                };
                if !matches {
                    return Err(Error::Integrity("authority retry"));
                }
            }
            RuntimeOperation::Genesis {
                context: c, steps, ..
            } => {
                let rows: Vec<_> = steps.iter().map(|s| s.row.clone()).collect();
                self.runtime_retry_native(*c, &rows, &steps.last().ok_or(Error::Conflict)?.native)?;
                self.rt_native_link(*c)?;
            }
            RuntimeOperation::Materialize {
                context: c,
                rows,
                native,
                ..
            }
            | RuntimeOperation::Drain {
                context: c,
                rows,
                native,
                ..
            } => {
                self.runtime_retry_native(*c, rows, native)?;
                self.rt_native_link(*c)?;
            }
            RuntimeOperation::AcceptPending {
                context,
                source,
                head,
                grant,
                admission_author_ref,
                auth,
                ..
            } => self.history2_check_retry(
                *context,
                &HistoryProtocol2Operation::AcceptPending {
                    source: source.clone(),
                    head: *head,
                    grant: *grant,
                    admission_author_ref: *admission_author_ref,
                    auth: auth.clone(),
                },
            )?,
            RuntimeOperation::Acknowledge {
                context, raw, auth, ..
            } => self.history2_check_retry(
                *context,
                &HistoryProtocol2Operation::Acknowledge {
                    receipt_format: 2,
                    receipt_raw: raw.clone(),
                    auth: auth.clone(),
                },
            )?,
            RuntimeOperation::Ready { context, raw, .. } => {
                let ready = read_runtime_ready(raw)?;
                let h = self.rt_head(*context)?;
                if h.ready[usize::from(ready.role)].as_ref() != Some(raw) {
                    return Err(Error::Integrity("Ready retry"));
                }
                self.rt_ready_sources(*context, &h, raw)?;
            }
            RuntimeOperation::Queue {
                context,
                request,
                text,
                ..
            } => {
                if self
                    .runtime_request(*context, *request)?
                    .is_none_or(|(_, old)| old != *text)
                {
                    return Err(Error::Integrity("queue retry"));
                }
            }
            RuntimeOperation::PrepareOffer {
                context,
                selection,
                private,
                ..
            }
            | RuntimeOperation::ReserveGrant {
                context,
                selection,
                private,
                ..
            } => {
                let stored: RuntimeSelection =
                    self.rt_read(*context, b"selection", SELECTION_BYTES)?;
                let raw: Option<Vec<u8>> =
                    self.rt_read(*context, b"pre-native", PRE_NATIVE_BYTES)?;
                if packed(&stored, SELECTION_BYTES)? != packed(selection, SELECTION_BYTES)?
                    || raw.as_ref() != Some(private)
                {
                    return Err(Error::Integrity("pre-session retry"));
                }
                self.rt_native_link(*context)?;
            }
            RuntimeOperation::Locator {
                context, own, raw, ..
            } => {
                let s: RuntimeContextSpec = self.rt_read(*context, b"spec", SPEC_BYTES)?;
                if (if *own { s.own_locator } else { s.peer_locator }).as_ref() != Some(raw) {
                    return Err(Error::Integrity("locator retry"));
                }
            }
            RuntimeOperation::RenewConsent { context, raw, .. } => {
                let s: RuntimeContextSpec = self.rt_read(*context, b"spec", SPEC_BYTES)?;
                if s.consent != *raw {
                    return Err(Error::Integrity("consent retry"));
                }
            }
            RuntimeOperation::HistoricalConsent { context, raw, .. } => {
                let s: RuntimeSelection = self.rt_read(*context, b"selection", SELECTION_BYTES)?;
                if s.selected_consent != *raw {
                    return Err(Error::Integrity("historical consent retry"));
                }
            }
            RuntimeOperation::AckProgress {
                context,
                first,
                ids,
                ..
            } => {
                let h = self.rt_head(*context)?;
                if h.ack_next
                    != first
                        .checked_add(ids.len() as u64)
                        .ok_or(Error::Bound("ACK cursor"))?
                {
                    return Err(Error::Integrity("ACK progress retry"));
                }
                self.rt_ack_ids(*context, &h, *first, ids)?;
            }
            RuntimeOperation::PeerHead {
                context,
                next,
                last,
                ..
            } => {
                let h = self.rt_head(*context)?;
                if (h.peer_source_next, h.peer_source_last) != (*next, *last) {
                    return Err(Error::Integrity("peer head retry"));
                }
            }
            RuntimeOperation::Block {
                context,
                reason,
                evidence,
                ..
            } => {
                let saved: Vec<Vec<u8>> = self.rt_read(*context, b"quarantine", 128 * 1024)?;
                if self.rt_head(*context)?.blocked.as_ref() != Some(reason) || saved != *evidence {
                    return Err(Error::Integrity("block retry"));
                }
            }
            RuntimeOperation::Repair {
                context, repair, ..
            } => {
                let saved: Option<(Vec<u8>, Vec<u8>)> =
                    self.rt_read(*context, b"repair-evidence", 4096)?;
                let mut ledger = Vec::new();
                self.root.ledger[usize::from(*context)].encode(&mut ledger);
                let request =
                    postcard::to_allocvec(repair).map_err(|_| Error::Bound("repair evidence"))?;
                if saved != Some((request, ledger)) {
                    return Err(Error::Integrity("repair retry evidence"));
                }
                self.rt_native_link(*context)?;
            }
            RuntimeOperation::InstallContext { .. } => {
                return Err(Error::Bound("nested runtime install"));
            }
        }
        Ok(())
    }
    fn rt_native_link(&mut self, c: u8) -> Result<()> {
        let h = self.rt_head(c)?;
        let raw = if h.phase < 3 {
            self.rt_read::<Option<Vec<u8>>>(c, b"pre-native", PRE_NATIVE_BYTES)?
        } else {
            self.get(Key::hash(c, Namespace::Native, [0; 32])?)?
        };
        if h.native_hash != raw.as_ref().map(|b| *blake3::hash(b).as_bytes()) {
            return Err(Error::Integrity("runtime native Head link"));
        }
        Ok(())
    }
    fn rt_ready_sources(&mut self, c: u8, h: &RuntimeHead, raw: &[u8]) -> Result<()> {
        let r = read_runtime_ready(raw)?;
        if h.phase != 3
            || (h.dialog, h.epoch, h.sid.as_ref(), h.mode_ref)
                != (Some(r.d), Some(r.e), Some(&r.sid), Some(r.q))
        {
            return Err(Error::Conflict);
        }
        for role in 0..2 {
            let (source, _) = self.runtime_event(c, role as u8, 0)?;
            if source.id != r.genesis[role] || source.q != r.q {
                return Err(Error::Conflict);
            }
        }
        self.rt_native_link(c)
    }
    fn rt_ack_ids(&mut self, c: u8, h: &RuntimeHead, first: u64, ids: &[Id]) -> Result<()> {
        for (offset, id) in ids.iter().enumerate() {
            let n = first
                .checked_add(offset as u64)
                .ok_or(Error::Bound("ACK ordinal"))?;
            let raw = self
                .runtime_receipt(c, 1 - h.role, n)?
                .ok_or(Error::Integrity("missing ACK receipt"))?;
            if history_types::hash_parts(b"kg:od2:receipt-id\0", &[&raw]) != *id {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    pub(super) fn runtime_apply_operation(&mut self, op: RuntimeOperation) -> Result<()> {
        let mut p = self.runtime_profile()?;
        if let RuntimeOperation::PinAuthority {
            account,
            revision,
            raw,
            expected_policy,
        } = op
        {
            if p.policy_generation != expected_policy {
                return Err(Error::StaleGeneration);
            }
            let (old_revision, old_raw) = if account == p.account {
                (p.enrollment.authority_revision, &p.enrollment.authority)
            } else {
                let pin = p
                    .peer
                    .as_ref()
                    .filter(|pin| pin.account == account)
                    .ok_or(Error::OwnerMismatch)?;
                (pin.revision, &pin.authority)
            };
            if revision < old_revision || (revision == old_revision && *old_raw != raw) {
                return Err(Error::Conflict);
            }
            if revision == old_revision {
                return Ok(());
            }
            if account == p.account {
                p.enrollment.authority_revision = revision;
                p.enrollment.authority = raw;
            } else {
                let pin = p.peer.as_mut().ok_or(Error::OwnerMismatch)?;
                pin.revision = revision;
                pin.authority = raw;
            }
            p.policy_generation = p
                .policy_generation
                .checked_add(1)
                .ok_or(Error::Bound("policy generation"))?;
            for c in 0..2 {
                if self.get(key(c, b"spec")?)?.is_some() {
                    let mut h = self.rt_head(c)?;
                    h.generation = h
                        .generation
                        .checked_add(1)
                        .ok_or(Error::Bound("context generation"))?;
                    self.runtime_put(c, b"head", &h, HEAD_BYTES)?;
                }
            }
            return self.runtime_put_key(profile_key()?, &p, PROFILE_BYTES);
        }
        let (c, fence) = op.context_fence().ok_or(Error::Bound("runtime purpose"))?;
        let fence = fence.clone();
        let mut h = self.rt_head(c)?;
        let mut spec: RuntimeContextSpec = self.rt_read(c, b"spec", SPEC_BYTES)?;
        let sending = matches!(&op, RuntimeOperation::Materialize { .. });
        match op {
            RuntimeOperation::RenewConsent {
                expected_ref, raw, ..
            } => {
                if spec.consent == raw {
                    return Ok(());
                }
                if history_types::hash_parts(b"kg:od2:consent\0", &[&spec.consent]) != expected_ref
                {
                    return Err(Error::Conflict);
                }
                spec.consent = raw;
                self.runtime_put(c, b"spec", &spec, SPEC_BYTES)?;
                p.policy_generation = p
                    .policy_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("policy generation"))?;
            }
            RuntimeOperation::Locator { own, raw, .. } => {
                let loc = read_runtime_locator(&raw)?;
                let role = usize::from(if own { h.role } else { 1 - h.role });
                if loc.c != h.context
                    || loc.cert != spec.certs[role]
                    || loc.peer_cert != spec.certs[1 - role]
                    || fence.wall < loc.not_before
                    || fence.wall >= loc.expires
                {
                    return Err(Error::Conflict);
                }
                let (revision, stored) = if own {
                    (&mut spec.own_locator_revision, &mut spec.own_locator)
                } else {
                    (&mut spec.peer_locator_revision, &mut spec.peer_locator)
                };
                if loc.revision < *revision
                    || (loc.revision == *revision && stored.as_ref() != Some(&raw))
                {
                    return Err(Error::Conflict);
                }
                if stored.as_ref() == Some(&raw) {
                    return Ok(());
                }
                if own
                    && loc.revision
                        != revision
                            .checked_add(1)
                            .ok_or(Error::Bound("locator revision"))?
                {
                    return Err(Error::Conflict);
                }
                *revision = loc.revision;
                *stored = Some(raw);
                self.runtime_put(c, b"spec", &spec, SPEC_BYTES)?;
            }
            RuntimeOperation::HistoricalConsent { raw, .. } => {
                let mut s: RuntimeSelection = self.rt_read(c, b"selection", SELECTION_BYTES)?;
                if s.selected_consent == raw {
                    return Ok(());
                }
                if h.phase != 0 || !s.offer.is_empty() || !s.proof.is_empty() {
                    return Err(Error::Conflict);
                }
                s.selected_consent = raw;
                self.runtime_put(c, b"selection", &s, SELECTION_BYTES)?;
            }
            RuntimeOperation::PrepareOffer {
                selection, private, ..
            } => {
                if h.role != 0 || h.phase != 0 {
                    return Err(Error::Conflict);
                }
                let offer = read_runtime_offer(&selection.offer)?;
                if offer.c != h.context
                    || offer.certs != spec.certs
                    || selection.initial_pair.is_empty()
                    || selection.selected_consent != spec.consent
                    || !selection.grant.is_empty()
                    || !selection.proof.is_empty()
                {
                    return Err(Error::Conflict);
                }
                h.phase = 1;
                h.offer = Some(offer.id);
                h.native_hash = Some(*blake3::hash(&private).as_bytes());
                h.native_generation = h
                    .native_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("native generation"))?;
                self.runtime_put(c, b"selection", &selection, SELECTION_BYTES)?;
                self.runtime_put(c, b"pre-native", &Some(private), PRE_NATIVE_BYTES)?;
            }
            RuntimeOperation::ReserveGrant {
                selection, private, ..
            } => {
                if h.role != 1 || h.phase != 0 || selection.initial_pair.is_empty() {
                    return Err(Error::Conflict);
                }
                let proof = self.rt_selection_check(c, &selection, &spec)?;
                h.phase = 2;
                h.offer = Some(proof.offer);
                h.grant = Some(proof.grant);
                h.dialog = Some(proof.d);
                h.epoch = Some(proof.e);
                h.mode_ref = Some(proof.q);
                h.native_hash = Some(*blake3::hash(&private).as_bytes());
                h.native_generation = h
                    .native_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("native generation"))?;
                self.runtime_put(c, b"selection", &selection, SELECTION_BYTES)?;
                self.runtime_put(c, b"pre-native", &Some(private), PRE_NATIVE_BYTES)?;
            }
            RuntimeOperation::Genesis {
                selection, steps, ..
            } => {
                if h.blocked.is_some() || !(1..=3).contains(&h.phase) {
                    return Err(Error::PolicyRejected("NeedHandshake/Blocked"));
                }
                let proof = self.rt_selection_check(c, &selection, &spec)?;
                let old: RuntimeSelection = self.rt_read(c, b"selection", SELECTION_BYTES)?;
                if h.offer != Some(proof.offer)
                    || old.offer != selection.offer
                    || old.selected_consent != selection.selected_consent
                    || old.initial_pair != selection.initial_pair
                    || (h.phase >= 2
                        && (old.grant != selection.grant || old.proof != selection.proof))
                {
                    return Err(Error::Conflict);
                }
                let first = read_event2(&steps[0].row.source)?;
                if first.scope.context != h.context
                    || first.scope.dialog != proof.d
                    || first.scope.epoch != proof.e
                    || first.q != proof.q
                {
                    return Err(Error::Conflict);
                }
                if h.phase < 3 {
                    if first.role != 0
                        || (h.role == 0 && steps.len() != 1)
                        || (h.role == 1 && steps.len() != 2)
                    {
                        return Err(Error::Conflict);
                    }
                    self.history2_apply(
                        c,
                        HistoryProtocol2Operation::Init {
                            init: HistoryInit {
                                scope: first.scope.clone(),
                                own_role: h.role,
                            },
                            binding: HistoryProtocol2Binding {
                                proof: selection.proof.clone(),
                                author_slots: if h.role == 0 { [0, 1] } else { [1, 0] },
                            },
                        },
                    )?;
                } else if h.role != 0
                    || steps.len() != 1
                    || first.role != 1
                    || h.sid.as_ref() != Some(&first.scope.session_id)
                {
                    return Err(Error::Conflict);
                }
                for (index, step) in steps.iter().enumerate() {
                    let e = read_event2(&step.row.source)?;
                    if e.n != 0
                        || e.role
                            != if steps.len() == 2 {
                                index as u8
                            } else {
                                first.role
                            }
                        || self.get(history::lane(c, e.role, 0)?)?.is_some()
                    {
                        return Err(Error::Conflict);
                    }
                    if e.role == h.role {
                        if e.sequence != p.next_author_sequence {
                            return Err(Error::Conflict);
                        }
                        p.next_author_sequence = p
                            .next_author_sequence
                            .checked_add(1)
                            .ok_or(Error::Bound("author sequence"))?;
                    }
                    self.history2_apply(
                        c,
                        HistoryProtocol2Operation::Genesis {
                            row: step.row.clone(),
                            native: step.native.clone(),
                        },
                    )?;
                }
                h.phase = 3;
                h.grant = Some(proof.grant);
                h.dialog = Some(proof.d);
                h.epoch = Some(proof.e);
                h.mode_ref = Some(proof.q);
                h.sid = Some(first.scope.session_id);
                h.native_hash = Some(
                    *blake3::hash(&steps.last().ok_or(Error::Conflict)?.native.after_blob)
                        .as_bytes(),
                );
                h.native_generation = h
                    .native_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("native generation"))?;
                self.runtime_put(c, b"pre-native", &Option::<Vec<u8>>::None, PRE_NATIVE_BYTES)?;
                self.runtime_put(c, b"selection", &selection, SELECTION_BYTES)?;
            }
            RuntimeOperation::Ready { raw, .. } => {
                if h.blocked.is_some() {
                    return Err(Error::PolicyRejected("Blocked"));
                }
                let r = read_runtime_ready(&raw)?;
                self.rt_ready_sources(c, &h, &raw)?;
                if let Some(old) = &h.ready[usize::from(r.role)] {
                    return if *old == raw {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                h.ready[usize::from(r.role)] = Some(raw);
                h.active = h.ready.iter().all(Option::is_some);
            }
            RuntimeOperation::Queue { request, text, .. } => {
                if h.phase != 3 || h.blocked.is_some() {
                    return Err(Error::PolicyRejected("NeedHandshake/Blocked"));
                }
                if let Some((_, old)) = self.runtime_request(c, request)? {
                    return if old == text {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                self.history_apply(c, HistoryOperation::QueueAccept { request, text })?;
            }
            RuntimeOperation::Materialize { rows, native, .. }
            | RuntimeOperation::Drain { rows, native, .. } => {
                Self::rt_active(&h)?;
                let role = read_event2(&rows[0].source)?.role;
                if role != if sending { h.role } else { 1 - h.role } {
                    return Err(Error::Conflict);
                }
                for row in &rows {
                    let e = read_event2(&row.source)?;
                    if e.n == 0 || e.kind != 3 || e.role != role {
                        return Err(Error::Conflict);
                    }
                    if e.role == h.role {
                        if e.sequence != p.next_author_sequence {
                            return Err(Error::Conflict);
                        }
                        p.next_author_sequence = p
                            .next_author_sequence
                            .checked_add(1)
                            .ok_or(Error::Bound("author sequence"))?;
                    }
                }
                self.history2_apply(
                    c,
                    HistoryProtocol2Operation::TextBatch {
                        rows,
                        native: native.clone(),
                    },
                )?;
                h.native_hash = Some(*blake3::hash(&native.after_blob).as_bytes());
                h.native_generation = h
                    .native_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("native generation"))?;
            }
            RuntimeOperation::AcceptPending {
                source,
                head,
                grant,
                admission_author_ref,
                auth,
                ..
            } => {
                Self::rt_active(&h)?;
                let before = self.root.roots;
                self.history2_apply(
                    c,
                    HistoryProtocol2Operation::AcceptPending {
                        source,
                        head,
                        grant,
                        admission_author_ref,
                        auth,
                    },
                )?;
                if self.root.roots == before {
                    return Ok(());
                }
            }
            RuntimeOperation::Acknowledge { raw, auth, .. } => {
                Self::rt_active(&h)?;
                let before = self.root.roots;
                self.history2_apply(
                    c,
                    HistoryProtocol2Operation::Acknowledge {
                        receipt_format: 2,
                        receipt_raw: raw,
                        auth,
                    },
                )?;
                if self.root.roots == before {
                    return Ok(());
                }
            }
            RuntimeOperation::Repair { repair, .. } => {
                Self::rt_active(&h)?;
                let before = self.root.ledger.clone();
                let request =
                    postcard::to_allocvec(&repair).map_err(|_| Error::Bound("repair evidence"))?;
                self.history_apply(c, HistoryOperation::RepairCredit(repair))?;
                if self.root.ledger == before {
                    return Ok(());
                }
                let mut ledger = Vec::new();
                self.root.ledger[usize::from(c)].encode(&mut ledger);
                self.runtime_put(c, b"repair-evidence", &Some((request, ledger)), 4096)?;
            }
            RuntimeOperation::AckProgress { first, ids, .. } => {
                Self::rt_active(&h)?;
                self.rt_ack_ids(c, &h, first, &ids)?;
                let next = first
                    .checked_add(ids.len() as u64)
                    .ok_or(Error::Bound("ACK progress"))?;
                if next <= h.ack_next {
                    return Ok(());
                }
                if first != h.ack_next {
                    return Err(Error::Conflict);
                }
                h.ack_next = next;
            }
            RuntimeOperation::PeerHead { next, last, .. } => {
                if (next == 0) != last.is_none() {
                    return Err(Error::Conflict);
                }
                if next < h.peer_source_next {
                    return Ok(());
                }
                if next == h.peer_source_next {
                    return if last == h.peer_source_last {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                h.peer_source_next = next;
                h.peer_source_last = last;
            }
            RuntimeOperation::Block {
                reason, evidence, ..
            } => {
                if let Some(old) = &h.blocked {
                    let saved: Vec<Vec<u8>> = self.rt_read(c, b"quarantine", 128 * 1024)?;
                    return if *old == reason && saved == evidence {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                self.runtime_put(c, b"quarantine", &evidence, 128 * 1024)?;
                h.blocked = Some(reason);
            }
            RuntimeOperation::InstallContext { .. } | RuntimeOperation::PinAuthority { .. } => {
                return Err(Error::Bound("runtime purpose"));
            }
        }
        self.rt_finish(c, h, &mut p, &fence)
    }
}
