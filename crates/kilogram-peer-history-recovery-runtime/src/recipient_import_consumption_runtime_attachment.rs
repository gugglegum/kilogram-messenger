//! Ephemeral attachment of an already authenticated guided-recovery context.
//!
//! The outer runtime IPC request deliberately cannot construct this value. A
//! trusted in-process recovery ceremony supplies the exact coordinator and
//! membership snapshots, and the slot destroys them on clear, expiry, or
//! process restart. Replacing a live context is never implicit.

use std::path::{Path, PathBuf};

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, ConversationMembershipSnapshot, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::{
    MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS, RuntimeIpcDescriptor,
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportMembershipReference,
    recipient_import_membership_set_digest,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    LoadedRecipientImportMembership, RecipientHistoryRecoveryImportMutationAdapter,
    RecipientImportConsumptionRuntimeContext, RecipientImportConsumptionRuntimeDispatch,
    RecipientImportConsumptionRuntimeService, RecipientImportConsumptionRuntimeServiceError,
    RecipientImportMembershipLoad, RecipientImportMembershipLoader,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentStore,
        recipient_import_membership_digest,
    },
};

const ATTACHMENT_ID_DOMAIN: &[u8] = b"kilogram:recipient-import-consumption-runtime-context:v1\0";

pub const MAX_RECIPIENT_IMPORT_CONSUMPTION_CONTEXT_LIFETIME_SECONDS: u64 =
    MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GuidedRecipientImportMembership {
    reference: RuntimeIpcRecipientImportMembershipReference,
    membership: ConversationMembershipSnapshot,
}

impl GuidedRecipientImportMembership {
    pub(crate) fn new(
        reference: RuntimeIpcRecipientImportMembershipReference,
        membership: ConversationMembershipSnapshot,
    ) -> Result<Self, RecipientImportConsumptionRuntimeAttachmentError> {
        let actual_digest = RuntimeIpcRecipientImportArtifactId::from_bytes(
            recipient_import_membership_digest(&membership).map_err(ContractError::from)?,
        )?;
        if reference.membership_digest != actual_digest {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::MembershipDigestMismatch);
        }
        Ok(Self {
            reference,
            membership,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        RuntimeIpcRecipientImportMembershipReference,
        ConversationMembershipSnapshot,
    ) {
        (self.reference, self.membership)
    }
}

/// Non-serializable authority hand-off emitted only by a successful guided
/// import mutation in this process. Its fields are private and there is no
/// public constructor, so IPC, CLI, environment, or caller-assembled recovery
/// fragments cannot manufacture it.
pub struct AuthenticatedGuidedRecipientImportCompletion {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    coordination: RecipientRecoveryCoordinationPlan,
    memberships: Vec<GuidedRecipientImportMembership>,
}

impl AuthenticatedGuidedRecipientImportCompletion {
    pub(crate) fn verify_runtime_binding(
        &self,
        state_directory: &Path,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<(), RecipientImportConsumptionRuntimeAttachmentError> {
        if self.state_directory != state_directory {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::StateDirectoryMismatch);
        }
        if self.owner_account_id != owner_account_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch);
        }
        if self.owner_device_id != owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_completed_mutation(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        coordination: RecipientRecoveryCoordinationPlan,
        references: &[RuntimeIpcRecipientImportMembershipReference],
        memberships: Vec<ConversationMembershipSnapshot>,
        expected_membership_set_digest: [u8; 32],
        final_coordinator_plan_id: [u8; 32],
        final_coordinator_sequence: u64,
    ) -> Result<Self, RecipientImportConsumptionRuntimeAttachmentError> {
        let state_directory = state_directory.as_ref().to_path_buf();
        if coordination.state_directory() != state_directory {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::StateDirectoryMismatch);
        }
        if coordination.owner_account_id() != owner_account_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch);
        }
        if coordination.owner_device_id() != owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch);
        }
        if coordination.plan_id() != final_coordinator_plan_id
            || coordination.sequence() != final_coordinator_sequence
            || coordination.requires_apply()
        {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::CoordinatorChanged);
        }
        if references.len() != memberships.len() {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::MembershipCountMismatch);
        }
        let actual_set_digest = recipient_import_membership_set_digest(references)?.to_bytes()?;
        if actual_set_digest != expected_membership_set_digest {
            return Err(
                RecipientImportConsumptionRuntimeAttachmentError::MembershipSetDigestMismatch,
            );
        }
        let memberships = references
            .iter()
            .cloned()
            .zip(memberships)
            .map(|(reference, membership)| {
                GuidedRecipientImportMembership::new(reference, membership)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            state_directory,
            owner_account_id,
            owner_device_id,
            coordination,
            memberships,
        })
    }
}

