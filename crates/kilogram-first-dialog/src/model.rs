use super::artifact::{self, Artifact, Identity, Kind, Recipient};
use anyhow::{Result, ensure};
use kilogram_bootstrap_contract::{
    ORDERED2_BOOTSTRAP_MODE, ORDERED2_RECOVERY_SCOPE, Ordered2BootstrapOutput,
    Ordered2BootstrapPublic,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroizing;

mod terminal;
pub use terminal::SetupAccessError;

#[derive(Debug)]
pub enum ReplyError {
    Invalid(anyhow::Error),
    NotReady,
    Mismatch,
    PeerConflict,
    Unconfirmed,
    Persist(anyhow::Error),
}

impl From<ReplyError> for anyhow::Error {
    fn from(error: ReplyError) -> Self {
        match error {
            ReplyError::Invalid(error) | ReplyError::Persist(error) => error,
            ReplyError::NotReady => anyhow::anyhow!("Account is not ready"),
            ReplyError::Mismatch => {
                anyhow::anyhow!("Reply belongs to a different invitation or recipient")
            }
            ReplyError::PeerConflict => anyhow::anyhow!("A different peer was already selected"),
            ReplyError::Unconfirmed => {
                anyhow::anyhow!("Compare the peer identity independently first")
            }
        }
    }
}

#[derive(Debug)]
pub enum ExportError {
    Failed(anyhow::Error),
    Conflict(anyhow::Error),
}

impl From<anyhow::Error> for ExportError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

impl From<ExportError> for anyhow::Error {
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::Failed(error) => error,
            ExportError::Conflict(error) => {
                anyhow::anyhow!("Public export already exists with different contents: {error}")
            }
        }
    }
}

const RECORD_CAP: usize = 128 * 1024;
pub fn read(path: &Path, cap: usize) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= cap as u64,
        "Expected bounded local regular file"
    );
    let mut b = Vec::new();
    File::open(path)?.take(cap as u64 + 1).read_to_end(&mut b)?;
    ensure!(b.len() <= cap, "Local file exceeded its bound");
    Ok(b)
}
pub(super) fn write(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    let mut tmp = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("Missing output parent"))?,
    )?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    if replace {
        tmp.persist(path)?;
    } else {
        tmp.persist_noclobber(path)?;
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub local_id: String,
    pub invitation: String,
    pub contexts: [String; 2],
    pub workspace: PathBuf,
    pub peer: Option<Identity>,
    pub joined: bool,
    pub acknowledged: bool,
    pub public: Option<Ordered2BootstrapPublic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<super::consent::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<super::connection::State>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    profile_mode: String,
    status: String,
    public: Ordered2BootstrapPublic,
    recovery_scope: String,
}
pub struct Setup {
    pub home: PathBuf,
    pub record: Record,
    pub phrase: Option<Zeroizing<String>>,
    pub incomplete: bool,
    pub consent_confirmed: bool,
    pub(super) pre_spawn_cleanup_unconfirmed: bool,
    _lock: Arc<File>,
}
/// Retains the SAME exclusively locked file object across an in-process owner
/// transfer/failure. This grants no open/reopen or lock-bypass capability.
#[derive(Clone)]
pub struct SetupOwnership {
    _file: Arc<File>,
}
impl Setup {
    pub fn retain_ownership(&self) -> SetupOwnership {
        SetupOwnership {_file:self._lock.clone()}
    }
    pub fn pre_spawn_cleanup_unconfirmed(&self) -> bool {
        self.pre_spawn_cleanup_unconfirmed
    }

