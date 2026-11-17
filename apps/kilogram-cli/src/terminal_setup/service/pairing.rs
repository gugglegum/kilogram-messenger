use super::{Preview, Service, access_error};
use kilogram_first_dialog::{
    artifact::{self, Artifact, Kind},
    model::{self, ExportError, ReplyError, Setup},
};
use kilogram_identity::{AccountId, DeviceId};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn identity(account: AccountId, device: DeviceId) -> Value {
    json!({"account_id": account.to_string(), "device_id": device.to_string()})
}

fn pins(artifact: &Artifact) -> Value {
    let sender = &artifact.sender;
    json!({
        "kind": artifact.kind, "invitation_id": artifact.invitation, "contexts": artifact.contexts,
        "sender": identity(sender.account, sender.device),
        "recipient": artifact.recipient.as_ref().map(|peer| identity(peer.account, peer.device)),
    })
}

fn ready(setup: &Setup) -> Result<(), &'static str> {
    if !setup.record.acknowledged || setup.phrase.is_some() {
        return Err("recovery_required");
    }
    Ok(())
}

fn reply_error(error: ReplyError) -> &'static str {
    match error {
        ReplyError::Invalid(_) => "artifact_invalid",
        ReplyError::NotReady => "invalid_state",
        ReplyError::Mismatch => "reply_mismatch",
        ReplyError::PeerConflict => "peer_conflict",
        ReplyError::Unconfirmed => "invalid_args",
        ReplyError::Persist(_) => "setup_invalid",
    }
}

impl Service {
    fn reply_setup(&mut self) -> Result<&mut Setup, &'static str> {
        let setup = self.refresh()?;
        if setup.record.joined {
            return Err("invalid_state");
        }
        ready(setup)?;
        Ok(setup)
    }

    pub(super) fn inspect(&mut self, kind: Kind, path: &Path) -> Result<Value, &'static str> {
        self.preview = None;
        match kind {
            Kind::Invite if self.setup.is_some() => return Err("invalid_state"),
            Kind::Reply => {
                self.reply_setup()?;
            }
            _ => {}
        }
        let raw = model::read(path, artifact::FILE_CAP).map_err(|_| "artifact_unreadable")?;
        let artifact = Artifact::decode(&raw).map_err(|_| "artifact_invalid")?;
        if artifact.kind != kind {
            return Err("wrong_artifact_kind");
        }
        if kind == Kind::Reply {
            self.setup
                .as_ref()
                .ok_or("not_open")?
                .check_reply_checked(&artifact)
                .map_err(reply_error)?;
        }
        let id = artifact::id().map_err(|_| "setup_invalid")?;
        let mut value = pins(&artifact);
        value["preview_id"] = json!(id);
        value["verification"] = json!({
            "credential_signatures_verified": true, "envelope_signed": false,
            "independent_verification_required": true, "connection_authenticated": false,
        });
        self.preview = Some(Preview::Pairing { id, artifact });
        Ok(value)
    }

    pub(super) fn confirm(&mut self, kind: Kind, id: &str) -> Result<(), &'static str> {
        if !matches!(
            &self.preview,
            Some(Preview::Pairing { id: old, artifact }) if old == id && artifact.kind == kind
        ) {
            return Err("preview_required");
        }
        let Some(Preview::Pairing { artifact, .. }) = self.preview.take() else {
            unreachable!("checked pairing preview");
        };
        match kind {
            Kind::Invite => {
                if self.setup.is_some() {
                    return Err("invalid_state");
                }
                self.setup =
                    Some(Setup::join_terminal(&self.home, artifact).map_err(access_error)?);
            }
            Kind::Reply => self
                .reply_setup()?
                .select_reply_checked(artifact, true)
                .map_err(reply_error)?,
        }
        Ok(())
    }

    pub(super) fn export(&mut self, kind: Kind) -> Result<Value, &'static str> {
        let setup = self.refresh()?;
        if setup.record.joined != (kind == Kind::Reply) {
            return Err("invalid_state");
        }
        ready(setup)?;
        let (path, artifact) = setup.export_terminal().map_err(|error| match error {
            ExportError::Conflict(_) => "export_conflict",
            ExportError::Failed(_) => "export_failed",
        })?;
        let mut value = pins(&artifact);
        value["path"] = json!(path.to_str().ok_or("export_failed")?);
        Ok(value)
    }
}
