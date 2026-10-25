use super::*;
use iroh::endpoint::Connection;
use owner::{Lease2, Owner2};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

pub(super) struct Admission2 {
    pub connection: Connection,
    pub binding: Id,
    pub generation: u64,
    pub policy: u64,
    pub deadline: Instant,
    pub hellos: [Option<wire::ParsedFrame2>; 2],
    pub proofs: [Option<wire::ParsedFrame2>; 2],
    pub transcript: Option<Id>,
}
#[derive(Clone)]
pub(super) struct Link2 {
    pub c: u8,
    pub binding: Id,
    pub generation: u64,
    pub valid: Arc<AtomicBool>,
    pub deadline: Instant,
}
impl Owner2 {
    pub(super) fn route_for_remote(
        &mut self,
        remote: Id,
    ) -> Result<kilogram_transport_iroh::RoutePolicy> {
        let mut policy = None;
        for c in self.enabled.clone() {
            if !self.repository.installed(c)? {
                continue;
            }
            let s = self.snapshot(c)?;
            if let Some(raw) = s.spec.peer_locator {
                let locator = wire::ParsedRuntimeLocator2::parse(&raw)?;
                if locator.fields().endpoint == remote {
                    let current = self.route.connection_policy(&locator)?;
                    ensure!(
                        policy.is_none_or(|old| old == current),
                        "inconsistent peer route policies"
                    );
                    policy = Some(current);
                }
            }
        }
        policy.context("actual TLS endpoint is absent from current locators")
    }
    pub(super) fn begin_admission(
        &mut self,
        c: u8,
        connection: Connection,
        binding: Id,
        deadline: Instant,
        request: Option<[u8; 16]>,
    ) -> Result<(u64, wire::ParsedFrame2)> {
        let (s, current) = self.require(c, false)?;
        ensure!(
            Instant::now() < deadline && connection.close_reason().is_none(),
            "admission deadline/connection"
        );
        ensure!(self.endpoint.is_some(), "LocatorUnavailable");
        let own = wire::ParsedRuntimeLocator2::parse(
            s.spec.own_locator.as_deref().context("own locator")?,
        )?;
        let peer = wire::ParsedRuntimeLocator2::parse(
            s.spec.peer_locator.as_deref().context("peer locator")?,
        )?;
        let wall = now()?;
        for (locator, role) in [(&own, s.head.role), (&peer, 1 - s.head.role)] {
            locator.verify(
                s.head.context,
                &current.peers[usize::from(role)],
                &current.peers[usize::from(1 - role)],
            )?;
            ensure!(
                locator.fields().not_before <= wall && wall < locator.fields().expires,
                "locator interval"
            );
        }
        ensure!(
            self.endpoint
                .as_ref()
                .is_some_and(|(_, addr)| addr.id.as_bytes() == &own.fields().endpoint)
                && connection.remote_id().as_bytes() == &peer.fields().endpoint,
            "actual TLS locator endpoint"
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
        if let Some(old) = self.leases[usize::from(c)].take() {
            old.invalidate();
        }
        let generation = self.next_connection;
        self.next_connection = self
            .next_connection
            .checked_add(1)
            .context("connection generation")?;
        let body = wire::FrameBody2::parse(
            wire::FrameKind::Hello,
            &wire::encode(
                &a([
                    b(s.head.context),
                    u(u64::from(s.head.role)),
                    b(own.fields().endpoint),
                    b(peer.fields().endpoint),
                    b(shared_admin::random::<32>()?),
                    a([u(1)]),
                    a([u(2)]),
                    b(current.peers[usize::from(s.head.role)].raw()),
                    b(&s.spec.membership),
                    b(current.consent.reference()),
                    b(own.id()),
                    wire::Origin::Fresh.value(),
                ]),
                96 * 1024,
            )?,
        )?;
        let hello = wire::ParsedFrame2::sign(
            body,
            binding,
            request.unwrap_or(shared_admin::random()?),
            &self.device,
        )?;
        let mut hellos = [None, None];
        hellos[usize::from(s.head.role)] = Some(hello.clone());
        self.admissions[usize::from(c)] = Some(Admission2 {
            connection,
            binding,
            generation,
            policy: s.profile.policy_generation,
            deadline,
            hellos,
            proofs: [None, None],
            transcript: None,
        });
        Ok((generation, hello))
    }
    fn admission(&self, c: u8, generation: u64) -> Result<&Admission2> {
        let pending = self.admissions[usize::from(c)]
            .as_ref()
            .context("admission missing")?;
        ensure!(
            pending.generation == generation
                && Instant::now() < pending.deadline
                && pending.connection.close_reason().is_none(),
            "admission stale/deadline"
        );
        Ok(pending)
    }
    pub(super) fn accept_hello(
        &mut self,
        c: u8,
        generation: u64,
        frame: wire::ParsedFrame2,
    ) -> Result<()> {
        let binding = self.admission(c, generation)?.binding;
        self.observe_hello_authority(c, binding, &frame)?;
        let (s, current) = self.require(c, false)?;
        let p = self.admission(c, generation)?;
        ensure!(
            p.policy == s.profile.policy_generation,
            "admission policy changed"
        );
        ensure!(frame.body().kind() == wire::FrameKind::Hello, "Hello first");
        let f = frame.body().value().fields(12)?;
        let credential = super::super::handshake::Credentials::decode(f[7].bytes()?)?;
        frame
            .clone()
            .verify(p.binding, credential.cert.device_id())?;
        let own = wire::ParsedRuntimeLocator2::parse(
            s.spec.own_locator.as_deref().context("own locator")?,
        )?;
        let peer = wire::ParsedRuntimeLocator2::parse(
            s.spec.peer_locator.as_deref().context("peer locator")?,
        )?;
        ensure!(
            f[0].fixed::<32>()? == s.head.context
                && f[1].uint()? == u64::from(1 - s.head.role)
                && f[2].fixed::<32>()? == peer.fields().endpoint
                && f[3].fixed::<32>()? == own.fields().endpoint
                && f[8].bytes()? == s.spec.membership
                && f[9].fixed::<32>()? == current.consent.reference()
                && f[10].fixed::<32>()? == peer.id()
                && wire::Origin::parse(&f[11])? == wire::Origin::Fresh
                && credential.raw == current.peers[usize::from(1 - s.head.role)].raw(),
            "Hello scope/pins/current permission"
        );
        ensure!(
            p.hellos[usize::from(1 - s.head.role)].is_none(),
            "duplicate Hello round"
        );
        let p = self.admissions[usize::from(c)]
            .as_mut()
            .context("admission")?;
        p.hellos[usize::from(1 - s.head.role)] = Some(frame);
        let t = wire::initial_transcript2(
            p.binding,
            p.hellos[0].as_ref().context("A Hello")?.raw(),
            p.hellos[1].as_ref().context("B Hello")?.raw(),
        )?;
        p.transcript = Some(t);
        Ok(())
    }
    pub(super) fn observe_hello_authority(
        &mut self,
        c: u8,
        binding: Id,
        frame: &wire::ParsedFrame2,
    ) -> Result<()> {
        let s = self.snapshot(c)?;
        ensure!(frame.body().kind() == wire::FrameKind::Hello, "Hello first");
        let f = frame.body().value().fields(12)?;
        let credential = super::super::handshake::Credentials::decode(f[7].bytes()?)?;
        ensure!(
            f[0].fixed::<32>()? == s.head.context
                && f[1].uint()? == u64::from(1 - s.head.role)
                && credential.cert_ref()? == s.spec.certs[usize::from(1 - s.head.role)],
            "Hello certificate/context pin"
        );
        frame.clone().verify(binding, credential.cert.device_id())?;
        // Retain a valid newer Root revocation even if this context is busy or blocked.
        self.import_authority(credential.authority.encode()?)?;
        Ok(())
    }
    fn admission_current(
        &self,
        c: u8,
        generation: u64,
        s: &db::RuntimeSnapshot,
    ) -> Result<&Admission2> {
        let p = self.admission(c, generation)?;
        ensure!(
            p.policy == s.profile.policy_generation,
            "admission policy changed"
        );
        for role in 0..2 {
            if let Some(hello) = &p.hellos[role] {
                let raw = if role == usize::from(s.head.role) {
                    s.spec.own_locator.as_deref()
                } else {
                    s.spec.peer_locator.as_deref()
                };
                let locator =
                    wire::ParsedRuntimeLocator2::parse(raw.context("admission locator missing")?)?;
                ensure!(
                    hello.body().value().fields(12)?[10].fixed::<32>()? == locator.id(),
                    "admission locator changed"
                );
            }
        }
        Ok(p)
    }
    pub(super) fn admission_proof(&mut self, c: u8, generation: u64) -> Result<wire::ParsedFrame2> {
        let (s, _) = self.require(c, false)?;
        let p = self.admission_current(c, generation, &s)?;
        if let Some(proof) = &p.proofs[usize::from(s.head.role)] {
            return Ok(proof.clone());
        }
        let request = if s.head.role == 1 {
            p.proofs[0].as_ref().context("A Proof")?.request()
        } else {
            shared_admin::random()?
        };
        let frame = wire::ParsedFrame2::sign(
            wire::FrameBody2::proof(
                p.transcript.context("transcript")?,
                s.head.context,
                s.spec.certs,
            )?,
            p.binding,
            request,
            &self.device,
        )?;
        self.admissions[usize::from(c)]
            .as_mut()
            .context("admission")?
            .proofs[usize::from(s.head.role)] = Some(frame.clone());
        Ok(frame)
    }
    pub(super) fn accept_proof(
        &mut self,
        c: u8,
        generation: u64,
        frame: wire::ParsedFrame2,
    ) -> Result<()> {
        let (s, current) = self.require(c, false)?;
        let p = self.admission_current(c, generation, &s)?;
        frame.clone().verify(
            p.binding,
            current.peers[usize::from(1 - s.head.role)]
                .certificate()
                .device_id(),
        )?;
        ensure!(
            frame.body().kind() == wire::FrameKind::Proof
                && frame.body().value()
                    == &wire::FrameBody2::proof(
                        p.transcript.context("transcript")?,
                        s.head.context,
                        s.spec.certs
                    )?
                    .value()
                    .clone(),
            "Proof transcript/certificates"
        );
        self.admissions[usize::from(c)]
            .as_mut()
            .context("admission")?
            .proofs[usize::from(1 - s.head.role)] = Some(frame);
        Ok(())
    }
    pub(super) fn admitted_frame(
        &mut self,
        c: u8,
        generation: u64,
        request: Option<[u8; 16]>,
    ) -> Result<wire::ParsedFrame2> {
        let (s, _) = self.require(c, false)?;
        let p = self.admission_current(c, generation, &s)?;
        ensure!(p.proofs.iter().all(Option::is_some), "both proofs required");
        let body = wire::FrameBody2::parse(
            wire::FrameKind::Admitted,
            &wire::encode(
                &a([
                    b(p.transcript.context("transcript")?),
                    b(s.head.context),
                    u(1),
                    u(2),
                ]),
                256,
            )?,
        )?;
        Ok(wire::ParsedFrame2::sign(
            body,
            p.binding,
            request.unwrap_or(shared_admin::random()?),
            &self.device,
        )?)
    }
    pub(super) fn accept_admitted(
        &mut self,
        c: u8,
        generation: u64,
        frame: wire::ParsedFrame2,
    ) -> Result<Link2> {
        let (s, current) = self.require(c, false)?;
        let p = self.admission_current(c, generation, &s)?;
        frame.clone().verify(
            p.binding,
            current.peers[usize::from(1 - s.head.role)]
                .certificate()
                .device_id(),
        )?;
        let t = p.transcript.context("transcript")?;
        ensure!(
            frame.body().kind() == wire::FrameKind::Admitted
                && frame.body().value() == &a([b(t), b(s.head.context), u(1), u(2)])
                && p.proofs.iter().all(Option::is_some),
            "Admitted transcript"
        );
        let own = wire::ParsedRuntimeLocator2::parse(
            s.spec.own_locator.as_deref().context("own locator")?,
        )?;
        let peer = wire::ParsedRuntimeLocator2::parse(
            s.spec.peer_locator.as_deref().context("peer locator")?,
        )?;
        let wall = now()?;
        let expires = wall
            .checked_add(hooks::load()?.lease_seconds.unwrap_or(300))
            .context("lease expiry")?
            .min(current.consent.interval().1)
            .min(own.fields().expires)
            .min(peer.fields().expires);
        ensure!(wall < expires, "lease interval");
        let pair = wire::encode(
            &a([
                u(2),
                b(t),
                b(p.hellos[0].as_ref().context("A Hello")?.raw()),
                b(p.hellos[1].as_ref().context("B Hello")?.raw()),
                b(p.proofs[0].as_ref().context("A Proof")?.raw()),
                b(p.proofs[1].as_ref().context("B Proof")?.raw()),
            ]),
            192 * 1024,
        )?;
        let lease = Lease2 {
            connection: p.connection.clone(),
            binding: p.binding,
            generation,
            policy: s.profile.policy_generation,
            deadline: Instant::now() + Duration::from_secs(expires - wall),
            wall_deadline: expires,
            own_locator: own.id(),
            peer_locator: peer.id(),
            valid: Arc::new(AtomicBool::new(true)),
            pair,
        };
        lease.check(&s, wall, &self.route)?;
        let link = Link2 {
            c,
            binding: lease.binding,
            generation,
            valid: lease.valid.clone(),
            deadline: lease.deadline,
        };
        self.admissions[usize::from(c)] = None;
        self.leases[usize::from(c)] = Some(lease);
        self.stats.admissions += 1;
        eprintln!(
            "od2_admitted context={} binding={} generation={generation}",
            hex(&s.head.context),
            hex(&link.binding)
        );
        Ok(link)
    }
}
