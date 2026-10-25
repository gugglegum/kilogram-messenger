//! Explicit charged real-d9b seed consumer; the runner supplies a new stopped seed.
use super::*;
use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId};
use kilogram_protocol::ordered_dialog_v1 as wire;
use kilogram_state::{
    StateDirectoryLock,
    indexed_dialog::{Config, migration7::V4Source},
};
use std::path::Path;
fn file_hash(path: &Path) -> Result<[u8; 32]> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0; 65536];
    let mut digest = blake3::Hasher::new();
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(*digest.finalize().as_bytes())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "fixture symlink");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
#[test]
fn migration_s5_real_d9b_source_native_fold_and_history() -> Result<()> {
    let seed = std::env::var_os("KILOGRAM_S5_FIXTURE_DIR")
        .context("run charged S5 feasibility first; KILOGRAM_S5_FIXTURE_DIR required")?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let base = workspace.join(".tmp/migration-s5-storage");
    std::fs::create_dir_all(&base)?;
    let branch = tempfile::Builder::new()
        .prefix("source-case-")
        .tempdir_in(&base)?;
    ensure!(
        branch
            .path()
            .canonicalize()?
            .starts_with(base.canonicalize()?),
        "branch cleanup stays in workspace"
    );
    for side in ["one", "two"] {
        let original = Path::new(&seed).join(format!("{side}.state"));
        let before = file_hash(&original.join("state-vault.redb"))?;
        let path = branch.path().join(format!("{side}.state"));
        copy_tree(&original, &path)?;
        let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
        let expected = match side {
            "one" => "82cbca9e0ab18b9fdff10615b8882d6a0711a8b126a42af900026da94b5e4b06",
            "two" => "05259d8a06cf838363ea8e663fd28cc8e157ea14b8795e0b24e8c315729dd80e",
            _ => unreachable!(),
        };
        ensure!(
            super::super::state::hex(&source.source_id()) == expected
                && source.entries().len() == 56,
            "stopped F-feasibility-01 seed exact authenticated provenance after power loss"
        );
        let claim = source.required(super::super::admin::CLAIM)?;
        let value = wire::decode(&claim, 1024)?;
        let f = value.fields(5)?;
        let account = AccountId::from_bytes(f[3].fixed()?);
        let device = DeviceId::from_bytes(f[4].fixed()?);
        let calls = kilogram_ratchet::ordered::native_call_counts();
        let verified = provenance::VerifiedSource::verify(&source, account, device)?;
        ensure!(
            verified.contexts.len() == 1,
            "small real seed has one context"
        );
        let c = verified.contexts.values().next().context("context")?;
        ensure!(
            c.head.phase == 3 && c.readable[0].len() == 3 && c.readable[1].len() == 3,
            "actual two Text/lane plus genesis"
        );
        ensure!(
            c.delivered.len() == 2 && c.receipt_refs.len() == 4 && c.imported.is_some(),
            "actual old receipt/Native/fold source"
        );
        ensure!(
            provenance::VerifiedSource::verify(&source, AccountId::from_bytes([99; 32]), device)
                .is_err(),
            "wrong-owner negative"
        );
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == calls,
            "zero migration Native calls"
        );
        println!(
            "case=M01.source side={side} source_id={} rows={} phase={} readable=3/3 receipts=4 zero_native_calls=true",
            super::super::state::hex(&source.source_id()),
            source.entries().len(),
            c.head.phase
        );
        drop(source);
        ensure!(
            before == file_hash(&original.join("state-vault.redb"))?,
            "frozen stopped seed not opened or mutated"
        );
    }
    Ok(())
}

