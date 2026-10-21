//! Bounded process-local custody for explicit import-consumption Plan stages.
//!
//! The cache deliberately stores the opaque dispatcher result instead of
//! reconstructing authority from a signed response. It is neither durable nor
//! serializable: a runtime restart, expiry, or capacity rejection requires a
//! fresh read-only Plan. Commit still runs through the response dispatcher,
//! which revalidates the exact request, descriptor, current Device and durable
//! state before any mutation.

use std::collections::VecDeque;

use kilogram_identity::DeviceIdentity;
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportArtifactId,
    RuntimeIpcRecipientImportConsumptionCommand, RuntimeIpcRecipientImportConsumptionOutcome,
    RuntimeIpcRecipientImportConsumptionPlanSummary, RuntimeIpcRecipientImportConsumptionRequest,
    SignedRuntimeIpcRecipientImportConsumptionResponse,
};
use thiserror::Error;

use crate::{
    RecipientHistoryRecoveryImportConsumptionResponseDispatcher,
    RecipientHistoryRecoveryImportMutationAdapter, RecipientImportConsumptionResponseDispatch,
    RecipientImportConsumptionResponseDispatcherError, RecipientImportMembershipLoader,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentStore,
    },
};

pub const MAX_RECIPIENT_IMPORT_CONSUMPTION_PLAN_SESSIONS: usize = 32;

