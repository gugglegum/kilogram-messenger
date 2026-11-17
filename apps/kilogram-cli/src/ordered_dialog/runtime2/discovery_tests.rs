use super::*;
use discovery::NetworkGeneration2;
use kilogram_transport_iroh::RoutePolicy;
use std::net::{Ipv4Addr, SocketAddr};
use wire::lan_discovery::{DATAGRAM_BYTES, ExpectedContext, Scope, seal};

const LOCAL: Ipv4Addr = Ipv4Addr::new(192, 168, 33, 2);
const PEER: Ipv4Addr = Ipv4Addr::new(192, 168, 33, 1);
const INSTANCE: Id = [19; 32];

fn configure_auto(owner: &mut Owner2, id: iroh::EndpointId) -> Result<iroh::EndpointAddr> {
    owner.route = transport::FreshRoute2::new(
        transport::RouteConfig2 {
            policy: RoutePolicy::DirectOnly,
            bind: Some(SocketAddr::from((LOCAL, 0))),
            relay: None,
        },
        Some(PEER),
    )?;
    let addr = iroh::EndpointAddr::new(id).with_ip_addr(SocketAddr::from((LOCAL, 41000)));
    // Represent the OS binding observation. The real endpoint below is loopback;
    // the TLS selected-path check is deliberately left in production form.
    owner.test_lan_advertisement = Some(addr.clone());
    for c in 0..2 {
        owner.rotate_locator(c, INSTANCE, &addr)?;
    }
    owner.enable_lan_discovery(NetworkGeneration2(1))?;
    Ok(addr)
}

fn status(owner: &mut Owner2, addr: &iroh::EndpointAddr) -> Result<serde_json::Value> {
    let request = wire::encode(&a([u(2), u(1)]), 128)?;
    let result = ipc2(owner, &request, INSTANCE, addr)?;
    let value = wire::decode(&result, 48 * 1024)?;
    Ok(serde_json::from_slice(value.fields(3)?[2].bytes()?)?)
}

fn peer_revisions(owner: &mut Owner2) -> Result<[u64; 2]> {
    Ok([
        owner.snapshot(0)?.spec.peer_locator_revision,
        owner.snapshot(1)?.spec.peer_locator_revision,
    ])
}

struct Announcement<'a> {
    actors: &'a [support::Actors; 2],
    slots: [ExpectedContext; 2],
    endpoint: Id,
    issued: u64,
}
impl Announcement<'_> {
    fn locators(&self, revisions: [u64; 2], socket: SocketAddr) -> Result<[Vec<u8>; 2]> {
        let locator = |c: usize| -> Result<Vec<u8>> {
            let actor = &self.actors[c];
            Ok(wire::ParsedRuntimeLocator2::sign(
                wire::RuntimeLocatorFields2 {
                    c: actor.c,
                    cert: actor.credentials[0].cert_ref(),
                    peer_cert: actor.credentials[1].cert_ref(),
                    endpoint: self.endpoint,
                    ips: vec![socket],
                    relay: None,
                    revision: revisions[c],
                    not_before: self.issued - 1,
                    expires: self.issued + 300,
                },
                &actor.devices[0],
            )?
            .raw()
            .to_vec())
        };
        Ok([locator(0)?, locator(1)?])
    }
    fn seal(
        &self,
        slots: [ExpectedContext; 2],
        locators: &[Vec<u8>; 2],
        ip: Ipv4Addr,
        issued: u64,
    ) -> Result<[u8; DATAGRAM_BYTES]> {
        let actor = &self.actors[0];
        Ok(seal(
            &Scope {
                sender: &actor.credentials[0],
                recipient: &actor.credentials[1],
                slots,
                sender_ipv4: ip,
                now: issued,
            },
            &actor.devices[0],
            issued,
            issued + 45,
            locators.each_ref().map(Vec::as_slice),
        )?)
    }
}

async fn current_slots(pair: &Actors2) -> Result<[ExpectedContext; 2]> {
    pair.handles[1]
        .call(|owner| {
            let slot = |owner: &mut Owner2, c| -> Result<ExpectedContext> {
                let (s, current) = owner.require(c, false)?;
                Ok(ExpectedContext {
                    context: s.head.context,
                    consent_ref: current.consent.reference(),
                    consent_interval: current.consent.interval(),
                })
            };
            Ok([slot(owner, 0)?, slot(owner, 1)?])
        })
        .await
}

