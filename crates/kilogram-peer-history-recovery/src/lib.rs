#![forbid(unsafe_code)]

//! Pure, offline planning for explicit old-history recovery after peer Device
//! re-enrollment.
//!
//! This crate deliberately cannot read message bodies, create rewrap bundles,
//! open a network endpoint, mutate a recovery checkpoint or write durable
//! state. It verifies bounded signed source claims and existing recipient-
//! signed checkpoints, exposes a privacy-safe inventory projection, and emits
//! a source-Device-signed consent plan bound to one already consumed peer
//! re-enrollment transition.

use std::collections::BTreeMap;

use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, ConversationMembershipSnapshot, DeviceCapability,
    DeviceId, DeviceIdentity, IdentityError, verify_device_authorization_with_snapshot,
};
use kilogram_peer_reenrollment::{PeerReenrollmentError, peer_reenrollment_authority_digest};
use kilogram_peer_reenrollment_acceptance::{
    PeerReenrollmentAcceptanceStoreError, SignedPeerReenrollmentAcceptanceReceipt,
};
use kilogram_peer_reenrollment_runtime::{
    DurablePeerReenrollmentRuntimeTransition, PeerReenrollmentRuntimeStoreError,
};
use kilogram_protocol::{ConversationId, ProtocolError, SignedHistoryRecoveryCheckpoint};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SOURCE_CLAIM_VERSION: u8 = 1;
const SOURCE_CLAIM_SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-history-source-claim:v1\0";
const SOURCE_CLAIM_ID_DOMAIN: &[u8] = b"kilogram:peer-history-source-claim-id:v1\0";
const INVENTORY_ID_DOMAIN: &[u8] = b"kilogram:peer-history-inventory-id:v1\0";
const CONVERSATION_TOKEN_DOMAIN: &[u8] = b"kilogram:peer-history-conversation-token:v1\0";
const MEMBERSHIP_DIGEST_DOMAIN: &[u8] = b"kilogram:peer-history-membership:v1\0";
const PLAN_VERSION: u8 = 1;
const PLAN_ID_DOMAIN: &[u8] = b"kilogram:peer-history-recovery-plan-id:v1\0";
const PLAN_SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-history-recovery-consent:v1\0";

