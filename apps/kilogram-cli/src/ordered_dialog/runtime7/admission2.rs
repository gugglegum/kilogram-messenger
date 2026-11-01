//! Existing ALPN2 admission. It binds current permission and the old signed
//! scope without manufacturing a fresh selection or requiring a symmetric Q.
use super::*;
use iroh::endpoint::Connection;
use kilogram_protocol::ordered_dialog_v2 as v2;
use owner::{Lease1, Owner7};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

pub(super) struct Admission7 {
    pub connection: Connection,
    pub binding: Id,
    pub generation: u64,
    pub policy: u64,
    pub deadline: Instant,
    pub opportunity_deadline: Instant,
    pub own_locator: Id,
    pub peer_locator: Id,
    pub hellos: [Option<v2::ParsedFrame2>; 2],
    pub proofs: [Option<v2::ParsedFrame2>; 2],
    pub transcript: Option<Id>,
}

pub(super) fn mode_value(state: &store::ExistingState7) -> Value {
    a([
        u(state.mode.wire()),
        opt(state.q.map(b)),
        a(state.cuts.map(|n| opt(n.map(u)))),
    ])
}
fn shape(role: u8, view: &v2::ModeView2) -> Result<()> {
    let cut = |r: usize| view.cuts[r].is_some_and(|n| (1..=512).contains(&n));
    let valid = match (role, view.stage) {
        (_, 1) => view.q.is_none() && view.cuts == [None, None],
        (0, 2) => view.q.is_none() && cut(0) && view.cuts[1].is_none(),
        (1, 3) => view.q.is_none() && cut(0) && cut(1),
        (0, 4) | (0 | 1, 5) => view.q.is_some() && cut(0) && cut(1),
        _ => false,
    };
    ensure!(valid, "existing ModeView role/optional shape");
    Ok(())
}
fn known(local: &store::ExistingState7, peer: &v2::ModeView2) -> Result<()> {
    for r in 0..2 {
        ensure!(
            local.cuts[r].zip(peer.cuts[r]).is_none_or(|(a, b)| a == b),
            "known mode cut conflict"
        );
    }
    ensure!(
        local.q.zip(peer.q).is_none_or(|(a, b)| a == b),
        "known mode Q conflict"
    );
    ensure!(
        local.peer_q_hint.zip(peer.q).is_none_or(|(a, b)| a == b),
        "opaque Q hint conflict"
    );
    Ok(())
}
pub(super) fn fresh_pair(
    role: u8,
    local: &store::ExistingState7,
    peer: &v2::ModeView2,
) -> Result<()> {
    shape(role, &v2::ModeView2::parse(&mode_value(local))?)?;
    shape(1 - role, peer)?;
    known(local, peer)?;
    let pair = if role == 0 {
        (local.mode.wire(), u64::from(peer.stage))
    } else {
        (u64::from(peer.stage), local.mode.wire())
    };
    ensure!(
        matches!(pair, (1, 1) | (2, 1) | (2, 3) | (4, 3) | (4, 5) | (5, 5)),
        "unreachable fresh existing mode pair"
    );
    Ok(())
}
pub(super) fn live_view(
    role: u8,
    local: &store::ExistingState7,
    lease: &Lease1,
    peer: &v2::ModeView2,
) -> Result<()> {
    shape(1 - role, peer)?;
    known(local, peer)?;
    ensure!(
        lease.peer_q.zip(peer.q).is_none_or(|(a, b)| a == b),
        "live known Q conflict"
    );
    for r in 0..2 {
        ensure!(
            lease.peer_cuts[r]
                .zip(peer.cuts[r])
                .is_none_or(|(a, b)| a == b),
            "live known cut conflict"
        );
    }
    if peer.stage > lease.peer_stage {
        fresh_pair(role, local, peer)?;
    }
    Ok(())
}

