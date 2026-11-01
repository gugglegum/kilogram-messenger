//! Production existing-control service, borrowed by the single Root7 owner.
//! The caller supplies its actual admitted-lease final gate for network actions;
//! this service independently verifies current Root permission, signed controls,
//! durable source/Native origin and closed storage purposes. Component callers
//! exercise that same core without claiming TLS admission.
use super::native::{MixedIdentity7, MixedNative7};
use super::*;
use kilogram_identity::{AccountAuthoritySnapshot, DeviceState};
use kilogram_protocol::ordered_dialog_v2 as v2;
mod scheduler;
pub use scheduler::*;

pub struct ExistingControls7<'a> {
    repository: &'a mut store::L1Repository,
    device: &'a DeviceState,
    identity: MixedIdentity7,
}
pub type ControlFinalGate<'a> = &'a mut dyn FnMut() -> db::Result<()>;
pub struct StagedNotification7 {
    context: u8,
    request: [u8; 16],
    body: v2::FrameBody2,
}
impl StagedNotification7 {
    pub fn request(&self) -> [u8; 16] {
        self.request
    }
    pub fn body(&self) -> &v2::FrameBody2 {
        &self.body
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum UpgradeStatus7 {
    NoLocalIntent,
    LocalIntentPending,
    NotificationPending,
    NotificationSentUnconfirmed,
    PeerIntentAccepted,
    ProposalDeferred(String),
    CoalescedWithP(Id),
    LocalIntentCancelledRemoteMayProceed,
    TooLate(store::ExistingMode),
}

impl<'a> ExistingControls7<'a> {
    pub fn new(repository: &'a mut store::L1Repository, device: &'a DeviceState) -> Result<Self> {
        let material = repository.owner_material()?;
        let cert = kilogram_identity::DeviceCertificate::decode_and_verify(&material.certificate)?;
        ensure!(
            cert.device_id() == device.identity().device_id()
                && cert.encryption_public_key() == device.encryption().public_key(),
            "existing original owner material"
        );
        let identity = MixedIdentity7 {
            incarnation: material.incarnation,
            claim: material.nonce,
            source: repository.source_id(),
        };
        Ok(Self {
            repository,
            device,
            identity,
        })
    }
    pub fn state(&mut self, c: u8) -> Result<store::ExistingState7> {
        Ok(self.repository.existing_state(c)?)
    }
    fn head(&mut self, c: u8) -> Result<Head> {
        let spec = Spec::parse(&self.repository.spec(c)?)?;
        let role = spec.role(self.device.identity().device_id())?;
        Head::parse(
            &self.repository.head(c)?,
            spec.c,
            spec.cert_refs()?[role as usize],
            role,
        )
    }
    pub fn scope(&mut self, c: u8) -> Result<v2::VerifiedExistingScope1> {
        let head = self.head(c)?;
        ensure!(head.phase == 3, "NeedHandshake");
        let spec = self.repository.spec(c)?;
        let offer = self
            .repository
            .signed_control(c, head.offer.context("Offer1")?)?;
        let grant = self
            .repository
            .signed_control(c, head.grant.context("Grant1")?)?;
        let pair = self
            .repository
            .control(c, store::L1Control::InitialPair)?
            .context("selected initial pair")?;
        let mut genesis = Vec::new();
        let mut ready = Vec::new();
        for role in 0..2 {
            genesis.push(b(self
                .repository
                .event_at(c, role, 0)?
                .context("readable genesis")?));
            ready.push(b(self
                .repository
                .control(c, store::L1Control::Ready(role))?
                .context("signed Ready1")?));
        }
        let raw = wire::encode(
            &a([
                u(1),
                b(spec),
                b(offer),
                b(grant),
                b(pair),
                a(genesis),
                a(ready),
            ]),
            v2::MAX_EXISTING_ORIGIN,
        )?;
        let verified = v2::VerifiedExistingScope1::verify_export(&raw)?;
        ensure!(
            head.c == verified.c()
                && head.d == Some(verified.d())
                && head.e == Some(verified.e())
                && head.sid.as_deref() == Some(verified.sid()),
            "existing current Head/signed old scope"
        );
        Ok(verified)
    }
    fn peers(
        &mut self,
        scope: &v2::VerifiedExistingScope1,
    ) -> Result<[v2::RootVerifiedCredential; 2]> {
        let mut peers = Vec::new();
        for old in scope.peers() {
            let raw = self
                .repository
                .authority(*old.certificate().account_id().as_bytes())?;
            let authority = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            ensure!(
                authority.encode()? == raw,
                "current authority exact encoding"
            );
            peers.push(v2::RootVerifiedCredential::from_parts(
                old.certificate().clone(),
                authority,
            )?);
        }
        peers
            .try_into()
            .map_err(|_| anyhow::anyhow!("current existing peer count"))
    }
    fn permission(&mut self, c: u8) -> Result<(u64, u64)> {
        let scope = self.scope(c)?;
        let peers = self.peers(&scope)?;
        let state = self.state(c)?;
        let raw = self
            .repository
            .existing_control(c, state.current_consent.context("current Consent2 absent")?)?;
        let consent = v2::ParsedConsent2::parse(&raw)?;
        scope.verify_consent(&consent, &peers)?;
        let meta = self.repository.metadata(c)?;
        let wall = now()?;
        let (nb, exp) = consent.interval();
        ensure!(
            wall >= meta.wall_high_water && nb <= wall && wall < exp,
            "current existing permission/clock"
        );
        ensure!(
            self.repository
                .control(c, store::L1Control::BlockedFork)?
                .is_none(),
            "Blocked: signed fork"
        );
        Ok((wall, exp))
    }
    pub fn current_permission(&mut self, c: u8) -> Result<u64> {
        Ok(self.permission(c)?.1)
    }
    pub fn status(&mut self, c: u8) -> Result<UpgradeStatus7> {
        let state = self.state(c)?;
        if state.mode == store::ExistingMode::L1
            && state.triggered()
            && let Err(error) = self.permission(c)
        {
            return Ok(UpgradeStatus7::ProposalDeferred(format!("{error:#}")));
        }
        if let Some(intent) = &state.local_intent {
            if intent.cancelled {
                return Ok(UpgradeStatus7::LocalIntentCancelledRemoteMayProceed);
            }
            if let Some(p) = state.propose {
                return Ok(UpgradeStatus7::CoalescedWithP(p));
            }
            if intent.notification_sent_unconfirmed {
                return Ok(UpgradeStatus7::NotificationSentUnconfirmed);
            }
            return Ok(if self.head(c)?.role == 1 {
                UpgradeStatus7::NotificationPending
            } else {
                UpgradeStatus7::LocalIntentPending
            });
        }
        if state.peer_intent.is_some() {
            return Ok(state.propose.map_or(
                UpgradeStatus7::PeerIntentAccepted,
                UpgradeStatus7::CoalescedWithP,
            ));
        }
        Ok(if state.mode == store::ExistingMode::L1 {
            UpgradeStatus7::NoLocalIntent
        } else {
            UpgradeStatus7::TooLate(state.mode)
        })
    }
    fn conflict(
        &mut self,
        c: u8,
        old: Id,
        new: &v2::ParsedControl2,
        gate: ControlFinalGate<'_>,
    ) -> Result<()> {
        if old != new.id() {
            let previous = self.repository.existing_control(c, old)?;
            self.commit(
                c,
                store::ExistingAction7::Block {
                    previous,
                    conflict: new.raw().to_vec(),
                },
                Some(gate),
                true,
            )?;
            anyhow::bail!("Blocked: conflicting signed existing control");
        }
        Ok(())
    }
    fn commit(
        &mut self,
        c: u8,
        action: store::ExistingAction7,
        network: Option<ControlFinalGate<'_>>,
        permission: bool,
    ) -> Result<()> {
        let (floor, expires) = if permission {
            self.permission(c)?
        } else {
            (now()?, u64::MAX)
        };
        let barrier = match &action {
            store::ExistingAction7::Propose { .. } => Some("P"),
            store::ExistingAction7::Prepare { .. } => Some("A"),
            store::ExistingAction7::Select { .. } => Some("K"),
            store::ExistingAction7::CommitB { .. } => Some("Committed"),
            _ => None,
        };
        let head = self.repository.head(c)?;
        let meta = self.repository.metadata(c)?;
        let op = store::ExistingOperation7 {
            context: c,
            expected_root: self.repository.generation(),
            expected_head: wire::hash(b"", &head),
            policy_generation: meta.policy_generation,
            wall_time: floor,
            action,
        };
        let mut network = network;
        let mut final_gate = || -> db::Result<()> {
            if let Some(gate) = network.as_mut() {
                gate()?;
            }
            if now().map_or(true, |wall| wall < floor || wall >= expires) {
                return Err(db::Error::PolicyRejected("existing final clock/permission"));
            }
            Ok(())
        };
        if let Some(name) = barrier {
            self.checkpoint_control(c, &format!("s53-{name}-before-commit"))?;
        }
        self.repository.commit_existing(op, Some(&mut final_gate))?;
        if let Some(name) = barrier {
            self.checkpoint_control(c, &format!("s53-{name}-after-commit"))?;
        }
        Ok(())
    }
    fn checkpoint_control(&mut self, c: u8, name: &str) -> Result<()> {
        if super::super::test_support::load()?.pause_point.as_deref() != Some(name) {
            return Ok(());
        }
        let state = self.state(c)?;
        let head = self.head(c)?;
        let facts = serde_json::json!({"context":state::hex(&head.c),"mode":format!("{:?}",state.mode),"q":state.q.map(|id|state::hex(&id)),"cuts":state.cuts,"native_hash":head.blob.map(|id|state::hex(&id)),"controls":{"p":state.propose.map(|id|state::hex(&id)),"a":state.accept.map(|id|state::hex(&id)),"k":state.select.map(|id|state::hex(&id)),"committed":state.committed.map(|id|state::hex(&id))}});
        super::super::test_support::checkpoint_facts(name, Some(&facts))
    }
    pub fn peer_hint(
        &mut self,
        c: u8,
        q: Id,
        cuts: [Option<u64>; 2],
        stage: u8,
        gate: ControlFinalGate<'_>,
    ) -> Result<()> {
        let state = self.state(c)?;
        if state.peer_q_hint == Some(q) {
            return Ok(());
        }
        self.commit(
            c,
            store::ExistingAction7::PeerHint { q, cuts, stage },
            Some(gate),
            true,
        )
    }
    pub fn peer_active(&mut self, c: u8, q: Id, gate: ControlFinalGate<'_>) -> Result<()> {
        if !self.state(c)?.control_debt {
            return Ok(());
        }
        self.commit(
            c,
            store::ExistingAction7::PeerSelected { q, peer_stage: 5 },
            Some(gate),
            true,
        )
    }
    pub fn import_consent(
        &mut self,
        c: u8,
        expected: Option<Id>,
        raw: Vec<u8>,
        historical: bool,
    ) -> Result<()> {
        let scope = self.scope(c)?;
        let consent = v2::ParsedConsent2::parse(&raw)?;
        if historical {
            scope.verify_consent(&consent, scope.peers())?;
        } else {
            let peers = self.peers(&scope)?;
            scope.verify_consent(&consent, &peers)?;
            let wall = now()?;
            let (nb, exp) = consent.interval();
            ensure!(nb <= wall && wall < exp, "current Consent2 interval");
        }
        self.commit(
            c,
            store::ExistingAction7::Consent {
                expected,
                raw,
                historical,
            },
            None,
            false,
        )
    }
    pub fn request_upgrade(&mut self, c: u8, request: [u8; 16]) -> Result<store::ExistingState7> {
        self.permission(c)?;
        let state = self.state(c)?;
        if state.local_outcomes.iter().any(|i| i.request == request) {
            return Ok(state);
        }
        self.commit(c, store::ExistingAction7::Intent { request }, None, true)?;
        self.state(c)
    }
    pub fn cancel(&mut self, c: u8, request: [u8; 16]) -> Result<store::ExistingState7> {
        let state = self.state(c)?;
        if state
            .local_outcomes
            .iter()
            .any(|i| i.request == request && i.cancelled)
        {
            return Ok(state);
        }
        self.commit(c, store::ExistingAction7::Cancel { request }, None, false)?;
        self.state(c)
    }
    pub fn notification_body(&mut self, c: u8) -> Result<Option<v2::FrameBody2>> {
        self.permission(c)?;
        let state = self.state(c)?;
        let scope = self.scope(c)?;
        if self.head(c)?.role != 1
            || state.mode != store::ExistingMode::L1
            || !state.local_intent.as_ref().is_some_and(|i| !i.cancelled)
        {
            return Ok(None);
        }
        let ids = scope.old_control_ids();
        Ok(Some(v2::FrameBody2::parse(
            v2::FrameKind::OpenIntent,
            &wire::encode(
                &a([
                    b(scope.c()),
                    u(3),
                    opt(Some(b(ids[0]))),
                    opt(Some(b(ids[1]))),
                    opt(Some(b(ids[2]))),
                    opt(Some(b(ids[3]))),
                ]),
                512,
            )?,
        )?))
    }
    pub fn stage_notification(&mut self, c: u8) -> Result<Option<StagedNotification7>> {
        let Some(body) = self.notification_body(c)? else {
            return Ok(None);
        };
        let request = self
            .state(c)?
            .local_intent
            .context("staged local intent")?
            .request;
        Ok(Some(StagedNotification7 {
            context: c,
            request,
            body,
        }))
    }
    /// Recheck cancellation immediately before handing the frame to transport.
    /// Persist conservative sent-unconfirmed state first: after this boundary a
    /// crash/cancel must allow that remote publication may already be in flight.
    pub fn handoff_notification(
        &mut self,
        staged: StagedNotification7,
        gate: ControlFinalGate<'_>,
    ) -> Result<Option<v2::FrameBody2>> {
        let state = self.state(staged.context)?;
        if state.mode != store::ExistingMode::L1
            || !state
                .local_intent
                .as_ref()
                .is_some_and(|i| i.request == staged.request && !i.cancelled)
        {
            return Ok(None);
        }
        self.commit(
            staged.context,
            store::ExistingAction7::NotificationSent {
                request: staged.request,
            },
            Some(gate),
            true,
        )?;
        Ok(Some(staged.body))
    }
    pub fn accept_peer_notification(
        &mut self,
        c: u8,
        frame: &v2::VerifiedFrame2,
        gate: ControlFinalGate<'_>,
    ) -> Result<store::ExistingState7> {
        let scope = self.scope(c)?;
        ensure!(
            self.head(c)?.role == 0 && frame.signer() == scope.peers()[1].certificate().device_id(),
            "B notification/A handler"
        );
        scope.verify_open_intent(frame.frame().body(), 1)?;
        self.publish_gate(c, gate)?;
        let body = wire::encode(frame.frame().body().value(), 512)?;
        let state = self.state(c)?;
        if let Some(peer) = &state.peer_intent {
            ensure!(
                peer.body_digest == wire::hash(b"kg:od2:peer-upgrade-intent/v7\0", &body),
                "saved peer trigger body correspondence"
            );
            return Ok(state);
        }
        if state.propose.is_some() {
            return Ok(state);
        }
        self.commit(
            c,
            store::ExistingAction7::PeerIntent {
                request: frame.frame().request(),
                body,
            },
            Some(gate),
            true,
        )?;
        self.state(c)
    }
    fn publish_gate(&mut self, c: u8, gate: ControlFinalGate<'_>) -> Result<()> {
        self.permission(c)?;
        gate()?;
        Ok(())
    }
    pub fn propose(&mut self, c: u8, gate: ControlFinalGate<'_>) -> Result<Vec<u8>> {
        ensure!(
            self.head(c)?.role == 0,
            "Propose requires original role A before retry lookup"
        );
        self.publish_gate(c, gate)?;
        let state = self.state(c)?;
        if let Some(id) = state.propose {
            return Ok(self.repository.existing_control(c, id)?);
        }
        let head = self.head(c)?;
        ensure!(
            head.role == 0 && state.mode == store::ExistingMode::L1 && state.triggered(),
            "A Propose trigger/stage"
        );
        let meta = self.repository.metadata(c)?;
        let consent = self
            .repository
            .existing_control(c, state.current_consent.context("current Consent2")?)?;
        let raw = v2::ParsedControl2::sign(
            18,
            &wire::encode(
                &a([
                    b(head.c),
                    b(head.d.context("D")?),
                    b(head.e.context("E")?),
                    b(head.sid.as_ref().context("SID")?.as_bytes()),
                    b(v2::ParsedConsent2::parse(&consent)?.reference()),
                    b(admin::random::<32>()?),
                    u(meta.next[0]),
                    b(meta.tails[0].context("A cut previous")?),
                ]),
                4096,
            )?,
            self.device.identity(),
        )?
        .raw()
        .to_vec();
        self.commit(
            c,
            store::ExistingAction7::Propose {
                raw: raw.clone(),
                consent,
            },
            Some(gate),
            true,
        )?;
        Ok(raw)
    }
    pub fn prepare(
        &mut self,
        c: u8,
        p_raw: &[u8],
        consent: &[u8],
        gate: ControlFinalGate<'_>,
    ) -> Result<Vec<u8>> {
        ensure!(
            self.head(c)?.role == 1,
            "Prepare requires original role B before retry lookup"
        );
        self.publish_gate(c, gate)?;
        let scope = self.scope(c)?;
        let p = v2::ParsedControl2::parse(p_raw)?;
        p.require_kind(18)?;
        p.verify(scope.peers()[0].certificate().device_id())?;
        let historical = v2::ParsedConsent2::parse(consent)?;
        scope.verify_consent(&historical, scope.peers())?;
        let fields = p.body().fields(8)?;
        ensure!(
            fields[0].fixed::<32>()? == scope.c()
                && fields[1].fixed::<32>()? == scope.d()
                && fields[2].fixed::<32>()? == scope.e()
                && fields[3].text()? == scope.sid()
                && fields[4].fixed::<32>()? == historical.reference(),
            "P signed scope/selected consent"
        );
        let state = self.state(c)?;
        if let Some(id) = state.propose {
            self.conflict(c, id, &p, gate)?;
            return Ok(self
                .repository
                .existing_control(c, state.accept.context("durable A")?)?);
        }
        ensure!(
            self.head(c)?.role == 1 && state.mode == store::ExistingMode::L1,
            "B Prepare role/stage"
        );
        let meta = self.repository.metadata(c)?;
        let raw = v2::ParsedControl2::sign(
            19,
            &wire::encode(
                &a([
                    b(scope.c()),
                    b(p.id()),
                    u(meta.next[1]),
                    b(meta.tails[1].context("B cut previous")?),
                ]),
                4096,
            )?,
            self.device.identity(),
        )?
        .raw()
        .to_vec();
        self.commit(
            c,
            store::ExistingAction7::Prepare {
                propose: p_raw.to_vec(),
                accept: raw.clone(),
                consent: consent.to_vec(),
            },
            Some(gate),
            true,
        )?;
        Ok(raw)
    }
    fn converted(&mut self, c: u8, q: Id) -> Result<store::ExistingNative7> {
        let mut head = self.head(c)?;
        let snapshot = self.repository.snapshot(c)?;
        let legacy = PrivateState::open(
            &head,
            self.identity.claim,
            self.device.encryption(),
            snapshot.native.as_deref().context("current Native1")?,
        )?;
        let guard = legacy.guard.as_ref().context("current Guard1")?;
        let role = head.role as usize;
        let mut next = [0; 2];
        let mut tails = [None; 2];
        next[role] = guard.own.next;
        next[1 - role] = guard.peer.next;
        tails[role] = guard.own.last;
        tails[1 - role] = guard.peer.last;
        ensure!(
            next == snapshot.meta.next && tails == snapshot.meta.tails,
            "current Native/readable frontiers"
        );
        let scope = self.scope(c)?;
        let mut lanes: [Vec<Event>; 2] = [Vec::new(), Vec::new()];
        for r in 0..2 {
            ensure!(next[r] <= 512, "current L1 frontier cap");
            for n in 0..next[r] {
                let raw = self
                    .repository
                    .event_at(c, r as u8, n)?
                    .context("current readable source")?;
                let declared = Event::parse(&raw, scope.peers()[r].certificate().device_id())?;
                let author_raw = self
                    .repository
                    .auth(c, declared.core.author_auth)?
                    .context("current readable exact historical author O absent")?;
                let author = v2::ParsedCredentialBundle::parse(&author_raw)?.verify_root()?;
                let event = scope.verify_event(&raw, &author)?;
                ensure!(
                    event.event().core.n == n && event.event().core.role == r as u64,
                    "indexed readable ordinal/role"
                );
                lanes[r].push(event.event().clone());
            }
        }
        let imported = super::super::storage_migration::fold_current_legacy(guard, &lanes)?;
        let (mixed, seen) = MixedNative7::from_legacy(legacy, imported, &head)?;
        head.bump(true)?;
        let native = mixed.seal(&self.identity, &mut head, q, self.device.encryption())?;
        Ok(store::ExistingNative7 {
            head: head.encode()?,
            native,
            seen,
        })
    }
    pub fn select(&mut self, c: u8, a_raw: &[u8], gate: ControlFinalGate<'_>) -> Result<Vec<u8>> {
        ensure!(
            self.head(c)?.role == 0,
            "Select requires original role A before retry lookup"
        );
        self.publish_gate(c, gate)?;
        let scope = self.scope(c)?;
        let accept = v2::ParsedControl2::parse(a_raw)?;
        accept.require_kind(19)?;
        accept.verify(scope.peers()[1].certificate().device_id())?;
        let state = self.state(c)?;
        let p = state.propose.context("durable P")?;
        ensure!(
            accept.body().fields(4)?[0].fixed::<32>()? == scope.c()
                && accept.body().fields(4)?[1].fixed::<32>()? == p,
            "A exact scope/P"
        );
        if let Some(id) = state.select {
            self.conflict(c, state.accept.context("durable A")?, &accept, gate)?;
            return Ok(self.repository.existing_control(c, id)?);
        }
        ensure!(
            self.head(c)?.role == 0 && state.mode == store::ExistingMode::Offering,
            "Select role/stage"
        );
        let k = v2::ParsedControl2::sign(
            20,
            &wire::encode(&a([b(scope.c()), b(p), b(accept.id())]), 4096)?,
            self.device.identity(),
        )?;
        let proof = wire::encode(
            &a([
                u(2),
                u(1),
                b(self.repository.existing_control(c, p)?),
                b(a_raw),
                b(k.raw()),
                b(self
                    .repository
                    .existing_control(c, state.selected_consent.context("selected consent")?)?),
            ]),
            v2::MAX_PROOF,
        )?;
        let selection = scope.selection(v2::ParsedModeProof2::parse(&proof)?)?;
        let p_raw = self.repository.existing_control(c, p)?;
        let p_control = v2::ParsedControl2::parse(&p_raw)?;
        self.repository.check_existing_cuts(
            c,
            selection.proof().proof().cuts(),
            [
                p_control.body().fields(8)?[7].fixed()?,
                accept.body().fields(4)?[3].fixed()?,
            ],
        )?;
        let converted = self.converted(c, selection.proof().q())?;
        self.commit(
            c,
            store::ExistingAction7::Select { proof, converted },
            Some(gate),
            true,
        )?;
        Ok(k.raw().to_vec())
    }
    pub fn commit_b(&mut self, c: u8, k_raw: &[u8], gate: ControlFinalGate<'_>) -> Result<Vec<u8>> {
        ensure!(
            self.head(c)?.role == 1,
            "CommitB requires original role B before retry lookup"
        );
        self.publish_gate(c, gate)?;
        let scope = self.scope(c)?;
        let k = v2::ParsedControl2::parse(k_raw)?;
        k.require_kind(20)?;
        k.verify(scope.peers()[0].certificate().device_id())?;
        let state = self.state(c)?;
        let p = state.propose.context("P")?;
        let accept = state.accept.context("A")?;
        ensure!(
            k.body().fields(3)?[0].fixed::<32>()? == scope.c()
                && k.body().fields(3)?[1].fixed::<32>()? == p,
            "K exact C/P scope"
        );
        if let Some(id) = state.committed {
            self.conflict(c, state.select.context("durable K")?, &k, gate)?;
            return Ok(self.repository.existing_control(c, id)?);
        }
        ensure!(
            k.body() == &a([b(scope.c()), b(p), b(accept)]),
            "K exact A scope"
        );
        ensure!(
            self.head(c)?.role == 1 && state.mode == store::ExistingMode::Prepared,
            "CommitB role/stage"
        );
        let proof = wire::encode(
            &a([
                u(2),
                u(1),
                b(self.repository.existing_control(c, p)?),
                b(self.repository.existing_control(c, accept)?),
                b(k_raw),
                b(self
                    .repository
                    .existing_control(c, state.selected_consent.context("selected consent")?)?),
            ]),
            v2::MAX_PROOF,
        )?;
        let selection = scope.selection(v2::ParsedModeProof2::parse(&proof)?)?;
        let q = selection.proof().q();
        let p_raw = self.repository.existing_control(c, p)?;
        let a_raw = self.repository.existing_control(c, accept)?;
        let p_control = v2::ParsedControl2::parse(&p_raw)?;
        let a_control = v2::ParsedControl2::parse(&a_raw)?;
        self.repository.check_existing_cuts(
            c,
            selection.proof().proof().cuts(),
            [
                p_control.body().fields(8)?[7].fixed()?,
                a_control.body().fields(4)?[3].fixed()?,
            ],
        )?;
        let converted = self.converted(c, q)?;
        let committed = v2::ParsedControl2::sign(
            21,
            &wire::encode(&a([b(scope.c()), b(q)]), 4096)?,
            self.device.identity(),
        )?
        .raw()
        .to_vec();
        self.commit(
            c,
            store::ExistingAction7::CommitB {
                proof,
                converted,
                committed: committed.clone(),
            },
            Some(gate),
            true,
        )?;
        Ok(committed)
    }
    pub fn activate(&mut self, c: u8, raw: &[u8], gate: ControlFinalGate<'_>) -> Result<()> {
        ensure!(
            self.head(c)?.role == 0,
            "Activate requires original role A before retry lookup"
        );
        self.publish_gate(c, gate)?;
        let state = self.state(c)?;
        let q = state.q.context("selected Q")?;
        let scope = self.scope(c)?;
        let proof = self.repository.existing_control(c, q)?;
        let selection = scope.selection(v2::ParsedModeProof2::parse(&proof)?)?;
        let committed = v2::ParsedControl2::parse(raw)?;
        committed.require_kind(21)?;
        committed.verify(scope.peers()[1].certificate().device_id())?;
        ensure!(
            committed.body().fields(2)?[0].fixed::<32>()? == scope.c(),
            "Committed scope"
        );
        if let Some(id) = state.committed {
            self.conflict(c, id, &committed, gate)?;
            return Ok(());
        }
        committed.verify_committed(&selection, &scope.peers()[1])?;
        self.commit(
            c,
            store::ExistingAction7::Activate {
                committed: raw.to_vec(),
            },
            Some(gate),
            true,
        )
    }
    pub fn validate_mixed(&mut self, c: u8) -> Result<()> {
        let state = self.state(c)?;
        ensure!(state.mode.selected(), "local selection required");
        let q = state.q.context("Q")?;
        let scope = self.scope(c)?;
        scope.selection(v2::ParsedModeProof2::parse(
            &self.repository.existing_control(c, q)?,
        )?)?;
        let head = self.head(c)?;
        let snapshot = self.repository.snapshot(c)?;
        let mixed = MixedNative7::open(
            &self.identity,
            &head,
            q,
            self.device.encryption(),
            snapshot
                .native
                .as_deref()
                .context("Mixed7 current native")?,
        )?;
        let role = head.role as usize;
        let mut next = [0; 2];
        let mut tails = [None; 2];
        next[role] = mixed.guard.own().next;
        next[1 - role] = mixed.guard.peer().next;
        tails[role] = mixed.guard.own().last;
        tails[1 - role] = mixed.guard.peer().last;
        ensure!(
            next == snapshot.meta.next && tails == snapshot.meta.tails,
            "Mixed7/metadata exact frontier"
        );
        for (r, lane) in [(role, mixed.guard.own()), (1 - role, mixed.guard.peer())] {
            if let Some(key) = lane.key {
                let raw = self
                    .repository
                    .seen(c, r as u8, key)?
                    .context("Mixed7 current Seen")?;
                ensure!(
                    raw.len() == 40
                        && raw[..8] == lane.g.to_be_bytes()
                        && Some(raw[8..].try_into()?) == lane.first,
                    "Mixed7 current Seen/guard"
                );
            }
        }
        Ok(())
    }
    pub fn validate_saved(&mut self, c: u8) -> Result<()> {
        let state = self.state(c)?;
        let scope = self.scope(c)?;
        for (id, kind, role) in [
            (state.propose, 18, 0),
            (state.accept, 19, 1),
            (state.select, 20, 0),
            (state.committed, 21, 1),
        ] {
            if let Some(id) = id {
                let control = v2::ParsedControl2::parse(&self.repository.existing_control(c, id)?)?;
                control.require_kind(kind)?;
                control.verify(scope.peers()[role].certificate().device_id())?;
            }
        }
        if let Some(id) = state.selected_consent {
            let consent = v2::ParsedConsent2::parse(&self.repository.existing_control(c, id)?)?;
            scope.verify_consent(&consent, scope.peers())?;
        }
        if state.mode.selected() {
            self.validate_mixed(c)?;
        }
        Ok(())
    }
}