/// Converts the unforgeable, same-process completion hand-off into the
/// short-lived M1.2.46 context. Producing a context performs no I/O and opens
/// no network path.
pub struct GuidedRecipientImportConsumptionContextProducer;

impl GuidedRecipientImportConsumptionContextProducer {
    pub fn produce(
        completion: AuthenticatedGuidedRecipientImportCompletion,
        installed_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<
        GuidedRecipientImportConsumptionContext,
        RecipientImportConsumptionRuntimeAttachmentError,
    > {
        GuidedRecipientImportConsumptionContext::from_authenticated_recovery(
            completion.state_directory,
            completion.owner_account_id,
            completion.owner_device_id,
            completion.coordination,
            completion.memberships,
            installed_at_unix_seconds,
            lifetime_seconds,
        )
    }
}

#[derive(Serialize)]
struct AttachmentIdBinding<'a> {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    memberships: &'a [RuntimeIpcRecipientImportMembershipReference],
    installed_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

pub struct GuidedRecipientImportConsumptionContext {
    attachment_id: RuntimeIpcRecipientImportArtifactId,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    state_directory: PathBuf,
    mutations: RecipientHistoryRecoveryImportMutationAdapter,
    intent_store: RecipientRecoveryImportRoundIntentStore,
    coordination: RecipientRecoveryCoordinationPlan,
    inbox: RecipientHistoryTransferInbox,
    imports: RecipientHistoryImportStore,
    memberships: Vec<GuidedRecipientImportMembership>,
    installed_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

impl GuidedRecipientImportConsumptionContext {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_authenticated_recovery(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        coordination: RecipientRecoveryCoordinationPlan,
        memberships: Vec<GuidedRecipientImportMembership>,
        installed_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<Self, RecipientImportConsumptionRuntimeAttachmentError> {
        let state_directory = state_directory.as_ref().to_path_buf();
        if coordination.state_directory() != state_directory {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::StateDirectoryMismatch);
        }
        if coordination.owner_account_id() != owner_account_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch);
        }
        if coordination.owner_device_id() != owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch);
        }
        if installed_at_unix_seconds == 0
            || !(1..=MAX_RECIPIENT_IMPORT_CONSUMPTION_CONTEXT_LIFETIME_SECONDS)
                .contains(&lifetime_seconds)
        {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::InvalidLifetime);
        }
        let expires_at_unix_seconds = installed_at_unix_seconds
            .checked_add(lifetime_seconds)
            .ok_or(RecipientImportConsumptionRuntimeAttachmentError::InvalidLifetime)?;
        let references = memberships
            .iter()
            .map(|entry| entry.reference.clone())
            .collect::<Vec<_>>();
        recipient_import_membership_set_digest(&references)?;
        let encoded = postcard::to_allocvec(&AttachmentIdBinding {
            version: 1,
            owner_account_id,
            owner_device_id,
            coordinator_plan_id: coordination.plan_id(),
            coordinator_sequence: coordination.sequence(),
            memberships: &references,
            installed_at_unix_seconds,
            expires_at_unix_seconds,
        })
        .map_err(ContractError::from)?;
        let attachment_id = RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(
            ATTACHMENT_ID_DOMAIN,
            &encoded,
        ))?;
        Ok(Self {
            attachment_id,
            owner_account_id,
            owner_device_id,
            mutations: RecipientHistoryRecoveryImportMutationAdapter::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            intent_store: RecipientRecoveryImportRoundIntentStore::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            inbox: RecipientHistoryTransferInbox::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            imports: RecipientHistoryImportStore::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            state_directory,
            coordination,
            memberships,
            installed_at_unix_seconds,
            expires_at_unix_seconds,
        })
    }

    pub fn attachment_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.attachment_id
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.owner_device_id
    }

    pub fn state_directory(&self) -> &Path {
        &self.state_directory
    }

    pub fn installed_at_unix_seconds(&self) -> u64 {
        self.installed_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }

    pub fn membership_count(&self) -> usize {
        self.memberships.len()
    }

    pub fn is_durable(&self) -> bool {
        false
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    fn is_expired(&self, now_unix_seconds: u64) -> bool {
        now_unix_seconds > self.expires_at_unix_seconds
    }

    fn is_not_yet_valid(&self, now_unix_seconds: u64) -> bool {
        now_unix_seconds < self.installed_at_unix_seconds
    }

    fn as_runtime_context<'a>(
        &'a self,
        signer: &'a DeviceIdentity,
    ) -> RecipientImportConsumptionRuntimeContext<'a, Self> {
        RecipientImportConsumptionRuntimeContext {
            mutations: &self.mutations,
            intent_store: &self.intent_store,
            coordination: &self.coordination,
            inbox: &self.inbox,
            imports: &self.imports,
            membership_loader: self,
            signer,
        }
    }
}

