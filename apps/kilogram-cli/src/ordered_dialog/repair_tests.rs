//! Narrow review regressions. Real administrative/native/vault APIs; the
//! component lease below is explicit and is not TLS/process acceptance.
use super::{
    admin,
    engine::{self, Engine, Lease},
    handshake,
    ratchet::Guard,
    runtime,
    state::{self, PrivateState},
};
use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountRootState, DeviceIdentity};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Frame, Id, Value, a, b, opt, u};
use kilogram_ratchet::ordered::Packet;
use kilogram_state::{OrderedMutation, OrderedVault, StateDirectoryLock};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

struct Pair {
    _tmp: tempfile::TempDir,
    accounts: [PathBuf; 2],
    a: Engine,
    b: Engine,
    c: Id,
    binding: Id,
}
fn reopen(engine: Engine) -> Result<Engine> {
    let root = engine.store.root().to_path_buf();
    drop(engine);
    Engine::new(OrderedVault::open(StateDirectoryLock::acquire(root)?)?)
}
impl Pair {
    fn new() -> Result<Self> {
        let tmp = tempfile::tempdir()?;
        let mut stores = Vec::new();
        for i in 0..2 {
            let owner = tmp.path().join(format!("owner{i}"));
            AccountRootState::create(&owner)?;
            let root = tmp.path().join(format!("device{i}"));
            admin::enroll(StateDirectoryLock::acquire(&root)?, &owner, None)?;
            let store = OrderedVault::open(StateDirectoryLock::acquire(root)?)?;
            stores.push((admin::credentials(&store)?.cert.device_id(), owner, store));
        }
        stores.sort_by_key(|(id, _, _)| *id);
        let (_, owner_b, mut sb) = stores.pop().context("B")?;
        let (_, owner_a, mut sa) = stores.pop().context("A")?;
        let ca = admin::credentials(&sa)?;
        let cb = admin::credentials(&sb)?;
        let now = engine::now()?;
        let (c, invitation) =
            admin::create_context(&sa, &owner_a, &cb.raw, cb.cert.account_id(), now)?;
        let (_, complete) = admin::import_context(
            &mut sb,
            Some(&owner_b),
            &invitation,
            ca.cert.account_id(),
            now,
        )?;
        admin::import_context(&mut sa, None, &complete, cb.cert.account_id(), now)?;
        Ok(Self {
            _tmp: tmp,
            accounts: [owner_a, owner_b],
            a: Engine::new(sa)?,
            b: Engine::new(sb)?,
            c,
            binding: [0; 32],
        })
    }
    fn renew(&mut self, serial: u64) -> Result<()> {
        let spec = self.a.spec(self.c)?;
        let value = wire::decode(&spec.consent, 4096)?;
        let mut body = value.fields(3)?[0].clone();
        if let Value::A(f) = &mut body {
            f[7] = u(engine::now()? + 30 * 86400 + serial);
        }
        let root_a = AccountRootState::load(&self.accounts[0])?;
        let invitation = wire::encode(
            &a([
                u(1),
                b(self.c),
                b(&spec.peers[0].raw),
                b(&spec.peers[1].raw),
                b(&spec.membership),
                body.clone(),
                b(root_a.sign_ordered_context_consent(&wire::encode(&body, 4096)?)),
                opt(None),
            ]),
            96 * 1024,
        )?;
        let ca = spec.peers[0].cert.account_id();
        let cb = spec.peers[1].cert.account_id();
        let (_, complete) = admin::import_context(
            &mut self.b.store,
            Some(&self.accounts[1]),
            &invitation,
            ca,
            engine::now()?,
        )?;
        admin::import_context(&mut self.a.store, None, &complete, cb, engine::now()?)?;
        Ok(())
    }
    fn pair_with_refs(&self, refs: [Id; 2], binding: Id) -> Result<Vec<u8>> {
        let mut hellos = Vec::new();
        for (role, engine) in [&self.a, &self.b].into_iter().enumerate() {
            let spec = engine.spec(self.c)?;
            let body = a([
                b(self.c),
                u(role as u64),
                b([role as u8; 32]),
                b([1 - role as u8; 32]),
                b(admin::random::<32>()?),
                a([u(1)]),
                a([u(1)]),
                b(admin::credentials(&engine.store)?.raw),
                b(&spec.membership),
                b(refs[role]),
                b([7; 32]),
            ]);
            hellos.push(
                Frame::signed(
                    1,
                    binding,
                    admin::random::<16>()?,
                    body,
                    engine.device.identity(),
                )?
                .raw()?,
            );
        }
        let t = handshake::transcript(binding, &hellos[0], &hellos[1])?;
        let certs = self.a.spec(self.c)?.cert_refs()?;
        let proofs = [&self.a, &self.b]
            .into_iter()
            .map(|e| {
                Frame::signed(
                    2,
                    binding,
                    [1; 16],
                    a([b(t), b(self.c), u(1), u(1), b(certs[0]), b(certs[1])]),
                    e.device.identity(),
                )
                .and_then(|f| f.raw())
            })
            .collect::<wire::Result<Vec<_>>>()?;
        handshake::initial_pair(
            t,
            hellos[0].clone(),
            hellos[1].clone(),
            proofs[0].clone(),
            proofs[1].clone(),
        )
    }
    fn admit(&mut self) -> Result<Vec<u8>> {
        self.binding = admin::random()?;
        for e in [&mut self.a, &mut self.b] {
            e.policy(self.c)?;
            e.leases.insert(
                self.c,
                Lease {
                    binding: self.binding,
                    generation: 1,
                    deadline: engine::now()? + 300,
                },
            );
        }
        let consent = admin::required(&self.a.store, &admin::consent_path(self.c))?;
        self.pair_with_refs([wire::hash(handshake::CONSENT, &consent); 2], self.binding)
    }
    fn activate(&mut self, pair: &[u8]) -> Result<()> {
        let offer = self.a.ensure_offer(self.c, pair, self.binding)?;
        let grant = self.b.grant(self.c, &offer, pair, self.binding)?;
        let init = self.a.init(self.c, &grant, self.binding)?;
        let confirm = self.b.confirm(self.c, &init, self.binding)?;
        let ready_a = self.a.receive_confirm(self.c, &confirm, self.binding)?;
        let ready_b = self.b.ready(self.c, &ready_a, self.binding)?;
        self.a.ready(self.c, &ready_b, self.binding)?;
        Ok(())
    }
}