#[test]
fn migration_s5_real_fence_copy_ready_reopen_and_capacity() -> Result<()> {
    use kilogram_state::indexed_dialog::migration7::{
        LegacyRead, MigrationConfig, MigrationProjection, MigrationRepository, MigrationStage,
    };
    let seed =
        std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged real d9b fixture required")?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let base = workspace.join(".tmp/migration-s5-storage");
    let branch = tempfile::Builder::new()
        .prefix("core-case-")
        .tempdir_in(&base)?;
    ensure!(
        branch
            .path()
            .canonicalize()?
            .starts_with(base.canonicalize()?),
        "owned branch cleanup scope"
    );
    for side in ["one", "two"] {
        let path = branch.path().join(format!("{side}.state"));
        copy_tree(&Path::new(&seed).join(format!("{side}.state")), &path)?;
        let backend = Config::default();
        let mut source = V4Source::open(StateDirectoryLock::acquire(&path)?, backend)?;
        let value = wire::decode(&source.required(super::super::admin::CLAIM)?, 1024)?;
        let cf = value.fields(5)?;
        let account = AccountId::from_bytes(cf[3].fixed()?);
        let device = DeviceId::from_bytes(cf[4].fixed()?);
        let calls = kilogram_ratchet::ordered::native_call_counts();
        let verified = provenance::VerifiedSource::verify(&source, account, device)?;
        let facts = verified.facts(&source)?;
        let mut config = MigrationConfig {
            logical_quota: backend.logical_quota,
            physical_capacity: backend.physical_quota,
            output_profile: branch.path().join(format!("{side}.profile")),
            control_ipc_file: branch.path().join(format!("{side}.ipc")),
            locator_directory: branch.path().join(format!("{side}.locators")),
        };
        let source_generation = source.generation();
        let source_id = source.source_id();
        let preview =
            source.preflight(&facts, &config, |entry, raw| derived(&verified, entry, raw))?;
        config.logical_quota = preview.logical_need;
        let preview =
            source.preflight(&facts, &config, |entry, raw| derived(&verified, entry, raw))?;
        let mut repo = MigrationRepository::fence(source, &facts, &config)?;
        let initial = repo.status()?;
        ensure!(
            initial.generation == source_generation + 1
                && initial.storage_stage == MigrationStage::Copying
                && initial.credits.repair_free == [0, 0],
            "atomic fence and unfunded pool"
        );
        while repo.copy_batch(&facts, |entry, raw| {
            if entry.kind != 12 {
                return Ok(None);
            }
            let ctx = verified
                .contexts
                .get(
                    &entry
                        .context
                        .ok_or(kilogram_state::indexed_dialog::Error::Integrity(
                            "projection context",
                        ))?,
                )
                .ok_or(kilogram_state::indexed_dialog::Error::Integrity(
                    "projection context",
                ))?;
            let id = super::super::state::parse_id(
                entry.path.rsplit('/').next().context("ID").map_err(|_| {
                    kilogram_state::indexed_dialog::Error::Integrity("projection ID")
                })?,
            )
            .map_err(|_| kilogram_state::indexed_dialog::Error::Integrity("projection ID"))?;
            let (text, request) = provenance::projection_raw(&verified.device, &ctx.head, id, raw)
                .map_err(|_| {
                    kilogram_state::indexed_dialog::Error::Integrity("actual original self-HPKE")
                })?;
            Ok(Some(MigrationProjection { text, request }))
        })? > 0
        {}
        repo.begin_verification(&facts)?;
        for ctx in verified.contexts.values() {
            for meta in ctx.readable.iter().flatten() {
                let projection = if meta.event.kind == 3 {
                    let (text, request) =
                        provenance::projection(&repo, &verified.device, &ctx.head, meta.id)?;
                    Some(MigrationProjection { text, request })
                } else {
                    None
                };
                repo.verify_readable_copy(
                    &facts,
                    ctx.spec.c,
                    meta.event.role,
                    meta.event.n,
                    projection.as_ref(),
                )?;
            }
        }
        let final_source = provenance::VerifiedSource::verify(&repo, account, device)?;
        let ready_facts = final_source.facts(&repo)?;
        repo.ready(ready_facts.clone())?;
        let ready = repo.status()?;
        ensure!(
            ready.storage_stage == MigrationStage::Ready
                && ready.credits.repair_free == [64 * 1024 * 1024, 0],
            "one Ready endowment, unused slot zero"
        );
        let actual = [0, 1].map(|c| {
            ready.credits.used[c]
                + ready.credits.reserved[c]
                + ready.credits.repair_free[c]
                + if ready.credits.head_in_use[c] {
                    0
                } else {
                    64 * 1024
                }
        });
        ensure!(
            actual == preview.final_need,
            "preflight recipe exactly matches final Root need: {actual:?} vs {:?}",
            preview.final_need
        );
        let mut mapper_entered = false;
        ensure!(
            repo.copy_batch(&facts, |_, _| {
                mapper_entered = true;
                Ok(None)
            })
            .is_err()
                && !mapper_entered,
            "closed lifecycle before mapper/retry"
        );
        repo.publish_profile(None)?;
        let old_profile = std::fs::read(&config.output_profile)?;
        repo.ready(ready_facts)?;
        ensure!(
            repo.status()?.credits.repair_free == ready.credits.repair_free,
            "Ready retry never funds twice"
        );
        let incarnation = repo.status()?.migration_id;
        drop(repo);
        let mut repo = MigrationRepository::open(
            StateDirectoryLock::acquire(&path)?,
            *account.as_bytes(),
            *device.as_bytes(),
            None,
        )?;
        ensure!(
            repo.source_id() == source_id && repo.status()?.migration_id == incarnation,
            "exact source/incarnation resume"
        );
        let s = repo.status()?;
        let hash = super::super::state::parse_id(&s.config_hash)?;
        let mut raised = config.clone();
        raised.logical_quota[0] += 1024 * 1024;
        repo.increase_capacity(s.generation, s.config_revision, hash, &raised)?;
        ensure!(
            repo.publish_profile(None).is_err(),
            "published profile needs explicit file-hash CAS"
        );
        repo.publish_profile(Some(*blake3::hash(&old_profile).as_bytes()))?;
        ensure!(
            repo.status()?.credits.repair_free == ready.credits.repair_free,
            "capacity CAS preserves funding"
        );
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == calls,
            "all migration steps zero packet crypto/constructors"
        );
        println!(
            "case=M01/M08.core side={side} fence/copy/Ready/reopen/capacity/profile-CAS PASS rows={} zero_native_calls=true",
            repo.entries().len()
        );
    }
    Ok(())
}