async fn connect_probe(pair: &Actors2) -> Result<(Connection, Connection)> {
    Ok(tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(
            async {
                Ok::<_, anyhow::Error>(
                    pair.endpoints[0]
                        .connect(pair.endpoints[1].addr(), wire::ALPN)
                        .await?,
                )
            },
            async {
                Ok::<_, anyhow::Error>(
                    pair.endpoints[1]
                        .accept()
                        .await
                        .context("probe incoming")?
                        .await?,
                )
            },
        )
    })
    .await??)
}

#[tokio::test]
async fn t13c_partial_pair_blocks_actual_authorization_and_repairs_after_reopen() -> Result<()> {
    let mut pair = Actors2::new(false).await?;
    pair.connect(0).await?;
    pair.fresh(0, false).await?;
    let hello = pair.handles[0]
        .call(|owner| {
            let lease = owner.leases[0].as_ref().context("manual admission")?;
            let frames = wire::decode(&lease.pair, 192 * 1024)?;
            Ok(wire::ParsedFrame2::parse(frames.fields(6)?[2].bytes()?)?)
        })
        .await?;
    let (dialed, accepted) = connect_probe(&pair).await?;
    let local = pair.endpoints[1].id();
    let binding = transport::binding(&accepted, *local.as_bytes(), false)?;
    let actor = &pair.artifacts[0];
    let renewal = actor
        .renewal(0, 3)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut fields = hello.body().value().fields(12)?.to_vec();
    fields[7] = b(renewal.raw());
    let renewed = wire::ParsedFrame2::sign(
        wire::FrameBody2::parse(
            wire::FrameKind::Hello,
            &wire::encode(&a(fields), 96 * 1024)?,
        )?,
        binding,
        hello.request(),
        &actor.devices[0],
    )?;
    let ann = Announcement {
        actors: &pair.artifacts,
        slots: current_slots(&pair).await?,
        endpoint: *pair.endpoints[0].id().as_bytes(),
        issued: now()?,
    };
    let socket = SocketAddr::from((PEER, 42000));
    let initial_pair = ann.seal(ann.slots, &ann.locators([2, 2], socket)?, PEER, ann.issued)?;
    let datagram = ann.seal(ann.slots, &ann.locators([3, 3], socket)?, PEER, ann.issued)?;
    pair.handles[1]
        .call(move |owner| {
            let addr = configure_auto(owner, local)?;
            let old = owner.capture_lan_discovery(NetworkGeneration2(1))?;
            ensure!(peer_revisions(owner)? == [1, 1]);
            ensure!(dial_peer(owner, 0).is_err() && owner.listener_pair(socket).is_err());
            owner.accept_lan_discovery(&old, &initial_pair)?;
            let before_update = owner.listener_pair(socket)?.context("first pair")?;
            let initial = owner.repository.generation();
            let mut commits = Vec::new();
            let failed =
                owner.commit_discovery_pair(&old, &datagram, now, |o, s, c, raw, wall, purpose| {
                    commits.push(c);
                    if c == 0 {
                        ensure!(o.listener_pair(socket).is_err());
                        return o.commit_peer_locator(s, c, raw, wall, purpose);
                    }
                    ensure!(peer_revisions(o)? == [3, 2]);
                    let snapshot = o.snapshot(0)?;
                    ensure!(dial_peer(o, 0).is_err() && o.listener_pair(socket).is_err());
                    ensure!(o.route_for_remote(ann.endpoint, None).is_err());
                    let error = o
                        .observe_hello_authority(
                            0,
                            &accepted,
                            binding,
                            &renewed,
                            Some(&before_update),
                        )
                        .err()
                        .context("early authority passed closed gate")?;
                    ensure!(error.to_string().contains("pair is closed"));
                    ensure!(
                        o.begin_admission(
                            0,
                            accepted.clone(),
                            binding,
                            Instant::now() + Duration::from_secs(10),
                            None,
                            Some(&before_update),
                        )
                        .is_err()
                    );
                    ensure!(o.admission_proof(0, 1).is_err());
                    let op = db::RuntimeOperation::PeerHead {
                        context: 0,
                        fence: snapshot.fence(wall),
                        next: 0,
                        last: None,
                    };
                    let error = o
                        .commit(&snapshot, op, owner::Purpose2::Peer, (0, 0))
                        .err()
                        .context("peer commit passed closed gate")?;
                    ensure!(error.to_string().contains("pair is closed"));
                    let view = status(o, &addr)?;
                    ensure!(view["contexts"][0]["live_admitted"].as_bool() == Some(false));
                    ensure!(view["contexts"][1]["live_admitted"].as_bool() == Some(false));
                    let import = wire::encode(&a([u(2), u(5), u(1), b(&raw)]), 4096)?;
                    ensure!(ipc2(o, &import, INSTANCE, &addr).is_err());
                    ensure!(o.repository.generation() == initial + 1);
                    // Fail the real second staged Locator commit at its authorization fence.
                    o.commit_peer_locator(
                        s,
                        c,
                        raw,
                        wall,
                        owner::Purpose2::DiscoveryLocator { expires: 0 },
                    )
                });
            let failure = failed
                .err()
                .context("second slot was unexpectedly committed")?;
            ensure!(
                format!("{failure:#}").contains("LAN discovery import expired"),
                "{failure:#}"
            );
            ensure!(commits == [0, 1]);
            ensure!(
                owner.repository.generation() == initial + 1 && peer_revisions(owner)? == [3, 2]
            );
            owner.recover()?;
            ensure!(peer_revisions(owner)? == [3, 2] && owner.listener_pair(socket).is_err());
            let post_reopen = owner.repository.generation();
            owner.accept_lan_discovery(&old, &datagram)?;
            ensure!(
                peer_revisions(owner)? == [3, 3]
                    && owner.repository.generation() == post_reopen + 1
            );
            ensure!(
                dial_peer(owner, 0)?
                    .0
                    .ip_addrs()
                    .eq(std::iter::once(&socket))
            );
            let token = owner.listener_pair(socket)?.context("open pair")?;
            ensure!(owner.route_for_remote(ann.endpoint, Some(&token))? == RoutePolicy::DirectOnly);
            ensure!(owner.route_for_remote([0; 32], Some(&token)).is_err());
            ensure!(
                owner
                    .listener_pair(SocketAddr::from((PEER, 42001)))
                    .is_err()
            );
            let before_queue = owner.repository.generation();
            owner.queue(0, [0xaf; 16], "local after pair".to_owned())?;
            ensure!(owner.repository.generation() == before_queue + 1);
            let s = owner.snapshot(0)?;
            owner.check_pair_attempt(&s, Some(&token))?;
            owner.withdraw_lan_discovery();
            owner.queue(0, [0xb0; 16], "local with gate closed".to_owned())?;
            owner.enable_lan_discovery(NetworkGeneration2(2))?;
            ensure!(owner.accept_lan_discovery(&old, &datagram).is_err());
            ensure!(owner.capture_lan_discovery(NetworkGeneration2(1)).is_err());
            ensure!(owner.listener_pair(socket).is_err());
            let fresh = owner.capture_lan_discovery(NetworkGeneration2(2))?;
            owner.accept_lan_discovery(&fresh, &datagram)?;
            let s = owner.snapshot(0)?;
            ensure!(owner.check_pair_attempt(&s, Some(&token)).is_err());
            accepted.close(0u32.into(), b"authorization probe done");
            Ok(())
        })
        .await?;
    dialed.close(0u32.into(), b"authorization probe done");
    Ok(())
}

