//! Dormant signed response dispatcher for the explicit import-consumption ceremony.
//!
//! Planning remains read-only. Commit requires the opaque plan-stage result
//! produced by this dispatcher, durably records the exact explicit
//! authorization, performs the crash-safe intent consumption, and only then
//! signs the response with the current descriptor Device. Nothing in this
//! module registers an active IPC command or opens a network path.

use std::path::{Path, PathBuf};

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportConsumptionCommand,
    RuntimeIpcRecipientImportConsumptionRequest,
    SignedRuntimeIpcRecipientImportConsumptionResponse,
};
use thiserror::Error;

use crate::{
    RecipientHistoryRecoveryImportConsumptionMutationAdapter,
    RecipientHistoryRecoveryImportConsumptionReadAdapter,
    RecipientHistoryRecoveryImportMutationAdapter,
    RecipientImportConsumptionAuthorizationCommitOutcome,
    RecipientImportConsumptionAuthorizationLedger,
    RecipientImportConsumptionAuthorizationLedgerError,
    RecipientImportConsumptionMutationAdapterError, RecipientImportConsumptionReadAdapterDispatch,
    RecipientImportConsumptionReadAdapterError, RecipientImportMembershipLoader,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentStore,
    },
};

pub struct RecipientImportConsumptionResponseDispatch {
    response: SignedRuntimeIpcRecipientImportConsumptionResponse,
    plan_request: Option<RuntimeIpcRecipientImportConsumptionRequest>,
    plan_dispatch: Option<RecipientImportConsumptionReadAdapterDispatch>,
    authorization_committed_before_mutation: bool,
    authorization_replayed: bool,
    mutation_receipt_persisted_before_response: bool,
    mutation_replayed: bool,
}

impl RecipientImportConsumptionResponseDispatch {
    pub fn response(&self) -> &SignedRuntimeIpcRecipientImportConsumptionResponse {
        &self.response
    }

    pub fn plan_stage_available(&self) -> bool {
        self.plan_request.is_some() && self.plan_dispatch.is_some()
    }

    pub fn authorization_committed_before_mutation(&self) -> bool {
        self.authorization_committed_before_mutation
    }

    pub fn authorization_replayed(&self) -> bool {
        self.authorization_replayed
    }

    pub fn mutation_receipt_persisted_before_response(&self) -> bool {
        self.mutation_receipt_persisted_before_response
    }

