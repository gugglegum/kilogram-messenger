use super::diagnostics::Rejection;
use super::{
    owner::{Owner2, Purpose2},
    *,
};
use iroh::EndpointAddr;
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
#[cfg(any(windows, test))]
use wire::lan_discovery::seal;
use wire::lan_discovery::{
    DATAGRAM_BYTES, ExpectedContext, HintContext, HintScope, Scope,
    open_authenticated_hint_observed,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NetworkGeneration2(pub u64);
pub(super) type TargetChanges2 = tokio::sync::watch::Receiver<Option<Arc<LocatorPair2>>>;
pub(super) type ContextTargets2 = [TargetChanges2; 2];
type DiscoveryTarget2 = tokio::sync::watch::Sender<Option<Arc<LocatorPair2>>>;
const HINT_LIFETIME: Duration = Duration::from_secs(45);
const HINT_RETRY_COOLDOWN: Duration = Duration::from_secs(15);

/// Created at the socket boundary; owner queueing consumes this same budget.
pub(super) struct ReceivedHint2 {
    pub(super) raw: Vec<u8>,
    network: NetworkGeneration2,
    received_at: Instant,
}
impl ReceivedHint2 {
    pub(super) fn capture(
        raw: [u8; DATAGRAM_BYTES],
        network: NetworkGeneration2,
        received_at: Instant,
    ) -> Self {
        Self {
            raw: raw.to_vec(),
            network,
            received_at,
        }
    }
    fn deadline(&self) -> Result<Instant> {
        self.received_at
            .checked_add(HINT_LIFETIME)
            .context("hint receipt deadline overflow")
    }
}

#[derive(Clone)]
pub(super) struct AdmissionPermit2 {
    pub(super) network: Arc<NetworkFreshness2>,
    target: TargetChanges2,
    pair: Arc<LocatorPair2>,
    floor: Arc<AtomicU64>,
    deadline: Instant,
    connection: iroh::endpoint::Connection,
    slot: u8,
}
impl AdmissionPermit2 {
    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(super) fn check_context(&self, slot: u8) -> Result<()> {
        ensure!(slot == self.slot, "hint source context differs");
        self.check()
    }
    pub(super) fn check(&self) -> Result<()> {
        let wall = now()?;
        ensure!(
            self.network.is_current()
                && Instant::now() < self.deadline
                && self.connection.close_reason().is_none()
                && self.connection.remote_id().as_bytes() == &self.pair.peer_endpoint
                && wall >= self.floor.load(Ordering::Acquire)
                && self.pair.valid_at(wall),
            "LAN admission permission expired"
        );
        same_pair(Some(&self.pair), self.target.borrow().as_ref())
    }
}

#[cfg(any(windows, test))]
pub(super) struct Publication2 {
    raw: [u8; DATAGRAM_BYTES],
    network: Arc<NetworkFreshness2>,
    valid: Arc<AtomicBool>,
    floor: Arc<AtomicU64>,
    issued: u64,
    expires: u64,
}
#[cfg(any(windows, test))]
impl Publication2 {
    pub(super) fn send_if_current(
        &self,
        clock: impl FnOnce() -> Result<u64>,
        send: impl FnOnce(&[u8; DATAGRAM_BYTES]) -> Result<()>,
    ) -> Result<()> {
        if let Ok(wall) = clock()
            && wall >= self.issued
            && wall >= self.floor.load(Ordering::Acquire)
            && wall < self.expires
            && self.network.is_current()
            && self.valid.load(Ordering::Acquire)
        {
            send(&self.raw)?;
        }
        Ok(())
    }
}

pub(super) const DISCOVERY_VALIDATIONS_PER_SECOND: usize = 8;

#[derive(Default)]
pub(super) struct DiscoveryQuota2(VecDeque<Instant>);

impl DiscoveryQuota2 {
    pub(super) fn take(&mut self, now: Instant) -> bool {
        while self
            .0
            .front()
            .is_some_and(|first| now.duration_since(*first) >= Duration::from_secs(1))
        {
            self.0.pop_front();
        }
        if self.0.len() == DISCOVERY_VALIDATIONS_PER_SECOND {
            return false;
        }
        self.0.push_back(now);
        true
    }
}

/// One observed network generation. An expired or withdrawn token cannot be renewed.
pub(super) struct NetworkFreshness2(Mutex<Option<Instant>>);
impl NetworkFreshness2 {
    #[cfg(any(windows, test))]
    pub(super) fn new(deadline: Instant) -> Self {
        Self(Mutex::new(Some(deadline)))
    }

    pub(super) fn is_current(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|deadline| deadline.is_some_and(|end| Instant::now() < end))
    }

    #[cfg(windows)]
    pub(super) fn renew(&self, deadline: Instant) -> bool {
        let Ok(mut current) = self.0.lock() else {
            return false;
        };
        let now = Instant::now();
        if current.is_some_and(|old| now < old) && now < deadline {
            *current = Some(deadline);
            true
        } else {
            *current = None;
            false
        }
    }

    pub(super) fn revoke(&self) {
        if let Ok(mut deadline) = self.0.lock() {
            *deadline = None;
        }
    }
}

