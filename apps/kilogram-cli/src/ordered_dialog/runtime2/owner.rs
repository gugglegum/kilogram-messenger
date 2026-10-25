use super::*;
use iroh::{EndpointAddr, endpoint::Connection};
use kilogram_identity::{AccountId, DeviceEncryptionIdentity, DeviceIdentity};
use kilogram_transport_iroh::{RoutePolicy, ordered2::RouteConfig2};
use native::Native2;
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

#[derive(Clone)]
pub(super) struct Lease2 {
    pub connection: Connection,
    pub binding: Id,
    pub generation: u64,
    pub policy: u64,
    pub deadline: Instant,
    pub wall_deadline: u64,
    pub own_locator: Id,
    pub peer_locator: Id,
    pub valid: Arc<AtomicBool>,
    pub pair: Vec<u8>,
}
impl Lease2 {
    pub fn check(&self, s: &db::RuntimeSnapshot, wall: u64, route: &RouteConfig2) -> Result<()> {
        let _timing = timing::span(timing::Metric::LeaseCheck);
        ensure!(
            self.valid.load(Ordering::Acquire)
                && self.connection.close_reason().is_none()
                && Instant::now() < self.deadline
                && wall < self.wall_deadline
                && self.policy == s.profile.policy_generation,
            "LeaseExpired/Closed"
        );
        let own = wire::ParsedRuntimeLocator2::parse(
            s.spec.own_locator.as_deref().context("own locator")?,
        )?;
        let peer = wire::ParsedRuntimeLocator2::parse(
            s.spec.peer_locator.as_deref().context("peer locator")?,
        )?;
        ensure!(
            own.id() == self.own_locator
                && peer.id() == self.peer_locator
                && self.connection.remote_id().as_bytes() == &peer.fields().endpoint,
            "lease locator binding"
        );
        let paths = self.connection.paths();
        let path = paths
            .iter()
            .find(|p| p.is_selected())
            .context("no selected path")?;
        match route.connection_policy(&peer)? {
            RoutePolicy::DirectOnly => ensure!(path.is_ip(), "route changed"),
            RoutePolicy::RelayOnly => ensure!(path.is_relay(), "route changed"),
            RoutePolicy::Auto => ensure!(path.is_ip() || path.is_relay(), "unsupported route"),
        }
        Ok(())
    }
    pub fn invalidate(&self) {
        self.valid.store(false, Ordering::Release);
        self.connection
            .close(0u32.into(), b"policy/locator changed");
    }
}
#[derive(Default, Serialize)]
pub struct Stats2 {
    pub packet_attempts: u64,
    pub committed_encrypt: u64,
    pub committed_decrypt: u64,
    pub aborted_packet_calls: u64,
    pub ambiguous_packet_calls: u64,
    pub recovered_commits: u64,
    pub materialized: u64,
    pub readable: u64,
    pub max_native_bytes: usize,
    pub max_commit_app_bytes: u64,
    pub max_ram_refs: [usize; 2],
    pub ram_refs: [usize; 2],
    pub admissions: u64,
    pub reopens: u64,
    pub prefix_pages: [u64; 2],
    pub prefix_head_pages: [u64; 2],
    pub max_pending: [u64; 2],
    pub head_repairs: [u64; 2],
}
pub(super) struct RepositorySlot {
    inner: Option<db::RuntimeRepository>,
    failed_lock: Option<kilogram_state::StateDirectoryLock>,
}
// Only reopen temporarily takes inner. Failure terminates the owner loop while
// failed_lock retains the same directory lease. No caller can resume that owner.
#[expect(
    clippy::expect_used,
    reason = "single owner terminates on failed recovery before any repository access"
)]
impl std::ops::Deref for RepositorySlot {
    type Target = db::RuntimeRepository;
    fn deref(&self) -> &Self::Target {
        self.inner
            .as_ref()
            .expect("owner repository is available outside recovery")
    }
}
#[expect(
    clippy::expect_used,
    reason = "single owner terminates on failed recovery before any repository access"
)]
impl std::ops::DerefMut for RepositorySlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
            .as_mut()
            .expect("owner repository is available outside recovery")
    }
}
impl RepositorySlot {
    fn reopen(&mut self) -> Result<()> {
        let repository = self.inner.take().context("repository already recovering")?;
        match repository.reopen() {
            Ok(repository) => {
                self.inner = Some(repository);
                Ok(())
            }
            Err(failure) => {
                self.failed_lock = Some(failure.lease);
                Err(failure.error.into())
            }
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Purpose2 {
    Import,
    Local,
    Peer,
}
pub struct Owner2 {
    pub(super) repository: RepositorySlot,
    pub(super) device: DeviceIdentity,
    pub(super) encryption: DeviceEncryptionIdentity,
    pub(super) leases: [Option<Lease2>; 2],
    pub(super) route: RouteConfig2,
    pub(super) enabled: Vec<u8>,
    pub(super) endpoint: Option<(Id, EndpointAddr)>,
    pub(super) stats: Stats2,
    pub(super) next_connection: u64,
    pub(super) admissions: [Option<admission::Admission2>; 2],
    pub(super) window: Arc<window::RefWindow2>,
    pub(super) resources: Arc<resources::Resources2>,
    current_cache: [Option<(Id, policy::Current2)>; 2],
    selection_cache: [Option<(Id, wire::StaticSelection2)>; 2],
    native_cache: [Option<(Id, Native2)>; 2],
    pub(super) credential_cache: std::collections::VecDeque<(Id, wire::RootVerifiedCredential)>,
    pub(super) needs_reopen: bool,
    pending_commit: Option<(Id, (u64, u64))>,
    pub(super) exported: [Option<Id>; 2],
    used_faults: std::collections::BTreeSet<String>,
    pub(super) public_import: Option<public_import::PublicImport2>,
    pub(super) trial_view: Option<data::TrialView2>,
    pub(super) clock_high_water: Arc<AtomicU64>,
}
impl Owner2 {
    pub fn new(
        mut repository: db::RuntimeRepository,
        route: RouteConfig2,
        enabled: Vec<u8>,
    ) -> Result<Self> {
        route.effective()?;
        let material = repository.owner_material()?;
        let device = DeviceIdentity::from_secret_bytes(*material.signing);
        let encryption = DeviceEncryptionIdentity::from_secret_bytes(*material.encryption);
        ensure!(
            device.device_id().as_bytes() == &material.profile.enrollment.device,
            "saved Device binding"
        );
        let cert = kilogram_identity::DeviceCertificate::decode_and_verify(
            &material.profile.enrollment.certificate,
        )?;
        ensure!(
            cert.device_id() == device.device_id()
                && cert.encryption_public_key() == encryption.public_key(),
            "saved certificate keys"
        );
        ensure!(
            !enabled.is_empty()
                && enabled.len() <= 2
                && enabled.iter().all(|c| *c < 2)
                && (enabled.len() == 1 || enabled[0] != enabled[1]),
            "enabled set"
        );
        for c in &enabled {
            if !repository.installed(*c)? {
                continue;
            }
            let s = repository.snapshot(*c)?;
            ensure!(
                wire::ParsedConsent2::parse(&s.spec.consent)?.origin() == &wire::Origin::Fresh,
                "UnsupportedExistingRuntime"
            );
            if s.head.phase == 3 {
                policy::selected(&s)?;
                repository.frontiers(*c)?;
            }
        }
        let window = window::RefWindow2::new(&enabled);
        let clock_high_water = Arc::new(AtomicU64::new(material.profile.wall_high_water));
        let mut owner = Self {
            repository: RepositorySlot {
                inner: Some(repository),
                failed_lock: None,
            },
            device,
            encryption,
            leases: [None, None],
            route,
            enabled,
            endpoint: None,
            stats: Stats2::default(),
            next_connection: 1,
            admissions: [None, None],
            window,
            resources: Arc::new(resources::Resources2::default()),
            current_cache: [None, None],
            selection_cache: [None, None],
            native_cache: [None, None],
            credential_cache: std::collections::VecDeque::new(),
            needs_reopen: false,
            pending_commit: None,
            exported: [None, None],
            used_faults: std::collections::BTreeSet::new(),
            public_import: None,
            trial_view: None,
            clock_high_water,
        };
        for c in owner.enabled.clone() {
            if !owner.repository.installed(c)? {
                continue;
            }
            let s = owner.snapshot(c)?;
            owner.restore_controls(c, &s)?;
            owner.repository.native(c)?;
            if s.head.phase != 0 {
                owner.native(c, &s)?;
            }
        }
        Ok(owner)
    }
    pub fn account(&mut self) -> Result<AccountId> {
        Ok(AccountId::from_bytes(self.repository.profile()?.account))
    }
    pub fn device(&self) -> &DeviceIdentity {
        &self.device
    }
    pub(super) fn snapshot(&mut self, c: u8) -> Result<db::RuntimeSnapshot> {
        let _timing = timing::span(timing::Metric::Snapshot);
        ensure!(self.enabled.contains(&c), "context is paused");
        Ok(self.repository.snapshot(c)?)
    }
    pub(super) fn invalidate(&mut self) {
        for lease in &mut self.leases {
            if let Some(old) = lease.take() {
                old.invalidate();
            }
        }
    }
    pub(super) fn current(
        &mut self,
        s: &db::RuntimeSnapshot,
        wall: u64,
    ) -> Result<policy::Current2> {
        let _timing = timing::span(timing::Metric::Current);
        ensure!(
            !hooks::load()?.authority_read_failure,
            "AuthorityReadFailure: verification provider unavailable"
        );
        let c = s
            .profile
            .contexts
            .iter()
            .position(|c| *c == s.head.context)
            .context("context slot")?;
        let raw = postcard::to_allocvec(&(
            s.head.context,
            s.head.role,
            s.profile.policy_generation,
            &s.profile.enrollment,
            &s.profile.peer,
            &s.spec,
        ))?;
        let key = wire::hash(b"kg:od2:current-crypto-cache\0", &raw);
        if self.current_cache[c]
            .as_ref()
            .is_none_or(|(old, _)| *old != key)
        {
            self.current_cache[c] = Some((key, policy::current(s, wall)?));
        }
        let current = self.current_cache[c]
            .as_ref()
            .context("current cache")?
            .1
            .clone();
        current.check_time(s, wall)?;
        Ok(current)
    }
    pub(super) fn selected(&mut self, s: &db::RuntimeSnapshot) -> Result<wire::StaticSelection2> {
        let _timing = timing::span(timing::Metric::Selected);
        let c = s
            .profile
            .contexts
            .iter()
            .position(|c| *c == s.head.context)
            .context("context slot")?;
        let raw = postcard::to_allocvec(&(
            &s.selection,
            &s.spec.credentials,
            &s.spec.membership,
            s.head.context,
            s.head.dialog,
            s.head.epoch,
            &s.head.sid,
            s.head.mode_ref,
        ))?;
        let key = wire::hash(b"kg:od2:selected-crypto-cache\0", &raw);
        if self.selection_cache[c]
            .as_ref()
            .is_none_or(|(old, _)| *old != key)
        {
            self.selection_cache[c] = Some((key, policy::selected(s)?));
        }
        Ok(self.selection_cache[c]
            .as_ref()
            .context("selected cache")?
            .1
            .clone())
    }
    pub(super) fn commit(
        &mut self,
        s: &db::RuntimeSnapshot,
        op: db::RuntimeOperation,
        purpose: Purpose2,
        calls: (u64, u64),
    ) -> Result<db::CommitReport> {
        let _timing = timing::span(timing::Metric::ClosedCommit);
        // Leases are indexed by immutable context slots, never by elected role.
        let c = s
            .profile
            .contexts
            .iter()
            .position(|id| *id == s.head.context)
            .context("context slot")?;
        let lease = if matches!(purpose, Purpose2::Peer) {
            Some(self.leases[c].clone().context("NeedAdmission")?)
        } else {
            None
        };
        let route = self.route.clone();
        let verified = if matches!(purpose, Purpose2::Import) {
            None
        } else {
            Some(self.current(s, now()?)?)
        };
        let name = match &op {
            db::RuntimeOperation::InstallContext { .. } => "InstallContext",
            db::RuntimeOperation::PinAuthority { .. } => "PinAuthority",
            db::RuntimeOperation::RenewConsent { .. } => "RenewConsent",
            db::RuntimeOperation::Locator { .. } => "Locator",
            db::RuntimeOperation::PrepareOffer { .. } => "PrepareOffer",
            db::RuntimeOperation::ReserveGrant { .. } => "ReserveGrant",
            db::RuntimeOperation::HistoricalConsent { .. } => "HistoricalConsent",
            db::RuntimeOperation::Genesis { .. } => "Genesis",
            db::RuntimeOperation::Ready { .. } => "Ready",
            db::RuntimeOperation::Queue { .. } => "Queue",
            db::RuntimeOperation::Materialize { .. } => "Materialize",
            db::RuntimeOperation::AcceptPending { .. } => "AcceptPending",
            db::RuntimeOperation::Drain { .. } => "Drain",
            db::RuntimeOperation::Acknowledge { .. } => "Acknowledge",
            db::RuntimeOperation::Repair { .. } => "Repair",
            db::RuntimeOperation::AckProgress { .. } => "AckProgress",
            db::RuntimeOperation::PeerHead { .. } => "PeerHead",
            db::RuntimeOperation::Block { .. } => "Block",
        };
        let committed_wall = op.captured_wall().unwrap_or(s.profile.wall_high_water);
        let mut gate = || -> db::Result<()> {
            hooks::checkpoint(&format!("{name}.staged"),Some(c as u8),serde_json::json!({"root_generation":s.root_generation,"native_hash":s.head.native_hash.map(|id|hex(&id)),"native_generation":s.head.native_generation,"active":s.head.active}))
                .map_err(|_|db::Error::PolicyRejected("verification gate controller"))?;
            if !matches!(purpose, Purpose2::Import) {
                let wall = now().map_err(|_| db::Error::PolicyRejected("ClockBlocked"))?;
                verified
                    .as_ref()
                    .ok_or(db::Error::PolicyRejected("current verification"))?
                    .check_time(s, wall)
                    .map_err(|_| db::Error::PolicyRejected("current policy/time"))?;
                if let Some(lease) = &lease {
                    lease
                        .check(s, wall, &route)
                        .map_err(|_| db::Error::PolicyRejected("actual lease"))?;
                }
            }
            Ok(())
        };
        let started = Instant::now();
        #[cfg(debug_assertions)]
        {
            let schedule = hooks::load()?;
            if schedule.applies(c as u8)
                && schedule.io_operation.as_deref() == Some(name)
                && let (Some(point), Some(token)) = (schedule.io_fault, schedule.io_token)
                && !self.used_faults.contains(&token)
            {
                ensure!(
                    self.used_faults.len() < 16,
                    "verification fault token bound"
                );
                self.repository
                    .verification_io_fault(&point, schedule.io_after)?;
                self.used_faults.insert(token);
            }
        }
        eprintln!(
            "od2_commit_begin operation={name} context={c} generation={} native_calls={}",
            s.root_generation,
            calls.0 + calls.1
        );
        let commit_id = shared_admin::random()?;
        let result = self
            .repository
            .commit(s.root_generation, commit_id, op, &mut gate);
        eprintln!(
            "od2_commit_end operation={name} context={c} generation={} ok={} micros={}",
            self.repository.generation(),
            result.is_ok(),
            started.elapsed().as_micros()
        );
        match &result {
            Ok(report) => {
                self.stats.max_commit_app_bytes = self
                    .stats
                    .max_commit_app_bytes
                    .max(report.app_buffer_bound_bytes);
                self.stats.committed_encrypt += calls.0;
                self.stats.committed_decrypt += calls.1;
                if calls.0 + calls.1 > 0 {
                    eprintln!(
                        "od2_native_commit pid={} context={c} last_attempt={} calls={} encrypt={} decrypt={}",
                        std::process::id(),
                        self.stats.packet_attempts,
                        calls.0 + calls.1,
                        calls.0,
                        calls.1
                    );
                }
            }
            Err(_) => {
                if self.repository.requires_reopen() {
                    self.pending_commit = Some((commit_id, calls));
                    self.stats.ambiguous_packet_calls += calls.0 + calls.1;
                } else {
                    self.stats.aborted_packet_calls += calls.0 + calls.1;
                }
            }
        }
        if self.repository.requires_reopen() {
            self.invalidate();
            self.needs_reopen = true;
        }
        if result.as_ref().is_ok_and(|report| !report.duplicate) {
            self.clock_high_water
                .fetch_max(committed_wall, Ordering::AcqRel);
        }
        if result.is_ok() {
            hooks::checkpoint(
                &format!("{name}.committed"),
                Some(c as u8),
                serde_json::json!({"generation":self.repository.generation()}),
            )?;
        }
        Ok(result?)
    }
    pub(super) fn recover(&mut self) -> Result<()> {
        self.invalidate();
        for pending in &mut self.admissions {
            if let Some(p) = pending.take() {
                p.connection.close(0u32.into(), b"repository recovery");
            }
        }
        self.native_cache = [None, None];
        self.current_cache = [None, None];
        self.selection_cache = [None, None];
        self.credential_cache.clear();
        self.repository.reopen()?;
        self.clock_high_water
            .fetch_max(self.repository.profile()?.wall_high_water, Ordering::AcqRel);
        for c in self.enabled.clone() {
            if !self.repository.installed(c)? {
                continue;
            }
            let s = self.snapshot(c)?;
            self.restore_controls(c, &s)?;
            if s.head.phase != 0 {
                self.native(c, &s)?;
            }
        }
        if let Some((id, calls)) = self.pending_commit.take() {
            let committed = self.repository.last_commit()? == id;
            self.stats.ambiguous_packet_calls = self
                .stats
                .ambiguous_packet_calls
                .saturating_sub(calls.0 + calls.1);
            if committed {
                self.stats.recovered_commits += 1;
                self.stats.committed_encrypt += calls.0;
                self.stats.committed_decrypt += calls.1;
                if calls.0 + calls.1 > 0 {
                    eprintln!(
                        "od2_native_commit pid={} calls={} encrypt={} decrypt={} recovered=true",
                        std::process::id(),
                        calls.0 + calls.1,
                        calls.0,
                        calls.1
                    );
                }
            }
            eprintln!(
                "od2_recovered_outcome committed={committed} packet_calls={}",
                calls.0 + calls.1
            );
        }
        self.needs_reopen = false;
        self.stats.reopens += 1;
        eprintln!(
            "od2_recovered generation={} same_directory_lock=true",
            self.repository.generation()
        );
        Ok(())
    }
    pub(super) fn native(&mut self, c: u8, s: &db::RuntimeSnapshot) -> Result<Native2> {
        let _timing = timing::span(timing::Metric::NativeOpen);
        let cache_key = Self::native_cache_key(s)?;
        if let Some((key, native)) = &self.native_cache[usize::from(c)]
            && *key == cache_key
        {
            return Ok(native.fork());
        }
        let native = Native2::open(
            &s.profile,
            &s.head,
            &self.encryption,
            &self.repository.native(c)?.context("native missing")?,
        )?;
        let offer = wire::ParsedControl2::parse(&s.selection.offer)?;
        let curve = if s.head.role == 0 {
            offer.body().fields(8)?[5].fixed::<32>()?
        } else {
            wire::ParsedControl2::parse(&s.selection.grant)?
                .body()
                .fields(8)?[4]
                .fixed::<32>()?
        };
        ensure!(
            native.account.curve_key() == curve,
            "native Account selected curve"
        );
        if s.head.phase == 3 {
            let genesis = self
                .repository
                .source(c, 0, 0)?
                .context("Native missing A0")?;
            let event = wire::ParsedEvent2::parse(&genesis.raw)?;
            ensure!(
                native.session.as_ref().context("Native session")?.keys()
                    == event.packet().header()?.prekey_keys.context("A0 keys")?,
                "Native session selected keys"
            );
            let frontiers = self.repository.frontiers(c)?;
            let guard = native.guard()?;
            for (role, lane) in [(s.head.role, guard.own()), (1 - s.head.role, guard.peer())] {
                let index = usize::from(role);
                let expected = if frontiers.tails[index].is_some() {
                    frontiers.next[index]
                } else {
                    0
                };
                ensure!(
                    lane.next == expected && lane.last == frontiers.tails[index],
                    "Native/history lane frontier"
                );
                if expected > 0 {
                    let source = self
                        .repository
                        .source(c, role, expected - 1)?
                        .context("Native lane source")?;
                    let meta = wire::ParsedEvent2::parse(&source.raw)?.meta();
                    ensure!(
                        lane.g == meta.expected.g
                            && lane.key == Some(meta.key)
                            && lane.index == meta.index
                            && lane.sequence == Some(meta.sequence)
                            && lane.parent == meta.expected.parent,
                        "Native lane source binding"
                    );
                    let first = self
                        .repository
                        .seen(c, role, meta.key)?
                        .context("Native lane Seen")?;
                    ensure!(
                        first.generation == lane.g && Some(first.first_event) == lane.first,
                        "Native lane first cause"
                    );
                }
            }
        }
        let trial = native.fork();
        self.native_cache[usize::from(c)] = Some((cache_key, native));
        Ok(trial)
    }
    fn native_cache_key(s: &db::RuntimeSnapshot) -> Result<Id> {
        let h = &s.head;
        Ok(wire::hash(
            b"kg:od2:accepted-native-cache\0",
            &postcard::to_allocvec(&(
                s.profile.incarnation,
                s.profile.claim,
                &s.profile.enrollment.certificate,
                h.context,
                h.role,
                h.phase,
                h.native_generation,
                h.offer,
                h.grant,
                h.dialog,
                h.epoch,
                &h.sid,
                h.mode_ref,
                h.native_hash,
            ))?,
        ))
    }
    pub(super) fn cache_native(&mut self, c: u8, hash: Id, native: Native2) -> Result<()> {
        let s = self.snapshot(c)?;
        ensure!(
            s.head.native_hash == Some(hash),
            "accepted Native cache hash"
        );
        self.native_cache[usize::from(c)] = Some((Self::native_cache_key(&s)?, native));
        Ok(())
    }
    pub(super) fn seen(
        &mut self,
        c: u8,
        event: &kilogram_ratchet::ordered_v2::EventMeta,
    ) -> Result<kilogram_ratchet::ordered_v2::SeenObservation> {
        let _timing = timing::span(timing::Metric::Seen);
        use kilogram_ratchet::ordered_v2::{Seen, SeenObservation};
        if let Some(view) = &self.trial_view {
            ensure!(view.context == c, "trial context scope");
            if let Some(seen) = view.seen.get(&(event.role, event.key)) {
                return Ok(SeenObservation::Present(*seen));
            }
        }
        if self.repository.snapshot(c)?.head.phase < 3 {
            return Ok(SeenObservation::Absent);
        }
        Ok(self
            .repository
            .seen(c, event.role, event.key)?
            .map_or(SeenObservation::Absent, |s| {
                SeenObservation::Present(Seen {
                    generation: s.generation,
                    first: s.first_event,
                })
            }))
    }
    pub(super) fn require(
        &mut self,
        c: u8,
        peer: bool,
    ) -> Result<(db::RuntimeSnapshot, policy::Current2)> {
        let _timing = timing::span(timing::Metric::Require);
        let result = (|| {
            let s = self.snapshot(c)?;
            let current = self.current(&s, now()?)?;
            if peer {
                self.leases[usize::from(c)]
                    .as_ref()
                    .context("NeedAdmission")?
                    .check(&s, now()?, &self.route)?;
            }
            Ok((s, current))
        })();
        if result.is_err()
            && c < 2
            && let Some(lease) = self.leases[usize::from(c)].take()
        {
            lease.invalidate();
        }
        result
    }
    pub fn rotate_locator(&mut self, c: u8, instance: Id, addr: &EndpointAddr) -> Result<Vec<u8>> {
        let (s, current) = self.require(c, false)?;
        let wall = now()?;
        let policy = self.route.effective()?;
        let relay = if policy == RoutePolicy::DirectOnly {
            None
        } else {
            self.route.relay.clone()
        };
        ensure!(
            addr.relay_urls()
                .all(|url| Some(url.as_str()) == relay.as_deref()),
            "actual relay snapshot"
        );
        let ips = if policy == RoutePolicy::RelayOnly {
            ensure!(addr.ip_addrs().next().is_none(), "RelayOnly advertised IP");
            vec![]
        } else {
            addr.ip_addrs().copied().collect()
        };
        let fields = wire::RuntimeLocatorFields2 {
            c: s.head.context,
            cert: s.spec.certs[usize::from(s.head.role)],
            peer_cert: s.spec.certs[usize::from(1 - s.head.role)],
            endpoint: *addr.id.as_bytes(),
            ips,
            relay,
            revision: s
                .spec
                .own_locator_revision
                .checked_add(1)
                .context("LocatorBlocked")?,
            not_before: wall,
            expires: wall.checked_add(3600).context("locator expiry")?,
        };
        let locator = wire::ParsedRuntimeLocator2::sign(fields, &self.device)?;
        locator.verify(
            s.head.context,
            &current.peers[usize::from(s.head.role)],
            &current.peers[usize::from(1 - s.head.role)],
        )?;
        self.commit(
            &s,
            db::RuntimeOperation::Locator {
                context: c,
                fence: s.fence(wall),
                own: true,
                raw: locator.raw().to_vec(),
            },
            Purpose2::Local,
            (0, 0),
        )?;
        hooks::checkpoint(
            "locator_committed",
            Some(c),
            serde_json::json!({"revision":locator.fields().revision,"endpoint":hex(&locator.fields().endpoint),"ips":locator.fields().ips,"relay":locator.fields().relay}),
        )?;
        self.invalidate();
        self.endpoint = Some((instance, addr.clone()));
        self.exported[usize::from(c)] = None;
        Ok(locator.raw().to_vec())
    }
    pub(super) fn locator_needs_rotation(
        &mut self,
        c: u8,
        instance: Id,
        addr: &EndpointAddr,
    ) -> Result<bool> {
        let (s, _) = self.require(c, false)?;
        let Some(raw) = s.spec.own_locator else {
            return Ok(true);
        };
        let own = wire::ParsedRuntimeLocator2::parse(&raw)?;
        let f = own.fields();
        let mut actual = addr.ip_addrs().copied().collect::<Vec<_>>();
        let mut signed = f.ips.clone();
        actual.sort();
        signed.sort();
        Ok(f.endpoint != *addr.id.as_bytes()
            || actual != signed
            || now()? < f.not_before
            || now()? >= f.expires
            || self
                .endpoint
                .as_ref()
                .is_none_or(|(old, _)| *old != instance))
    }
    pub(super) fn mark_exported(&mut self, c: u8, raw: &[u8]) -> Result<()> {
        let s = self.snapshot(c)?;
        ensure!(
            s.spec.own_locator.as_deref() == Some(raw),
            "stale locator export completion"
        );
        self.exported[usize::from(c)] = Some(wire::hash(wire::DESCRIPTOR, raw));
        Ok(())
    }
    pub fn export_locator(&mut self, c: u8, instance: Id, addr: &EndpointAddr) -> Result<Vec<u8>> {
        let (s, current) = self.require(c, false)?;
        ensure!(
            self.endpoint
                .as_ref()
                .is_some_and(|(i, a)| *i == instance && a == addr),
            "LocatorUnavailable"
        );
        let raw = s.spec.own_locator.context("LocatorUnavailable")?;
        let loc = wire::ParsedRuntimeLocator2::parse(&raw)?;
        let f = loc.fields();
        ensure!(
            f.endpoint == *addr.id.as_bytes() && f.not_before <= now()? && now()? < f.expires,
            "LocatorUnavailable"
        );
        loc.verify(
            s.head.context,
            &current.peers[usize::from(s.head.role)],
            &current.peers[usize::from(1 - s.head.role)],
        )?;
        let mut actual = addr.ip_addrs().copied().collect::<Vec<_>>();
        let mut signed = f.ips.clone();
        actual.sort();
        signed.sort();
        ensure!(actual == signed, "locator addresses changed");
        Ok(raw)
    }
    pub fn import_locator(&mut self, c: u8, raw: Vec<u8>) -> Result<()> {
        let (s, current) = self.require(c, false)?;
        let loc = wire::ParsedRuntimeLocator2::parse(&raw)?;
        loc.verify(
            s.head.context,
            &current.peers[usize::from(1 - s.head.role)],
            &current.peers[usize::from(s.head.role)],
        )?;
        self.route.remote(&loc)?;
        let f = loc.fields();
        let wall = now()?;
        ensure!(f.not_before <= wall && wall < f.expires, "locator time");
        ensure!(
            f.revision >= s.spec.peer_locator_revision
                && (f.revision != s.spec.peer_locator_revision
                    || s.spec.peer_locator.as_ref() == Some(&raw)),
            "locator rollback/conflict"
        );
        if s.spec.peer_locator.as_ref() == Some(&raw) {
            return Ok(());
        }
        self.commit(
            &s,
            db::RuntimeOperation::Locator {
                context: c,
                fence: s.fence(wall),
                own: false,
                raw,
            },
            Purpose2::Local,
            (0, 0),
        )?;
        self.invalidate();
        Ok(())
    }
    pub fn import_authority(&mut self, raw: Vec<u8>) -> Result<()> {
        ensure!(raw.len() <= 16 * 1024, "authority cap");
        let snapshot = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&raw)?;
        ensure!(snapshot.encode()? == raw, "exact authority bytes");
        let p = self.repository.profile()?;
        let account = *snapshot.account_id().as_bytes();
        let (revision, old) = if account == p.account {
            (p.enrollment.authority_revision, &p.enrollment.authority)
        } else {
            let pin = p
                .peer
                .as_ref()
                .filter(|pin| pin.account == account)
                .context("authority account pin")?;
            (pin.revision, &pin.authority)
        };
        ensure!(
            snapshot.revision() >= revision && (snapshot.revision() != revision || old == &raw),
            "authority rollback/conflict"
        );
        let previous = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(old)?;
        for revoked in previous.revocations() {
            ensure!(
                snapshot
                    .revocations()
                    .iter()
                    .any(|next| next.device_id() == revoked.device_id()
                        && next.authority_sequence() >= revoked.authority_sequence()),
                "known revocation cannot be removed"
            );
        }
        if *old == raw {
            return Ok(());
        }
        let mut installed = None;
        for c in self.enabled.clone() {
            if self.repository.installed(c)? {
                installed = Some(c);
                break;
            }
        }
        if let Some(c) = installed {
            let s = self.snapshot(c)?;
            self.commit(
                &s,
                db::RuntimeOperation::PinAuthority {
                    account,
                    revision: snapshot.revision(),
                    raw,
                    expected_policy: p.policy_generation,
                },
                Purpose2::Import,
                (0, 0),
            )?;
        } else {
            let generation = self.repository.generation();
            self.repository.commit(
                generation,
                shared_admin::random()?,
                db::RuntimeOperation::PinAuthority {
                    account,
                    revision: snapshot.revision(),
                    raw,
                    expected_policy: p.policy_generation,
                },
                &mut || Ok(()),
            )?;
        }
        self.invalidate();
        Ok(())
    }
    pub fn import_consent(
        &mut self,
        c: u8,
        raw: Vec<u8>,
        expected: Id,
        historical: bool,
    ) -> Result<()> {
        let s = self.snapshot(c)?;
        let peers = s
            .spec
            .credentials
            .iter()
            .map(|v| wire::ParsedCredentialBundle::parse(v)?.verify_root())
            .collect::<wire::Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| anyhow::anyhow!("peer pair"))?;
        let consent = wire::ParsedConsent2::parse(&raw)?;
        consent.verify(&peers)?;
        ensure!(
            consent.c() == s.head.context && consent.origin() == &wire::Origin::Fresh,
            "consent scope/origin"
        );
        let op = if historical {
            db::RuntimeOperation::HistoricalConsent {
                context: c,
                fence: s.fence(now()?),
                raw,
            }
        } else {
            db::RuntimeOperation::RenewConsent {
                context: c,
                fence: s.fence(now()?),
                expected_ref: expected,
                raw,
            }
        };
        self.commit(&s, op, Purpose2::Import, (0, 0))?;
        if !historical {
            self.invalidate();
        }
        Ok(())
    }
}
