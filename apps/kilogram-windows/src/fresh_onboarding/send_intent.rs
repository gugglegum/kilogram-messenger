//! One DPAPI-protected intent under the existing Setup lock.
use super::model::{self, Setup};
use anyhow::{Context, Result, ensure};
use kilogram_runtime_ipc::ordered2::{Profile2Document, id};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, Metadata},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};

const CAP: usize = 32 * 1024;
const FILE: &str = "intent.dpapi";
const STAGE_PREFIX: &str = "stage-";
const STAGE_SUFFIX: &str = ".dpapi.tmp";
const STAGE_RANDOM: usize = 12;
const TEXT_CAP: usize = kilogram_protocol::ordered_dialog_v2::MAX_TEXT;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Binding {
    setup: String,
    invitation: String,
    account: String,
    device: String,
    claim: String,
    peer_account: String,
    peer_device: String,
    contexts: [String; 2],
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Phase {
    Prepared,
    Accepted { queue_sequence: u64 },
    Conflict,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    #[serde(rename = "kilogram-cs3.3-send")]
    Send,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Intent {
    kind: Kind,
    version: u8,
    binding: Binding,
    pub(super) slot: u8,
    context: [u8; 32],
    pub(super) request: [u8; 16],
    pub(super) phase: Phase,
    pub(super) text: String,
}
impl Drop for Intent {
    fn drop(&mut self) {
        self.text.zeroize();
        self.binding.claim.zeroize();
    }
}

pub(super) struct Store {
    home: PathBuf,
    binding: Binding,
    #[cfg(test)]
    fault: std::cell::Cell<Option<Mutation>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutation {
    Publish,
    Replace,
    #[cfg(test)]
    Remove,
}

impl Store {
    pub(super) fn new(setup: &Setup, profile: &Profile2Document) -> Result<Self> {
        let info = setup
            .record
            .public
            .as_ref()
            .context("Pending intent account")?;
        let peer = setup.record.peer.as_ref().context("Pending intent peer")?;
        info.validate()?;
        ensure!(
            profile.state_dir == info.state_dir
                && profile.state_dir == setup.record.workspace.join("device")
                && profile.account == info.account_id.to_string()
                && profile.device == info.device_id.to_string()
                && profile.claim == info.claim
                && profile.contexts == setup.record.contexts
                && profile.contexts == info.contexts
                && profile.enabled == [0, 1],
            "Pending intent profile binding"
        );
        for path in [&setup.home, &setup.record.workspace, &info.state_dir] {
            check_directory(path)?;
        }
        Ok(Self {
            home: setup.home.clone(),
            binding: Binding {
                setup: setup.record.local_id.clone(),
                invitation: setup.record.invitation.clone(),
                account: profile.account.clone(),
                device: profile.device.clone(),
                claim: profile.claim.clone(),
                peer_account: peer.account.to_string(),
                peer_device: peer.device.to_string(),
                contexts: profile.contexts.clone(),
            },
            #[cfg(test)]
            fault: std::cell::Cell::default(),
        })
    }

    pub(super) fn context(&self, slot: u8) -> Result<[u8; 32]> {
        id(self
            .binding
            .contexts
            .get(usize::from(slot))
            .context("Pending intent slot")?)
    }

    pub(super) fn load(&self) -> Result<Option<Intent>> {
        let dir = self.directory();
        check_directory(&self.home)?;
        if metadata(&dir)?.is_none() {
            return Ok(None);
        }
        check_directory(&dir)?;
        let final_path = dir.join(FILE);
        let mut staged = None;
        for entry in fs::read_dir(&dir)?.take(3) {
            let entry = entry?;
            let name = entry.file_name();
            if name == FILE {
                continue;
            }
            ensure!(
                staged.is_none() && name.to_str().is_some_and(stage_name),
                "Unexpected pending intent directory entry"
            );
            check_file(&entry.path())?.context("Missing staging file")?;
            staged = Some(entry.path());
        }
        let intent = if check_file(&final_path)?.is_some() {
            let protected = model::read(&final_path, CAP)?;
            let raw = Zeroizing::new(unprotect(&protected)?);
            ensure!(raw.len() <= CAP, "Pending intent plaintext cap");
            let intent: Intent = serde_json::from_slice(&raw)
                .map_err(|_| anyhow::anyhow!("Invalid pending intent encoding"))?;
            ensure!(
                intent.version == 1
                    && intent.binding == self.binding
                    && self.context(intent.slot)? == intent.context
                    && valid_text(&intent.text),
                "Pending intent binding/shape"
            );
            Some(intent)
        } else {
            None
        };
        if let Some(staged) = staged {
            // Only the final name can authorize IPC. An interrupted temp write
            // never becomes an intent, even when its ciphertext is readable.
            fs::remove_file(staged)?;
        }
        Ok(intent)
    }

    pub(super) fn prepare(
        &self,
        slot: u8,
        request: [u8; 16],
        mut text: Zeroizing<String>,
    ) -> Result<Intent> {
        ensure!(slot < 2 && valid_text(&text), "Pending intent input");
        ensure!(self.load()?.is_none(), "A pending intent already exists");
        let intent = Intent {
            kind: Kind::Send,
            version: 1,
            binding: self.binding.clone(),
            slot,
            context: self.context(slot)?,
            request,
            phase: Phase::Prepared,
            text: std::mem::take(&mut *text),
        };
        self.persist(&intent, Mutation::Publish)?;
        Ok(intent)
    }

    pub(super) fn transition(&self, intent: &mut Intent, phase: Phase) -> Result<()> {
        if intent.phase == phase {
            return Ok(());
        }
        ensure!(
            matches!(
                (intent.phase, phase),
                (Phase::Prepared, Phase::Accepted { .. })
                    | (Phase::Prepared | Phase::Accepted { .. }, Phase::Conflict)
            ),
            "Invalid pending intent transition"
        );
        ensure!(
            self.load()?.as_ref() == Some(intent),
            "Pending intent changed"
        );
        let previous = std::mem::replace(&mut intent.phase, phase);
        let result = self.persist(intent, Mutation::Replace);
        if result.is_err() {
            intent.phase = previous;
        }
        result
    }

    pub(super) fn remove_accepted(&self, intent: &Intent) -> Result<()> {
        ensure!(
            matches!(intent.phase, Phase::Accepted { .. }) && self.load()?.as_ref() == Some(intent),
            "Pending intent changed before cleanup"
        );
        #[cfg(test)]
        self.check_fault(Mutation::Remove)?;
        fs::remove_file(self.directory().join(FILE))?;
        Ok(())
    }

    fn directory(&self) -> PathBuf {
        self.home.join("pending-send")
    }

    fn persist(&self, intent: &Intent, mutation: Mutation) -> Result<()> {
        let raw = Zeroizing::new(
            serde_json::to_vec(intent).map_err(|_| anyhow::anyhow!("Encode pending intent"))?,
        );
        ensure!(raw.len() <= CAP, "Pending intent plaintext cap");
        let sealed = protect(&raw)?;
        ensure!(
            !sealed.is_empty() && sealed.len() <= CAP,
            "Pending intent ciphertext cap"
        );
        let dir = self.directory();
        if metadata(&dir)?.is_none() {
            fs::create_dir(&dir)?;
        }
        check_directory(&dir)?;
        let mut temp = tempfile::Builder::new()
            .prefix(STAGE_PREFIX)
            .suffix(STAGE_SUFFIX)
            .rand_bytes(STAGE_RANDOM)
            .tempfile_in(&dir)?;
        temp.write_all(&sealed)?;
        temp.as_file().sync_all()?;
        #[cfg(test)]
        self.check_fault(mutation)?;
        let path = dir.join(FILE);
        if mutation == Mutation::Publish {
            temp.persist_noclobber(path)?;
        } else {
            temp.persist(path)?;
        }
        ensure!(
            self.load()?.as_ref() == Some(intent),
            "Pending intent publication not confirmed"
        );
        Ok(())
    }

    #[cfg(test)]
    fn check_fault(&self, mutation: Mutation) -> Result<()> {
        if self.fault.get() == Some(mutation) {
            self.fault.set(None);
            anyhow::bail!("Injected pending intent storage failure");
        }
        Ok(())
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        self.binding.claim.zeroize();
    }
}

pub(super) fn valid_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= TEXT_CAP
}

fn stage_name(name: &str) -> bool {
    name.strip_prefix(STAGE_PREFIX)
        .and_then(|name| name.strip_suffix(STAGE_SUFFIX))
        .is_some_and(|token| {
            token.len() == STAGE_RANDOM && token.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

fn metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(!meta.file_type().is_symlink(), "Pending intent symlink");
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    meta.file_attributes() & 0x400 == 0,
                    "Pending intent reparse point"
                );
            }
            Ok(Some(meta))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn check_directory(path: &Path) -> Result<()> {
    ensure!(
        metadata(path)?.is_some_and(|meta| meta.is_dir()) && fs::canonicalize(path)? == path,
        "Unexpected pending intent directory"
    );
    Ok(())
}

fn check_file(path: &Path) -> Result<Option<Metadata>> {
    let meta = metadata(path)?;
    ensure!(
        meta.as_ref()
            .is_none_or(|meta| meta.is_file() && meta.len() <= CAP as u64),
        "Expected bounded pending intent file"
    );
    Ok(meta)
}

#[cfg(windows)]
fn protect(raw: &[u8]) -> Result<Vec<u8>> {
    stellar_agent_windows_identity::dpapi_protect(raw)
        .map_err(|_| anyhow::anyhow!("Pending intent Windows protection unavailable"))
}
#[cfg(windows)]
fn unprotect(raw: &[u8]) -> Result<Vec<u8>> {
    stellar_agent_windows_identity::dpapi_unprotect(raw)
        .map_err(|_| anyhow::anyhow!("Pending intent Windows protection unavailable"))
}
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("Pending intent requires Windows protection")
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("Pending intent requires Windows protection")
}

#[cfg(all(test, windows))]
mod tests;