/// Capture at receipt, before handing a datagram to the owner for processing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryObservation2 {
    network: NetworkGeneration2,
    route: wire::DirectLan2,
    policy: u64,
    endpoint: (Id, EndpointAddr),
    own_locators: [Id; 2],
}

#[derive(Default)]
pub(super) enum Discovery2 {
    #[default]
    Manual,
    Auto(Auto2),
}

pub(super) struct Auto2 {
    network: NetworkGeneration2,
    freshness: Option<Arc<NetworkFreshness2>>,
    pair: Option<Arc<LocatorPair2>>,
    target: [DiscoveryTarget2; 2],
}

pub(super) struct LocatorPair2 {
    observation: DiscoveryObservation2,
    contexts: [Id; 2],
    consents: [Id; 2],
    peer_locators: [Id; 2],
    peer_revisions: [u64; 2],
    peer_endpoint: Id,
    peer_socket: SocketAddr,
    expires: u64,
    budget: Mutex<HintBudget2>,
}
struct HintBudget2 {
    deadline: Instant,
    retry_after: Instant,
}
impl PartialEq for LocatorPair2 {
    fn eq(&self, other: &Self) -> bool {
        self.observation == other.observation
            && self.contexts == other.contexts
            && self.consents == other.consents
            && self.peer_locators == other.peer_locators
            && self.peer_revisions == other.peer_revisions
            && self.peer_endpoint == other.peer_endpoint
            && self.peer_socket == other.peer_socket
            && self.expires == other.expires
    }
}
impl Eq for LocatorPair2 {}
impl LocatorPair2 {
    pub(super) fn candidate_deadline(&self, at: Instant) -> Result<Instant> {
        let budget = self
            .budget
            .lock()
            .map_err(|_| anyhow::anyhow!("hint budget poisoned"))?;
        ensure!(at < budget.deadline, "hint receipt budget expired");
        Ok(budget.deadline)
    }
    fn renew_untrusted_budget(&self, receipt: &ReceivedHint2, at: Instant) -> Result<bool> {
        let mut budget = self
            .budget
            .lock()
            .map_err(|_| anyhow::anyhow!("hint budget poisoned"))?;
        if at < budget.deadline || receipt.received_at < budget.retry_after {
            return Ok(false);
        }
        let deadline = receipt.deadline()?;
        ensure!(at < deadline, "hint receipt budget expired");
        *budget = HintBudget2 {
            deadline,
            retry_after: deadline
                .checked_add(HINT_RETRY_COOLDOWN)
                .context("hint retry deadline overflow")?,
        };
        Ok(true)
    }
    pub(super) fn valid_at(&self, wall: u64) -> bool {
        wall < self.expires
    }

    pub(super) fn expires(&self) -> u64 {
        self.expires
    }
}

