//! Process-owned controller for one explicit guided recipient-import session.
//!
//! The controller is deliberately dormant. It is not registered with runtime
//! IPC, CLI, desktop, a timer, or a network worker. A future local UX owner may
//! pass only the unforgeable in-process start token; paths, coordinator objects,
//! membership snapshots, and signing secrets are not accepted by session
//! operations.

use std::path::{Path, PathBuf};

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, ConversationMembershipSnapshot, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportArtifactId,
    RuntimeIpcRecipientImportExecutionCommand,
    RuntimeIpcRecipientImportExecutionConfirmationSummary,
    RuntimeIpcRecipientImportExecutionOutcome, RuntimeIpcRecipientImportExecutionPlanSummary,
    RuntimeIpcRecipientImportExecutionReceiptSummary, RuntimeIpcRecipientImportExecutionRequest,
    RuntimeIpcRecipientImportExplicitApproval, RuntimeIpcRecipientImportMembershipReference,
    RuntimeIpcRequestId, recipient_import_membership_set_digest,
};
use serde::Serialize;
use thiserror::Error;

use crate::{
    DurableRecipientImportConfirmation, LoadedRecipientImportMembership,
    RecipientHistoryRecoveryImportMutationAdapter, RecipientHistoryRecoveryImportReadAdapter,
    RecipientImportConfirmationCommitOutcome, RecipientImportConfirmationLedger,
    RecipientImportConfirmationLedgerError, RecipientImportConsumptionRuntimeActor,
    RecipientImportConsumptionRuntimeActorError,
    RecipientImportConsumptionRuntimeContextInstallOutcome,
    RecipientImportConsumptionRuntimeDispatch, RecipientImportGuidedRecoveryHostInstallationOwner,
    RecipientImportMembershipLoad, RecipientImportMembershipLoader,
    RecipientImportMutationAdapterError, RecipientImportReadAdapterDispatch,
    RecipientImportReadAdapterError,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentStore,
        recipient_import_membership_digest,
    },
    recipient_import_consumption_runtime_attachment::GuidedRecipientImportMembership,
};

const SESSION_ID_DOMAIN: &[u8] = b"kilogram:recipient-import-guided-session:v1\0";
const RECOVERY_LAUNCH_BINDING_DOMAIN: &[u8] =
    b"kilogram:recipient-import-guided-recovery-launch-binding:v1\0";

pub const MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS: u64 = 5 * 60;

/// In-process authority supplied by an authenticated recovery ceremony.
///
/// There is intentionally no public constructor, serialization implementation,
/// clone implementation, or debug representation. Session operations consume
/// this value and never accept its sensitive components independently.
pub struct AuthenticatedGuidedRecipientImportSession {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    descriptor: RuntimeIpcDescriptor,
    plan_request: RuntimeIpcRecipientImportExecutionRequest,
    coordination: RecipientRecoveryCoordinationPlan,
    references: Vec<RuntimeIpcRecipientImportMembershipReference>,
    memberships: Vec<ConversationMembershipSnapshot>,
}

