//! Closed storage correspondence/atomicity only. Opaque Native fixtures are not MAC evidence.
use super::tests::unhex;
use super::*;
use std::time::Instant;
type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn head(major: u8, n: u64) -> Vec<u8> {
    let mut v = vec![];
    if n < 24 {
        v.push((major << 5) | n as u8);
    } else if n <= 255 {
        v.extend([(major << 5) | 24, n as u8]);
    } else if n <= 65535 {
        v.push((major << 5) | 25);
        v.extend((n as u16).to_be_bytes());
    } else if n <= u64::from(u32::MAX) {
        v.push((major << 5) | 26);
        v.extend((n as u32).to_be_bytes());
    } else {
        v.push((major << 5) | 27);
        v.extend(n.to_be_bytes());
    }
    v
}
fn u(n: u64) -> Vec<u8> {
    head(0, n)
}
fn b(raw: impl AsRef<[u8]>) -> Vec<u8> {
    let raw = raw.as_ref();
    let mut v = head(2, raw.len() as u64);
    v.extend(raw);
    v
}
fn a(items: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    let items = items.into_iter().collect::<Vec<_>>();
    let mut v = head(4, items.len() as u64);
    for item in items {
        v.extend(item);
    }
    v
}
fn opt(id: Option<Id>) -> Vec<u8> {
    a(id.map(b))
}
fn id(n: u64) -> Id {
    hash_parts(b"S3 structural fixture\0", &[&n.to_be_bytes()])
}
#[derive(Clone)]
struct Source {
    scope: HistoryScope,
    q: Id,
    role: u8,
    n: u64,
    previous: Option<Id>,
    request: [u8; 16],
    parents: Vec<Id>,
    g: u64,
    key: Id,
    index: u64,
    parent: Option<Id>,
    tail: Option<source::Tail>,
    author: Id,
}
impl Source {
    fn raw(&self) -> Vec<u8> {
        let s = &self.scope;
        let tail = self
            .tail
            .as_ref()
            .map(|t| a([u(t.g), b(t.key), u(t.index), b(t.last)]));
        let ex = a([u(self.g), u(self.index), opt(self.parent), a(tail)]);
        let core = a([
            u(2),
            b(s.context),
            b(s.dialog),
            b(s.epoch),
            b(s.session_id.as_bytes()),
            u(u64::from(self.role)),
            u(self.n),
            opt(self.previous),
            u(self.n),
            u(if self.n == 0 {
                u64::from(self.role) + 1
            } else {
                3
            }),
            b(self.request),
            a(self.parents.iter().map(b)),
            ex,
            b(self.author),
            b(self.q),
        ]);
        a([
            u(2),
            core,
            a([
                u(u64::from(!(self.role == 0 && self.n == 0))),
                b(self.key),
                u(self.index),
            ]),
            b([0x44]),
            b([0; 64]),
        ])
    }
    fn event(&self) -> Id {
        hash_parts(b"kg:od2:event-id\0", &[&self.raw()])
    }
    fn receipt(&self, cert: Id, auth: Id) -> Vec<u8> {
        a([
            a([
                u(2),
                b(self.scope.context),
                b(self.scope.dialog),
                b(self.scope.epoch),
                b(self.scope.session_id.as_bytes()),
                b(self.q),
                u(2),
                u(u64::from(self.role)),
                u(self.n),
                b(self.event()),
                b(cert),
                b(auth),
            ]),
            b([0; 64]),
        ])
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    store: IndexedDialogVault,
    scope: HistoryScope,
    proof: Vec<u8>,
    fields: FreshFields,
    auth: [HistoryAuth; 4],
    rows: Vec<Source>,
}
fn owner() -> ExpectedOwner {
    ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    }
}
fn config() -> Config {
    Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Default::default()
    }
}
impl Fixture {
    fn new() -> TestResult<Self> {
        Self::build(true)
    }
    fn build(populated: bool) -> TestResult<Self> {
        let all: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../kilogram-protocol/tests/data/s3-differential.json"
            ))?)?;
        let get = |name: &str| -> TestResult<Vec<u8>> {
            unhex(
                all.iter().find(|v| v["name"] == name).ok_or("fixture")?["raw"]
                    .as_str()
                    .ok_or("raw")?,
            )
        };
        let proof = get("D03-fresh")?;
        let fields = read_fresh_binding2(&proof)?;
        let scope = HistoryScope {
            context: fields.c,
            dialog: fields.d,
            epoch: fields.e,
            session_id: "S".repeat(43),
        };
        let mut auth = vec![];
        for name in ["D04-O", "D04-A", "D04-F", "D04-R"] {
            let bytes = get(name)?;
            auth.push(HistoryAuth {
                id: read_auth_bundle(&bytes)?.id,
                bytes,
            });
        }
        let auth: [HistoryAuth; 4] = auth.try_into().map_err(|_| "auth")?;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/a3-s3/state");
        std::fs::create_dir_all(&root)?;
        let dir = tempfile::tempdir_in(root)?;
        let mut store = IndexedDialogVault::create(
            StateDirectoryLock::acquire(dir.path())?,
            CreationClaim {
                account: owner().account,
                claim: owner().claim,
                signing_secret: Zeroizing::new([3; 32]),
                encryption_secret: Zeroizing::new([4; 32]),
                contexts: [scope.context, [0xb6; 32]],
            },
            config(),
        )?;
        if populated {
            store.initialize_history_protocol2(
                0,
                store.generation(),
                id(1),
                HistoryInit {
                    scope: scope.clone(),
                    own_role: 0,
                },
                HistoryProtocol2Binding {
                    proof: proof.clone(),
                    author_slots: [0, 1],
                },
            )?;
        }
        let mut f = Self {
            dir,
            store,
            scope,
            proof,
            fields,
            auth,
            rows: vec![],
        };
        if populated {
            for role in 0..2 {
                let row = f.next(role)?;
                f.append(row, false)?;
            }
            for _ in 0..4 {
                for role in 0..2 {
                    let row = f.next(role)?;
                    f.append(row, true)?;
                }
            }
            assert_eq!(f.rows.len(), 10);
        }
        Ok(f)
    }
    fn next(&self, role: u8) -> TestResult<Source> {
        let last = self.rows.iter().rev().find(|s| s.role == role);
        let n = last.map_or(0, |s| s.n + 1);
        let mut request = [0; 16];
        if n == 0 {
            request.copy_from_slice(
                &if role == 0 {
                    self.fields.offer
                } else {
                    self.fields.grant
                }[..16],
            );
            assert_ne!(request, [0; 16]);
        } else {
            request[..8].copy_from_slice(&u64::from(role).to_be_bytes());
            request[8..].copy_from_slice(&n.to_be_bytes());
        }
        let mut parents = self
            .rows
            .iter()
            .rev()
            .filter(|s| s.n > 0)
            .fold([None, None], |mut heads, s| {
                heads[usize::from(s.role)].get_or_insert(s.event());
                heads
            })
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if n == 0 {
            parents.clear();
        }
        parents.sort();
        parents.dedup();
        let peer = self.rows.iter().rev().find(|s| s.role != role);
        Ok(Source {
            scope: self.scope.clone(),
            q: self.fields.q,
            role,
            n,
            previous: last.map(Source::event),
            request,
            parents,
            g: n,
            key: id(100 + u64::from(role) * 32 + n),
            index: 0,
            parent: peer.map(Source::event),
            tail: last.map(|s| source::Tail {
                g: s.g,
                key: s.key,
                index: s.index,
                last: s.event(),
            }),
            author: self.auth[if role == 0 { 0 } else { 3 }].id,
        })
    }
    fn execute(&mut self, op: HistoryProtocol2Operation) -> Result<CommitReport> {
        let g = self.store.generation();
        self.store.commit_history_protocol2(0, g, id(g + 1000), op)
    }
    fn prepare(&mut self, s: &Source) -> Result<()> {
        if s.n == 0 {
            return Ok(());
        }
        if s.role == 0 {
            self.store.commit_history(
                0,
                self.store.generation(),
                id(2000 + self.store.generation()),
                HistoryOperation::QueueAccept {
                    request: s.request,
                    text: format!("structural:{}:{}", s.role, s.n),
                },
            )?;
        } else {
            self.execute(HistoryProtocol2Operation::AcceptPending {
                source: s.raw(),
                head: false,
                grant: None,
                admission_author_ref: s.author,
                auth: vec![],
            })?;
        }
        Ok(())
    }
    fn readable(&self, s: &Source) -> HistoryReadable2 {
        HistoryReadable2 {
            source: s.raw(),
            text: if s.n == 0 {
                String::new()
            } else {
                format!("structural:{}:{}", s.role, s.n)
            },
            auth: if s.n == 0 {
                vec![self.auth[if s.role == 0 { 0 } else { 3 }].clone()]
            } else {
                vec![]
            },
            readable_author_ref: s.author,
            receiver_receipt: (s.n > 0 && s.role == 1)
                .then(|| s.receipt(self.fields.certs[0], self.auth[0].id)),
        }
    }
    fn native(&mut self, sources: &[Source]) -> Result<HistoryNativeTransition> {
        Ok(HistoryNativeTransition {
            before_hash: self
                .store
                .history_checkpoint(0, self.store.generation())?
                .native
                .map(|n| n.hash),
            after_blob: [
                vec![0x4e; 224],
                sources.last().map_or([0; 32], Source::event).to_vec(),
            ]
            .concat(),
            frame_ids: sources.iter().map(Source::event).collect(),
        })
    }
    fn operation(&mut self, s: &Source) -> Result<HistoryProtocol2Operation> {
        let native = self.native(std::slice::from_ref(s))?;
        let row = self.readable(s);
        Ok(if s.n == 0 {
            HistoryProtocol2Operation::Genesis { row, native }
        } else {
            HistoryProtocol2Operation::TextBatch {
                rows: vec![row],
                native,
            }
        })
    }
    fn append(&mut self, s: Source, ack: bool) -> Result<()> {
        self.prepare(&s)?;
        let op = self.operation(&s)?;
        self.execute(op)?;
        if ack && s.role == 0 {
            let raw = s.receipt(self.fields.certs[1], self.auth[3].id);
            self.execute(HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: raw,
                auth: vec![self.auth[3].clone()],
            })?;
        }
        self.rows.push(s);
        Ok(())
    }
    fn assert_unchanged(&self, before: &[u8]) {
        assert_eq!(self.store.root.encode().as_slice(), before);
    }
    fn reopen(mut self) -> TestResult<Self> {
        drop(self.store);
        self.store = IndexedDialogVault::open(
            StateDirectoryLock::acquire(self.dir.path())?,
            owner(),
            config(),
        )?;
        Ok(self)
    }
    fn c2(&mut self) -> TestResult<Vec<(Key, Vec<u8>)>> {
        use super::super::super::tests as old;
        let mut init = old::init(1);
        init.scope.context = [0xb6; 32];
        self.store
            .initialize_history(1, self.store.generation(), id(8000), init.clone())?;
        let mut points = vec![];
        for role in 0..2 {
            let mut row = old::prepared(1, role, 0, None, [0; 16], "", vec![])?;
            row.scope = init.scope.clone();
            let event = row.event_id()?;
            let native = old::native(&mut self.store, 1, std::slice::from_ref(&row))?;
            self.store.commit_history(
                1,
                self.store.generation(),
                id(8001 + u64::from(role)),
                HistoryOperation::Genesis { row, native },
            )?;
            let key = Key::hash(1, Namespace::Event, event)?;
            points.push((key, self.store.get(key)?.ok_or("C2 source")?));
        }
        Ok(points)
    }
}
fn snapshot(store: &IndexedDialogVault) -> Vec<u8> {
    store.root.encode().to_vec()
}
#[test]
fn a3_s3_s01_s02_stateful_source_receipt_rejections() -> TestResult {
    let mut f = Fixture::new()?;
    let source = f.next(0)?;
    f.prepare(&source)?;
    let before = snapshot(&f.store);
    for case in 0..8 {
        let mut bad = source.clone();
        match case {
            0 => bad.role = 1,
            1 => bad.n += 1,
            2 => {
                bad.previous = Some(id(99));
                bad.parents[0] = id(99);
                bad.parents.sort();
            }
            3 => bad.scope.context = [99; 32],
            4 => bad.q = [99; 32],
            5 => bad.request = [99; 16],
            6 => bad.key = f.rows[0].key,
            _ => bad.g += 1,
        }
        read_event2(&bad.raw())?;
        let op = f.operation(&bad)?;
        assert!(f.execute(op).is_err(), "S01 case{case}");
        f.assert_unchanged(&before);
    }
    let op = f.operation(&source)?;
    // The eight shared D05 bytes are also rejected by this closed writer. The
    // contextual vectors above isolate each stateful disagreement at the current head.
    let shared: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kilogram-protocol/tests/data/s3-differential.json"
    ))?)?;
    let mut d05 = 0;
    for v in shared
        .iter()
        .filter(|v| v["name"].as_str().is_some_and(|s| s.starts_with("D05-")))
    {
        let raw = unhex(v["raw"].as_str().ok_or("raw")?)?;
        let e = read_event2(&raw)?;
        let mut row = f.readable(&source);
        row.source = raw;
        let mut native = f.native(std::slice::from_ref(&source))?;
        native.frame_ids = vec![e.id];
        assert!(
            f.execute(HistoryProtocol2Operation::TextBatch {
                rows: vec![row],
                native
            })
            .is_err()
        );
        f.assert_unchanged(&before);
        d05 += 1;
    }
    assert_eq!(d05, 8);
    f.execute(op)?;
    f.rows.push(source.clone());
    let before = snapshot(&f.store);
    for case in 0..8 {
        let mut bad = source.clone();
        let mut cert = f.fields.certs[1];
        let mut auth = f.auth[3].id;
        match case {
            0 => {
                bad.scope.context = [99; 32];
                bad.scope.dialog = [98; 32];
                bad.scope.epoch = [97; 32];
                bad.scope.session_id = "Z".repeat(43);
            }
            1 => bad.q = [99; 32],
            3 => bad.role = 1,
            4 => bad.n += 1,
            6 => cert = f.fields.certs[0],
            7 => auth = f.auth[0].id,
            _ => {}
        }
        let raw = a([
            a([
                u(2),
                b(bad.scope.context),
                b(bad.scope.dialog),
                b(bad.scope.epoch),
                b(bad.scope.session_id.as_bytes()),
                b(bad.q),
                u(if case == 2 { 1 } else { 2 }),
                u(u64::from(bad.role)),
                u(bad.n),
                b(if case == 5 { [99; 32] } else { source.event() }),
                b(cert),
                b(auth),
            ]),
            b([0; 64]),
        ]);
        assert!(
            f.execute(HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: raw,
                auth: vec![]
            })
            .is_err(),
            "S02 case{case}"
        );
        f.assert_unchanged(&before);
        assert!(
            f.store
                .get(Key::ordinal(0, Namespace::Undelivered, source.n)?)?
                .is_some()
        );
    }
    println!("a3_s3_S01=8 S02=8 D05_same_raw_writer_rejects=8 structural_only=true native_calls=0");
    Ok(())
}
macro_rules! work {
    ($f:expr,$w:ident,$body:block) => {{
        let tx = $f.store.vault.database.begin_read()?;
        let nodes = tx.open_table(NODES)?;
        let values = tx.open_table(VALUES)?;
        let mut $w = Work::new(&nodes, &values, &$f.store.crypto, $f.store.root.clone());
        $body
    }};
}
#[test]
fn a3_s3_s03_s04_atomic_retry_and_shared_fences() -> TestResult {
    let mut f = Fixture::new()?;
    let c2 = f.c2()?;
    let c2_roots = f.store.root.roots[24..48].to_vec();
    let source = f.next(0)?;
    f.prepare(&source)?;
    let op = f.operation(&source)?;
    let before = snapshot(&f.store);
    let g = f.store.generation();
    assert!(matches!(
        f.store
            .commit_history_protocol2(0, g - 1, id(9000), op.clone()),
        Err(Error::StaleGeneration)
    ));
    f.assert_unchanged(&before);
    let mut wrong = op.clone();
    if let HistoryProtocol2Operation::TextBatch { native, .. } = &mut wrong {
        native.before_hash = Some([99; 32]);
    }
    assert!(f.execute(wrong).is_err());
    f.assert_unchanged(&before);
    work!(f, w, {
        let key = seen_key(0, 0, f.rows[0].key)?;
        let old = w.hrequired(key)?;
        let mut bad = old;
        bad[39] ^= 1;
        assert!(matches!(w.put(key, &bad, 0, false), Err(Error::Conflict)));
    });
    work!(f, w, {
        let key = Key::hash(0, Namespace::Event, f.rows[0].event())?;
        let mut bad = f.rows[0].raw();
        let end = bad.len() - 1;
        bad[end] ^= 1;
        assert!(matches!(w.put(key, &bad, 0, false), Err(Error::Conflict)));
    });
    let row = f.readable(&source);
    let native = f.native(std::slice::from_ref(&source))?;
    for duplicate in [false, true] {
        work!(f, w, {
            let (identity, heads) = w.history_view(0)?;
            let (b, p) = w.proto_required(0)?;
            let mut plan = w.proto_rows(
                0,
                &identity,
                heads,
                &b,
                &p,
                RowsInput2 {
                    rows: vec![row.clone()],
                    native: native.clone(),
                    genesis: false,
                },
            )?;
            if duplicate {
                plan.rows.push(plan.rows[0].clone());
                plan.credits.push(plan.credits[0].clone());
            } else {
                plan.credits.clear();
            }
            assert!(w.history_execute(plan).is_err());
        });
        f.assert_unchanged(&before);
    }
    let commit = id(9001);
    let report = f.store.commit_history_protocol2(0, g, commit, op.clone())?;
    assert!(!report.duplicate);
    let accepted = snapshot(&f.store);
    let duplicate = f.store.commit_history_protocol2(0, g, commit, op.clone())?;
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.changed_nodes, 0);
    f.assert_unchanged(&accepted);
    let mut different = op.clone();
    if let HistoryProtocol2Operation::TextBatch { rows, .. } = &mut different {
        rows[0].source[4] ^= 1;
    }
    assert!(matches!(
        f.store.commit_history_protocol2(0, g, commit, different),
        Err(Error::Conflict)
    ));
    f.assert_unchanged(&accepted);
    let raw = source.receipt(f.fields.certs[1], f.auth[3].id);
    let ack = HistoryProtocol2Operation::Acknowledge {
        receipt_format: 2,
        receipt_raw: raw.clone(),
        auth: vec![],
    };
    let g = f.store.generation();
    let cid = id(9002);
    f.store.commit_history_protocol2(0, g, cid, ack.clone())?;
    let mut changed = raw;
    let end = changed.len() - 1;
    changed[end] ^= 1;
    let accepted = snapshot(&f.store);
    assert!(matches!(
        f.store.commit_history_protocol2(
            0,
            g,
            cid,
            HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: changed,
                auth: vec![]
            }
        ),
        Err(Error::Conflict)
    ));
    f.assert_unchanged(&accepted);
    assert_eq!(f.store.root.roots[24..48], c2_roots);
    for (key, value) in c2 {
        assert_eq!(f.store.get(key)?, Some(value));
    }
    f.rows.push(source);
    let mut f = f.reopen()?;
    assert_eq!(f.store.history_first(0, 32)?.items.len(), 9);
    println!(
        "a3_s3_S03=6 S04_direct=3 S04_ambiguous_in_S05=1 C2_genesis=2 C1_base_sources=10 cross_C_unchanged=true"
    );
    Ok(())
}
#[test]
fn a3_s3_s05_faults_capacity_and_ambiguous_retry() -> TestResult {
    for point in ["before_write", "before_sync", "after_sync"] {
        let mut f = Fixture::new()?;
        let source = f.next(0)?;
        f.prepare(&source)?;
        let op = f.operation(&source)?;
        let g = f.store.generation();
        let commit = id(9100);
        let before = snapshot(&f.store);
        f.store.backend.fail(point, 0)?;
        assert!(
            f.store
                .commit_history_protocol2(0, g, commit, op.clone())
                .is_err()
        );
        assert!(matches!(
            f.store.history_first(0, 32),
            Err(Error::ReopenRequired)
        ));
        let mut f = f.reopen()?;
        let new = f
            .store
            .get(Key::hash(0, Namespace::Event, source.event())?)?
            .is_some();
        if !new {
            f.assert_unchanged(&before);
        } else {
            assert_eq!(f.store.generation(), g + 1);
        }
        let retry = f.store.commit_history_protocol2(0, g, commit, op.clone())?;
        assert_eq!(retry.duplicate, new);
        let accepted = snapshot(&f.store);
        assert!(
            f.store
                .commit_history_protocol2(0, g, commit, op)?
                .duplicate
        );
        f.assert_unchanged(&accepted);
        assert_eq!(
            f.store
                .get(Key::hash(0, Namespace::Event, source.event())?)?,
            Some(source.raw())
        );
        assert_eq!(
            f.store
                .history_point(0, source.event())?
                .ok_or("row")?
                .ordinal,
            source.n
        );
        assert_eq!(f.store.history_first(0, 32)?.items.len(), 9);
        assert!(
            f.store
                .get(Key::ordinal(0, Namespace::Undelivered, source.n)?)?
                .is_some()
        );
        println!(
            "a3_s3_storage_fault={point} root_outcome={} exact_retry_duplicate={} next_native_trial=0",
            if new { "new" } else { "old" },
            retry.duplicate
        );
    }
    let mut f = Fixture::new()?;
    let funded = f.next(0)?;
    f.prepare(&funded)?;
    let before = snapshot(&f.store);
    let path = f.dir.path().to_path_buf();
    drop(f.store);
    let mut tight = config();
    tight.physical_quota = 1;
    assert!(matches!(
        IndexedDialogVault::open(StateDirectoryLock::acquire(&path)?, owner(), tight),
        Err(Error::ConfiguredCapacity { .. })
    ));
    f.store = IndexedDialogVault::open(StateDirectoryLock::acquire(&path)?, owner(), config())?;
    f.assert_unchanged(&before);
    assert!(
        f.store
            .history_request(0, funded.request)?
            .is_some_and(|r| r.event.is_none())
    );
    println!(
        "a3_s3_S05=4 S04_ambiguous=1 configured_capacity_open_refusal=1 funded_obligation_preserved=true"
    );
    Ok(())
}
#[test]
fn a3_s3_s06_closed_writer_bypasses_rejected() -> TestResult {
    use super::super::super::tests as old;
    let mut f = Fixture::new()?;
    let c2 = f.c2()?;
    let before = snapshot(&f.store);
    assert!(
        f.store
            .commit(
                f.store.generation(),
                id(9200),
                vec![Mutation::put(
                    Key::hash(0, Namespace::Event, id(99))?,
                    vec![1]
                )],
                vec![]
            )
            .is_err()
    );
    f.assert_unchanged(&before);
    let trusted = old::prepared(0, 0, 1, Some(f.rows[0].event()), [1; 16], "bypass", vec![])?;
    let native = f.native(&[])?;
    let original = f.auth[0].clone();
    let pending = old::prepared(0, 1, 5, Some(f.rows[9].event()), [2; 16], "bypass", vec![])?;
    let cases = [
        HistoryOperation::TextBatch {
            rows: vec![trusted],
            native,
        },
        HistoryOperation::Acknowledge {
            ordinal: 1,
            event: f.rows[2].event(),
            auth: vec![],
        },
        HistoryOperation::AcceptPending(HistoryPending {
            scope: f.scope.clone(),
            format: 2,
            ordinal: 5,
            source: pending.source,
            head: false,
            grant: None,
            capture: None,
        }),
        HistoryOperation::CapturePending {
            ordinal: 5,
            event: id(99),
            capture: HistoryCapture {
                original: original.clone(),
                admission: original,
            },
        },
    ];
    for op in cases {
        assert!(matches!(
            f.store
                .commit_history(0, f.store.generation(), id(9201), op),
            Err(Error::Bound("protocol2 managed writer required"))
        ));
        f.assert_unchanged(&before);
    }
    let mut bare = Fixture::build(false)?;
    bare.store.commit(
        bare.store.generation(),
        id(9290),
        vec![Mutation::put(
            Key::hash(0, Namespace::Native, [0; 32])?,
            vec![0x4e; 128],
        )],
        vec![],
    )?;
    let dirty = snapshot(&bare.store);
    assert!(matches!(
        bare.execute(HistoryProtocol2Operation::Init {
            init: HistoryInit {
                scope: bare.scope.clone(),
                own_role: 0
            },
            binding: HistoryProtocol2Binding {
                proof: bare.proof.clone(),
                author_slots: [0, 1]
            }
        }),
        Err(Error::ExistingStateRequiresMigration)
    ));
    bare.assert_unchanged(&dirty);
    f.assert_unchanged(&before);
    for (key, value) in c2 {
        assert_eq!(f.store.get(key)?, Some(value));
    }
    println!("a3_s3_S06=6 bound2_bypasses=0 cross_C_unchanged=true");
    Ok(())
}
#[test]
fn a3_s3_a02_resolver_inputs_missing_roles_and_integrity() -> TestResult {
    let mut f = Fixture::new()?;
    let mut peer = f.next(1)?;
    let pending = |s: &Source, admission: Id, auth: Vec<HistoryAuth>| {
        HistoryProtocol2Operation::AcceptPending {
            source: s.raw(),
            head: false,
            grant: None,
            admission_author_ref: admission,
            auth,
        }
    };
    let before = snapshot(&f.store);
    let wrong = HistoryAuth {
        id: [99; 32],
        bytes: f.auth[3].bytes.clone(),
    };
    assert!(f.execute(pending(&peer, peer.author, vec![wrong])).is_err());
    f.assert_unchanged(&before);
    peer.author = [99; 32];
    assert!(matches!(
        f.execute(pending(&peer, peer.author, vec![])),
        Err(Error::Integrity("MissingReferencedAuth"))
    ));
    f.assert_unchanged(&before);
    peer.author = f.auth[3].id;
    assert!(matches!(
        f.execute(pending(&peer, [99; 32], vec![])),
        Err(Error::Integrity("MissingReferencedAuth"))
    ));
    f.assert_unchanged(&before);
    let duplicate = f.auth[3].clone();
    assert!(
        f.execute(pending(
            &peer,
            peer.author,
            vec![duplicate.clone(), duplicate]
        ))
        .is_err()
    );
    f.assert_unchanged(&before);
    assert!(
        f.execute(pending(&peer, peer.author, vec![f.auth[1].clone()]))
            .is_err()
    );
    f.assert_unchanged(&before);
    let own = f.next(0)?;
    f.prepare(&own)?;
    let mut op = f.operation(&own)?;
    if let HistoryProtocol2Operation::TextBatch { rows, .. } = &mut op {
        rows[0].readable_author_ref = [99; 32];
    }
    let before = snapshot(&f.store);
    assert!(matches!(
        f.execute(op),
        Err(Error::Integrity("MissingReferencedAuth"))
    ));
    f.assert_unchanged(&before);
    let op = f.operation(&own)?;
    f.execute(op)?;
    let before = snapshot(&f.store);
    assert!(matches!(
        f.execute(HistoryProtocol2Operation::Acknowledge {
            receipt_format: 2,
            receipt_raw: own.receipt(f.fields.certs[1], [99; 32]),
            auth: vec![]
        }),
        Err(Error::Integrity("MissingReferencedAuth"))
    ));
    f.assert_unchanged(&before);
    assert!(
        f.execute(HistoryProtocol2Operation::Acknowledge {
            receipt_format: 2,
            receipt_raw: own.receipt(f.fields.certs[0], f.auth[0].id),
            auth: vec![]
        })
        .is_err()
    );
    f.assert_unchanged(&before);
    println!(
        "a3_s3_A02=8 exact_missing_auth_categories=4 inputs_checked_hash_sorted_used=true root_credits_native_unchanged=true"
    );
    Ok(())
}
fn corrupt_cipher(store: &mut IndexedDialogVault, key: Key, remove: bool) -> TestResult<()> {
    let address = store
        .crypto
        .address(key.slot()? as u8, b"value", &key.bytes);
    let tx = store.vault.database.begin_write()?;
    {
        let mut table = tx.open_table(VALUES)?;
        let mut raw = table
            .get(address.as_slice())?
            .ok_or("expected cipher")?
            .value()
            .to_vec();
        if remove {
            table.remove(address.as_slice())?;
        } else {
            let end = raw.len() - 1;
            raw[end] ^= 1;
            table.insert(address.as_slice(), raw.as_slice())?;
        }
    }
    tx.commit()?;
    Ok(())
}
#[test]
fn a3_s3_s07_genesis_normalization_and_fragment_correspondence() -> TestResult {
    let f = Fixture::new()?;
    let mut f = f.reopen()?;
    for role in 0..2 {
        let s = &f.rows[role];
        assert_ne!(s.request, [0; 16]);
        let raw = f
            .store
            .get(Key::hash(0, Namespace::Event, s.event())?)?
            .ok_or("source")?;
        assert_eq!(raw, s.raw());
        let e = read_event2(&raw)?;
        let projected = ProjectionValue::decode(
            &f.store
                .get(Key::hash(0, Namespace::Projection, s.event())?)?
                .ok_or("projection")?,
        )?;
        let binding = Binding::decode(&projected.binding)?;
        assert_eq!(binding.request, [0; 16]);
        assert_eq!(binding.core_offset, e.core_offset);
        assert_eq!(binding.core_len, e.core_len);
        binding.verify_source(s.event(), &raw)?;
        assert!(f.store.history_point(0, s.event())?.is_none());
        let mut wrong = s.clone();
        wrong.request[0] ^= 1;
        let op = f.operation(&wrong)?;
        let before = snapshot(&f.store);
        assert!(matches!(
            f.execute(op),
            Err(Error::Integrity("genesis request/R4"))
        ));
        f.assert_unchanged(&before);
    }
    let c2 = f.c2()?;
    {
        let (key, raw) = c2.into_iter().next().ok_or("unbound genesis")?;
        let projected = ProjectionValue::decode(
            &f.store
                .get(Key::hash(1, Namespace::Projection, key.bytes)?)?
                .ok_or("unbound projection")?,
        )?;
        let mut binding = Binding::decode(&projected.binding)?;
        assert_eq!(binding.format, 1);
        assert_eq!(binding.request, [0; 16]);
        binding.verify_source(key.bytes, &raw)?;
        binding.request = [1; 16];
        assert!(binding.encode().is_err());
    }
    let source = f.rows[6].clone();
    let raw = source.raw();
    let key = Key::hash(0, Namespace::Projection, source.event())?;
    let projected = ProjectionValue::decode(&f.store.get(key)?.ok_or("projection")?)?;
    let mut bad = Binding::decode(&projected.binding)?;
    bad.core_offset += 1;
    bad.core_len -= 1;
    bad.core_hash = hash_parts(
        b"kg:history2:core\0",
        &[&[2], core_span(&raw, bad.core_offset, bad.core_len)?],
    );
    bad.verify_source(source.event(), &raw)?;
    let before = snapshot(&f.store);
    work!(f, w, {
        w.put(
            key,
            &ProjectionValue {
                text: projected.text,
                binding: bad.encode()?,
            }
            .encode()?,
            0,
            true,
        )?;
        let (_, h) = w.history_view(0)?;
        assert!(matches!(
            w.history_projection(0, source.event(), &h.view),
            Err(Error::Integrity(
                "protocol2 source/projection correspondence"
            ))
        ));
        assert!(w.proto_source(0, source.event(), &h).is_err());
    });
    f.assert_unchanged(&before);
    println!(
        "a3_s3_S07_positive_genesis=2 unbound_compatibility_vectors=2 signed_prefix_negatives_in_C09=2 corruption_fragment_vectors=1 HPB1_predicate_unchanged=true"
    );
    Ok(())
}
#[test]
fn a3_s3_mandatory_evidence_fail_closed_after_reopen() -> TestResult {
    for case in 0..8 {
        let mut f = Fixture::new()?;
        let source = f.rows[7].clone();
        let view = f.store.history_checkpoint(0, f.store.generation())?.view;
        let key = match case {
            0 | 1 => binding_key(0)?,
            2 | 3 => final_key(0, view, source.event())?,
            4 | 5 => Key::hash(0, Namespace::Auth, source.author)?,
            _ => wire_receipt_key(0, view, source.role, source.n)?,
        };
        let cursor = f.store.history_first(0, 1)?.cursor;
        let root = snapshot(&f.store);
        corrupt_cipher(&mut f.store, key, case % 2 == 0)?;
        f.assert_unchanged(&root);
        assert!(
            matches!(f.store.history_first(0, 32), Err(Error::Integrity(_))),
            "case{case}"
        );
        assert!(
            matches!(
                f.store.history_page(0, &cursor, 32),
                Err(Error::Integrity(_))
            ),
            "cursor case{case}"
        );
        assert!(
            matches!(
                f.store.history_point(0, f.rows[8].event()),
                Err(Error::Integrity(_))
            ),
            "noninitial parent case{case}"
        );
        let mut f = f.reopen()?;
        assert!(
            matches!(f.store.history_first(0, 32), Err(Error::Integrity(_))),
            "reopen case{case}"
        );
        let generation = f.store.generation();
        assert!(
            matches!(
                f.store.history_seen(0, generation, source.role, source.key),
                Err(Error::Integrity(_))
            ),
            "first source case{case}"
        );
        assert!(
            matches!(
                f.store.history_checkpoint(0, generation),
                Err(Error::Integrity(_))
            ),
            "checkpoint coherence case{case}"
        );
    }
    // Missing historical R also rejects an exact already-durable ACK; no duplicate shortcut.
    let mut f = Fixture::new()?;
    let own = f
        .rows
        .iter()
        .find(|s| s.role == 0 && s.n == 4)
        .ok_or("own")?
        .clone();
    let raw = own.receipt(f.fields.certs[1], f.auth[3].id);
    let auth_key = Key::hash(0, Namespace::Auth, f.auth[3].id)?;
    corrupt_cipher(&mut f.store, auth_key, true)?;
    assert!(matches!(
        f.execute(HistoryProtocol2Operation::Acknowledge {
            receipt_format: 2,
            receipt_raw: raw,
            auth: vec![]
        }),
        Err(Error::Integrity(_))
    ));
    let mut f = Fixture::new()?;
    let own = f.next(0)?;
    f.append(own.clone(), false)?;
    let g = f.store.generation();
    let cid = id(9900);
    let ack = HistoryProtocol2Operation::Acknowledge {
        receipt_format: 2,
        receipt_raw: own.receipt(f.fields.certs[1], f.auth[3].id),
        auth: vec![],
    };
    f.store.commit_history_protocol2(0, g, cid, ack.clone())?;
    let before = snapshot(&f.store);
    let auth_key = Key::hash(0, Namespace::Auth, f.auth[3].id)?;
    corrupt_cipher(&mut f.store, auth_key, true)?;
    assert!(matches!(
        f.store.commit_history_protocol2(0, g, cid, ack),
        Err(Error::Integrity(_))
    ));
    f.assert_unchanged(&before);
    println!(
        "a3_s3_required_evidence_negatives=8 missing_R_duplicate_ack_calls=2 no_partial_page_or_cursor=true noninitial_parent_covered=true"
    );
    Ok(())
}
fn read_measure(store: &mut IndexedDialogVault, c: u8, bound2: bool, tag: &str) -> TestResult<()> {
    for limit in [1, 32] {
        let timer = Instant::now();
        let mut page = store.history_first(c, limit)?;
        let mut peak = (0, 0, 0);
        let mut rows = 0;
        let mut pages = 0;
        loop {
            let bound = if bound2 {
                40 * (limit as u64 + 2) + 3
            } else {
                8 * (limit as u64 + 2) + 2
            };
            assert!(page.stats.point_gets <= bound);
            assert!(page.stats.max_candidates <= 2);
            assert!(page.stats.candidate_loads <= limit as u64 + 2);
            peak.0 = peak.0.max(page.stats.point_gets);
            peak.1 = peak.1.max(page.stats.candidate_loads);
            peak.2 = peak.2.max(page.stats.max_candidates);
            rows += page.items.len();
            pages += 1;
            if page.finished {
                break;
            }
            page = store.history_page(c, &page.cursor, limit)?;
        }
        assert_eq!(rows, 8);
        println!(
            "history_read mode={} tag={tag} limit={limit} rows={rows} pages={pages} observed_max_point_gets={} candidate_loads={} max_candidates={} elapsed_seconds={:.6}",
            if bound2 { "bound2" } else { "unbound" },
            peak.0,
            peak.1,
            peak.2,
            timer.elapsed().as_secs_f64()
        );
    }
    Ok(())
}
#[test]
fn a3_s3_bound2_and_unbound_budget_separation() -> TestResult {
    use super::super::super::tests as old;
    let mut f = Fixture::new()?;
    let genesis = f.c2()?;
    let mut heads = [genesis[0].0.bytes, genesis[1].0.bytes];
    let mut parent_heads: [Option<HistoryParent>; 2] = [None, None];
    for n in 1..=4 {
        for role in 0..2 {
            let parents = parent_heads.iter().flatten().cloned().collect();
            let mut row = old::prepared(
                1,
                role as u8,
                n,
                Some(heads[role]),
                [role as u8 * 16 + n as u8; 16],
                "unbound",
                parents,
            )?;
            row.scope.context = [0xb6; 32];
            let event = row.event_id()?;
            old::append(&mut f.store, 1, row)?;
            heads[role] = event;
            parent_heads[role] = Some(HistoryParent {
                event,
                role: role as u8,
                ordinal: n,
            });
        }
    }
    read_measure(&mut f.store, 0, true, "before-reopen")?;
    read_measure(&mut f.store, 1, false, "before-reopen")?;
    let mut f = f.reopen()?;
    read_measure(&mut f.store, 0, true, "after-reopen")?;
    read_measure(&mut f.store, 1, false, "after-reopen")?;
    Ok(())
}