    fn lock(home: &Path) -> Result<(PathBuf, File)> {
        fs::create_dir_all(home)?;
        let home = fs::canonicalize(home)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(home.join(".onboarding.lock"))?;
        lock.try_lock()
            .map_err(|_| anyhow::anyhow!("This setup is already open in another window"))?;
        Ok((home, lock))
    }
    fn validate(record: &Record, home: &Path) -> Result<()> {
        ensure!(record.version == 1, "Unsupported setup record");
        for s in [
            &record.local_id,
            &record.invitation,
            &record.contexts[0],
            &record.contexts[1],
        ] {
            artifact::canonical_id(s)?;
        }
        ensure!(
            record.contexts[0] != record.contexts[1]
                && record.workspace == home.join(format!("account-{}", record.local_id)),
            "Setup workspace/context binding mismatch"
        );
        if let Some(peer) = &record.peer {
            peer.validate()?;
        }
        ensure!(
            !record.joined || record.peer.is_some(),
            "Joined setup lost its peer"
        );
        ensure!(
            !record.acknowledged || record.public.is_some(),
            "Recovery cannot be acknowledged before ready creation"
        );
        if let Some(info) = &record.public {
            info.validate()?;
            ensure!(
                info.workspace_dir == record.workspace && info.contexts == record.contexts,
                "Setup public binding mismatch"
            );
        }
        if let Some(state) = &record.consent {
            state.validate()?;
        }
        Ok(())
    }
    pub fn begin(home: &Path, invite: Option<Artifact>) -> Result<Self> {
        Self::validate_invite(invite.as_ref())?;
        let (home, lock) = Self::lock(home)?;
        Self::begin_locked(home, lock, invite)
    }

    fn validate_invite(invite: Option<&Artifact>) -> Result<()> {
        if let Some(invite) = invite {
            invite.validate()?;
            ensure!(
                invite.kind == Kind::Invite,
                "Join requires an invitation, not a reply"
            );
        }
        Ok(())
    }

    fn begin_locked(home: PathBuf, lock: File, invite: Option<Artifact>) -> Result<Self> {
        ensure!(
            !home.join("setup.json").try_exists()?,
            "Open the existing account setup; do not create another account for a contact"
        );
        let local_id = artifact::id()?;
        let (invitation, contexts, peer, joined) = if let Some(invite) = invite {
            (
                invite.invitation,
                invite.contexts,
                Some(invite.sender),
                true,
            )
        } else {
            let a = artifact::id()?;
            let mut b = artifact::id()?;
            while b == a {
                b = artifact::id()?;
            }
            (artifact::id()?, [a, b], None, false)
        };
        let record = Record {
            version: 1,
            workspace: home.join(format!("account-{local_id}")),
            local_id,
            invitation,
            contexts,
            peer,
            joined,
            acknowledged: false,
            public: None,
            consent: None,
            connection: None,
        };
        Self::validate(&record, &home)?;
        ensure!(
            fs::symlink_metadata(&record.workspace)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "Account destination must not exist"
        );
        // Deliberately outside the CS2 destination, before helper launch.
        write(
            &home.join("setup.json"),
            &serde_json::to_vec_pretty(&record)?,
            false,
        )?;
        Ok(Self {
            home,
            record,
            phrase: None,
            incomplete: true,
            consent_confirmed: false,
            pre_spawn_cleanup_unconfirmed: false,
            _lock: Arc::new(lock),
        })
    }
    pub fn open(home: &Path) -> Result<Self> {
        let (home, lock) = Self::lock(home)?;
        Self::open_locked(home, lock, false)
    }

    fn open_locked(home: PathBuf, lock: File, terminal: bool) -> Result<Self> {
        let record = Self::read_record(&home)?;
        let mut setup = Self {
            home,
            record,
            phrase: None,
            incomplete: true,
            consent_confirmed: false,
            pre_spawn_cleanup_unconfirmed: false,
            _lock: Arc::new(lock),
        };
        setup.inspect_ready(terminal)?;
        Ok(setup)
    }

