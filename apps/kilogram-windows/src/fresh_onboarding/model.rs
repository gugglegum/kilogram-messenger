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
};
use zeroize::Zeroizing;

const RECORD_CAP: usize = 128 * 1024;
pub(super) fn read(path: &Path, cap: usize) -> Result<Vec<u8>> {
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
fn write(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
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
pub(super) struct Record {
    pub version: u8,
    pub local_id: String,
    pub invitation: String,
    pub contexts: [String; 2],
    pub workspace: PathBuf,
    pub peer: Option<Identity>,
    pub joined: bool,
    pub acknowledged: bool,
    pub public: Option<Ordered2BootstrapPublic>,
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
pub(super) struct Setup {
    pub home: PathBuf,
    pub record: Record,
    pub phrase: Option<Zeroizing<String>>,
    pub incomplete: bool,
    _lock: File,
}
impl Setup {
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
        Ok(())
    }
    pub fn begin(home: &Path, invite: Option<Artifact>) -> Result<Self> {
        if let Some(invite) = &invite {
            invite.validate()?;
            ensure!(
                invite.kind == Kind::Invite,
                "Join requires an invitation, not a reply"
            );
        }
        let (home, lock) = Self::lock(home)?;
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
            _lock: lock,
        })
    }
    pub fn open(home: &Path) -> Result<Self> {
        let (home, lock) = Self::lock(home)?;
        let record: Record = serde_json::from_slice(&read(&home.join("setup.json"), RECORD_CAP)?)?;
        Self::validate(&record, &home)?;
        let mut setup = Self {
            home,
            record,
            phrase: None,
            incomplete: true,
            _lock: lock,
        };
        // Missing ready is incomplete. Existing but malformed ready is an error,
        // never a reason to erase the stored acknowledgement or retry creation.
        match fs::symlink_metadata(setup.record.workspace.join("bootstrap-ready.json")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure!(
                    setup.record.public.is_none() && !setup.record.acknowledged,
                    "Previously ready account is now missing its receipt"
                );
            }
            Err(e) => return Err(e.into()),
            Ok(_) => {
                let info = setup.ready()?;
                if setup.record.public.is_none() {
                    setup.update(|r| r.public = Some(info))?;
                }
                setup.incomplete = false;
            }
        }
        Ok(setup)
    }
    fn update(&mut self, f: impl FnOnce(&mut Record)) -> Result<()> {
        let current: Record =
            serde_json::from_slice(&read(&self.home.join("setup.json"), RECORD_CAP)?)?;
        ensure!(
            current == self.record,
            "Setup changed on disk; reopen it before continuing"
        );
        let mut next = self.record.clone();
        f(&mut next);
        Self::validate(&next, &self.home)?;
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
    fn identity(info: &Ordered2BootstrapPublic) -> Result<Identity> {
        Identity::new(
            &read(&info.credential_file, 16 * 1024)?,
            &read(&info.authority_file, 16 * 1024)?,
        )
    }
    pub fn export(&self) -> Result<PathBuf> {
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
        let artifact = Artifact {
            version: 1,
            kind,
            mode: ORDERED2_BOOTSTRAP_MODE.into(),
            invitation: self.record.invitation.clone(),
            contexts: self.record.contexts.clone(),
            sender,
            recipient,
        };
        let bytes = artifact.encode()?;
        let public = self.home.join("exchange");
        fs::create_dir_all(&public)?;
        let path = public.join(if self.record.joined {
            "reply.kilogram"
        } else {
            "invite.kilogram"
        });
        match write(&path, &bytes, false) {
            Ok(()) => (),
            Err(e) => {
                ensure!(
                    path.is_file() && read(&path, artifact::FILE_CAP)? == bytes,
                    "Public export already exists with different contents: {e}"
                );
            }
        }
        Ok(path)
    }
    pub fn check_reply(&self, reply: &Artifact) -> Result<()> {
        reply.validate()?;
        let info = self
            .record
            .public
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Account is not ready"))?;
        ensure!(
            !self.record.joined
                && self.record.acknowledged
                && reply.kind == Kind::Reply
                && reply.invitation == self.record.invitation
                && reply.contexts == self.record.contexts
                && reply.recipient
                    == Some(Recipient {
                        account: info.account_id,
                        device: info.device_id
                    }),
            "Reply belongs to a different invitation or recipient"
        );
        if let Some(peer) = &self.record.peer {
            ensure!(
                peer == &reply.sender,
                "A different peer was already selected"
            );
        }
        Ok(())
    }
    pub fn select_reply(&mut self, reply: Artifact, confirmed: bool) -> Result<()> {
        ensure!(confirmed, "Compare the peer identity independently first");
        self.check_reply(&reply)?;
        self.update(|r| r.peer = Some(reply.sender))
    }
}