struct DiscoveryScope2 {
    snapshots: [db::RuntimeSnapshot; 2],
    currents: [policy::Current2; 2],
    slots: [ExpectedContext; 2],
    observation: DiscoveryObservation2,
    expires: u64,
}

fn locator_id(raw: &[u8]) -> Id {
    wire::hash(wire::DESCRIPTOR, raw)
}

pub(super) fn same_pair(
    expected: Option<&Arc<LocatorPair2>>,
    current: Option<&Arc<LocatorPair2>>,
) -> Result<()> {
    ensure!(
        match (expected, current) {
            (None, None) => true,
            (Some(expected), Some(current)) => Arc::ptr_eq(expected, current),
            _ => false,
        },
        "LAN discovery attempt is stale"
    );
    Ok(())
}

impl Owner2 {
    /// Candidate timing controls only unverified work, never a healthy lease.
    pub(super) fn expire_lan_candidates(&mut self, at: Instant) {
        let (pair, targets) = match &self.discovery {
            Discovery2::Auto(auto) => (auto.pair.clone(), auto.target.clone()),
            _ => return,
        };
        let Some(pair) = pair else { return };
        if pair.candidate_deadline(at).is_ok() {
            return;
        }
        let wall = now().ok();
        for (slot, target) in targets.iter().enumerate() {
            let healthy = self.leases[slot].as_ref().is_some_and(|lease| {
                lease.valid.load(Ordering::Acquire)
                    && lease.connection.close_reason().is_none()
                    && at < lease.deadline
                    && wall.is_some_and(|w| w < lease.wall_deadline)
                    && lease.policy == pair.observation.policy
                    && lease
                        .discovery_pair
                        .as_ref()
                        .is_some_and(|p| Arc::ptr_eq(p, &pair))
            });
            if !healthy {
                if target.borrow().is_some() {
                    target.send_replace(None);
                }
                if let Some(pending) = self.admissions[slot].take() {
                    pending
                        .connection
                        .close(0u32.into(), b"hint candidate budget expired");
                }
            }
        }
    }

    pub(super) fn hint_attempt_deadline(
        &self,
        slot: u8,
        pair: &Arc<LocatorPair2>,
        at: Instant,
    ) -> Result<Instant> {
        let Discovery2::Auto(auto) = &self.discovery else {
            anyhow::bail!("hint requires Auto mode")
        };
        let target = auto
            .target
            .get(usize::from(slot))
            .context("hint context slot")?;
        same_pair(Some(pair), target.borrow().as_ref())?;
        pair.candidate_deadline(at)
    }

    pub(super) fn auto_discovery(&self) -> bool {
        matches!(self.discovery, Discovery2::Auto(_))
    }

    pub(super) fn select_auto_discovery(&mut self) {
        self.publication_valid.store(false, Ordering::Release);
        self.clear_discovery_admissions();
        self.discovery = Discovery2::Auto(Auto2 {
            network: NetworkGeneration2(0),
            freshness: None,
            pair: None,
            target: std::array::from_fn(|_| tokio::sync::watch::channel(None).0),
        });
    }

    pub(super) fn network_freshness(&self) -> Option<&Arc<NetworkFreshness2>> {
        match &self.discovery {
            Discovery2::Manual => None,
            Discovery2::Auto(auto) => auto.freshness.as_ref(),
        }
    }

    pub(super) fn discovery_target(&self) -> Option<[DiscoveryTarget2; 2]> {
        match &self.discovery {
            Discovery2::Manual => None,
            Discovery2::Auto(auto) => Some(auto.target.clone()),
        }
    }

    #[cfg(test)]
    pub fn enable_lan_discovery(&mut self, network: NetworkGeneration2) -> Result<()> {
        if let Discovery2::Auto(auto) = &self.discovery {
            ensure!(
                network > auto.network,
                "LAN discovery generation must advance"
            );
            self.withdraw_lan_discovery();
        }
        self.discovery_scope(network, now()?)?;
        // An attempt started in Manual cannot finish after switching modes.
        self.clear_discovery_admissions();
        self.discovery = Discovery2::Auto(Auto2 {
            network,
            freshness: Some(Arc::new(NetworkFreshness2::new(
                Instant::now() + std::time::Duration::from_secs(120),
            ))),
            pair: None,
            target: std::array::from_fn(|_| tokio::sync::watch::channel(None).0),
        });
        Ok(())
    }