impl Owner7 {
    pub(super) fn peer2(&mut self, c: u8, role: u64) -> Result<v2::RootVerifiedCredential> {
        Ok(v2::ParsedCredentialBundle::parse(&self.credentials(c, role)?.raw)?.verify_root()?)
    }
    pub(super) fn scope2(&mut self, c: u8) -> Result<v2::VerifiedExistingScope1> {
        self.existing(c, |controls| controls.scope(c))
    }
    pub(super) fn selection2(&mut self, c: u8) -> Result<v2::StaticSelection2> {
        let state = self.repo()?.existing_state(c)?;
        ensure!(state.mode.selected(), "local selection required");
        let q = state.q.context("local Q absent")?;
        let raw = self.repo()?.existing_control(c, q)?;
        let selection = self
            .scope2(c)?
            .selection(v2::ParsedModeProof2::parse(&raw)?)?;
        ensure!(selection.proof().q() == q, "local selected proof/Q");
        Ok(selection)
    }
    pub(super) fn configure_route(
        &mut self,
        route: kilogram_transport_iroh::ordered2::RouteConfig2,
    ) -> Result<()> {
        use kilogram_transport_iroh::RoutePolicy;
        if route.effective()? != RoutePolicy::DirectOnly {
            for c in self.enabled.clone() {
                ensure!(
                    self.repo()?.existing_state(c)?.mode == store::ExistingMode::Active,
                    "L1RequiresDirectOnly"
                );
            }
        }
        route.effective()?;
        self.route = route;
        Ok(())
    }
    pub(super) fn locator2(&mut self, c: u8, local: bool) -> Result<v2::ParsedRuntimeLocator2> {
        self.current_existing(c)?;
        let raw = self
            .repo()?
            .existing_locator(c, local)?
            .context("Locator2Unavailable")?;
        let locator = v2::ParsedRuntimeLocator2::parse(&raw)?;
        let role = if local {
            self.role(c)?
        } else {
            1 - self.role(c)?
        };
        locator.verify(
            self.spec(c)?.c,
            &self.peer2(c, role)?,
            &self.peer2(c, 1 - role)?,
        )?;
        let wall = now()?;
        ensure!(
            locator.fields().not_before <= wall && wall < locator.fields().expires,
            "Locator2 interval"
        );
        self.route.remote(&locator)?;
        Ok(locator)
    }
    pub(super) fn import_locator2(&mut self, c: u8, raw: Vec<u8>) -> Result<()> {
        self.current_existing(c)?;
        let locator = v2::ParsedRuntimeLocator2::parse(&raw)?;
        let role = self.role(c)?;
        locator.verify(
            self.spec(c)?.c,
            &self.peer2(c, 1 - role)?,
            &self.peer2(c, role)?,
        )?;
        let wall = now()?;
        ensure!(
            locator.fields().not_before <= wall && wall < locator.fields().expires,
            "Locator2 interval"
        );
        self.route.remote(&locator)?;
        let state = self.repo()?.existing_state(c)?;
        if state.locator_revision[1] == locator.fields().revision {
            ensure!(
                self.repo()?.existing_locator(c, false)?.as_deref() == Some(raw.as_slice()),
                "Locator2 revision conflict"
            );
            return Ok(());
        }
        self.invalidate(c);
        self.existing_local(
            c,
            store::ExistingAction7::Locator {
                local: false,
                revision: locator.fields().revision,
                raw,
            },
        )?;
        self.wake.notify_waiters();
        Ok(())
    }
    pub(super) fn publish_locator2(&mut self, c: u8) -> Result<Vec<u8>> {
        use kilogram_transport_iroh::RoutePolicy;
        self.current_existing(c)?;
        let address = self
            .endpoint
            .clone()
            .context("Locator2 endpoint unavailable")?;
        let policy = self.route.effective()?;
        let relay = if policy == RoutePolicy::DirectOnly {
            None
        } else {
            self.route.relay.clone()
        };
        ensure!(
            address
                .relay_urls()
                .all(|url| Some(url.as_str()) == relay.as_deref()),
            "actual relay snapshot"
        );
        let ips = if policy == RoutePolicy::RelayOnly {
            ensure!(
                address.ip_addrs().next().is_none(),
                "RelayOnly advertised IP"
            );
            Vec::new()
        } else {
            address.ip_addrs().copied().collect::<Vec<_>>()
        };
        let wall = now()?;
        let role = self.role(c)?;
        let prior_revision = self.repo()?.existing_state(c)?.locator_revision[0];
        if let Some(raw) = self.repo()?.existing_locator(c, true)? {
            // Expiry or a deliberate route change may require publication, but
            // corrupt stored bytes must not be hidden by rotating the locator.
            let old = v2::ParsedRuntimeLocator2::parse(&raw)?;
            old.verify(
                self.spec(c)?.c,
                &self.peer2(c, role)?,
                &self.peer2(c, 1 - role)?,
            )?;
            ensure!(
                old.fields().revision == prior_revision,
                "stored Locator2 revision"
            );
            let same_ips = old
                .fields()
                .ips
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                == ips.iter().copied().collect();
            if old.fields().endpoint == *address.id.as_bytes()
                && same_ips
                && old.fields().relay == relay
                && old.fields().not_before <= wall
                && old.fields().expires > wall.saturating_add(60)
            {
                return Ok(old.raw().to_vec());
            }
        } else {
            ensure!(
                prior_revision == 0,
                "stored Locator2 absent at nonzero revision"
            );
        }
        let revision = prior_revision
            .checked_add(1)
            .context("Locator2 revision exhausted")?;
        let locator = v2::ParsedRuntimeLocator2::sign(
            v2::RuntimeLocatorFields2 {
                c: self.spec(c)?.c,
                cert: self.peer2(c, role)?.cert_ref(),
                peer_cert: self.peer2(c, 1 - role)?.cert_ref(),
                endpoint: *address.id.as_bytes(),
                ips,
                relay,
                revision,
                not_before: wall,
                expires: wall.checked_add(3600).context("Locator2 expiry")?,
            },
            self.device.identity(),
        )?;
        self.invalidate(c);
        self.existing_local(
            c,
            store::ExistingAction7::Locator {
                local: true,
                revision,
                raw: locator.raw().to_vec(),
            },
        )?;
        Ok(locator.raw().to_vec())
    }
    fn existing_local(
        &mut self,
        c: u8,
        action: store::ExistingAction7,
    ) -> Result<db::CommitReport> {
        let expires = self.current_existing(c)?;
        let floor = now()?;
        let op = store::ExistingOperation7 {
            context: c,
            expected_root: self.repo()?.generation(),
            expected_head: wire::hash(b"", &self.repo()?.head(c)?),
            policy_generation: self.repo()?.metadata(c)?.policy_generation,
            wall_time: floor,
            action,
        };
        let result = self
            .repo()?
            .commit_existing(
                op,
                Some(&mut || {
                    if now().map_or(true, |n| n < floor || n >= expires) {
                        return Err(db::Error::PolicyRejected("existing local permission/clock"));
                    }
                    Ok(())
                }),
            )
            .map_err(anyhow::Error::from);
        if !self.repo()?.requires_reopen() {
            self.sync_high_water()?;
        }
        result
    }
    pub(super) fn check_existing_lease(&mut self, c: u8, binding: Id) -> Result<Lease1> {
        self.current_existing(c)?;
        let lease = self.leases[usize::from(c)]
            .clone()
            .context("NeedExistingAdmission")?;
        ensure!(
            lease.family == 2
                && lease.binding == binding
                && lease.policy_generation == self.repo()?.metadata(c)?.policy_generation,
            "stale existing lease/policy"
        );
        lease.gate()?;
        if self.repo()?.existing_state(c)?.mode == store::ExistingMode::L1 {
            ensure!(
                lease
                    .opportunity_deadline
                    .is_some_and(|deadline| Instant::now() < deadline),
                "pre-P opportunity expired"
            );
        }
        let own = self.locator2(c, true)?;
        let peer = self.locator2(c, false)?;
        ensure!(
            own.id() == lease.own_locator
                && peer.id() == lease.peer_locator
                && lease.connection.remote_id().as_bytes() == &peer.fields().endpoint,
            "stale existing locator/endpoint"
        );
        ensure!(
            self.route.connection_policy(&peer)? == lease.route,
            "existing route changed"
        );
        Ok(lease)
    }
    pub(super) fn signed2(
        &self,
        kind: v2::FrameKind,
        binding: Id,
        request: Option<[u8; 16]>,
        body: Value,
    ) -> Result<v2::ParsedFrame2> {
        Ok(v2::ParsedFrame2::sign(
            v2::FrameBody2::parse(kind, &wire::encode(&body, v2::MAX_WIRE - 4)?)?,
            binding,
            request.unwrap_or(admin::random()?),
            self.device.identity(),
        )?)
    }
    pub(super) fn begin2(
        &mut self,
        c: u8,
        connection: Connection,
        binding: Id,
        deadline: Instant,
        opportunity_deadline: Instant,
        request: Option<[u8; 16]>,
    ) -> Result<(u64, v2::ParsedFrame2)> {
        self.current_existing(c)?;
        let scope = self.scope2(c)?;
        ensure!(
            connection.alpn() == v2::ALPN
                && Instant::now() < deadline
                && connection.close_reason().is_none(),
            "existing ALPN/deadline"
        );
        if let Some(old) = &self.admissions[usize::from(c)] {
            ensure!(
                old.connection.close_reason().is_some() || Instant::now() >= old.deadline,
                "context authenticating"
            );
        }
        if let Some(old) = &self.leases[usize::from(c)] {
            ensure!(
                old.connection.close_reason().is_some() || Instant::now() >= old.deadline,
                "context already admitted"
            );
        }
        self.invalidate(c);
        let own = self.locator2(c, true)?;
        let peer = self.locator2(c, false)?;
        ensure!(
            self.endpoint
                .as_ref()
                .is_some_and(|address| address.id.as_bytes() == &own.fields().endpoint)
                && connection.remote_id().as_bytes() == &peer.fields().endpoint,
            "actual TLS locator endpoint"
        );
        let role = self.role(c)? as usize;
        let current = self
            .repo()?
            .existing_state(c)?
            .current_consent
            .context("current existing consent")?;
        let generation = self.next_connection;
        self.next_connection = generation
            .checked_add(1)
            .context("connection generation exhausted")?;
        let credential = self.peer2(c, role as u64)?.raw().to_vec();
        let hello = self.signed2(
            v2::FrameKind::Hello,
            binding,
            request,
            a([
                b(scope.c()),
                u(role as u64),
                b(own.fields().endpoint),
                b(peer.fields().endpoint),
                b(admin::random::<32>()?),
                a([u(1)]),
                a([u(2)]),
                b(credential),
                b(scope.membership()),
                b(current),
                b(own.id()),
                scope.origin().value(),
            ]),
        )?;
        let mut hellos = [None, None];
        hellos[role] = Some(hello.clone());
        self.admissions[usize::from(c)] = Some(Admission7 {
            connection,
            binding,
            generation,
            policy: self.repo()?.metadata(c)?.policy_generation,
            deadline,
            opportunity_deadline,
            own_locator: own.id(),
            peer_locator: peer.id(),
            hellos,
            proofs: [None, None],
            transcript: None,
        });
        Ok((generation, hello))
    }
    fn admission2(&mut self, c: u8, generation: u64) -> Result<&Admission7> {
        self.current_existing(c)?;
        let policy = self.repo()?.metadata(c)?.policy_generation;
        let own = self.locator2(c, true)?.id();
        let peer = self.locator2(c, false)?.id();
        let admission = self.admissions[usize::from(c)]
            .as_ref()
            .context("existing admission missing")?;
        ensure!(
            admission.generation == generation
                && admission.policy == policy
                && admission.own_locator == own
                && admission.peer_locator == peer
                && Instant::now() < admission.deadline
                && admission.connection.close_reason().is_none(),
            "stale existing admission"
        );
        Ok(admission)
    }
    pub(super) fn observe_hello2(
        &mut self,
        c: u8,
        binding: Id,
        frame: &v2::ParsedFrame2,
    ) -> Result<()> {
        ensure!(frame.body().kind() == v2::FrameKind::Hello, "Hello2 first");
        let f = frame.body().value().fields(12)?;
        let credential = v2::ParsedCredentialBundle::parse(f[7].bytes()?)?.verify_root()?;
        let peer = 1 - self.role(c)?;
        ensure!(
            f[0].fixed::<32>()? == self.spec(c)?.c
                && f[1].uint()? == peer
                && credential.cert_ref() == self.spec(c)?.cert_refs()?[peer as usize],
            "Hello2 original certificate/C pin"
        );
        frame
            .clone()
            .verify(binding, credential.certificate().device_id())?;
        self.pin_authority(&credential.authority().encode()?)
    }
    pub(super) fn accept_hello2(
        &mut self,
        c: u8,
        generation: u64,
        frame: v2::ParsedFrame2,
    ) -> Result<()> {
        let binding = self.admission2(c, generation)?.binding;
        let scope = self.scope2(c)?;
        let peer = 1 - self.role(c)? as usize;
        frame.clone().verify(
            binding,
            self.peer2(c, peer as u64)?.certificate().device_id(),
        )?;
        ensure!(frame.body().kind() == v2::FrameKind::Hello, "Hello2 first");
        let f = frame.body().value().fields(12)?;
        let own = self.locator2(c, true)?;
        let remote = self.locator2(c, false)?;
        ensure!(
            f[0].fixed::<32>()? == scope.c()
                && f[1].uint()? == peer as u64
                && f[2].fixed::<32>()? == remote.fields().endpoint
                && f[3].fixed::<32>()? == own.fields().endpoint
                && f[7].bytes()? == self.peer2(c, peer as u64)?.raw()
                && f[8].bytes()? == scope.membership()
                && Some(f[9].fixed::<32>()?) == self.repo()?.existing_state(c)?.current_consent
                && f[10].fixed::<32>()? == remote.id()
                && v2::Origin::parse(&f[11])? == scope.origin(),
            "Hello2 existing scope/current pins"
        );
        let pending = self.admissions[usize::from(c)]
            .as_mut()
            .context("existing admission")?;
        ensure!(pending.hellos[peer].is_none(), "duplicate Hello2");
        pending.hellos[peer] = Some(frame);
        pending.transcript = Some(v2::initial_transcript2(
            binding,
            pending.hellos[0].as_ref().context("A Hello2")?.raw(),
            pending.hellos[1].as_ref().context("B Hello2")?.raw(),
        )?);
        Ok(())
    }
    pub(super) fn proof2(&mut self, c: u8, generation: u64) -> Result<v2::ParsedFrame2> {
        let role = self.role(c)? as usize;
        let scope = self.scope2(c)?;
        let p = self.admission2(c, generation)?;
        if let Some(frame) = &p.proofs[role] {
            return Ok(frame.clone());
        }
        let request = if role == 1 {
            Some(p.proofs[0].as_ref().context("A Proof2")?.request())
        } else {
            None
        };
        let binding = p.binding;
        let t = p.transcript.context("Hello2 transcript")?;
        let frame = self.signed2(
            v2::FrameKind::Proof,
            binding,
            request,
            v2::FrameBody2::proof(t, scope.c(), scope.certs())?
                .value()
                .clone(),
        )?;
        self.admissions[usize::from(c)]
            .as_mut()
            .context("existing admission")?
            .proofs[role] = Some(frame.clone());
        Ok(frame)
    }
    pub(super) fn accept_proof2(
        &mut self,
        c: u8,
        generation: u64,
        frame: v2::ParsedFrame2,
    ) -> Result<()> {
        let scope = self.scope2(c)?;
        let peer = 1 - self.role(c)? as usize;
        let p = self.admission2(c, generation)?;
        let binding = p.binding;
        let t = p.transcript.context("Hello2 transcript")?;
        frame.clone().verify(
            binding,
            self.peer2(c, peer as u64)?.certificate().device_id(),
        )?;
        ensure!(
            frame.body().kind() == v2::FrameKind::Proof
                && frame.body().value()
                    == v2::FrameBody2::proof(t, scope.c(), scope.certs())?.value(),
            "Proof2 exact existing transcript"
        );
        let p = self.admissions[usize::from(c)]
            .as_mut()
            .context("existing admission")?;
        ensure!(p.proofs[peer].is_none(), "duplicate Proof2");
        p.proofs[peer] = Some(frame);
        Ok(())
    }
    pub(super) fn admitted2(
        &mut self,
        c: u8,
        generation: u64,
        request: Option<[u8; 16]>,
    ) -> Result<v2::ParsedFrame2> {
        let context = self.spec(c)?.c;
        let p = self.admission2(c, generation)?;
        ensure!(p.proofs.iter().all(Option::is_some), "both Proof2 required");
        let binding = p.binding;
        let t = p.transcript.context("Hello2 transcript")?;
        self.signed2(
            v2::FrameKind::Admitted,
            binding,
            request,
            a([b(t), b(context), u(1), u(2)]),
        )
    }
    pub(super) fn verify_admitted2(
        &mut self,
        c: u8,
        generation: u64,
        frame: &v2::ParsedFrame2,
    ) -> Result<()> {
        let context = self.spec(c)?.c;
        let peer = 1 - self.role(c)?;
        let p = self.admission2(c, generation)?;
        let binding = p.binding;
        let t = p.transcript.context("Hello2 transcript")?;
        ensure!(p.proofs.iter().all(Option::is_some), "both Proof2 required");
        frame
            .clone()
            .verify(binding, self.peer2(c, peer)?.certificate().device_id())?;
        ensure!(
            frame.body().kind() == v2::FrameKind::Admitted
                && frame.body().value() == &a([b(t), b(context), u(1), u(2)]),
            "Admitted2 transcript"
        );
        Ok(())
    }
    pub(super) fn finish2(
        &mut self,
        c: u8,
        generation: u64,
        head: v2::ParsedFrame2,
    ) -> Result<Lease1> {
        let role = self.role(c)?;
        let p = self.admission2(c, generation)?;
        let binding = p.binding;
        head.clone()
            .verify(binding, self.peer2(c, 1 - role)?.certificate().device_id())?;
        ensure!(
            head.body().kind() == v2::FrameKind::Head,
            "existing admission requires Head2"
        );
        let view = self.validate_head2(c, head.body().value(), None)?;
        let state = self.repo()?.existing_state(c)?;
        fresh_pair(role as u8, &state, &view)?;
        let own = self.locator2(c, true)?;
        let peer = self.locator2(c, false)?;
        let expires = now()?
            .checked_add(300)
            .context("existing lease expiry")?
            .min(self.current_existing(c)?)
            .min(own.fields().expires)
            .min(peer.fields().expires);
        let peer_credential = self.peer2(c, 1 - role)?.raw().to_vec();
        let route = self.route.connection_policy(&peer)?;
        let high_water = self.high_water[usize::from(c)].clone();
        let p = self.admission2(c, generation)?;
        let lease = Lease1 {
            family: 2,
            route,
            peer_stage: view.stage,
            peer_q: view.q,
            peer_cuts: view.cuts,
            connection: p.connection.clone(),
            binding,
            generation,
            opportunity_deadline: Some(
                (Instant::now() + control::PROBE_QUIET7).min(p.opportunity_deadline),
            ),
            high_water,
            policy_generation: p.policy,
            expires,
            deadline: Instant::now() + Duration::from_secs(expires.saturating_sub(now()?)),
            own_locator: own.id(),
            peer_locator: peer.id(),
            peer_credential,
            valid: Arc::new(AtomicBool::new(true)),
        };
        lease.gate()?;
        self.admissions[usize::from(c)] = None;
        self.leases[usize::from(c)] = Some(lease.clone());
        self.stats.admissions += 1;
        self.accept_head2(c, binding, head.body().value(), true)?;
        Ok(lease)
    }
}