fn fixture_root() -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let seed =
        std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged new d9b seed required")?;
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp/migration-s5-storage")
        .canonicalize()?;
    Ok((Path::new(&seed).join("one.state"), base))
}
fn pins(
    source: &dyn kilogram_state::indexed_dialog::migration7::LegacyRead,
) -> Result<(AccountId, DeviceId)> {
    let v = wire::decode(&source.required(super::super::admin::CLAIM)?, 1024)?;
    let f = v.fields(5)?;
    Ok((
        AccountId::from_bytes(f[3].fixed()?),
        DeviceId::from_bytes(f[4].fixed()?),
    ))
}
fn config(base: &Path) -> kilogram_state::indexed_dialog::migration7::MigrationConfig {
    let default = Config::default();
    kilogram_state::indexed_dialog::migration7::MigrationConfig {
        logical_quota: default.logical_quota,
        physical_capacity: default.physical_quota,
        output_profile: base.join("indexed.profile"),
        control_ipc_file: base.join("indexed.ipc"),
        locator_directory: base.join("indexed.locators"),
    }
}
#[test]
fn migration_s5_archived_scope_substitutions_before_fence_and_resumed_ready() -> Result<()> {
    use super::super::{
        engine,
        handshake::{Descriptor, Spec},
    };
    use kilogram_identity::{AccountRootState, ConversationScopeId, DeviceState};
    use kilogram_state::{
        OrderedMutation, OrderedVault,
        indexed_dialog::migration7::{MigrationRepository, MigrationStage},
    };
    use wire::{a, b, u};
    let (seed, base) = fixture_root()?;
    let seed_base = seed.parent().context("seed parent")?;
    let branch = tempfile::Builder::new()
        .prefix("archived-scope-")
        .tempdir_in(&base)?;
    for side in ["one", "two"] {
        copy_tree(
            &seed_base.join(format!("{side}.state")),
            &branch.path().join(format!("{side}.state")),
        )?;
        copy_tree(
            &seed_base.join(format!("{side}.account")),
            &branch.path().join(format!("{side}.account")),
        )?;
    }
    let calls = kilogram_ratchet::ordered::native_call_counts();
    let peer_source = V4Source::open(
        StateDirectoryLock::acquire(branch.path().join("two.state"))?,
        Config::default(),
    )?;
    let peer_device = DeviceState::from_secret_material(
        peer_source.root(),
        peer_source
            .required("device-secret.key")?
            .as_slice()
            .try_into()?,
        peer_source
            .required("device-encryption-secret.key")?
            .as_slice()
            .try_into()?,
    );
    drop(peer_source);
    let path = branch.path().join("one.state");
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    let (account, device) = pins(&source)?;
    let good = provenance::VerifiedSource::verify(&source, account, device)?;
    let original_facts = good.facts(&source)?;
    let context = good.contexts.values().next().context("actual C1")?;
    let peer_role = 1 - context.head.role;
    let peer_path = format!("{}peer-locator", engine::prefix(context.spec.c));
    let descriptor = Descriptor::parse(&context.spec, peer_role, &source.required(&peer_path)?)?;
    let foreign_authority = source.required("account-authority.snapshot")?.to_vec();
    let authority_path = format!(
        "peer-authority/{}.snapshot",
        context.spec.peers[peer_role as usize].cert.account_id()
    );
    drop(source);
    let one_root = AccountRootState::load(branch.path().join("one.account"))?;
    let two_root = AccountRootState::load(branch.path().join("two.account"))?;
    let roots = if context.head.role == 0 {
        [&one_root, &two_root]
    } else {
        [&two_root, &one_root]
    };
    // A genuine second public Root ceremony and Device signature. No Native,
    // Head or Event is constructed/imported for C2, including in a positive origin.
    let c2 = super::super::admin::random::<32>()?;
    let refs = context.spec.cert_refs()?;
    let body = a([
        u(1),
        b(c2),
        b(refs[0]),
        b(refs[1]),
        u(1),
        u(1),
        u(1),
        u(3601),
    ]);
    let consent_bytes = wire::encode(&body, 4096)?;
    let spec2 = Spec {
        c: c2,
        peers: context.spec.peers.clone(),
        membership: roots[0]
            .create_conversation_membership(
                ConversationScopeId::from_bytes(c2),
                &[roots[1].account_id()],
            )?
            .encode()?,
        consent: wire::encode(
            &a([
                body,
                b(roots[0].sign_ordered_context_consent(&consent_bytes)),
                b(roots[1].sign_ordered_context_consent(&consent_bytes)),
            ]),
            4096,
        )?,
    };
    let spec2 = Spec::parse(&spec2.raw()?)?;
    let c2_descriptor = Descriptor::issue(
        &spec2,
        peer_role,
        descriptor.endpoint,
        descriptor.addresses.clone(),
        1,
        1,
        peer_device.identity(),
    )?;
    Descriptor::parse(&spec2, peer_role, &c2_descriptor.raw)?;
    for (label, target, replacement, expected) in [
        (
            "signed-C2-at-C1",
            peer_path,
            c2_descriptor.raw,
            "OD1 descriptor pins",
        ),
        (
            "signed-foreign-authority",
            authority_path,
            foreign_authority,
            "archived peer authority account/path/registered peer",
        ),
    ] {
        let case = branch.path().join(label);
        std::fs::create_dir(&case)?;
        let path = case.join("state");
        copy_tree(&seed, &path)?;
        let mut store = OrderedVault::open(StateDirectoryLock::acquire(&path)?)?;
        store.commit(
            store.generation(),
            vec![OrderedMutation::put(&target, replacement)],
        )?;
        drop(store);
        let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
        let bad_id = source.source_id();
        let generation = source.generation();
        drop(source);
        let cfg = config(&case);
        let error = upgrade(
            StateDirectoryLock::acquire(&path)?,
            account,
            device,
            &cfg,
            false,
            None,
        )
        .err()
        .context("invalid archived source must refuse before Fence")?;
        ensure!(
            format!("{error:#}").contains(expected),
            "exact static before-Fence refusal: {error:#}"
        );
        let mut source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
        ensure!(
            source.source_id() == bad_id && source.generation() == generation,
            "before-Fence source unchanged and no Root7"
        );
        ensure!(
            provenance::VerifiedSource::verify_history(&source, account, device, context.spec.c)
                .is_err(),
            "same scoped history static check"
        );
        // Model a checkpoint created by the old incomplete owner: only the
        // archived witness changed, actual Native/Head/Events/fold stay genuine.
        let mut old_facts = original_facts.clone();
        old_facts.source_id = bad_id;
        source.preflight(&old_facts, &cfg, |entry, raw| derived(&good, entry, raw))?;
        let mut repo = MigrationRepository::fence(source, &old_facts, &cfg)?;
        repo.pause()?;
        drop(repo);
        let error = upgrade(
            StateDirectoryLock::acquire(&path)?,
            account,
            device,
            &cfg,
            false,
            None,
        )
        .err()
        .context("invalid resumed source must refuse before Ready")?;
        ensure!(
            format!("{error:#}").contains(expected),
            "resumed static refusal: {error:#}"
        );
        let mut repo = MigrationRepository::open(
            StateDirectoryLock::acquire(&path)?,
            *account.as_bytes(),
            *device.as_bytes(),
            None,
        )?;
        let status = repo.status()?;
        ensure!(
            status.storage_stage == MigrationStage::Blocked
                && status.credits.repair_free == [0, 0]
                && status.reason.contains(expected)
                && !cfg.output_profile.exists(),
            "resumed source cannot reach Ready or receive endowment"
        );
        println!(
            "R2 {label} coherent_AEAD_index=true before_Fence_unchanged=true resumed_Blocked=true repair_free=0 zero_native_calls=true"
        );
    }
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == calls,
        "archival negatives do not call Native"
    );
    Ok(())
}

