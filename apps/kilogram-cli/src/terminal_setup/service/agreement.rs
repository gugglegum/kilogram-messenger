use super::{Preview, Service, helper_error};
use kilogram_first_dialog::{
    artifact, consent,
    consent::{Action, Failure, Outcome},
    consent_artifact::{Envelope, Kind},
    helper::HelperError,
    model::{self, Setup},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

fn code(error: &anyhow::Error, fallback: &'static str) -> &'static str {
    if let Some(error) = error.downcast_ref::<HelperError>() {
        return if matches!(error, HelperError::Failed(_)) {
            fallback
        } else {
            helper_error(*error)
        };
    }
    match error.downcast_ref::<Failure>() {
        Some(Failure::InvalidArtifact) => "artifact_invalid",
        Some(Failure::Mismatch) => "consent_mismatch",
        Some(Failure::WrongRole) => "wrong_consent_role",
        Some(Failure::NotReady) => "consent_not_ready",
        Some(Failure::NotCurrent) => "consent_not_current",
        Some(Failure::Conflict) => "consent_conflict",
        Some(Failure::ExportConflict) => "export_conflict",
        None => fallback,
    }
}

fn outcome(value: Outcome) -> &'static str {
    match value {
        Outcome::NotStarted => "not_started",
        Outcome::Started => "started",
        Outcome::Accepted => "accepted",
        Outcome::Failed => "failed",
    }
}

pub(super) fn snapshot(setup: &Setup) -> Result<Option<Value>, &'static str> {
    let Ok(role) = consent::role(setup) else {
        return Ok(None);
    };
    let state = setup.record.consent.as_ref();
    let interval = state
        .map(|_| consent::period(setup).map_err(|_| "setup_invalid"))
        .transpose()?;
    let stage = match state {
        None => "not_started",
        Some(s) if s.final_artifact.is_some() => "final_saved",
        Some(s) if s.proposal.is_some() => "proposal_selected",
        Some(s) if role == 0 && s.slots.iter().all(|s| s.output.is_some()) => "proposal_ready",
        Some(_) => "in_progress",
    };
    let slots = std::array::from_fn::<_, 2, _>(|index| {
        let slot = state.map(|s| &s.slots[index]);
        json!({
            "context_id": setup.record.contexts[index],
            "signing": outcome(slot.map_or(Outcome::NotStarted, |s| s.signing)),
            "importing": outcome(slot.map_or(Outcome::NotStarted, |s| s.importing)),
        })
    });
    let installation =
        if state.is_some_and(|s| s.slots.iter().all(|s| s.importing == Outcome::Accepted)) {
            if setup.consent_confirmed {
                "confirmed_current_session"
            } else {
                "previously_accepted"
            }
        } else {
            "unconfirmed"
        };
    Ok(Some(json!({
        "role": if role == 0 { "proposer" } else { "cosigner" },
        "stage": stage,
        "not_before": interval.map(|(start, _)| start),
        "expires_at": interval.map(|(_, end)| end),
        "selected_proposal_digest": state.and_then(|s| s.proposal.as_ref()).map(|r| &r.digest),
        "final_digest": state.and_then(|s| s.final_artifact.as_ref()).map(|r| &r.digest),
        "slots": slots,
        "local_installation": installation,
    })))
}

impl Service {
    pub(super) fn consent_setup(&mut self) -> Result<&mut Setup, &'static str> {
        let setup = self.refresh()?;
        if setup.incomplete {
            return Err("invalid_state");
        }
        if !setup.record.acknowledged || setup.phrase.is_some() {
            return Err("recovery_required");
        }
        if setup.record.peer.is_none() {
            return Err("peer_required");
        }
        consent::role(setup).map_err(|_| "invalid_state")?;
        Ok(setup)
    }

    pub(super) fn inspect_consent(
        &mut self,
        kind: Kind,
        path: &Path,
    ) -> Result<Value, &'static str> {
        self.preview = None;
        let setup = self.consent_setup()?;
        let raw = model::read(path, artifact::FILE_CAP).map_err(|_| "artifact_unreadable")?;
        let envelope = Envelope::decode(&raw).map_err(|_| "artifact_invalid")?;
        if envelope.kind != kind {
            return Err("wrong_artifact_kind");
        }
        let public = consent::preview(setup, &envelope).map_err(|e| code(&e, "consent_failed"))?;
        let id = artifact::id().map_err(|_| "consent_failed")?;
        let mut value = json!(public);
        value["preview_id"] = json!(id);
        self.preview = Some(Preview::Consent { id, envelope });
        Ok(value)
    }

    pub(super) fn accept_consent(&mut self, kind: Kind, id: &str) -> Result<(), &'static str> {
        if !matches!(
            &self.preview,
            Some(Preview::Consent { id: old, envelope }) if old == id && envelope.kind == kind
        ) {
            return Err("preview_required");
        }
        let Some(Preview::Consent { envelope, .. }) = self.preview.take() else {
            unreachable!("checked consent preview");
        };
        consent::select(self.consent_setup()?, &envelope).map_err(|e| code(&e, "consent_failed"))
    }

    pub(super) fn export_consent(&mut self) -> Result<Value, &'static str> {
        let (path, public) =
            consent::export_public(self.consent_setup()?).map_err(|e| code(&e, "export_failed"))?;
        let mut value = json!(public);
        value["path"] = json!(path.to_str().ok_or("export_failed")?);
        Ok(value)
    }

    pub(super) fn run_consent(
        &mut self,
        action: Action,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), &'static str> {
        let cli = self.cli.clone();
        consent::execute(self.consent_setup()?, &cli, action, cancel).map_err(|error| {
            let fallback = match (error.downcast_ref::<HelperError>(), action) {
                (Some(HelperError::Failed(_)), Action::Import) => "consent_import_failed",
                (Some(HelperError::Failed(_)), _) => "consent_sign_failed",
                _ => "consent_failed",
            };
            code(&error, fallback)
        })
    }
}
