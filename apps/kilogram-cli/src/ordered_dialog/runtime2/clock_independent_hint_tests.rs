//! Actual synthetic Owner2/state integration. No preserved field profiles.
use super::*;
use discovery::{NetworkGeneration2, ReceivedHint2};
use kilogram_transport_iroh::RoutePolicy;
use std::net::{Ipv4Addr, SocketAddr};
use wire::lan_discovery::{ExpectedContext, Scope, seal};
const LOCAL: Ipv4Addr = Ipv4Addr::new(192, 168, 44, 2);
const PEER: Ipv4Addr = Ipv4Addr::new(192, 168, 44, 1);

fn configure(owner: &mut Owner2, id: iroh::EndpointId) -> Result<()> {
    owner.route = transport::FreshRoute2::new(
        transport::RouteConfig2 {
            policy: RoutePolicy::DirectOnly,
            bind: Some((LOCAL, 0).into()),
            relay: None,
        },
        Some(PEER),
    )?;
    let address = iroh::EndpointAddr::new(id).with_ip_addr(SocketAddr::from((LOCAL, 41000)));
    owner.test_lan_advertisement = Some(address.clone());
    for c in 0..2 {
        owner.rotate_locator(c, [29; 32], &address)?;
    }
    owner.enable_lan_discovery(NetworkGeneration2(1))
}
async fn slots(fixtures: &Actors2) -> Result<[ExpectedContext; 2]> {
    fixtures.handles[1]
        .call(|o| {
            let mut slots = Vec::new();
            for c in 0..2 {
                let (s, current) = o.require(c, false)?;
                slots.push(ExpectedContext {
                    context: s.head.context,
                    consent_ref: current.consent.reference(),
                    consent_interval: current.consent.interval(),
                });
            }
            Ok(slots.try_into().map_err(|_| anyhow::anyhow!("two slots"))?)
        })
        .await
}
fn announcement(
    fixtures: &Actors2,
    slots: [ExpectedContext; 2],
    issued: u64,
    revisions: [u64; 2],
    port: u16,
) -> Result<[u8; 759]> {
    announcement_from(fixtures, slots, issued, revisions, port, 0)
}
fn announcement_from(
    fixtures: &Actors2,
    slots: [ExpectedContext; 2],
    issued: u64,
    revisions: [u64; 2],
    port: u16,
    sender: usize,
) -> Result<[u8; 759]> {
    let mut locators = Vec::new();
    for c in 0..2 {
        let actor = &fixtures.artifacts[c];
        locators.push(
            wire::ParsedRuntimeLocator2::sign(
                wire::RuntimeLocatorFields2 {
                    c: actor.c,
                    cert: actor.credentials[sender].cert_ref(),
                    peer_cert: actor.credentials[1 - sender].cert_ref(),
                    endpoint: *fixtures.endpoints[sender].id().as_bytes(),
                    ips: vec![(PEER, port).into()],
                    relay: None,
                    revision: revisions[c],
                    not_before: issued.saturating_sub(1),
                    expires: issued + 300,
                },
                &actor.devices[sender],
            )?
            .raw()
            .to_vec(),
        );
    }
    let actor = &fixtures.artifacts[0];
    // Only the producer's own synthetic calendar is shifted. No fake receiver now.
    let sender_slots = slots.map(|mut slot| {
        slot.consent_interval = (issued.saturating_sub(60), issued + 600);
        slot
    });
    Ok(seal(
        &Scope {
            sender: &actor.credentials[sender],
            recipient: &actor.credentials[1 - sender],
            slots: sender_slots,
            sender_ipv4: PEER,
            now: issued,
        },
        &actor.devices[sender],
        issued,
        issued + 45,
        [&locators[0], &locators[1]],
    )?)
}

