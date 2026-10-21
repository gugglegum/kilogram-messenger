//! Process-owned service core for the explicit import-consumption ceremony.
//!
//! One service owns one bounded, non-durable Plan-stage cache. Callers must
//! supply the already authenticated live recovery context; the service never
//! reconstructs source authority from the nested request and never opens a
//! network path. This is the operational seam used by the active runtime once
//! a guided recovery context is attached to that process.

use std::path::Path;

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportConsumptionCommand,
    RuntimeIpcRecipientImportConsumptionOutcome, RuntimeIpcRecipientImportConsumptionRequest,
};
use thiserror::Error;

use crate::{
    RecipientHistoryRecoveryImportConsumptionResponseDispatcher,
    RecipientHistoryRecoveryImportMutationAdapter, RecipientImportConsumptionPlanSessionCache,
    RecipientImportConsumptionPlanSessionCacheError,
    RecipientImportConsumptionResponseDispatcherError, RecipientImportMembershipLoader,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentStore,
    },
};

/// The live, already authenticated recovery state required to consume one
/// completed import intent. It contains references only and grants no lifetime
/// beyond the current dispatch call.
pub struct RecipientImportConsumptionRuntimeContext<'a, L> {
    pub mutations: &'a RecipientHistoryRecoveryImportMutationAdapter,
    pub intent_store: &'a RecipientRecoveryImportRoundIntentStore,
    pub coordination: &'a RecipientRecoveryCoordinationPlan,
    pub inbox: &'a RecipientHistoryTransferInbox,
    pub imports: &'a RecipientHistoryImportStore,
    pub membership_loader: &'a L,
    pub signer: &'a DeviceIdentity,
}

pub struct RecipientImportConsumptionRuntimeService {
    dispatcher: RecipientHistoryRecoveryImportConsumptionResponseDispatcher,
    sessions: RecipientImportConsumptionPlanSessionCache,
}

pub struct RecipientImportConsumptionRuntimeDispatch {
    signed_response: Vec<u8>,
    state_changed: bool,
}

impl RecipientImportConsumptionRuntimeDispatch {
    pub fn signed_response(&self) -> &[u8] {
        &self.signed_response
    }

    pub fn into_signed_response(self) -> Vec<u8> {
        self.signed_response
    }

    pub fn state_changed(&self) -> bool {
        self.state_changed
    }
}

impl RecipientImportConsumptionRuntimeService {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        maximum_sessions: usize,
    ) -> Result<Self, RecipientImportConsumptionRuntimeServiceError> {
        Ok(Self {
            dispatcher: RecipientHistoryRecoveryImportConsumptionResponseDispatcher::new(
                state_directory,
                owner_account_id,
                owner_device_id,
            ),
            sessions: RecipientImportConsumptionPlanSessionCache::new(maximum_sessions)?,
        })
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn maximum_sessions(&self) -> usize {
        self.sessions.maximum_sessions()
    }

    pub fn clear_plan_sessions(&mut self) -> usize {
        self.sessions.clear()
    }

    pub fn is_process_local(&self) -> bool {
        !self.sessions.is_durable()
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<L: RecipientImportMembershipLoader>(
        &mut self,
        encoded_request: &[u8],
        descriptor: &RuntimeIpcDescriptor,
        context: RecipientImportConsumptionRuntimeContext<'_, L>,
        now_unix_seconds: u64,
    ) -> Result<Vec<u8>, RecipientImportConsumptionRuntimeServiceError> {
        Ok(self
            .dispatch_with_metadata(encoded_request, descriptor, context, now_unix_seconds)?
            .into_signed_response())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_with_metadata<L: RecipientImportMembershipLoader>(
        &mut self,
        encoded_request: &[u8],
        descriptor: &RuntimeIpcDescriptor,
        context: RecipientImportConsumptionRuntimeContext<'_, L>,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionRuntimeDispatch,
        RecipientImportConsumptionRuntimeServiceError,
    > {
        let request = RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
            encoded_request,
            descriptor,
        )?;
        let response = match request.command() {
            RuntimeIpcRecipientImportConsumptionCommand::Plan { .. } => {
                if let Some(replayed) = self
                    .sessions
                    .exact_plan_replay(&request, now_unix_seconds)?
                {
                    replayed
                } else {
                    let dispatch = self.dispatcher.dispatch_plan(
                        &request,
                        descriptor,
                        context.mutations,
                        context.intent_store,
                        context.coordination,
                        context.inbox,
                        context.imports,
                        context.membership_loader,
                        context.signer,
                        now_unix_seconds,
                    )?;
                    let response = dispatch.response().clone();
                    if matches!(
                        response.outcome(),
                        RuntimeIpcRecipientImportConsumptionOutcome::Planned(_)
                    ) {
                        self.sessions.insert(dispatch, now_unix_seconds)?;
                    }
                    response
                }
            }
            RuntimeIpcRecipientImportConsumptionCommand::Commit { .. } => self
                .sessions
                .dispatch_commit(
                    &self.dispatcher,
                    &request,
                    descriptor,
                    context.mutations,
                    context.intent_store,
                    context.coordination,
                    context.inbox,
                    context.imports,
                    context.membership_loader,
                    context.signer,
                    now_unix_seconds,
                )?
                .response()
                .clone(),
        };
        let state_changed = matches!(
            response.outcome(),
            RuntimeIpcRecipientImportConsumptionOutcome::Consumed(_)
        );
        Ok(RecipientImportConsumptionRuntimeDispatch {
            signed_response: response.encode()?,
            state_changed,
        })
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionRuntimeServiceError {
    #[error(transparent)]
    Session(#[from] RecipientImportConsumptionPlanSessionCacheError),
    #[error(transparent)]
    Dispatch(Box<RecipientImportConsumptionResponseDispatcherError>),
    #[error(transparent)]
    Contract(#[from] ContractError),
}

impl From<RecipientImportConsumptionResponseDispatcherError>
    for RecipientImportConsumptionRuntimeServiceError
{
    fn from(error: RecipientImportConsumptionResponseDispatcherError) -> Self {
        Self::Dispatch(Box::new(error))
    }
}
