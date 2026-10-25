//! Root7 metadata and authenticated reservations; no signature or Native DTO authority.
use super::super::*;
use super::{
    codec::{self, Value, a, b, opt, u},
    source,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(super) const META_CAP: usize = 4096;
pub(super) const B4: u64 = 147_980_288;
pub(super) const OVERHEAD: u64 = 41 + 32 + 2 * NODE_CAP as u64;
pub(super) fn profile_stage(path: &std::path::Path) -> Result<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or(Error::Bound("profile stage name"))?
        .to_os_string();
    name.push(".kilogram-migration-stage");
    Ok(path.with_file_name(name))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationConfig {
    pub logical_quota: [u64; 2],
    pub physical_capacity: u64,
    pub output_profile: PathBuf,
    pub control_ipc_file: PathBuf,
    pub locator_directory: PathBuf,
}
impl MigrationConfig {
    pub(super) fn paths(&self) -> Result<Value> {
        let mut paths = Vec::new();
        let mut normalized_paths = Vec::new();
        for (index, path) in [
            &self.output_profile,
            &self.control_ipc_file,
            &self.locator_directory,
        ]
        .into_iter()
        .enumerate()
        {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(Error::Bound("migration paths must be absolute"));
            }
            let parent = path
                .parent()
                .ok_or(Error::Bound("publication parent"))?
                .canonicalize()?;
            let normalized = parent.join(path.file_name().ok_or(Error::Bound("publication name"))?);
            if normalized.exists() {
                crate::reject_symlink(&normalized)?;
                let meta = std::fs::metadata(&normalized)?;
                if (index < 2 && (!meta.is_file() || meta.len() > 64 * 1024))
                    || (index == 2 && !meta.is_dir())
                {
                    return Err(Error::Bound("public artifact type/cap"));
                }
            }
            let text = normalized.to_str().ok_or(Error::Bound("path UTF8"))?;
            if text.len() > 1024 {
                return Err(Error::Bound("publication path cap"));
            }
            paths.push(b(text.as_bytes()));
            normalized_paths.push(normalized);
        }
        if normalized_paths[0] == normalized_paths[1]
            || normalized_paths[0].starts_with(&normalized_paths[2])
            || normalized_paths[1].starts_with(&normalized_paths[2])
            || normalized_paths[1] == profile_stage(&normalized_paths[0])?
            || normalized_paths[2] == profile_stage(&normalized_paths[0])?
        {
            return Err(Error::Bound("distinct publication paths"));
        }
        Ok(a(paths))
    }
    pub(super) fn capacities(&self) -> Value {
        a([a(self.logical_quota.map(u)), u(self.physical_capacity)])
    }
    pub(super) fn backend(&self, retained: u64) -> Config {
        Config {
            logical_quota: self.logical_quota,
            physical_quota: self.physical_capacity,
            retained_external_bytes: retained,
        }
    }
}
/// Facts produced by the private owner after real V4/Native verification. State
/// independently binds them to exact source rows; IPC cannot deserialize this type.
#[derive(Clone)]
pub struct MigrationOriginFacts {
    pub source_id: Id,
    pub account: Id,
    pub device: Id,
    pub claim: Id,
    pub contexts: Vec<MigrationContextFacts>,
}
#[derive(Clone)]
pub struct MigrationContextFacts {
    pub context: Id,
    pub role: u8,
    pub devices: [Id; 2],
    pub certificates: [Id; 2],
    pub head_hash: Id,
    pub native_hash: Option<Id>,
    pub next: [u64; 2],
    pub seen: Vec<(u8, Id, u64, Id)>,
    pub ordinary_pending: u64,
    pub head_pending: Option<u64>,
}
impl MigrationOriginFacts {
    pub(super) fn value(&self) -> Result<Value> {
        if self.contexts.len() > 2 {
            return Err(Error::Bound("migration contexts"));
        }
        let mut contexts = Vec::new();
        for c in &self.contexts {
            if c.role > 1
                || c.next.iter().any(|n| *n > 512)
                || c.seen.len() > 1024
                || c.ordinary_pending > 64
                || c.head_pending.is_some_and(|n| n >= 512)
            {
                return Err(Error::Bound("migration origin facts"));
            }
            contexts.push(a([
                b(c.context),
                u(c.role.into()),
                a(c.devices.map(b)),
                a(c.certificates.map(b)),
                b(c.head_hash),
                opt(c.native_hash.map(b)),
                a(c.next.map(u)),
                a(c.seen
                    .iter()
                    .map(|s| a([u(s.0.into()), b(s.1), u(s.2), b(s.3)]))),
                u(c.ordinary_pending),
                opt(c.head_pending.map(u)),
            ]));
        }
        Ok(a([
            u(7),
            b(self.source_id),
            b(self.account),
            b(self.device),
            b(self.claim),
            a(contexts),
        ]))
    }
    pub(super) fn digest(&self) -> Result<Id> {
        codec::digest(b"kg:od2:migration-origin-facts\0", &self.value()?)
    }
}
pub(super) fn migration_key(mid: Id, kind: u8, logical: &[u8]) -> Result<Key> {
    Key::hash(
        0,
        Namespace::Migration,
        codec::digest(
            b"kg:od2:migration-record\0",
            &a([b(mid), u(kind.into()), b(logical)]),
        )?,
    )
}
pub(super) fn profile_key() -> Result<Key> {
    Key::hash(0, Namespace::RuntimeProfile, [0; 32])
}
pub(super) fn head_key(c: u8) -> Result<Key> {
    Key::hash(c, Namespace::Runtime, [0; 32])
}
pub(super) fn budget_key(c: u8) -> Result<Key> {
    Key::hash(c, Namespace::LegacyBudget, [0; 32])
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
pub enum MigrationStage {
    Copying = 0,
    Verifying = 1,
    Paused = 2,
    Blocked = 3,
    Ready = 4,
}
#[derive(Clone)]
pub(super) struct Progress {
    pub mid: Id,
    pub stage: MigrationStage,
    pub phase: u64,
    pub position: u64,
    pub last_path: Option<String>,
    pub processed: Id,
    pub counts: [u64; 15],
    pub output: Id,
    pub reason: Vec<u8>,
}
impl Progress {
    pub fn raw(&self) -> Result<Vec<u8>> {
        let counts = a([
            u(self.counts[0]),
            a((0..2).map(|c| {
                a(self.counts[1 + c * 7..1 + (c + 1) * 7]
                    .iter()
                    .copied()
                    .map(u))
            })),
        ]);
        codec::record(
            2,
            &a([
                u(7),
                b(self.mid),
                u(self.stage as u64),
                u(self.phase),
                u(self.position),
                opt(self.last_path.as_ref().map(|p| b(p.as_bytes()))),
                b(self.processed),
                counts,
                b(self.output),
                b(&self.reason),
            ]),
            8192,
        )
    }
    pub fn parse(raw: &[u8], mid: Id) -> Result<Self> {
        let value = codec::read_record(raw, 2, 8192)?;
        let f = value.fields(10)?;
        let stage = match f[2].uint()? {
            0 => MigrationStage::Copying,
            1 => MigrationStage::Verifying,
            2 => MigrationStage::Paused,
            3 => MigrationStage::Blocked,
            4 => MigrationStage::Ready,
            _ => return Err(Error::Integrity("migration stage")),
        };
        if f[0].uint()? != 7
            || f[1].fixed::<32>()? != mid
            || f[3].uint()? > 3
            || f[9].bytes()?.len() > 1024
        {
            return Err(Error::Integrity("migration progress"));
        }
        let v = f[7].fields(2)?;
        let per = v[1].fields(2)?;
        let mut counts = [0; 15];
        counts[0] = v[0].uint()?;
        for (c, p) in per.iter().enumerate() {
            for (i, n) in p.fields(7)?.iter().enumerate() {
                counts[1 + c * 7 + i] = n.uint()?;
            }
        }
        let last_path = f[5]
            .optional()?
            .map(|v| v.text().map(ToOwned::to_owned))
            .transpose()?;
        if last_path.as_ref().is_some_and(|p| p.len() > 256) {
            return Err(Error::Integrity("progress path"));
        }
        Ok(Self {
            mid,
            stage,
            phase: f[3].uint()?,
            position: f[4].uint()?,
            last_path,
            processed: f[6].fixed()?,
            counts,
            output: f[8].fixed()?,
            reason: f[9].bytes()?.to_vec(),
        })
    }
}
#[derive(Clone)]
pub(super) struct Profile {
    pub source: Id,
    pub mid: Id,
    pub device: Id,
    pub mask: u8,
    pub identity: Value,
    pub initial_config: Id,
    pub origin: Id,
    pub contexts: Vec<(Id, u8)>,
    pub signers: Vec<Id>,
    pub plan_revision: u64,
    pub plan_hash: Id,
}
impl Profile {
    pub fn raw(&self) -> Result<Vec<u8>> {
        codec::record(
            9,
            &a([
                u(7),
                b(self.source),
                b(self.mid),
                b(self.device),
                u(self.mask.into()),
                self.identity.clone(),
                b(self.initial_config),
                b(self.origin),
                a(self.contexts.iter().map(|(c, r)| a([b(c), u((*r).into())]))),
                a(self.signers.iter().map(b)),
                u(self.plan_revision),
                b(self.plan_hash),
            ]),
            16 * 1024,
        )
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let value = codec::read_record(raw, 9, 16 * 1024)?;
        let f = value.fields(12)?;
        if f[0].uint()? != 7 || f[4].uint()? > 3 {
            return Err(Error::Integrity("migration profile marker"));
        }
        let contexts = f[8]
            .list()?
            .iter()
            .map(|v| {
                let x = v.fields(2)?;
                let role = u8::try_from(x[1].uint()?).map_err(|_| Error::Integrity("role"))?;
                if role > 1 {
                    return Err(Error::Integrity("role"));
                }
                Ok((x[0].fixed()?, role))
            })
            .collect::<Result<Vec<_>>>()?;
        let mask = f[4].uint()? as u8;
        if contexts.len() > 2
            || mask != (1 << contexts.len()) - 1
            || contexts.windows(2).any(|x| x[0].0 >= x[1].0)
        {
            return Err(Error::Integrity("occupied context slots"));
        }
        let signers = f[9]
            .list()?
            .iter()
            .map(Value::fixed)
            .collect::<Result<Vec<Id>>>()?;
        let device = f[3].fixed::<32>()?;
        if signers.is_empty()
            || signers.len() > 3
            || !signers.contains(&device)
            || signers.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(Error::Integrity("migration signer slots"));
        }
        Ok(Self {
            source: f[1].fixed()?,
            mid: f[2].fixed()?,
            device,
            mask,
            identity: f[5].clone(),
            initial_config: f[6].fixed()?,
            origin: f[7].fixed()?,
            contexts,
            signers,
            plan_revision: f[10].uint()?,
            plan_hash: f[11].fixed()?,
        })
    }
}
#[derive(Clone)]
pub(super) struct Effective {
    pub mid: Id,
    pub revision: u64,
    pub identity: Id,
    pub initial: Id,
    pub quotas: [u64; 2],
    pub physical: u64,
    pub previous: Option<Id>,
}
impl Effective {
    pub fn raw(&self) -> Result<Vec<u8>> {
        codec::record(
            7,
            &a([
                u(7),
                b(self.mid),
                u(self.revision),
                b(self.identity),
                b(self.initial),
                a(self.quotas.map(u)),
                u(self.physical),
                opt(self.previous.map(b)),
            ]),
            META_CAP,
        )
    }
    pub fn hash(raw: &[u8]) -> Id {
        let mut h = blake3::Hasher::new();
        h.update(b"kg:od2:effective-migration-config\0");
        h.update(raw);
        *h.finalize().as_bytes()
    }
    pub fn parse(raw: &[u8], profile: &Profile) -> Result<Self> {
        let value = codec::read_record(raw, 7, META_CAP)?;
        let f = value.fields(8)?;
        let q = f[5].fields(2)?;
        let revision = f[2].uint()?;
        let previous = f[7].optional()?.map(Value::fixed).transpose()?;
        if f[0].uint()? != 7
            || f[1].fixed::<32>()? != profile.mid
            || revision == 0
            || (revision == 1) != previous.is_none()
            || f[3].fixed::<32>()?
                != codec::digest(b"kg:od2:migration-config-identity\0", &profile.identity)?
            || f[4].fixed::<32>()? != profile.initial_config
        {
            return Err(Error::Integrity("effective migration config"));
        }
        Ok(Self {
            mid: profile.mid,
            revision,
            identity: f[3].fixed()?,
            initial: f[4].fixed()?,
            quotas: [q[0].uint()?, q[1].uint()?],
            physical: f[6].uint()?,
            previous,
        })
    }
}
pub(super) fn desired_profile(profile: &Profile, config: &Effective) -> Result<Vec<u8>> {
    let f = profile.identity.fields(14)?;
    let paths = f[11].fields(3)?;
    let value = serde_json::json!({"version":3,"profile_mode":"ordered-dialog-indexed",
        "account":source::hex(&f[1].fixed::<32>()?),"claim":source::hex(&f[2].fixed::<32>()?),"device":source::hex(&profile.device),
        "source_id":source::hex(&profile.source),"migration_id":source::hex(&profile.mid),"config_revision":config.revision,
        "state_dir":f[10].text()?,"ipc_file":paths[1].text()?,"locator_directory":paths[2].text()?,
        "logical_quota":config.quotas,"physical_capacity":config.physical,"route_policy":"direct-only",
        "enabled_contexts":profile.contexts.iter().map(|(c,_)|source::hex(c)).collect::<Vec<_>>()});
    let mut raw =
        serde_json::to_vec_pretty(&value).map_err(|_| Error::Bound("migration profile JSON"))?;
    raw.push(b'\n');
    if raw.len() > 64 * 1024 {
        return Err(Error::Bound("migration profile size"));
    }
    Ok(raw)
}
pub(super) fn profile_plan(profile: &Profile, config: &Effective) -> Result<Id> {
    let raw = desired_profile(profile, config)?;
    let mut h = blake3::Hasher::new();
    h.update(b"kg:od2:migration-profile-plan\0");
    h.update(&raw);
    Ok(*h.finalize().as_bytes())
}
pub(super) fn covered(root: &Root, c: usize) -> Result<(u64, u64)> {
    let mut used = 0u64;
    let mut reserved = 0u64;
    for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 20] {
        let s = root.roots[c * 24 + n];
        used = used
            .checked_add(s.used[c])
            .ok_or(Error::Bound("covered used"))?;
        reserved = reserved
            .checked_add(s.reserved[c])
            .ok_or(Error::Bound("covered reserved"))?;
    }
    Ok((used, reserved))
}
impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn migration_output_digest(&mut self, mid: Id) -> Result<Id> {
        let mut tree = self.tree.fork_for_digest();
        let mut roots = self.root.roots;
        for kind in [2, 3] {
            let key = migration_key(mid, kind, b"")?;
            if tree.find(51, roots[51], key.bytes)?.is_some() {
                roots[51] = tree.remove(51, roots[51], key.bytes)?;
            }
        }
        self.history_buffer_peak = self.history_buffer_peak.max(
            self.buffer_bound_bytes()
                .checked_add(tree.buffer_bound_bytes())
                .ok_or(Error::Bound("digest scratch buffers"))?,
        );
        let mut encoded = Vec::with_capacity(16 * 1024);
        for root in roots {
            root.encode(&mut encoded);
        }
        let mut hash = blake3::Hasher::new();
        hash.update(b"kg:od2:migration-output\0");
        hash.update(&encoded);
        Ok(*hash.finalize().as_bytes())
    }
    pub(super) fn migration_profile(&mut self) -> Result<Profile> {
        if self.root.layout != RootLayout::Migrated7 {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        Profile::parse(
            &self
                .get(profile_key()?)?
                .ok_or(Error::Integrity("missing migration marker"))?,
        )
    }
    pub(super) fn migration_progress(&mut self, mid: Id) -> Result<Progress> {
        Progress::parse(
            &self
                .get(migration_key(mid, 2, b"")?)?
                .ok_or(Error::Integrity("missing copy progress"))?,
            mid,
        )
    }
    pub(super) fn migration_budget_put(
        &mut self,
        profile: &Profile,
        stage: MigrationStage,
    ) -> Result<()> {
        for c in 0..2usize {
            if profile.mask & (1 << c) == 0 {
                continue;
            }
            let (used, reserved) = covered(&self.root, c)?;
            let hold = B4.saturating_sub(
                used.checked_add(reserved)
                    .ok_or(Error::Bound("covered total"))?,
            );
            let ready = stage == MigrationStage::Ready;
            let e = if ready { 0 } else { REPAIR_POOL };
            let raw = codec::record(
                6,
                &a([
                    u(7),
                    b(profile.mid),
                    u(c as u64),
                    u(B4),
                    u(0),
                    u(used),
                    u(reserved),
                    u(hold),
                    u(u64::from(ready)),
                    u(e),
                    a([]),
                    a([]),
                ]),
                META_CAP,
            )?;
            let metadata = (META_CAP - raw.len()) as u64;
            self.put(
                budget_key(c as u8)?,
                &raw,
                metadata
                    .checked_add(hold)
                    .and_then(|v| v.checked_add(e))
                    .ok_or(Error::Bound("legacy escrow"))?,
                true,
            )?;
        }
        Ok(())
    }
    pub(super) fn migration_leaf_check(
        &mut self,
        key: Key,
        raw: &[u8],
        reserved: u64,
    ) -> Result<()> {
        let slot = key.slot()?;
        let leaf = self
            .tree
            .find(slot as u8, self.root.roots[slot], key.bytes)?
            .ok_or(Error::Integrity("required migration leaf"))?;
        match leaf.body {
            tree::Body::Leaf {
                owner,
                charge,
                reserved: r,
                length,
                ..
            } if owner == key.context
                && charge == raw.len() as u64 + OVERHEAD
                && length == raw.len() as u64
                && r == reserved =>
            {
                Ok(())
            }
            _ => Err(Error::Integrity(
                "migration payload/owner/reservation correspondence",
            )),
        }
    }
    pub(super) fn migration_budget_check(
        &mut self,
        profile: &Profile,
        c: usize,
        stage: MigrationStage,
    ) -> Result<()> {
        let raw = self
            .get(budget_key(c as u8)?)?
            .ok_or(Error::Integrity("missing legacy budget"))?;
        let value = codec::read_record(&raw, 6, META_CAP)?;
        value.fields(12)?;
        let (used, reserved) = covered(&self.root, c)?;
        let hold = B4.saturating_sub(
            used.checked_add(reserved)
                .ok_or(Error::Bound("covered total"))?,
        );
        let ready = stage == MigrationStage::Ready;
        let e = if ready { 0 } else { REPAIR_POOL };
        if value
            != a([
                u(7),
                b(profile.mid),
                u(c as u64),
                u(B4),
                u(0),
                u(used),
                u(reserved),
                u(hold),
                u(u64::from(ready)),
                u(e),
                a([]),
                a([]),
            ])
        {
            return Err(Error::Integrity("legacy budget derived totals/funding"));
        }
        if self.root.roots[23 + 24 * c].count != 1 {
            return Err(Error::Integrity("budget singleton count"));
        }
        self.migration_leaf_check(
            budget_key(c as u8)?,
            &raw,
            (META_CAP - raw.len()) as u64 + hold + e,
        )
    }
    pub(super) fn migration_check(&mut self) -> Result<()> {
        let profile = self.migration_profile()?;
        let profile_raw = profile.raw()?;
        self.migration_leaf_check(
            profile_key()?,
            &profile_raw,
            (16 * 1024 - profile_raw.len()) as u64,
        )?;
        if codec::digest(
            b"kg:od2:migration-instance\0",
            &a([b(profile.source), b(self.root.incarnation)]),
        )? != profile.mid
        {
            return Err(Error::Integrity("migration incarnation"));
        }
        let progress = self.migration_progress(profile.mid)?;
        let progress_raw = progress.raw()?;
        self.migration_leaf_check(
            migration_key(profile.mid, 2, b"")?,
            &progress_raw,
            (8192 - progress_raw.len()) as u64,
        )?;
        let config_raw = self
            .get(migration_key(profile.mid, 7, b"")?)?
            .ok_or(Error::Integrity("missing effective config"))?;
        let config = Effective::parse(&config_raw, &profile)?;
        self.migration_leaf_check(
            migration_key(profile.mid, 7, b"")?,
            &config_raw,
            (META_CAP - config_raw.len()) as u64,
        )?;
        if profile.plan_revision != config.revision
            || profile.plan_hash != profile_plan(&profile, &config)?
        {
            return Err(Error::Integrity("effective profile publication plan"));
        }
        for c in 0..2usize {
            if self.root.ledger[c].quota != config.quotas[c] {
                return Err(Error::Integrity("effective quota/root"));
            }
            if profile.mask & (1 << c) == 0 {
                if (c * 24..c * 24 + 22).any(|slot| self.root.roots[slot] != Summary::default()) {
                    return Err(Error::Integrity("data in unoccupied context"));
                }
                if self.root.roots[23 + 24 * c] != Summary::default()
                    || self.root.roots[21 + 24 * c] != Summary::default()
                    || self.root.ledger[c].runtime_state().repair_free != 0
                    || self.root.ledger[c].runtime_state().outstanding != 0
                    || self.root.ledger[c].runtime_state().pending != 0
                    || self.root.ledger[c].runtime_state().head.is_some()
                    || self.root.ledger[c].runtime_state().grant.is_some()
                {
                    return Err(Error::Integrity("unoccupied migration slot"));
                }
                continue;
            }
            if self.root.contexts[c] != profile.contexts[c].0 {
                return Err(Error::Integrity("migration context identity"));
            }
            let head_raw = self
                .get(head_key(c as u8)?)?
                .ok_or(Error::Integrity("required Head7"))?;
            let head = codec::read_record(&head_raw, 10, 8192)?;
            self.migration_leaf_check(
                head_key(c as u8)?,
                &head_raw,
                (8192 - head_raw.len()) as u64,
            )?;
            let h = head.fields(9)?;
            if self.root.roots[c * 24 + 21].count != 1
                || h[0].uint()? != 7
                || h[1].fixed::<32>()? != profile.source
                || h[2].fixed::<32>()? != profile.contexts[c].0
                || h[3].uint()? != u64::from(profile.contexts[c].1)
                || h[6].uint()? != 1
                || h[7].optional()?.is_some()
                || h[8]
                    .fields(2)?
                    .iter()
                    .any(|v| !v.optional().is_ok_and(|n| n.is_none()))
            {
                return Err(Error::Integrity("Head7 L1/current-purpose shape"));
            }
            let legacy = codec::decode(h[4].bytes()?, 4096)?;
            let lh = legacy.fields(13)?;
            let phase = lh[5].uint()?;
            let native = lh[12].optional()?.map(Value::fixed).transpose()?;
            if lh[0].uint()? != 1
                || lh[1].fixed::<32>()? != profile.contexts[c].0
                || lh[3].uint()? != u64::from(profile.contexts[c].1)
                || phase > 3
                || (phase == 0) != native.is_none()
            {
                return Err(Error::Integrity("original Head phase/scope"));
            }
            let ready = progress.stage == MigrationStage::Ready;
            if h[5].optional()?.map(Value::fixed).transpose()? != if ready { native } else { None }
            {
                return Err(Error::Integrity(
                    "Native current pointer before/after Ready",
                ));
            }
            let native_key = Key::hash(c as u8, Namespace::Native, [0; 32])?;
            let raw = self.get(native_key)?;
            if self.root.roots[c * 24 + 15].count != u64::from(raw.is_some())
                || (ready && raw.is_some() != native.is_some())
            {
                return Err(Error::Integrity("required current Native1"));
            }
            if let Some(raw) = raw {
                if raw.len() > 128 * 1024
                    || Some(super::copy::raw_hash(b"kg:od1:native-state\0", &raw)) != native
                {
                    return Err(Error::Integrity("copied original Native1"));
                }
                self.migration_leaf_check(native_key, &raw, (128 * 1024 - raw.len()) as u64)?;
            }
            self.migration_budget_check(&profile, c, progress.stage)?;
            {
                let state = self.root.ledger[c].runtime_state();
                if state.repair_free != if ready { REPAIR_POOL } else { 0 }
                    || state.outstanding != 0
                    || state.grant.is_some()
                    || state.pending + u64::from(state.head.is_some())
                        != self.root.roots[c * 24 + 9].count
                {
                    return Err(Error::Integrity("migration funding and pending ledger"));
                }
            }
            self.migration_ticket_check(&profile, c as u8)?;
        }
        let raw = self
            .get(migration_key(profile.mid, 8, b"")?)?
            .ok_or(Error::Integrity("source backing missing"))?;
        let v = codec::read_record(&raw, 8, META_CAP)?;
        let f = v.fields(6)?;
        if f[0].uint()? != 7
            || f[1].fixed::<32>()? != profile.mid
            || f[2].fixed::<32>()? != profile.source
        {
            return Err(Error::Integrity("source backing identity"));
        }
        f[3].fixed::<32>()?;
        f[4].uint()?;
        self.migration_leaf_check(
            migration_key(profile.mid, 8, b"")?,
            &raw,
            ((META_CAP - raw.len()) as u64)
                .checked_add(f[5].uint()?)
                .ok_or(Error::Bound("backing liability"))?,
        )?;
        let intent_raw = self
            .get(migration_key(profile.mid, 1, b"")?)?
            .ok_or(Error::Integrity("missing migration intent"))?;
        let intent = codec::read_record(&intent_raw, 1, 24 * 1024)?;
        self.migration_leaf_check(
            migration_key(profile.mid, 1, b"")?,
            &intent_raw,
            (24 * 1024 - intent_raw.len()) as u64,
        )?;
        let f = intent.fields(8)?;
        if f[0].uint()? != 7
            || codec::digest(b"kg:od2:migration-source\0", &f[1])? != profile.source
            || f[2].fixed::<32>()? != profile.source
            || f[3].fixed::<32>()? != profile.mid
            || f[5].fixed::<32>()? != profile.initial_config
            || f[6].fixed::<32>()? != config.identity
            || f[7].uint()? != u64::from(profile.mask)
        {
            return Err(Error::Integrity("migration intent/source/config"));
        }
        let identity = f[1].fields(9)?;
        let source_manifest: crate::vault::VaultManifest =
            postcard::from_bytes(identity[1].bytes()?)
                .map_err(|_| Error::Integrity("saved source manifest"))?;
        if source_manifest.schema_version != 4
            || progress.position > source_manifest.record_count
            || progress.counts[0] != progress.position
            || progress
                .counts
                .iter()
                .any(|n| *n > source_manifest.record_count)
            || identity[3].uint()? >= self.root.generation
        {
            return Err(Error::Integrity("bounded copy/source generation"));
        }
        let claim = codec::decode(identity[5].bytes()?, 1024)?;
        let cf = claim.fields(5)?;
        if identity[0].uint()? != 1
            || identity[6].fixed::<32>()? != self.root.account
            || identity[7].fixed::<32>()? != profile.device
            || cf[0].uint()? != 1
            || cf[1].uint()? != 4
            || cf[2].fixed::<32>()? != self.root.claim
            || cf[3].fixed::<32>()? != self.root.account
            || cf[4].fixed::<32>()? != profile.device
        {
            return Err(Error::Integrity("original authenticated owner"));
        }
        let mode = f[4].fields(3)?;
        mode[0].fixed::<32>()?;
        if mode[1].bytes()?.len() > 576 || mode[2].bytes()? != source::MODE1 {
            return Err(Error::Integrity("saved mode override"));
        }
        let origin_raw = self
            .get(migration_key(profile.mid, 12, b"")?)?
            .ok_or(Error::Integrity("missing Native origin witness"))?;
        let origin_value = codec::read_record(&origin_raw, 12, META_CAP)?;
        self.migration_leaf_check(
            migration_key(profile.mid, 12, b"")?,
            &origin_raw,
            (META_CAP - origin_raw.len()) as u64,
        )?;
        if origin_value != a([u(7), b(profile.mid), b(profile.source), b(profile.origin)]) {
            return Err(Error::Integrity("Native origin witness binding"));
        }
        if progress.stage == MigrationStage::Ready {
            let raw = self
                .get(migration_key(profile.mid, 3, b"")?)?
                .ok_or(Error::Integrity("Ready marker missing"))?;
            let value = codec::read_record(&raw, 3, META_CAP)?;
            self.migration_leaf_check(
                migration_key(profile.mid, 3, b"")?,
                &raw,
                (META_CAP - raw.len()) as u64,
            )?;
            let r = value.fields(7)?;
            if r[0].uint()? != 7
                || r[1].fixed::<32>()? != profile.mid
                || r[2].fixed::<32>()? != identity[8].fixed::<32>()?
                || r[3].fixed::<32>()? != progress.output
                || progress.position != source_manifest.record_count
            {
                return Err(Error::Integrity("Ready completeness marker"));
            }
            for field in &r[4..7] {
                field.fixed::<32>()?;
            }
        }
        if progress.stage != MigrationStage::Ready
            && progress.output != self.migration_output_digest(profile.mid)?
        {
            return Err(Error::Integrity("copy output digest"));
        }
        self.root.check()
    }
}