#[test]
fn review_r1_renewal_before_offer_and_historical_selection() -> Result<()> {
    for selected_at in [0, 1, 2, 3] {
        let mut p = Pair::new()?;
        if selected_at == 0 {
            p.renew(1)?;
        }
        let pair = p.admit()?;
        let offer = p.a.ensure_offer(p.c, &pair, p.binding)?;
        let mut grant = None;
        if selected_at >= 2 {
            grant = Some(p.b.grant(p.c, &offer, &pair, p.binding)?);
        }
        if selected_at == 3 {
            p.activate(&pair)?;
        }
        let offer_hash = wire::hash(b"", &offer);
        p.renew(2)?;
        p.renew(3)?;
        p.a = reopen(p.a)?;
        p.b = reopen(p.b)?;
        let current = p.admit()?;
        assert_eq!(
            wire::hash(b"", &p.a.ensure_offer(p.c, &current, p.binding)?),
            offer_hash
        );
        let replay = p.b.grant(p.c, &offer, &pair, p.binding)?;
        if let Some(grant) = grant {
            assert_eq!(grant, replay);
        }
        p.activate(&pair)?;
        assert!(p.a.active(p.c)? && p.b.active(p.c)?);
        println!("review_r1 selected_at={selected_at} renewal_restart=PASS");
    }
    // A fresh selection requires the exact current consent. Matching malicious
    // peer signatures alone cannot change that; historical pair must also agree.
    let mut p = Pair::new()?;
    let current = p.admit()?;
    let bad = p.pair_with_refs([[99; 32]; 2], p.binding)?;
    assert!(p.a.ensure_offer(p.c, &bad, p.binding).is_err());
    let offer = p.a.ensure_offer(p.c, &current, p.binding)?;
    let mismatch = p.pair_with_refs([[99; 32], [98; 32]], p.binding)?;
    // Bind the signed Offer to this very pair: rejection must come from the
    // inconsistent consents, not merely a different transcript hash.
    let bad_t = wire::decode(&mismatch, 192 * 1024)?.fields(6)?[1].clone();
    let mut bad_offer = wire::control(&offer, 1, p.a.device.identity().device_id())?;
    if let Value::A(fields) = &mut bad_offer {
        fields[3] = bad_t;
    }
    let bad_offer = wire::signed_control(1, bad_offer, p.a.device.identity())?;
    assert!(p.b.grant(p.c, &bad_offer, &mismatch, p.binding).is_err());
    let mut unsigned = current.clone();
    let last = unsigned.last_mut().context("pair")?;
    *last ^= 1;
    assert!(p.b.grant(p.c, &offer, &unsigned, p.binding).is_err());
    let consent_before = admin::required(&p.b.store, &admin::consent_path(p.c))?;
    let consent = wire::decode(&consent_before, 4096)?;
    let fields = consent.fields(3)?;
    let mut bad_root_signature = fields[1].bytes()?.to_vec();
    bad_root_signature[0] ^= 1;
    let spec = p.a.spec(p.c)?;
    let wrong_consent = wire::encode(
        &a([
            u(1),
            b(p.c),
            b(&spec.peers[0].raw),
            b(&spec.peers[1].raw),
            b(&spec.membership),
            fields[0].clone(),
            b(bad_root_signature),
            opt(Some(fields[2].clone())),
        ]),
        96 * 1024,
    )?;
    assert!(
        admin::import_context(
            &mut p.b.store,
            None,
            &wrong_consent,
            spec.peers[0].cert.account_id(),
            engine::now()?
        )
        .is_err()
    );
    assert_eq!(
        admin::required(&p.b.store, &admin::consent_path(p.c))?,
        consent_before
    );
    Ok(())
}

