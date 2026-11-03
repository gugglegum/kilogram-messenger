//! Actual authenticated V4 -> Ready storage path, not an Engine/Native origin claim.
use super::*;
use crate::vault::indexed_dialog::migration7::{MigrationRepository, V4Source};
use crate::vault::ordered::{OrderedMutation, OrderedVault};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
struct Fixture {
    _dir: tempfile::TempDir,
    state: std::path::PathBuf,
    config: super::super::super::MigrationConfig,
    owner: ExpectedOwner,
    heads: Vec<Vec<u8>>,
    native: Option<Vec<u8>>,
}
impl Fixture {
    fn ready(
        contexts: usize,
        with_native: bool,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?
            .join(".tmp/s54-block4ae-cold-read-01/fixtures");
        std::fs::create_dir_all(&base)?;
        let dir = tempfile::Builder::new().prefix("fresh-").tempdir_in(base)?;
        let state = dir.path().join("state");
        std::fs::create_dir(&state)?;
        let owner = ExpectedOwner {
            account: [1; 32],
            claim: [2; 32],
        };
        let device = [3; 32];
        let peer = [4; 32];
        let certificate = [5; 32];
        let peer_certificate = [6; 32];
        let certs = [
            *blake3::hash(&certificate).as_bytes(),
            *blake3::hash(&peer_certificate).as_bytes(),
        ];
        let native = with_native.then(|| vec![7; 64]);
        let native_hash = native
            .as_ref()
            .map(|raw| hash(b"kg:od1:native-state\0", raw));
        let authority = vec![8; 64];
        let config = super::super::super::MigrationConfig {
            logical_quota: [1 << 30; 2],
            physical_capacity: 2 << 30,
            output_profile: dir.path().join("indexed.profile"),
            control_ipc_file: dir.path().join("indexed.ipc"),
            locator_directory: dir.path().join("locators"),
        };
        let mut rows = vec![
            OrderedMutation::put(
                "runtime/od1/profile-claim",
                codec::encode(
                    &a([u(1), u(4), b(owner.claim), b(owner.account), b(device)]),
                    1024,
                )?,
            ),
            OrderedMutation::put("device-secret.key", vec![9; 32]),
            OrderedMutation::put("device-encryption-secret.key", vec![10; 32]),
            OrderedMutation::put("device-certificate.cert", certificate.to_vec()),
            OrderedMutation::put("account-authority.snapshot", authority.clone()),
            OrderedMutation::put(
                format!(
                    "peer-authority/{}.snapshot",
                    super::super::super::source::hex(&peer)
                ),
                authority,
            ),
        ];
        let mut heads = vec![];
        let mut facts = vec![];
        for c in 0..contexts {
            let context = [20 + c as u8; 32];
            let credentials = |cert: &[u8]| codec::encode(&a([u(1), b(cert), b([0; 64])]), 1024);
            let spec = codec::encode(
                &a([
                    u(1),
                    b(context),
                    b(credentials(&certificate)?),
                    b(credentials(&peer_certificate)?),
                    u(0),
                    u(0),
                ]),
                4096,
            )?;
            let head = codec::encode(
                &a([
                    u(1),
                    b(context),
                    b(certs[0]),
                    u(0),
                    u(1),
                    u(u64::from(with_native)),
                    u(u64::from(with_native)),
                    opt(with_native.then(|| b([44; 32]))),
                    opt(None),
                    opt(None),
                    opt(None),
                    opt(None),
                    opt(native_hash.map(b)),
                ]),
                4096,
            )?;
            let p = prefix(context);
            rows.push(OrderedMutation::put(format!("{p}spec"), spec));
            rows.push(OrderedMutation::put(format!("{p}head"), head.clone()));
            if let Some(native) = &native {
                rows.push(OrderedMutation::put(
                    format!(
                        "ratchet/od1-contexts/{}/native",
                        super::super::super::source::hex(&context)
                    ),
                    native.clone(),
                ));
            }
            facts.push(super::super::super::MigrationContextFacts {
                context,
                role: 0,
                devices: [device, [4; 32]],
                certificates: certs,
                head_hash: *blake3::hash(&head).as_bytes(),
                native_hash,
                next: [0, 0],
                seen: vec![],
                ordinary_pending: 0,
                head_pending: None,
            });
            heads.push(head);
        }
        drop(OrderedVault::create(
            StateDirectoryLock::acquire(&state)?,
            rows,
        )?);
        let mut source = V4Source::open(
            StateDirectoryLock::acquire(&state)?,
            Config {
                physical_quota: config.physical_capacity,
                ..Config::default()
            },
        )?;
        let origin = super::super::super::MigrationOriginFacts {
            source_id: source.source_id(),
            account: owner.account,
            device,
            claim: owner.claim,
            contexts: facts,
        };
        source.preflight(&origin, &config, |_, _| Ok(None))?;
        let mut migration = MigrationRepository::fence(source, &origin, &config)?;
        while migration.copy_batch(&origin, |_, _| Ok(None))? > 0 {}
        migration.begin_verification(&origin)?;
        migration.ready(origin)?;
        assert!(matches!(
            migration.status()?.storage_stage,
            super::super::super::MigrationStage::Ready
        ));
        drop(migration);
        Ok(Self {
            _dir: dir,
            state,
            config,
            owner,
            heads,
            native,
        })
    }
    fn open(&self) -> Result<L1Repository> {
        L1Repository::open_ready(
            StateDirectoryLock::acquire(&self.state)?,
            self.owner,
            [3; 32],
            self.config.clone(),
        )
    }
}

