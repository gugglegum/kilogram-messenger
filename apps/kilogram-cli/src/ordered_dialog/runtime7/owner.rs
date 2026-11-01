use super::*;
use iroh::{EndpointAddr, endpoint::Connection};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, DeviceCapability, DeviceCertificate, DeviceState,
    verify_device_authorization_with_snapshot,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Lease1 {
    pub family: u8,
    pub route: kilogram_transport_iroh::RoutePolicy,
    pub peer_stage: u8,
    pub peer_q: Option<Id>,
    pub peer_cuts: [Option<u64>; 2],
    pub opportunity_deadline: Option<Instant>,
    pub high_water: Arc<AtomicU64>,
    pub connection: Connection,
    pub binding: Id,
    pub generation: u64,
    pub policy_generation: u64,
    pub expires: u64,
    pub deadline: Instant,
    pub own_locator: Id,
    pub peer_locator: Id,
    pub peer_credential: Vec<u8>,
    pub valid: Arc<AtomicBool>,
}
impl Lease1 {
    pub fn gate(&self) -> db::Result<()> {
        if !self.valid.load(Ordering::Acquire)
            || self.connection.close_reason().is_some()
            || Instant::now() >= self.deadline
            || now().map_or(true, |n| {
                n >= self.expires || n < self.high_water.load(Ordering::Acquire)
            })
        {
            return Err(db::Error::PolicyRejected("closed/expired L1 lease"));
        }
        let paths = self.connection.paths();
        let allowed = paths
            .iter()
            .filter(|p| p.is_selected())
            .any(|p| match self.route {
                kilogram_transport_iroh::RoutePolicy::DirectOnly => p.is_ip(),
                kilogram_transport_iroh::RoutePolicy::RelayOnly => p.is_relay(),
                kilogram_transport_iroh::RoutePolicy::Auto => p.is_ip() || p.is_relay(),
            });
        if !allowed {
            return Err(db::Error::PolicyRejected(
                "L1 requires actual direct IP path",
            ));
        }
        Ok(())
    }
}
#[derive(Default, serde::Serialize)]
pub(super) struct Stats7 {
    #[cfg(test)]
    #[serde(skip)]
    pub worker_head_progress: [Option<(Id, u64)>; 2],
    pub open_intent_received: u64,
    pub packet_attempts: u64,
    pub committed_encrypt: u64,
    pub committed_decrypt: u64,
    pub aborted_packet_calls: u64,
    pub ambiguous_packet_calls: u64,
    pub account_constructors: u64,
    pub outbound_constructors: u64,
    pub inbound_constructors: u64,
    pub materialized: u64,
    pub readable: u64,
    pub reopens: u64,
    pub max_native_bytes: usize,
    pub admissions: u64,
    pub max_commit_app_bytes: u64,
}
pub(super) struct Owner7 {
    pub repository: Option<store::L1Repository>,
    failed_lock: Option<kilogram_state::StateDirectoryLock>,
    pub device: DeviceState,
    pub nonce: Id,
    pub account: AccountId,
    pub specs: Vec<Spec>,
    pub enabled: Vec<u8>,
    pub leases: [Option<Lease1>; 2],
    pub endpoint: Option<EndpointAddr>,
    pub publication_errors: [[Option<String>; 2]; 2],
    pub origin_error: [Option<String>; 2],
    pub origin_ready: [bool; 2],
    pub stats: Stats7,
    pub resources: Arc<resources::Resources7>,
    pub window: Arc<window::RefWindow2>,
    pub wake: Arc<tokio::sync::Notify>,
    pub(super) native_cache: [Option<(Id, native::Native7)>; 2],
    pub route: kilogram_transport_iroh::ordered2::RouteConfig2,
    pub(super) admissions: [Option<super::admission2::Admission7>; 2],
    pub(super) next_connection: u64,
    pub(super) mixed_identity: native::MixedIdentity7,
    pub(super) high_water: [Arc<AtomicU64>; 2],
    current_cache: [Option<(u64, u64, u64)>; 2],
    // At most the three already pinned accounts; public snapshots only.
    // Failed persistence cannot revive permission from an older durable pin.
    pending_authorities: BTreeMap<Id, (u64, Vec<u8>)>,
}
impl Owner7 {
    pub fn repo(&mut self) -> Result<&mut store::L1Repository> {
        self.repository.as_mut().context("owner recovery failed")
    }
    pub fn new(mut repository: store::L1Repository, enabled_ids: &[Id]) -> Result<Self> {
        let material = repository.owner_material()?;
        let device = DeviceState::from_secret_material(
            repository.root(),
            *material.signing,
            *material.encryption,
        );
        let cert = DeviceCertificate::decode_and_verify(&material.certificate)?;
        ensure!(
            cert.account_id().as_bytes() == &material.account
                && cert.device_id() == device.identity().device_id()
                && cert.encryption_public_key() == device.encryption().public_key(),
            "original Device/Root7 certificate binding"
        );
        let contexts = repository.contexts().to_vec();
        let mut enabled = Vec::new();
        for id in enabled_ids {
            enabled.push(
                contexts
                    .iter()
                    .position(|(c, _)| c == id)
                    .context("enabled C not registered")? as u8,
            );
        }
        let mut specs = Vec::new();
        let mut accounts = Vec::new();
        let mut pins = BTreeMap::new();
        for (c, (context, role)) in contexts.iter().enumerate() {
            let spec = Spec::parse(&repository.spec(c as u8)?)?;
            ensure!(
                spec.c == *context && spec.role(device.identity().device_id())? == u64::from(*role),
                "indexed immutable Spec/role"
            );
            let pair = [
                *spec.peers[0].cert.account_id().as_bytes(),
                *spec.peers[1].cert.account_id().as_bytes(),
            ];
            for account in pair {
                let raw = repository.authority(account)?;
                let snapshot = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
                ensure!(
                    snapshot.account_id().as_bytes() == &account && snapshot.encode()? == raw,
                    "saved authority binding"
                );
                pins.insert(
                    account,
                    store::L1Authority {
                        account,
                        revision: snapshot.revision(),
                        raw,
                    },
                );
            }
            specs.push(spec);
            accounts.push(pair);
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = pins.entry(material.account) {
            let raw = repository.authority(material.account)?;
            let pin = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            entry.insert(store::L1Authority {
                account: material.account,
                revision: pin.revision(),
                raw,
            });
        }
        if !repository.initialized()? {
            repository.initialize(accounts, pins.into_values().collect())?;
        }
        for c in 0..specs.len() {
            let mode = repository.existing_state(c as u8)?.mode;
            ensure!(
                mode == store::ExistingMode::L1
                    || repository.metadata(c as u8)?.request_index_ready,
                "transition requires the saved complete request index"
            );
            while !repository.metadata(c as u8)?.request_index_ready {
                repository.commit(
                    store::L1Operation::PrepareRequests { context: c as u8 },
                    None,
                )?;
            }
            if repository
                .control(c as u8, store::L1Control::Ready(0))?
                .is_some()
                && repository
                    .control(c as u8, store::L1Control::Ready(1))?
                    .is_some()
            {
                if !repository.history_initialized(c as u8)? {
                    ensure!(
                        mode == store::ExistingMode::L1,
                        "transition history identity absent"
                    );
                    repository.commit(
                        store::L1Operation::InitializeHistory { context: c as u8 },
                        None,
                    )?;
                }
            } else {
                ensure!(
                    mode == store::ExistingMode::L1,
                    "transition requires both original Ready1"
                );
            }
        }
        let window = window::RefWindow2::new(&enabled);
        let source = repository.source_id();
        let mut owner = Self {
            repository: Some(repository),
            failed_lock: None,
            device,
            nonce: material.nonce,
            account: AccountId::from_bytes(material.account),
            specs,
            enabled,
            leases: [None, None],
            endpoint: None,
            publication_errors: std::array::from_fn(|_| [None, None]),
            origin_error: [None, None],
            origin_ready: [false, false],
            stats: Stats7::default(),
            resources: Arc::new(resources::Resources7::default()),
            window,
            wake: Arc::new(tokio::sync::Notify::new()),
            native_cache: [None, None],
            route: kilogram_transport_iroh::ordered2::RouteConfig2 {
                policy: kilogram_transport_iroh::RoutePolicy::DirectOnly,
                bind: Some("127.0.0.1:0".parse()?),
                relay: None,
            },
            admissions: [None, None],
            next_connection: 1,
            mixed_identity: native::MixedIdentity7 {
                incarnation: material.incarnation,
                claim: material.nonce,
                source,
            },
            high_water: std::array::from_fn(|_| Arc::new(AtomicU64::new(0))),
            current_cache: [None, None],
            pending_authorities: BTreeMap::new(),
        };
        owner.sync_high_water()?;
        for c in 0..owner.specs.len() {
            let mode = owner.repo()?.existing_state(c as u8)?.mode;
            if mode != store::ExistingMode::L1 {
                owner.existing(c as u8, |controls| controls.validate_saved(c as u8))?;
            }
        }
        Ok(owner)
    }
    pub fn slot(&self, c: Id) -> Result<u8> {
        Ok(self
            .specs
            .iter()
            .position(|s| s.c == c)
            .context("unknown indexed C")? as u8)
    }
    pub fn spec(&self, c: u8) -> Result<&Spec> {
        self.specs
            .get(usize::from(c))
            .context("unknown indexed slot")
    }
    pub fn role(&self, c: u8) -> Result<u64> {
        self.spec(c)?.role(self.device.identity().device_id())
    }
    /// The one writer owns existing controls as well as L1. Invalidate socket
    /// tokens before returning from a durable freeze/policy change, including an
    /// ambiguous write; asynchronous tasks cannot publish on the old L1 lease.
    pub fn existing<T>(
        &mut self,
        c: u8,
        action: impl FnOnce(&mut control::ExistingControls7<'_>) -> Result<T>,
    ) -> Result<T> {
        self.authorities_persisted(c)?;
        let policy = self.repo()?.metadata(c)?.policy_generation;
        let result = {
            let repository = self.repository.as_mut().context("owner recovery failed")?;
            action(&mut control::ExistingControls7::new(
                repository,
                &self.device,
            )?)
        };
        if self.repo()?.requires_reopen() {
            for slot in 0..self.specs.len() {
                self.invalidate(slot as u8);
            }
        } else if self.repo()?.metadata(c)?.policy_generation != policy
            || !self.repo()?.existing_state(c)?.mode.allows_l1_transport()
                && self.leases[usize::from(c)]
                    .as_ref()
                    .is_some_and(|lease| lease.family == 1)
        {
            self.invalidate(c);
        }
        if !self.repo()?.requires_reopen() {
            self.sync_high_water()?;
        }
        result
    }
    pub fn head(&mut self, c: u8) -> Result<Head> {
        let role = self.role(c)?;
        let cert = self.spec(c)?.peers[role as usize].cert_ref()?;
        let context = self.spec(c)?.c;
        Head::parse(&self.repo()?.head(c)?, context, cert, role)
    }
    pub fn control(&mut self, c: u8, control: store::L1Control) -> Result<Vec<u8>> {
        self.repo()?
            .control(c, control)?
            .context("required indexed L1 control")
    }
    pub fn active(&mut self, c: u8) -> Result<bool> {
        Ok(
            self.repo()?.existing_state(c)?.mode == store::ExistingMode::L1
                && self.head(c)?.phase == 3
                && self
                    .repo()?
                    .control(c, store::L1Control::Ready(0))?
                    .is_some()
                && self
                    .repo()?
                    .control(c, store::L1Control::Ready(1))?
                    .is_some(),
        )
    }
    pub fn current(&mut self, c: u8) -> Result<u64> {
        if !self.repo()?.existing_state(c)?.mode.allows_l1_transport() {
            if self.leases[usize::from(c)]
                .as_ref()
                .is_some_and(|lease| lease.family == 1)
            {
                self.invalidate(c);
            }
            anyhow::bail!("ALPN1 forbidden after durable Propose/Prepared");
        }
        self.authorities_persisted(c)?;
        ensure!(
            self.repo()?
                .control(c, store::L1Control::BlockedFork)?
                .is_none(),
            "Blocked: signed fork evidence"
        );
        let meta = self.repo()?.metadata(c)?;
        let wall = now()?;
        ensure!(
            wall >= meta.wall_high_water,
            "AuthBlocked: wall clock rollback"
        );
        if let Some((generation, not_before, expires)) = self.current_cache[usize::from(c)]
            && generation == meta.policy_generation
            && wall.saturating_add(120) >= not_before
            && wall < expires
        {
            return Ok(expires);
        }
        let consent = self.control(c, store::L1Control::Consent)?;
        let spec = self.spec(c)?.clone();
        let (not_before, expires) = spec.verify_consent(&consent)?;
        ensure!(
            wall.saturating_add(120) >= not_before && wall < expires,
            "AuthBlocked: current Consent1 interval"
        );
        for credential in &spec.peers {
            let raw = self
                .repo()?
                .authority(*credential.cert.account_id().as_bytes())?;
            let authority = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            verify_device_authorization_with_snapshot(
                credential.cert.account_id(),
                &credential.cert,
                &authority,
                &DeviceCapability::MESSAGING,
            )
            .context("AuthBlocked: current account pin")?;
        }
        self.current_cache[usize::from(c)] = Some((meta.policy_generation, not_before, expires));
        Ok(expires)
    }
    pub fn current_existing(&mut self, c: u8) -> Result<u64> {
        self.authorities_persisted(c)?;
        self.existing(c, |controls| controls.current_permission(c))
    }
    pub fn data_permission(&mut self, c: u8) -> Result<u64> {
        if self.repo()?.existing_state(c)?.mode != store::ExistingMode::L1
            || self.leases[usize::from(c)]
                .as_ref()
                .is_some_and(|l| l.family == 2)
        {
            self.current_existing(c)
        } else {
            self.current(c)
        }
    }
    pub fn data_ready(&mut self, c: u8) -> Result<bool> {
        Ok(self.head(c)?.phase == 3
            && self
                .repo()?
                .control(c, store::L1Control::Ready(0))?
                .is_some()
            && self
                .repo()?
                .control(c, store::L1Control::Ready(1))?
                .is_some())
    }
    pub(super) fn sync_high_water(&mut self) -> Result<()> {
        for c in 0..self.specs.len() {
            let floor = self.repo()?.metadata(c as u8)?.wall_high_water;
            self.high_water[c].fetch_max(floor, Ordering::AcqRel);
        }
        Ok(())
    }
    fn authorities_persisted(&self, c: u8) -> Result<()> {
        ensure!(
            !self.spec(c)?.peers.iter().any(|p| self
                .pending_authorities
                .contains_key(p.cert.account_id().as_bytes())),
            "AuthBlocked: authority persistence pending"
        );
        Ok(())
    }
    pub fn credentials(&mut self, c: u8, role: u64) -> Result<Credentials> {
        let cert = self.spec(c)?.peers[usize::try_from(role)?].cert.clone();
        let authority = AccountAuthoritySnapshot::decode_and_verify(
            &self.repo()?.authority(*cert.account_id().as_bytes())?,
        )?;
        Credentials::new(cert, authority)
    }
    pub fn invalidate(&mut self, c: u8) {
        if let Some(pending) = self.admissions[usize::from(c)].take() {
            pending
                .connection
                .close(0_u32.into(), b"existing admission invalidated");
        }
        if let Some(lease) = self.leases[usize::from(c)].take() {
            lease.valid.store(false, Ordering::Release);
            lease.connection.close(0_u32.into(), b"L1 policy changed");
        }
        self.current_cache[usize::from(c)] = None;
        self.native_cache[usize::from(c)] = None;
    }
    pub fn pin_authority(&mut self, raw: &[u8]) -> Result<()> {
        ensure!(raw.len() <= 16 * 1024, "authority cap");
        let pin = AccountAuthoritySnapshot::decode_and_verify(raw)?;
        ensure!(pin.encode()? == raw, "authority canonical bytes");
        let account = *pin.account_id().as_bytes();
        ensure!(
            account == *self.account.as_bytes()
                || self.specs.iter().any(|s| s.peers.iter().any(|p| p
                    .cert
                    .account_id()
                    .as_bytes()
                    == &account)),
            "unpinned authority account"
        );
        let prior = self.repo()?.authority(account)?;
        let old = AccountAuthoritySnapshot::decode_and_verify(&prior)?;
        if let Some((revision, pending)) = self.pending_authorities.get(&account) {
            ensure!(
                pin.revision() >= *revision && (pin.revision() != *revision || pending == raw),
                "authority pending rollback/revision conflict"
            );
        }
        ensure!(
            pin.revision() >= old.revision() && (pin.revision() != old.revision() || prior == raw),
            "authority rollback/revision conflict"
        );
        if prior == raw && !self.pending_authorities.contains_key(&account) {
            return Ok(());
        }
        self.pending_authorities
            .insert(account, (pin.revision(), raw.to_vec()));
        // Close the atomic network tokens before the durable policy transition.
        // Socket tasks run independently of the serialized owner thread.
        for c in 0..self.specs.len() {
            if self.specs[c]
                .peers
                .iter()
                .any(|p| p.cert.account_id().as_bytes() == &account)
            {
                self.invalidate(c as u8);
            }
        }
        self.repo()?.commit(
            store::L1Operation::PinAuthority {
                authority: store::L1Authority {
                    account,
                    revision: pin.revision(),
                    raw: raw.to_vec(),
                },
                now: now()?,
            },
            None,
        )?;
        self.pending_authorities.remove(&account);
        self.sync_high_water()?;
        self.wake.notify_waiters();
        Ok(())
    }
    pub fn renew_consent(&mut self, c: u8, expected: Option<Id>, raw: Vec<u8>) -> Result<()> {
        self.authorities_persisted(c)?;
        let spec = self.spec(c)?.clone();
        let (_, expires) = spec.verify_consent(&raw)?;
        spec.valid_now(&raw, now()?)?;
        ensure!(now()? < expires, "expired Consent1 renewal");
        // Renewal can update permission while expired, but cannot reverse authority/block knowledge.
        for credential in &spec.peers {
            let authority = AccountAuthoritySnapshot::decode_and_verify(
                &self
                    .repo()?
                    .authority(*credential.cert.account_id().as_bytes())?,
            )?;
            verify_device_authorization_with_snapshot(
                credential.cert.account_id(),
                &credential.cert,
                &authority,
                &DeviceCapability::MESSAGING,
            )?;
        }
        let prior = self.repo()?.control(c, store::L1Control::Consent)?;
        ensure!(
            prior.as_ref().map(|v| wire::hash(handshake::CONSENT, v)) == expected,
            "Conflict: current consent CAS"
        );
        if prior.as_deref() == Some(raw.as_slice()) {
            return Ok(());
        }
        self.invalidate(c);
        self.repo()?.commit(
            store::L1Operation::RenewConsent {
                context: c,
                expected,
                raw,
                now: now()?,
            },
            None,
        )?;
        self.wake.notify_waiters();
        Ok(())
    }
    pub fn descriptor(&mut self, c: u8, local: bool) -> Result<Descriptor> {
        let role = self.role(c)?;
        let raw = self.control(c, store::L1Control::Locator(local))?;
        Descriptor::parse(self.spec(c)?, if local { role } else { 1 - role }, &raw)
    }
    pub fn import_locator(&mut self, c: u8, raw: Vec<u8>) -> Result<()> {
        self.current(c)?;
        let role = 1 - self.role(c)?;
        let locator = Descriptor::parse(self.spec(c)?, role, &raw)?;
        locator.valid_now(now()?)?;
        if let Some(prior) = self.repo()?.control(c, store::L1Control::Locator(false))? {
            let previous = Descriptor::parse(self.spec(c)?, role, &prior)?;
            ensure!(
                locator.revision >= previous.revision
                    && (locator.revision != previous.revision || prior == raw),
                "Conflict: locator revision"
            );
            if prior == raw {
                return Ok(());
            }
        }
        self.invalidate(c);
        self.repo()?.commit(
            store::L1Operation::Locator {
                context: c,
                local: false,
                revision: locator.revision,
                raw,
                now: now()?,
            },
            None,
        )?;
        self.wake.notify_waiters();
        Ok(())
    }
    pub fn publish_locator(&mut self, c: u8) -> Result<Vec<u8>> {
        self.current(c)?;
        let endpoint = self.endpoint.clone().context("endpoint not started")?;
        let role = self.role(c)?;
        let old = self.repo()?.control(c, store::L1Control::Locator(true))?;
        let previous = old
            .as_ref()
            .map(|raw| Descriptor::parse(self.spec(c)?, role, raw))
            .transpose()?;
        if let Some(previous) = &previous
            && previous.endpoint == *endpoint.id.as_bytes()
            && previous.valid_now(now()?).is_ok()
        {
            return Ok(previous.raw.clone());
        }
        let revision = previous.map_or(Ok(1), |p| {
            p.revision
                .checked_add(1)
                .context("locator revision exhausted")
        })?;
        let descriptor = Descriptor::issue(
            self.spec(c)?,
            role,
            *endpoint.id.as_bytes(),
            endpoint.ip_addrs().copied().collect(),
            revision,
            now()?,
            self.device.identity(),
        )?;
        self.invalidate(c);
        self.repo()?.commit(
            store::L1Operation::Locator {
                context: c,
                local: true,
                revision,
                raw: descriptor.raw.clone(),
                now: now()?,
            },
            None,
        )?;
        Ok(descriptor.raw)
    }
    pub fn check_lease(&mut self, c: u8, binding: Id) -> Result<Lease1> {
        if self.leases[usize::from(c)]
            .as_ref()
            .is_some_and(|l| l.family == 2)
        {
            return self.check_existing_lease(c, binding);
        }
        self.current(c)?;
        let meta = self.repo()?.metadata(c)?;
        let lease = self.leases[usize::from(c)]
            .clone()
            .context("AuthBlocked: no live L1 lease")?;
        ensure!(
            lease.binding == binding && lease.policy_generation == meta.policy_generation,
            "AuthBlocked: stale lease/policy"
        );
        lease.gate()?;
        let local = self.descriptor(c, true)?;
        let peer = self.descriptor(c, false)?;
        ensure!(
            local.reference() == lease.own_locator
                && peer.reference() == lease.peer_locator
                && lease.connection.remote_id().as_bytes() == &peer.endpoint,
            "AuthBlocked: stale locator/endpoint lease"
        );
        Ok(lease)
    }
    pub(super) fn native1_materialize_gate(&mut self, c: u8, binding: Id) -> Result<Lease1> {
        ensure!(
            self.repo()?.existing_state(c)?.mode == store::ExistingMode::L1,
            "Native1 materialization frozen"
        );
        ensure!(
            self.leases[usize::from(c)]
                .as_ref()
                .is_some_and(|lease| lease.family == 1),
            "Native1 materialization requires its L1 lease"
        );
        self.current(c)?;
        let lease = self.check_lease(c, binding)?;
        ensure!(lease.family == 1, "Native1 permission family");
        Ok(lease)
    }
    pub fn admit(
        &mut self,
        c: u8,
        connection: Connection,
        binding: Id,
        peer_credential: Vec<u8>,
        generation: u64,
    ) -> Result<Lease1> {
        let expires = self
            .current(c)?
            .min(now()?.saturating_add(300))
            .min(self.descriptor(c, true)?.expires)
            .min(self.descriptor(c, false)?.expires);
        let lease = Lease1 {
            family: 1,
            route: kilogram_transport_iroh::RoutePolicy::DirectOnly,
            peer_stage: 1,
            peer_q: None,
            peer_cuts: [None, None],
            opportunity_deadline: None,
            high_water: self.high_water[usize::from(c)].clone(),
            connection,
            binding,
            generation,
            policy_generation: self.repo()?.metadata(c)?.policy_generation,
            expires,
            deadline: Instant::now() + Duration::from_secs(expires.saturating_sub(now()?)),
            own_locator: self.descriptor(c, true)?.reference(),
            peer_locator: self.descriptor(c, false)?.reference(),
            peer_credential,
            valid: Arc::new(AtomicBool::new(true)),
        };
        lease.gate()?;
        if let Some(old) = self.leases[usize::from(c)].replace(lease.clone()) {
            old.valid.store(false, Ordering::Release);
            old.connection.close(0_u32.into(), b"L1 lease replaced");
        }
        self.stats.admissions += 1;
        Ok(lease)
    }
    pub fn native(&mut self, c: u8, head: &Head) -> Result<PrivateState> {
        ensure!(
            !self.repo()?.existing_state(c)?.mode.selected(),
            "Legacy7 open after selection forbidden"
        );
        let hash = head.blob.context("private state absent")?;
        if let Some((saved, native::Native7::Legacy(native))) = &self.native_cache[usize::from(c)]
            && *saved == hash
        {
            return Ok(native.fork());
        }
        let snapshot = self.repo()?.snapshot(c)?;
        let raw = snapshot.native.context("current Native missing")?;
        let private = PrivateState::open(head, self.nonce, self.device.encryption(), &raw)?;
        if let Some(guard) = &private.guard {
            guard.validate(head.role)?;
            let role = head.role as usize;
            let mut next = [0; 2];
            let mut tails = [None; 2];
            next[role] = guard.own.next;
            next[1 - role] = guard.peer.next;
            tails[role] = guard.own.last;
            tails[1 - role] = guard.peer.last;
            ensure!(
                next == snapshot.meta.next && tails == snapshot.meta.tails,
                "current Native/Guard1/indexed frontiers"
            );
            for (r, lane) in [(role, &guard.own), (1 - role, &guard.peer)] {
                ensure!(
                    self.repo()?.seen_count(c, r as u8)? == lane.seen.len() as u64,
                    "Guard1/Seen mirror count"
                );
                for key in &lane.seen {
                    ensure!(
                        self.repo()?.seen(c, r as u8, *key)?.is_some(),
                        "Guard1 key absent from Seen mirror"
                    );
                }
            }
        } else {
            ensure!(
                snapshot.meta.next == [0, 0],
                "partial Native cannot invent readable genesis"
            );
        }
        let trial = private.fork();
        self.native_cache[usize::from(c)] = Some((hash, native::Native7::Legacy(private)));
        Ok(trial)
    }
    pub fn commit(
        &mut self,
        c: u8,
        binding: Id,
        op: store::L1Operation,
    ) -> Result<db::CommitReport> {
        let lease = if matches!(&op, store::L1Operation::Materialize { .. }) {
            self.native1_materialize_gate(c, binding)?
        } else {
            self.check_lease(c, binding)?
        };
        if lease.family == 2 {
            let data = match op {
                store::L1Operation::Drain { change, rows } => {
                    store::ExistingData7::Readable { change, rows }
                }
                store::L1Operation::Pending {
                    author,
                    raw,
                    original,
                    admission,
                    head,
                    ..
                } => store::ExistingData7::Pending {
                    author,
                    raw,
                    original,
                    admission,
                    head,
                    grant: None,
                },
                store::L1Operation::Quarantine { ordinal, .. } => {
                    store::ExistingData7::Quarantine { ordinal }
                }
                store::L1Operation::Acknowledge {
                    event,
                    receipt,
                    receiver,
                    ..
                } => store::ExistingData7::Acknowledge {
                    event,
                    receipt,
                    receiver,
                },
                store::L1Operation::AckProgress { next, .. } => {
                    store::ExistingData7::AckProgress { next }
                }
                store::L1Operation::ObserveHead { next, last, .. } => {
                    store::ExistingData7::ObserveHead { next, last }
                }
                store::L1Operation::Block { proof, .. } => store::ExistingData7::Block { proof },
                _ => anyhow::bail!("ALPN2 rejects L1 bootstrap/administrative purpose"),
            };
            return self.commit_data2(c, Some(binding), data);
        }
        let native = matches!(
            &op,
            store::L1Operation::Materialize { .. } | store::L1Operation::Drain { .. }
        ) || matches!(&op, store::L1Operation::Bootstrap { change, .. } if change.native.is_some());
        let floor = now()?;
        let mut gate = || {
            if native {
                super::super::test_support::checkpoint("l1-before-native-commit")
                    .map_err(|_| db::Error::PolicyRejected("native checkpoint"))?;
            }
            lease.gate()?;
            if now().map_or(true, |wall| wall < floor) {
                return Err(db::Error::PolicyRejected("clock rollback before commit"));
            }
            Ok(())
        };
        let result = self.repo()?.commit(op, Some(&mut gate));
        match result {
            Ok(report) => {
                self.sync_high_water()?;
                if native {
                    super::super::test_support::checkpoint("l1-after-native-commit")?;
                }
                self.stats.max_commit_app_bytes = self
                    .stats
                    .max_commit_app_bytes
                    .max(report.app_buffer_bound_bytes);
                Ok(report)
            }
            Err(error) => {
                if self.repo()?.requires_reopen() {
                    for c in 0..self.specs.len() {
                        self.invalidate(c as u8);
                    }
                    self.stats.ambiguous_packet_calls += 1;
                }
                Err(error.into())
            }
        }
    }
    pub fn recover(&mut self) -> Result<()> {
        if !self.repo()?.requires_reopen() {
            return Ok(());
        }
        for c in 0..self.specs.len() {
            self.invalidate(c as u8);
        }
        let repo = self
            .repository
            .take()
            .context("repository already recovering")?;
        match repo.reopen() {
            Ok(repo) => {
                self.repository = Some(repo);
                let pending = self
                    .pending_authorities
                    .iter()
                    .map(|(id, (revision, raw))| (*id, *revision, raw.clone()))
                    .collect::<Vec<_>>();
                for (id, revision, raw) in pending {
                    let durable = self.repo()?.authority(id)?;
                    let authority = AccountAuthoritySnapshot::decode_and_verify(&durable)?;
                    if authority.revision() > revision
                        || authority.revision() == revision && durable == raw
                    {
                        self.pending_authorities.remove(&id);
                    }
                }
                for c in 0..self.specs.len() {
                    if self.repo()?.existing_state(c as u8)?.mode.selected() {
                        self.existing(c as u8, |controls| controls.validate_mixed(c as u8))?;
                    }
                }
                self.stats.reopens += 1;
                Ok(())
            }
            Err(failure) => {
                self.failed_lock = Some(failure.lease);
                Err(failure.error.into())
            }
        }
    }
    pub fn capacity(
        &mut self,
        expected: u64,
        revision: u64,
        hash: Id,
        candidate: db::migration7::MigrationConfig,
    ) -> Result<()> {
        let plan = self
            .repo()?
            .prepare_capacity(expected, revision, hash, candidate)?;
        if let Some(value) = plan.profile_route()
            && !value.is_null()
        {
            let route: kilogram_transport_iroh::ordered2::RouteConfig2 =
                serde_json::from_value(value.clone())?;
            route.effective()?;
        }
        let repository = self
            .repository
            .take()
            .context("capacity repository already taken")?;
        match repository.apply_capacity(plan) {
            Ok((repository, result)) => {
                self.repository = Some(repository);
                result.map_err(Into::into)
            }
            Err(failure) => {
                self.failed_lock = Some(failure.lease);
                Err(failure.error.into())
            }
        }
    }
    pub fn change(
        &mut self,
        c: u8,
        head: &mut Head,
        private: Option<&PrivateState>,
    ) -> Result<store::L1NativeChange> {
        let before = self.repo()?.head(c)?;
        head.bump(private.is_some())?;
        let native = private
            .map(|p| p.seal(head, self.nonce, self.device.encryption().public_key()))
            .transpose()?;
        if let Some(raw) = &native {
            self.stats.max_native_bytes = self.stats.max_native_bytes.max(raw.len());
        }
        Ok(store::L1NativeChange {
            context: c,
            expected_head: wire::hash(b"", &before),
            policy_generation: self.repo()?.metadata(c)?.policy_generation,
            wall_time: now()?,
            head: head.encode()?,
            native,
        })
    }
    pub fn retain_native(&mut self, c: u8, head: &Head, private: PrivateState) {
        if let Some(hash) = head.blob {
            self.native_cache[usize::from(c)] = Some((hash, native::Native7::Legacy(private)));
        }
    }
    pub fn event_at(&mut self, c: u8, role: u64, n: u64) -> Result<Option<Event>> {
        let signer = self.spec(c)?.peers[role as usize].cert.device_id();
        self.repo()?
            .event_at(c, u8::try_from(role)?, n)?
            .map(|raw| Event::parse(&raw, signer).map_err(Into::into))
            .transpose()
    }
    pub fn signed(&self, kind: u64, binding: Id, body: Value) -> Result<Frame> {
        let frame = Frame::signed(
            kind,
            binding,
            admin::random::<16>()?,
            body,
            self.device.identity(),
        )?;
        super::super::test_support::ready_frame(frame, self.device.identity())
    }
}