#[test]
fn review_r2_fence_only_refused_and_durable_claim_resumes_same_keys() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let owners = [tmp.path().join("X"), tmp.path().join("Y")];
    for owner in &owners {
        AccountRootState::create(owner)?;
    }
    // Obtain the production fence from a normal fresh enrollment, not a guess.
    let normal = tmp.path().join("normal");
    admin::enroll(StateDirectoryLock::acquire(&normal)?, &owners[0], None)?;
    let actual = std::fs::read(normal.join("ratchet/account.pickle"))?;
    let mut failures = Vec::new();
    for (i, owner) in owners.iter().enumerate() {
        for with_db in [false, true] {
            let fence_root = tmp.path().join(format!("fence-only-{i}-{with_db}"));
            std::fs::create_dir_all(fence_root.join("ratchet"))?;
            std::fs::write(fence_root.join("ratchet/account.pickle"), &actual)?;
            if with_db {
                drop(kilogram_state::EncryptedStateVault::open_or_create(
                    &fence_root,
                )?);
            }
            let result = admin::enroll(StateDirectoryLock::acquire(&fence_root)?, owner, None);
            failures.push(
                result
                    .as_ref()
                    .err()
                    .is_some_and(|e| format!("{e:#}").contains("CreationConflict")),
            );
            assert_eq!(
                std::fs::read(fence_root.join("ratchet/account.pickle"))?,
                actual
            );
        }
    }
    assert_eq!(
        failures,
        [true, true, true, true],
        "ambiguous fence-only adoption"
    );
    let root = tmp.path().join("durable-claim");
    let owner = AccountRootState::load(&owners[0])?;
    let sign = DeviceIdentity::generate()?;
    let enc = kilogram_crypto::DeviceEncryptionIdentity::generate()?;
    let claim = wire::encode(
        &a([
            u(1),
            u(4),
            b(admin::random::<32>()?),
            b(owner.account_id().as_bytes()),
            b(sign.device_id().as_bytes()),
        ]),
        1024,
    )?;
    let store = OrderedVault::create(
        StateDirectoryLock::acquire(&root)?,
        vec![
            OrderedMutation::put(admin::CLAIM, claim.clone()),
            OrderedMutation::put("device-secret.key", sign.secret_bytes().to_vec()),
            OrderedMutation::put("device-encryption-secret.key", enc.secret_bytes().to_vec()),
        ],
    )?;
    drop(store);
    assert!(
        admin::enroll(StateDirectoryLock::acquire(&root)?, &owners[1], None)
            .err()
            .context("different owner accepted durable claim")?
            .to_string()
            .contains("different owner")
    );
    admin::enroll(StateDirectoryLock::acquire(&root)?, &owners[0], None)?;
    let store = OrderedVault::open(StateDirectoryLock::acquire(&root)?)?;
    assert_eq!(admin::required(&store, admin::CLAIM)?, claim);
    assert_eq!(
        admin::device(&store)?.identity().device_id(),
        sign.device_id()
    );
    assert_eq!(
        admin::device(&store)?.encryption().public_key(),
        enc.public_key()
    );
    drop(store);
    let store = OrderedVault::open(StateDirectoryLock::acquire(&normal)?)?;
    assert!(admin::credentials(&store).is_ok());
    println!(
        "review_r2 fence_only_same_different=CreationConflict durable_resume_same_keys=PASS normal_reopen=PASS"
    );
    Ok(())
}