#[tokio::test]
async fn t13c_replay_bad_candidates_and_policy_change() -> Result<()> {
    let pair = Actors2::new(false).await?;
    let (dialed, accepted) = connect_probe(&pair).await?;
    kilogram_transport_iroh::await_route_policy(
        &accepted,
        RoutePolicy::DirectOnly,
        Duration::from_secs(5),
    )
    .await?;
    let ann = Announcement {
        actors: &pair.artifacts,
        slots: current_slots(&pair).await?,
        endpoint: *pair.endpoints[0].id().as_bytes(),
        issued: now()?,
    };
    let issued = ann.issued;
    let endpoint = ann.endpoint;
    let socket = SocketAddr::from((PEER, 42000));
    let locators = ann.locators([3, 3], socket)?;
    let original = ann.seal(ann.slots, &locators, PEER, issued)?;
    let duplicate = ann.seal(ann.slots, &locators, PEER, issued + 1)?;
    ensure!(original != duplicate);
    let mut wrong_consent = ann.slots;
    wrong_consent[1].consent_ref[0] ^= 1;
    let stale_consent = ann.seal(wrong_consent, &locators, PEER, issued)?;
    let lower = ann.seal(ann.slots, &ann.locators([4, 2], socket)?, PEER, issued)?;
    let conflict = ann.seal(
        ann.slots,
        &ann.locators([3, 3], SocketAddr::from((PEER, 42001)))?,
        PEER,
        issued,
    )?;
    let other = Ipv4Addr::new(192, 168, 33, 3);
    let wrong_route = ann.seal(
        ann.slots,
        &ann.locators([4, 4], SocketAddr::from((other, 42000)))?,
        other,
        issued,
    )?;
    let mut altered = original;
    altered[DATAGRAM_BYTES - 1] ^= 1;
    let local = pair.endpoints[1].id();
    let binding = transport::binding(&accepted, *local.as_bytes(), false)?;
    let new_authority = pair.artifacts[0]
        .renewal(0, 3)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .authority()
        .encode()?;
    pair.handles[1]
        .call(move |owner| {
            ensure!(owner.enable_lan_discovery(NetworkGeneration2(1)).is_err());
            ensure!(!owner.auto_discovery());
            let loopback = owner.route.clone();
            let addr = configure_auto(owner, local)?;
            let observed = owner.capture_lan_discovery(NetworkGeneration2(1))?;
            owner.accept_lan_discovery(&observed, &original)?;
            let token = owner.listener_pair(socket)?.context("healthy pair")?;
            let generation = owner.repository.generation();
            let s = owner.snapshot(0)?;
            let valid = Arc::new(AtomicBool::new(true));
            // Exercise a lease's pair identity and lifetime with a real TLS
            // connection, using the loopback route for the selected-path check.
            owner.leases[0] = Some(owner::Lease2 {
                connection: accepted,
                binding,
                generation: 1,
                policy: s.profile.policy_generation,
                deadline: Instant::now() + Duration::from_secs(300),
                wall_deadline: issued + 300,
                own_locator: wire::ParsedRuntimeLocator2::parse(
                    s.spec.own_locator.as_deref().context("own")?,
                )?
                .id(),
                peer_locator: wire::ParsedRuntimeLocator2::parse(&locators[0])?.id(),
                valid: valid.clone(),
                pair: Vec::new(),
                discovery_pair: Some(token.clone()),
            });
            for datagram in [altered, stale_consent, lower, conflict, wrong_route] {
                ensure!(owner.accept_lan_discovery(&observed, &datagram).is_err());
            }
            let write_never = |_: &mut Owner2, _: &db::RuntimeSnapshot, _, _, _, _| {
                anyhow::bail!("duplicate must not reach the locator commit")
            };
            let duplicate_wall = (issued + 2).max(s.profile.wall_high_water);
            owner.commit_discovery_pair(
                &observed,
                &duplicate,
                || Ok(duplicate_wall),
                write_never,
            )?;
            ensure!(
                owner
                    .commit_discovery_pair(&observed, &original, || Ok(issued + 46), write_never)
                    .is_err()
            );
            let s = owner.snapshot(0)?;
            let after_expiry = owner
                .pair_for(&s, issued + 46)?
                .context("announcement only expired")?;
            ensure!(Arc::ptr_eq(&token, after_expiry));
            ensure!(valid.load(Ordering::Acquire));
            let lease = owner.leases[0]
                .as_ref()
                .context("duplicate should keep lease")?;
            lease.check(&s, issued + 46, &loopback, None, Some(after_expiry))?;
            ensure!(
                lease
                    .check(&s, issued + 300, &loopback, None, Some(after_expiry))
                    .is_err()
            );
            ensure!(owner.pair_for(&s, issued + 300).is_err());
            ensure!(owner.repository.generation() == generation);
            owner.route_for_remote(endpoint, Some(&token))?;
            owner.test_lan_advertisement =
                Some(addr.with_ip_addr(SocketAddr::from((LOCAL, 41001))));
            ensure!(owner.listener_pair(socket).is_err());
            owner.test_lan_advertisement = owner.endpoint.as_ref().map(|(_, a)| a.clone());
            owner.import_authority(new_authority)?;
            ensure!(!valid.load(Ordering::Acquire));
            ensure!(owner.accept_lan_discovery(&observed, &original).is_err());
            ensure!(owner.listener_pair(socket).is_err());
            let fresh = owner.capture_lan_discovery(NetworkGeneration2(1))?;
            owner.accept_lan_discovery(&fresh, &original)?;
            ensure!(owner.repository.generation() == generation + 1);
            ensure!(owner.route_for_remote(endpoint, Some(&token)).is_err());
            Ok(())
        })
        .await?;
    dialed.close(0u32.into(), b"lease probe done");
    Ok(())
}