#[test]
fn migration_s5_expired_locator_and_revocation_remain_archivable() -> Result<()> {
    use super::super::{engine, handshake::Descriptor};
    use kilogram_identity::{
        AccountRootState, DeviceCapability, verify_device_authorization_with_snapshot,
    };
    use kilogram_state::{
        OrderedMutation, OrderedVault,
        indexed_dialog::migration7::{LegacyRead, MigrationRepository},
    };
    let (seed, base) = fixture_root()?;
    let branch = tempfile::Builder::new()
        .prefix("archived-positive-")
        .tempdir_in(&base)?;
    let path = branch.path().join("state");
    copy_tree(&seed, &path)?;
    copy_tree(
        &seed.parent().context("seed parent")?.join("two.account"),
        &branch.path().join("peer.account"),
    )?;
    let calls = kilogram_ratchet::ordered::native_call_counts();
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    let (account, device) = pins(&source)?;
    let good = provenance::VerifiedSource::verify(&source, account, device)?;
    let context = good.contexts.values().next().context("actual C")?;
    let target = format!("{}local-locator", engine::prefix(context.spec.c));
    let old = Descriptor::parse(&context.spec, context.head.role, &source.required(&target)?)?;
    let expired = Descriptor::issue(
        &context.spec,
        context.head.role,
        old.endpoint,
        old.addresses.clone(),
        old.revision + 1,
        1,
        good.device.identity(),
    )?;
    ensure!(
        expired.valid_now(3602).is_err(),
        "control is expired for live use"
    );
    Descriptor::parse(&context.spec, context.head.role, &expired.raw)?;
    let peer = &context.spec.peers[(1 - context.head.role) as usize].cert;
    let root = AccountRootState::load(branch.path().join("peer.account"))?;
    ensure!(
        root.account_id() == peer.account_id(),
        "copied peer Root pin"
    );
    root.revoke_device(peer.device_id())?;
    let revoked = root.authority_snapshot()?;
    ensure!(
        verify_device_authorization_with_snapshot(
            peer.account_id(),
            peer,
            &revoked,
            &DeviceCapability::MESSAGING
        )
        .is_err(),
        "revocation denies current messaging"
    );
    let authority_path = format!("peer-authority/{}.snapshot", peer.account_id());
    let authority = revoked.encode()?;
    drop(source);
    let mut store = OrderedVault::open(StateDirectoryLock::acquire(&path)?)?;
    store.commit(
        store.generation(),
        vec![
            OrderedMutation::put(&target, expired.raw.clone()),
            OrderedMutation::put(&authority_path, authority.clone()),
        ],
    )?;
    drop(store);
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    provenance::VerifiedSource::verify(&source, account, device)?;
    provenance::VerifiedSource::verify_history(&source, account, device, context.spec.c)?;
    drop(source);
    let output = upgrade(
        StateDirectoryLock::acquire(&path)?,
        account,
        device,
        &config(branch.path()),
        false,
        None,
    )?;
    ensure!(
        output["storage_stage"] == "Ready" && output["zero_migration_native_calls"] == true,
        "archival witness migration reaches Ready"
    );
    let repo = MigrationRepository::open(
        StateDirectoryLock::acquire(&path)?,
        *account.as_bytes(),
        *device.as_bytes(),
        None,
    )?;
    ensure!(
        repo.required(&target)?.as_slice() == expired.raw
            && repo.required(&authority_path)?.as_slice() == authority,
        "historical witnesses retained byte for byte"
    );
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == calls,
        "archival positive zero Native calls"
    );
    println!(
        "R2 correctly_signed_expired_locator=true scoped_revocation_current_denied=true static_archive_Ready=true exact_bytes_retained=true zero_native_calls=true"
    );
    Ok(())
}
#[test]
fn migration_s5_source_failures_and_quota_before_fence() -> Result<()> {
    use kilogram_state::indexed_dialog::{
        self as db,
        migration7::{LegacyEntry, LegacyRead, MigrationRepository},
    };
    use zeroize::Zeroizing;
    struct Missing<'a> {
        reader: &'a dyn LegacyRead,
        path: String,
    }
    impl LegacyRead for Missing<'_> {
        fn entries(&self) -> &[LegacyEntry] {
            self.reader.entries()
        }
        fn source_id(&self) -> db::Id {
            self.reader.source_id()
        }
        fn root(&self) -> &Path {
            self.reader.root()
        }
        fn get(&self, path: &str) -> db::Result<Option<Zeroizing<Vec<u8>>>> {
            if path == self.path {
                Ok(None)
            } else {
                self.reader.get(path)
            }
        }
    }
    let (seed, base) = fixture_root()?;
    let branch = tempfile::Builder::new()
        .prefix("negative-")
        .tempdir_in(&base)?;
    let path = branch.path().join("state");
    copy_tree(&seed, &path)?;
    // Stale unauthenticated shadows have no authority over the primary source.
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    let (account, device) = pins(&source)?;
    let original_id = source.source_id();
    let stale = source
        .entries()
        .iter()
        .find(|e| e.kind == 11)
        .context("Event1")?
        .path
        .clone();
    drop(source);
    std::fs::write(path.join(stale), b"stale shadow must not be adopted")?;
    let mut source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    ensure!(
        source.source_id() == original_id,
        "shadow is separately physical, never source authority"
    );
    let calls = kilogram_ratchet::ordered::native_call_counts();
    let verified = provenance::VerifiedSource::verify(&source, account, device)?;
    let context = verified.contexts.values().next().context("context")?;
    let event = context
        .readable
        .iter()
        .flatten()
        .find(|m| m.event.kind == 3)
        .context("Text")?;
    let raw = source.required(&super::super::engine::event_path(context.spec.c, event.id))?;
    let value = wire::decode(&raw, wire::MAX_EVENT)?;
    let original = value.fields(5)?[1].fields(14)?[13].fixed::<32>()?;
    let missing_o = Missing {
        reader: &source,
        path: super::super::admin::auth_path(original),
    };
    ensure!(
        provenance::VerifiedSource::verify(&missing_o, account, device).is_err(),
        "M11 mandatory O cannot become Unrecorded"
    );
    let receipt = source
        .entries()
        .iter()
        .find(|e| e.kind == 17)
        .context("receipt")?
        .path
        .clone();
    let missing_r = Missing {
        reader: &source,
        path: receipt,
    };
    ensure!(
        provenance::VerifiedSource::verify(&missing_r, account, device).is_err(),
        "M11 mandatory actual R cannot be invented"
    );
    let facts = verified.facts(&source)?;
    let mut cfg = config(branch.path());
    let mut bad = cfg.clone();
    bad.output_profile = branch.path().to_path_buf();
    ensure!(
        source
            .preflight(&facts, &bad, |entry, raw| derived(&verified, entry, raw))
            .is_err(),
        "directory cannot become a pinned profile file"
    );
    let mut bad = cfg.clone();
    bad.output_profile = path.join("not-public.profile");
    ensure!(
        source
            .preflight(&facts, &bad, |entry, raw| derived(&verified, entry, raw))
            .is_err(),
        "public output inside state refuses before Fence"
    );
    let preview = source.preflight(&facts, &cfg, |entry, raw| derived(&verified, entry, raw))?;
    cfg.logical_quota = preview.logical_need;
    cfg.logical_quota[0] -= 1;
    source.preflight(&facts, &cfg, |entry, raw| derived(&verified, entry, raw))?;
    ensure!(
        matches!(
            MigrationRepository::fence(source, &facts, &cfg),
            Err(db::Error::Quota)
        ),
        "need-1 must reject before Fence"
    );
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    ensure!(
        source.source_id() == original_id,
        "quota refusal left original V4 identity"
    );
    let target = source
        .entries()
        .iter()
        .find(|e| e.kind == 7)
        .context("Auth source")?
        .record_key;
    drop(source);
    // Real corruption of one primary Auth row, not a private Native fixture.
    {
        let mut builder = redb::Database::builder();
        builder.set_cache_size(64 * 1024 * 1024);
        let database = builder.open(path.join("state-vault.redb"))?;
        let tx = database.begin_write()?;
        tx.open_table(redb::TableDefinition::<&[u8], &[u8]>::new(
            "vault-records-v1",
        ))?
        .remove(target.as_slice())?;
        tx.commit()?;
    }
    ensure!(
        V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default()).is_err(),
        "primary Auth/index integrity corruption refuses before Fence"
    );
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == calls,
        "negative preflight zero Native calls"
    );
    println!(
        "M01/M08/M11 primary-vs-shadow, mandatory O/R, need-1-before-Fence, actual primary corruption PASS"
    );
    Ok(())
}
#[test]
fn migration_s5_io_poison_reopen_and_capacity_refusals() -> Result<()> {
    use kilogram_state::indexed_dialog::{self as db, migration7::MigrationRepository};
    let (seed, base) = fixture_root()?;
    for point in ["before_write", "after_write", "after_sync"] {
        let branch = tempfile::Builder::new().prefix("io-").tempdir_in(&base)?;
        let path = branch.path().join("state");
        copy_tree(&seed, &path)?;
        let mut source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
        let (account, device) = pins(&source)?;
        let verified = provenance::VerifiedSource::verify(&source, account, device)?;
        let facts = verified.facts(&source)?;
        let cfg = config(branch.path());
        source.preflight(&facts, &cfg, |entry, raw| derived(&verified, entry, raw))?;
        let mut repo = MigrationRepository::fence(source, &facts, &cfg)?;
        let before = repo.status()?;
        let before_commit = repo.last_commit()?;
        repo.verification_io_fault(point, 0)?;
        ensure!(
            repo.copy_batch(&facts, |entry, raw| derived(&verified, entry, raw))
                .is_err(),
            "injected IO must report ambiguity"
        );
        ensure!(
            matches!(repo.status(), Err(db::Error::ReopenRequired)),
            "IO poisoned the current handle"
        );
        drop(repo);
        let mut repo = MigrationRepository::open(
            StateDirectoryLock::acquire(&path)?,
            *account.as_bytes(),
            *device.as_bytes(),
            Some(&cfg),
        )?;
        let after = repo.status()?;
        ensure!(
            after.source_id == before.source_id
                && after.migration_id == before.migration_id
                && (after.generation == before.generation
                    || after.generation == before.generation + 1),
            "whole old/new atomic batch"
        );
        ensure!(
            (after.generation == before.generation) == (repo.last_commit()? == before_commit),
            "lastCommitId resolves durable outcome"
        );
        let mut lower = cfg.clone();
        lower.logical_quota[0] -= 1;
        let hash = super::super::state::parse_id(&after.config_hash)?;
        ensure!(
            repo.increase_capacity(after.generation, after.config_revision, hash, &lower)
                .is_err(),
            "lower quota CAS refusal"
        );
        let mut retarget = cfg.clone();
        retarget.output_profile = branch.path().join("retarget.profile");
        ensure!(
            repo.increase_capacity(after.generation, after.config_revision, hash, &retarget)
                .is_err(),
            "path retarget refusal"
        );
        ensure!(
            repo.increase_capacity(after.generation, after.config_revision + 1, hash, &cfg)
                .is_err(),
            "stale config revision refusal"
        );
        let mut raised = cfg.clone();
        raised.logical_quota[0] += 1024;
        repo.increase_capacity(after.generation, after.config_revision, hash, &raised)?;
        let funded = repo.status()?.credits.repair_free;
        ensure!(funded == [0, 0], "capacity before Ready does not fund pool");
        drop(repo);
        let output = upgrade(
            StateDirectoryLock::acquire(&path)?,
            account,
            device,
            &raised,
            false,
            None,
        )?;
        ensure!(
            output["storage_stage"] == "Ready" && output["zero_migration_native_calls"] == true,
            "resume after IO completes exactly once"
        );
        println!(
            "M08 point={point} old_generation={} reopened_generation={} old/new/poison/CAS/Ready PASS",
            before.generation, after.generation
        );
    }
    Ok(())
}