    #[cfg(windows)]
    pub(super) fn attach_lan_generation(
        &mut self,
        network: NetworkGeneration2,
        freshness: Arc<NetworkFreshness2>,
        instance: Id,
        endpoint: iroh::Endpoint,
    ) -> Result<ContextTargets2> {
        let Discovery2::Auto(auto) = &self.discovery else {
            anyhow::bail!("LAN Auto required")
        };
        ensure!(
            network > auto.network && freshness.is_current(),
            "stale LAN generation"
        );
        let addr = self.route.advertisement(&endpoint)?;
        let target = auto.target.clone();
        self.withdraw_lan_discovery();
        self.endpoint = Some((instance, addr));
        self.lan_endpoint = Some(endpoint);
        self.exported = [None, None];
        let changes = target.each_ref().map(|target| target.subscribe());
        self.discovery = Discovery2::Auto(Auto2 {
            network,
            freshness: Some(freshness),
            pair: None,
            target,
        });
        Ok(changes)
    }

    #[cfg(windows)]
    pub(super) fn detach_lan_generation(&mut self, network: NetworkGeneration2) {
        if matches!(&self.discovery, Discovery2::Auto(auto) if auto.network == network) {
            self.withdraw_lan_discovery();
            self.endpoint = None;
            self.lan_endpoint = None;
            self.exported = [None, None];
        }
    }

    /// Stop using a generation as soon as its network observation is lost.
    /// The next enabling observation must carry a strictly newer generation.
    pub fn withdraw_lan_discovery(&mut self) {
        if let Discovery2::Auto(auto) = &mut self.discovery {
            self.publication_valid.store(false, Ordering::Release);
            if let Some(freshness) = auto.freshness.take() {
                freshness.revoke();
            }
            self.close_discovery_pair();
        }
    }

    pub(super) fn admission_permit(
        &self,
        slot: u8,
        pair: Option<&Arc<LocatorPair2>>,
        deadline: Instant,
        connection: &iroh::endpoint::Connection,
    ) -> Result<Option<AdmissionPermit2>> {
        let Discovery2::Auto(auto) = &self.discovery else {
            return Ok(None);
        };
        let pair = pair.context("LAN admission pair missing")?;
        let target = auto
            .target
            .get(usize::from(slot))
            .context("hint context slot")?;
        let deadline = deadline.min(self.hint_attempt_deadline(slot, pair, Instant::now())?);
        let permission = AdmissionPermit2 {
            network: self
                .network_freshness()
                .context("LAN network unavailable")?
                .clone(),
            target: target.subscribe(),
            pair: pair.clone(),
            floor: self.clock_high_water.clone(),
            deadline,
            connection: connection.clone(),
            slot,
        };
        permission.check()?;
        Ok(Some(permission))
    }

    fn clear_discovery_admissions(&mut self) {
        self.invalidate();
        for admission in &mut self.admissions {
            if let Some(old) = admission.take() {
                old.connection
                    .close(0u32.into(), b"LAN discovery pair changed");
            }
        }
    }

    pub(super) fn close_discovery_pair(&mut self) {
        if let Discovery2::Auto(auto) = &mut self.discovery {
            if auto.pair.take().is_some() {
                for target in &auto.target {
                    target.send_replace(None);
                }
            }
            self.clear_discovery_admissions();
        }
    }

    fn local_discovery_endpoint(&self) -> Result<&(Id, EndpointAddr)> {
        let local = self
            .endpoint
            .as_ref()
            .context("LAN discovery local endpoint")?;
        self.check_lan_advertisement(&local.1)?;
        Ok(local)
    }