#[tokio::test]
async fn a2hint_owner_state_symmetric_dates_and_real_wall_fence() -> Result<()> {
    for receiver in 0..2 {
        let fixtures = Actors2::new(false).await?;
        let slots = slots(&fixtures).await?;
        let wall = now()?;
        let raws = [
            -86400_i64, -3600, -60, -10, -5, -3, -1, 0, 1, 3, 5, 10, 60, 3600, 86400,
        ]
        .into_iter()
        .enumerate()
        .map(|(n, delta)| {
            announcement_from(
                &fixtures,
                slots,
                wall.checked_add_signed(delta).context("synthetic date")?,
                [n as u64 + 2; 2],
                42000,
                1 - receiver,
            )
        })
        .collect::<Result<Vec<_>>>()?;
        let id = fixtures.endpoints[receiver].id();
        fixtures.handles[receiver]
        .call(move |o| {
            configure(o, id)?;
            let observed = o.capture_lan_discovery(NetworkGeneration2(1))?;
            let before_wall = o.repository.profile()?.wall_high_water;
            for (n, raw) in raws.into_iter().enumerate() {
                o.discovery_quota = discovery::DiscoveryQuota2::default();
                let receipt = ReceivedHint2::capture(raw, NetworkGeneration2(1), Instant::now());
                o.accept_received_lan_hint(&observed, &receipt)?;
                for c in 0..2 {
                    ensure!(o.snapshot(c)?.spec.peer_locator_revision == n as u64 + 2);
                }
                let floor = o.repository.profile()?.wall_high_water;
                ensure!(
                    floor >= before_wall && floor <= now()?,
                    "real receiver wall, not peer date"
                );
            }
            println!("A2HINT actual_owner_state_offsets=15 receiver_role={receiver} receiver_wall_unmodified=true");
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn a2hint_receipt_queue_boundary_and_duplicate_budget() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let slots = slots(&fixtures).await?;
    let wall = now()?;
    let raw = announcement(&fixtures, slots, wall + 3600, [2, 2], 42000)?;
    // The second wrapper must wrap identical locator bytes; construct through
    // the production sealing codec below from the first hint's parsed locators.
    let actor = &fixtures.artifacts[0];
    let hint = wire::lan_discovery::open_authenticated_hint(
        &raw,
        &wire::lan_discovery::HintScope {
            sender: &actor.credentials[0],
            recipient: &actor.credentials[1],
            slots: slots.map(|s| wire::lan_discovery::HintContext {
                context: s.context,
                consent_ref: s.consent_ref,
            }),
            sender_ipv4: PEER,
        },
        &kilogram_crypto::DeviceEncryptionIdentity::from_secret_bytes([32; 32]),
    )?;
    let duplicate = seal(
        &Scope {
            sender: &actor.credentials[0],
            recipient: &actor.credentials[1],
            slots: slots.map(|mut s| {
                s.consent_interval = (wall, wall + 7200);
                s
            }),
            sender_ipv4: PEER,
            now: wall + 3601,
        },
        &actor.devices[0],
        wall + 3601,
        wall + 3646,
        hint.locators().each_ref().map(|l| l.raw()),
    )?;
    let id = fixtures.endpoints[1].id();
    fixtures.handles[1]
        .call(move |o| {
            configure(o, id)?;
            let observed = o.capture_lan_discovery(NetworkGeneration2(1))?;
            let at = Instant::now();
            let generation = o.repository.generation();
            let expired =
                ReceivedHint2::capture(raw, NetworkGeneration2(1), at - Duration::from_secs(45));
            ensure!(
                o.commit_received_hint(
                    &observed,
                    &expired,
                    now,
                    || at,
                    Owner2::commit_peer_locator
                )
                .is_err()
            );
            ensure!(o.repository.generation() == generation);
            let receipt = ReceivedHint2::capture(raw, NetworkGeneration2(1), at);
            o.commit_received_hint(&observed, &receipt, now, || at, Owner2::commit_peer_locator)?;
            let pair = o.listener_pair((PEER, 42000).into())?.context("pair")?;
            let first = pair.candidate_deadline(at)?;
            let generation = o.repository.generation();
            let receipt = ReceivedHint2::capture(
                duplicate,
                NetworkGeneration2(1),
                at + Duration::from_secs(10),
            );
            o.commit_received_hint(
                &observed,
                &receipt,
                now,
                || at + Duration::from_secs(10),
                Owner2::commit_peer_locator,
            )?;
            ensure!(
                pair.candidate_deadline(at)? == first && o.repository.generation() == generation
            );
            o.expire_lan_candidates(first);
            ensure!(o.hint_attempt_deadline(1, &pair, first).is_err());
            let premature =
                ReceivedHint2::capture(raw, NetworkGeneration2(1), first + Duration::from_secs(1));
            o.commit_received_hint(
                &observed,
                &premature,
                now,
                || first + Duration::from_secs(1),
                Owner2::commit_peer_locator,
            )?;
            ensure!(
                o.hint_attempt_deadline(1, &pair, first + Duration::from_secs(1))
                    .is_err()
            );
            let eligible = first + Duration::from_secs(15);
            let replay = ReceivedHint2::capture(raw, NetworkGeneration2(1), eligible);
            o.commit_received_hint(
                &observed,
                &replay,
                now,
                || eligible,
                Owner2::commit_peer_locator,
            )?;
            ensure!(
                o.hint_attempt_deadline(1, &pair, eligible)? == eligible + Duration::from_secs(45)
            );
            ensure!(o.leases.iter().all(Option::is_none));
            println!(
                "A2HINT receipt_boundary=true duplicate_fixed=true later_replay_untrusted=true"
            );
            Ok(())
        })
        .await
}

#[tokio::test]
async fn a2hint_invalid_later_slot_partial_commit_restart_and_policy() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let slots = slots(&fixtures).await?;
    let wall = now()?;
    let original = announcement(&fixtures, slots, wall + 86400, [2, 2], 42000)?;
    let lower = announcement(&fixtures, slots, wall - 86400, [1, 1], 42000)?;
    let conflict = announcement(&fixtures, slots, wall + 86400, [2, 2], 42001)?;
    let higher = announcement(&fixtures, slots, wall - 86400, [3, 3], 42000)?;
    let mut bad_slots = slots;
    let last = usize::from(slots[0].context < slots[1].context);
    bad_slots[last].consent_ref[0] ^= 1;
    let invalid = announcement(&fixtures, bad_slots, wall + 86400, [999, 999], 42000)?;
    let id = fixtures.endpoints[1].id();
    fixtures.handles[1].call(move|o| {
        configure(o,id)?;let obs=o.capture_lan_discovery(NetworkGeneration2(1))?;
        let receive=|raw|ReceivedHint2::capture(raw,NetworkGeneration2(1),Instant::now());
        o.accept_received_lan_hint(&obs,&receive(original))?;
        let before=o.repository.generation();let pair=o.listener_pair((PEER,42000).into())?.context("pair")?;
        for raw in [invalid,lower,conflict] {
            ensure!(o.accept_received_lan_hint(&obs,&receive(raw)).is_err());
            ensure!(o.repository.generation()==before);
            ensure!(Arc::ptr_eq(&pair,&o.listener_pair((PEER,42000).into())?.context("same pair")?));
        }
        let mut writes=Vec::new();
        ensure!(o.commit_received_hint(&obs,&receive(higher),now,Instant::now,|o,s,c,raw,w,purpose|{
            writes.push(c);if c==1 {anyhow::bail!("synthetic second-slot interruption");}
            o.commit_peer_locator(s,c,raw,w,purpose)
        }).is_err());
        ensure!(writes==[0,1] && o.repository.generation()==before+1);
        ensure!(o.snapshot(0)?.spec.peer_locator_revision==3 && o.snapshot(1)?.spec.peer_locator_revision==2);
        ensure!(o.listener_pair((PEER,42000).into()).is_err());
        o.recover()?;ensure!(o.listener_pair((PEER,42000).into()).is_err());
        ensure!(o.leases.iter().all(Option::is_none));
        ensure!(o.accept_received_lan_hint(&obs,&receive(original)).is_err());
        o.discovery_quota=discovery::DiscoveryQuota2::default();
        o.accept_received_lan_hint(&obs,&receive(higher))?;
        ensure!(o.snapshot(0)?.spec.peer_locator_revision==3 && o.snapshot(1)?.spec.peer_locator_revision==3);
        let generation=o.repository.generation();let (_,current)=o.require(0,false)?;
        ensure!(o.commit_received_hint(&obs,&receive(higher),||Ok(current.consent.interval().1),Instant::now,Owner2::commit_peer_locator).is_err());
        let floor=o.repository.profile()?.wall_high_water;
        ensure!(o.commit_received_hint(&obs,&receive(higher),||Ok(floor-1),Instant::now,Owner2::commit_peer_locator).is_err());
        ensure!(o.repository.generation()==generation);
        // The legacy/manual state operation still enforces the peer calendar.
        let s=o.snapshot(0)?;let raw=s.spec.peer_locator.clone().context("peer")?;
        ensure!(o.commit_peer_locator(&s,0,raw,now()?,owner::Purpose2::Local).is_err());
        println!("A2HINT invalid_later_no_displacement=true partial_closed=true durable_replay=true real_permission_and_floor=true manual_time_strict=true");Ok(())
    }).await
}

async fn tls_pair(fixtures: &Actors2) -> Result<(Connection, Connection)> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let (a, b) = tokio::try_join!(
            async {
                Ok::<_, anyhow::Error>(
                    fixtures.endpoints[0]
                        .connect(fixtures.endpoints[1].addr(), wire::ALPN)
                        .await?,
                )
            },
            async {
                Ok::<_, anyhow::Error>(
                    fixtures.endpoints[1]
                        .accept()
                        .await
                        .context("loopback incoming")?
                        .await?,
                )
            },
        )?;
        kilogram_transport_iroh::await_route_policy(
            &b,
            RoutePolicy::DirectOnly,
            Duration::from_secs(5),
        )
        .await?;
        Ok((a, b))
    })
    .await?
}
fn remote_hello(
    o: &mut Owner2,
    c: u8,
    binding: Id,
    credential: &[u8],
) -> Result<wire::ParsedFrame2> {
    let (s, current) = o.require(c, false)?;
    let own = wire::ParsedRuntimeLocator2::parse(s.spec.own_locator.as_deref().context("own")?)?;
    let peer = wire::ParsedRuntimeLocator2::parse(s.spec.peer_locator.as_deref().context("peer")?)?;
    let actor = support::Actors::new(s.head.context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let peer_role = 1 - s.head.role;
    let body = wire::FrameBody2::parse(
        wire::FrameKind::Hello,
        &wire::encode(
            &a([
                b(s.head.context),
                u(peer_role.into()),
                b(peer.fields().endpoint),
                b(own.fields().endpoint),
                b(shared_admin::random::<32>()?),
                a([u(1)]),
                a([u(2)]),
                b(credential),
                b(&s.spec.membership),
                b(current.consent.reference()),
                b(peer.id()),
                wire::Origin::Fresh.value(),
            ]),
            96 * 1024,
        )?,
    )?;
    Ok(wire::ParsedFrame2::sign(
        body,
        binding,
        [17; 16],
        &actor.devices[usize::from(peer_role)],
    )?)
}
fn complete_receiver_admission(
    o: &mut Owner2,
    c: u8,
    connection: Connection,
    binding: Id,
    pair: &Arc<discovery::LocatorPair2>,
    credential: &[u8],
) -> Result<Link2> {
    let (generation, local_hello, _) = o.begin_admission(
        c,
        connection,
        binding,
        Instant::now() + Duration::from_secs(30),
        None,
        Some(pair),
    )?;
    let remote_hello = remote_hello(o, c, binding, credential)?;
    let s = o.snapshot(c)?;
    let actor = support::Actors::new(s.head.context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let peer = usize::from(1 - s.head.role);
    let t = if s.head.role == 0 {
        wire::initial_transcript2(binding, local_hello.raw(), remote_hello.raw())?
    } else {
        wire::initial_transcript2(binding, remote_hello.raw(), local_hello.raw())?
    };
    o.accept_hello(c, generation, remote_hello)?;
    if s.head.role == 0 {
        o.admission_proof(c, generation)?;
    }
    let proof = wire::ParsedFrame2::sign(
        wire::FrameBody2::proof(t, s.head.context, s.spec.certs)?,
        binding,
        [18; 16],
        &actor.devices[peer],
    )?;
    o.accept_proof(c, generation, proof)?;
    if s.head.role == 1 {
        o.admission_proof(c, generation)?;
    }
    o.admitted_frame(c, generation, None)?;
    let body = wire::FrameBody2::parse(
        wire::FrameKind::Admitted,
        &wire::encode(&a([b(t), b(s.head.context), u(1), u(2)]), 256)?,
    )?;
    let admitted = wire::ParsedFrame2::sign(body, binding, [19; 16], &actor.devices[peer])?;
    o.accept_admitted(c, generation, admitted)
}

#[tokio::test]
async fn a2hint_actual_admission_healthy_context_and_expired_second() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let slots = slots(&fixtures).await?;
    let wall = now()?;
    let raw = announcement(&fixtures, slots, wall - 86400, [2, 2], 42000)?;
    let (_dialed, accepted) = tls_pair(&fixtures).await?;
    let endpoint = fixtures.endpoints[1].clone();
    let binding = transport::binding(&accepted, *endpoint.id().as_bytes(), false)?;
    let credential = fixtures.artifacts[0].credentials[0].raw().to_vec();
    fixtures.handles[1].call(move|o| {
        configure(o,endpoint.id())?;o.lan_endpoint=Some(endpoint);
        let obs=o.capture_lan_discovery(NetworkGeneration2(1))?;let at=Instant::now();
        o.accept_received_lan_hint(&obs,&ReceivedHint2::capture(raw,NetworkGeneration2(1),at))?;
        let pair=o.listener_pair((PEER,42000).into())?.context("pair")?;
        let before_deadline=pair.candidate_deadline(at)?;
        let peer=wire::ParsedRuntimeLocator2::parse(o.snapshot(0)?.spec.peer_locator.as_deref().context("peer")?)?;
        ensure!(peer.fields().expires<now()?,"actual peer-calendar expiry ignored only as Auto hint");
        let link=complete_receiver_admission(o,0,accepted.clone(),binding,&pair,&credential)?;
        let lease_end=o.leases[0].as_ref().context("healthy lease")?.deadline;
        let generation=o.repository.generation();
        o.accept_received_lan_hint(&obs,&ReceivedHint2::capture(raw,NetworkGeneration2(1),Instant::now()))?;
        ensure!(o.repository.generation()==generation && o.stats.admissions==1);
        ensure!(pair.candidate_deadline(at)?==before_deadline && o.leases[0].as_ref().context("same lease")?.deadline==lease_end);
        o.require(0,true)?;ensure!(o.require(1,true).is_err());
        o.expire_lan_candidates(before_deadline);
        ensure!(link.valid.load(Ordering::Acquire) && o.leases[0].as_ref().context("retained")?.deadline==lease_end);
        o.require(0,true)?;
        ensure!(o.hint_attempt_deadline(1,&pair,Instant::now()).is_err());
        ensure!(o.begin_admission(1,accepted,binding,Instant::now()+Duration::from_secs(30),None,Some(&pair)).is_err());
        ensure!(o.leases[1].is_none());
        // Scheduled untrusted retry cannot renew the healthy context's lease.
        let retry=before_deadline+Duration::from_secs(15);
        o.commit_received_hint(&obs,&ReceivedHint2::capture(raw,NetworkGeneration2(1),retry),now,||retry,Owner2::commit_peer_locator)?;
        ensure!(o.hint_attempt_deadline(1,&pair,retry).is_ok());
        ensure!(o.leases[0].as_ref().context("unextended lease")?.deadline==lease_end && o.leases[1].is_none());
        println!("A2HINT actual_tls_bound_hello_proof_admitted=true healthy_hint_expiry_independent=true expired_second_denied=true");Ok(())
    }).await
}

#[tokio::test]
async fn a2hint_verified_hello_root_before_admission_and_final_revocation() -> Result<()> {
    for revoke_source in [true, false] {
        let fixtures = Actors2::new(false).await?;
        let slots = slots(&fixtures).await?;
        let wall = now()?;
        let raw = announcement(&fixtures, slots, wall + 86400, [2, 2], 42000)?;
        let (_dialed, accepted) = tls_pair(&fixtures).await?;
        let endpoint = fixtures.endpoints[1].clone();
        let binding = transport::binding(&accepted, *endpoint.id().as_bytes(), false)?;
        let credential = fixtures.artifacts[0]
            .renewal(0, 3)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .raw()
            .to_vec();
        fixtures.handles[1].call(move|o| {
            configure(o,endpoint.id())?;o.lan_endpoint=Some(endpoint);
            let obs=o.capture_lan_discovery(NetworkGeneration2(1))?;
            o.accept_received_lan_hint(&obs,&ReceivedHint2::capture(raw,NetworkGeneration2(1),Instant::now()))?;
            let pair=o.listener_pair((PEER,42000).into())?.context("pair")?;
            let hello=remote_hello(o,0,binding,&credential)?;
            let before=o.repository.generation();
            let permit=o.admission_permit(0,Some(&pair),Instant::now()+Duration::from_secs(10),&accepted)?.context("permit")?;
            ensure!(permit.check_context(1).is_err());
            o.test_revoke_lan_at_commit=revoke_source;
            let outcome=o.observe_hello_authority(0,&accepted,binding,&hello,Some(&pair),Instant::now()+Duration::from_secs(10));
            if revoke_source {
                ensure!(outcome.is_err() && o.repository.generation()==before);
                ensure!(o.repository.profile()?.peer.context("peer pin")?.revision==2);
            } else {
                outcome?;ensure!(o.repository.generation()==before+1);
                ensure!(o.repository.profile()?.peer.context("peer pin")?.revision==3);
                ensure!(o.leases.iter().all(Option::is_none));
            }
            println!("A2HINT verified_hello_root_source_revoked={revoke_source} full_admission_not_required=true");Ok(())
        }).await?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct SignedTest<T> {
    content: T,
    signature: Vec<u8>,
}
#[derive(serde::Serialize)]
struct RevocationTest {
    version: u8,
    account_id: kilogram_identity::AccountId,
    device_id: kilogram_identity::DeviceId,
    authority_sequence: u64,
}
#[derive(serde::Serialize)]
struct AuthorityTest {
    version: u8,
    account_id: kilogram_identity::AccountId,
    revision: u64,
    revocations: Vec<kilogram_identity::DeviceRevocation>,
}
fn sign_test<T: serde::Serialize>(
    content: T,
    root: &kilogram_identity::DeviceIdentity,
    domain: &[u8],
) -> Result<Vec<u8>> {
    let mut message = domain.to_vec();
    message.extend(postcard::to_allocvec(&content)?);
    let signature = root.sign(&message).to_vec();
    Ok(postcard::to_allocvec(&SignedTest { content, signature })?)
}
fn revoked_credential(actor: &support::Actors, revision: u64) -> Result<Vec<u8>> {
    let cert = actor.credentials[0].certificate();
    let rev = sign_test(
        RevocationTest {
            version: 1,
            account_id: cert.account_id(),
            device_id: cert.device_id(),
            authority_sequence: 1,
        },
        &actor.roots[0],
        b"kilogram:device-revocation-signature:v1\0",
    )?;
    let authority = sign_test(
        AuthorityTest {
            version: 1,
            account_id: cert.account_id(),
            revision,
            revocations: vec![kilogram_identity::DeviceRevocation::decode_and_verify(
                &rev,
            )?],
        },
        &actor.roots[0],
        b"kilogram:account-authority-snapshot-signature:v1\0",
    )?;
    // Root-valid credentials intentionally need not authorize their now-revoked signer.
    Ok(wire::encode(
        &a([u(1), b(cert.encode()?), b(authority)]),
        96 * 1024,
    )?)
}
#[tokio::test]
async fn a2hint_fresh_hello_learns_signer_revocation_and_durable_root() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let slots = slots(&fixtures).await?;
    let raw = announcement(&fixtures, slots, now()? + 3600, [2, 2], 42000)?;
    let (_dialed, accepted) = tls_pair(&fixtures).await?;
    let endpoint = fixtures.endpoints[1].clone();
    let binding = transport::binding(&accepted, *endpoint.id().as_bytes(), false)?;
    let credential = revoked_credential(&fixtures.artifacts[0], 3)?;
    let conflict = fixtures.artifacts[0]
        .renewal(0, 3)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .authority()
        .encode()?;
    let removal = fixtures.artifacts[0]
        .renewal(0, 4)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .authority()
        .encode()?;
    let old = fixtures.artifacts[0].credentials[0].authority().encode()?;
    fixtures.handles[1].call(move|o| {
        configure(o,endpoint.id())?;o.lan_endpoint=Some(endpoint);
        let obs=o.capture_lan_discovery(NetworkGeneration2(1))?;
        o.accept_received_lan_hint(&obs,&ReceivedHint2::capture(raw,NetworkGeneration2(1),Instant::now()))?;
        let pair=o.listener_pair((PEER,42000).into())?.context("pair")?;
        let hello=remote_hello(o,0,binding,&credential)?;
        // A recorded Hello under another TLS binding cannot import even valid Root data.
        ensure!(o.observe_hello_authority(0,&accepted,[42;32],&hello,Some(&pair),Instant::now()+Duration::from_secs(10)).is_err());
        ensure!(o.repository.profile()?.peer.context("peer pin")?.revision==2);
        o.observe_hello_authority(0,&accepted,binding,&hello,Some(&pair),Instant::now()+Duration::from_secs(10))?;
        ensure!(o.repository.profile()?.peer.context("peer pin")?.revision==3);
        ensure!(o.require(0,false).is_err() && o.require(1,false).is_err());
        ensure!(o.leases.iter().all(Option::is_none));
        ensure!(o.begin_admission(0,accepted,binding,Instant::now()+Duration::from_secs(10),None,Some(&pair)).is_err());
        o.recover()?;
        let before=o.repository.generation();
        for authority in [old,conflict,removal] {
            ensure!(o.import_authority(authority).is_err());
            ensure!(o.repository.profile()?.peer.context("persisted peer")?.revision==3);
        }
        ensure!(o.repository.generation()==before);
        println!("A2HINT signer_revocation_learned_pre_admission=true revoked_denied=true wrong_binding_denied=true durable_root_rollback_conflict_removal_denied=true");Ok(())
    }).await
}

#[tokio::test]
async fn a2hint_permit_rechecked_after_actual_await() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let slots = slots(&fixtures).await?;
    let raw = announcement(&fixtures, slots, now()? - 3600, [2, 2], 42000)?;
    let (_dialed, accepted) = tls_pair(&fixtures).await?;
    let endpoint = fixtures.endpoints[1].clone();
    let permit = fixtures.handles[1]
        .call(move |o| {
            configure(o, endpoint.id())?;
            o.lan_endpoint = Some(endpoint);
            let obs = o.capture_lan_discovery(NetworkGeneration2(1))?;
            o.accept_received_lan_hint(
                &obs,
                &ReceivedHint2::capture(raw, NetworkGeneration2(1), Instant::now()),
            )?;
            let pair = o.listener_pair((PEER, 42000).into())?.context("pair")?;
            o.admission_permit(
                1,
                Some(&pair),
                Instant::now() + Duration::from_secs(2),
                &accepted,
            )?
            .context("permit")
        })
        .await?;
    let deadline = permit.deadline();
    let waited = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = waited.clone();
    ensure!(
        admission_stream(Some(&permit), async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
            observed.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .is_err()
    );
    ensure!(waited.load(Ordering::Acquire));
    println!("A2HINT permit_rechecked_after_await=true");
    Ok(())
}
