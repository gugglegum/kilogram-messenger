//! Storage/purpose tests use opaque enrollment bytes; not signature/MAC evidence.
use super::*;

fn fixture() -> Result<(tempfile::TempDir, RuntimeRepository, ExpectedOwner, Config)> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/a3-s4/state");
    std::fs::create_dir_all(&base)?;
    let dir = tempfile::Builder::new()
        .prefix("runtime-")
        .tempdir_in(base)?;
    let owner = ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    };
    let config = Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Config::default()
    };
    let repository = RuntimeRepository::create(
        StateDirectoryLock::acquire(dir.path())?,
        CreationClaim {
            account: owner.account,
            claim: owner.claim,
            signing_secret: Zeroizing::new([3; 32]),
            encryption_secret: Zeroizing::new([4; 32]),
            contexts: [[5; 32], [6; 32]],
        },
        RuntimeEnrollment {
            device: [7; 32],
            certificate: vec![8; 32],
            authority: vec![9; 32],
            authority_revision: 1,
        },
        config,
    )?;
    Ok((dir, repository, owner, config))
}
fn spec(profile: &RuntimeProfile) -> RuntimeContextSpec {
    RuntimeContextSpec {
        role: 0,
        accounts: [profile.account, [10; 32]],
        certs: [
            *blake3::hash(&profile.enrollment.certificate).as_bytes(),
            *blake3::hash(&[16; 32]).as_bytes(),
        ],
        credentials: [
            credential(
                &profile.enrollment.certificate,
                &profile.enrollment.authority,
            ),
            credential(&[16; 32], &[17; 32]),
        ],
        membership: vec![14],
        consent: vec![15],
        own_locator: None,
        peer_locator: None,
        own_locator_revision: 0,
        peer_locator_revision: 0,
    }
}
fn credential(cert: &[u8], authority: &[u8]) -> Vec<u8> {
    let mut value = vec![0x83, 1, 0x58, cert.len() as u8];
    value.extend(cert);
    value.extend([0x58, authority.len() as u8]);
    value.extend(authority);
    value
}
#[test]
fn a3_s4_authenticated_root_layout_and_component_fences() -> Result<()> {
    let (dir, mut repo, owner, config) = fixture()?;
    let encoded = repo.store.root.encode();
    assert_eq!(&encoded[..11], b"\x06KGOD2RT\0\x00\x01");
    let mut future = encoded.to_vec();
    future[10] = 2;
    assert!(Root::decode(&future).is_err());
    let mut extra = encoded.to_vec();
    extra.push(0);
    assert!(Root::decode(&extra).is_err());
    let mut magic = encoded.to_vec();
    magic[1] ^= 1;
    assert!(Root::decode(&magic).is_err());
    let before = repo.store.root.encode();
    assert!(matches!(
        repo.store.commit(1, [20; 32], vec![], vec![]),
        Err(Error::RuntimeWriterRequired)
    ));
    assert!(matches!(
        repo.store.commit_history(
            0,
            1,
            [20; 32],
            HistoryOperation::QueueAccept {
                request: [1; 16],
                text: "blocked".into()
            }
        ),
        Err(Error::RuntimeWriterRequired)
    ));
    assert!(matches!(
        repo.store.commit_history_protocol2(
            0,
            1,
            [20; 32],
            HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: vec![],
                auth: vec![]
            }
        ),
        Err(Error::RuntimeWriterRequired)
    ));
    assert_eq!(before, repo.store.root.encode());
    drop(repo);
    assert!(matches!(
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner, config),
        Err(Error::UnsupportedRuntimeProfile)
    ));
    let mut reopened =
        RuntimeRepository::open(StateDirectoryLock::acquire(dir.path())?, owner, config)?;
    assert_eq!(reopened.profile()?.contexts, [[5; 32], [6; 32]]);
    assert_eq!(*reopened.owner_material()?.signing, [3; 32]);
    Ok(())
}
#[test]
fn a3_s4_post_staging_gate_abort_preserves_root_and_reuses_repository() -> Result<()> {
    let (dir, mut repo, owner, config) = fixture()?;
    let profile = repo.profile()?;
    let s = spec(&profile);
    let operation = RuntimeOperation::InstallContext {
        context: 0,
        spec: s,
        peer_revision: 1,
    };
    let before = repo.store.root.encode();
    let credit = repo.credits()?;
    let mut calls = 0;
    let rejected = repo.commit(1, [21; 32], operation.clone(), &mut || {
        calls += 1;
        Err(Error::PolicyRejected("test lease closed after staging"))
    });
    assert!(matches!(rejected, Err(Error::PolicyRejected(_))));
    assert_eq!(calls, 1);
    assert_eq!(repo.store.root.encode(), before);
    assert_eq!(repo.credits()?.used, credit.used);
    assert!(!repo.store.poisoned);
    assert!(repo.store.get(key(0, b"spec")?)?.is_none());
    let report = repo.commit(1, [21; 32], operation.clone(), &mut || Ok(()))?;
    assert_eq!(report.generation, 2);
    let duplicate = repo.commit(1, [21; 32], operation, &mut || {
        Err(Error::Integrity("duplicate staged"))
    })?;
    assert!(duplicate.duplicate);
    assert_eq!(repo.snapshot(0)?.head.phase, 0);
    drop(repo);
    let mut reopened =
        RuntimeRepository::open(StateDirectoryLock::acquire(dir.path())?, owner, config)?;
    assert_eq!(reopened.generation(), 2);
    assert_eq!(reopened.snapshot(0)?.head.context, [5; 32]);
    Ok(())
}
#[test]
fn a3_s4_metadata_growth_consumes_only_its_prepaid_capacity() -> Result<()> {
    let (_dir, mut repo, _owner, _config) = fixture()?;
    let before = repo.credits()?;
    repo.commit(
        repo.generation(),
        [31; 32],
        RuntimeOperation::PinAuthority {
            account: [1; 32],
            revision: 2,
            raw: vec![21; 12 * 1024],
            expected_policy: 1,
        },
        &mut || Ok(()),
    )?;
    let after = repo.credits()?;
    assert!(after.used[0] > before.used[0]);
    assert!(after.reserved[0] < before.reserved[0]);
    for c in 0..2 {
        assert_eq!(
            before.used[c] + before.reserved[c],
            after.used[c] + after.reserved[c]
        );
    }
    let tx = repo.store.vault.database.begin_read().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut w = Work::new(&nodes, &values, &repo.store.crypto, repo.store.root.clone());
    let profile = w.runtime_profile()?;
    assert!(matches!(
        w.runtime_put_key(profile_key()?, &profile, PROFILE_BYTES + 1024),
        Err(Error::Integrity("runtime metadata prepaid capacity"))
    ));
    Ok(())
}
fn anchor_fixture(repo: &RuntimeRepository, plain: &[u8]) -> Result<()> {
    let mut raw = repo.store.root.incarnation.to_vec();
    raw.extend(seal(
        &repo.store.crypto.value,
        &root_aad(repo.store.root.incarnation),
        plain,
    )?);
    let sentinel = postcard::to_allocvec(&VaultManifest {
        schema_version: 5,
        record_count: 0,
        plaintext_bytes: 0,
        snapshot_id: *blake3::hash(&raw).as_bytes(),
    })
    .map_err(|_| Error::Bound("negative fixture sentinel"))?;
    let tx = repo.store.vault.database.begin_write().map_err(db)?;
    {
        let mut table = tx.open_table(META_TABLE).map_err(db)?;
        table.insert(ANCHOR, raw.as_slice()).map_err(db)?;
        table
            .insert(MANIFEST_KEY, sentinel.as_slice())
            .map_err(db)?;
    }
    tx.commit().map_err(db)
}
#[test]
fn a3_s4_f10_future_layout_and_missing_marker_cold_open_refused() -> Result<()> {
    for case in 0..6 {
        let (dir, repo, owner, config) = fixture()?;
        let mut root = repo.store.root.clone();
        let mut plain = root.encode().to_vec();
        match case {
            0 => plain[0] = 7,
            1 => plain[1] ^= 1,
            2 => plain[10] = 2,
            3 => plain.push(0),
            4 => {
                root.roots[22] = Summary::default();
                plain = root.encode().to_vec();
            }
            5 => {
                root.layout = RootLayout::Component5;
                plain = root.encode().to_vec();
            }
            _ => unreachable!(),
        }
        anchor_fixture(&repo, &plain)?;
        drop(repo);
        assert!(matches!(
            RuntimeRepository::open(StateDirectoryLock::acquire(dir.path())?, owner, config),
            Err(Error::Integrity(_))
        ));
        println!(
            "a3_s4_F10_cold_case={case} authenticated_input=true refused_before_runtime_handle=true"
        );
    }
    Ok(())
}
#[test]
fn a3_s4_f10_marker_binding_mismatch_cold_open_refused() -> Result<()> {
    let (dir, mut repo, owner, config) = fixture()?;
    let mut profile = repo.profile()?;
    profile.claim[0] ^= 1;
    let tx = repo.store.vault.database.begin_write().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut work = Work::new(&nodes, &values, &repo.store.crypto, repo.store.root.clone());
    work.runtime_put_key(profile_key()?, &profile, PROFILE_BYTES)?;
    work.tree.seal_changes()?;
    let root = work.root.clone();
    let changed = std::mem::take(&mut work.changed);
    let changed_nodes = std::mem::take(&mut work.tree.changed);
    drop(work);
    drop(nodes);
    drop(values);
    {
        let mut nodes = tx.open_table(NODES).map_err(db)?;
        for (key, value) in changed_nodes {
            match value {
                Some(value) => {
                    nodes.insert(key.as_slice(), value.as_slice()).map_err(db)?;
                }
                None => {
                    nodes.remove(key.as_slice()).map_err(db)?;
                }
            }
        }
    }
    {
        let mut values = tx.open_table(VALUES).map_err(db)?;
        for (key, value) in changed {
            match value {
                Some(value) => {
                    values
                        .insert(key.as_slice(), value.as_slice())
                        .map_err(db)?;
                }
                None => {
                    values.remove(key.as_slice()).map_err(db)?;
                }
            }
        }
    }
    repo.store.write_anchor(&tx, &root)?;
    tx.commit().map_err(db)?;
    drop(repo);
    assert!(matches!(
        RuntimeRepository::open(StateDirectoryLock::acquire(dir.path())?, owner, config),
        Err(Error::Integrity("runtime marker binding"))
    ));
    Ok(())
}
#[test]
fn a3_s4_f10_internal_component_purpose_retry_cannot_bypass_runtime() -> Result<()> {
    let (_dir, mut repo, _owner, _config) = fixture()?;
    repo.commit(
        repo.generation(),
        [61; 32],
        RuntimeOperation::PinAuthority {
            account: [1; 32],
            revision: 2,
            raw: vec![62; 32],
            expected_policy: 1,
        },
        &mut || Ok(()),
    )?;
    let before = repo.store.root.encode();
    let id = repo.last_commit()?;
    let digest = repo.store.root.request_digest;
    let inputs = [
        CommitInput::Generic {
            mutations: vec![],
            credits: vec![],
        },
        CommitInput::History {
            context: 0,
            operation: Box::new(HistoryOperation::QueueAccept {
                request: [1; 16],
                text: "forbidden".into(),
            }),
        },
        CommitInput::Protocol2 {
            context: 0,
            operation: Box::new(HistoryProtocol2Operation::Acknowledge {
                receipt_format: 2,
                receipt_raw: vec![],
                auth: vec![],
            }),
        },
    ];
    for input in inputs {
        assert!(matches!(
            repo.store
                .commit_with_gate(repo.generation(), id, digest, input, 0, None),
            Err(Error::RuntimeWriterRequired)
        ));
        assert_eq!(before, repo.store.root.encode());
    }
    Ok(())
}
#[test]
fn a3_s4_f01_stale_captured_policy_is_a_closed_writer_refusal() -> Result<()> {
    let (_dir, mut repo, _owner, _config) = fixture()?;
    let spec = spec(&repo.profile()?);
    repo.commit(
        repo.generation(),
        [41; 32],
        RuntimeOperation::InstallContext {
            context: 0,
            spec,
            peer_revision: 1,
        },
        &mut || Ok(()),
    )?;
    let snapshot = repo.snapshot(0)?;
    let mut fence = snapshot.fence(1);
    fence.policy_generation -= 1;
    let before = repo.store.root.encode();
    let mut staged = false;
    assert!(matches!(
        repo.commit(
            repo.generation(),
            [42; 32],
            RuntimeOperation::Queue {
                context: 0,
                fence,
                request: [43; 16],
                text: "stale capture".into()
            },
            &mut || {
                staged = true;
                Ok(())
            }
        ),
        Err(Error::StaleGeneration)
    ));
    assert!(!staged);
    assert_eq!(repo.store.root.encode(), before);
    assert!(!repo.requires_reopen());
    Ok(())
}
#[test]
fn a3_s4_reopen_retains_directory_lock_on_success_and_failure() -> Result<()> {
    let (dir, repo, _owner, _config) = fixture()?;
    assert!(StateDirectoryLock::acquire(dir.path()).is_err());
    let mut repo = repo.reopen().map_err(|f| f.error)?;
    assert!(StateDirectoryLock::acquire(dir.path()).is_err());
    repo.config.physical_quota = HEADROOM;
    let failure = match repo.reopen() {
        Ok(_) => return Err(Error::Integrity("capacity did not prevent reopen")),
        Err(failure) => failure,
    };
    assert!(matches!(failure.error, Error::ConfiguredCapacity { .. }));
    assert!(StateDirectoryLock::acquire(dir.path()).is_err());
    drop(failure);
    let _lock = StateDirectoryLock::acquire(dir.path())?;
    Ok(())
}
