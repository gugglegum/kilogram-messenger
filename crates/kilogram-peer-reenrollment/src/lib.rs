#![forbid(unsafe_code)]

//! Offline, Device-authorized planning for recovery from a quarantined peer
//! capability chain.
//!
//! This crate does not mutate the peer capability store, contact state,
//! runtime IPC or transport. It validates an exact Root-signed authority view,
//! a fresh Device-signed current invitation and the exact durable quarantine
//! head, then emits a locally Device-signed tombstone plan. Applying that plan
//! remains outside this pure planning crate.

use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, DeviceCapability, DeviceId, DeviceIdentity, IdentityError,
};
use kilogram_peer_capability_store::DurablePeerCapabilityQuarantineHead;
use kilogram_sync_capability::{
    PeerSyncCapabilityFloor, PeerSyncCapabilityQuarantineReason, SignedCurrentPeerInvitation,
    SyncTransportCapabilityError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PLAN_VERSION: u8 = 1;
const PLAN_SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-tombstone:v1\0";
const PLAN_ID_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-plan-id:v1\0";
const AUTHORITY_DIGEST_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-authority:v1\0";
pub const DEFAULT_PEER_REENROLLMENT_PLAN_VALIDITY_SECONDS: u64 = 15 * 60;
pub const MAX_PEER_REENROLLMENT_PLAN_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitPeerReenrollmentApproval {
    NotAuthorized,
    UserOrTestOperatorApproved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentPlanContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    local_authority_revision: u64,
    local_authority_digest: [u8; 32],
    peer_account_id: AccountId,
    quarantined_peer_device_id: DeviceId,
    quarantine_record_id: [u8; 32],
    quarantine_sequence: u64,
    quarantine_reason: PeerSyncCapabilityQuarantineReason,
    predecessor_record_id: [u8; 32],
    predecessor_floor: PeerSyncCapabilityFloor,
    peer_authority_revision: u64,
    peer_authority_digest: [u8; 32],
    replacement_peer_device_id: DeviceId,
    invitation_id: [u8; 32],
    required_capability_id: [u8; 32],
    ticket_publication_epoch: u64,
    invitation_issued_at_unix_seconds: u64,
    invitation_expires_at_unix_seconds: u64,
    prepared_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    nonce: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedPeerReenrollmentTombstoneWire {
    content: PeerReenrollmentPlanContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReenrollmentPlan {
    content: PeerReenrollmentPlanContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPeerReenrollmentTombstone {
    content: PeerReenrollmentPlanContent,
    signature: Vec<u8>,
}

impl PeerReenrollmentPlan {
    pub fn prepare(
        local_device_id: DeviceId,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
    ) -> Result<Self, PeerReenrollmentError> {
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(PeerReenrollmentError::SecureRandom)?;
        Self::prepare_with_nonce(
            local_device_id,
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            now_unix_seconds,
            nonce,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_with_nonce(
        local_device_id: DeviceId,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
        nonce: [u8; 32],
    ) -> Result<Self, PeerReenrollmentError> {
        let content = build_validated_content(
            local_device_id,
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            now_unix_seconds,
            nonce,
        )?;
        Ok(Self { content })
    }

    pub fn plan_id(&self) -> Result<[u8; 32], PeerReenrollmentError> {
        plan_id(&self.content)
    }

    pub fn decode_and_validate(
        encoded: &[u8],
        expected_local_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES {
            return Err(PeerReenrollmentError::ArtifactLength(encoded.len()));
        }
        let content: PeerReenrollmentPlanContent =
            postcard::from_bytes(encoded).map_err(PeerReenrollmentError::Codec)?;
        validate_content(&content)?;
        if content.local_device_id != expected_local_device_id {
            return Err(PeerReenrollmentError::WrongLocalDevice);
        }
        let plan = Self { content };
        if plan.encode()? != encoded {
            return Err(PeerReenrollmentError::NonCanonicalArtifact);
        }
        Ok(plan)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentError> {
        validate_content(&self.content)?;
        let encoded = postcard::to_allocvec(&self.content).map_err(PeerReenrollmentError::Codec)?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES {
            return Err(PeerReenrollmentError::ArtifactLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn confirmation_phrase(&self) -> Result<String, PeerReenrollmentError> {
        Ok(confirmation_phrase(self.plan_id()?))
    }

    pub fn confirm_and_sign(
        self,
        identity: &DeviceIdentity,
        supplied_confirmation: &str,
        approval: ExplicitPeerReenrollmentApproval,
    ) -> Result<SignedPeerReenrollmentTombstone, PeerReenrollmentError> {
        if approval != ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved {
            return Err(PeerReenrollmentError::ExplicitApprovalRequired);
        }
        if identity.device_id() != self.content.local_device_id {
            return Err(PeerReenrollmentError::WrongLocalDevice);
        }
        if supplied_confirmation != self.confirmation_phrase()? {
            return Err(PeerReenrollmentError::ConfirmationMismatch);
        }
        let signature = identity.sign(&signing_bytes(&self.content)?).to_vec();
        let tombstone = SignedPeerReenrollmentTombstone {
            content: self.content,
            signature,
        };
        tombstone.verify_signature()?;
        Ok(tombstone)
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn quarantined_peer_device_id(&self) -> DeviceId {
        self.content.quarantined_peer_device_id
    }

    pub fn replacement_peer_device_id(&self) -> DeviceId {
        self.content.replacement_peer_device_id
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

impl SignedPeerReenrollmentTombstone {
    pub fn decode_and_verify(
        encoded: &[u8],
        expected_local_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES {
            return Err(PeerReenrollmentError::ArtifactLength(encoded.len()));
        }
        let wire: SignedPeerReenrollmentTombstoneWire =
            postcard::from_bytes(encoded).map_err(PeerReenrollmentError::Codec)?;
        let tombstone = Self {
            content: wire.content,
            signature: wire.signature,
        };
        if tombstone.local_device_id() != expected_local_device_id {
            return Err(PeerReenrollmentError::WrongLocalDevice);
        }
        tombstone.verify_signature()?;
        if tombstone.encode()? != encoded {
            return Err(PeerReenrollmentError::NonCanonicalArtifact);
        }
        Ok(tombstone)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentError> {
        self.verify_signature()?;
        let encoded = postcard::to_allocvec(&SignedPeerReenrollmentTombstoneWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })
        .map_err(PeerReenrollmentError::Codec)?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES {
            return Err(PeerReenrollmentError::ArtifactLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn plan_id(&self) -> Result<[u8; 32], PeerReenrollmentError> {
        plan_id(&self.content)
    }

    pub fn verify_context(
        &self,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
    ) -> Result<(), PeerReenrollmentError> {
        self.verify_bound_context(local_authority, quarantine, peer_authority, invitation)?;
        if now_unix_seconds < self.content.prepared_at_unix_seconds {
            return Err(PeerReenrollmentError::PlanNotYetValid);
        }
        if now_unix_seconds > self.content.expires_at_unix_seconds {
            return Err(PeerReenrollmentError::PlanExpired);
        }
        invitation.verify_at(now_unix_seconds)?;
        if invitation.expires_at_unix_seconds() <= now_unix_seconds {
            return Err(PeerReenrollmentError::InvitationNotFresh);
        }
        Ok(())
    }

    /// Re-verify every exact signed input without imposing the current clock.
    ///
    /// This exists only so a byte-identical replay of an already committed
    /// receipt remains idempotent after the short approval window expires.
    pub fn verify_bound_context(
        &self,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
    ) -> Result<(), PeerReenrollmentError> {
        self.verify_signature()?;
        let expected = build_validated_content(
            self.content.local_device_id,
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            self.content.prepared_at_unix_seconds,
            self.content.nonce,
        )?;
        if expected != self.content {
            return Err(PeerReenrollmentError::ContextMismatch);
        }
        Ok(())
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn quarantined_peer_device_id(&self) -> DeviceId {
        self.content.quarantined_peer_device_id
    }

    pub fn replacement_peer_device_id(&self) -> DeviceId {
        self.content.replacement_peer_device_id
    }

    pub fn quarantine_record_id(&self) -> [u8; 32] {
        self.content.quarantine_record_id
    }

    pub fn predecessor_record_id(&self) -> [u8; 32] {
        self.content.predecessor_record_id
    }

    pub fn predecessor_floor(&self) -> PeerSyncCapabilityFloor {
        self.content.predecessor_floor
    }

    pub fn quarantine_sequence(&self) -> u64 {
        self.content.quarantine_sequence
    }

    pub fn quarantine_reason(&self) -> PeerSyncCapabilityQuarantineReason {
        self.content.quarantine_reason
    }

    pub fn local_authority_revision(&self) -> u64 {
        self.content.local_authority_revision
    }

    pub fn local_authority_digest(&self) -> [u8; 32] {
        self.content.local_authority_digest
    }

    pub fn peer_authority_revision(&self) -> u64 {
        self.content.peer_authority_revision
    }

    pub fn peer_authority_digest(&self) -> [u8; 32] {
        self.content.peer_authority_digest
    }

    pub fn invitation_id(&self) -> [u8; 32] {
        self.content.invitation_id
    }

    pub fn required_capability_id(&self) -> [u8; 32] {
        self.content.required_capability_id
    }

    pub fn ticket_publication_epoch(&self) -> u64 {
        self.content.ticket_publication_epoch
    }

    pub fn prepared_at_unix_seconds(&self) -> u64 {
        self.content.prepared_at_unix_seconds
    }

    pub fn successor_floor(&self) -> PeerSyncCapabilityFloor {
        PeerSyncCapabilityFloor::CurrentInvitationRequired {
            invitation_id: self.content.invitation_id,
            required_capability_id: self.content.required_capability_id,
            minimum_authority_revision: self.content.peer_authority_revision,
            minimum_ticket_publication_epoch: self.content.ticket_publication_epoch,
        }
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    fn verify_signature(&self) -> Result<(), PeerReenrollmentError> {
        validate_content(&self.content)?;
        self.local_device_id()
            .verify(&signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_validated_content(
    local_device_id: DeviceId,
    local_authority: &AccountDeviceListSnapshot,
    quarantine: &DurablePeerCapabilityQuarantineHead,
    peer_authority: &AccountDeviceListSnapshot,
    invitation: &SignedCurrentPeerInvitation,
    now_unix_seconds: u64,
    nonce: [u8; 32],
) -> Result<PeerReenrollmentPlanContent, PeerReenrollmentError> {
    local_authority.verify()?;
    authorize_device(local_authority, local_device_id, true)?;
    peer_authority.verify()?;
    invitation.verify_at(now_unix_seconds)?;
    if invitation.issued_at_unix_seconds() > now_unix_seconds
        || invitation.expires_at_unix_seconds() <= now_unix_seconds
    {
        return Err(PeerReenrollmentError::InvitationNotFresh);
    }
    if local_authority.account_id() == peer_authority.account_id() {
        return Err(PeerReenrollmentError::SameAccount);
    }
    let key = quarantine.key();
    if key.peer_account_id != peer_authority.account_id()
        || key.peer_account_id != invitation.listener_account_id()
    {
        return Err(PeerReenrollmentError::PeerAccountMismatch);
    }
    if invitation.allowed_requester_account_id() != local_authority.account_id() {
        return Err(PeerReenrollmentError::InvitationRequesterMismatch);
    }
    if invitation.authority_revision() != peer_authority.revision() {
        return Err(PeerReenrollmentError::PeerAuthorityRevisionMismatch);
    }
    authorize_device(peer_authority, invitation.listener_device_id(), false)?;
    validate_successor(quarantine, invitation)?;

    let head = quarantine.head();
    let predecessor = quarantine.predecessor();
    let expires_at_unix_seconds = now_unix_seconds
        .saturating_add(DEFAULT_PEER_REENROLLMENT_PLAN_VALIDITY_SECONDS)
        .min(invitation.expires_at_unix_seconds());
    let content = PeerReenrollmentPlanContent {
        version: PLAN_VERSION,
        local_account_id: local_authority.account_id(),
        local_device_id,
        local_authority_revision: local_authority.revision(),
        local_authority_digest: peer_reenrollment_authority_digest(local_authority)?,
        peer_account_id: peer_authority.account_id(),
        quarantined_peer_device_id: key.peer_device_id,
        quarantine_record_id: head.record_id,
        quarantine_sequence: head.sequence,
        quarantine_reason: quarantine.reason(),
        predecessor_record_id: predecessor.record_id,
        predecessor_floor: predecessor.floor,
        peer_authority_revision: peer_authority.revision(),
        peer_authority_digest: peer_reenrollment_authority_digest(peer_authority)?,
        replacement_peer_device_id: invitation.listener_device_id(),
        invitation_id: invitation.invitation_id()?,
        required_capability_id: invitation.required_capability_id(),
        ticket_publication_epoch: invitation.ticket_publication_channel_epoch(),
        invitation_issued_at_unix_seconds: invitation.issued_at_unix_seconds(),
        invitation_expires_at_unix_seconds: invitation.expires_at_unix_seconds(),
        prepared_at_unix_seconds: now_unix_seconds,
        expires_at_unix_seconds,
        nonce,
    };
    validate_content(&content)?;
    Ok(content)
}

fn authorize_device(
    authority: &AccountDeviceListSnapshot,
    device_id: DeviceId,
    local: bool,
) -> Result<(), PeerReenrollmentError> {
    let certificate = authority.certificate_for(device_id).ok_or(if local {
        PeerReenrollmentError::LocalDeviceNotAuthorized
    } else {
        PeerReenrollmentError::ReplacementDeviceNotAuthorized
    })?;
    if !certificate.has_capability(DeviceCapability::SyncHistory) {
        return Err(if local {
            PeerReenrollmentError::LocalDeviceNotAuthorized
        } else {
            PeerReenrollmentError::ReplacementDeviceNotAuthorized
        });
    }
    Ok(())
}

fn validate_successor(
    quarantine: &DurablePeerCapabilityQuarantineHead,
    invitation: &SignedCurrentPeerInvitation,
) -> Result<(), PeerReenrollmentError> {
    let (minimum_revision, minimum_epoch) = match quarantine.predecessor().floor {
        PeerSyncCapabilityFloor::CurrentInvitationRequired {
            minimum_authority_revision,
            minimum_ticket_publication_epoch,
            ..
        }
        | PeerSyncCapabilityFloor::CanonicalCandidatePinned {
            minimum_authority_revision,
            minimum_ticket_publication_epoch,
        } => (minimum_authority_revision, minimum_ticket_publication_epoch),
        _ => return Err(PeerReenrollmentError::InvalidQuarantineContext),
    };
    if invitation.authority_revision() < minimum_revision {
        return Err(PeerReenrollmentError::AuthorityRegression);
    }
    if invitation.listener_device_id() == quarantine.key().peer_device_id {
        if invitation.ticket_publication_channel_epoch() < minimum_epoch {
            return Err(PeerReenrollmentError::TicketEpochRegression);
        }
        if invitation.authority_revision() == minimum_revision
            && invitation.ticket_publication_channel_epoch() == minimum_epoch
        {
            return Err(PeerReenrollmentError::NotASuccessor);
        }
    }
    Ok(())
}

fn validate_content(content: &PeerReenrollmentPlanContent) -> Result<(), PeerReenrollmentError> {
    if content.version != PLAN_VERSION {
        return Err(PeerReenrollmentError::UnsupportedVersion(content.version));
    }
    if content.local_account_id == content.peer_account_id
        || content.local_authority_revision == 0
        || content.peer_authority_revision == 0
        || content.local_authority_digest == [0_u8; 32]
        || content.peer_authority_digest == [0_u8; 32]
        || content.quarantine_record_id == [0_u8; 32]
        || content.predecessor_record_id == [0_u8; 32]
        || content.quarantine_sequence < 2
        || content.invitation_id == [0_u8; 32]
        || content.required_capability_id == [0_u8; 32]
        || content.nonce == [0_u8; 32]
        || content.invitation_issued_at_unix_seconds > content.prepared_at_unix_seconds
        || content.prepared_at_unix_seconds >= content.expires_at_unix_seconds
        || content.expires_at_unix_seconds > content.invitation_expires_at_unix_seconds
        || content
            .expires_at_unix_seconds
            .saturating_sub(content.prepared_at_unix_seconds)
            > DEFAULT_PEER_REENROLLMENT_PLAN_VALIDITY_SECONDS
        || !matches!(
            content.predecessor_floor,
            PeerSyncCapabilityFloor::CurrentInvitationRequired { .. }
                | PeerSyncCapabilityFloor::CanonicalCandidatePinned { .. }
        )
    {
        return Err(PeerReenrollmentError::InvalidContent);
    }
    Ok(())
}

/// Returns the domain-separated digest used to bind a Root-signed authority
/// snapshot into peer re-enrollment artifacts.
///
/// Runtime consumption deliberately reuses this exact function instead of
/// defining a second digest domain that could drift from the approved plan.
pub fn peer_reenrollment_authority_digest(
    authority: &AccountDeviceListSnapshot,
) -> Result<[u8; 32], PeerReenrollmentError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(AUTHORITY_DIGEST_DOMAIN);
    hasher.update(&authority.encode()?);
    Ok(*hasher.finalize().as_bytes())
}

fn plan_id(content: &PeerReenrollmentPlanContent) -> Result<[u8; 32], PeerReenrollmentError> {
    validate_content(content)?;
    let encoded = postcard::to_allocvec(content).map_err(PeerReenrollmentError::Codec)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PLAN_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

fn signing_bytes(content: &PeerReenrollmentPlanContent) -> Result<Vec<u8>, PeerReenrollmentError> {
    validate_content(content)?;
    let encoded = postcard::to_allocvec(content).map_err(PeerReenrollmentError::Codec)?;
    let mut signing = Vec::with_capacity(PLAN_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(PLAN_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn confirmation_phrase(id: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(7 + 19);
    encoded.push_str("REENROLL");
    for chunk in id[..8].chunks(2) {
        encoded.push(' ');
        for byte in chunk {
            use std::fmt::Write as _;
            let _ = write!(&mut encoded, "{byte:02x}");
        }
    }
    encoded
}

#[derive(Debug, Error)]
pub enum PeerReenrollmentError {
    #[error("explicit local approval is required for peer re-enrollment")]
    ExplicitApprovalRequired,
    #[error("peer re-enrollment confirmation phrase does not match")]
    ConfirmationMismatch,
    #[error("peer re-enrollment plan is bound to another local Device")]
    WrongLocalDevice,
    #[error("local Device is not authorized for history synchronization")]
    LocalDeviceNotAuthorized,
    #[error("replacement peer Device is not authorized for history synchronization")]
    ReplacementDeviceNotAuthorized,
    #[error("local and peer Accounts must differ")]
    SameAccount,
    #[error("peer authority, invitation and quarantine Account bindings do not match")]
    PeerAccountMismatch,
    #[error("current peer invitation does not authorize the local Account")]
    InvitationRequesterMismatch,
    #[error("current peer invitation does not match the exact peer authority revision")]
    PeerAuthorityRevisionMismatch,
    #[error("current peer invitation is not fresh at plan preparation time")]
    InvitationNotFresh,
    #[error("peer authority revision regresses below the quarantined high-water mark")]
    AuthorityRegression,
    #[error("peer ticket publication epoch regresses below the quarantined high-water mark")]
    TicketEpochRegression,
    #[error("peer invitation does not advance the quarantined same-Device high-water mark")]
    NotASuccessor,
    #[error("durable peer quarantine context is invalid")]
    InvalidQuarantineContext,
    #[error("peer re-enrollment artifact no longer matches the exact verified context")]
    ContextMismatch,
    #[error("peer re-enrollment plan is not yet valid")]
    PlanNotYetValid,
    #[error("peer re-enrollment plan has expired")]
    PlanExpired,
    #[error("peer re-enrollment artifact has unsupported version {0}")]
    UnsupportedVersion(u8),
    #[error("peer re-enrollment artifact content is invalid")]
    InvalidContent,
    #[error("peer re-enrollment artifact is {0} bytes and violates its bound")]
    ArtifactLength(usize),
    #[error("peer re-enrollment artifact is not canonically encoded")]
    NonCanonicalArtifact,
    #[error("peer re-enrollment artifact encoding is invalid: {0}")]
    Codec(postcard::Error),
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Capability(#[from] SyncTransportCapabilityError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceState};
    use kilogram_peer_capability_store::{
        PeerCapabilityKey, PeerCapabilityReadStore, PeerCapabilityStore, PeerCapabilityStoreError,
    };
    use kilogram_sync_capability::{CurrentPeerInvitationBinding, PeerSyncCapabilityEvidence};
    use tempfile::TempDir;

    struct Fixture {
        _root: TempDir,
        local_identity: DeviceIdentity,
        local_authority: AccountDeviceListSnapshot,
        quarantine: DurablePeerCapabilityQuarantineHead,
        peer_authority: AccountDeviceListSnapshot,
        invitation: SignedCurrentPeerInvitation,
        peer_old_identity: DeviceIdentity,
        peer_old_authority: AccountDeviceListSnapshot,
        peer_new_identity: DeviceIdentity,
        store_path: std::path::PathBuf,
    }

    fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let local_root = AccountRootState::create(root.path().join("local-root"))?;
        let local_device = DeviceState::load_or_create(root.path().join("local-device"))?;
        let local_certificate = local_root.issue_device_certificate(
            local_device.identity().device_id(),
            local_device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_authority = local_root.publish_device_list(&[local_certificate])?;

        let peer_root = AccountRootState::create(root.path().join("peer-root"))?;
        let peer_old = DeviceState::load_or_create(root.path().join("peer-old"))?;
        let old_certificate = peer_root.issue_device_certificate(
            peer_old.identity().device_id(),
            peer_old.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let old_authority =
            peer_root.publish_device_list(std::slice::from_ref(&old_certificate))?;
        let old_invitation = invitation(
            peer_old.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            old_authority.revision(),
            7,
            [7_u8; 32],
        )?;

        let store_path = root.path().join("peer-capability-store");
        let store = PeerCapabilityStore::open(&store_path, local_device.identity().device_id())?;
        store.apply_current_invitation(
            local_device.identity(),
            local_root.account_id(),
            &old_invitation,
            1_100,
        )?;
        let old_key =
            PeerCapabilityKey::new(peer_root.account_id(), peer_old.identity().device_id());
        store.apply_authenticated_observation(
            local_device.identity(),
            old_key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        let quarantine = store.load_quarantined_head(old_key)?;
        drop(store);

        let peer_new = DeviceState::load_or_create(root.path().join("peer-new"))?;
        let new_certificate = peer_root.issue_device_certificate(
            peer_new.identity().device_id(),
            peer_new.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_authority = peer_root.publish_device_list(&[old_certificate, new_certificate])?;
        let invitation = invitation(
            peer_new.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            peer_authority.revision(),
            1,
            [9_u8; 32],
        )?;
        Ok(Fixture {
            _root: root,
            local_identity: DeviceIdentity::from_secret_bytes(
                local_device.identity().secret_bytes(),
            ),
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            peer_old_identity: DeviceIdentity::from_secret_bytes(
                peer_old.identity().secret_bytes(),
            ),
            peer_old_authority: old_authority,
            peer_new_identity: DeviceIdentity::from_secret_bytes(
                peer_new.identity().secret_bytes(),
            ),
            store_path,
        })
    }

    fn invitation(
        listener: &DeviceIdentity,
        listener_account_id: AccountId,
        requester_account_id: AccountId,
        authority_revision: u64,
        epoch: u64,
        capability_id: [u8; 32],
    ) -> Result<SignedCurrentPeerInvitation, SyncTransportCapabilityError> {
        SignedCurrentPeerInvitation::issue(
            listener,
            &CurrentPeerInvitationBinding {
                listener_account_id,
                listener_device_id: listener.device_id(),
                allowed_requester_account_id: requester_account_id,
                authority_revision,
                endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                ticket_publication_channel_epoch: epoch,
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 2_000,
                connection_ticket: "Exact_current_ticket_01",
                required_capability_id: capability_id,
            },
        )
    }

    fn prepared(fixture: &Fixture) -> Result<PeerReenrollmentPlan, PeerReenrollmentError> {
        PeerReenrollmentPlan::prepare_with_nonce(
            fixture.local_identity.device_id(),
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_200,
            [11_u8; 32],
        )
    }

    #[test]
    fn explicit_confirmation_yields_bounded_signed_context_without_store_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let plan = prepared(&fixture)?;
        let encoded_plan = plan.encode()?;
        assert_eq!(
            PeerReenrollmentPlan::decode_and_validate(
                &encoded_plan,
                fixture.local_identity.device_id(),
            )?,
            plan
        );
        assert_eq!(
            plan.local_account_id(),
            fixture.local_authority.account_id()
        );
        assert_eq!(plan.peer_account_id(), fixture.peer_authority.account_id());
        assert_ne!(
            plan.quarantined_peer_device_id(),
            plan.replacement_peer_device_id()
        );
        let phrase = plan.confirmation_phrase()?;
        assert!(phrase.starts_with("REENROLL "));
        let tombstone = plan.confirm_and_sign(
            &fixture.local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        let encoded = tombstone.encode()?;
        assert!(encoded.len() <= MAX_PEER_REENROLLMENT_PLAN_BYTES);
        let decoded = SignedPeerReenrollmentTombstone::decode_and_verify(
            &encoded,
            fixture.local_identity.device_id(),
        )?;
        decoded.verify_context(
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_300,
        )?;
        assert!(matches!(
            decoded.successor_floor(),
            PeerSyncCapabilityFloor::CurrentInvitationRequired {
                minimum_authority_revision: 2,
                minimum_ticket_publication_epoch: 1,
                ..
            }
        ));
        let read_store = PeerCapabilityReadStore::open_existing(
            &fixture.store_path,
            fixture.local_identity.device_id(),
        )?;
        assert_eq!(
            read_store.load_quarantined_head(fixture.quarantine.key())?,
            fixture.quarantine
        );
        Ok(())
    }

    #[test]
    fn missing_approval_wrong_phrase_wrong_signer_and_tamper_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let phrase = prepared(&fixture)?.confirmation_phrase()?;
        assert!(matches!(
            prepared(&fixture)?.confirm_and_sign(
                &fixture.local_identity,
                &phrase,
                ExplicitPeerReenrollmentApproval::NotAuthorized,
            ),
            Err(PeerReenrollmentError::ExplicitApprovalRequired)
        ));
        assert!(matches!(
            prepared(&fixture)?.confirm_and_sign(
                &fixture.local_identity,
                "REENROLL 0000 0000 0000 0000",
                ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
            ),
            Err(PeerReenrollmentError::ConfirmationMismatch)
        ));
        let wrong = DeviceIdentity::from_secret_bytes([44_u8; 32]);
        assert!(matches!(
            prepared(&fixture)?.confirm_and_sign(
                &wrong,
                &phrase,
                ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
            ),
            Err(PeerReenrollmentError::WrongLocalDevice)
        ));
        let tombstone = prepared(&fixture)?.confirm_and_sign(
            &fixture.local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        let mut encoded = tombstone.encode()?;
        let middle = encoded.len() / 2;
        encoded[middle] ^= 1;
        assert!(
            SignedPeerReenrollmentTombstone::decode_and_verify(
                &encoded,
                fixture.local_identity.device_id(),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn substituted_invitation_authority_and_expiry_fail_context_verification()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let plan = prepared(&fixture)?;
        let phrase = plan.confirmation_phrase()?;
        let tombstone = plan.confirm_and_sign(
            &fixture.local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        let substituted = invitation(
            &fixture.peer_new_identity,
            fixture.peer_authority.account_id(),
            fixture.local_authority.account_id(),
            fixture.peer_authority.revision(),
            2,
            [10_u8; 32],
        )?;
        assert!(matches!(
            tombstone.verify_context(
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_authority,
                &substituted,
                1_300,
            ),
            Err(PeerReenrollmentError::ContextMismatch)
        ));
        assert!(matches!(
            tombstone.verify_context(
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.local_authority,
                &fixture.invitation,
                1_300,
            ),
            Err(PeerReenrollmentError::SameAccount)
        ));
        assert!(matches!(
            tombstone.verify_context(
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_authority,
                &fixture.invitation,
                tombstone.expires_at_unix_seconds() + 1,
            ),
            Err(PeerReenrollmentError::PlanExpired)
        ));
        Ok(())
    }

    #[test]
    fn same_device_stale_or_regressed_invitation_is_not_a_successor()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let peer_account = fixture.peer_authority.account_id();
        let requester = fixture.local_authority.account_id();
        let stale = invitation(
            &fixture.peer_old_identity,
            peer_account,
            requester,
            fixture.peer_old_authority.revision(),
            7,
            [12_u8; 32],
        )?;
        assert!(matches!(
            PeerReenrollmentPlan::prepare_with_nonce(
                fixture.local_identity.device_id(),
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_old_authority,
                &stale,
                1_200,
                [13_u8; 32],
            ),
            Err(PeerReenrollmentError::NotASuccessor)
        ));
        let regressed = invitation(
            &fixture.peer_old_identity,
            peer_account,
            requester,
            fixture.peer_authority.revision(),
            6,
            [14_u8; 32],
        )?;
        assert!(matches!(
            PeerReenrollmentPlan::prepare_with_nonce(
                fixture.local_identity.device_id(),
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_authority,
                &regressed,
                1_200,
                [15_u8; 32],
            ),
            Err(PeerReenrollmentError::TicketEpochRegression)
        ));
        Ok(())
    }

    #[test]
    fn store_requires_real_quarantine_context() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::from_secret_bytes([61_u8; 32]);
        let peer = DeviceIdentity::from_secret_bytes([62_u8; 32]);
        let key = PeerCapabilityKey::new(AccountId::from_bytes([63_u8; 32]), peer.device_id());
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        assert!(matches!(
            store.load_quarantined_head(key),
            Err(PeerCapabilityStoreError::PeerStateMissing)
        ));
        store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id: [1_u8; 32],
                authority_revision: 1,
                ticket_publication_epoch: 1,
            },
        )?;
        assert!(matches!(
            store.load_quarantined_head(key),
            Err(PeerCapabilityStoreError::PeerStateNotQuarantined)
        ));
        Ok(())
    }
}