fn native_from(engine: &Engine, head: &state::Head, bytes: &[u8]) -> Result<PrivateState> {
    PrivateState::open(head, engine.nonce, engine.device.encryption(), bytes)
}
fn malicious_genesis(
    engine: &Engine,
    private: &mut PrivateState,
    template: &Event,
    payload: &Value,
    wrong_parents: bool,
) -> Result<Event> {
    let mut core = template.core.clone();
    core.sid = private.session.as_ref().context("session")?.sid();
    if wrong_parents {
        core.parents = vec![[9; 32]];
    } else {
        core.request[0] ^= 1;
    }
    let plain = Event::inner(&core, payload.clone())?;
    let packet = private
        .session
        .as_mut()
        .context("session")?
        .encrypt(&plain)?;
    let header = packet.header()?;
    let mut event = Event {
        core,
        native_kind: packet.kind.into(),
        key: header.key,
        index: header.index,
        ciphertext: packet.bytes,
        signature: Vec::new(),
    };
    event.sign(engine.device.identity())?;
    Ok(event)
}
#[test]
fn review_r4_self_consistent_genesis_request_and_user_parents_refused() -> Result<()> {
    let mut p = Pair::new()?;
    let pair = p.admit()?;
    let offer = p.a.ensure_offer(p.c, &pair, p.binding)?;
    let grant = p.b.grant(p.c, &offer, &pair, p.binding)?;
    let ha = p.a.head(p.c)?;
    let hb = p.b.head(p.c)?;
    let ba = admin::required(&p.a.store, &state::native_path(p.c))?;
    let bb = admin::required(&p.b.store, &state::native_path(p.c))?;
    let init = p.a.init(p.c, &grant, p.binding)?;
    let a0 = Event::parse(&init, p.a.device.identity().device_id())?;
    let gb = wire::control(&grant, 2, p.b.device.identity().device_id())?;
    let t = wire::control(&offer, 1, p.a.device.identity().device_id())?.fields(8)?[3].clone();
    for parents in [false, true] {
        let mut private = native_from(&p.a, &ha, &ba)?;
        private.session = Some(
            private
                .account
                .outbound(gb.fields(8)?[4].fixed()?, gb.fields(8)?[5].fixed()?)?,
        );
        let keys = private.session.as_ref().context("session")?.keys();
        let bad = malicious_genesis(
            &p.a,
            &mut private,
            &a0,
            &a([
                b(ha.offer.context("Offer")?),
                b(hb.grant.context("Grant")?),
                t.clone(),
                a(keys.iter().map(b)),
            ]),
            parents,
        )?;
        let error =
            p.b.confirm(p.c, &bad.raw()?, p.binding)
                .err()
                .context("bad Init accepted")?;
        ensure!(
            error.to_string().contains("genesis"),
            "wrong rejection: {error}"
        );
        assert!(
            admin::required(&p.b.store, &state::native_path(p.c))? == bb,
            "Init changed opaque native blob"
        );
    }
    let confirm = p.b.confirm(p.c, &init, p.binding)?;
    let b0 = Event::parse(&confirm, p.b.device.identity().device_id())?;
    let before = admin::required(&p.a.store, &state::native_path(p.c))?;
    for parents in [false, true] {
        let mut private = native_from(&p.b, &hb, &bb)?;
        let packet = Packet {
            kind: 0,
            bytes: a0.ciphertext.clone(),
        };
        let keys = packet.header()?.prekey_keys.context("prekey")?;
        private.session = Some(private.account.inbound(keys[0], &packet)?.0);
        private.guard = Some(Guard::new(1));
        private
            .guard
            .as_mut()
            .context("guard")?
            .commit_receive(&a0, a0.id()?)?;
        let bad = malicious_genesis(
            &p.b,
            &mut private,
            &b0,
            &a([
                b(ha.offer.context("Offer")?),
                b(hb.grant.context("Grant")?),
                b(a0.core.sid.as_bytes()),
                b(a0.id()?),
            ]),
            parents,
        )?;
        let error =
            p.a.receive_confirm(p.c, &bad.raw()?, p.binding)
                .err()
                .context("bad Confirm accepted")?;
        ensure!(
            error.to_string().contains("genesis"),
            "wrong rejection: {error}"
        );
        assert!(
            admin::required(&p.a.store, &state::native_path(p.c))? == before,
            "Confirm changed opaque native blob"
        );
    }
    p.a.receive_confirm(p.c, &confirm, p.binding)?;
    println!(
        "review_r4 real_native_inner_outer wrong_request_and_parents_init_confirm=refused original_after_reject=PASS"
    );
    Ok(())
}