#[test]
fn ae_actual_ready_strict_snapshot_requires_owner_initialization() -> TestResult {
    for contexts in [1, 2] {
        for native in [false, true] {
            let fixture = Fixture::ready(contexts, native)?;
            assert_eq!(fixture.native.is_some(), native);
            let mut repo = fixture.open()?;
            assert!(!repo.initialized()?);
            assert_eq!(repo.contexts().len(), contexts);
            for c in 0..contexts {
                assert_eq!(repo.head(c as u8)?, fixture.heads[c]);
                assert!(matches!(
                    repo.snapshot(c as u8),
                    Err(Error::Integrity("L1 owner not initialized"))
                ));
            }
        }
    }
    println!(
        "ae_actual_ready_baseline=4;strict_missing_metadata_refused=true;Owner7_not_constructed=true"
    );
    Ok(())
}

fn initialize(repo: &mut L1Repository) -> Result<()> {
    let accounts = vec![[[1; 32], [4; 32]]; repo.contexts().len()];
    let mut pins = vec![];
    for account in [[1; 32], [4; 32]] {
        pins.push(L1Authority {
            account,
            revision: 1,
            raw: repo.authority(account)?,
        });
    }
    repo.initialize(accounts, pins)?;
    Ok(())
}

// Same authenticated Work/tree/anchor mutation pattern as runtime2_tests.
// Deliberately invalid fresh records only; never a production writer bypass.
fn mutate(repo: &mut L1Repository, rows: Vec<(Key, Option<Vec<u8>>, usize)>) -> Result<()> {
    let tx = repo.core.vault.database.begin_write().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut work = Work::new(&nodes, &values, &repo.core.crypto, repo.core.root.clone());
    for (key, raw, cap) in rows {
        if let Some(raw) = raw {
            work.migration_meta_put(key, &raw, cap, 0)?;
        } else {
            work.remove(key)?;
        }
    }
    work.tree.seal_changes()?;
    let root = work.root.clone();
    let changed = std::mem::take(&mut work.changed);
    let changed_nodes = std::mem::take(&mut work.tree.changed);
    drop(work);
    drop(nodes);
    drop(values);
    {
        let mut table = tx.open_table(NODES).map_err(db)?;
        for (key, value) in changed_nodes {
            if let Some(value) = value {
                table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
            } else {
                table.remove(key.as_slice()).map_err(db)?;
            }
        }
    }
    {
        let mut table = tx.open_table(VALUES).map_err(db)?;
        for (key, value) in changed {
            if let Some(value) = value {
                table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
            } else {
                table.remove(key.as_slice()).map_err(db)?;
            }
        }
    }
    repo.core.write_anchor(&tx, &root)?;
    tx.commit().map_err(db)?;
    repo.core.root = root;
    Ok(())
}
fn head_fields(repo: &mut L1Repository, c: u8) -> Result<Vec<Value>> {
    let raw = repo
        .core
        .get(head_key(c)?)?
        .ok_or(Error::Integrity("fixture Head7"))?;
    Ok(codec::read_record(&raw, 10, 8192)?.fields(9)?.to_vec())
}
fn change_head(repo: &mut L1Repository, c: u8, fields: Vec<Value>) -> Result<()> {
    mutate(
        repo,
        vec![(
            head_key(c)?,
            Some(codec::record(10, &a(fields), 8192)?),
            8192,
        )],
    )
}
fn disk_hash(fixture: &Fixture) -> std::result::Result<Id, std::io::Error> {
    use std::io::Read;
    let mut file = std::fs::File::open(fixture.state.join(STATE_VAULT_FILE))?;
    let mut h = blake3::Hasher::new();
    let mut chunk = [0; 65536];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        h.update(&chunk[..n]);
    }
    Ok(*h.finalize().as_bytes())
}
fn writes(stats: BackendStats) -> (u64, u64, u64, u64, u64, u64) {
    (
        stats.writes,
        stats.write_bytes,
        stats.set_len_calls,
        stats.growth_bytes,
        stats.sync_calls,
        stats.length,
    )
}
#[test]
fn ae_cold_ready_reads_actual_bytes_without_persisted_change() -> TestResult {
    let mut cases = 0;
    let mut physical_open_close_changes = 0;
    for contexts in [1, 2] {
        for native in [false, true] {
            let fixture = Fixture::ready(contexts, native)?;
            let disk = disk_hash(&fixture)?;
            // redb opening/closing alone may change physical bookkeeping bytes.
            let control = fixture.open()?;
            let root = control.core.root.encode();
            drop(control);
            physical_open_close_changes += usize::from(disk_hash(&fixture)? != disk);
            let mut repo = fixture.open()?;
            assert!(repo.core.root.encode() == root);
            let before_writes = writes(repo.backend_stats()?);
            let source = repo.source_id();
            let generation = repo.generation();
            let commit = repo.last_commit()?;
            for c in 0..contexts {
                let actual = repo.cold_ready_snapshot(c as u8)?;
                assert!(actual.head == fixture.heads[c] && actual.native == fixture.native);
                assert!(matches!(
                    repo.snapshot(c as u8),
                    Err(Error::Integrity("L1 owner not initialized"))
                ));
            }
            assert!(!repo.initialized()? && repo.core.root.encode() == root);
            assert!(
                repo.generation() == generation
                    && repo.last_commit()? == commit
                    && repo.source_id() == source
            );
            assert!(writes(repo.backend_stats()?) == before_writes);
            drop(repo);
            let mut reopened = fixture.open()?;
            assert!(reopened.core.root.encode() == root && !reopened.initialized()?);
            let before_writes = writes(reopened.backend_stats()?);
            for c in 0..contexts {
                assert!(reopened.cold_ready_snapshot(c as u8)?.native == fixture.native);
            }
            assert!(writes(reopened.backend_stats()?) == before_writes);
            drop(reopened);
            cases += 1;
        }
    }
    println!(
        "ae_cold_ready_read_only_cases={cases};actual_migration=true;persisted_root_generation_commit_identity_unchanged=true;backend_writes_growth_sync_unchanged=true;absent_native_supported=true;redb_empty_open_close_physical_changes={physical_open_close_changes}"
    );
    Ok(())
}
#[test]
fn ae_cold_ready_rejects_partial_metadata_in_either_context() -> TestResult {
    let warm = Fixture::ready(2, true)?;
    let mut initialized = warm.open()?;
    initialize(&mut initialized)?;
    let mut metadata = vec![];
    for c in 0..2 {
        metadata.push(
            initialized
                .core
                .get(key(c, b"meta")?)?
                .ok_or(Error::Integrity("fixture initialized meta"))?,
        );
    }
    let mut cases = 0;
    for partial in 0..2 {
        let fixture = Fixture::ready(2, true)?;
        let mut repo = fixture.open()?;
        mutate(
            &mut repo,
            vec![(
                key(partial, b"meta")?,
                Some(metadata[partial as usize].clone()),
                META,
            )],
        )?;
        assert!(!repo.initialized()?);
        for requested in 0..2 {
            assert!(matches!(
                repo.cold_ready_snapshot(requested),
                Err(Error::Integrity("cold Ready partial initialization"))
            ));
            cases += 1;
        }
    }
    println!("ae_partial_metadata_refusals={cases};both_contexts_checked=true");
    Ok(())
}
#[test]
fn ae_warm_snapshot_stays_strict_and_keeps_selected_native_domain() -> TestResult {
    let fixture = Fixture::ready(2, true)?;
    let mut repo = fixture.open()?;
    initialize(&mut repo)?;
    for c in 0..2 {
        let snap = repo.snapshot(c)?;
        assert!(
            snap.head == fixture.heads[c as usize]
                && snap.native == fixture.native
                && snap.meta.next == [0, 0]
        );
        assert!(matches!(
            repo.cold_ready_snapshot(c),
            Err(Error::Integrity("cold Ready owner already initialized"))
        ));
    }
    let state = ExistingState7 {
        mode: ExistingMode::Active,
        selected_consent: Some([31; 32]),
        propose: Some([32; 32]),
        accept: Some([33; 32]),
        select: Some([34; 32]),
        committed: Some([35; 32]),
        q: Some([36; 32]),
        cuts: [Some(1), Some(1)],
        previous: [Some([37; 32]), Some([38; 32])],
        ..Default::default()
    };
    let native = vec![39; 72];
    let mut head = head_fields(&mut repo, 1)?;
    head[5] = opt(Some(b(hash(MIXED_NATIVE_ID, &native))));
    head[6] = u(state.mode.wire());
    head[7] = opt(state.q.map(b));
    head[8] = a(state.cuts.map(|n| opt(n.map(u))));
    mutate(
        &mut repo,
        vec![
            (key(1, b"existing")?, Some(packed(&state)?), 24 * 1024),
            (
                Key::hash(1, Namespace::Native, [0; 32])?,
                Some(native.clone()),
                128 * 1024,
            ),
            (head_key(1)?, Some(codec::record(10, &a(head), 8192)?), 8192),
        ],
    )?;
    assert!(repo.snapshot(1)?.native == Some(native));
    // Both metadata records removed with global still present: never cold fallback.
    mutate(
        &mut repo,
        vec![
            (key(0, b"meta")?, None, META),
            (key(1, b"meta")?, Some(b"bad metadata".to_vec()), META),
        ],
    )?;
    assert!(matches!(
        repo.cold_ready_snapshot(0),
        Err(Error::Integrity("cold Ready owner already initialized"))
    ));
    assert!(matches!(
        repo.snapshot(0),
        Err(Error::Integrity("L1 owner not initialized"))
    ));
    assert!(matches!(
        repo.snapshot(1),
        Err(Error::Integrity("L1 metadata envelope"))
    ));
    // No global/meta, but a structurally valid selected mode: still not cold L1.
    mutate(
        &mut repo,
        vec![
            (key(0, b"global")?, None, META),
            (key(1, b"meta")?, None, META),
        ],
    )?;
    assert!(matches!(
        repo.cold_ready_snapshot(0),
        Err(Error::UnsupportedRuntimeProfile)
    ));
    println!(
        "ae_warm_L1_and_Active_supported=true;global_present_missing_malformed_meta_refused=true;non_L1_other_context_refused=true"
    );
    Ok(())
}
#[test]
fn ae_shared_head_native_checks_reject_pointer_and_mode_in_either_context() -> TestResult {
    let mut cases = 0;
    for bad in 0..2 {
        for field in [5, 6, 7, 8] {
            let fixture = Fixture::ready(2, true)?;
            let mut repo = fixture.open()?;
            let mut head = head_fields(&mut repo, bad)?;
            head[field] = match field {
                5 => opt(Some(b([99; 32]))),
                6 => u(99),
                7 => opt(Some(b([99; 32]))),
                8 => a([opt(Some(u(2))), opt(None)]),
                _ => unreachable!(),
            };
            change_head(&mut repo, bad, head)?;
            if field == 5 {
                assert!(matches!(
                    repo.snapshot(bad),
                    Err(Error::Integrity("current Native pointer"))
                ));
                assert!(matches!(
                    repo.cold_ready_snapshot(1 - bad),
                    Err(Error::Integrity("current Native pointer"))
                ));
            } else {
                assert!(matches!(
                    repo.snapshot(bad),
                    Err(Error::UnsupportedRuntimeProfile)
                ));
                assert!(matches!(
                    repo.cold_ready_snapshot(1 - bad),
                    Err(Error::UnsupportedRuntimeProfile)
                ));
            }
            cases += 1;
        }
    }
    println!(
        "ae_shared_pointer_mode_q_cuts_negative_cases={cases};warm_and_cold_same_validation=true;other_context_checked=true"
    );
    Ok(())
}
#[test]
fn ae_cold_ready_checks_slot_stage_and_propagates_corrupt_reads() -> TestResult {
    let fixture = Fixture::ready(1, false)?;
    let mut repo = fixture.open()?;
    for c in [1, 2, 255] {
        assert!(matches!(
            repo.snapshot(c),
            Err(Error::Bound("unoccupied L1 slot"))
        ));
        assert!(matches!(
            repo.cold_ready_snapshot(c),
            Err(Error::Bound("unoccupied L1 slot"))
        ));
    }
    let key_progress = migration_key(repo.migration_id(), 2, b"")?;
    let raw = repo
        .core
        .get(key_progress)?
        .ok_or(Error::Integrity("fixture progress"))?;
    let mut progress = Progress::parse(&raw, repo.migration_id())?;
    progress.stage = super::super::super::MigrationStage::Copying;
    mutate(&mut repo, vec![(key_progress, Some(progress.raw()?), 8192)])?;
    assert!(matches!(
        repo.cold_ready_snapshot(0),
        Err(Error::UnsupportedRuntimeProfile)
    ));
    let fixture = Fixture::ready(2, true)?;
    let mut repo = fixture.open()?;
    mutate(
        &mut repo,
        vec![(key(1, b"existing")?, Some(b"malformed".to_vec()), 24 * 1024)],
    )?;
    assert!(matches!(
        repo.cold_ready_snapshot(0),
        Err(Error::Integrity("L1 metadata envelope"))
    ));
    let fixture = Fixture::ready(1, true)?;
    let mut repo = fixture.open()?;
    mutate(&mut repo, vec![(head_key(0)?, Some(vec![0]), 8192)])?;
    assert!(repo.snapshot(0).is_err() && repo.cold_ready_snapshot(0).is_err());
    let fixture = Fixture::ready(1, true)?;
    let mut repo = fixture.open()?;
    repo.core.poisoned = true;
    assert!(matches!(
        repo.cold_ready_snapshot(0),
        Err(Error::ReopenRequired)
    ));
    assert!(matches!(repo.snapshot(0), Err(Error::ReopenRequired)));
    println!("ae_invalid_slots=3;non_Ready_corrupt_existing_head_and_poisoned_reads_refused=true");
    Ok(())
}