    fn discovery_scope(
        &mut self,
        network: NetworkGeneration2,
        wall: u64,
    ) -> Result<DiscoveryScope2> {
        let lan = self
            .route
            .lan()
            .context("LAN discovery requires an exact LAN route")?;
        let endpoint = self.local_discovery_endpoint()?.clone();
        ensure!(
            !self.needs_reopen,
            "LAN discovery repository requires recovery"
        );
        let snapshots = [self.snapshot(0)?, self.snapshot(1)?];
        let currents = [
            self.current(&snapshots[0], wall)?,
            self.current(&snapshots[1], wall)?,
        ];
        let mut own_locators = [[0; 32]; 2];
        let mut expires = u64::MAX;
        let slots = std::array::from_fn(|c| ExpectedContext {
            context: snapshots[c].head.context,
            consent_ref: currents[c].consent.reference(),
            consent_interval: currents[c].consent.interval(),
        });
        let policy = snapshots[0].profile.policy_generation;
        let certs = |c: usize| {
            let s = &snapshots[c];
            let own = usize::from(s.head.role);
            (s.spec.certs[own], s.spec.certs[1 - own])
        };
        ensure!(
            certs(0) == certs(1),
            "LAN discovery credential pair disagrees"
        );
        ensure!(
            slots[0].context != slots[1].context,
            "LAN discovery requires two contexts"
        );
        for (c, s) in snapshots.iter().enumerate() {
            let current = &currents[c];
            current.check_time(s, wall)?;
            let own = wire::ParsedRuntimeLocator2::parse(
                s.spec
                    .own_locator
                    .as_deref()
                    .context("LAN discovery own locator")?,
            )?;
            let role = usize::from(s.head.role);
            own.verify(
                s.head.context,
                &current.peers[role],
                &current.peers[1 - role],
            )?;
            let socket = own.direct_lan_socket(lan.local())?;
            let f = own.fields();
            ensure!(
                s.profile.policy_generation == policy
                    && f.endpoint == *endpoint.1.id.as_bytes()
                    && endpoint.1.ip_addrs().eq(std::iter::once(&socket))
                    && f.not_before <= wall
                    && wall < f.expires,
                "LAN discovery local locator or policy changed"
            );
            own_locators[c] = own.id();
            expires = expires.min(f.expires).min(slots[c].consent_interval.1);
        }
        Ok(DiscoveryScope2 {
            snapshots,
            currents,
            slots,
            observation: DiscoveryObservation2 {
                network,
                route: lan,
                policy,
                endpoint,
                own_locators,
            },
            expires,
        })
    }

    fn discovery_network(&self) -> Result<NetworkGeneration2> {
        match &self.discovery {
            Discovery2::Auto(auto)
                if auto
                    .freshness
                    .as_ref()
                    .is_some_and(|proof| proof.is_current()) =>
            {
                Ok(auto.network)
            }
            _ => anyhow::bail!("LAN discovery generation is unavailable"),
        }
    }

    pub fn capture_lan_discovery(
        &mut self,
        network: NetworkGeneration2,
    ) -> Result<DiscoveryObservation2> {
        ensure!(
            network == self.discovery_network()?,
            "stale LAN discovery generation"
        );
        Ok(self.discovery_scope(network, now()?)?.observation)
    }