impl RecipientImportMembershipLoader for GuidedRecipientImportConsumptionContext {
    fn load_membership(
        &self,
        reference: &RuntimeIpcRecipientImportMembershipReference,
    ) -> RecipientImportMembershipLoad {
        let mut matches = self
            .memberships
            .iter()
            .filter(|entry| entry.reference == *reference);
        let Some(exact) = matches.next() else {
            return RecipientImportMembershipLoad::Missing;
        };
        if matches.next().is_some() {
            return RecipientImportMembershipLoad::Ambiguous;
        }
        RecipientImportMembershipLoad::Exact(LoadedRecipientImportMembership::new(
            exact.reference.authority_reference_id.clone(),
            exact.reference.authenticated_context_id.clone(),
            exact.membership.clone(),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientImportConsumptionRuntimeContextInstallOutcome {
    Installed,
    ExactReplay,
}

pub struct RecipientImportConsumptionRuntimeContextSlot {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    attached: Option<GuidedRecipientImportConsumptionContext>,
}

impl RecipientImportConsumptionRuntimeContextSlot {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Self {
        Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            owner_account_id,
            owner_device_id,
            attached: None,
        }
    }

    pub fn install(
        &mut self,
        context: GuidedRecipientImportConsumptionContext,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeContextInstallOutcome,
        RecipientImportConsumptionRuntimeAttachmentError,
    > {
        if context.owner_account_id != self.owner_account_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch);
        }
        if context.owner_device_id != self.owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch);
        }
        if context.state_directory != self.state_directory {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::StateDirectoryMismatch);
        }
        if context.is_not_yet_valid(now_unix_seconds) {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ClockRollback);
        }
        if context.is_expired(now_unix_seconds) {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ContextExpired);
        }
        if let Some(existing) = self.attached.as_ref() {
            if existing.attachment_id == context.attachment_id {
                return Ok(RecipientImportConsumptionRuntimeContextInstallOutcome::ExactReplay);
            }
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ContextAlreadyAttached);
        }
        self.attached = Some(context);
        Ok(RecipientImportConsumptionRuntimeContextInstallOutcome::Installed)
    }

    pub fn clear(
        &mut self,
        expected_attachment_id: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<
        GuidedRecipientImportConsumptionContext,
        RecipientImportConsumptionRuntimeAttachmentError,
    > {
        let attached = self
            .attached
            .as_ref()
            .ok_or(RecipientImportConsumptionRuntimeAttachmentError::ContextUnavailable)?;
        if &attached.attachment_id != expected_attachment_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ContextIdMismatch);
        }
        self.attached
            .take()
            .ok_or(RecipientImportConsumptionRuntimeAttachmentError::ContextUnavailable)
    }

    pub fn purge_expired(&mut self, now_unix_seconds: u64) -> bool {
        if self
            .attached
            .as_ref()
            .is_some_and(|context| context.is_expired(now_unix_seconds))
        {
            self.attached = None;
            true
        } else {
            false
        }
    }

    pub fn attached(&self) -> bool {
        self.attached.is_some()
    }

    pub fn attached_context_id(&self) -> Option<&RuntimeIpcRecipientImportArtifactId> {
        self.attached.as_ref().map(|context| &context.attachment_id)
    }

    fn require(
        &self,
        signer: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        &GuidedRecipientImportConsumptionContext,
        RecipientImportConsumptionRuntimeAttachmentError,
    > {
        let attached = self
            .attached
            .as_ref()
            .ok_or(RecipientImportConsumptionRuntimeAttachmentError::ContextUnavailable)?;
        if attached.is_not_yet_valid(now_unix_seconds) {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ClockRollback);
        }
        if attached.is_expired(now_unix_seconds) {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::ContextExpired);
        }
        if signer.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::SignerMismatch);
        }
        Ok(attached)
    }
}

pub struct RecipientImportConsumptionRuntimeActor {
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    service: RecipientImportConsumptionRuntimeService,
    context_slot: RecipientImportConsumptionRuntimeContextSlot,
}

