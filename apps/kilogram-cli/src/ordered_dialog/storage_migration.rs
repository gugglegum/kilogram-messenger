//! Offline migration owner. Cryptographic source validation is separate from state lowering.
#[path = "storage_migration/fold.rs"]
mod fold;
#[path = "storage_migration/provenance.rs"]
mod provenance;
use super::{admin, state};
use anyhow::{Context, Result, bail, ensure};
use kilogram_identity::{AccountId, DeviceId};
use kilogram_state::{
    StateDirectoryLock,
    indexed_dialog::{
        self as db,
        migration7::{
            LegacyEntry, MigrationConfig, MigrationInput, MigrationProjection, MigrationRepository,
            MigrationStage,
        },
    },
};
use std::path::Path;

fn derived(
    verified: &provenance::VerifiedSource,
    entry: &LegacyEntry,
    raw: &[u8],
) -> db::Result<Option<MigrationProjection>> {
    if entry.kind != 12 {
        return Ok(None);
    }
    let c = entry
        .context
        .ok_or(db::Error::Integrity("projection context"))?;
    let context = verified
        .contexts
        .get(&c)
        .ok_or(db::Error::Integrity("projection context"))?;
    let id = state::parse_id(
        entry
            .path
            .rsplit('/')
            .next()
            .ok_or(db::Error::Integrity("projection ID"))?,
    )
    .map_err(|_| db::Error::Integrity("projection ID"))?;
    let (text, request) = provenance::projection_raw(&verified.device, &context.head, id, raw)
        .map_err(|_| db::Error::Integrity("actual source self-HPKE"))?;
    Ok(Some(MigrationProjection { text, request }))
}
pub fn load_config(path: &Path) -> Result<MigrationConfig> {
    Ok(serde_json::from_slice(&admin::read_public(
        path,
        16 * 1024,
    )?)?)
}
pub fn upgrade(
    lease: StateDirectoryLock,
    account: AccountId,
    device: DeviceId,
    config: &MigrationConfig,
    check: bool,
    expected_profile: Option<db::Id>,
) -> Result<serde_json::Value> {
    let calls = kilogram_ratchet::ordered::native_call_counts();
    let input = MigrationInput::open(lease, *account.as_bytes(), *device.as_bytes(), config)?;
    let mut repo = match input {
        MigrationInput::Legacy(source) => {
            let mut source = *source;
            let verified = provenance::VerifiedSource::verify(&source, account, device)?;
            let facts = verified.facts(&source)?;
            let preview =
                source.preflight(&facts, config, |entry, raw| derived(&verified, entry, raw))?;
            if check {
                let mut output = serde_json::to_value(preview)?;
                output["origins"] = verified.summary();
                return Ok(output);
            }
            MigrationRepository::fence(source, &facts, config)?
        }
        MigrationInput::Indexed(repo) => {
            let mut repo = *repo;
            let stored = repo.effective_config()?;
            ensure!(
                stored.logical_quota == config.logical_quota
                    && stored.physical_capacity == config.physical_capacity,
                "CapacityCASRequired: resume requires stored effective config"
            );
            if check {
                return Ok(serde_json::to_value(repo.status()?)?);
            }
            repo
        }
    };
    let result = (|| -> Result<serde_json::Value> {
        // Ready resumes only the latest committed publication plan. It does not
        // reseal Native or revisit the closed copy lifecycle.
        if repo.status()?.storage_stage != MigrationStage::Ready {
            let verified = provenance::VerifiedSource::verify(&repo, account, device)?;
            let facts = verified.facts(&repo)?;
            repo.resume_copy(&facts)?;
            while repo.status()?.storage_stage == MigrationStage::Copying
                && repo.copy_batch(&facts, |entry, raw| derived(&verified, entry, raw))? > 0
            {
            }
            repo.begin_verification(&facts)?;
            for context in verified.contexts.values() {
                for meta in context.readable.iter().flatten() {
                    let projection = if meta.event.kind == 3 {
                        let (text, request) = provenance::projection(
                            &repo,
                            &verified.device,
                            &context.head,
                            meta.id,
                        )?;
                        Some(MigrationProjection { text, request })
                    } else {
                        None
                    };
                    repo.verify_readable_copy(
                        &facts,
                        context.spec.c,
                        meta.event.role,
                        meta.event.n,
                        projection.as_ref(),
                    )?;
                }
            }
            let final_source = provenance::VerifiedSource::verify(&repo, account, device)?;
            let final_facts = final_source.facts(&repo)?;
            repo.ready(final_facts)?;
        }
        repo.publish_profile(expected_profile)?;
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == calls,
            "zero migration packet crypto/constructors"
        );
        let mut output = serde_json::to_value(repo.status()?)?;
        output["zero_migration_native_calls"] = true.into();
        Ok(output)
    })();
    if let Err(error) = &result
        && repo
            .status()
            .is_ok_and(|s| s.storage_stage != MigrationStage::Ready)
    {
        // IO ambiguity poisons the handle and cannot be turned into a second
        // write. Reopen resolves the whole durable outcome first.
        let reason = format!("{error:#}");
        let reason = if reason.len() > 1024 {
            "MigrationVerificationBlocked"
        } else {
            &reason
        };
        repo.block(reason)
            .context("persist migration blocked checkpoint")?;
    }
    result
}
pub fn status(
    lease: StateDirectoryLock,
    account: AccountId,
    device: DeviceId,
) -> Result<serde_json::Value> {
    let mut repo = MigrationRepository::open(lease, *account.as_bytes(), *device.as_bytes(), None)?;
    Ok(serde_json::to_value(repo.status()?)?)
}
pub fn pause(
    lease: StateDirectoryLock,
    account: AccountId,
    device: DeviceId,
) -> Result<serde_json::Value> {
    let mut repo = MigrationRepository::open(lease, *account.as_bytes(), *device.as_bytes(), None)?;
    repo.pause()?;
    Ok(serde_json::to_value(repo.status()?)?)
}
pub fn increase_capacity(
    lease: StateDirectoryLock,
    account: AccountId,
    device: DeviceId,
    config: &MigrationConfig,
    generation: u64,
    revision: u64,
    hash: db::Id,
) -> Result<serde_json::Value> {
    let mut repo =
        MigrationRepository::open(lease, *account.as_bytes(), *device.as_bytes(), Some(config))?;
    repo.increase_capacity(generation, revision, hash, config)?;
    Ok(serde_json::to_value(repo.status()?)?)
}
/// The immutable legacy offset API has max1024 metadata entries/C and max8 output
/// projections. It returns the old exact DAG/Event IDs; it is not an opaque cursor.
pub fn history(
    lease: StateDirectoryLock,
    account: AccountId,
    device: DeviceId,
    c: db::Id,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value> {
    ensure!(
        limit > 0 && limit <= 8 && offset <= 1024,
        "legacy history bounds: offset<=1024, page1..8"
    );
    let repo = MigrationRepository::open(lease, *account.as_bytes(), *device.as_bytes(), None)?;
    let verified = provenance::VerifiedSource::verify_history(&repo, account, device, c)?;
    let context = verified
        .contexts
        .get(&c)
        .context("HistoryUnavailable: context")?;
    ensure!(
        context.head.sid.is_some(),
        "HistoryUnavailable: NeedHandshake"
    );
    let mut remaining = context
        .readable
        .iter()
        .flatten()
        .filter(|m| m.event.kind == 3)
        .map(|m| (m.id, m))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut emitted = std::collections::BTreeSet::new();
    while !remaining.is_empty() {
        let id = remaining
            .iter()
            .find(|(_, m)| m.parents.iter().all(|p| emitted.contains(p)))
            .map(|(id, _)| *id)
            .context("HistoryUnavailable: causal graph")?;
        let meta = remaining.remove(&id).context("history metadata")?;
        emitted.insert(id);
        ordered.push(meta);
    }
    let mut items = Vec::new();
    for meta in ordered.iter().skip(offset).take(limit) {
        let (text, request) =
            provenance::projection(&repo, &verified.device, &context.head, meta.id)?;
        ensure!(request == meta.request, "history exact source request");
        items.push(serde_json::json!({"event_id":state::hex(&meta.id),"role":meta.event.role,"ordinal":meta.event.n,"sequence":meta.event.sequence,"request_id":state::hex(&request),"text":text,"delivered":context.delivered.contains(&meta.id)}));
    }
    let output = serde_json::json!({"profile_mode":"ordered-dialog-v1","source_id":state::hex(&repo.source_id()),"context":state::hex(&c),"total":ordered.len(),"offset":offset,"items":items});
    ensure!(
        serde_json::to_vec(&output)?.len() <= 48 * 1024,
        "legacy history escaped output bound"
    );
    Ok(output)
}
pub fn reject_indexed_runtime(profile_file: &Path) -> Result<()> {
    let value: serde_json::Value =
        serde_json::from_slice(&admin::read_public(profile_file, 64 * 1024)?)?;
    if value.get("profile_mode").and_then(|v| v.as_str()) == Some("ordered-dialog-indexed") {
        bail!(
            "IndexedStorage: live indexed runtime unavailable until S5.2; use offline storage status/history"
        );
    }
    Ok(())
}
#[cfg(test)]
#[path = "storage_migration/tests.rs"]
mod tests;
