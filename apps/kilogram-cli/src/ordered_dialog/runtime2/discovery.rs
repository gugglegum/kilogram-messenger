use super::{
    owner::{Owner2, Purpose2},
    *,
};
use iroh::EndpointAddr;
use std::{net::SocketAddr, sync::Arc};
use wire::lan_discovery::{ExpectedContext, Scope, open};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NetworkGeneration2(pub u64);

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
    verified: bool,
    pair: Option<Arc<LocatorPair2>>,
}

#[derive(PartialEq, Eq)]
pub(super) struct LocatorPair2 {
    observation: DiscoveryObservation2,
    contexts: [Id; 2],
    consents: [Id; 2],
    peer_locators: [Id; 2],
    peer_revisions: [u64; 2],
    peer_endpoint: Id,
    peer_socket: SocketAddr,
    expires: u64,
}
impl LocatorPair2 {
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
    pub(super) fn auto_discovery(&self) -> bool {
        matches!(self.discovery, Discovery2::Auto(_))
    }

    /// Start an explicit Auto generation on the verified bound LAN endpoint.
    /// Runtime construction remains Manual until the internal integration opts in.
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
            verified: true,
            pair: None,
        });
        Ok(())
    }

    /// Stop using a generation as soon as its network observation is lost.
    /// The next enabling observation must carry a strictly newer generation.
    pub fn withdraw_lan_discovery(&mut self) {
        if let Discovery2::Auto(auto) = &mut self.discovery {
            auto.verified = false;
            self.close_discovery_pair();
        }
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
            auto.pair = None;
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
            Discovery2::Auto(auto) if auto.verified => Ok(auto.network),
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

    /// Re-open the wire datagram with current owner pins, then commit the exact pair.
    pub fn accept_lan_discovery(
        &mut self,
        observed: &DiscoveryObservation2,
        datagram: &[u8],
    ) -> Result<()> {
        self.commit_discovery_pair(observed, datagram, now, Self::commit_peer_locator)
    }

    pub(super) fn commit_discovery_pair(
        &mut self,
        observed: &DiscoveryObservation2,
        datagram: &[u8],
        wall: impl Fn() -> Result<u64>,
        mut write: impl FnMut(&mut Self, &db::RuntimeSnapshot, u8, Vec<u8>, u64, Purpose2) -> Result<()>,
    ) -> Result<()> {
        let now = wall()?;
        let local = self.discovery_scope(self.discovery_network()?, now)?;
        ensure!(
            local.observation == *observed,
            "stale LAN discovery observation"
        );
        let lan = self.route.lan().context("LAN discovery route")?;
        let role = usize::from(local.snapshots[0].head.role);
        let scope = Scope {
            sender: &local.currents[0].peers[1 - role],
            recipient: &local.currents[0].peers[role],
            slots: local.slots,
            sender_ipv4: lan.peer(),
            now,
        };
        let candidate = open(datagram, &scope, &self.encryption)?;
        let mut pair = LocatorPair2 {
            observation: observed.clone(),
            contexts: local.slots.map(|slot| slot.context),
            consents: local.slots.map(|slot| slot.consent_ref),
            peer_locators: candidate.locators.each_ref().map(|locator| locator.id()),
            peer_revisions: candidate
                .locators
                .each_ref()
                .map(|locator| locator.fields().revision),
            peer_endpoint: candidate.locators[0].fields().endpoint,
            peer_socket: candidate.socket,
            expires: local.expires,
        };
        for (c, locator) in candidate.locators.iter().enumerate() {
            let s = &local.snapshots[c];
            let role = usize::from(s.head.role);
            locator.verify(
                s.head.context,
                &local.currents[c].peers[1 - role],
                &local.currents[c].peers[role],
            )?;
            self.route.remote(locator)?;
            Self::check_peer_locator_revision(s, locator)?;
            pair.expires = pair.expires.min(locator.fields().expires);
        }
        if matches!(&self.discovery, Discovery2::Auto(auto) if auto.pair.as_deref() == Some(&pair))
        {
            ensure!(
                candidate
                    .locators
                    .iter()
                    .zip(&local.snapshots)
                    .all(|(locator, s)| { s.spec.peer_locator.as_deref() == Some(locator.raw()) }),
                "LAN discovery duplicate differs from storage"
            );
            return Ok(());
        }
        self.close_discovery_pair();
        let import_expires = pair.expires.min(candidate.announcement_interval.1);
        for (c, locator) in (0..2u8).zip(&candidate.locators) {
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
                ensure!(commit_wall < import_expires, "LAN discovery import expired");
                write(
                    self,
                    &s,
                    c,
                    locator.raw().to_vec(),
                    commit_wall,
                    Purpose2::DiscoveryLocator {
                        expires: import_expires,
                    },
                )?;
            }
        }
        let now = wall()?;
        let latest = self.discovery_scope(self.discovery_network()?, now)?;
        ensure!(
            latest.observation == *observed && now < import_expires,
            "LAN discovery changed during commit"
        );
        for (s, locator) in latest.snapshots.iter().zip(&candidate.locators) {
            ensure!(
                s.spec.peer_locator.as_deref() == Some(locator.raw())
                    && s.spec.peer_locator_revision == locator.fields().revision,
                "LAN discovery pair was not stored"
            );
        }
        if let Discovery2::Auto(auto) = &mut self.discovery {
            auto.pair = Some(Arc::new(pair));
        }
        Ok(())
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
            auto.verified
                && auto.network == pair.observation.network
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