impl AuthenticatedGuidedRecipientImportSession {
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn from_authenticated_recovery(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        descriptor: RuntimeIpcDescriptor,
        plan_request: RuntimeIpcRecipientImportExecutionRequest,
        coordination: RecipientRecoveryCoordinationPlan,
        memberships: Vec<GuidedRecipientImportMembership>,
    ) -> Result<Self, RecipientImportGuidedSessionControllerError> {
        let (references, memberships): (Vec<_>, Vec<_>) = memberships
            .into_iter()
            .map(GuidedRecipientImportMembership::into_parts)
            .unzip();
        let authority = Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            owner_account_id,
            owner_device_id,
            descriptor,
            plan_request,
            coordination,
            references,
            memberships,
        };
        authority.verify_authenticated_recovery()?;
        Ok(authority)
    }

    fn verify_authenticated_recovery(
        &self,
    ) -> Result<(), RecipientImportGuidedSessionControllerError> {
        if self.coordination.state_directory() != self.state_directory {
            return Err(RecipientImportGuidedSessionControllerError::StateDirectoryMismatch);
        }
        if self.coordination.owner_account_id() != self.owner_account_id
            || self.descriptor.account_id() != self.owner_account_id
        {
            return Err(RecipientImportGuidedSessionControllerError::OwnerAccountMismatch);
        }
        if self.coordination.owner_device_id() != self.owner_device_id
            || self.descriptor.device_id() != self.owner_device_id
        {
            return Err(RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch);
        }
        let encoded = self.plan_request.encode()?;
        RuntimeIpcRecipientImportExecutionRequest::decode_and_validate(&encoded, &self.descriptor)?;
        let RuntimeIpcRecipientImportExecutionCommand::Plan {
            coordinator_plan_id,
            coordinator_sequence,
            memberships: requested_memberships,
            ..
        } = self.plan_request.command()
        else {
            return Err(RecipientImportGuidedSessionControllerError::NotPlanRequest);
        };
        if coordinator_plan_id.to_bytes()? != self.coordination.plan_id()
            || *coordinator_sequence != self.coordination.sequence()
        {
            return Err(RecipientImportGuidedSessionControllerError::CoordinatorChanged);
        }
        if self.references.len() != self.memberships.len() {
            return Err(RecipientImportGuidedSessionControllerError::MembershipSetChanged);
        }
        recipient_import_membership_set_digest(&self.references)?;
        if requested_memberships != &self.references {
            return Err(RecipientImportGuidedSessionControllerError::MembershipSetChanged);
        }
        for (reference, membership) in self.references.iter().zip(&self.memberships) {
            let digest = RuntimeIpcRecipientImportArtifactId::from_bytes(
                recipient_import_membership_digest(membership).map_err(ContractError::from)?,
            )?;
            if reference.membership_digest != digest {
                return Err(RecipientImportGuidedSessionControllerError::MembershipSetChanged);
            }
        }
        Ok(())
    }

    fn verify_controller_binding(
        &self,
        state_directory: &Path,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<(), RecipientImportGuidedSessionControllerError> {
        if self.state_directory != state_directory {
            return Err(RecipientImportGuidedSessionControllerError::StateDirectoryMismatch);
        }
        if self.owner_account_id != owner_account_id {
            return Err(RecipientImportGuidedSessionControllerError::OwnerAccountMismatch);
        }
        if self.owner_device_id != owner_device_id {
            return Err(RecipientImportGuidedSessionControllerError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    /// Consumes the complete authenticated recovery assembly and binds it to
    /// the exact local signer that may create the process-owned host.
    ///
    /// The resulting value is intentionally crate-private and opaque. It is
    /// the only input accepted by the M1.2.52 installation owner, so paths,
    /// descriptors, coordinators, and memberships never become loose public
    /// installation arguments.
    #[allow(dead_code)]
    pub(crate) fn into_host_installation(
        self,
        signer: &DeviceIdentity,
    ) -> Result<
        AuthenticatedGuidedRecipientImportHostInstallation<'_>,
        RecipientImportGuidedSessionControllerError,
    > {
        self.verify_controller_binding(
            &self.state_directory,
            self.owner_account_id,
            signer.device_id(),
        )?;
        Ok(AuthenticatedGuidedRecipientImportHostInstallation {
            authority: self,
            signer,
        })
    }
}

/// Test-only construction seam for the older controller, local-UX, producer,
/// and host regression fixtures.
///
/// Production code cannot call the loose session constructor. New production
/// assembly must enter through `RecipientImportGuidedRecoveryAssemblyIssuer`.
#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticated_guided_recipient_import_session_fixture(
    state_directory: impl AsRef<Path>,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    descriptor: RuntimeIpcDescriptor,
    plan_request: RuntimeIpcRecipientImportExecutionRequest,
    coordination: RecipientRecoveryCoordinationPlan,
    memberships: Vec<GuidedRecipientImportMembership>,
) -> Result<AuthenticatedGuidedRecipientImportSession, RecipientImportGuidedSessionControllerError>
{
    AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(
        state_directory,
        owner_account_id,
        owner_device_id,
        descriptor,
        plan_request,
        coordination,
        memberships,
    )
}

/// Complete, one-shot input for the process-owned recovery-host installer.
///
/// This value has no public constructor and deliberately implements neither
/// Clone, Debug, Serialize, nor Deserialize.
pub(crate) struct AuthenticatedGuidedRecipientImportHostInstallation<'identity> {
    authority: AuthenticatedGuidedRecipientImportSession,
    signer: &'identity DeviceIdentity,
}

impl<'identity> AuthenticatedGuidedRecipientImportHostInstallation<'identity> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        PathBuf,
        AccountId,
        &'identity DeviceIdentity,
        AuthenticatedGuidedRecipientImportSession,
    ) {
        let state_directory = self.authority.state_directory.clone();
        let owner_account_id = self.authority.owner_account_id;
        (
            state_directory,
            owner_account_id,
            self.signer,
            self.authority,
        )
    }
}

/// Complete authenticated input retained by exactly one process-local issuer.
///
/// This value is private, non-cloneable, and non-serializable. It owns the
/// canonical signed Plan result, exact coordinator, and already-loaded ordered
/// memberships until `issue` consumes the surrounding issuer.
struct AuthenticatedGuidedRecipientImportAssemblyInput<'identity> {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    signer: &'identity DeviceIdentity,
    descriptor: RuntimeIpcDescriptor,
    plan_request: RuntimeIpcRecipientImportExecutionRequest,
    coordination: RecipientRecoveryCoordinationPlan,
    memberships: Vec<GuidedRecipientImportMembership>,
}

/// One-shot issuer for one complete authenticated guided-recovery assembly.
///
/// There is deliberately no public constructor. The issuer exposes no
/// authority fragments and can emit only the M1.2.52 installation owner.
/// Issuing consumes `self` before validation, so both success and failure make
/// the input unavailable for replay.
pub struct RecipientImportGuidedRecoveryAssemblyIssuer<'identity> {
    input: AuthenticatedGuidedRecipientImportAssemblyInput<'identity>,
}