    pub fn mutation_replayed(&self) -> bool {
        self.mutation_replayed
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub fn registered_active_runtime_command(&self) -> bool {
        false
    }
}

pub struct RecipientHistoryRecoveryImportConsumptionResponseDispatcher {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientHistoryRecoveryImportConsumptionResponseDispatcher {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Self {
        Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            owner_account_id,
            owner_device_id,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_plan<L: RecipientImportMembershipLoader>(
        &self,
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
        RecipientImportConsumptionResponseDispatcherError,
    > {
        self.validate_owner(descriptor, signer)?;
        let request = revalidate_request(request, descriptor)?;
        if !matches!(
            request.command(),
            RuntimeIpcRecipientImportConsumptionCommand::Plan { .. }
        ) {
            return Err(RecipientImportConsumptionResponseDispatcherError::StageMismatch);
        }
        let dispatch = RecipientHistoryRecoveryImportConsumptionReadAdapter::dispatch(
            &request,
            mutations,
            store,
            current,
            inbox,
            imports,
            loader,
            now_unix_seconds,
        )?;
        let response = SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
            &request,
            descriptor,
            signer,
            dispatch.outcome().clone(),
        )?;
        let planned = dispatch.ephemeral_plan().is_some();
        Ok(RecipientImportConsumptionResponseDispatch {
            response,
            plan_request: planned.then_some(request),
            plan_dispatch: planned.then_some(dispatch),
            authorization_committed_before_mutation: false,
            authorization_replayed: false,
            mutation_receipt_persisted_before_response: false,
            mutation_replayed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_commit<L: RecipientImportMembershipLoader>(
        &self,
        planned: &RecipientImportConsumptionResponseDispatch,
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
        RecipientImportConsumptionResponseDispatcherError,
    > {
        self.validate_owner(descriptor, signer)?;
        let request = revalidate_request(request, descriptor)?;
        if !matches!(
            request.command(),
            RuntimeIpcRecipientImportConsumptionCommand::Commit { .. }
        ) {
            return Err(RecipientImportConsumptionResponseDispatcherError::StageMismatch);
        }
        let plan_request = planned
            .plan_request
            .as_ref()
            .ok_or(RecipientImportConsumptionResponseDispatcherError::MissingPlanStage)?;
        let plan_dispatch = planned
            .plan_dispatch
            .as_ref()
            .ok_or(RecipientImportConsumptionResponseDispatcherError::MissingPlanStage)?;
        let exact_plan_request = revalidate_request(plan_request, descriptor)?;
        let encoded_plan_response = planned.response.encode()?;
        SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify(
            &encoded_plan_response,
            &exact_plan_request,
            descriptor,
        )?;

        let authorizations = RecipientImportConsumptionAuthorizationLedger::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        );
        let authorization_plan = authorizations.plan(plan_dispatch, &request, signer)?;
        let authorization = authorizations.commit(&authorization_plan, now_unix_seconds)?;
        let authorization_replayed = matches!(
            authorization,
            RecipientImportConsumptionAuthorizationCommitOutcome::AlreadyCommitted(_)
        );

        let consumption = RecipientHistoryRecoveryImportConsumptionMutationAdapter::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )
        .dispatch(
            &request,
            &authorizations,
            mutations,
            store,
            current,
            inbox,
            imports,
            loader,
            signer,
            now_unix_seconds,
        )?;
        let response = SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
            &request,
            descriptor,
            signer,
            consumption.outcome().clone(),
        )?;
        Ok(RecipientImportConsumptionResponseDispatch {
            response,
            plan_request: None,
            plan_dispatch: None,
            authorization_committed_before_mutation: true,
            authorization_replayed,
            mutation_receipt_persisted_before_response: consumption
                .persisted_receipt_before_response(),
            mutation_replayed: consumption.replayed_completed_receipt()
                || consumption.reconciled_consumed_intent(),
        })
    }

    fn validate_owner(
        &self,
        descriptor: &RuntimeIpcDescriptor,
        signer: &DeviceIdentity,
    ) -> Result<(), RecipientImportConsumptionResponseDispatcherError> {
        if descriptor.account_id() != self.owner_account_id {
            return Err(
                RecipientImportConsumptionResponseDispatcherError::DescriptorAccountMismatch,
            );
        }
        if descriptor.device_id() != self.owner_device_id
            || signer.device_id() != self.owner_device_id
        {
            return Err(
                RecipientImportConsumptionResponseDispatcherError::DescriptorDeviceMismatch,
            );
        }
        Ok(())
    }
}

fn revalidate_request(
    request: &RuntimeIpcRecipientImportConsumptionRequest,
    descriptor: &RuntimeIpcDescriptor,
) -> Result<
    RuntimeIpcRecipientImportConsumptionRequest,
    RecipientImportConsumptionResponseDispatcherError,
> {
    Ok(
        RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
            &request.encode()?,
            descriptor,
        )?,
    )
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionResponseDispatcherError {
    #[error("recipient import consumption response dispatcher stage does not match request")]
    StageMismatch,
    #[error("recipient import consumption response dispatcher has no exact plan stage")]
    MissingPlanStage,
    #[error("recipient import consumption response descriptor Account does not match")]
    DescriptorAccountMismatch,
    #[error("recipient import consumption response descriptor Device does not match")]
    DescriptorDeviceMismatch,
    #[error(transparent)]
    Read(#[from] RecipientImportConsumptionReadAdapterError),
    #[error(transparent)]
    Authorization(#[from] RecipientImportConsumptionAuthorizationLedgerError),
    #[error(transparent)]
    Mutation(#[from] RecipientImportConsumptionMutationAdapterError),
    #[error(transparent)]
    Contract(#[from] ContractError),
}