    #[cfg(any(windows, test))]
    pub(super) fn announce_lan(
        &mut self,
        network: NetworkGeneration2,
    ) -> Result<(Publication2, DiscoveryObservation2)> {
        ensure!(
            self.discovery_network()? == network,
            "stale LAN discovery generation"
        );
        let (instance, addr) = self.local_discovery_endpoint()?.clone();
        for c in 0..2 {
            if self.locator_needs_rotation(c, instance, &addr)? {
                self.rotate_locator(c, instance, &addr)?;
            }
        }
        let now = now()?;
        let local = self.discovery_scope(network, now)?;
        let role = usize::from(local.snapshots[0].head.role);
        let scope = Scope {
            sender: &local.currents[0].peers[role],
            recipient: &local.currents[0].peers[1 - role],
            slots: local.slots,
            sender_ipv4: local.observation.route.local(),
            now,
        };
        let expires = now
            .checked_add(45)
            .context("LAN announcement expiry")?
            .min(local.expires);
        let [first, second] = &local.snapshots;
        let raw = seal(
            &scope,
            &self.device,
            now,
            expires,
            [
                first
                    .spec
                    .own_locator
                    .as_deref()
                    .context("first own locator")?,
                second
                    .spec
                    .own_locator
                    .as_deref()
                    .context("second own locator")?,
            ],
        )?;
        ensure!(
            self.discovery_network()? == network,
            "LAN network changed while signing"
        );
        if !self.publication_valid.load(Ordering::Acquire) {
            self.publication_valid = Arc::new(AtomicBool::new(true));
        }
        let publication = Publication2 {
            raw,
            network: self
                .network_freshness()
                .context("LAN network unavailable")?
                .clone(),
            valid: self.publication_valid.clone(),
            floor: self.clock_high_water.clone(),
            issued: now,
            expires,
        };
        Ok((publication, local.observation))
    }

    /// Re-open the wire datagram with current owner pins, then commit the exact pair.
    #[cfg(test)]
    pub fn accept_lan_discovery(
        &mut self,
        observed: &DiscoveryObservation2,
        datagram: &[u8],
    ) -> Result<()> {
        let receipt = ReceivedHint2 {
            raw: datagram.to_vec(),
            network: observed.network,
            received_at: Instant::now(),
        };
        self.accept_received_lan_hint(observed, &receipt)
    }

    pub(super) fn accept_received_lan_hint(
        &mut self,
        observed: &DiscoveryObservation2,
        receipt: &ReceivedHint2,
    ) -> Result<()> {
        self.diagnostics
            .event(super::diagnostics::Event::LocatorAttempt);
        let mut rejection = Rejection::GenerationFreshness;
        let prerequisite = (|| {
            ensure!(
                self.discovery_network()? == observed.network
                    && receipt.network == observed.network,
                "stale LAN discovery generation"
            );
            rejection = Rejection::ValidationQuota;
            ensure!(
                self.discovery_quota.take(Instant::now()),
                "LAN discovery validation quota"
            );
            Ok::<_, anyhow::Error>(())
        })();
        prerequisite.inspect_err(|_| self.diagnostics.rejected(rejection))?;
        self.commit_received_hint(
            observed,
            receipt,
            now,
            Instant::now,
            Self::commit_peer_locator,
        )
    }

    /// Synthetic call-site adapter. Production must supply the socket receipt.
    #[cfg(test)]
    pub(super) fn commit_discovery_pair(
        &mut self,
        observed: &DiscoveryObservation2,
        datagram: &[u8],
        wall: impl Fn() -> Result<u64>,
        write: impl FnMut(&mut Self, &db::RuntimeSnapshot, u8, Vec<u8>, u64, Purpose2) -> Result<()>,
    ) -> Result<()> {
        let receipt = ReceivedHint2 {
            raw: datagram.to_vec(),
            network: observed.network,
            received_at: Instant::now(),
        };
        self.commit_received_hint(observed, &receipt, wall, Instant::now, write)
    }

