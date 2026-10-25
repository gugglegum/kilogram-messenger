//! Root artifacts use public test keys. Every Account/Session/packet and lease below
//! comes from the production owner and actual TLS admission, never from support::fresh.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as wire;
use zeroize::Zeroizing;
#[allow(dead_code)]
mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs"
    ));
}
struct Actors2 {
    handles: [Handle2; 2],
    endpoints: [Endpoint; 2],
    threads: Vec<std::thread::JoinHandle<()>>,
    connections: Vec<Connection>,
    artifacts: [support::Actors; 2],
    _path: std::path::PathBuf,
}
impl Drop for Actors2 {
    fn drop(&mut self) {
        for connection in &self.connections {
            connection.close(0u32.into(), b"component completed");
        }
        for h in &self.handles {
            h.stop.store(true, Ordering::Release);
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
impl Actors2 {
    async fn new(short_locator: bool) -> Result<Self> {
        let parent =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/a3-s4/actor");
        std::fs::create_dir_all(&parent)?;
        let path = tempfile::Builder::new()
            .prefix("actual-")
            .tempdir_in(parent)?
            .keep();
        let contexts = [
            wire::hash(b"S4 actor C1", path.to_string_lossy().as_bytes()),
            wire::hash(b"S4 actor C2", path.to_string_lossy().as_bytes()),
        ];
        let artifacts = [
            support::Actors::new(contexts[0]).map_err(|e| anyhow::anyhow!(e.to_string()))?,
            support::Actors::new(contexts[1]).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ];
        let wall = now()?;
        let consents = artifacts
            .iter()
            .map(|actor| -> Result<Vec<u8>> {
                let body = a([
                    u(2),
                    b(actor.c),
                    b(actor.credentials[0].cert_ref()),
                    b(actor.credentials[1].cert_ref()),
                    u(1),
                    u(2),
                    wire::Origin::Fresh.value(),
                    u(1),
                    u(wall.saturating_sub(60)),
                    u(wall + 3600),
                ]);
                let message = wire::signing_bytes(wire::CONSENT, &body, 4096)?;
                Ok(wire::encode(
                    &a([
                        body,
                        b(actor.roots[0].sign(&message)),
                        b(actor.roots[1].sign(&message)),
                    ]),
                    4096,
                )?)
            })
            .collect::<Result<Vec<_>>>()?;
        let route = kilogram_transport_iroh::ordered2::RouteConfig2 {
            policy: kilogram_transport_iroh::RoutePolicy::DirectOnly,
            bind: Some("127.0.0.1:0".parse()?),
            relay: None,
        };
        let endpoints = [
            transport::bind(&route).await?,
            transport::bind(&route).await?,
        ];
        let mut owners = Vec::new();
        let mut locators = [Vec::new(), Vec::new()];
        for role in 0..2 {
            let credential = &artifacts[0].credentials[role];
            let state = path.join(format!("state{role}"));
            let mut repository = db::RuntimeRepository::create(
                kilogram_state::StateDirectoryLock::acquire(&state)?,
                db::CreationClaim {
                    account: *credential.certificate().account_id().as_bytes(),
                    claim: [41 + role as u8; 32],
                    signing_secret: Zeroizing::new(artifacts[0].devices[role].secret_bytes()),
                    encryption_secret: Zeroizing::new([31 + role as u8; 32]),
                    contexts,
                },
                db::RuntimeEnrollment {
                    device: *credential.certificate().device_id().as_bytes(),
                    certificate: credential.certificate().encode()?,
                    authority: credential.authority().encode()?,
                    authority_revision: credential.authority().revision(),
                },
                db::Config::default(),
            )?;
            for (index, actor) in artifacts.iter().enumerate() {
                admin::install(
                    &mut repository,
                    &wire::encode(
                        &a([
                            u(2),
                            b(actor.c),
                            b(actor.credentials[0].raw()),
                            b(actor.credentials[1].raw()),
                            b(&actor.membership),
                            b(&consents[index]),
                        ]),
                        96 * 1024,
                    )?,
                )?;
            }
            let mut owner = Owner2::new(repository, route.clone(), vec![0, 1])?;
            for c in 0..2 {
                let mut raw =
                    owner.rotate_locator(c, [role as u8 + 1; 32], &endpoints[role].addr())?;
                if short_locator && c == 0 {
                    let s = owner.snapshot(c)?;
                    let mut fields = wire::ParsedRuntimeLocator2::parse(&raw)?.fields().clone();
                    let wall = now()?;
                    fields.revision += 1;
                    fields.not_before = wall;
                    fields.expires = wall + 8;
                    raw = wire::ParsedRuntimeLocator2::sign(fields, &owner.device)?
                        .raw()
                        .to_vec();
                    owner.commit(
                        &s,
                        db::RuntimeOperation::Locator {
                            context: c,
                            fence: s.fence(wall),
                            own: true,
                            raw: raw.clone(),
                        },
                        owner::Purpose2::Local,
                        (0, 0),
                    )?;
                }
                locators[role].push(raw);
            }
            owners.push(owner);
        }
        for role in 0..2 {
            for c in 0..2 {
                owners[role].import_locator(c, locators[1 - role][usize::from(c)].clone())?;
            }
        }
        let mut iter = owners.into_iter();
        let left = iter.next().context("A owner")?;
        let right = iter.next().context("B owner")?;
        let (ha, ja) = owner_thread(left)?;
        let (hb, jb) = owner_thread(right)?;
        Ok(Self {
            handles: [ha, hb],
            endpoints,
            threads: vec![ja, jb],
            connections: vec![],
            artifacts,
            _path: path,
        })
    }
    async fn connect(&mut self, c: u8) -> Result<[Link2; 2]> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let ((ca, la), (cb, lb)) = tokio::try_join!(
            admit_a(&self.handles[0], &self.endpoints[0], c, deadline),
            async {
                let cb = self.endpoints[1]
                    .accept()
                    .await
                    .context("actual incoming")?
                    .await?;
                let lb = admit_b(&self.handles[1], &self.endpoints[1], &cb, deadline).await?;
                Ok::<_, anyhow::Error>((cb, lb))
            }
        )?;
        self.connections.push(ca);
        self.connections.push(cb);
        Ok([la, lb])
    }
    async fn sources(&self, node: usize, c: u8, n: u64) -> Result<()> {
        let start = Instant::now();
        loop {
            if self.handles[node]
                .call(move |o| Ok(o.repository.counts(c)?.sources == n))
                .await?
            {
                return Ok(());
            }
            ensure!(
                start.elapsed() < Duration::from_secs(30),
                "actual owner source convergence"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    async fn state(&self, node: usize, c: u8) -> Result<(Option<Id>, u64, u64, u64)> {
        self.handles[node]
            .call(move |o| {
                let s = o.snapshot(c)?;
                Ok((
                    s.head.native_hash,
                    o.repository.counts(c)?.sources,
                    o.stats.packet_attempts,
                    s.head.native_generation,
                ))
            })
            .await
    }
    async fn fresh(&self, c: u8, negative_q: bool) -> Result<()> {
        let (offer, pair) = self.handles[0].call(move |o| o.offer(c)).await?;
        let grant = self.handles[1]
            .call(move |o| o.grant(c, offer, pair))
            .await?;
        let init = self.handles[0].call(move |o| o.init(c, grant)).await?;
        if negative_q {
            let event = wire::ParsedEvent2::parse(&init)?;
            let mut core = event.core().clone();
            core.q[0] ^= 1;
            let invalid = wire::sign_event(
                core,
                &event.packet(),
                &self.artifacts[usize::from(c)].devices[0],
            )?
            .raw()
            .to_vec();
            let before = self.state(1, c).await?;
            ensure!(
                self.handles[1]
                    .call(move |o| o.receive_genesis(c, invalid))
                    .await
                    .is_err(),
                "wrong-Q genesis"
            );
            ensure!(
                before == self.state(1, c).await?,
                "wrong-Q genesis entered Native"
            );
        }
        let confirm = self.handles[1]
            .call(move |o| o.receive_genesis(c, init))
            .await?;
        self.handles[0]
            .call(move |o| o.receive_genesis(c, confirm))
            .await?;
        let ar = self.handles[0].call(move |o| o.ready(c)).await?;
        let br = self.handles[1].call(move |o| o.ready(c)).await?;
        if negative_q {
            let parsed = wire::ParsedControl2::parse(&br)?;
            let mut body = parsed.body().fields(9)?.to_vec();
            let mut q = body[8].fixed::<32>()?;
            q[0] ^= 1;
            body[8] = b(q);
            let invalid = wire::ParsedControl2::sign(
                3,
                &wire::encode(&a(body), 1024)?,
                &self.artifacts[usize::from(c)].devices[1],
            )?
            .raw()
            .to_vec();
            let before = self.state(0, c).await?;
            ensure!(
                self.handles[0]
                    .call(move |o| o.accept_ready(c, invalid))
                    .await
                    .is_err(),
                "wrong-Q Ready"
            );
            ensure!(
                before == self.state(0, c).await?,
                "wrong-Q Ready advanced activation"
            );
        }
        self.handles[0].call(move |o| o.accept_ready(c, br)).await?;
        self.handles[1].call(move |o| o.accept_ready(c, ar)).await?;
        Ok(())
    }
    async fn packet(&self, c: u8, n: u64) -> Result<Vec<u8>> {
        self.handles[0]
            .call(move |o| o.queue(c, [n as u8 + 1; 16], format!("actor {c} packet {n}")))
            .await?;
        self.sources(0, c, n + 2).await?;
        self.handles[0]
            .call(move |o| {
                Ok(o.repository
                    .source(c, 0, n)?
                    .context("actual committed source")?
                    .raw)
            })
            .await
    }
    fn auth(&self, c: u8) -> std::collections::BTreeMap<Id, Vec<u8>> {
        let credential = &self.artifacts[usize::from(c)].credentials[0];
        std::collections::BTreeMap::from([(credential.reference(), credential.raw().to_vec())])
    }
    async fn ingest(&self, c: u8, raw: Vec<u8>) -> Result<Id> {
        let auth = self.auth(c);
        self.handles[1]
            .call(move |o| o.ingest(c, raw, &auth, false, None))
            .await
    }
}
#[tokio::test]
async fn a3_s4_f01_actual_expired_lease_and_connection_generation() -> Result<()> {
    let mut pair = Actors2::new(false).await?;
    let old = pair.connect(0).await?;
    let hello = pair.handles[0]
        .call(|o| {
            let lease = o.leases[0].as_ref().context("actual lease")?;
            let p = wire::decode(&lease.pair, 192 * 1024)?;
            Ok(wire::ParsedFrame2::parse(p.fields(6)?[2].bytes()?)?)
        })
        .await?;
    let link = old[1].clone();
    let frame = hello.clone();
    pair.handles[1]
        .call(move |o| o.verify_frame(&link, &frame))
        .await?;
    for connection in &pair.connections {
        connection.close(0u32.into(), b"actual generation change");
    }
    let current = pair.connect(0).await?;
    ensure!(
        old[0].binding != current[0].binding && old[0].generation != current[0].generation,
        "actual reconnect binding/generation"
    );
    let link = old[1].clone();
    let frame = hello;
    ensure!(
        pair.handles[1]
            .call(move |o| o.verify_frame(&link, &frame))
            .await
            .is_err(),
        "old connection permit accepted"
    );
    ensure!(
        pair.state(1, 0).await?.2 == 0,
        "connection generation probe invoked native"
    );
    drop(pair);
    let mut pair = Actors2::new(true).await?;
    let links = pair.connect(0).await?;
    let link = links[0].clone();
    let dispatch = pair.handles[0]
        .call(move |o| {
            let lease = o.leases[0].as_ref().context("actual lease")?;
            let p = wire::decode(&lease.pair, 192 * 1024)?;
            let hello = wire::ParsedFrame2::parse(p.fields(6)?[2].bytes()?)?;
            o.frame(
                &link,
                wire::FrameKind::Hello,
                hello.body().value().clone(),
                [3; 16],
                None,
            )
        })
        .await?;
    dispatch.check(&pair.connections[0])?;
    tokio::time::sleep(Duration::from_secs(9)).await;
    pair.handles[0]
        .call(|o| o.require(0, false).map(|_| ()))
        .await?;
    ensure!(
        dispatch.check(&pair.connections[0]).is_err() && pair.state(0, 0).await?.2 == 0,
        "expired actual lease permitted dispatch/native"
    );
    println!(
        "S4_F01_actual_cases=2 old_connection_generation=true actual_short_signed_locator_lease_expired=true current_Root2_still_valid=true native_calls=0"
    );
    Ok(())
}
#[tokio::test]
async fn a3_s4_f02_f09_wrong_q_signed_fork_and_invalid_after_prefix() -> Result<()> {
    let mut pair = Actors2::new(false).await?;
    pair.connect(0).await?;
    pair.connect(1).await?;
    pair.fresh(0, true).await?;
    pair.fresh(1, false).await?;
    let raw = pair.packet(0, 1).await?;
    let event = wire::ParsedEvent2::parse(&raw)?;
    let mut core = event.core().clone();
    core.q[0] ^= 1;
    let invalid = wire::sign_event(core, &event.packet(), &pair.artifacts[0].devices[0])?
        .raw()
        .to_vec();
    let before = pair.state(1, 0).await?;
    ensure!(
        pair.ingest(0, invalid).await.is_err() && before == pair.state(1, 0).await?,
        "wrong-Q Text entered pending/native"
    );
    pair.ingest(0, raw.clone()).await?;
    pair.sources(1, 0, 3).await?;
    let receipt = pair.handles[1]
        .call(|o| {
            o.repository
                .receipt(0, 0, 1)?
                .context("actual readable receipt")
        })
        .await?;
    let mut fields = wire::ParsedReceipt2::parse(&receipt)?.fields().clone();
    fields.q[0] ^= 1;
    let wrong = wire::ParsedReceipt2::sign(fields, &pair.artifacts[0].devices[1])?
        .raw()
        .to_vec();
    let before = pair.state(0, 0).await?;
    ensure!(
        pair.handles[0]
            .call(move |o| o.acknowledge(0, wrong, &std::collections::BTreeMap::new()))
            .await
            .is_err()
            && before == pair.state(0, 0).await?,
        "wrong-Q receipt removed debt/native"
    );
    pair.handles[0]
        .call(move |o| o.acknowledge(0, receipt, &std::collections::BTreeMap::new()))
        .await?;
    let mut core = event.core().clone();
    core.sequence = core.sequence.checked_add(10).context("negative sequence")?;
    let fork = wire::sign_event(core, &event.packet(), &pair.artifacts[0].devices[0])?
        .raw()
        .to_vec();
    let before = pair.state(1, 0).await?;
    ensure!(
        pair.ingest(0, fork).await.is_err() && before == pair.state(1, 0).await?,
        "signed fork advanced accepted source/native"
    );
    ensure!(
        pair.handles[1]
            .call(|o| Ok(o.snapshot(0)?.head.blocked.is_some()))
            .await?,
        "fork evidence not durable"
    );
    let raw = pair.packet(1, 1).await?;
    pair.ingest(1, raw).await?;
    pair.sources(1, 1, 3).await?;
    let raw = pair.packet(1, 2).await?;
    let event = wire::ParsedEvent2::parse(&raw)?;
    let mut packet = event.packet();
    let last = packet.bytes.last_mut().context("actual ciphertext")?;
    *last ^= 1;
    let bad = wire::sign_event(event.core().clone(), &packet, &pair.artifacts[1].devices[0])?
        .raw()
        .to_vec();
    let before = pair.state(1, 1).await?;
    pair.ingest(1, bad).await?;
    let start = Instant::now();
    loop {
        if pair.handles[1]
            .call(|o| Ok(o.snapshot(1)?.head.blocked.is_some()))
            .await?
        {
            break;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(20),
            "invalid ciphertext quarantine"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let after = pair.state(1, 1).await?;
    ensure!(
        after.0 == before.0 && after.1 == before.1 && after.3 == before.3 && after.2 > before.2,
        "invalid after prefix changed accepted Native/source"
    );
    println!(
        "S4_F02_F09_PASS wrong_Q_boundaries=4 signed_fork_pre_native=true C2_after_C1_block=true invalid_after_durable_prefix=true invalid_receipt_not_created=true"
    );
    Ok(())
}
#[tokio::test]
async fn a3_s4_f02_old_offer_current_renewal_historical_support() -> Result<()> {
    let mut pair = Actors2::new(false).await?;
    pair.connect(0).await?;
    let (offer, initial_pair) = pair.handles[0].call(|o| o.offer(0)).await?;
    let before = pair.state(0, 0).await?;
    let old = pair.handles[0]
        .call(|o| Ok(o.snapshot(0)?.spec.consent))
        .await?;
    let parsed = wire::decode(&old, 4096)?;
    let mut body = parsed.fields(3)?[0].fields(10)?.to_vec();
    body[9] = u(body[9].uint()? + 60);
    let body = a(body);
    let message = wire::signing_bytes(wire::CONSENT, &body, 4096)?;
    let renewed = wire::encode(
        &a([
            body,
            b(pair.artifacts[0].roots[0].sign(&message)),
            b(pair.artifacts[0].roots[1].sign(&message)),
        ]),
        4096,
    )?;
    let expected = wire::ParsedConsent2::parse(&old)?.reference();
    for h in &pair.handles {
        let raw = renewed.clone();
        h.call(move |o| o.import_consent(0, raw, expected, false))
            .await?;
    }
    pair.connect(0).await?;
    let raw = offer.clone();
    let historical = initial_pair.clone();
    let error = pair.handles[1]
        .call(move |o| o.grant(0, raw, historical))
        .await
        .err()
        .context("old Offer without historical support must fail")?;
    ensure!(
        format!("{error:#}").contains("NeedHistoricalConsent") && pair.state(1, 0).await?.2 == 0,
        "missing support crossed constructor"
    );
    pair.handles[1]
        .call(move |o| o.import_consent(0, old, expected, true))
        .await?;
    let retried = pair.handles[0].call(|o| o.offer(0)).await?;
    ensure!(
        retried == (offer.clone(), initial_pair.clone()) && before == pair.state(0, 0).await?,
        "renewal recreated OfferedA Native/control"
    );
    let grant = pair.handles[1]
        .call(move |o| o.grant(0, offer, initial_pair))
        .await?;
    let init = pair.handles[0].call(move |o| o.init(0, grant)).await?;
    let confirm = pair.handles[1]
        .call(move |o| o.receive_genesis(0, init))
        .await?;
    pair.handles[0]
        .call(move |o| o.receive_genesis(0, confirm))
        .await?;
    let ar = pair.handles[0].call(|o| o.ready(0)).await?;
    let br = pair.handles[1].call(|o| o.ready(0)).await?;
    pair.handles[0].call(move |o| o.accept_ready(0, br)).await?;
    pair.handles[1].call(move |o| o.accept_ready(0, ar)).await?;
    ensure!(
        pair.handles[0]
            .call(|o| Ok(o.snapshot(0)?.head.active))
            .await?,
        "supported old Offer did not activate"
    );
    println!(
        "S4_F02_HISTORICAL_PASS old_offer_exact=true current_Hello_refs_renewed=true missing_support_pre_constructor=true explicit_support_exact_continuation=true"
    );
    Ok(())
}
#[tokio::test]
async fn a3_s4_f08_existing_origin_and_f12_actual_endpoint_mismatch() -> Result<()> {
    let mut pair = Actors2::new(false).await?;
    let actor = &pair.artifacts[0];
    let wall = now()?;
    let body = a([
        u(2),
        b(actor.c),
        b(actor.credentials[0].cert_ref()),
        b(actor.credentials[1].cert_ref()),
        u(1),
        u(2),
        wire::Origin::Existing {
            d: [11; 32],
            e: [12; 32],
            sid: "a".repeat(43),
        }
        .value(),
        u(1),
        u(wall - 1),
        u(wall + 600),
    ]);
    let message = wire::signing_bytes(wire::CONSENT, &body, 4096)?;
    let consent = wire::encode(
        &a([
            body,
            b(actor.roots[0].sign(&message)),
            b(actor.roots[1].sign(&message)),
        ]),
        4096,
    )?;
    wire::ParsedConsent2::parse(&consent)?.verify(&actor.credentials)?;
    let artifact = wire::encode(
        &a([
            u(2),
            b(actor.c),
            b(actor.credentials[0].raw()),
            b(actor.credentials[1].raw()),
            b(&actor.membership),
            b(consent),
        ]),
        96 * 1024,
    )?;
    let before = pair.state(0, 0).await?;
    let error = pair.handles[0]
        .call(move |o| admin::install(&mut o.repository, &artifact))
        .await
        .err()
        .context("existing origin refused")?;
    ensure!(
        format!("{error:#}").contains("UnsupportedExistingRuntime")
            && before == pair.state(0, 0).await?,
        "existing origin crossed protected boundary"
    );
    let route = transport::RouteConfig2 {
        policy: kilogram_transport_iroh::RoutePolicy::DirectOnly,
        bind: Some("127.0.0.1:0".parse()?),
        relay: None,
    };
    let foreign = transport::bind(&route).await?;
    let (dialed, accepted) = tokio::try_join!(
        async {
            Ok::<_, anyhow::Error>(
                foreign
                    .connect(pair.endpoints[1].addr(), wire::ALPN)
                    .await?,
            )
        },
        async {
            Ok::<_, anyhow::Error>(
                pair.endpoints[1]
                    .accept()
                    .await
                    .context("actual foreign incoming")?
                    .await?,
            )
        }
    )?;
    let error = admit_b(
        &pair.handles[1],
        &pair.endpoints[1],
        &accepted,
        Instant::now() + Duration::from_secs(10),
    )
    .await
    .err()
    .context("actual endpoint must mismatch signed locator")?;
    ensure!(
        format!("{error:#}").contains("actual TLS endpoint is absent")
            && pair.state(1, 0).await?.2 == 0,
        "actual endpoint mismatch crossed admission/native"
    );
    dialed.close(0u32.into(), b"done");
    accepted.close(0u32.into(), b"done");
    foreign.close().await;
    pair.connections.clear();
    println!(
        "S4_F08_F12_PASS valid_bilateral_existing_origin_refused_before_mutation=true actual_TLS_vs_current_signed_locator_endpoint_mismatch_pre_Hello=true native_calls=0"
    );
    Ok(())
}

#[tokio::test]
async fn a3_s4_f12_explicit_route_table_url_only_and_no_defaults() -> Result<()> {
    use kilogram_transport_iroh::RoutePolicy;
    use transport::RouteConfig2;
    use wire::ParsedRuntimeLocator2;
    let signer = kilogram_identity::DeviceIdentity::generate()?;
    let fields = wire::RuntimeLocatorFields2 {
        c: [1; 32],
        cert: [2; 32],
        peer_cert: [3; 32],
        endpoint: *signer.device_id().as_bytes(),
        ips: vec!["127.0.0.1:21001".parse()?],
        relay: None,
        revision: 1,
        not_before: 1,
        expires: u64::MAX,
    };
    let direct = ParsedRuntimeLocator2::sign(fields.clone(), &signer)?;
    let mut url_fields = fields.clone();
    url_fields.ips.clear();
    url_fields.relay = Some("http://127.0.0.1:21002/".into());
    let url_only = ParsedRuntimeLocator2::sign(url_fields.clone(), &signer)?;
    let direct_config = RouteConfig2 {
        policy: RoutePolicy::DirectOnly,
        bind: Some("127.0.0.1:0".parse()?),
        relay: None,
    };
    let relay = RouteConfig2 {
        policy: RoutePolicy::RelayOnly,
        bind: None,
        relay: url_fields.relay.clone(),
    };
    ensure!(
        direct_config.remote(&direct)?.ip_addrs().next().is_some()
            && direct_config.remote(&url_only).is_err(),
        "direct-only route table"
    );
    let remote = relay.remote(&url_only)?;
    ensure!(
        remote.ip_addrs().next().is_none() && remote.relay_urls().count() == 1,
        "URL-only route"
    );
    ensure!(
        relay.remote(&direct).is_err(),
        "RelayOnly missing peer URL accepted"
    );
    let mut wrong = url_fields.clone();
    wrong.relay = Some("http://127.0.0.1:21003/".into());
    ensure!(
        relay
            .remote(&ParsedRuntimeLocator2::sign(wrong, &signer)?)
            .is_err(),
        "different permitted relay accepted"
    );
    let mut empty = fields;
    empty.ips.clear();
    ensure!(
        ParsedRuntimeLocator2::sign(empty, &signer).is_err(),
        "empty/empty locator accepted"
    );
    ensure!(
        RouteConfig2 {
            relay: None,
            ..relay
        }
        .effective()
        .is_err(),
        "RelayOnly None accepted"
    );
    let auto = RouteConfig2 {
        policy: RoutePolicy::Auto,
        ..direct_config
    };
    ensure!(
        auto.effective()? == RoutePolicy::DirectOnly && auto.remote(&url_only).is_err(),
        "Auto None used implicit relay"
    );
    let endpoint = transport::bind(&auto).await?;
    ensure!(
        endpoint.addr().relay_urls().next().is_none()
            && endpoint.addr().ip_addrs().next().is_some(),
        "actual Minimal transport snapshot"
    );
    endpoint.close().await;
    println!(
        "S4_F12_ROUTE_PASS direct_ip=true URL_only_empty_IP=true empty_empty_reject=true relay_None_missing_different_reject=true Auto_None_direct=true builder_Minimal_clear_all_explicit_transports_lookup0=true"
    );
    Ok(())
}