async fn ipc(
    engine: &runtime::Shared,
    control: &runtime::RuntimeControl,
    c: Id,
    request: Value,
) -> Result<Value> {
    let raw =
        runtime::ipc_request(engine, control, &[c], &wire::encode(&request, 32 * 1024)?).await?;
    Ok(wire::decode(&raw, wire::MAX_WIRE)?)
}
#[tokio::test]
async fn review_r3_engine_ipc_lifetime_capacity_reopen_receive_and_repair() -> Result<()> {
    let mut p = Pair::new()?;
    let pair = p.admit()?;
    p.activate(&pair)?;
    let shared = Arc::new(Mutex::new(p.a));
    let control = runtime::RuntimeControl::default();
    control
        .statuses
        .lock()
        .await
        .insert(p.c, "Active".to_owned());
    // Preserve an actual accepted sealed queue record to reconstruct the old
    // version's oversubscribed snapshot later. No native/credential fixture.
    let legacy = {
        let mut e = shared.lock().await;
        e.queue(p.c, [6; 16], "previously accepted queue")?;
        let path = e
            .store
            .paths(&format!("{}queue/", engine::prefix(p.c)))
            .pop()
            .context("queue")?;
        let raw = admin::required(&e.store, &path)?;
        let generation = e.store.generation();
        e.store
            .commit(generation, vec![OrderedMutation::remove(&path)])?;
        (path, raw)
    };
    let mut reserved_capacity_denied = false;
    for n in 1..512_u64 {
        let mut request = [0; 16];
        request[..8].copy_from_slice(&n.to_be_bytes());
        ipc(
            &shared,
            &control,
            p.c,
            a([u(2), b(p.c), b(request), b(format!("text{n}").as_bytes())]),
        )
        .await?;
        if n == 510 {
            let mut last = [0; 16];
            last[..8].copy_from_slice(&511_u64.to_be_bytes());
            ipc(
                &shared,
                &control,
                p.c,
                a([u(2), b(p.c), b(last), b(b"text511")]),
            )
            .await?;
            let outcome = ipc(
                &shared,
                &control,
                p.c,
                a([u(2), b(p.c), b([8; 16]), b(b"beyond reserved slots")]),
            )
            .await;
            reserved_capacity_denied = outcome
                .err()
                .is_some_and(|e| format!("{e:#}").contains("UpgradeRequired"));
        }
        let mut a_engine = shared.lock().await;
        // A bounded component lease is refreshed explicitly, not presented as TLS.
        a_engine.leases.get_mut(&p.c).context("lease")?.deadline = engine::now()? + 300;
        p.b.leases.get_mut(&p.c).context("lease")?.deadline = engine::now()? + 300;
        let event = a_engine
            .materialize_one(p.c, p.binding)?
            .context("materialized Text")?;
        let frame = a_engine.deliver_frame(p.c, &[event], p.binding, [1; 16])?;
        let batch = p.b.accept_data_frame(p.c, &frame)?;
        ensure!(
            batch.error.is_none() && batch.readable == 1,
            "real native readable Text"
        );
        let (acks, ordinals) = p.b.ack_intents(p.c, p.binding)?.context("ACK intent")?;
        a_engine.accept_ack_frame(p.c, &acks)?;
        p.b.mark_ack_intents(p.c, p.binding, &ordinals, 2)?;
        if n % 128 == 0 {
            println!("review_r3 real_native_texts={n}");
        }
    }
    let new_request = [8; 16];
    let denied = ipc(
        &shared,
        &control,
        p.c,
        a([u(2), b(p.c), b(new_request), b(b"over lifetime")]),
    )
    .await;
    let explicit = denied
        .as_ref()
        .err()
        .is_some_and(|e| format!("{e:#}").contains("UpgradeRequired"));
    {
        let mut e = shared.lock().await;
        let generation = e.store.generation();
        e.store.commit(
            generation,
            vec![OrderedMutation::put(&legacy.0, legacy.1.clone())],
        )?;
    }
    assert_eq!(
        ipc(
            &shared,
            &control,
            p.c,
            a([u(2), b(p.c), b([6; 16]), b(b"previously accepted queue")])
        )
        .await?
        .fields(2)?[1]
            .uint()?,
        0
    );
    let status = ipc(&shared, &control, p.c, a([u(1)])).await?;
    let row = &status.fields(3)?[2].list()?[0];
    let state = row.fields(9)?[7].text()?.to_owned();
    let mut first = [0; 16];
    first[..8].copy_from_slice(&1_u64.to_be_bytes());
    assert_eq!(
        ipc(
            &shared,
            &control,
            p.c,
            a([u(2), b(p.c), b(first), b(b"text1")])
        )
        .await?
        .fields(2)?[1]
            .uint()?,
        0
    );
    let engine = Arc::try_unwrap(shared)
        .map_err(|_| anyhow::anyhow!("shared owner"))?
        .into_inner();
    let mut a_engine = reopen(engine)?;
    a_engine.leases.insert(
        p.c,
        Lease {
            binding: p.binding,
            generation: 2,
            deadline: engine::now()? + 300,
        },
    );
    assert_eq!(
        a_engine
            .native(&a_engine.head(p.c)?)?
            .guard
            .context("guard")?
            .own
            .next,
        512
    );
    assert!(a_engine.materialize_one(p.c, p.binding)?.is_none());
    assert!(
        admin::required(&a_engine.store, &legacy.0)? == legacy.1,
        "old queue changed at exhaustion"
    );
    // Opposite lane remains usable, as do history, prefix serving, old ACK query
    // and byte-exact retry of already committed ciphertext after reopen.
    p.b.queue(p.c, [7; 16], "peer after exhaustion")?;
    let opposite =
        p.b.materialize_one(p.c, p.binding)?
            .context("peer materialization")?;
    let frame = p.b.deliver_frame(p.c, &[opposite], p.binding, [2; 16])?;
    ensure!(
        a_engine.accept_data_frame(p.c, &frame)?.readable == 1,
        "opposite receive"
    );
    let original = a_engine.event_at(p.c, 0, 1)?.context("original")?;
    let bytes = original.raw()?;
    assert_eq!(
        a_engine
            .deliver_frame(p.c, &[original], p.binding, [3; 16])?
            .kind,
        10
    );
    assert_eq!(
        a_engine.event_at(p.c, 0, 1)?.context("retry")?.raw()?,
        bytes
    );
    assert_eq!(a_engine.history(p.c, 0, 8)?.fields(4)?[1].uint()?, 512);
    // Explicit repair of old ciphertext still works at own.next=512 even though
    // the honest receiver already possesses it (simulate a retrying request).
    let head = a_engine.head(p.c)?;
    let genesis = a_engine.event_at(p.c, 0, 0)?.context("genesis")?.id()?;
    let body = a([
        b(p.c),
        b(head.d.context("D")?),
        b(head.e.context("E")?),
        u(0),
        u(1),
        u(2),
        opt(Some(b(genesis))),
        u(1),
        u(wire::MAX_WIRE as u64),
        u(64),
        u(64 * 64 * 1024),
        u(64 * 1024),
    ]);
    let frame = Frame::signed(11, p.binding, [4; 16], body, p.b.device.identity())?;
    assert_eq!(a_engine.prefix_response(p.c, &frame)?.kind, 12);
    let (acks, _) = a_engine
        .ack_intents(p.c, p.binding)?
        .context("opposite ACK")?;
    assert_eq!(p.b.accept_ack_frame(p.c, &acks)?, 1);
    let shared = Arc::new(Mutex::new(a_engine));
    let reopened = ipc(&shared, &control, p.c, a([u(1)])).await?;
    let reopened_state = reopened.fields(3)?[2].list()?[0].fields(9)?[7]
        .text()?
        .to_owned();
    println!(
        "review_r3 real_text511 own_next512 new_request_upgrade={explicit} status={state} reopened={reopened_state} opposite_readable=PASS history_exact_retry=PASS"
    );
    assert!(explicit, "new request beyond lifetime accepted");
    assert!(
        reserved_capacity_denied,
        "queue overcommitted lifetime slots"
    );
    assert_eq!(state, "UpgradeRequired");
    assert_eq!(reopened_state, "UpgradeRequired");
    Ok(())
}