impl RecipientImportConsumptionRuntimeActor {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        maximum_sessions: usize,
    ) -> Result<Self, RecipientImportConsumptionRuntimeActorError> {
        let state_directory = state_directory.as_ref().to_path_buf();
        Ok(Self {
            owner_account_id,
            owner_device_id,
            service: RecipientImportConsumptionRuntimeService::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
                maximum_sessions,
            )?,
            context_slot: RecipientImportConsumptionRuntimeContextSlot::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
        })
    }

    pub fn install_authenticated_guided_recovery(
        &mut self,
        completion: AuthenticatedGuidedRecipientImportCompletion,
        now_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeContextInstallOutcome,
        RecipientImportConsumptionRuntimeActorError,
    > {
        completion.verify_runtime_binding(
            &self.context_slot.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let context = GuidedRecipientImportConsumptionContextProducer::produce(
            completion,
            now_unix_seconds,
            lifetime_seconds,
        )?;
        self.install_guided_context(context, now_unix_seconds)
    }

    pub fn install_guided_context(
        &mut self,
        context: GuidedRecipientImportConsumptionContext,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeContextInstallOutcome,
        RecipientImportConsumptionRuntimeActorError,
    > {
        let outcome = self.context_slot.install(context, now_unix_seconds)?;
        if outcome == RecipientImportConsumptionRuntimeContextInstallOutcome::Installed {
            self.service.clear_plan_sessions();
        }
        Ok(outcome)
    }

    pub fn clear_guided_context(
        &mut self,
        expected_attachment_id: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<usize, RecipientImportConsumptionRuntimeActorError> {
        self.context_slot.clear(expected_attachment_id)?;
        Ok(self.service.clear_plan_sessions())
    }

    pub fn context_attached(&self) -> bool {
        self.context_slot.attached()
    }

    pub fn context_id(&self) -> Option<&RuntimeIpcRecipientImportArtifactId> {
        self.context_slot.attached_context_id()
    }

    pub fn session_count(&self) -> usize {
        self.service.session_count()
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub fn dispatch(
        &mut self,
        encoded_request: &[u8],
        descriptor: &RuntimeIpcDescriptor,
        signer: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeDispatch,
        RecipientImportConsumptionRuntimeActorError,
    > {
        if descriptor.account_id() != self.owner_account_id {
            return Err(
                RecipientImportConsumptionRuntimeAttachmentError::OwnerAccountMismatch.into(),
            );
        }
        if descriptor.device_id() != self.owner_device_id {
            return Err(
                RecipientImportConsumptionRuntimeAttachmentError::OwnerDeviceMismatch.into(),
            );
        }
        if signer.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionRuntimeAttachmentError::SignerMismatch.into());
        }
        if self.context_slot.purge_expired(now_unix_seconds) {
            self.service.clear_plan_sessions();
            return Err(
                RecipientImportConsumptionRuntimeAttachmentError::ContextUnavailable.into(),
            );
        }
        let context = self.context_slot.require(signer, now_unix_seconds)?;
        self.service
            .dispatch_with_metadata(
                encoded_request,
                descriptor,
                context.as_runtime_context(signer),
                now_unix_seconds,
            )
            .map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionRuntimeAttachmentError {
    #[error("recipient import-consumption runtime service is not configured")]
    ContextUnavailable,
    #[error("recipient import-consumption guided context is already attached")]
    ContextAlreadyAttached,
    #[error("recipient import-consumption guided context ID does not match")]
    ContextIdMismatch,
    #[error("recipient import-consumption guided context has expired")]
    ContextExpired,
    #[error("recipient import-consumption guided context observes a clock rollback")]
    ClockRollback,
    #[error("recipient import-consumption guided context lifetime is invalid")]
    InvalidLifetime,
    #[error("recipient import-consumption guided context state directory changed")]
    StateDirectoryMismatch,
    #[error("recipient import-consumption guided context Account changed")]
    OwnerAccountMismatch,
    #[error("recipient import-consumption guided context Device changed")]
    OwnerDeviceMismatch,
    #[error("recipient import-consumption guided context signer changed")]
    SignerMismatch,
    #[error("recipient import-consumption guided membership digest changed")]
    MembershipDigestMismatch,
    #[error("recipient import-consumption guided membership-set digest changed")]
    MembershipSetDigestMismatch,
    #[error("recipient import-consumption guided membership count changed")]
    MembershipCountMismatch,
    #[error("recipient import-consumption guided coordinator changed")]
    CoordinatorChanged,
    #[error(transparent)]
    Contract(#[from] ContractError),
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionRuntimeActorError {
    #[error(transparent)]
    Attachment(#[from] RecipientImportConsumptionRuntimeAttachmentError),
    #[error(transparent)]
    Service(Box<RecipientImportConsumptionRuntimeServiceError>),
}

impl From<RecipientImportConsumptionRuntimeServiceError>
    for RecipientImportConsumptionRuntimeActorError
{
    fn from(error: RecipientImportConsumptionRuntimeServiceError) -> Self {
        Self::Service(Box::new(error))
    }
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}