#[test]
fn migration_s5_fold_negatives_and_context_scoped_history() -> Result<()> {
    use kilogram_state::indexed_dialog::{
        self as db,
        migration7::{LegacyEntry, LegacyRead},
    };
    let (seed, base) = fixture_root()?;
    let branch = tempfile::Builder::new()
        .prefix("fold-scope-")
        .tempdir_in(&base)?;
    let path = branch.path().join("state");
    copy_tree(&seed, &path)?;
    let source = V4Source::open(StateDirectoryLock::acquire(&path)?, Config::default())?;
    let (account, device) = pins(&source)?;
    let verified = provenance::VerifiedSource::verify(&source, account, device)?;
    let context = verified.contexts.values().next().context("source C")?;
    let private = super::super::state::PrivateState::open(
        &context.head,
        verified.nonce,
        verified.device.encryption(),
        &source.required(&super::super::state::native_path(context.spec.c))?,
    )?;
    let guard = private.guard.as_ref().context("actual Guard1")?;
    let mut bad = context.readable.clone();
    let text = bad
        .iter_mut()
        .flatten()
        .find(|m| m.event.kind == 3)
        .context("Text")?;
    text.parents.push([99; 32]);
    ensure!(
        fold::fold(guard, &bad).is_err(),
        "unknown dependency is not readable membership"
    );
    let mut wrong = guard.clone();
    wrong.own.seen.clear();
    ensure!(
        fold::fold(&wrong, &context.readable).is_err(),
        "same-looking summary with different seen set"
    );
    let mut bad = context.readable.clone();
    bad[0][0].event.n = 1;
    ensure!(
        fold::fold(guard, &bad).is_err(),
        "non-bijective genesis lane"
    );
    // Unit scope sentinel: the real C1 source is supplemented with metadata for a
    // second context whose body must never be requested by the scoped decoder.
    struct Scope<'a> {
        reader: &'a V4Source,
        entries: Vec<LegacyEntry>,
        forbidden: String,
    }
    impl LegacyRead for Scope<'_> {
        fn entries(&self) -> &[LegacyEntry] {
            &self.entries
        }
        fn source_id(&self) -> db::Id {
            self.reader.source_id()
        }
        fn root(&self) -> &Path {
            self.reader.root()
        }
        fn get(&self, path: &str) -> db::Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
            if path == self.forbidden {
                return Err(db::Error::Integrity("C2 body was requested"));
            }
            self.reader.get(path)
        }
    }
    let mut entries = source.entries().to_vec();
    let mut sentinel = entries
        .iter()
        .find(|e| e.kind == 8)
        .context("spec")?
        .clone();
    sentinel.context = Some([98; 32]);
    sentinel.path = format!(
        "runtime/od1/contexts/{}/spec",
        super::super::state::hex(&[98; 32])
    );
    let forbidden = sentinel.path.clone();
    entries.push(sentinel);
    let scoped = Scope {
        reader: &source,
        entries,
        forbidden,
    };
    let read =
        provenance::VerifiedSource::verify_history(&scoped, account, device, context.spec.c)?;
    ensure!(
        read.contexts.len() == 1 && read.contexts.contains_key(&context.spec.c),
        "C1 scoped history never reads C2 body"
    );
    ensure!(
        provenance::VerifiedSource::verify_history(&source, account, device, [97; 32]).is_err(),
        "unknown-C history refusal"
    );
    println!("M01/M10 fold dependency/seen/genesis negatives and C1-only history decoder PASS");
    Ok(())
}

