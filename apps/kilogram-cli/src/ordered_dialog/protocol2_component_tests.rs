//! Real library components only: no listener, actor activation, lease or migration command.
use kilogram_crypto::SealedMessage;
use kilogram_identity::DeviceEncryptionIdentity;
use kilogram_protocol::ordered_dialog_v2 as wire;
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession},
    ordered_v2::{self as guard, Guard2, SeenObservation},
};
use kilogram_state::indexed_dialog as db;
use std::{collections::BTreeMap, time::Instant};
use wire::{Id, a, b, u};
use zeroize::Zeroizing;
#[allow(dead_code)]
mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs"
    ));
}
use support::TestResult;
#[path = "protocol2_failure_tests.rs"]
mod failure_tests;
#[path = "protocol2_legacy_tests.rs"]
mod legacy_tests;
const INFO: &[u8] = b"Kilogram OD2 component native checkpoint";
fn component_budget(start: Instant) -> TestResult<()> {
    if start.elapsed().as_secs_f64() > 600.0 {
        return Err("S3 component exceeded 600 seconds".into());
    }
    Ok(())
}
struct Native {
    account: OrderedAccount,
    session: Option<OrderedSession>,
    guard: Guard2,
    generation: u64,
    key: Zeroizing<Id>,
}
struct TrialRow {
    text: String,
    auth: Vec<db::HistoryAuth>,
    final_ref: Id,
    receipt: Option<Vec<u8>>,
}
impl Native {
    fn fork(&self) -> Self {
        Self {
            account: self.account.fork(),
            session: self.session.as_ref().map(OrderedSession::fork),
            guard: self.guard.clone(),
            generation: self.generation,
            key: Zeroizing::new(*self.key),
        }
    }
    fn aad(
        scope: &db::HistoryScope,
        q: Id,
        role: u8,
        generation: u64,
        view: Id,
        cert: Id,
    ) -> TestResult<Vec<u8>> {
        Ok(postcard::to_allocvec(&(
            2u8,
            view,
            cert,
            scope.context,
            scope.dialog,
            scope.epoch,
            &scope.session_id,
            role,
            q,
            generation,
        ))?)
    }
    fn seal(
        &self,
        scope: &db::HistoryScope,
        q: Id,
        role: u8,
        crypto: &DeviceEncryptionIdentity,
        view: Id,
        cert: Id,
    ) -> TestResult<Vec<u8>> {
        let plain = Zeroizing::new(postcard::to_allocvec(&(
            *self.key,
            self.account.snapshot(&self.key),
            self.session.as_ref().ok_or("session")?.snapshot(&self.key),
            self.guard.encode()?,
        ))?);
        let aad = Self::aad(scope, q, role, self.generation, view, cert)?;
        let sealed = crypto.public_key().seal(&plain, INFO, &aad)?;
        let raw = postcard::to_allocvec(&(
            2u8,
            self.generation,
            sealed.encapsulated_key,
            sealed.ciphertext,
        ))?;
        guard::check_checkpoint_size(&raw)?;
        Ok(raw)
    }
    fn open(
        raw: &[u8],
        scope: &db::HistoryScope,
        q: Id,
        role: u8,
        crypto: &DeviceEncryptionIdentity,
        view: Id,
        cert: Id,
    ) -> TestResult<Self> {
        guard::check_checkpoint_size(raw)?;
        let (version, generation, encapsulated_key, ciphertext): (u8, u64, Vec<u8>, Vec<u8>) =
            postcard::from_bytes(raw)?;
        if version != 2 {
            return Err("checkpoint version".into());
        }
        let plain = Zeroizing::new(crypto.open(
            &SealedMessage {
                encapsulated_key,
                ciphertext,
            },
            INFO,
            &Self::aad(scope, q, role, generation, view, cert)?,
        )?);
        let (key, account, session, g): (Id, Vec<u8>, Vec<u8>, Vec<u8>) =
            postcard::from_bytes(&plain)?;
        let key = Zeroizing::new(key);
        let session = OrderedSession::restore(&session, &key)?;
        if session.sid() != scope.session_id {
            return Err("checkpoint SID".into());
        }
        Ok(Self {
            account: OrderedAccount::restore(&account, &key)?,
            session: Some(session),
            guard: Guard2::restore(&g, role)?,
            generation,
            key,
        })
    }
}
struct Node {
    view: Id,
    cert: Id,
    store: Option<db::IndexedDialogVault>,
    dir: Option<tempfile::TempDir>,
    owner: db::ExpectedOwner,
    config: db::Config,
    native: Native,
    scope: db::HistoryScope,
    q: Id,
    role: u8,
    crypto: DeviceEncryptionIdentity,
    commits: u64,
    native_calls: u64,
    max_blob: usize,
    max_guard: usize,
}
impl Node {
    fn create(
        scope: db::HistoryScope,
        fresh: &support::Fresh,
        actors: &support::Actors,
        role: u8,
        native: Native,
    ) -> TestResult<Self> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/a3-s3");
        std::fs::create_dir_all(&root)?;
        let dir = tempfile::Builder::new()
            .prefix(&format!("component-{role}-"))
            .tempdir_in(root)?;
        let claim_id = wire::hash(
            b"component claim\0",
            &[scope.context.as_slice(), &[role]].concat(),
        );
        let account = *actors.credentials[usize::from(role)]
            .certificate()
            .account_id()
            .as_bytes();
        let owner = db::ExpectedOwner {
            account,
            claim: claim_id,
        };
        let config = db::Config {
            physical_quota: 1024 * 1024 * 1024,
            ..Default::default()
        };
        let mut store = db::IndexedDialogVault::create(
            kilogram_state::StateDirectoryLock::acquire(dir.path())?,
            db::CreationClaim {
                account,
                claim: claim_id,
                signing_secret: Zeroizing::new(actors.devices[usize::from(role)].secret_bytes()),
                encryption_secret: Zeroizing::new([31 + role; 32]),
                contexts: [scope.context, [0xb6; 32]],
            },
            config,
        )?;
        store.initialize_history_protocol2(
            0,
            store.generation(),
            [1; 32],
            db::HistoryInit {
                scope: scope.clone(),
                own_role: role,
            },
            db::HistoryProtocol2Binding {
                proof: fresh.raw.clone(),
                author_slots: [0, 1],
            },
        )?;
        let view = store.history_checkpoint(0, store.generation())?.view;
        Ok(Self {
            view,
            cert: actors.credentials[usize::from(role)].cert_ref(),
            store: Some(store),
            dir: Some(dir),
            owner,
            config,
            native,
            scope,
            q: fresh.proof.q(),
            role,
            crypto: DeviceEncryptionIdentity::from_secret_bytes([31 + role; 32]),
            commits: 2,
            native_calls: 0,
            max_blob: 0,
            max_guard: 0,
        })
    }
    fn store(&mut self) -> TestResult<&mut db::IndexedDialogVault> {
        self.store.as_mut().ok_or("store closed".into())
    }
    fn commit_id(&mut self) -> Id {
        let n = self.commits;
        self.commits += 1;
        wire::hash(
            b"component commit\0",
            &[&self.scope.context[..], &[self.role], &n.to_be_bytes()].concat(),
        )
    }
    fn commit(&mut self, op: db::HistoryProtocol2Operation) -> TestResult<db::CommitReport> {
        let id = self.commit_id();
        let store = self.store()?;
        Ok(store.commit_history_protocol2(0, store.generation(), id, op)?)
    }
    fn seen(&mut self, meta: &guard::EventMeta) -> TestResult<SeenObservation> {
        let store = self.store()?;
        Ok(store
            .history_seen(0, store.generation(), meta.role, meta.key)?
            .map_or(SeenObservation::Absent, |s| {
                SeenObservation::Present(guard::Seen {
                    generation: s.generation,
                    first: s.first_event,
                })
            }))
    }
    fn before(&mut self) -> TestResult<(u64, Option<Id>)> {
        let store = self.store()?;
        let g = store.generation();
        Ok((g, store.history_checkpoint(0, g)?.native.map(|n| n.hash)))
    }
    fn save_trial(
        &mut self,
        trial: Native,
        event: &wire::ParsedEvent2,
        row: TrialRow,
        before: (u64, Option<Id>),
    ) -> TestResult<()> {
        let TrialRow {
            text,
            auth,
            final_ref,
            receipt,
        } = row;
        let after = trial.seal(
            &self.scope,
            self.q,
            self.role,
            &self.crypto,
            self.view,
            self.cert,
        )?;
        self.max_blob = self.max_blob.max(after.len());
        self.max_guard = self.max_guard.max(trial.guard.encode()?.len());
        let row = db::HistoryReadable2 {
            source: event.raw().to_vec(),
            text,
            auth,
            readable_author_ref: final_ref,
            receiver_receipt: receipt,
        };
        let native = db::HistoryNativeTransition {
            before_hash: before.1,
            after_blob: after,
            frame_ids: vec![event.id()],
        };
        let op = if event.core().n == 0 {
            db::HistoryProtocol2Operation::Genesis { row, native }
        } else {
            db::HistoryProtocol2Operation::TextBatch {
                rows: vec![row],
                native,
            }
        };
        let commit = self.commit_id();
        self.store()?
            .commit_history_protocol2(0, before.0, commit, op)?;
        self.native = trial;
        Ok(())
    }
    fn reload(&mut self) -> TestResult<()> {
        drop(self.store.take());
        let path = self.dir.as_ref().ok_or("fixture dir")?.path();
        self.store = Some(db::IndexedDialogVault::open(
            kilogram_state::StateDirectoryLock::acquire(path)?,
            self.owner,
            self.config,
        )?);
        let store = self.store()?;
        let bytes = store
            .history_checkpoint(0, store.generation())?
            .native
            .ok_or("native after reopen")?
            .blob;
        let restored = Native::open(
            &bytes,
            &self.scope,
            self.q,
            self.role,
            &self.crypto,
            self.view,
            self.cert,
        )?;
        assert_eq!(restored.guard, self.native.guard);
        assert_eq!(
            restored.account.curve_key(),
            self.native.account.curve_key()
        );
        self.native = restored;
        Ok(())
    }
}
fn auth(xs: &[&wire::RootVerifiedCredential]) -> Vec<db::HistoryAuth> {
    let mut map = BTreeMap::new();
    for x in xs {
        map.insert(x.reference(), x.raw().to_vec());
    }
    map.into_iter()
        .map(|(id, bytes)| db::HistoryAuth { id, bytes })
        .collect()
}
#[derive(Clone)]
struct Record {
    original: wire::RootVerifiedCredential,
    event: wire::ParsedEvent2,
    text: String,
    request: [u8; 16],
}
struct Pair {
    actors: support::Actors,
    fresh: support::Fresh,
    selection: wire::StaticSelection2,
    nodes: [Node; 2],
    records: Vec<Record>,
    case: u32,
}
impl Pair {
    fn fresh(case: u32) -> TestResult<Self> {
        let c = wire::hash(b"S3 component C\0", &case.to_be_bytes());
        let actors = support::Actors::new(c)?;
        let (aa, _) = OrderedAccount::fresh(false);
        let (bb, otk) = OrderedAccount::fresh(true);
        let otk = otk.ok_or("OTK")?;
        let fresh = actors.fresh(aa.curve_key(), bb.curve_key(), otk)?;
        let session = aa.outbound(bb.curve_key(), otk)?;
        let sid = session.sid();
        let scope = db::HistoryScope {
            context: c,
            dialog: fresh.proof.proof().d(),
            epoch: fresh.proof.proof().e(),
            session_id: sid.clone(),
        };
        let selection = wire::StaticSelection2::new(fresh.proof.clone(), Some(sid))?;
        let a_node = Node::create(
            scope.clone(),
            &fresh,
            &actors,
            0,
            Native {
                account: aa,
                session: Some(session),
                guard: Guard2::new(0)?,
                generation: 0,
                key: Zeroizing::new([181; 32]),
            },
        )?;
        let b_node = Node::create(
            scope,
            &fresh,
            &actors,
            1,
            Native {
                account: bb,
                session: None,
                guard: Guard2::new(1)?,
                generation: 0,
                key: Zeroizing::new([182; 32]),
            },
        )?;
        let mut p = Self {
            actors,
            fresh,
            selection,
            nodes: [a_node, b_node],
            records: vec![],
            case,
        };
        let a0 = p.send(0)?;
        p.deliver(&a0)?;
        let b0 = p.send(1)?;
        p.deliver(&b0)?;
        assert_eq!(p.calls(), 4);
        Ok(p)
    }
    fn calls(&self) -> u64 {
        self.nodes.iter().map(|n| n.native_calls).sum()
    }
    fn send(&mut self, role: usize) -> TestResult<Record> {
        let node = &mut self.nodes[role];
        let n = node.native.guard.own().next;
        let kind = if n == 0 { role as u8 + 1 } else { 3 };
        let mut request = [0; 16];
        if n == 0 {
            let id = if role == 0 {
                self.fresh.offer.id()
            } else {
                self.fresh.grant.id()
            };
            request.copy_from_slice(&id[..16]);
            assert_ne!(request, [0; 16]);
        } else {
            request[..4].copy_from_slice(&self.case.to_be_bytes());
            request[4..8].copy_from_slice(&(role as u32).to_be_bytes());
            request[8..].copy_from_slice(&n.to_be_bytes());
        }
        let text = if n == 0 {
            String::new()
        } else {
            format!("S3:{}:{role}:{n}:Привет", self.case)
        };
        if n > 0 {
            let id = node.commit_id();
            let store = node.store()?;
            store.commit_history(
                0,
                store.generation(),
                id,
                db::HistoryOperation::QueueAccept {
                    request,
                    text: text.clone(),
                },
            )?;
        }
        let mut parents = vec![];
        if n > 1 {
            parents.extend(node.native.guard.own().last);
        }
        if n > 0 && node.native.guard.peer().next > 1 {
            parents.extend(node.native.guard.peer().last);
        }
        parents.sort();
        parents.dedup();
        let cred = &self.actors.credentials[role];
        let core = wire::Core2 {
            c: self.selection.c(),
            d: self.selection.d(),
            e: self.selection.e(),
            sid: node.scope.session_id.clone(),
            role: role as u8,
            n,
            previous: node.native.guard.own().last,
            sequence: n,
            kind,
            request,
            parents,
            expected: node.native.guard.expected_send()?,
            author_auth: cred.reference(),
            q: self.selection.proof().q(),
        };
        let payload = match kind {
            1 => a([
                b(self.fresh.offer.id()),
                b(self.fresh.grant.id()),
                b(
                    wire::decode(self.fresh.offer.raw(), 4096)?.fields(4)?[2].fields(8)?[3]
                        .fixed::<32>()?,
                ),
                a(node
                    .native
                    .session
                    .as_ref()
                    .ok_or("session")?
                    .keys()
                    .iter()
                    .map(b)),
            ]),
            2 => a([
                b(self.fresh.offer.id()),
                b(self.fresh.grant.id()),
                b(core.sid.as_bytes()),
                b(node.native.guard.peer().first.ok_or("A0")?),
            ]),
            _ => a([b(text.as_bytes())]),
        };
        let plain = Zeroizing::new(wire::inner2(&core, payload)?);
        let (root_generation, before) = node.before()?;
        let expected = guard::EventMeta {
            role: core.role,
            n,
            previous: core.previous,
            sequence: core.sequence,
            kind,
            native_kind: if kind == 1 { 0 } else { 1 },
            key: [0; 32],
            index: core.expected.index,
            expected: core.expected.clone(),
        };
        let step =
            node.native
                .guard
                .preflight_send(&expected, root_generation, node.native.generation)?;
        if !node.native.guard.rotate_due() && node.native.guard.own().next > 0 {
            let own = node.native.guard.own();
            let required = guard::Seen {
                generation: own.g,
                first: own.first.ok_or("own first")?,
            };
            let mut current = expected.clone();
            current.key = own.key.ok_or("own key")?;
            if node.seen(&current)? != SeenObservation::Present(required) {
                return Err("own continuation Seen missing/conflicting".into());
            }
        }
        let mut trial = node.native.fork();
        node.native_calls += 1;
        let packet = trial.session.as_mut().ok_or("session")?.encrypt(&plain)?;
        let event = wire::sign_event(core, &packet, &self.actors.devices[role])?;
        let verified = event.clone().verify(&self.selection, cred)?;
        verified.verify_inner(&plain)?;
        let meta = event.meta();
        let seen = node.seen(&meta)?;
        trial.guard.apply(step, &meta, event.id(), seen)?;
        trial.generation = guard::successor(trial.generation, guard::Counter::NativeGeneration)?;
        node.save_trial(
            trial,
            &event,
            TrialRow {
                text: text.clone(),
                auth: auth(&[cred]),
                final_ref: cred.reference(),
                receipt: None,
            },
            (root_generation, before),
        )?;
        let record = Record {
            original: cred.clone(),
            event,
            text,
            request,
        };
        self.records.push(record.clone());
        Ok(record)
    }
    fn deliver(&mut self, record: &Record) -> TestResult<()> {
        let source = record.event.core().role as usize;
        let role = 1 - source;
        let event = wire::ParsedEvent2::parse(record.event.raw())?;
        let verified = event.clone().verify(&self.selection, &record.original)?;
        let node = &mut self.nodes[role];
        let lane = if source == 0 {
            db::Namespace::LaneA
        } else {
            db::Namespace::LaneB
        };
        if let Some(old) = node
            .store()?
            .get(db::Key::ordinal(0, lane, event.core().n)?)?
        {
            if old.as_slice() != event.id() {
                return Err("conflicting readable duplicate".into());
            }
            let source_raw = node
                .store()?
                .get(db::Key::hash(0, db::Namespace::Event, event.id())?)?
                .ok_or("duplicate source missing")?;
            if source_raw != event.raw() {
                return Err("duplicate immutable bytes".into());
            }
            if event.core().n > 0 {
                let key = wire::hash(
                    b"kg:history2:receipt-wire\0",
                    &[
                        &node.view[..],
                        &[source as u8],
                        &event.core().n.to_be_bytes(),
                    ]
                    .concat(),
                );
                let raw = node
                    .store()?
                    .get(db::Key::hash(0, db::Namespace::Control, key)?)?
                    .ok_or("duplicate receipt missing")?;
                if raw.first() != Some(&2) {
                    return Err("duplicate receipt format".into());
                }
                let receipt = wire::ParsedReceipt2::parse(&raw[1..])?;
                let bundle = node
                    .store()?
                    .get(db::Key::hash(
                        0,
                        db::Namespace::Auth,
                        receipt.fields().receiver_auth,
                    )?)?
                    .ok_or("duplicate receipt authority missing")?;
                let receiver = wire::ParsedCredentialBundle::parse(&bundle)?.verify_root()?;
                receipt.verify(&verified, &self.selection, &receiver)?;
            }
            return Ok(());
        }
        if event.core().n > 0 && node.store()?.pending(0, event.core().n)?.is_none() {
            node.commit(db::HistoryProtocol2Operation::AcceptPending {
                source: event.raw().to_vec(),
                head: false,
                grant: None,
                admission_author_ref: self.actors.credentials[source].reference(),
                auth: auth(&[&record.original, &self.actors.credentials[source]]),
            })?;
        }
        for parent in &event.core().parents {
            let raw = node
                .store()?
                .get(db::Key::hash(0, db::Namespace::Event, *parent)?)?
                .ok_or("parent missing")?;
            assert_eq!(wire::ParsedEvent2::parse(&raw)?.core().kind, 3);
        }
        let (root_generation, before) = node.before()?;
        let meta = event.meta();
        let seen = node.seen(&meta)?;
        let step = node.native.guard.preflight_receive(
            &meta,
            seen,
            root_generation,
            node.native.generation,
        )?;
        let mut trial = node.native.fork();
        node.native_calls += 1;
        let plain = Zeroizing::new(if let Some(session) = &mut trial.session {
            session.decrypt(&event.packet())?
        } else {
            let packet = event.packet();
            let header = packet.header()?;
            let keys = header.prekey_keys.ok_or("prekey keys")?;
            let (session, plain) = trial.account.inbound(keys[0], &packet)?;
            assert_eq!(session.keys(), keys);
            assert_eq!(session.sid(), event.core().sid);
            trial.session = Some(session);
            plain
        });
        let payload = verified.verify_inner(&plain)?;
        if event.core().kind == 3 {
            assert_eq!(payload.value().fields(1)?[0].text()?, record.text);
        } else {
            let f = payload.value().fields(4)?;
            assert_eq!(f[0].fixed::<32>()?, self.fresh.offer.id());
            assert_eq!(f[1].fixed::<32>()?, self.fresh.grant.id());
            if event.core().kind == 1 {
                assert_eq!(
                    f[2],
                    wire::decode(self.fresh.offer.raw(), 4096)?.fields(4)?[2].fields(8)?[3]
                );
                assert_eq!(
                    f[3],
                    a(trial
                        .session
                        .as_ref()
                        .ok_or("session")?
                        .keys()
                        .iter()
                        .map(b))
                );
            } else {
                assert_eq!(f[2].text()?, node.scope.session_id);
                assert_eq!(
                    f[3].fixed::<32>()?,
                    node.native.guard.own().first.ok_or("own A0")?
                );
            }
        }
        trial.guard.apply(step, &meta, event.id(), seen)?;
        trial.generation = guard::successor(trial.generation, guard::Counter::NativeGeneration)?;
        let receipt = if event.core().n > 0 {
            Some(wire::ParsedReceipt2::sign(
                wire::ReceiptFields2 {
                    c: event.core().c,
                    d: event.core().d,
                    e: event.core().e,
                    sid: event.core().sid.clone(),
                    q: event.core().q,
                    format: 2,
                    lane: source as u8,
                    n: event.core().n,
                    event: event.id(),
                    receiver_cert: self.actors.credentials[role].cert_ref(),
                    receiver_auth: self.actors.credentials[role].reference(),
                },
                &self.actors.devices[role],
            )?)
        } else {
            None
        };
        let row_auth = if receipt.is_some() {
            auth(&[
                &self.actors.credentials[source],
                &self.actors.credentials[role],
            ])
        } else {
            auth(&[&self.actors.credentials[source]])
        };
        node.save_trial(
            trial,
            &event,
            TrialRow {
                text: record.text.clone(),
                auth: row_auth,
                final_ref: self.actors.credentials[source].reference(),
                receipt: receipt.as_ref().map(|r| r.raw().to_vec()),
            },
            (root_generation, before),
        )?;
        if let Some(receipt) = receipt {
            receipt
                .clone()
                .verify(&verified, &self.selection, &self.actors.credentials[role])?;
            self.nodes[source].commit(db::HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: receipt.raw().to_vec(),
                auth: auth(&[&self.actors.credentials[role]]),
            })?;
        }
        Ok(())
    }
    fn reload(&mut self) -> TestResult<()> {
        for node in &mut self.nodes {
            node.reload()?;
        }
        Ok(())
    }
    fn verify(&mut self, expected_text: usize) -> TestResult<()> {
        self.verify_timed(expected_text, Instant::now())
    }
    fn verify_timed(&mut self, expected_text: usize, start: Instant) -> TestResult<()> {
        let mut seen_oracle = BTreeMap::new();
        for r in &self.records {
            let m = r.event.meta();
            seen_oracle.entry((m.role, m.key)).or_insert(guard::Seen {
                generation: m.expected.g,
                first: r.event.id(),
            });
        }
        for node in &mut self.nodes {
            for role in 0..=1 {
                let ns = if role == 0 {
                    db::Namespace::SeenA
                } else {
                    db::Namespace::SeenB
                };
                let expected = seen_oracle
                    .iter()
                    .filter(|((r, _), _)| *r == role)
                    .collect::<Vec<_>>();
                assert_eq!(
                    node.store()?
                        .range(db::Key::hash(0, ns, [0; 32])?, 128)?
                        .len(),
                    expected.len()
                );
                let store = node.store()?;
                let generation = store.generation();
                for ((r, key), expected) in expected {
                    let actual = store
                        .history_seen(0, generation, *r, *key)?
                        .ok_or("seen source")?;
                    assert_eq!(actual.generation, expected.generation);
                    assert_eq!(actual.first_event, expected.first);
                }
            }
            for record in &self.records {
                component_budget(start)?;
                let id = record.event.id();
                let raw = node
                    .store()?
                    .get(db::Key::hash(0, db::Namespace::Event, id)?)?
                    .ok_or("source missing")?;
                assert_eq!(raw, record.event.raw());
                let row = node.store()?.history_point(0, id)?;
                if record.event.core().n == 0 {
                    assert!(row.is_none());
                } else {
                    let row = row.ok_or("projection")?;
                    assert_eq!(row.text, record.text);
                    assert_eq!(row.request, record.request);
                }
            }
            let mut count = 0;
            let mut page = node.store()?.history_first(0, 32)?;
            loop {
                component_budget(start)?;
                count += page.items.len();
                if page.finished {
                    break;
                }
                page = node.store()?.history_page(0, &page.cursor, 32)?;
            }
            assert_eq!(count, expected_text);
            assert!(node.max_blob <= guard::MAX_CHECKPOINT);
            assert!(node.max_guard <= guard::MAX_GUARD);
        }
        Ok(())
    }
    fn evidence(&mut self, name: &str, elapsed: f64) -> TestResult<()> {
        println!(
            "{name} sources={} text={} native_calls={} elapsed_seconds={elapsed:.3}",
            self.records.len(),
            self.records.iter().filter(|r| r.event.core().n > 0).count(),
            self.calls()
        );
        for node in &mut self.nodes {
            let mut seen_counts = [0; 2];
            for (role, count) in seen_counts.iter_mut().enumerate() {
                let ns = if role == 0 {
                    db::Namespace::SeenA
                } else {
                    db::Namespace::SeenB
                };
                *count = node
                    .store()?
                    .range(db::Key::hash(0, ns, [0; 32])?, 128)?
                    .len();
            }
            println!(
                "{name} role={} SID={} g_own={} g_peer={} native_generation={} max_blob={} max_guard={}",
                node.role,
                node.scope.session_id,
                node.native.guard.own().g,
                node.native.guard.peer().g,
                node.native.generation,
                node.max_blob,
                node.max_guard
            );
            println!(
                "{name} role={} indexed_seen_counts={seen_counts:?}",
                node.role
            );
            drop(node.store.take());
            let dir = node.dir.take().ok_or("dir")?.keep();
            println!("{name} fixture={}", dir.display());
        }
        Ok(())
    }
}
#[test]
fn a3_s3_component_authority_atomic_reopen() -> TestResult {
    let timer = Instant::now();
    let mut p = Pair::fresh(3)?;
    for _ in 0..4 {
        for role in 0..2 {
            let r = p.send(role)?;
            p.deliver(&r)?;
        }
    }
    p.reload()?;
    p.verify(8)?;
    let record = p.send(0)?;
    let original = record.original.clone();
    p.actors.credentials[0] = p.actors.renewal(0, 3)?;
    let admission = p.actors.credentials[0].clone();
    p.nodes[1].commit(db::HistoryProtocol2Operation::AcceptPending {
        source: record.event.raw().to_vec(),
        head: false,
        grant: None,
        admission_author_ref: admission.reference(),
        auth: auth(&[&original, &admission]),
    })?;
    p.actors.credentials[0] = p.actors.renewal(0, 4)?;
    let final_author = p.actors.credentials[0].clone();
    let before_pending = failure_tests::snapshot(&mut p.nodes[1])?;
    let duplicate_pending = p.nodes[1].commit(db::HistoryProtocol2Operation::AcceptPending {
        source: record.event.raw().to_vec(),
        head: false,
        grant: None,
        admission_author_ref: admission.reference(),
        auth: vec![],
    })?;
    assert!(duplicate_pending.duplicate);
    assert!(
        p.nodes[1]
            .commit(db::HistoryProtocol2Operation::AcceptPending {
                source: record.event.raw().to_vec(),
                head: false,
                grant: None,
                admission_author_ref: final_author.reference(),
                auth: auth(&[&final_author])
            })
            .is_err()
    );
    assert_eq!(failure_tests::snapshot(&mut p.nodes[1])?, before_pending);
    p.actors.credentials[1] = p.actors.renewal(1, 3)?;
    let receiver = p.actors.credentials[1].clone();
    let q = p.selection.proof().q();
    p.deliver(&record)?;
    assert_eq!(p.selection.proof().q(), q);
    let id = record.event.id();
    let view = p.nodes[1].view;
    let fk = wire::hash(
        b"kg:history2:readable-authority\0",
        &[view.as_slice(), id.as_slice()].concat(),
    );
    let f = p.nodes[1]
        .store()?
        .get(db::Key::hash(0, db::Namespace::Control, fk)?)?
        .ok_or("F link")?;
    assert_eq!(f, [vec![1], final_author.reference().to_vec()].concat());
    let a = p.nodes[1]
        .store()?
        .get(db::Key::hash(0, db::Namespace::Admission, id)?)?
        .ok_or("admission")?;
    assert_eq!(
        a,
        [
            vec![5],
            original.reference().to_vec(),
            admission.reference().to_vec()
        ]
        .concat()
    );
    for cred in [&original, &admission, &final_author, &receiver] {
        assert_eq!(
            p.nodes[1]
                .store()?
                .get(db::Key::hash(0, db::Namespace::Auth, cred.reference())?)?
                .ok_or("auth evidence")?,
            cred.raw()
        );
    }
    let n = record.event.core().n;
    let rk = wire::hash(
        b"kg:history2:receipt-wire\0",
        &[view.as_slice(), &[0], &n.to_be_bytes()].concat(),
    );
    let raw = p.nodes[1]
        .store()?
        .get(db::Key::hash(0, db::Namespace::Control, rk)?)?
        .ok_or("wire receipt")?;
    assert_eq!(raw[0], 2);
    let receipt = wire::ParsedReceipt2::parse(&raw[1..])?;
    assert_eq!(receipt.fields().receiver_auth, receiver.reference());
    receipt.clone().verify(
        &record
            .event
            .clone()
            .verify(&p.selection, &record.original)?,
        &p.selection,
        &receiver,
    )?;
    p.actors.credentials[1] = p.actors.renewal(1, 4)?;
    let generation = p.nodes[0].store()?.generation();
    let duplicate = p.nodes[0].commit(db::HistoryProtocol2Operation::Acknowledge {
        receipt_format: 2,
        receipt_raw: receipt.raw().to_vec(),
        auth: vec![],
    })?;
    assert!(duplicate.duplicate);
    assert_eq!(p.nodes[0].store()?.generation(), generation);
    assert!(
        p.nodes[0]
            .commit(db::HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: receipt.raw().to_vec(),
                auth: auth(&[&p.actors.credentials[1]])
            })
            .is_err()
    );
    assert_eq!(p.nodes[0].store()?.generation(), generation);
    assert_eq!(p.calls(), 22);
    p.reload()?;
    p.verify(9)?;
    // Own source without delivery plus renewed peer receipts/F links on the opposite vault.
    let _undelivered = p.send(0)?;
    for node in &mut p.nodes {
        failure_tests::measure_reads(node, "renewal-delivered-undelivered")?;
    }
    p.reload()?;
    for node in &mut p.nodes {
        failure_tests::measure_reads(node, "after-reopen")?;
    }
    let before = failure_tests::snapshot(&mut p.nodes[1])?;
    let commits = p.nodes[1].commits;
    let mut bad = final_author.raw().to_vec();
    let end = bad.len() - 1;
    bad[end] ^= 1;
    let parsed = wire::ParsedCredentialBundle::parse(&bad)?;
    assert_eq!(parsed.cert_ref(), final_author.cert_ref());
    assert_eq!(parsed.reference(), wire::hash(b"", &bad));
    assert!(parsed.verify_root().is_err());
    assert_eq!(p.nodes[1].commits, commits);
    assert_eq!(failure_tests::snapshot(&mut p.nodes[1])?, before);
    println!(
        "a3_s3_A01_positive_traces=2 A03_duplicate_traces=3 A04_bad_root_boundary=1 commit_attempts_after_bad_root=0"
    );
    p.evidence("A01", timer.elapsed().as_secs_f64())
}
#[test]
fn a3_s3_n01_real_native_indexed_long_chain() -> TestResult {
    let timer = Instant::now();
    let mut p = Pair::fresh(1)?;
    for n in 1..=640 {
        component_budget(timer)?;
        let r = p.send(0)?;
        assert_eq!(r.event.meta().index, n - 1);
        assert_eq!(r.event.core().expected.g, 1);
        p.deliver(&r)?;
        if n == 256 || n == 512 {
            p.reload()?;
            println!(
                "N01 checkpoint_text={n} elapsed_seconds={:.3}",
                timer.elapsed().as_secs_f64()
            );
        }
    }
    assert_eq!(p.records.len(), 642);
    assert_eq!(p.calls(), 1284);
    assert_eq!(p.nodes[0].native.guard.own().g, 1);
    assert_eq!(p.nodes[1].native.guard.own().g, 0);
    p.verify_timed(640, timer)?;
    component_budget(timer)?;
    p.evidence("N01", timer.elapsed().as_secs_f64())
}
#[test]
fn a3_s3_n02_real_native_indexed_duplex_late_tails() -> TestResult {
    let timer = Instant::now();
    let mut p = Pair::fresh(2)?;
    let mut carried = None;
    for cycle in 1..=20 {
        component_budget(timer)?;
        let mut aa = vec![];
        if let Some(first) = carried.take() {
            aa.push(first);
        }
        while aa.len() < 16 {
            aa.push(p.send(0)?);
        }
        if cycle == 7 {
            p.deliver(&aa[0])?;
            let bb = (0..16).map(|_| p.send(1)).collect::<TestResult<Vec<_>>>()?;
            p.deliver(&bb[0])?;
            for r in &aa[1..] {
                p.deliver(r)?;
            }
            let a113 = p.send(0)?;
            assert_eq!(a113.event.core().n, 113);
            p.deliver(&a113)?;
            p.reload()?;
            let own = p.nodes[0].native.guard.own().clone();
            assert_eq!((own.g, p.nodes[0].native.guard.peer().g), (8, 7));
            assert!(!p.nodes[0].native.guard.rotate_due());
            for r in &bb[1..] {
                p.deliver(r)?;
                assert_eq!(p.nodes[0].native.guard.own(), &own);
                assert!(!p.nodes[0].native.guard.rotate_due());
                assert_eq!(guard::turn(0, own.g)?, 16);
            }
            carried = Some(a113);
        } else {
            for (i, r) in aa.iter().enumerate() {
                if cycle == 8 && i == 0 {
                    continue;
                }
                p.deliver(r)?;
            }
            for _ in 0..16 {
                let r = p.send(1)?;
                p.deliver(&r)?;
            }
        }
        if matches!(cycle, 5 | 10 | 15) {
            p.reload()?;
            println!(
                "N02 checkpoint_cycle={cycle} elapsed_seconds={:.3}",
                timer.elapsed().as_secs_f64()
            );
        }
    }
    assert_eq!(p.records.len(), 642);
    assert_eq!(p.calls(), 1284);
    for node in &p.nodes {
        assert_eq!(node.native.guard.own().g, 20);
        assert_eq!(node.native.guard.peer().g, 20);
    }
    p.verify_timed(640, timer)?;
    component_budget(timer)?;
    p.evidence("N02", timer.elapsed().as_secs_f64())
}