pub const MAX_HISTORY_SOURCES: usize = 8;
pub const MAX_HISTORY_CONVERSATIONS: usize = 64;
pub const MAX_HISTORY_RANGES: usize = 256;
pub const MAX_HISTORY_CHECKPOINTS: usize = 256;
pub const MAX_HISTORY_EVENTS_PER_CONVERSATION: u64 = 4_096;
pub const MAX_OLD_HISTORY_RECOVERY_PLAN_BYTES: usize = 64 * 1024;
pub const DEFAULT_OLD_HISTORY_RECOVERY_PLAN_VALIDITY_SECONDS: u64 = 15 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitOldHistoryRecoveryConsent {
    NotAuthorized,
    HistoryOwnerApproved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReadableHistoryRange {
    start: u64,
    end: u64,
}

impl ReadableHistoryRange {
    pub fn new(start: u64, end: u64) -> Result<Self, PeerHistoryRecoveryError> {
        if start >= end || end > MAX_HISTORY_EVENTS_PER_CONVERSATION {
            return Err(PeerHistoryRecoveryError::InvalidReadableRange);
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> u64 {
        self.start
    }

    pub fn end(self) -> u64 {
        self.end
    }

    fn contains(self, start: u64, end: u64) -> bool {
        self.start <= start && start < end && end <= self.end
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationHistoryCoverage {
    conversation_id: ConversationId,
    inventory_event_count: u64,
    inventory_digest: [u8; 32],
    readable_ranges: Vec<ReadableHistoryRange>,
}

impl ConversationHistoryCoverage {
    pub fn new(
        conversation_id: ConversationId,
        inventory_event_count: u64,
        inventory_digest: [u8; 32],
        readable_ranges: Vec<ReadableHistoryRange>,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        let coverage = Self {
            conversation_id,
            inventory_event_count,
            inventory_digest,
            readable_ranges,
        };
        validate_coverage(&coverage)?;
        Ok(coverage)
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn inventory_event_count(&self) -> u64 {
        self.inventory_event_count
    }

    pub fn inventory_digest(&self) -> [u8; 32] {
        self.inventory_digest
    }

    pub fn readable_ranges(&self) -> &[ReadableHistoryRange] {
        &self.readable_ranges
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HistorySourceClaimContent {
    version: u8,
    account_id: AccountId,
    authority_revision: u64,
    authority_digest: [u8; 32],
    source_device_id: DeviceId,
    coverages: Vec<ConversationHistoryCoverage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedHistorySourceClaim {
    content: HistorySourceClaimContent,
    signature: Vec<u8>,
}

impl SignedHistorySourceClaim {
    pub fn issue(
        source: &DeviceIdentity,
        authority: &AccountDeviceListSnapshot,
        coverages: Vec<ConversationHistoryCoverage>,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        verify_history_device(authority, source.device_id())?;
        let content = HistorySourceClaimContent {
            version: SOURCE_CLAIM_VERSION,
            account_id: authority.account_id(),
            authority_revision: authority.revision(),
            authority_digest: peer_reenrollment_authority_digest(authority)?,
            source_device_id: source.device_id(),
            coverages,
        };
        validate_source_claim_content(&content)?;
        let signature = source.sign(&source_claim_signing_bytes(&content)?).to_vec();
        let claim = Self { content, signature };
        claim.verify_for_authority(authority)?;
        Ok(claim)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
        self.verify_signature()?;
        let encoded = postcard::to_allocvec(self)?;
        if encoded.is_empty() || encoded.len() > MAX_OLD_HISTORY_RECOVERY_PLAN_BYTES {
            return Err(PeerHistoryRecoveryError::ArtifactLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        authority: &AccountDeviceListSnapshot,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        if encoded.is_empty() || encoded.len() > MAX_OLD_HISTORY_RECOVERY_PLAN_BYTES {
            return Err(PeerHistoryRecoveryError::ArtifactLength(encoded.len()));
        }
        let claim: Self = postcard::from_bytes(encoded)?;
        claim.verify_for_authority(authority)?;
        if claim.encode()? != encoded {
            return Err(PeerHistoryRecoveryError::NonCanonicalArtifact);
        }
        Ok(claim)
    }

    pub fn claim_id(&self) -> Result<[u8; 32], PeerHistoryRecoveryError> {
        Ok(domain_digest(SOURCE_CLAIM_ID_DOMAIN, &self.encode()?))
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub fn coverages(&self) -> &[ConversationHistoryCoverage] {
        &self.content.coverages
    }

    fn coverage(&self, conversation_id: ConversationId) -> Option<&ConversationHistoryCoverage> {
        self.content
            .coverages
            .binary_search_by_key(&conversation_id, |coverage| coverage.conversation_id)
            .ok()
            .map(|index| &self.content.coverages[index])
    }

    fn verify_signature(&self) -> Result<(), PeerHistoryRecoveryError> {
        validate_source_claim_content(&self.content)?;
        self.content
            .source_device_id
            .verify(&source_claim_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn verify_for_authority(
        &self,
        authority: &AccountDeviceListSnapshot,
    ) -> Result<(), PeerHistoryRecoveryError> {
        self.verify_signature()?;
        verify_history_device(authority, self.content.source_device_id)?;
        if self.content.account_id != authority.account_id()
            || self.content.authority_revision != authority.revision()
            || self.content.authority_digest != peer_reenrollment_authority_digest(authority)?
        {
            return Err(PeerHistoryRecoveryError::AuthorityMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecoveryCheckpointBinding {
    checkpoint_id: [u8; 32],
    source_device_id: DeviceId,
    conversation_id: ConversationId,
    approved_range_start: u64,
    approved_range_end: u64,
    next_range_start: u64,
    inventory_event_count: Option<u64>,
    inventory_digest: Option<[u8; 32]>,
}

impl RecoveryCheckpointBinding {
    fn from_checkpoint(
        checkpoint: &SignedHistoryRecoveryCheckpoint,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        checkpoint.verify_signature()?;
        Ok(Self {
            checkpoint_id: checkpoint.checkpoint_id()?,
            source_device_id: checkpoint.source_device_id(),
            conversation_id: checkpoint.conversation_id(),
            approved_range_start: checkpoint.approved_range_start(),
            approved_range_end: checkpoint.approved_range_end(),
            next_range_start: checkpoint.next_range_start(),
            inventory_event_count: checkpoint.inventory_event_count(),
            inventory_digest: checkpoint.inventory_digest().copied(),
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SourceClaimAgreement {
    Agreed,
    Divergent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryConversationInventoryView {
    pub conversation_token: [u8; 16],
    pub source_count: u8,
    pub readable_range_count: u16,
    pub checkpoint_count: u16,
    pub inventory_event_count: Option<u64>,
    pub agreement: SourceClaimAgreement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OldHistoryRecoveryInventoryView {
    pub source_count: u8,
    pub conversation_count: u16,
    pub checkpoint_count: u16,
    pub conversations: Vec<HistoryConversationInventoryView>,
    pub contains_message_bodies: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OldHistoryRecoveryInventoryContent {
    account_id: AccountId,
    authority_revision: u64,
    authority_digest: [u8; 32],
    replacement_device_id: DeviceId,
    source_claims: Vec<SignedHistorySourceClaim>,
    checkpoints: Vec<RecoveryCheckpointBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OldHistoryRecoveryInventory {
    content: OldHistoryRecoveryInventoryContent,
}

impl OldHistoryRecoveryInventory {
    pub fn assemble(
        authority: &AccountDeviceListSnapshot,
        replacement_device_id: DeviceId,
        mut source_claims: Vec<SignedHistorySourceClaim>,
        checkpoints: &[SignedHistoryRecoveryCheckpoint],
    ) -> Result<Self, PeerHistoryRecoveryError> {
        verify_history_device(authority, replacement_device_id)?;
        if source_claims.is_empty() || source_claims.len() > MAX_HISTORY_SOURCES {
            return Err(PeerHistoryRecoveryError::SourceCount(source_claims.len()));
        }
        source_claims.sort_by_key(SignedHistorySourceClaim::source_device_id);
        if source_claims
            .windows(2)
            .any(|pair| pair[0].source_device_id() == pair[1].source_device_id())
        {
            return Err(PeerHistoryRecoveryError::DuplicateSource);
        }
        for claim in &source_claims {
            claim.verify_for_authority(authority)?;
            if claim.source_device_id() == replacement_device_id {
                return Err(PeerHistoryRecoveryError::SourceIsReplacement);
            }
        }
        if checkpoints.len() > MAX_HISTORY_CHECKPOINTS {
            return Err(PeerHistoryRecoveryError::CheckpointCount(checkpoints.len()));
        }
        let mut checkpoint_bindings = Vec::with_capacity(checkpoints.len());
        for checkpoint in checkpoints {
            checkpoint.verify_signature()?;
            if checkpoint.account_id() != authority.account_id()
                || checkpoint.recipient_device_id() != replacement_device_id
            {
                return Err(PeerHistoryRecoveryError::CheckpointContextMismatch);
            }
            let claim = source_claims
                .iter()
                .find(|claim| claim.source_device_id() == checkpoint.source_device_id())
                .ok_or(PeerHistoryRecoveryError::CheckpointSourceMissing)?;
            let coverage = claim
                .coverage(checkpoint.conversation_id())
                .ok_or(PeerHistoryRecoveryError::CheckpointConversationMissing)?;
            match (
                checkpoint.inventory_event_count(),
                checkpoint.inventory_digest(),
            ) {
                (None, None) => {}
                (Some(count), Some(digest))
                    if count == coverage.inventory_event_count
                        && *digest == coverage.inventory_digest => {}
                _ => return Err(PeerHistoryRecoveryError::CheckpointClaimMismatch),
            }
            if checkpoint.approved_range_end() > coverage.inventory_event_count
                || !coverage.readable_ranges.iter().any(|range| {
                    range.contains(
                        checkpoint.approved_range_start(),
                        checkpoint.approved_range_end(),
                    )
                })
            {
                return Err(PeerHistoryRecoveryError::CheckpointRangeMismatch);
            }
            checkpoint_bindings.push(RecoveryCheckpointBinding::from_checkpoint(checkpoint)?);
        }
        checkpoint_bindings
            .sort_by_key(|checkpoint| (checkpoint.source_device_id, checkpoint.conversation_id));
        if checkpoint_bindings.windows(2).any(|pair| {
            pair[0].source_device_id == pair[1].source_device_id
                && pair[0].conversation_id == pair[1].conversation_id
        }) {
            return Err(PeerHistoryRecoveryError::AmbiguousCheckpoint);
        }
        let inventory = Self {
            content: OldHistoryRecoveryInventoryContent {
                account_id: authority.account_id(),
                authority_revision: authority.revision(),
                authority_digest: peer_reenrollment_authority_digest(authority)?,
                replacement_device_id,
                source_claims,
                checkpoints: checkpoint_bindings,
            },
        };
        inventory.verify_for_authority(authority, replacement_device_id)?;
        Ok(inventory)
    }

    pub fn inventory_id(&self) -> Result<[u8; 32], PeerHistoryRecoveryError> {
        self.validate()?;
        Ok(domain_digest(
            INVENTORY_ID_DOMAIN,
            &postcard::to_allocvec(&self.content)?,
        ))
    }

    pub fn view(&self) -> Result<OldHistoryRecoveryInventoryView, PeerHistoryRecoveryError> {
        self.validate()?;
        let mut conversations =
            BTreeMap::<ConversationId, Vec<&ConversationHistoryCoverage>>::new();
        for claim in &self.content.source_claims {
            for coverage in claim.coverages() {
                conversations
                    .entry(coverage.conversation_id)
                    .or_default()
                    .push(coverage);
            }
        }
        let mut views = Vec::with_capacity(conversations.len());
        for (conversation_id, coverages) in conversations {
            let agreement = coverage_agreement(&coverages);
            let agreed_count = match agreement {
                SourceClaimAgreement::Agreed => Some(coverages[0].inventory_event_count),
                SourceClaimAgreement::Divergent => None,
            };
            let checkpoint_count = self
                .content
                .checkpoints
                .iter()
                .filter(|checkpoint| checkpoint.conversation_id == conversation_id)
                .count();
            let readable_range_count: usize = coverages
                .iter()
                .map(|coverage| coverage.readable_ranges.len())
                .sum();
            views.push(HistoryConversationInventoryView {
                conversation_token: conversation_token(conversation_id),
                source_count: u8::try_from(coverages.len())
                    .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
                readable_range_count: u16::try_from(readable_range_count)
                    .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
                checkpoint_count: u16::try_from(checkpoint_count)
                    .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
                inventory_event_count: agreed_count,
                agreement,
            });
        }
        Ok(OldHistoryRecoveryInventoryView {
            source_count: u8::try_from(self.content.source_claims.len())
                .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
            conversation_count: u16::try_from(views.len())
                .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
            checkpoint_count: u16::try_from(self.content.checkpoints.len())
                .map_err(|_| PeerHistoryRecoveryError::InvalidContent)?,
            conversations: views,
            contains_message_bodies: false,
        })
    }

    fn source_claim(&self, source_device_id: DeviceId) -> Option<&SignedHistorySourceClaim> {
        self.content
            .source_claims
            .binary_search_by_key(
                &source_device_id,
                SignedHistorySourceClaim::source_device_id,
            )
            .ok()
            .map(|index| &self.content.source_claims[index])
    }

    fn checkpoint(
        &self,
        source_device_id: DeviceId,
        conversation_id: ConversationId,
    ) -> Option<&RecoveryCheckpointBinding> {
        self.content.checkpoints.iter().find(|checkpoint| {
            checkpoint.source_device_id == source_device_id
                && checkpoint.conversation_id == conversation_id
        })
    }

    fn agreement(&self, conversation_id: ConversationId) -> Option<SourceClaimAgreement> {
        let coverages: Vec<_> = self
            .content
            .source_claims
            .iter()
            .filter_map(|claim| claim.coverage(conversation_id))
            .collect();
        (!coverages.is_empty()).then(|| coverage_agreement(&coverages))
    }

    fn verify_for_authority(
        &self,
        authority: &AccountDeviceListSnapshot,
        replacement_device_id: DeviceId,
    ) -> Result<(), PeerHistoryRecoveryError> {
        self.validate()?;
        verify_history_device(authority, replacement_device_id)?;
        if self.content.account_id != authority.account_id()
            || self.content.authority_revision != authority.revision()
            || self.content.authority_digest != peer_reenrollment_authority_digest(authority)?
            || self.content.replacement_device_id != replacement_device_id
        {
            return Err(PeerHistoryRecoveryError::InventoryContextMismatch);
        }
        for claim in &self.content.source_claims {
            claim.verify_for_authority(authority)?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), PeerHistoryRecoveryError> {
        if self.content.source_claims.is_empty()
            || self.content.source_claims.len() > MAX_HISTORY_SOURCES
            || self.content.checkpoints.len() > MAX_HISTORY_CHECKPOINTS
            || self.content.authority_revision == 0
            || self.content.authority_digest == [0_u8; 32]
        {
            return Err(PeerHistoryRecoveryError::InvalidContent);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentHistoryBindingContent {
    local_account_id: AccountId,
    local_device_id: DeviceId,
    history_account_id: AccountId,
    replacement_device_id: DeviceId,
    history_authority_revision: u64,
    history_authority_digest: [u8; 32],
    acceptance_receipt_id: [u8; 32],
    transition_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReenrollmentHistoryBinding {
    content: PeerReenrollmentHistoryBindingContent,
}

impl PeerReenrollmentHistoryBinding {
    pub fn from_consumed_transition(
        receipt: &SignedPeerReenrollmentAcceptanceReceipt,
        transition: &DurablePeerReenrollmentRuntimeTransition,
        history_authority: &AccountDeviceListSnapshot,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        receipt.encode()?;
        transition.transition().encode()?;
        history_authority.verify()?;
        if !transition.consumed() {
            return Err(PeerHistoryRecoveryError::TransitionNotConsumed);
        }
        let signed_transition = transition.transition();
        let receipt_id = receipt.receipt_id()?;
        let transition_id = signed_transition.transition_id()?;
        if transition.transition_id() != transition_id
            || signed_transition.acceptance_receipt_id() != receipt_id
            || signed_transition.local_account_id() != receipt.local_account_id()
            || signed_transition.local_device_id() != receipt.local_device_id()
            || signed_transition.peer_account_id() != receipt.peer_account_id()
            || signed_transition.quarantined_peer_device_id()
                != receipt.quarantined_peer_device_id()
            || signed_transition.replacement_peer_device_id()
                != receipt.replacement_peer_device_id()
            || history_authority.account_id() != receipt.peer_account_id()
            || history_authority.revision() != receipt.peer_authority_revision()
            || peer_reenrollment_authority_digest(history_authority)?
                != receipt.peer_authority_digest()
        {
            return Err(PeerHistoryRecoveryError::ReenrollmentBindingMismatch);
        }
        verify_history_device(history_authority, receipt.replacement_peer_device_id())?;
        Ok(Self {
            content: PeerReenrollmentHistoryBindingContent {
                local_account_id: receipt.local_account_id(),
                local_device_id: receipt.local_device_id(),
                history_account_id: receipt.peer_account_id(),
                replacement_device_id: receipt.replacement_peer_device_id(),
                history_authority_revision: receipt.peer_authority_revision(),
                history_authority_digest: receipt.peer_authority_digest(),
                acceptance_receipt_id: receipt_id,
                transition_id,
            },
        })
    }

    pub fn replacement_device_id(&self) -> DeviceId {
        self.content.replacement_device_id
    }

    pub fn transition_id(&self) -> [u8; 32] {
        self.content.transition_id
    }

    fn verify_authority(
        &self,
        authority: &AccountDeviceListSnapshot,
    ) -> Result<(), PeerHistoryRecoveryError> {
        authority.verify()?;
        if authority.account_id() != self.content.history_account_id
            || authority.revision() != self.content.history_authority_revision
            || peer_reenrollment_authority_digest(authority)?
                != self.content.history_authority_digest
        {
            return Err(PeerHistoryRecoveryError::AuthorityMismatch);
        }
        verify_history_device(authority, self.content.replacement_device_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryRangeSelection {
    pub conversation_id: ConversationId,
    pub range_start: u64,
    pub range_end: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct BoundRecoveryRange {
    conversation_id: ConversationId,
    membership_revision: u64,
    membership_owner_account_id: AccountId,
    membership_digest: [u8; 32],
    inventory_event_count: u64,
    inventory_digest: [u8; 32],
    range_start: u64,
    range_end: u64,
    checkpoint_id: Option<[u8; 32]>,
    checkpoint_next_range_start: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OldHistoryRecoveryPlanContent {
    version: u8,
    reenrollment: PeerReenrollmentHistoryBindingContent,
    source_device_id: DeviceId,
    source_claim_id: [u8; 32],
    inventory_id: [u8; 32],
    selections: Vec<BoundRecoveryRange>,
    prepared_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    nonce: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OldHistoryRecoveryPlan {
    content: OldHistoryRecoveryPlanContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedOldHistoryRecoveryPlanWire {
    content: OldHistoryRecoveryPlanContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedOldHistoryRecoveryPlan {
    content: OldHistoryRecoveryPlanContent,
    signature: Vec<u8>,
}

impl OldHistoryRecoveryPlan {
    pub fn prepare(
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        source_device_id: DeviceId,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(PeerHistoryRecoveryError::SecureRandom)?;
        Self::prepare_with_nonce(
            binding,
            authority,
            memberships,
            inventory,
            source_device_id,
            selections,
            now_unix_seconds,
            nonce,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_with_nonce(
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        source_device_id: DeviceId,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
        nonce: [u8; 32],
    ) -> Result<Self, PeerHistoryRecoveryError> {
        let content = build_plan_content(
            binding,
            authority,
            memberships,
            inventory,
            source_device_id,
            selections,
            now_unix_seconds,
            nonce,
        )?;
        Ok(Self { content })
    }

    pub fn plan_id(&self) -> Result<[u8; 32], PeerHistoryRecoveryError> {
        validate_plan_content(&self.content)?;
        Ok(domain_digest(
            PLAN_ID_DOMAIN,
            &postcard::to_allocvec(&self.content)?,
        ))
    }

    pub fn confirmation_phrase(&self) -> Result<String, PeerHistoryRecoveryError> {
        let id = self.plan_id()?;
        Ok(format!(
            "RECOVER HISTORY {:02X}{:02X} {:02X}{:02X} {:02X}{:02X}",
            id[0], id[1], id[2], id[3], id[4], id[5]
        ))
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
        validate_plan_content(&self.content)?;
        bounded_encode(&self.content)
    }

    pub fn decode_and_validate(
        encoded: &[u8],
        expected_replacement_device_id: DeviceId,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        validate_artifact_length(encoded)?;
        let content: OldHistoryRecoveryPlanContent = postcard::from_bytes(encoded)?;
        validate_plan_content(&content)?;
        if content.reenrollment.replacement_device_id != expected_replacement_device_id {
            return Err(PeerHistoryRecoveryError::WrongReplacementDevice);
        }
        let plan = Self { content };
        if plan.encode()? != encoded {
            return Err(PeerHistoryRecoveryError::NonCanonicalArtifact);
        }
        Ok(plan)
    }

    pub fn confirm_and_sign(
        self,
        source: &DeviceIdentity,
        supplied_confirmation: &str,
        consent: ExplicitOldHistoryRecoveryConsent,
    ) -> Result<SignedOldHistoryRecoveryPlan, PeerHistoryRecoveryError> {
        if consent != ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved {
            return Err(PeerHistoryRecoveryError::ExplicitConsentRequired);
        }
        if source.device_id() != self.content.source_device_id {
            return Err(PeerHistoryRecoveryError::WrongSourceDevice);
        }
        if supplied_confirmation != self.confirmation_phrase()? {
            return Err(PeerHistoryRecoveryError::ConfirmationMismatch);
        }
        let signature = source.sign(&plan_signing_bytes(&self.content)?).to_vec();
        let signed = SignedOldHistoryRecoveryPlan {
            content: self.content,
            signature,
        };
        signed.verify_signature()?;
        Ok(signed)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_context(
        &self,
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        source_device_id: DeviceId,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
    ) -> Result<(), PeerHistoryRecoveryError> {
        verify_plan_context(
            &self.content,
            binding,
            authority,
            memberships,
            inventory,
            source_device_id,
            selections,
            now_unix_seconds,
        )
    }
}

impl SignedOldHistoryRecoveryPlan {
    pub fn encode(&self) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
        self.verify_signature()?;
        bounded_encode(&SignedOldHistoryRecoveryPlanWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        expected_source_device_id: DeviceId,
        expected_replacement_device_id: DeviceId,
    ) -> Result<Self, PeerHistoryRecoveryError> {
        validate_artifact_length(encoded)?;
        let wire: SignedOldHistoryRecoveryPlanWire = postcard::from_bytes(encoded)?;
        let signed = Self {
            content: wire.content,
            signature: wire.signature,
        };
        signed.verify_signature()?;
        if signed.content.source_device_id != expected_source_device_id {
            return Err(PeerHistoryRecoveryError::WrongSourceDevice);
        }
        if signed.content.reenrollment.replacement_device_id != expected_replacement_device_id {
            return Err(PeerHistoryRecoveryError::WrongReplacementDevice);
        }
        if signed.encode()? != encoded {
            return Err(PeerHistoryRecoveryError::NonCanonicalArtifact);
        }
        Ok(signed)
    }

    pub fn plan_id(&self) -> Result<[u8; 32], PeerHistoryRecoveryError> {
        validate_plan_content(&self.content)?;
        Ok(domain_digest(
            PLAN_ID_DOMAIN,
            &postcard::to_allocvec(&self.content)?,
        ))
    }

    pub fn transition_id(&self) -> [u8; 32] {
        self.content.reenrollment.transition_id
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub fn replacement_device_id(&self) -> DeviceId {
        self.content.reenrollment.replacement_device_id
    }

    pub fn selection_count(&self) -> usize {
        self.content.selections.len()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_context(
        &self,
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        source_device_id: DeviceId,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
    ) -> Result<(), PeerHistoryRecoveryError> {
        self.verify_signature()?;
        verify_plan_context(
            &self.content,
            binding,
            authority,
            memberships,
            inventory,
            source_device_id,
            selections,
            now_unix_seconds,
        )
    }

    fn verify_signature(&self) -> Result<(), PeerHistoryRecoveryError> {
        validate_plan_content(&self.content)?;
        self.content
            .source_device_id
            .verify(&plan_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_plan_content(
    binding: &PeerReenrollmentHistoryBinding,
    authority: &AccountDeviceListSnapshot,
    memberships: &[ConversationMembershipSnapshot],
    inventory: &OldHistoryRecoveryInventory,
    source_device_id: DeviceId,
    selections: &[HistoryRecoveryRangeSelection],
    prepared_at_unix_seconds: u64,
    nonce: [u8; 32],
) -> Result<OldHistoryRecoveryPlanContent, PeerHistoryRecoveryError> {
    binding.verify_authority(authority)?;
    inventory.verify_for_authority(authority, binding.replacement_device_id())?;
    verify_history_device(authority, source_device_id)?;
    if source_device_id == binding.replacement_device_id() {
        return Err(PeerHistoryRecoveryError::SourceIsReplacement);
    }
    let source_claim = inventory
        .source_claim(source_device_id)
        .ok_or(PeerHistoryRecoveryError::SourceClaimMissing)?;
    if selections.is_empty() || selections.len() > MAX_HISTORY_CONVERSATIONS {
        return Err(PeerHistoryRecoveryError::SelectionCount(selections.len()));
    }
    let mut memberships_by_conversation = BTreeMap::new();
    for membership in memberships {
        membership.verify()?;
        membership.require_member(binding.content.local_account_id)?;
        membership.require_member(binding.content.history_account_id)?;
        let conversation_id = ConversationId::from_bytes(*membership.conversation_id().as_bytes());
        if memberships_by_conversation
            .insert(conversation_id, membership)
            .is_some()
        {
            return Err(PeerHistoryRecoveryError::DuplicateMembership);
        }
    }
    let mut previous_conversation = None;
    let mut bound = Vec::with_capacity(selections.len());
    for selection in selections {
        if previous_conversation.is_some_and(|previous| previous >= selection.conversation_id) {
            return Err(PeerHistoryRecoveryError::NonCanonicalSelections);
        }
        previous_conversation = Some(selection.conversation_id);
        if inventory.agreement(selection.conversation_id) != Some(SourceClaimAgreement::Agreed) {
            return Err(PeerHistoryRecoveryError::DivergentSourceClaims);
        }
        let coverage = source_claim
            .coverage(selection.conversation_id)
            .ok_or(PeerHistoryRecoveryError::SourceCoverageMissing)?;
        if selection.range_start >= selection.range_end
            || selection.range_end > coverage.inventory_event_count
            || !coverage
                .readable_ranges
                .iter()
                .any(|range| range.contains(selection.range_start, selection.range_end))
        {
            return Err(PeerHistoryRecoveryError::SelectionOutsideCoverage);
        }
        let membership = memberships_by_conversation
            .get(&selection.conversation_id)
            .ok_or(PeerHistoryRecoveryError::MembershipMissing)?;
        let membership_bytes = membership.encode()?;
        let checkpoint = inventory.checkpoint(source_device_id, selection.conversation_id);
        if let Some(checkpoint) = checkpoint
            && (selection.range_start != checkpoint.next_range_start
                || selection.range_end > checkpoint.approved_range_end
                || selection.range_start < checkpoint.approved_range_start)
        {
            return Err(PeerHistoryRecoveryError::CheckpointRangeMismatch);
        }
        bound.push(BoundRecoveryRange {
            conversation_id: selection.conversation_id,
            membership_revision: membership.revision(),
            membership_owner_account_id: membership.owner_account_id(),
            membership_digest: domain_digest(MEMBERSHIP_DIGEST_DOMAIN, &membership_bytes),
            inventory_event_count: coverage.inventory_event_count,
            inventory_digest: coverage.inventory_digest,
            range_start: selection.range_start,
            range_end: selection.range_end,
            checkpoint_id: checkpoint.map(|checkpoint| checkpoint.checkpoint_id),
            checkpoint_next_range_start: checkpoint.map(|checkpoint| checkpoint.next_range_start),
        });
    }
    let expires_at_unix_seconds = prepared_at_unix_seconds
        .checked_add(DEFAULT_OLD_HISTORY_RECOVERY_PLAN_VALIDITY_SECONDS)
        .ok_or(PeerHistoryRecoveryError::InvalidContent)?;
    let content = OldHistoryRecoveryPlanContent {
        version: PLAN_VERSION,
        reenrollment: binding.content.clone(),
        source_device_id,
        source_claim_id: source_claim.claim_id()?,
        inventory_id: inventory.inventory_id()?,
        selections: bound,
        prepared_at_unix_seconds,
        expires_at_unix_seconds,
        nonce,
    };
    validate_plan_content(&content)?;
    Ok(content)
}

#[allow(clippy::too_many_arguments)]
fn verify_plan_context(
    content: &OldHistoryRecoveryPlanContent,
    binding: &PeerReenrollmentHistoryBinding,
    authority: &AccountDeviceListSnapshot,
    memberships: &[ConversationMembershipSnapshot],
    inventory: &OldHistoryRecoveryInventory,
    source_device_id: DeviceId,
    selections: &[HistoryRecoveryRangeSelection],
    now_unix_seconds: u64,
) -> Result<(), PeerHistoryRecoveryError> {
    let expected = build_plan_content(
        binding,
        authority,
        memberships,
        inventory,
        source_device_id,
        selections,
        content.prepared_at_unix_seconds,
        content.nonce,
    )?;
    if expected != *content {
        return Err(PeerHistoryRecoveryError::PlanContextMismatch);
    }
    validate_plan_time(content, now_unix_seconds)
}

fn validate_coverage(
    coverage: &ConversationHistoryCoverage,
) -> Result<(), PeerHistoryRecoveryError> {
    if coverage.inventory_event_count == 0
        || coverage.inventory_event_count > MAX_HISTORY_EVENTS_PER_CONVERSATION
        || coverage.inventory_digest == [0_u8; 32]
        || coverage.readable_ranges.is_empty()
        || coverage.readable_ranges.len() > MAX_HISTORY_RANGES
    {
        return Err(PeerHistoryRecoveryError::InvalidCoverage);
    }
    let mut previous_end = None;
    for range in &coverage.readable_ranges {
        if range.start >= range.end
            || range.end > coverage.inventory_event_count
            || previous_end.is_some_and(|end| end >= range.start)
        {
            return Err(PeerHistoryRecoveryError::InvalidReadableRange);
        }
        previous_end = Some(range.end);
    }
    Ok(())
}

fn validate_source_claim_content(
    content: &HistorySourceClaimContent,
) -> Result<(), PeerHistoryRecoveryError> {
    if content.version != SOURCE_CLAIM_VERSION
        || content.authority_revision == 0
        || content.authority_digest == [0_u8; 32]
        || content.coverages.is_empty()
        || content.coverages.len() > MAX_HISTORY_CONVERSATIONS
    {
        return Err(PeerHistoryRecoveryError::InvalidContent);
    }
    let mut previous = None;
    let mut total_ranges = 0_usize;
    for coverage in &content.coverages {
        validate_coverage(coverage)?;
        if previous.is_some_and(|conversation| conversation >= coverage.conversation_id) {
            return Err(PeerHistoryRecoveryError::NonCanonicalCoverages);
        }
        previous = Some(coverage.conversation_id);
        total_ranges = total_ranges.saturating_add(coverage.readable_ranges.len());
    }
    if total_ranges > MAX_HISTORY_RANGES {
        return Err(PeerHistoryRecoveryError::RangeCount(total_ranges));
    }
    Ok(())
}

fn validate_plan_content(
    content: &OldHistoryRecoveryPlanContent,
) -> Result<(), PeerHistoryRecoveryError> {
    if content.version != PLAN_VERSION
        || content.reenrollment.history_authority_revision == 0
        || content.reenrollment.history_authority_digest == [0_u8; 32]
        || content.reenrollment.acceptance_receipt_id == [0_u8; 32]
        || content.reenrollment.transition_id == [0_u8; 32]
        || content.source_claim_id == [0_u8; 32]
        || content.inventory_id == [0_u8; 32]
        || content.nonce == [0_u8; 32]
        || content.source_device_id == content.reenrollment.replacement_device_id
        || content.selections.is_empty()
        || content.selections.len() > MAX_HISTORY_CONVERSATIONS
        || content.prepared_at_unix_seconds >= content.expires_at_unix_seconds
        || content
            .expires_at_unix_seconds
            .saturating_sub(content.prepared_at_unix_seconds)
            != DEFAULT_OLD_HISTORY_RECOVERY_PLAN_VALIDITY_SECONDS
    {
        return Err(PeerHistoryRecoveryError::InvalidContent);
    }
    let mut previous_conversation = None;
    for selection in &content.selections {
        if selection.membership_revision == 0
            || selection.membership_digest == [0_u8; 32]
            || selection.inventory_event_count == 0
            || selection.inventory_digest == [0_u8; 32]
            || selection.range_start >= selection.range_end
            || selection.range_end > selection.inventory_event_count
            || selection.checkpoint_id.is_some() != selection.checkpoint_next_range_start.is_some()
        {
            return Err(PeerHistoryRecoveryError::InvalidContent);
        }
        if previous_conversation.is_some_and(|previous| previous >= selection.conversation_id) {
            return Err(PeerHistoryRecoveryError::NonCanonicalSelections);
        }
        previous_conversation = Some(selection.conversation_id);
    }
    Ok(())
}

fn validate_plan_time(
    content: &OldHistoryRecoveryPlanContent,
    now_unix_seconds: u64,
) -> Result<(), PeerHistoryRecoveryError> {
    if now_unix_seconds < content.prepared_at_unix_seconds {
        return Err(PeerHistoryRecoveryError::PlanNotYetValid);
    }
    if now_unix_seconds > content.expires_at_unix_seconds {
        return Err(PeerHistoryRecoveryError::PlanExpired);
    }
    Ok(())
}

fn verify_history_device(
    authority: &AccountDeviceListSnapshot,
    device_id: DeviceId,
) -> Result<(), PeerHistoryRecoveryError> {
    authority.verify()?;
    let certificate = authority
        .certificate_for(device_id)
        .ok_or(PeerHistoryRecoveryError::DeviceMissing)?;
    verify_device_authorization_with_snapshot(
        authority.account_id(),
        certificate,
        authority.authority_snapshot(),
        &[DeviceCapability::SyncHistory],
    )?;
    Ok(())
}

fn coverage_agreement(coverages: &[&ConversationHistoryCoverage]) -> SourceClaimAgreement {
    let first = coverages[0];
    if coverages.iter().skip(1).any(|coverage| {
        coverage.inventory_event_count != first.inventory_event_count
            || coverage.inventory_digest != first.inventory_digest
    }) {
        SourceClaimAgreement::Divergent
    } else {
        SourceClaimAgreement::Agreed
    }
}

fn conversation_token(conversation_id: ConversationId) -> [u8; 16] {
    let digest = domain_digest(CONVERSATION_TOKEN_DOMAIN, conversation_id.as_bytes());
    let mut token = [0_u8; 16];
    token.copy_from_slice(&digest[..16]);
    token
}

fn source_claim_signing_bytes(
    content: &HistorySourceClaimContent,
) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SOURCE_CLAIM_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SOURCE_CLAIM_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn plan_signing_bytes(
    content: &OldHistoryRecoveryPlanContent,
) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(PLAN_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(PLAN_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn bounded_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, PeerHistoryRecoveryError> {
    let encoded = postcard::to_allocvec(value)?;
    validate_artifact_length(&encoded)?;
    Ok(encoded)
}

fn validate_artifact_length(encoded: &[u8]) -> Result<(), PeerHistoryRecoveryError> {
    if encoded.is_empty() || encoded.len() > MAX_OLD_HISTORY_RECOVERY_PLAN_BYTES {
        return Err(PeerHistoryRecoveryError::ArtifactLength(encoded.len()));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PeerHistoryRecoveryError {
    #[error("old-history source count {0} is outside its bound")]
    SourceCount(usize),
    #[error("old-history conversation selection count {0} is outside its bound")]
    SelectionCount(usize),
    #[error("old-history readable range count {0} is outside its bound")]
    RangeCount(usize),
    #[error("old-history checkpoint count {0} is outside its bound")]
    CheckpointCount(usize),
    #[error("old-history readable range is invalid")]
    InvalidReadableRange,
    #[error("old-history source coverage is invalid")]
    InvalidCoverage,
    #[error("old-history source coverages are not canonical")]
    NonCanonicalCoverages,
    #[error("old-history selections are not canonical")]
    NonCanonicalSelections,
    #[error("old-history source Device is duplicated")]
    DuplicateSource,
    #[error("old-history source Device is the replacement Device")]
    SourceIsReplacement,
    #[error("old-history Device is absent or not authorized for history sync")]
    DeviceMissing,
    #[error("old-history authority does not match the bound authority")]
    AuthorityMismatch,
    #[error("old-history inventory context changed")]
    InventoryContextMismatch,
    #[error("history checkpoint belongs to another account or recipient")]
    CheckpointContextMismatch,
    #[error("history checkpoint source is absent from the inventory")]
    CheckpointSourceMissing,
    #[error("history checkpoint conversation is absent from its source claim")]
    CheckpointConversationMissing,
    #[error("history checkpoint source claim changed")]
    CheckpointClaimMismatch,
    #[error("history checkpoint range is outside readable source coverage")]
    CheckpointRangeMismatch,
    #[error("more than one current checkpoint exists for a source conversation")]
    AmbiguousCheckpoint,
    #[error("peer re-enrollment transition has not been consumed by a fresh runtime")]
    TransitionNotConsumed,
    #[error("peer re-enrollment receipt, transition and authority do not match")]
    ReenrollmentBindingMismatch,
    #[error("selected source claim is absent")]
    SourceClaimMissing,
    #[error("selected source has no claim for a conversation")]
    SourceCoverageMissing,
    #[error("source claims diverge for a selected conversation")]
    DivergentSourceClaims,
    #[error("selected recovery range is outside readable coverage")]
    SelectionOutsideCoverage,
    #[error("conversation membership is missing")]
    MembershipMissing,
    #[error("conversation membership is duplicated")]
    DuplicateMembership,
    #[error("old-history plan no longer matches its exact context")]
    PlanContextMismatch,
    #[error("old-history recovery requires separate explicit history-owner consent")]
    ExplicitConsentRequired,
    #[error("old-history confirmation phrase does not match")]
    ConfirmationMismatch,
    #[error("old-history plan is bound to another source Device")]
    WrongSourceDevice,
    #[error("old-history plan is bound to another replacement Device")]
    WrongReplacementDevice,
    #[error("old-history plan is not yet valid")]
    PlanNotYetValid,
    #[error("old-history plan has expired")]
    PlanExpired,
    #[error("old-history artifact content is invalid")]
    InvalidContent,
    #[error("old-history artifact is {0} bytes and violates its bound")]
    ArtifactLength(usize),
    #[error("old-history artifact is not canonically encoded")]
    NonCanonicalArtifact,
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Acceptance(#[from] PeerReenrollmentAcceptanceStoreError),
    #[error(transparent)]
    Runtime(#[from] PeerReenrollmentRuntimeStoreError),
    #[error(transparent)]
    Reenrollment(#[from] PeerReenrollmentError),
    #[error(transparent)]
    Codec(#[from] postcard::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCertificate, DeviceState};
    use tempfile::TempDir;

    struct Fixture {
        _root: TempDir,
        history_root: AccountRootState,
        history_certificates: Vec<DeviceCertificate>,
        local_account_id: AccountId,
        history_authority: AccountDeviceListSnapshot,
        source_one: DeviceIdentity,
        source_two: DeviceIdentity,
        replacement: DeviceIdentity,
        membership: ConversationMembershipSnapshot,
        conversation_id: ConversationId,
        binding: PeerReenrollmentHistoryBinding,
    }

    fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let local_root = AccountRootState::create(root.path().join("local-root"))?;
        let history_root = AccountRootState::create(root.path().join("history-root"))?;
        let source_one = DeviceState::load_or_create(root.path().join("source-one"))?;
        let source_two = DeviceState::load_or_create(root.path().join("source-two"))?;
        let replacement = DeviceState::load_or_create(root.path().join("replacement"))?;
        let mut certificates = Vec::new();
        for device in [&source_one, &source_two, &replacement] {
            certificates.push(history_root.issue_device_certificate(
                device.identity().device_id(),
                device.encryption().public_key(),
                &DeviceCapability::MESSAGING,
            )?);
        }
        let history_authority = history_root.publish_device_list(&certificates)?;
        let conversation_id = ConversationId::from_label("peer-old-history-plan");
        let membership = history_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[local_root.account_id()],
        )?;
        let binding = PeerReenrollmentHistoryBinding {
            content: PeerReenrollmentHistoryBindingContent {
                local_account_id: local_root.account_id(),
                local_device_id: DeviceIdentity::from_secret_bytes([91_u8; 32]).device_id(),
                history_account_id: history_root.account_id(),
                replacement_device_id: replacement.identity().device_id(),
                history_authority_revision: history_authority.revision(),
                history_authority_digest: peer_reenrollment_authority_digest(&history_authority)?,
                acceptance_receipt_id: [41_u8; 32],
                transition_id: [42_u8; 32],
            },
        };
        Ok(Fixture {
            _root: root,
            history_root,
            history_certificates: certificates,
            local_account_id: local_root.account_id(),
            history_authority,
            source_one: DeviceIdentity::from_secret_bytes(source_one.identity().secret_bytes()),
            source_two: DeviceIdentity::from_secret_bytes(source_two.identity().secret_bytes()),
            replacement: DeviceIdentity::from_secret_bytes(replacement.identity().secret_bytes()),
            membership,
            conversation_id,
            binding,
        })
    }

    fn coverage(
        conversation_id: ConversationId,
        digest: [u8; 32],
    ) -> Result<ConversationHistoryCoverage, PeerHistoryRecoveryError> {
        ConversationHistoryCoverage::new(
            conversation_id,
            12,
            digest,
            vec![ReadableHistoryRange::new(0, 12)?],
        )
    }

    fn inventory(
        fixture: &Fixture,
        second_digest: [u8; 32],
        with_checkpoint: bool,
    ) -> Result<OldHistoryRecoveryInventory, PeerHistoryRecoveryError> {
        let claim_one = SignedHistorySourceClaim::issue(
            &fixture.source_one,
            &fixture.history_authority,
            vec![coverage(fixture.conversation_id, [7_u8; 32])?],
        )?;
        let claim_two = SignedHistorySourceClaim::issue(
            &fixture.source_two,
            &fixture.history_authority,
            vec![coverage(fixture.conversation_id, second_digest)?],
        )?;
        let checkpoint = if with_checkpoint {
            let sas = kilogram_protocol::HistoryRewrapSas::derive(
                &fixture.history_authority,
                fixture.source_one.device_id(),
                fixture.replacement.device_id(),
            )?;
            vec![SignedHistoryRecoveryCheckpoint::start(
                &fixture.replacement,
                fixture.history_authority.account_id(),
                fixture.conversation_id,
                fixture.source_one.device_id(),
                sas,
                0,
                12,
                4,
            )?]
        } else {
            Vec::new()
        };
        OldHistoryRecoveryInventory::assemble(
            &fixture.history_authority,
            fixture.replacement.device_id(),
            vec![claim_one, claim_two],
            &checkpoint,
        )
    }

    fn selection(fixture: &Fixture) -> [HistoryRecoveryRangeSelection; 1] {
        [HistoryRecoveryRangeSelection {
            conversation_id: fixture.conversation_id,
            range_start: 0,
            range_end: 12,
        }]
    }

    #[test]
    fn bounded_inventory_is_body_free_and_surfaces_agreement_and_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let current_inventory = inventory(&fixture, [7_u8; 32], true)?;
        let view = current_inventory.view()?;
        assert_eq!(view.source_count, 2);
        assert_eq!(view.conversation_count, 1);
        assert_eq!(view.checkpoint_count, 1);
        assert!(!view.contains_message_bodies);
        assert_eq!(
            view.conversations[0].agreement,
            SourceClaimAgreement::Agreed
        );
        assert_eq!(view.conversations[0].inventory_event_count, Some(12));
        assert_eq!(view.conversations[0].source_count, 2);
        assert_eq!(view.conversations[0].readable_range_count, 2);
        assert_ne!(view.conversations[0].conversation_token, [0_u8; 16]);
        Ok(())
    }

    #[test]
    fn separate_history_owner_consent_yields_content_addressed_signed_plan()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let current_inventory = inventory(&fixture, [7_u8; 32], true)?;
        let plan = OldHistoryRecoveryPlan::prepare_with_nonce(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &current_inventory,
            fixture.source_one.device_id(),
            &selection(&fixture),
            1_000,
            [13_u8; 32],
        )?;
        let plan_bytes = plan.encode()?;
        assert_eq!(
            OldHistoryRecoveryPlan::decode_and_validate(
                &plan_bytes,
                fixture.replacement.device_id(),
            )?,
            plan
        );
        let phrase = plan.confirmation_phrase()?;
        assert!(phrase.starts_with("RECOVER HISTORY "));
        let signed = plan.confirm_and_sign(
            &fixture.source_one,
            &phrase,
            ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
        )?;
        let encoded = signed.encode()?;
        let decoded = SignedOldHistoryRecoveryPlan::decode_and_verify(
            &encoded,
            fixture.source_one.device_id(),
            fixture.replacement.device_id(),
        )?;
        assert_eq!(decoded.plan_id()?, signed.plan_id()?);
        assert_eq!(decoded.transition_id(), fixture.binding.transition_id());
        assert_eq!(decoded.selection_count(), 1);
        decoded.verify_context(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &current_inventory,
            fixture.source_one.device_id(),
            &selection(&fixture),
            1_001,
        )?;
        Ok(())
    }

    #[test]
    fn reenrollment_acceptance_is_not_history_consent() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let inventory = inventory(&fixture, [7_u8; 32], false)?;
        let plan = OldHistoryRecoveryPlan::prepare_with_nonce(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &inventory,
            fixture.source_one.device_id(),
            &selection(&fixture),
            1_000,
            [14_u8; 32],
        )?;
        let phrase = plan.confirmation_phrase()?;
        assert!(matches!(
            plan.clone().confirm_and_sign(
                &fixture.source_one,
                &phrase,
                ExplicitOldHistoryRecoveryConsent::NotAuthorized,
            ),
            Err(PeerHistoryRecoveryError::ExplicitConsentRequired)
        ));
        assert!(matches!(
            plan.clone().confirm_and_sign(
                &fixture.source_one,
                "REENROLL 0000 0000 0000 0000",
                ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
            ),
            Err(PeerHistoryRecoveryError::ConfirmationMismatch)
        ));
        assert!(matches!(
            plan.confirm_and_sign(
                &fixture.replacement,
                &phrase,
                ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
            ),
            Err(PeerHistoryRecoveryError::WrongSourceDevice)
        ));
        Ok(())
    }

    #[test]
    fn divergent_source_claims_remain_visible_and_block_planning()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let inventory = inventory(&fixture, [8_u8; 32], false)?;
        let view = inventory.view()?;
        assert_eq!(
            view.conversations[0].agreement,
            SourceClaimAgreement::Divergent
        );
        assert_eq!(view.conversations[0].inventory_event_count, None);
        assert!(matches!(
            OldHistoryRecoveryPlan::prepare_with_nonce(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &inventory,
                fixture.source_one.device_id(),
                &selection(&fixture),
                1_000,
                [15_u8; 32],
            ),
            Err(PeerHistoryRecoveryError::DivergentSourceClaims)
        ));
        Ok(())
    }

    #[test]
    fn changed_inventory_membership_recipient_range_or_transition_invalidates_plan()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let inventory = inventory(&fixture, [7_u8; 32], false)?;
        let selections = selection(&fixture);
        let plan = OldHistoryRecoveryPlan::prepare_with_nonce(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &inventory,
            fixture.source_one.device_id(),
            &selections,
            1_000,
            [16_u8; 32],
        )?;
        plan.verify_context(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &inventory,
            fixture.source_one.device_id(),
            &selections,
            1_001,
        )?;

        let changed_inventory = OldHistoryRecoveryInventory::assemble(
            &fixture.history_authority,
            fixture.replacement.device_id(),
            vec![SignedHistorySourceClaim::issue(
                &fixture.source_one,
                &fixture.history_authority,
                vec![ConversationHistoryCoverage::new(
                    fixture.conversation_id,
                    12,
                    [7_u8; 32],
                    vec![
                        ReadableHistoryRange::new(0, 6)?,
                        ReadableHistoryRange::new(7, 12)?,
                    ],
                )?],
            )?],
            &[],
        )?;
        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &changed_inventory,
                fixture.source_one.device_id(),
                &selections,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::SelectionOutsideCoverage)
                | Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));

        let changed_range = [HistoryRecoveryRangeSelection {
            conversation_id: fixture.conversation_id,
            range_start: 0,
            range_end: 6,
        }];
        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &inventory,
                fixture.source_one.device_id(),
                &changed_range,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));

        let additional_member =
            AccountRootState::create(fixture._root.path().join("additional-membership-account"))?;
        let changed_membership = fixture.history_root.add_conversation_members(
            fixture.conversation_id.scope_id(),
            &[additional_member.account_id()],
        )?;
        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&changed_membership),
                &inventory,
                fixture.source_one.device_id(),
                &selections,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));

        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &inventory,
                fixture.source_two.device_id(),
                &selections,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));

        let additional_device =
            DeviceState::load_or_create(fixture._root.path().join("additional-history-device"))?;
        let additional_certificate = fixture.history_root.issue_device_certificate(
            additional_device.identity().device_id(),
            additional_device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let mut changed_certificates = fixture.history_certificates.clone();
        changed_certificates.push(additional_certificate);
        let changed_authority = fixture
            .history_root
            .publish_device_list(&changed_certificates)?;
        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &changed_authority,
                std::slice::from_ref(&fixture.membership),
                &inventory,
                fixture.source_one.device_id(),
                &selections,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::AuthorityMismatch)
        ));

        let mut changed_binding = fixture.binding.clone();
        changed_binding.content.transition_id = [99_u8; 32];
        assert!(matches!(
            plan.verify_context(
                &changed_binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &inventory,
                fixture.source_one.device_id(),
                &selections,
                1_001,
            ),
            Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));

        let wrong_recipient = DeviceIdentity::from_secret_bytes([88_u8; 32]);
        let wrong_inventory = OldHistoryRecoveryInventory::assemble(
            &fixture.history_authority,
            wrong_recipient.device_id(),
            vec![SignedHistorySourceClaim::issue(
                &fixture.source_one,
                &fixture.history_authority,
                vec![coverage(fixture.conversation_id, [7_u8; 32])?],
            )?],
            &[],
        );
        assert!(wrong_inventory.is_err());
        Ok(())
    }

    #[test]
    fn tamper_expiry_and_checkpoint_drift_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let current_inventory = inventory(&fixture, [7_u8; 32], true)?;
        let plan = OldHistoryRecoveryPlan::prepare_with_nonce(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &current_inventory,
            fixture.source_one.device_id(),
            &selection(&fixture),
            1_000,
            [17_u8; 32],
        )?;
        assert!(matches!(
            plan.verify_context(
                &fixture.binding,
                &fixture.history_authority,
                std::slice::from_ref(&fixture.membership),
                &current_inventory,
                fixture.source_one.device_id(),
                &selection(&fixture),
                2_000,
            ),
            Err(PeerHistoryRecoveryError::PlanExpired)
        ));
        let phrase = plan.confirmation_phrase()?;
        let signed = plan.confirm_and_sign(
            &fixture.source_one,
            &phrase,
            ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
        )?;
        let mut encoded = signed.encode()?;
        let middle = encoded.len() / 2;
        encoded[middle] ^= 1;
        assert!(
            SignedOldHistoryRecoveryPlan::decode_and_verify(
                &encoded,
                fixture.source_one.device_id(),
                fixture.replacement.device_id(),
            )
            .is_err()
        );

        let without_checkpoint = inventory(&fixture, [7_u8; 32], false)?;
        let context_result = signed.verify_context(
            &fixture.binding,
            &fixture.history_authority,
            std::slice::from_ref(&fixture.membership),
            &without_checkpoint,
            fixture.source_one.device_id(),
            &selection(&fixture),
            1_001,
        );
        assert!(matches!(
            context_result,
            Err(PeerHistoryRecoveryError::PlanContextMismatch)
        ));
        assert_ne!(
            fixture.local_account_id,
            fixture.history_authority.account_id()
        );
        Ok(())
    }
}