    pub(super) fn commit_received_hint(
        &mut self,
        observed: &DiscoveryObservation2,
        receipt: &ReceivedHint2,
        wall: impl Fn() -> Result<u64>,
        monotonic: impl Fn() -> Instant,
        mut write: impl FnMut(&mut Self, &db::RuntimeSnapshot, u8, Vec<u8>, u64, Purpose2) -> Result<()>,
    ) -> Result<()> {
        let mut rejection = Rejection::LocalScope;
        let result = (|| {
            let deadline = receipt.deadline()?;
            let at = monotonic();
            ensure!(
                receipt.received_at <= at && at < deadline,
                "hint receipt budget expired"
            );
            ensure!(
                receipt.network == self.discovery_network()?,
                "stale hint receipt generation"
            );
            let now = wall()?;
            let local = self.discovery_scope(self.discovery_network()?, now)?;
            rejection = Rejection::Observation;
            ensure!(
                local.observation == *observed,
                "stale LAN discovery observation"
            );
            rejection = Rejection::Route;
            let lan = self.route.lan().context("LAN discovery route")?;
            let role = usize::from(local.snapshots[0].head.role);
            let scope = HintScope {
                sender: &local.currents[0].peers[1 - role],
                recipient: &local.currents[0].peers[role],
                slots: local.slots.map(|slot| HintContext {
                    context: slot.context,
                    consent_ref: slot.consent_ref,
                }),
                sender_ipv4: lan.peer(),
            };
            let candidate = open_authenticated_hint_observed(
                &receipt.raw,
                &scope,
                &self.encryption,
                |reason| {
                    rejection = reason.into();
                },
            )?;
            self.diagnostics
                .event(super::diagnostics::Event::LocatorOpened);
            let pair = LocatorPair2 {
                observation: observed.clone(),
                contexts: local.slots.map(|slot| slot.context),
                consents: local.slots.map(|slot| slot.consent_ref),
                peer_locators: candidate.locators().each_ref().map(|locator| locator.id()),
                peer_revisions: candidate
                    .locators()
                    .each_ref()
                    .map(|locator| locator.fields().revision),
                peer_endpoint: candidate.locators()[0].fields().endpoint,
                peer_socket: candidate.socket(),
                expires: local.expires,
                budget: Mutex::new(HintBudget2 {
                    deadline,
                    retry_after: deadline
                        .checked_add(HINT_RETRY_COOLDOWN)
                        .context("hint retry deadline overflow")?,
                }),
            };
            for (c, locator) in candidate.locators().iter().enumerate() {
                let s = &local.snapshots[c];
                let role = usize::from(s.head.role);
                rejection = Rejection::PostLocatorVerification;
                locator.verify(
                    s.head.context,
                    &local.currents[c].peers[1 - role],
                    &local.currents[c].peers[role],
                )?;
                rejection = Rejection::PostRoute;
                self.route.remote(locator)?;
                rejection = Rejection::Revision;
                Self::check_peer_locator_revision(s, locator)?;
            }
            self.diagnostics
                .event(super::diagnostics::Event::LocatorValidated);
            rejection = Rejection::Commit;
            if matches!(&self.discovery, Discovery2::Auto(auto) if auto.pair.as_deref() == Some(&pair))
            {
                ensure!(
                    candidate
                        .locators()
                        .iter()
                        .zip(&local.snapshots)
                        .all(|(locator, s)| {
                            s.spec.peer_locator.as_deref() == Some(locator.raw())
                        }),
                    "LAN discovery duplicate differs from storage"
                );
                self.diagnostics
                    .event(super::diagnostics::Event::LocatorDuplicate);
                let old = match &self.discovery {
                    Discovery2::Auto(auto) => auto.pair.clone(),
                    _ => None,
                }
                .context("hint pair disappeared")?;
                if old.renew_untrusted_budget(receipt, monotonic())?
                    && let Discovery2::Auto(auto) = &self.discovery
                {
                    for target in &auto.target {
                        if target
                            .borrow()
                            .as_ref()
                            .is_none_or(|p| !Arc::ptr_eq(p, &old))
                        {
                            target.send_replace(Some(old.clone()));
                        }
                    }
                }
                return Ok(());
            }
            self.close_discovery_pair();
            let import_expires = pair.expires;
            for (c, locator) in (0..2u8).zip(candidate.locators()) {
                // The other slot can have advanced the database generation.
                let s = self.snapshot(c)?;
                ensure!(
                    self.discovery_network()? == observed.network
                        && s.profile.policy_generation == observed.policy
                        && self.route.lan() == Some(observed.route)
                        && self.local_discovery_endpoint()? == &observed.endpoint,
                    "LAN discovery changed before locator commit"
                );
                if s.spec.peer_locator.as_deref() != Some(locator.raw()) {
                    let commit_wall = wall()?;
                    ensure!(
                        commit_wall < import_expires && monotonic() < deadline,
                        "LAN discovery import expired"
                    );
                    write(
                        self,
                        &s,
                        c,
                        locator.raw().to_vec(),
                        commit_wall,
                        Purpose2::AutoLanPeerHint { deadline },
                    )?;
                }
            }
            let now = wall()?;
            let latest = self.discovery_scope(self.discovery_network()?, now)?;
            ensure!(
                latest.observation == *observed && now < import_expires && monotonic() < deadline,
                "LAN discovery changed during commit"
            );
            for (s, locator) in latest.snapshots.iter().zip(candidate.locators()) {
                ensure!(
                    s.spec.peer_locator.as_deref() == Some(locator.raw())
                        && s.spec.peer_locator_revision == locator.fields().revision,
                    "LAN discovery pair was not stored"
                );
            }
            if let Discovery2::Auto(auto) = &mut self.discovery {
                let pair = Arc::new(pair);
                auto.pair = Some(pair.clone());
                for target in &auto.target {
                    target.send_replace(Some(pair.clone()));
                }
            }
            self.diagnostics
                .event(super::diagnostics::Event::LocatorCommitted);
            Ok(())
        })();
        result.inspect_err(|_| self.diagnostics.rejected(rejection))
    }