#[test]
fn migration_s5_r4_old_engine_genesis_role_forks_remain_archivable() -> Result<()> {
    use super::super::{
        admin,
        engine::{self, Engine, Lease},
    };
    use kilogram_state::{
        OrderedMutation, OrderedVault,
        indexed_dialog::migration7::{LegacyRead, MigrationRepository, MigrationStage},
    };
    use std::collections::BTreeMap;
    use wire::{Event, a, b};
    let (seed, base) = fixture_root()?;
    let seed_base = seed.parent().context("seed parent")?;
    let seed_hashes = ["one", "two"]
        .map(|side| file_hash(&seed_base.join(format!("{side}.state/state-vault.redb"))))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let branch = tempfile::Builder::new()
        .prefix("fork-r4-")
        .tempdir_in(&base)?;
    let calls = kilogram_ratchet::ordered::native_call_counts();
    for (reason, role) in [("Init", 0), ("Confirm", 1)] {
        let case = branch.path().join(reason);
        std::fs::create_dir(&case)?;
        for side in ["one", "two"] {
            copy_tree(
                &seed_base.join(format!("{side}.state")),
                &case.join(format!("{side}.state")),
            )?;
        }
        let one = OrderedVault::open(StateDirectoryLock::acquire(case.join("one.state"))?)?;
        let spec = admin::specs(&one)?
            .into_iter()
            .next()
            .context("actual seed context")?;
        let one_role = spec.role(admin::device(&one)?.identity().device_id())?;
        drop(one);
        let (receiver, sender, expected_id) = if one_role == 1 - role {
            (
                case.join("one.state"),
                case.join("two.state"),
                "82cbca9e0ab18b9fdff10615b8882d6a0711a8b126a42af900026da94b5e4b06",
            )
        } else {
            (
                case.join("two.state"),
                case.join("one.state"),
                "05259d8a06cf838363ea8e663fd28cc8e157ea14b8795e0b24e8c315729dd80e",
            )
        };
        let sender_store = OrderedVault::open(StateDirectoryLock::acquire(&sender)?)?;
        let signer = admin::device(&sender_store)?;
        ensure!(
            signer.identity().device_id() == spec.peers[role as usize].cert.device_id(),
            "real source peer signer"
        );
        drop(sender_store);
        let source = V4Source::open(StateDirectoryLock::acquire(&receiver)?, Config::default())?;
        ensure!(
            state::hex(&source.source_id()) == expected_id,
            "exact stopped seed provenance"
        );
        let (account, device) = pins(&source)?;
        let before = provenance::VerifiedSource::verify(&source, account, device)?;
        let context = before.contexts.get(&spec.c).context("genuine source C")?;
        ensure!(
            context.head.role == 1 - role
                && context.head.phase == 3
                && context.readable.iter().all(|lane| lane.len() == 3),
            "actual complete readable origin"
        );
        let old_head = context.head.clone();
        let native = source.required(&state::native_path(spec.c))?;
        let original_entries = source.entries().to_vec();
        let mut expected_history = BTreeMap::new();
        for meta in context
            .readable
            .iter()
            .flatten()
            .filter(|m| m.event.kind == 3)
        {
            let (text, request) =
                provenance::projection(&source, &before.device, &context.head, meta.id)?;
            let id = state::hex(&meta.id);
            expected_history.insert(id.clone(), serde_json::json!({"event_id":id,"role":meta.event.role,"ordinal":meta.event.n,"sequence":meta.event.sequence,"request_id":state::hex(&request),"text":text,"delivered":context.delivered.contains(&meta.id)}));
        }
        let old_id: [u8; 32] = source
            .required(&engine::ordinal_path(spec.c, role, 0))?
            .as_slice()
            .try_into()?;
        let old = source.required(&engine::event_path(spec.c, old_id))?;
        let mut conflict = Event::parse(&old, signer.identity().device_id())?;
        conflict.core.role = 1 - role;
        conflict.sign(signer.identity())?;
        let conflict_raw = conflict.raw()?;
        let conflict_id = conflict.id()?;
        let original = Event::parse(&old, signer.identity().device_id())?;
        ensure!(
            conflict.core.role != role
                && conflict_id != old_id
                && conflict.native_kind == original.native_kind
                && conflict.key == original.key
                && conflict.index == original.index
                && conflict.ciphertext == original.ciphertext,
            "only conflicting signed outer role; original packet untouched"
        );
        drop(source);
        let mut owner = Engine::new(OrderedVault::open(StateDirectoryLock::acquire(&receiver)?)?)?;
        owner.policy(spec.c)?;
        let binding = admin::random()?;
        owner.leases.insert(
            spec.c,
            Lease {
                binding,
                generation: 1,
                deadline: engine::now()? + 300,
            },
        );
        // Exercise the unchanged production capture method under its real current
        // policy check and a test admission lease; no handshake/Native is created.
        let captured = if role == 0 {
            owner.confirm(spec.c, &conflict_raw, binding)
        } else {
            owner.receive_confirm(spec.c, &conflict_raw, binding)
        };
        let error = captured
            .err()
            .context("old writer must capture signed fork")?;
        ensure!(
            format!("{error:#}").contains(&format!("Blocked: signed {reason} fork")),
            "actual Engine capture: {error:#}"
        );
        let fork_path = format!("{}blocked-fork", engine::prefix(spec.c));
        let witness = admin::required(&owner.store, &fork_path)?;
        let envelope = wire::decode(&witness, 68 * 1024)?;
        let fields = envelope.fields(4)?;
        ensure!(
            fields[0].uint()? == 1
                && fields[1].bytes()? == reason.as_bytes()
                && fields[2].bytes()? == old.as_slice()
                && fields[3].bytes()? == conflict_raw,
            "writer persisted exact old/new fork witness"
        );
        ensure!(
            owner.store.get(&state::native_path(spec.c))?.as_deref() == Some(native.as_slice())
                && owner
                    .event_at(spec.c, role, 0)?
                    .context("saved genesis")?
                    .id()?
                    == old_id
                && owner.policy(spec.c).is_err(),
            "capture preserves actual Native/readable and blocks context"
        );
        let mut expected_head = old_head.clone();
        expected_head.bump(false)?;
        ensure!(
            owner.head(spec.c)?.encode()? == expected_head.encode()?,
            "only public Head generation changes at fork capture"
        );
        drop(owner);
        let source = V4Source::open(StateDirectoryLock::acquire(&receiver)?, Config::default())?;
        for entry in original_entries.iter().filter(|e| e.kind != 9) {
            let after = source
                .entries()
                .iter()
                .find(|e| e.path == entry.path)
                .context("original source entry")?;
            ensure!(
                after.cipher_hash == entry.cipher_hash
                    && after.content_hash == entry.content_hash
                    && after.length == entry.length,
                "old primary row unchanged: {}",
                entry.path
            );
        }
        let accepted = provenance::VerifiedSource::verify(&source, account, device)?;
        ensure!(
            accepted.contexts[&spec.c]
                .readable
                .iter()
                .all(|lane| lane.len() == 3)
                && accepted.contexts[&spec.c]
                    .readable
                    .iter()
                    .flatten()
                    .all(|meta| meta.id != conflict_id),
            "conflict is not a readable origin"
        );
        provenance::VerifiedSource::verify_history(&source, account, device, spec.c)?;
        drop(source);
        // Fork witnesses are append-only. On fresh genuine-source copies, the
        // ordinary primary writer adds only a corrupted witness for negatives.
        let negative_seed = seed_base.join(receiver.file_name().context("receiver seed name")?);
        for bad_kind in ["signature", "context"] {
            let bad_path = case.join(format!("bad-{bad_kind}.state"));
            copy_tree(&negative_seed, &bad_path)?;
            let mut bad = conflict.clone();
            if bad_kind == "signature" {
                bad.signature[0] ^= 1;
            } else {
                bad.core.c = [88; 32];
                bad.sign(signer.identity())?;
                Event::parse(&bad.raw()?, signer.identity().device_id())?;
            }
            let mut bad_fields = fields.to_vec();
            bad_fields[3] = b(bad.raw()?);
            let mut store = OrderedVault::open(StateDirectoryLock::acquire(&bad_path)?)?;
            store.commit(
                store.generation(),
                vec![OrderedMutation::put(
                    &fork_path,
                    wire::encode(&a(bad_fields), 68 * 1024)?,
                )],
            )?;
            drop(store);
            let bad_source =
                V4Source::open(StateDirectoryLock::acquire(&bad_path)?, Config::default())?;
            ensure!(
                provenance::VerifiedSource::verify(&bad_source, account, device).is_err()
                    && provenance::VerifiedSource::verify_history(
                        &bad_source,
                        account,
                        device,
                        spec.c
                    )
                    .is_err(),
                "{reason} signed-role allowance must reject bad {bad_kind} witness"
            );
        }
        let cfg = config(&case);
        let result = upgrade(
            StateDirectoryLock::acquire(&receiver)?,
            account,
            device,
            &cfg,
            false,
            None,
        )?;
        ensure!(
            result["storage_stage"] == "Ready",
            "blocked source is archivable"
        );
        let history = history(
            StateDirectoryLock::acquire(&receiver)?,
            account,
            device,
            spec.c,
            0,
            8,
        )?;
        let actual_history = history["items"]
            .as_array()
            .context("history items")?
            .iter()
            .map(|item| {
                Ok((
                    item["event_id"].as_str().context("history ID")?.to_owned(),
                    item.clone(),
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        ensure!(
            actual_history == expected_history && history["total"] == 4,
            "actual readable IDs/requests/text/status unchanged through migration"
        );
        for _ in 0..2 {
            let mut repo = MigrationRepository::open(
                StateDirectoryLock::acquire(&receiver)?,
                *account.as_bytes(),
                *device.as_bytes(),
                None,
            )?;
            ensure!(
                repo.status()?.storage_stage == MigrationStage::Ready
                    && repo.required(&fork_path)?.as_slice() == witness
                    && repo.required(&state::native_path(spec.c))?.as_slice() == native.as_slice()
                    && repo
                        .required(&engine::event_path(spec.c, old_id))?
                        .as_slice()
                        == old.as_slice()
                    && repo
                        .get(&engine::event_path(spec.c, conflict_id))?
                        .is_none(),
                "reopen retains block/evidence/native and never promotes fork to readable"
            );
            provenance::VerifiedSource::verify_history(&repo, account, device, spec.c)?;
        }
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == calls,
            "Engine fork capture/migration/history/reopen zero Native calls"
        );
        println!(
            "R4 reason={reason} actual_Engine_capture=true signed_role_conflict=true old_rows_cipher_unchanged=true Head_native_unchanged=true policy_blocked=true Ready_retains_exact_block_evidence=true readable=3/3 history_texts=4 reopen=2 bad_signature/context_rejected=true zero_native_calls=true"
        );
    }
    for (index, side) in ["one", "two"].iter().enumerate() {
        ensure!(
            file_hash(&seed_base.join(format!("{side}.state/state-vault.redb")))?
                == seed_hashes[index],
            "original stopped seed never opened or changed"
        );
    }
    Ok(())
}
