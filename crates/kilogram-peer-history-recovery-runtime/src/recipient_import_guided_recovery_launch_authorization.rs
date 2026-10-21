//! Explicit, short-lived local authorization for launching one guided
//! recipient-import recovery assembly.
//!
//! This layer is deliberately dormant. It has no IPC, CLI, desktop, timer,
//! background, filesystem-discovery, or network registration.

use thiserror::Error;

use crate::{
    RecipientImportGuidedRecoveryAssemblyIssuer,
    RecipientImportGuidedRecoveryHostInstallationOwner,
    RecipientImportGuidedSessionControllerError,
};

const LAUNCH_CONFIRMATION_DOMAIN: &[u8] =
    b"kilogram:recipient-import-guided-recovery-launch-confirmation:v1\0";

pub const MAX_GUIDED_RECIPIENT_IMPORT_LAUNCH_AUTHORIZATION_LIFETIME_SECONDS: u64 = 5 * 60;

/// Safe UI projection for one process-local launch decision.
///
/// It intentionally contains no path, Account/Device ID, descriptor, signer,
/// coordinator, request, nonce, membership snapshot, or authority object.
#[derive(Eq, PartialEq)]
pub struct RecipientImportGuidedRecoveryLaunchPreview {
    membership_count: usize,
    expires_at_unix_seconds: u64,
    confirmation_phrase: String,
}

impl RecipientImportGuidedRecoveryLaunchPreview {
    pub fn membership_count(&self) -> usize {
        self.membership_count
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }

    pub fn confirmation_phrase(&self) -> &str {
        &self.confirmation_phrase
    }
}

/// Typed local approval. It carries only the phrase copied from the safe
/// preview and never carries recovery authority or assembly fragments.
pub struct RecipientImportGuidedRecoveryLaunchApproval {
    confirmation_phrase: String,
}

impl RecipientImportGuidedRecoveryLaunchApproval {
    pub fn new(confirmation_phrase: impl Into<String>) -> Self {
        Self {
            confirmation_phrase: confirmation_phrase.into(),
        }
    }
}

/// One-shot custody for exactly one authenticated assembly issuer.
///
/// The authorization deliberately implements neither Clone, Debug, Serialize,
/// nor Deserialize. Approval, cancellation, expiry, mismatch, and downstream
/// assembly failure all consume the value, making the grant unavailable for
/// replay. Process restart loses it.
pub struct RecipientImportGuidedRecoveryLaunchAuthorization<'identity> {
    issuer: RecipientImportGuidedRecoveryAssemblyIssuer<'identity>,
    preview: RecipientImportGuidedRecoveryLaunchPreview,
    issued_at_unix_seconds: u64,
}

impl<'identity> RecipientImportGuidedRecoveryLaunchAuthorization<'identity> {
    /// Internal entry from the already authenticated one-shot issuer.
    ///
    /// Keeping this constructor crate-private prevents public callers from
    /// assembling launch authority from loose paths, identities, or payloads.
    #[allow(dead_code)]
    pub(crate) fn from_authenticated_issuer(
        issuer: RecipientImportGuidedRecoveryAssemblyIssuer<'identity>,
        issued_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<Self, RecipientImportGuidedRecoveryLaunchAuthorizationError> {
        let binding = issuer.launch_binding()?;
        if issued_at_unix_seconds == 0
            || !(1..=MAX_GUIDED_RECIPIENT_IMPORT_LAUNCH_AUTHORIZATION_LIFETIME_SECONDS)
                .contains(&lifetime_seconds)
        {
            return Err(RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime);
        }
        let expires_at_unix_seconds = issued_at_unix_seconds
            .checked_add(lifetime_seconds)
            .ok_or(RecipientImportGuidedRecoveryLaunchAuthorizationError::InvalidLifetime)?;
        let (assembly_digest, membership_count, membership_count_u64) = binding.into_parts();
        let confirmation_phrase = confirmation_phrase(
            assembly_digest,
            membership_count_u64,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        );
        Ok(Self {
            issuer,
            preview: RecipientImportGuidedRecoveryLaunchPreview {
                membership_count,
                expires_at_unix_seconds,
                confirmation_phrase,
            },
            issued_at_unix_seconds,
        })
    }

    pub fn preview(&self) -> &RecipientImportGuidedRecoveryLaunchPreview {
        &self.preview
    }

    /// Consumes the authorization before checking the fallible approval path.
    pub fn approve(
        self,
        approval: RecipientImportGuidedRecoveryLaunchApproval,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedRecoveryHostInstallationOwner<'identity>,
        RecipientImportGuidedRecoveryLaunchAuthorizationError,
    > {
        let Self {
            issuer,
            preview,
            issued_at_unix_seconds,
        } = self;
        if now_unix_seconds < issued_at_unix_seconds {
            return Err(RecipientImportGuidedRecoveryLaunchAuthorizationError::ClockRegressed);
        }
        if now_unix_seconds >= preview.expires_at_unix_seconds {
            return Err(RecipientImportGuidedRecoveryLaunchAuthorizationError::Expired);
        }
        if approval.confirmation_phrase != preview.confirmation_phrase {
            return Err(
                RecipientImportGuidedRecoveryLaunchAuthorizationError::ConfirmationMismatch,
            );
        }
        issuer.issue().map_err(Into::into)
    }

    /// Explicitly consumes and discards the retained issuer.
    pub fn cancel(self) {}
}

fn confirmation_phrase(
    assembly_digest: [u8; 32],
    membership_count: u64,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(LAUNCH_CONFIRMATION_DOMAIN);
    hasher.update(&assembly_digest);
    hasher.update(&membership_count.to_le_bytes());
    hasher.update(&issued_at_unix_seconds.to_le_bytes());
    hasher.update(&expires_at_unix_seconds.to_le_bytes());
    let digest = hasher.finalize();
    let bytes = &digest.as_bytes()[..6];
    format!(
        "RECOVER-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedRecoveryLaunchAuthorizationError {
    #[error("guided recovery launch authorization lifetime is invalid")]
    InvalidLifetime,
    #[error("guided recovery launch authorization clock regressed")]
    ClockRegressed,
    #[error("guided recovery launch authorization expired")]
    Expired,
    #[error("guided recovery launch confirmation phrase does not match")]
    ConfirmationMismatch,
    #[error(transparent)]
    Controller(Box<RecipientImportGuidedSessionControllerError>),
}

impl From<RecipientImportGuidedSessionControllerError>
    for RecipientImportGuidedRecoveryLaunchAuthorizationError
{
    fn from(error: RecipientImportGuidedSessionControllerError) -> Self {
        Self::Controller(Box::new(error))
    }
}