/// Safe, non-authoritative commitment used by the local launch-confirmation
/// layer. It deliberately reveals only a membership count and an opaque
/// digest; it cannot recreate, inspect, or issue the retained assembly.
pub(crate) struct AuthenticatedGuidedRecoveryLaunchBinding {
    assembly_digest: [u8; 32],
    membership_count: usize,
    membership_count_u64: u64,
}

impl AuthenticatedGuidedRecoveryLaunchBinding {
    pub(crate) fn into_parts(self) -> ([u8; 32], usize, u64) {
        (
            self.assembly_digest,
            self.membership_count,
            self.membership_count_u64,
        )
    }
}

impl<'identity> RecipientImportGuidedRecoveryAssemblyIssuer<'identity> {
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_authenticated_plan_assembly(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        signer: &'identity DeviceIdentity,
        descriptor: RuntimeIpcDescriptor,
        plan_request: RuntimeIpcRecipientImportExecutionRequest,
        coordination: RecipientRecoveryCoordinationPlan,
        memberships: Vec<GuidedRecipientImportMembership>,
    ) -> Self {
        Self {
            input: AuthenticatedGuidedRecipientImportAssemblyInput {
                state_directory: state_directory.as_ref().to_path_buf(),
                owner_account_id,
                owner_device_id,
                signer,
                descriptor,
                plan_request,
                coordination,
                memberships,
            },
        }
    }

    /// Creates only a safe commitment for an explicit local launch preview.
    ///
    /// The issuer remains owned by its caller. No path, identity, signer,
    /// descriptor, coordinator, request, membership snapshot, or reusable
    /// authority is returned.
    pub(crate) fn launch_binding(
        &self,
    ) -> Result<AuthenticatedGuidedRecoveryLaunchBinding, RecipientImportGuidedSessionControllerError>
    {
        let encoded_plan_request = self.input.plan_request.encode()?;
        let membership_count = self.input.memberships.len();
        let membership_count_u64 = u64::try_from(membership_count)
            .map_err(|_| RecipientImportGuidedSessionControllerError::MembershipSetChanged)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(RECOVERY_LAUNCH_BINDING_DOMAIN);
        hasher.update(&encoded_plan_request);
        hasher.update(&self.input.coordination.plan_id());
        hasher.update(&self.input.coordination.sequence().to_le_bytes());
        hasher.update(&membership_count_u64.to_le_bytes());
        Ok(AuthenticatedGuidedRecoveryLaunchBinding {
            assembly_digest: *hasher.finalize().as_bytes(),
            membership_count,
            membership_count_u64,
        })
    }

    pub fn issue(
        self,
    ) -> Result<
        RecipientImportGuidedRecoveryHostInstallationOwner<'identity>,
        RecipientImportGuidedSessionControllerError,
    > {
        let AuthenticatedGuidedRecipientImportAssemblyInput {
            state_directory,
            owner_account_id,
            owner_device_id,
            signer,
            descriptor,
            plan_request,
            coordination,
            memberships,
        } = self.input;

        let authority = AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery(
            state_directory,
            owner_account_id,
            owner_device_id,
            descriptor,
            plan_request,
            coordination,
            memberships,
        )?;
        let installation = authority.into_host_installation(signer)?;
        Ok(
            RecipientImportGuidedRecoveryHostInstallationOwner::from_authenticated_recovery(
                installation,
            ),
        )
    }
}