struct RecipientImportConsumptionPlanSession {
    plan: RuntimeIpcRecipientImportConsumptionPlanSummary,
    plan_request_digest: [u8; 32],
    dispatch: RecipientImportConsumptionResponseDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientImportConsumptionPlanSessionInsertOutcome {
    Inserted,
    ExactReplay,
}

pub struct RecipientImportConsumptionPlanSessionCache {
    maximum_sessions: usize,
    sessions: VecDeque<RecipientImportConsumptionPlanSession>,
}

impl RecipientImportConsumptionPlanSessionCache {
    pub fn new(
        maximum_sessions: usize,
    ) -> Result<Self, RecipientImportConsumptionPlanSessionCacheError> {
        if !(1..=MAX_RECIPIENT_IMPORT_CONSUMPTION_PLAN_SESSIONS).contains(&maximum_sessions) {
            return Err(RecipientImportConsumptionPlanSessionCacheError::InvalidCapacity);
        }
        Ok(Self {
            maximum_sessions,
            sessions: VecDeque::with_capacity(maximum_sessions),
        })
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn maximum_sessions(&self) -> usize {
        self.maximum_sessions
    }

    pub fn is_durable(&self) -> bool {
        false
    }

    pub fn grants_commit_authority(&self) -> bool {
        false
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub fn insert(
        &mut self,
        dispatch: RecipientImportConsumptionResponseDispatch,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionPlanSessionInsertOutcome,
        RecipientImportConsumptionPlanSessionCacheError,
    > {
        self.purge_expired(now_unix_seconds);
        if !dispatch.plan_stage_available() {
            return Err(RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage);
        }
        let RuntimeIpcRecipientImportConsumptionOutcome::Planned(plan) =
            dispatch.response().outcome()
        else {
            return Err(RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage);
        };
        if plan.expired(now_unix_seconds) {
            return Err(RecipientImportConsumptionPlanSessionCacheError::ExpiredPlan);
        }
        let plan = plan.clone();
        let plan_request_digest = dispatch.response().request_digest();
        if let Some(existing) = self
            .sessions
            .iter()
            .find(|entry| entry.plan.consumption_plan_id == plan.consumption_plan_id)
        {
            if existing.plan == plan && existing.plan_request_digest == plan_request_digest {
                return Ok(RecipientImportConsumptionPlanSessionInsertOutcome::ExactReplay);
            }
            return Err(RecipientImportConsumptionPlanSessionCacheError::PlanCollision);
        }
        if self.sessions.len() == self.maximum_sessions {
            return Err(RecipientImportConsumptionPlanSessionCacheError::CapacityReached);
        }
        self.sessions
            .push_back(RecipientImportConsumptionPlanSession {
                plan,
                plan_request_digest,
                dispatch,
            });
        Ok(RecipientImportConsumptionPlanSessionInsertOutcome::Inserted)
    }

    pub fn contains(
        &mut self,
        consumption_plan_id: &RuntimeIpcRecipientImportArtifactId,
        now_unix_seconds: u64,
    ) -> bool {
        self.purge_expired(now_unix_seconds);
        self.sessions
            .iter()
            .any(|entry| &entry.plan.consumption_plan_id == consumption_plan_id)
    }

    /// Returns the exact signed response for a repeated canonical Plan request.
    ///
    /// The runtime must consult this before planning again: `planned_at` is
    /// runtime-owned, so redispatching the same request at a later instant
    /// would otherwise mint a second plan instead of replaying the first one.
    pub fn exact_plan_replay(
        &mut self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        now_unix_seconds: u64,
    ) -> Result<
        Option<SignedRuntimeIpcRecipientImportConsumptionResponse>,
        RecipientImportConsumptionPlanSessionCacheError,
    > {
        self.purge_expired(now_unix_seconds);
        if !matches!(
            request.command(),
            RuntimeIpcRecipientImportConsumptionCommand::Plan { .. }
        ) {
            return Err(RecipientImportConsumptionPlanSessionCacheError::StageMismatch);
        }
        let request_digest = request.request_digest()?;
        Ok(self
            .sessions
            .iter()
            .find(|entry| entry.plan_request_digest == request_digest)
            .map(|entry| entry.dispatch.response().clone()))
    }

    pub fn purge_expired(&mut self, now_unix_seconds: u64) -> usize {
        let before = self.sessions.len();
        self.sessions
            .retain(|entry| !entry.plan.expired(now_unix_seconds));
        before.saturating_sub(self.sessions.len())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_commit<L: RecipientImportMembershipLoader>(
        &mut self,
        dispatcher: &RecipientHistoryRecoveryImportConsumptionResponseDispatcher,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        descriptor: &RuntimeIpcDescriptor,
        mutations: &RecipientHistoryRecoveryImportMutationAdapter,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        signer: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionResponseDispatch,
        RecipientImportConsumptionPlanSessionCacheError,
    > {
        self.purge_expired(now_unix_seconds);
        let RuntimeIpcRecipientImportConsumptionCommand::Commit { plan, .. } = request.command()
        else {
            return Err(RecipientImportConsumptionPlanSessionCacheError::StageMismatch);
        };
        if plan.expired(now_unix_seconds) {
            return Err(RecipientImportConsumptionPlanSessionCacheError::ExpiredPlan);
        }
        let Some(session) = self
            .sessions
            .iter()
            .find(|entry| entry.plan.consumption_plan_id == plan.consumption_plan_id)
        else {
            return Err(RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage);
        };
        if &session.plan != plan.as_ref() {
            return Err(RecipientImportConsumptionPlanSessionCacheError::PlanMismatch);
        }
        dispatcher
            .dispatch_commit(
                &session.dispatch,
                request,
                descriptor,
                mutations,
                store,
                current,
                inbox,
                imports,
                loader,
                signer,
                now_unix_seconds,
            )
            .map_err(|error| {
                RecipientImportConsumptionPlanSessionCacheError::Dispatch(Box::new(error))
            })
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionPlanSessionCacheError {
    #[error("recipient import consumption Plan cache capacity is invalid")]
    InvalidCapacity,
    #[error("recipient import consumption Plan cache reached its bounded capacity")]
    CapacityReached,
    #[error("recipient import consumption Plan cache received no opaque Plan stage")]
    MissingPlanStage,
    #[error("recipient import consumption Plan cache received an expired Plan")]
    ExpiredPlan,
    #[error("recipient import consumption Plan cache detected a plan-id collision")]
    PlanCollision,
    #[error("recipient import consumption Commit names a different cached Plan")]
    PlanMismatch,
    #[error("recipient import consumption Plan cache stage does not match request")]
    StageMismatch,
    #[error(transparent)]
    Contract(#[from] anyhow::Error),
    #[error(transparent)]
    Dispatch(Box<RecipientImportConsumptionResponseDispatcherError>),
}