    pub(super) fn pair_for(
        &self,
        s: &db::RuntimeSnapshot,
        wall: u64,
    ) -> Result<Option<&Arc<LocatorPair2>>> {
        let Discovery2::Auto(auto) = &self.discovery else {
            return Ok(None);
        };
        let pair = auto.pair.as_ref().context("LAN discovery pair is closed")?;
        let c = pair
            .contexts
            .iter()
            .position(|id| *id == s.head.context)
            .context("LAN discovery context")?;
        ensure!(
            self.discovery_network()? == pair.observation.network
                && !self.needs_reopen
                && s.profile.policy_generation == pair.observation.policy
                && s.profile.contexts == pair.contexts
                && wall >= s.profile.wall_high_water
                && pair.valid_at(wall)
                && self.local_discovery_endpoint()? == &pair.observation.endpoint
                && self.route.lan() == Some(pair.observation.route)
                && s.spec.own_locator.as_deref().map(locator_id)
                    == Some(pair.observation.own_locators[c])
                && s.spec.peer_locator.as_deref().map(locator_id) == Some(pair.peer_locators[c])
                && wire::hash(wire::CONSENT, &s.spec.consent) == pair.consents[c]
                && s.spec.peer_locator_revision == pair.peer_revisions[c],
            "LAN discovery pair is stale"
        );
        Ok(Some(pair))
    }

    pub(super) fn listener_pair(
        &mut self,
        source: SocketAddr,
    ) -> Result<Option<Arc<LocatorPair2>>> {
        let (s, _) = self.require(0, false)?;
        let pair = self
            .pair_for(&s, now()?)?
            .context("LAN discovery listener requires Auto")?;
        ensure!(
            pair.candidate_deadline(Instant::now()).is_ok(),
            "listener hint receipt budget expired"
        );
        ensure!(
            source == pair.peer_socket,
            "LAN discovery listener source differs from pair"
        );
        Ok(Some(pair.clone()))
    }

    pub(super) fn check_pair_attempt(
        &self,
        s: &db::RuntimeSnapshot,
        expected: Option<&Arc<LocatorPair2>>,
    ) -> Result<()> {
        let current = if self.auto_discovery() {
            self.pair_for(s, now()?)?
        } else {
            None
        };
        same_pair(expected, current)
    }

    pub(super) fn check_pair_remote(&self, s: &db::RuntimeSnapshot, remote: Id) -> Result<()> {
        if self.auto_discovery()
            && let Some(pair) = self.pair_for(s, now()?)?
        {
            ensure!(
                pair.peer_endpoint == remote,
                "LAN discovery TLS endpoint differs from pair"
            );
        }
        Ok(())
    }
}
