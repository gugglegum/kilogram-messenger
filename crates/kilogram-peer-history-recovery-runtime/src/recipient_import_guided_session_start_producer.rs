//! One-shot hand-off from an authenticated recovery owner into local UX.
//!
//! The hand-off and resulting start command are process-local Rust values.
//! Neither can be reconstructed from IPC, CLI, environment, filesystem, or
//! independently supplied authority fragments.

use thiserror::Error;

use crate::recipient_import_guided_session_controller::MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS;
use crate::{
    AuthenticatedGuidedRecipientImportSession, RecipientImportGuidedSessionController,
    RecipientImportGuidedSessionControllerError, RecipientImportGuidedSessionLocalUxStart,
};

/// Sealed authority owned by the already authenticated recovery ceremony.
///
/// The value intentionally has no public constructor, Clone, Debug,
/// Serialize, or Deserialize implementation. Production integration may only
/// receive it from the in-process recovery owner. A production attempt takes
/// the authority before any fallible check, so success, rejection, expiry, and
/// cancellation are all one-shot outcomes.
pub struct AuthenticatedGuidedRecipientImportSessionStartHandoff {
    authority: Option<AuthenticatedGuidedRecipientImportSession>,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

impl AuthenticatedGuidedRecipientImportSessionStartHandoff {
    #[allow(dead_code)]
    pub(crate) fn from_authenticated_recovery(
        authority: AuthenticatedGuidedRecipientImportSession,
        issued_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<Self, RecipientImportGuidedSessionStartProducerError> {
        if issued_at_unix_seconds == 0
            || !(1..=MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS)
                .contains(&lifetime_seconds)
        {
            return Err(RecipientImportGuidedSessionStartProducerError::InvalidHandoffLifetime);
        }
        let expires_at_unix_seconds = issued_at_unix_seconds
            .checked_add(lifetime_seconds)
            .ok_or(RecipientImportGuidedSessionStartProducerError::InvalidHandoffLifetime)?;
        Ok(Self {
            authority: Some(authority),
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        })
    }

    pub fn consumed(&self) -> bool {
        self.authority.is_none()
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }

    pub fn cancel(&mut self) -> bool {
        self.authority.take().is_some()
    }
}

/// Converts one sealed owner hand-off into one local-UX start command.
///
/// It borrows the exact controller that will later consume the command and
/// repeats its complete read-only Plan preflight before releasing authority.
pub struct RecipientImportGuidedSessionStartProducer<'controller, 'identity> {
    controller: &'controller mut RecipientImportGuidedSessionController<'identity>,
}

impl<'controller, 'identity> RecipientImportGuidedSessionStartProducer<'controller, 'identity> {
    pub fn new(
        controller: &'controller mut RecipientImportGuidedSessionController<'identity>,
    ) -> Self {
        Self { controller }
    }

    pub fn produce(
        &mut self,
        handoff: &mut AuthenticatedGuidedRecipientImportSessionStartHandoff,
        now_unix_seconds: u64,
        session_lifetime_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxStart,
        RecipientImportGuidedSessionStartProducerError,
    > {
        let authority = handoff
            .authority
            .take()
            .ok_or(RecipientImportGuidedSessionStartProducerError::HandoffConsumed)?;
        if now_unix_seconds < handoff.issued_at_unix_seconds {
            return Err(RecipientImportGuidedSessionStartProducerError::HandoffTimeInvalid);
        }
        if now_unix_seconds >= handoff.expires_at_unix_seconds {
            return Err(RecipientImportGuidedSessionStartProducerError::HandoffExpired);
        }
        if !(1..=MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS)
            .contains(&session_lifetime_seconds)
        {
            return Err(RecipientImportGuidedSessionStartProducerError::InvalidSessionLifetime);
        }
        let session_expires_at = now_unix_seconds
            .checked_add(session_lifetime_seconds)
            .ok_or(RecipientImportGuidedSessionStartProducerError::InvalidSessionLifetime)?;
        if session_expires_at > handoff.expires_at_unix_seconds {
            return Err(RecipientImportGuidedSessionStartProducerError::SessionOutlivesHandoff);
        }
        self.controller.preflight_local_ux_start(
            &authority,
            now_unix_seconds,
            session_lifetime_seconds,
        )?;
        Ok(
            RecipientImportGuidedSessionLocalUxStart::from_authenticated_recovery(
                authority,
                session_lifetime_seconds,
            ),
        )
    }

    pub fn opened_network(&self) -> bool {
        false
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedSessionStartProducerError {
    #[error("guided recipient-import start hand-off lifetime is invalid")]
    InvalidHandoffLifetime,
    #[error("guided recipient-import start hand-off was already consumed")]
    HandoffConsumed,
    #[error("guided recipient-import start hand-off clock moved before issuance")]
    HandoffTimeInvalid,
    #[error("guided recipient-import start hand-off expired")]
    HandoffExpired,
    #[error("guided recipient-import session lifetime is invalid")]
    InvalidSessionLifetime,
    #[error("guided recipient-import session would outlive its start hand-off")]
    SessionOutlivesHandoff,
    #[error(transparent)]
    Controller(Box<RecipientImportGuidedSessionControllerError>),
}

impl From<RecipientImportGuidedSessionControllerError>
    for RecipientImportGuidedSessionStartProducerError
{
    fn from(error: RecipientImportGuidedSessionControllerError) -> Self {
        Self::Controller(Box::new(error))
    }
}