impl RecipientImportMembershipLoader for AuthenticatedGuidedRecipientImportSession {
    fn load_membership(
        &self,
        reference: &RuntimeIpcRecipientImportMembershipReference,
    ) -> RecipientImportMembershipLoad {
        let mut matches = self
            .references
            .iter()
            .zip(&self.memberships)
            .filter(|(candidate, _)| *candidate == reference);
        let Some((exact_reference, membership)) = matches.next() else {
            return RecipientImportMembershipLoad::Missing;
        };
        if matches.next().is_some() {
            return RecipientImportMembershipLoad::Ambiguous;
        }
        RecipientImportMembershipLoad::Exact(LoadedRecipientImportMembership::new(
            exact_reference.authority_reference_id.clone(),
            exact_reference.authenticated_context_id.clone(),
            membership.clone(),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientImportGuidedSessionPhase {
    Idle,
    Planned,
    Confirmed,
    Attached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionPlan {
    session_id: RuntimeIpcRecipientImportArtifactId,
    summary: RuntimeIpcRecipientImportExecutionPlanSummary,
    expires_at_unix_seconds: u64,
}

impl RecipientImportGuidedSessionPlan {
    pub fn session_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.session_id
    }

    pub fn summary(&self) -> &RuntimeIpcRecipientImportExecutionPlanSummary {
        &self.summary
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionConfirmation {
    session_id: RuntimeIpcRecipientImportArtifactId,
    request_id: RuntimeIpcRequestId,
    confirmation_nonce: RuntimeIpcRecipientImportArtifactId,
    confirmation_phrase: String,
}

impl RecipientImportGuidedSessionConfirmation {
    pub fn new(
        session_id: RuntimeIpcRecipientImportArtifactId,
        request_id: RuntimeIpcRequestId,
        confirmation_nonce: RuntimeIpcRecipientImportArtifactId,
        confirmation_phrase: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            request_id,
            confirmation_nonce,
            confirmation_phrase: confirmation_phrase.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionConfirmationReceipt {
    session_id: RuntimeIpcRecipientImportArtifactId,
    confirmation: RuntimeIpcRecipientImportExecutionConfirmationSummary,
    expires_at_unix_seconds: u64,
}

impl RecipientImportGuidedSessionConfirmationReceipt {
    pub fn session_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.session_id
    }

    pub fn confirmation(&self) -> &RuntimeIpcRecipientImportExecutionConfirmationSummary {
        &self.confirmation
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionMutation {
    session_id: RuntimeIpcRecipientImportArtifactId,
    request_id: RuntimeIpcRequestId,
    mutation_nonce: RuntimeIpcRecipientImportArtifactId,
}

impl RecipientImportGuidedSessionMutation {
    pub fn new(
        session_id: RuntimeIpcRecipientImportArtifactId,
        request_id: RuntimeIpcRequestId,
        mutation_nonce: RuntimeIpcRecipientImportArtifactId,
    ) -> Self {
        Self {
            session_id,
            request_id,
            mutation_nonce,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionAttachment {
    session_id: RuntimeIpcRecipientImportArtifactId,
    attachment_id: RuntimeIpcRecipientImportArtifactId,
    receipt: RuntimeIpcRecipientImportExecutionReceiptSummary,
    expires_at_unix_seconds: u64,
}

impl RecipientImportGuidedSessionAttachment {
    pub fn session_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.session_id
    }

    pub fn attachment_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.attachment_id
    }

    pub fn receipt(&self) -> &RuntimeIpcRecipientImportExecutionReceiptSummary {
        &self.receipt
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }
}

struct PlannedGuidedRecipientImportSession {
    session_id: RuntimeIpcRecipientImportArtifactId,
    authority: AuthenticatedGuidedRecipientImportSession,
    dispatch: RecipientImportReadAdapterDispatch,
    expires_at_unix_seconds: u64,
}

struct ConfirmedGuidedRecipientImportSession {
    session_id: RuntimeIpcRecipientImportArtifactId,
    authority: AuthenticatedGuidedRecipientImportSession,
    summary: RuntimeIpcRecipientImportExecutionPlanSummary,
    confirmation: DurableRecipientImportConfirmation,
    expires_at_unix_seconds: u64,
}

enum ActiveGuidedRecipientImportSession {
    Planned(Box<PlannedGuidedRecipientImportSession>),
    Confirmed(Box<ConfirmedGuidedRecipientImportSession>),
    Attached {
        session_id: RuntimeIpcRecipientImportArtifactId,
        attachment_id: RuntimeIpcRecipientImportArtifactId,
        descriptor: RuntimeIpcDescriptor,
        receipt: Box<RuntimeIpcRecipientImportExecutionReceiptSummary>,
        expires_at_unix_seconds: u64,
    },
}

pub(crate) enum RecipientImportGuidedSessionLocalSnapshot {
    Idle,
    Planned {
        session_id: RuntimeIpcRecipientImportArtifactId,
        completed_source_count: u8,
        total_source_count: u8,
        planned_step_count: u8,
        confirmation_phrase: String,
        expires_at_unix_seconds: u64,
    },
    Confirmed {
        session_id: RuntimeIpcRecipientImportArtifactId,
        completed_source_count: u8,
        total_source_count: u8,
        planned_step_count: u8,
        expires_at_unix_seconds: u64,
    },
    Attached {
        session_id: RuntimeIpcRecipientImportArtifactId,
        final_completed_source_count: u8,
        total_source_count: u8,
        imported_event_count: u64,
        complete: bool,
        expires_at_unix_seconds: u64,
    },
}

impl ActiveGuidedRecipientImportSession {
    fn session_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        match self {
            Self::Planned(session) => &session.session_id,
            Self::Confirmed(session) => &session.session_id,
            Self::Attached { session_id, .. } => session_id,
        }
    }

    fn expires_at_unix_seconds(&self) -> u64 {
        match self {
            Self::Planned(session) => session.expires_at_unix_seconds,
            Self::Confirmed(session) => session.expires_at_unix_seconds,
            Self::Attached {
                expires_at_unix_seconds,
                ..
            } => *expires_at_unix_seconds,
        }
    }

    fn phase(&self) -> RecipientImportGuidedSessionPhase {
        match self {
            Self::Planned(_) => RecipientImportGuidedSessionPhase::Planned,
            Self::Confirmed(_) => RecipientImportGuidedSessionPhase::Confirmed,
            Self::Attached { .. } => RecipientImportGuidedSessionPhase::Attached,
        }
    }
}

pub struct RecipientImportGuidedSessionController<'identity> {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    signer: &'identity DeviceIdentity,
    intent_store: RecipientRecoveryImportRoundIntentStore,
    inbox: RecipientHistoryTransferInbox,
    imports: RecipientHistoryImportStore,
    confirmations: RecipientImportConfirmationLedger,
    mutations: RecipientHistoryRecoveryImportMutationAdapter,
    actor: RecipientImportConsumptionRuntimeActor,
    active: Option<ActiveGuidedRecipientImportSession>,
}

impl<'identity> RecipientImportGuidedSessionController<'identity> {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        signer: &'identity DeviceIdentity,
    ) -> Result<Self, RecipientImportGuidedSessionControllerError> {
        let state_directory = state_directory.as_ref().to_path_buf();
        let owner_device_id = signer.device_id();
        Ok(Self {
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
            confirmations: RecipientImportConfirmationLedger::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            mutations: RecipientHistoryRecoveryImportMutationAdapter::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
            ),
            actor: RecipientImportConsumptionRuntimeActor::new(
                &state_directory,
                owner_account_id,
                owner_device_id,
                1,
            )?,
            state_directory,
            owner_account_id,
            owner_device_id,
            signer,
            active: None,
        })
    }

    pub fn phase(&self) -> RecipientImportGuidedSessionPhase {
        self.active
            .as_ref()
            .map_or(RecipientImportGuidedSessionPhase::Idle, |session| {
                session.phase()
            })
    }

    pub fn active_session_id(&self) -> Option<&RuntimeIpcRecipientImportArtifactId> {
        self.active
            .as_ref()
            .map(ActiveGuidedRecipientImportSession::session_id)
    }

    pub fn context_attached(&self) -> bool {
        self.actor.context_attached()
    }

    pub fn consumption_plan_session_count(&self) -> usize {
        self.actor.session_count()
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub(crate) fn local_ux_snapshot(
        &self,
    ) -> Result<
        RecipientImportGuidedSessionLocalSnapshot,
        RecipientImportGuidedSessionControllerError,
    > {
        Ok(match self.active.as_ref() {
            None => RecipientImportGuidedSessionLocalSnapshot::Idle,
            Some(ActiveGuidedRecipientImportSession::Planned(session)) => {
                let summary = session
                    .dispatch
                    .planned_summary()
                    .ok_or(RecipientImportGuidedSessionControllerError::PlanRejected)?;
                RecipientImportGuidedSessionLocalSnapshot::Planned {
                    session_id: session.session_id.clone(),
                    completed_source_count: summary.completed_source_count,
                    total_source_count: summary.total_source_count,
                    planned_step_count: summary.planned_step_count,
                    confirmation_phrase: summary.confirmation_phrase.clone(),
                    expires_at_unix_seconds: session.expires_at_unix_seconds,
                }
            }
            Some(ActiveGuidedRecipientImportSession::Confirmed(session)) => {
                RecipientImportGuidedSessionLocalSnapshot::Confirmed {
                    session_id: session.session_id.clone(),
                    completed_source_count: session.summary.completed_source_count,
                    total_source_count: session.summary.total_source_count,
                    planned_step_count: session.summary.planned_step_count,
                    expires_at_unix_seconds: session.expires_at_unix_seconds,
                }
            }
            Some(ActiveGuidedRecipientImportSession::Attached {
                session_id,
                receipt,
                expires_at_unix_seconds,
                ..
            }) => RecipientImportGuidedSessionLocalSnapshot::Attached {
                session_id: session_id.clone(),
                final_completed_source_count: receipt.final_completed_source_count,
                total_source_count: receipt.total_source_count,
                imported_event_count: receipt.imported_event_count,
                complete: receipt.complete,
                expires_at_unix_seconds: *expires_at_unix_seconds,
            },
        })
    }

    pub fn begin(
        &mut self,
        authority: AuthenticatedGuidedRecipientImportSession,
        now_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionPlan, RecipientImportGuidedSessionControllerError> {
        let dispatch = self.prepare_start(&authority, now_unix_seconds, lifetime_seconds)?;
        let summary = dispatch
            .planned_summary()
            .cloned()
            .ok_or(RecipientImportGuidedSessionControllerError::PlanRejected)?;
        let expires_at_unix_seconds = now_unix_seconds
            .checked_add(lifetime_seconds)
            .ok_or(RecipientImportGuidedSessionControllerError::InvalidLifetime)?;
        let session_id = session_id(
            self.owner_account_id,
            self.owner_device_id,
            authority.plan_request.request_digest()?,
            summary.execution_plan_id.to_bytes()?,
            now_unix_seconds,
            expires_at_unix_seconds,
        )?;
        let result = RecipientImportGuidedSessionPlan {
            session_id: session_id.clone(),
            summary: summary.clone(),
            expires_at_unix_seconds,
        };
        self.active = Some(ActiveGuidedRecipientImportSession::Planned(Box::new(
            PlannedGuidedRecipientImportSession {
                session_id,
                authority,
                dispatch,
                expires_at_unix_seconds,
            },
        )));
        Ok(result)
    }

    pub(crate) fn preflight_local_ux_start(
        &mut self,
        authority: &AuthenticatedGuidedRecipientImportSession,
        now_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<(), RecipientImportGuidedSessionControllerError> {
        self.prepare_start(authority, now_unix_seconds, lifetime_seconds)?;
        Ok(())
    }

    fn prepare_start(
        &mut self,
        authority: &AuthenticatedGuidedRecipientImportSession,
        now_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<RecipientImportReadAdapterDispatch, RecipientImportGuidedSessionControllerError>
    {
        self.purge_expired(now_unix_seconds)?;
        if self.active.is_some() {
            return Err(RecipientImportGuidedSessionControllerError::SessionAlreadyActive);
        }
        validate_lifetime(now_unix_seconds, lifetime_seconds)?;
        authority.verify_controller_binding(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        authority.verify_authenticated_recovery()?;
        if self.actor.context_attached() {
            return Err(RecipientImportGuidedSessionControllerError::ActorStateChanged);
        }
        let dispatch = RecipientHistoryRecoveryImportReadAdapter::dispatch(
            &authority.plan_request,
            &self.intent_store,
            &authority.coordination,
            &self.inbox,
            &self.imports,
            authority,
            None,
        )?;
        dispatch
            .planned_summary()
            .ok_or(RecipientImportGuidedSessionControllerError::PlanRejected)?;
        Ok(dispatch)
    }

    pub fn confirm(
        &mut self,
        request: RecipientImportGuidedSessionConfirmation,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionConfirmationReceipt,
        RecipientImportGuidedSessionControllerError,
    > {
        let active = self
            .active
            .take()
            .ok_or(RecipientImportGuidedSessionControllerError::SessionUnavailable)?;
        let ActiveGuidedRecipientImportSession::Planned(planned) = active else {
            self.detach_active(active)?;
            return Err(RecipientImportGuidedSessionControllerError::WrongPhase);
        };
        let planned = *planned;
        if request.session_id != planned.session_id {
            return Err(RecipientImportGuidedSessionControllerError::SessionIdMismatch);
        }
        ensure_not_expired(planned.expires_at_unix_seconds, now_unix_seconds)?;
        let summary = planned
            .dispatch
            .planned_summary()
            .cloned()
            .ok_or(RecipientImportGuidedSessionControllerError::PlanRejected)?;
        let confirm_request = RuntimeIpcRecipientImportExecutionRequest::new(
            &planned.authority.descriptor,
            request.request_id,
            RuntimeIpcRecipientImportExecutionCommand::Confirm {
                intent_id: summary.intent_id.clone(),
                execution_plan_id: summary.execution_plan_id.clone(),
                coordinator_plan_id: summary.coordinator_plan_id.clone(),
                coordinator_sequence: summary.coordinator_sequence,
                memberships: planned.authority.references.clone(),
                confirmation_nonce: request.confirmation_nonce,
                confirmation_phrase: request.confirmation_phrase,
                explicit_approval: RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
            },
        )?;
        let plan = self.confirmations.plan(
            &planned.dispatch,
            &confirm_request,
            self.signer,
            now_unix_seconds,
            planned.expires_at_unix_seconds,
        )?;
        let confirmation = match self.confirmations.commit(&plan, now_unix_seconds)? {
            RecipientImportConfirmationCommitOutcome::Committed(confirmation)
            | RecipientImportConfirmationCommitOutcome::AlreadyCommitted(confirmation) => {
                confirmation
            }
        };
        let result = RecipientImportGuidedSessionConfirmationReceipt {
            session_id: planned.session_id.clone(),
            confirmation: confirmation.summary()?,
            expires_at_unix_seconds: planned.expires_at_unix_seconds,
        };
        self.active = Some(ActiveGuidedRecipientImportSession::Confirmed(Box::new(
            ConfirmedGuidedRecipientImportSession {
                session_id: planned.session_id,
                authority: planned.authority,
                summary,
                confirmation,
                expires_at_unix_seconds: planned.expires_at_unix_seconds,
            },
        )));
        Ok(result)
    }

    pub fn apply(
        &mut self,
        request: RecipientImportGuidedSessionMutation,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionAttachment, RecipientImportGuidedSessionControllerError>
    {
        let active = self
            .active
            .take()
            .ok_or(RecipientImportGuidedSessionControllerError::SessionUnavailable)?;
        let ActiveGuidedRecipientImportSession::Confirmed(confirmed) = active else {
            self.detach_active(active)?;
            return Err(RecipientImportGuidedSessionControllerError::WrongPhase);
        };
        let confirmed = *confirmed;
        if request.session_id != confirmed.session_id {
            return Err(RecipientImportGuidedSessionControllerError::SessionIdMismatch);
        }
        ensure_not_expired(confirmed.expires_at_unix_seconds, now_unix_seconds)?;
        let apply_request = RuntimeIpcRecipientImportExecutionRequest::new(
            &confirmed.authority.descriptor,
            request.request_id,
            RuntimeIpcRecipientImportExecutionCommand::ApplyBounded {
                intent_id: confirmed.summary.intent_id.clone(),
                execution_plan_id: confirmed.summary.execution_plan_id.clone(),
                coordinator_plan_id: confirmed.summary.coordinator_plan_id.clone(),
                coordinator_sequence: confirmed.summary.coordinator_sequence,
                memberships: confirmed.authority.references.clone(),
                confirmation_id: RuntimeIpcRecipientImportArtifactId::from_bytes(
                    confirmed.confirmation.confirmation_id(),
                )?,
                mutation_nonce: request.mutation_nonce,
                max_steps: confirmed.summary.max_steps,
            },
        )?;
        let dispatch = self.mutations.dispatch(
            &apply_request,
            &self.intent_store,
            &confirmed.authority.coordination,
            &self.inbox,
            &self.imports,
            &confirmed.authority,
            self.signer,
            now_unix_seconds,
        )?;
        let RuntimeIpcRecipientImportExecutionOutcome::Executed(receipt) = dispatch.outcome()
        else {
            return Err(RecipientImportGuidedSessionControllerError::MutationRejected);
        };
        let receipt = receipt.clone();
        if !receipt.complete {
            return Err(RecipientImportGuidedSessionControllerError::MutationIncomplete);
        }
        let completion = dispatch
            .into_authenticated_guided_completion()
            .ok_or(RecipientImportGuidedSessionControllerError::MissingAuthenticatedCompletion)?;
        let remaining_lifetime = confirmed
            .expires_at_unix_seconds
            .checked_sub(now_unix_seconds)
            .filter(|remaining| *remaining != 0)
            .ok_or(RecipientImportGuidedSessionControllerError::SessionExpired)?;
        let install = self.actor.install_authenticated_guided_recovery(
            completion,
            now_unix_seconds,
            remaining_lifetime,
        )?;
        let attachment_id = self
            .actor
            .context_id()
            .cloned()
            .ok_or(RecipientImportGuidedSessionControllerError::ActorStateChanged)?;
        if install == RecipientImportConsumptionRuntimeContextInstallOutcome::ExactReplay {
            self.actor.clear_guided_context(&attachment_id)?;
            return Err(RecipientImportGuidedSessionControllerError::ActorStateChanged);
        }
        let result = RecipientImportGuidedSessionAttachment {
            session_id: confirmed.session_id.clone(),
            attachment_id: attachment_id.clone(),
            receipt: receipt.clone(),
            expires_at_unix_seconds: confirmed.expires_at_unix_seconds,
        };
        self.active = Some(ActiveGuidedRecipientImportSession::Attached {
            session_id: confirmed.session_id,
            attachment_id,
            descriptor: confirmed.authority.descriptor,
            receipt: Box::new(receipt),
            expires_at_unix_seconds: confirmed.expires_at_unix_seconds,
        });
        Ok(result)
    }

    pub fn dispatch_consumption(
        &mut self,
        expected_session_id: &RuntimeIpcRecipientImportArtifactId,
        encoded_request: &[u8],
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeDispatch,
        RecipientImportGuidedSessionControllerError,
    > {
        if self.purge_expired(now_unix_seconds)? {
            return Err(RecipientImportGuidedSessionControllerError::SessionExpired);
        }
        let Some(ActiveGuidedRecipientImportSession::Attached {
            session_id,
            descriptor,
            ..
        }) = self.active.as_ref()
        else {
            return Err(RecipientImportGuidedSessionControllerError::WrongPhase);
        };
        if session_id != expected_session_id {
            return Err(RecipientImportGuidedSessionControllerError::SessionIdMismatch);
        }
        self.actor
            .dispatch(encoded_request, descriptor, self.signer, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn cancel(
        &mut self,
        expected_session_id: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<usize, RecipientImportGuidedSessionControllerError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RecipientImportGuidedSessionControllerError::SessionUnavailable)?;
        if active.session_id() != expected_session_id {
            return Err(RecipientImportGuidedSessionControllerError::SessionIdMismatch);
        }
        let active = self
            .active
            .take()
            .ok_or(RecipientImportGuidedSessionControllerError::SessionUnavailable)?;
        self.detach_active(active)
    }

    pub fn purge_expired(
        &mut self,
        now_unix_seconds: u64,
    ) -> Result<bool, RecipientImportGuidedSessionControllerError> {
        let Some(active) = self.active.as_ref() else {
            return Ok(false);
        };
        if now_unix_seconds < active.expires_at_unix_seconds() {
            return Ok(false);
        }
        let active = self
            .active
            .take()
            .ok_or(RecipientImportGuidedSessionControllerError::SessionUnavailable)?;
        self.detach_active(active)?;
        Ok(true)
    }

    fn detach_active(
        &mut self,
        active: ActiveGuidedRecipientImportSession,
    ) -> Result<usize, RecipientImportGuidedSessionControllerError> {
        match active {
            ActiveGuidedRecipientImportSession::Attached { attachment_id, .. } => self
                .actor
                .clear_guided_context(&attachment_id)
                .map_err(Into::into),
            ActiveGuidedRecipientImportSession::Planned(_)
            | ActiveGuidedRecipientImportSession::Confirmed(_) => Ok(0),
        }
    }
}

#[derive(Serialize)]
struct SessionIdBinding {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    plan_request_digest: [u8; 32],
    execution_plan_id: [u8; 32],
    started_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

fn session_id(
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    plan_request_digest: [u8; 32],
    execution_plan_id: [u8; 32],
    started_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportGuidedSessionControllerError> {
    let encoded = postcard::to_allocvec(&SessionIdBinding {
        version: 1,
        owner_account_id,
        owner_device_id,
        plan_request_digest,
        execution_plan_id,
        started_at_unix_seconds,
        expires_at_unix_seconds,
    })
    .map_err(ContractError::from)?;
    RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(SESSION_ID_DOMAIN, &encoded))
        .map_err(Into::into)
}

fn validate_lifetime(
    now_unix_seconds: u64,
    lifetime_seconds: u64,
) -> Result<(), RecipientImportGuidedSessionControllerError> {
    if now_unix_seconds == 0
        || !(1..=MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS).contains(&lifetime_seconds)
        || now_unix_seconds.checked_add(lifetime_seconds).is_none()
    {
        return Err(RecipientImportGuidedSessionControllerError::InvalidLifetime);
    }
    Ok(())
}

fn ensure_not_expired(
    expires_at_unix_seconds: u64,
    now_unix_seconds: u64,
) -> Result<(), RecipientImportGuidedSessionControllerError> {
    if now_unix_seconds >= expires_at_unix_seconds {
        return Err(RecipientImportGuidedSessionControllerError::SessionExpired);
    }
    Ok(())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedSessionControllerError {
    #[error("guided recipient-import session is already active")]
    SessionAlreadyActive,
    #[error("guided recipient-import session is unavailable")]
    SessionUnavailable,
    #[error("guided recipient-import session ID does not match")]
    SessionIdMismatch,
    #[error("guided recipient-import session is in the wrong phase")]
    WrongPhase,
    #[error("guided recipient-import session lifetime is invalid")]
    InvalidLifetime,
    #[error("guided recipient-import session expired")]
    SessionExpired,
    #[error("guided recipient-import session state directory changed")]
    StateDirectoryMismatch,
    #[error("guided recipient-import session Account changed")]
    OwnerAccountMismatch,
    #[error("guided recipient-import session Device changed")]
    OwnerDeviceMismatch,
    #[error("guided recipient-import session coordinator changed")]
    CoordinatorChanged,
    #[error("guided recipient-import session membership set changed")]
    MembershipSetChanged,
    #[error("guided recipient-import session requires a Plan request")]
    NotPlanRequest,
    #[error("guided recipient-import Plan was rejected")]
    PlanRejected,
    #[error("guided recipient-import mutation was rejected")]
    MutationRejected,
    #[error("guided recipient-import mutation did not complete the intent")]
    MutationIncomplete,
    #[error("guided recipient-import mutation emitted no authenticated completion")]
    MissingAuthenticatedCompletion,
    #[error("guided recipient-import runtime actor state changed")]
    ActorStateChanged,
    #[error(transparent)]
    Contract(Box<ContractError>),
    #[error(transparent)]
    ReadAdapter(Box<RecipientImportReadAdapterError>),
    #[error(transparent)]
    Confirmation(Box<RecipientImportConfirmationLedgerError>),
    #[error(transparent)]
    Mutation(Box<RecipientImportMutationAdapterError>),
    #[error(transparent)]
    Actor(Box<RecipientImportConsumptionRuntimeActorError>),
}

impl From<ContractError> for RecipientImportGuidedSessionControllerError {
    fn from(error: ContractError) -> Self {
        Self::Contract(Box::new(error))
    }
}

impl From<RecipientImportReadAdapterError> for RecipientImportGuidedSessionControllerError {
    fn from(error: RecipientImportReadAdapterError) -> Self {
        Self::ReadAdapter(Box::new(error))
    }
}

impl From<RecipientImportConfirmationLedgerError> for RecipientImportGuidedSessionControllerError {
    fn from(error: RecipientImportConfirmationLedgerError) -> Self {
        Self::Confirmation(Box::new(error))
    }
}

impl From<RecipientImportMutationAdapterError> for RecipientImportGuidedSessionControllerError {
    fn from(error: RecipientImportMutationAdapterError) -> Self {
        Self::Mutation(Box::new(error))
    }
}

impl From<RecipientImportConsumptionRuntimeActorError>
    for RecipientImportGuidedSessionControllerError
{
    fn from(error: RecipientImportConsumptionRuntimeActorError) -> Self {
        Self::Actor(Box::new(error))
    }
}