    fn inspect_ready(&mut self, terminal: bool) -> Result<()> {
        if terminal {
            self.workspace_reserved()
                .map_err(|_| anyhow::anyhow!("Invalid setup workspace"))?;
        }
        // Missing ready is incomplete. Existing but malformed ready is an error,
        // never a reason to erase the stored acknowledgement or retry creation.
        match fs::symlink_metadata(self.record.workspace.join("bootstrap-ready.json")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure!(
                    self.record.public.is_none() && !self.record.acknowledged,
                    "Previously ready account is now missing its receipt"
                );
            }
            Err(e) => return Err(e.into()),
            Ok(_) => {
                let info = self.ready()?;
                if terminal {
                    ensure!(
                        info.root_key_protection == "windows-dpapi-current-user",
                        "Invalid Root protection"
                    );
                }
                if self.record.public.is_none() {
                    self.update(|r| r.public = Some(info))?;
                }
                self.incomplete = false;
            }
        }
        super::consent::review(self, false)?;
        if let Some(state) = &self.record.connection {
            state.validate()?;
        }
        Ok(())
    }
    pub(super) fn read_record(home: &Path) -> Result<Record> {
        let record = serde_json::from_slice(&read(&home.join("setup.json"), RECORD_CAP)?)?;
        Self::validate(&record, home)?;
        Ok(record)
    }
    fn check_current(&self) -> Result<()> {
        let current: Record =
            serde_json::from_slice(&read(&self.home.join("setup.json"), RECORD_CAP)?)?;
        ensure!(
            current == self.record,
            "Setup changed on disk; reopen it before continuing"
        );
        Ok(())
    }

    pub(super) fn update(&mut self, f: impl FnOnce(&mut Record)) -> Result<()> {
        self.check_current()?;
        let mut next = self.record.clone();
        f(&mut next);
        Self::validate(&next, &self.home)?;
        if let Some(state) = &next.connection {
            state.validate()?;
        }
        let raw = serde_json::to_vec_pretty(&next)?;
        ensure!(raw.len() <= RECORD_CAP, "Setup record cap");
        write(&self.home.join("setup.json"), &raw, true)?;
        self.record = next;
        Ok(())
    }
    pub fn ready(&self) -> Result<Ordered2BootstrapPublic> {
        let parse = |name: &str, status: &str| -> Result<Ordered2BootstrapPublic> {
            let receipt: Receipt =
                serde_json::from_slice(&read(&self.record.workspace.join(name), 64 * 1024)?)?;
            ensure!(
                receipt.version == 1
                    && receipt.profile_mode == ORDERED2_BOOTSTRAP_MODE
                    && receipt.status == status
                    && receipt.recovery_scope == ORDERED2_RECOVERY_SCOPE,
                "Account ready receipt is invalid"
            );
            receipt.public.validate()?;
            Ok(receipt.public)
        };
        let info = parse("bootstrap-ready.json", "ready-runtime6")?;
        ensure!(
            info == parse(
                "bootstrap-candidate.json",
                "candidate-validated-before-commit"
            )? && info.workspace_dir == self.record.workspace
                && info.contexts == self.record.contexts,
            "Ready/public setup bindings differ"
        );
        if let Some(saved) = &self.record.public {
            ensure!(saved == &info, "Ready identity changed since creation");
        }
        let identity = Self::identity(&info)?;
        ensure!(
            identity.account == info.account_id && identity.device == info.device_id,
            "Ready credential identity mismatch"
        );
        let credential = kilogram_protocol::ordered_dialog_v2::ParsedCredentialBundle::parse(
            &read(&info.credential_file, 16 * 1024)?,
        )?
        .verify_root()?;
        ensure!(
            read(&info.certificate_file, 16 * 1024)? == credential.certificate().encode()?
                && read(&info.authority_file, 16 * 1024)? == credential.authority().encode()?,
            "Ready certificate/authority mismatch"
        );
        let devices = kilogram_identity::AccountDeviceListSnapshot::decode_and_verify(&read(
            &info.device_list_file,
            64 * 1024,
        )?)?;
        ensure!(
            devices.account_id() == info.account_id
                && devices.authority_snapshot().encode()? == credential.authority().encode()?
                && devices
                    .certificate_for(info.device_id)
                    .is_some_and(|c| c == credential.certificate()),
            "Ready signed device-list mismatch"
        );
        ensure!(
            info.state_dir.is_dir() && info.account_root_dir.is_dir(),
            "Ready directories missing"
        );
        if let Some(peer) = &self.record.peer {
            ensure!(
                peer.account != identity.account && peer.device != identity.device,
                "Setup cannot select itself as peer"
            );
        }
        Ok(info)
    }
    pub fn accept(&mut self, mut output: Ordered2BootstrapOutput, generation: &str) -> Result<()> {
        ensure!(
            generation == self.record.local_id
                && !self.record.acknowledged
                && self.phrase.is_none(),
            "Stale account-helper result ignored"
        );
        ensure!(
            output.public().workspace_dir == self.record.workspace
                && output.public().contexts == self.record.contexts,
            "Account-helper request mismatch"
        );
        let info = self.ready()?;
        ensure!(
            &info == output.public(),
            "Account-helper public receipt mismatch"
        );
        self.update(|r| r.public = Some(info))?;
        self.phrase = Some(output.take_recovery_phrase());
        self.incomplete = false;
        Ok(())
    }
    pub fn acknowledge(&mut self, explicitly_saved: bool) -> Result<()> {
        ensure!(
            explicitly_saved && !self.incomplete,
            "Confirm you actually saved the recovery words"
        );
        let _ = self.ready()?;
        self.update(|r| r.acknowledged = true)?;
        self.phrase = None;
        Ok(())
    }
    pub(super) fn identity(info: &Ordered2BootstrapPublic) -> Result<Identity> {
        Identity::new(
            &read(&info.credential_file, 16 * 1024)?,
            &read(&info.authority_file, 16 * 1024)?,
        )
    }
    pub fn export(&self) -> Result<PathBuf> {
        Ok(self.export_artifact(false)?.0)
    }

    pub fn export_terminal(&self) -> std::result::Result<(PathBuf, Artifact), ExportError> {
        self.export_artifact(true)
    }

    fn public_artifact(&self) -> Result<Artifact> {
        ensure!(
            self.record.acknowledged && self.phrase.is_none(),
            "Save and hide recovery words before public export"
        );
        let info = self.ready()?;
        let sender = Self::identity(&info)?;
        let kind = if self.record.joined {
            Kind::Reply
        } else {
            Kind::Invite
        };
        let recipient = if self.record.joined {
            self.record.peer.as_ref().map(|p| Recipient {
                account: p.account,
                device: p.device,
            })
        } else {
            None
        };
        Ok(Artifact {
            version: 1,
            kind,
            mode: ORDERED2_BOOTSTRAP_MODE.into(),
            invitation: self.record.invitation.clone(),
            contexts: self.record.contexts.clone(),
            sender,
            recipient,
        })
    }

    fn export_artifact(
        &self,
        terminal: bool,
    ) -> std::result::Result<(PathBuf, Artifact), ExportError> {
        let artifact = self.public_artifact()?;
        let bytes = artifact.encode()?;
        let public = self.home.join("exchange");
        let path = public.join(if self.record.joined {
            "reply.kilogram"
        } else {
            "invite.kilogram"
        });
        let check_path = || {
            if terminal {
                super::connection::validate_artifact_path(&self.home, &self.record, &path)?;
            }
            Ok::<(), anyhow::Error>(())
        };
        check_path()?;
        fs::create_dir_all(&public).map_err(anyhow::Error::from)?;
        check_path()?;
        match write(&path, &bytes, false) {
            Ok(()) => (),
            Err(error) => {
                check_path()?;
                match fs::symlink_metadata(&path) {
                    Ok(meta)
                        if meta.is_file()
                            && read(&path, artifact::FILE_CAP).is_ok_and(|raw| raw == bytes) =>
                    {
                        ()
                    }
                    Ok(_) => return Err(ExportError::Conflict(error)),
                    Err(_) => return Err(ExportError::Failed(error)),
                }
            }
        }
        Ok((path, artifact))
    }
    pub fn check_reply(&self, reply: &Artifact) -> Result<()> {
        Ok(self.check_reply_checked(reply)?)
    }

    pub fn check_reply_checked(&self, reply: &Artifact) -> std::result::Result<(), ReplyError> {
        reply.validate().map_err(ReplyError::Invalid)?;
        let info = self.record.public.as_ref().ok_or(ReplyError::NotReady)?;
        if self.record.joined
            || !self.record.acknowledged
            || reply.kind != Kind::Reply
            || reply.invitation != self.record.invitation
            || reply.contexts != self.record.contexts
            || reply.recipient
                != Some(Recipient {
                    account: info.account_id,
                    device: info.device_id,
                })
        {
            return Err(ReplyError::Mismatch);
        }
        if self
            .record
            .peer
            .as_ref()
            .is_some_and(|peer| peer != &reply.sender)
        {
            return Err(ReplyError::PeerConflict);
        }
        Ok(())
    }
    pub fn select_reply(&mut self, reply: Artifact, confirmed: bool) -> Result<()> {
        Ok(self.select_reply_checked(reply, confirmed)?)
    }

    pub fn select_reply_checked(
        &mut self,
        reply: Artifact,
        confirmed: bool,
    ) -> std::result::Result<(), ReplyError> {
        if !confirmed {
            return Err(ReplyError::Unconfirmed);
        }
        self.check_reply_checked(&reply)?;
        if self.record.peer.is_some() {
            self.check_current().map_err(ReplyError::Persist)?;
            return Ok(());
        }
        self.update(|r| r.peer = Some(reply.sender))
            .map_err(ReplyError::Persist)
    }
}
