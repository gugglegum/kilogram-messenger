//! Dormant cross-process contract for the explicit recipient import-intent
//! consumption ceremony.
//!
//! Import execution never consumes an intent implicitly. This separate
//! contract first creates a short-lived, content-addressed plan bound to one
//! completed mutation record and receipt, then requires a distinct explicit
//! commit request. It is deliberately not registered in the active runtime
//! command enum.

use anyhow::{Context, Result, ensure};
use kilogram_identity::DeviceIdentity;
use serde::{Deserialize, Serialize};

use super::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportArtifactId,
    RuntimeIpcRecipientImportExecutionReceiptSummary, RuntimeIpcRecipientImportMembershipReference,
    RuntimeIpcRequestId, recipient_import_membership_set_digest,
};

pub const RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION: u8 = 1;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_RESPONSE_BYTES: usize = 8 * 1024;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS: u64 = 300;

const REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-consumption-request:v1\0";
const DESCRIPTOR_BINDING_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-consumption-descriptor:v1\0";
const DEVICE_BINDING_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-consumption-device:v1\0";
const EVIDENCE_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-consumption-evidence:v1\0";
const PLAN_ID_DOMAIN: &[u8] = b"kilogram:runtime-ipc-recipient-import-consumption-plan:v1\0";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-consumption-response:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportCompletedMutationEvidence {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub execution_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub confirmation_id: RuntimeIpcRecipientImportArtifactId,
    pub mutation_nonce: RuntimeIpcRecipientImportArtifactId,
    pub completed_mutation_record_id: RuntimeIpcRecipientImportArtifactId,
    pub completed_mutation_receipt_digest: RuntimeIpcRecipientImportArtifactId,
    pub membership_set_digest: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_sequence: u64,
    pub completed_source_count: u8,
    pub total_source_count: u8,
    pub imported_event_count: u64,
}

impl RuntimeIpcRecipientImportCompletedMutationEvidence {
    pub fn from_completed_receipt(
        completed_mutation_record_id: RuntimeIpcRecipientImportArtifactId,
        membership_set_digest: RuntimeIpcRecipientImportArtifactId,
        receipt: &RuntimeIpcRecipientImportExecutionReceiptSummary,
    ) -> Result<Self> {
        receipt.validate()?;
        ensure!(
            receipt.complete && receipt.final_completed_source_count == receipt.total_source_count,
            "runtime IPC recipient import consumption evidence requires a complete mutation receipt"
        );
        let evidence = Self {
            intent_id: receipt.intent_id.clone(),
            execution_plan_id: receipt.execution_plan_id.clone(),
            confirmation_id: receipt.confirmation_id.clone(),
            mutation_nonce: receipt.mutation_nonce.clone(),
            completed_mutation_record_id,
            completed_mutation_receipt_digest: receipt.receipt_digest()?,
            membership_set_digest,
            final_coordinator_plan_id: receipt.final_coordinator_plan_id.clone(),
            final_coordinator_sequence: receipt.final_coordinator_sequence,
            completed_source_count: receipt.final_completed_source_count,
            total_source_count: receipt.total_source_count,
            imported_event_count: receipt.imported_event_count,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn evidence_digest(&self) -> Result<RuntimeIpcRecipientImportArtifactId> {
        self.validate()?;
        let encoded = postcard::to_allocvec(self)
            .context("encode runtime IPC recipient import consumption evidence")?;
        RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(
            EVIDENCE_DIGEST_DOMAIN,
            &encoded,
        ))
    }

    fn validate(&self) -> Result<()> {
        for artifact in [
            &self.intent_id,
            &self.execution_plan_id,
            &self.confirmation_id,
            &self.mutation_nonce,
            &self.completed_mutation_record_id,
            &self.completed_mutation_receipt_digest,
            &self.membership_set_digest,
            &self.final_coordinator_plan_id,
        ] {
            validate_artifact(artifact)?;
        }
        ensure!(
            self.completed_source_count != 0
                && self.completed_source_count == self.total_source_count
                && usize::from(self.total_source_count)
                    <= super::MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES,
            "runtime IPC recipient import consumption evidence is not complete"
        );
        ensure!(
            self.completed_mutation_record_id != self.completed_mutation_receipt_digest
                && self.completed_mutation_record_id != self.mutation_nonce
                && self.completed_mutation_receipt_digest != self.mutation_nonce,
            "runtime IPC recipient import consumption evidence domains overlap"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportConsumptionExplicitApproval {
    NotAuthorized,
    UserConfirmed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportConsumptionPlanBinding {
    evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
    consumption_nonce: RuntimeIpcRecipientImportArtifactId,
    planned_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportConsumptionPlanSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub consumption_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub evidence_digest: RuntimeIpcRecipientImportArtifactId,
    pub completed_mutation_record_id: RuntimeIpcRecipientImportArtifactId,
    pub completed_mutation_receipt_digest: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_sequence: u64,
    pub consumption_nonce: RuntimeIpcRecipientImportArtifactId,
    pub planned_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub confirmation_phrase: String,
}

impl RuntimeIpcRecipientImportConsumptionPlanSummary {
    pub fn issue(
        evidence: &RuntimeIpcRecipientImportCompletedMutationEvidence,
        consumption_nonce: RuntimeIpcRecipientImportArtifactId,
        planned_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<Self> {
        evidence.validate()?;
        validate_artifact(&consumption_nonce)?;
        ensure!(
            consumption_nonce != evidence.mutation_nonce
                && consumption_nonce != evidence.completed_mutation_record_id
                && consumption_nonce != evidence.completed_mutation_receipt_digest,
            "runtime IPC recipient import consumption nonce reuses mutation evidence"
        );
        ensure!(
            planned_at_unix_seconds != 0
                && (1..=MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS)
                    .contains(&lifetime_seconds),
            "runtime IPC recipient import consumption plan lifetime is invalid"
        );
        let expires_at_unix_seconds = planned_at_unix_seconds
            .checked_add(lifetime_seconds)
            .context("runtime IPC recipient import consumption plan expiry overflow")?;
        let binding = RuntimeIpcRecipientImportConsumptionPlanBinding {
            evidence: evidence.clone(),
            consumption_nonce: consumption_nonce.clone(),
            planned_at_unix_seconds,
            expires_at_unix_seconds,
        };
        let consumption_plan_id = plan_id(&binding)?;
        let summary = Self {
            intent_id: evidence.intent_id.clone(),
            consumption_plan_id: consumption_plan_id.clone(),
            evidence_digest: evidence.evidence_digest()?,
            completed_mutation_record_id: evidence.completed_mutation_record_id.clone(),
            completed_mutation_receipt_digest: evidence.completed_mutation_receipt_digest.clone(),
            final_coordinator_plan_id: evidence.final_coordinator_plan_id.clone(),
            final_coordinator_sequence: evidence.final_coordinator_sequence,
            consumption_nonce,
            planned_at_unix_seconds,
            expires_at_unix_seconds,
            confirmation_phrase: recipient_import_consumption_confirmation_phrase(
                &consumption_plan_id,
            ),
        };
        summary.validate_against_evidence(evidence)?;
        Ok(summary)
    }

    pub fn expired(&self, now_unix_seconds: u64) -> bool {
        now_unix_seconds > self.expires_at_unix_seconds
    }

    fn validate(&self) -> Result<()> {
        for artifact in [
            &self.intent_id,
            &self.consumption_plan_id,
            &self.evidence_digest,
            &self.completed_mutation_record_id,
            &self.completed_mutation_receipt_digest,
            &self.final_coordinator_plan_id,
            &self.consumption_nonce,
        ] {
            validate_artifact(artifact)?;
        }
        let lifetime = self
            .expires_at_unix_seconds
            .checked_sub(self.planned_at_unix_seconds)
            .context("runtime IPC recipient import consumption plan expiry precedes creation")?;
        ensure!(
            self.planned_at_unix_seconds != 0
                && (1..=MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS)
                    .contains(&lifetime),
            "runtime IPC recipient import consumption plan lifetime is invalid"
        );
        ensure!(
            self.confirmation_phrase
                == recipient_import_consumption_confirmation_phrase(&self.consumption_plan_id),
            "runtime IPC recipient import consumption confirmation phrase is inconsistent"
        );
        Ok(())
    }

    fn validate_against_evidence(
        &self,
        evidence: &RuntimeIpcRecipientImportCompletedMutationEvidence,
    ) -> Result<()> {
        self.validate()?;
        evidence.validate()?;
        let binding = RuntimeIpcRecipientImportConsumptionPlanBinding {
            evidence: evidence.clone(),
            consumption_nonce: self.consumption_nonce.clone(),
            planned_at_unix_seconds: self.planned_at_unix_seconds,
            expires_at_unix_seconds: self.expires_at_unix_seconds,
        };
        ensure!(
            self.intent_id == evidence.intent_id
                && self.evidence_digest == evidence.evidence_digest()?
                && self.completed_mutation_record_id == evidence.completed_mutation_record_id
                && self.completed_mutation_receipt_digest
                    == evidence.completed_mutation_receipt_digest
                && self.final_coordinator_plan_id == evidence.final_coordinator_plan_id
                && self.final_coordinator_sequence == evidence.final_coordinator_sequence
                && self.consumption_plan_id == plan_id(&binding)?,
            "runtime IPC recipient import consumption plan names different completed evidence"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportConsumptionCommand {
    Plan {
        evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
        consumption_nonce: RuntimeIpcRecipientImportArtifactId,
        requested_lifetime_seconds: u64,
    },
    Commit {
        evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
        plan: Box<RuntimeIpcRecipientImportConsumptionPlanSummary>,
        confirmation_phrase: String,
        explicit_approval: RuntimeIpcRecipientImportConsumptionExplicitApproval,
    },
}

impl RuntimeIpcRecipientImportConsumptionCommand {
    pub fn intent_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        match self {
            Self::Plan { evidence, .. } | Self::Commit { evidence, .. } => &evidence.intent_id,
        }
    }

    pub fn evidence(&self) -> &RuntimeIpcRecipientImportCompletedMutationEvidence {
        match self {
            Self::Plan { evidence, .. } | Self::Commit { evidence, .. } => evidence,
        }
    }

    pub fn memberships(&self) -> &[RuntimeIpcRecipientImportMembershipReference] {
        match self {
            Self::Plan { memberships, .. } | Self::Commit { memberships, .. } => memberships,
        }
    }

    fn validate(&self) -> Result<()> {
        self.evidence().validate()?;
        ensure!(
            recipient_import_membership_set_digest(self.memberships())?
                == self.evidence().membership_set_digest,
            "runtime IPC recipient import consumption membership references do not match completed evidence"
        );
        match self {
            Self::Plan {
                evidence,
                memberships: _,
                consumption_nonce,
                requested_lifetime_seconds,
            } => {
                validate_artifact(consumption_nonce)?;
                ensure!(
                    consumption_nonce != &evidence.mutation_nonce
                        && consumption_nonce != &evidence.completed_mutation_record_id
                        && consumption_nonce != &evidence.completed_mutation_receipt_digest,
                    "runtime IPC recipient import consumption nonce reuses mutation evidence"
                );
                ensure!(
                    (1..=MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS)
                        .contains(requested_lifetime_seconds),
                    "runtime IPC recipient import consumption requested lifetime is invalid"
                );
            }
            Self::Commit {
                evidence,
                memberships: _,
                plan,
                confirmation_phrase,
                explicit_approval,
            } => {
                plan.validate_against_evidence(evidence)?;
                ensure!(
                    *explicit_approval
                        == RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                    "runtime IPC recipient import consumption commit must be explicit"
                );
                ensure!(
                    confirmation_phrase == &plan.confirmation_phrase,
                    "runtime IPC recipient import consumption phrase does not match its plan"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportConsumptionRequestWire {
    version: u8,
    request_id: RuntimeIpcRequestId,
    runtime_descriptor_binding: [u8; 32],
    command: RuntimeIpcRecipientImportConsumptionCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIpcRecipientImportConsumptionRequest {
    wire: RuntimeIpcRecipientImportConsumptionRequestWire,
}

impl RuntimeIpcRecipientImportConsumptionRequest {
    pub fn new(
        descriptor: &RuntimeIpcDescriptor,
        request_id: RuntimeIpcRequestId,
        command: RuntimeIpcRecipientImportConsumptionCommand,
    ) -> Result<Self> {
        descriptor.verify()?;
        command.validate()?;
        let request = Self {
            wire: RuntimeIpcRecipientImportConsumptionRequestWire {
                version: RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION,
                request_id,
                runtime_descriptor_binding: descriptor_binding(descriptor)?,
                command,
            },
        };
        request.validate_for_descriptor(descriptor)?;
        Ok(request)
    }

    pub fn decode_and_validate(encoded: &[u8], descriptor: &RuntimeIpcDescriptor) -> Result<Self> {
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_REQUEST_BYTES,
            "runtime IPC recipient import consumption request length is invalid"
        );
        let request = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode runtime IPC recipient import consumption request")?,
        };
        request.validate_for_descriptor(descriptor)?;
        ensure!(
            request.encode()? == encoded,
            "runtime IPC recipient import consumption request is not canonical"
        );
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode runtime IPC recipient import consumption request")?;
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_REQUEST_BYTES,
            "runtime IPC recipient import consumption request length is invalid"
        );
        Ok(encoded)
    }

    pub fn request_id(&self) -> RuntimeIpcRequestId {
        self.wire.request_id
    }

    pub fn command(&self) -> &RuntimeIpcRecipientImportConsumptionCommand {
        &self.wire.command
    }

    pub fn request_digest(&self) -> Result<[u8; 32]> {
        Ok(domain_digest(REQUEST_DIGEST_DOMAIN, &self.encode()?))
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.wire.version == RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION,
            "unsupported runtime IPC recipient import consumption request version"
        );
        ensure!(
            self.wire.runtime_descriptor_binding != [0_u8; 32]
                && self
                    .wire
                    .request_id
                    .as_bytes()
                    .iter()
                    .any(|byte| *byte != 0),
            "runtime IPC recipient import consumption request binding is invalid"
        );
        self.wire.command.validate()
    }

    fn validate_for_descriptor(&self, descriptor: &RuntimeIpcDescriptor) -> Result<()> {
        self.validate()?;
        descriptor.verify()?;
        ensure!(
            self.wire.runtime_descriptor_binding == descriptor_binding(descriptor)?,
            "runtime IPC recipient import consumption request names another runtime descriptor"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportConsumptionHazard {
    NoActiveIntent,
    IntentAlreadyConsumed,
    IntentIncomplete,
    CompletedMutationMissing,
    CompletedMutationChanged,
    CoordinatorChanged,
    PlanExpired,
    ConsumptionReplay,
    CommitRejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportConsumptionReceiptSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub consumption_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub evidence_digest: RuntimeIpcRecipientImportArtifactId,
    pub consumption_nonce: RuntimeIpcRecipientImportArtifactId,
    pub consumption_id: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_sequence: u64,
    pub consumed_at_unix_seconds: u64,
    pub active_intent_cleared: bool,
}

impl RuntimeIpcRecipientImportConsumptionReceiptSummary {
    fn validate(&self) -> Result<()> {
        for artifact in [
            &self.intent_id,
            &self.consumption_plan_id,
            &self.evidence_digest,
            &self.consumption_nonce,
            &self.consumption_id,
            &self.final_coordinator_plan_id,
        ] {
            validate_artifact(artifact)?;
        }
        ensure!(
            self.consumption_id != self.consumption_nonce
                && self.consumed_at_unix_seconds != 0
                && self.active_intent_cleared,
            "runtime IPC recipient import consumption receipt is invalid"
        );
        Ok(())
    }

    fn validate_against_plan(
        &self,
        plan: &RuntimeIpcRecipientImportConsumptionPlanSummary,
    ) -> Result<()> {
        self.validate()?;
        plan.validate()?;
        ensure!(
            self.intent_id == plan.intent_id
                && self.consumption_plan_id == plan.consumption_plan_id
                && self.evidence_digest == plan.evidence_digest
                && self.consumption_nonce == plan.consumption_nonce
                && self.final_coordinator_plan_id == plan.final_coordinator_plan_id
                && self.final_coordinator_sequence == plan.final_coordinator_sequence
                && (plan.planned_at_unix_seconds..=plan.expires_at_unix_seconds)
                    .contains(&self.consumed_at_unix_seconds),
            "runtime IPC recipient import consumption receipt names another plan"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportConsumptionOutcome {
    Planned(RuntimeIpcRecipientImportConsumptionPlanSummary),
    Consumed(RuntimeIpcRecipientImportConsumptionReceiptSummary),
    AlreadyConsumed(RuntimeIpcRecipientImportConsumptionReceiptSummary),
    Rejected {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        hazard: RuntimeIpcRecipientImportConsumptionHazard,
    },
}

impl RuntimeIpcRecipientImportConsumptionOutcome {
    pub fn intent_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        match self {
            Self::Planned(summary) => &summary.intent_id,
            Self::Consumed(summary) | Self::AlreadyConsumed(summary) => &summary.intent_id,
            Self::Rejected { intent_id, .. } => intent_id,
        }
    }

    pub fn consumed_intent(&self) -> bool {
        matches!(self, Self::Consumed(_) | Self::AlreadyConsumed(_))
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Planned(summary) => summary.validate(),
            Self::Consumed(summary) | Self::AlreadyConsumed(summary) => summary.validate(),
            Self::Rejected { intent_id, .. } => validate_artifact(intent_id),
        }
    }

    fn validate_for_command(
        &self,
        command: &RuntimeIpcRecipientImportConsumptionCommand,
    ) -> Result<()> {
        self.validate()?;
        ensure!(
            self.intent_id() == command.intent_id(),
            "runtime IPC recipient import consumption outcome names another intent"
        );
        if matches!(self, Self::Rejected { .. }) {
            return Ok(());
        }
        match (command, self) {
            (
                RuntimeIpcRecipientImportConsumptionCommand::Plan {
                    evidence,
                    memberships: _,
                    consumption_nonce,
                    requested_lifetime_seconds,
                },
                Self::Planned(plan),
            ) => {
                plan.validate_against_evidence(evidence)?;
                let lifetime = plan
                    .expires_at_unix_seconds
                    .checked_sub(plan.planned_at_unix_seconds)
                    .context("runtime IPC recipient import consumption plan expiry underflow")?;
                ensure!(
                    &plan.consumption_nonce == consumption_nonce
                        && lifetime <= *requested_lifetime_seconds,
                    "runtime IPC recipient import consumption plan exceeds its request"
                );
            }
            (
                RuntimeIpcRecipientImportConsumptionCommand::Commit { plan, .. },
                Self::Consumed(receipt) | Self::AlreadyConsumed(receipt),
            ) => receipt.validate_against_plan(plan)?,
            _ => {
                anyhow::bail!(
                    "runtime IPC recipient import consumption outcome skips or rewinds its stage"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportConsumptionSignedResponseContent {
    version: u8,
    request_digest: [u8; 32],
    runtime_descriptor_binding: [u8; 32],
    current_device_binding: [u8; 32],
    explicit_commit_required: bool,
    automatic_consumption: bool,
    intent_consumed: bool,
    outcome: RuntimeIpcRecipientImportConsumptionOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportConsumptionSignedResponseWire {
    content: RuntimeIpcRecipientImportConsumptionSignedResponseContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRuntimeIpcRecipientImportConsumptionResponse {
    wire: RuntimeIpcRecipientImportConsumptionSignedResponseWire,
}

impl SignedRuntimeIpcRecipientImportConsumptionResponse {
    pub fn issue(
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        descriptor: &RuntimeIpcDescriptor,
        identity: &DeviceIdentity,
        outcome: RuntimeIpcRecipientImportConsumptionOutcome,
    ) -> Result<Self> {
        request.validate_for_descriptor(descriptor)?;
        outcome.validate_for_command(request.command())?;
        ensure!(
            descriptor.device_id() == identity.device_id(),
            "runtime IPC recipient import consumption response signer is not the descriptor Device"
        );
        let intent_consumed = outcome.consumed_intent();
        let content = RuntimeIpcRecipientImportConsumptionSignedResponseContent {
            version: RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION,
            request_digest: request.request_digest()?,
            runtime_descriptor_binding: descriptor_binding(descriptor)?,
            current_device_binding: device_binding(descriptor.device_id().as_bytes()),
            explicit_commit_required: true,
            automatic_consumption: false,
            intent_consumed,
            outcome,
        };
        let signature = identity.sign(&response_signing_bytes(&content)?).to_vec();
        let response = Self {
            wire: RuntimeIpcRecipientImportConsumptionSignedResponseWire { content, signature },
        };
        response.verify(request, descriptor)?;
        Ok(response)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<Self> {
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_RESPONSE_BYTES,
            "runtime IPC recipient import consumption response length is invalid"
        );
        let response = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode signed runtime IPC recipient import consumption response")?,
        };
        response.verify(request, descriptor)?;
        ensure!(
            response.encode()? == encoded,
            "runtime IPC recipient import consumption response is not canonical"
        );
        Ok(response)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode signed runtime IPC recipient import consumption response")?;
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_RESPONSE_BYTES,
            "runtime IPC recipient import consumption response length is invalid"
        );
        Ok(encoded)
    }

    pub fn outcome(&self) -> &RuntimeIpcRecipientImportConsumptionOutcome {
        &self.wire.content.outcome
    }

    pub fn request_digest(&self) -> [u8; 32] {
        self.wire.content.request_digest
    }

    pub fn explicit_commit_required(&self) -> bool {
        self.wire.content.explicit_commit_required
    }

    pub fn automatic_consumption(&self) -> bool {
        self.wire.content.automatic_consumption
    }

    pub fn intent_consumed(&self) -> bool {
        self.wire.content.intent_consumed
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.wire.content.version == RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_CONTRACT_VERSION
                && self.wire.content.explicit_commit_required
                && !self.wire.content.automatic_consumption
                && self.wire.content.intent_consumed == self.wire.content.outcome.consumed_intent(),
            "runtime IPC recipient import consumption response grants implicit authority"
        );
        self.wire.content.outcome.validate()
    }

    fn verify(
        &self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<()> {
        request.validate_for_descriptor(descriptor)?;
        self.validate()?;
        self.wire
            .content
            .outcome
            .validate_for_command(request.command())?;
        ensure!(
            self.wire.content.request_digest == request.request_digest()?
                && self.wire.content.runtime_descriptor_binding == descriptor_binding(descriptor)?
                && self.wire.content.current_device_binding
                    == device_binding(descriptor.device_id().as_bytes()),
            "runtime IPC recipient import consumption response binding changed"
        );
        descriptor
            .device_id()
            .verify(
                &response_signing_bytes(&self.wire.content)?,
                &self.wire.signature,
            )
            .context("verify runtime IPC recipient import consumption response signature")?;
        Ok(())
    }
}

pub fn recipient_import_consumption_confirmation_phrase(
    plan_id: &RuntimeIpcRecipientImportArtifactId,
) -> String {
    let encoded = plan_id.as_str();
    format!(
        "CONSUME {} {} {} {}",
        &encoded[0..4],
        &encoded[4..8],
        &encoded[8..12],
        &encoded[12..16]
    )
}

fn validate_artifact(value: &RuntimeIpcRecipientImportArtifactId) -> Result<()> {
    value.to_bytes().map(|_| ())
}

fn plan_id(
    binding: &RuntimeIpcRecipientImportConsumptionPlanBinding,
) -> Result<RuntimeIpcRecipientImportArtifactId> {
    binding.evidence.validate()?;
    validate_artifact(&binding.consumption_nonce)?;
    let encoded = postcard::to_allocvec(binding)
        .context("encode runtime IPC recipient import consumption plan binding")?;
    RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(PLAN_ID_DOMAIN, &encoded))
}

fn descriptor_binding(descriptor: &RuntimeIpcDescriptor) -> Result<[u8; 32]> {
    descriptor.verify()?;
    let encoded = postcard::to_allocvec(descriptor)
        .context("encode runtime IPC recipient import consumption descriptor binding")?;
    Ok(domain_digest(DESCRIPTOR_BINDING_DOMAIN, &encoded))
}

fn device_binding(device_id: &[u8; 32]) -> [u8; 32] {
    domain_digest(DEVICE_BINDING_DOMAIN, device_id)
}

fn response_signing_bytes(
    content: &RuntimeIpcRecipientImportConsumptionSignedResponseContent,
) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content)
        .context("encode runtime IPC recipient import consumption response content")?;
    let mut signing = Vec::with_capacity(RESPONSE_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(RESPONSE_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn domain_digest(domain: &[u8], encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        net::{IpAddr, SocketAddr},
    };

    use kilogram_identity::{AccountId, DeviceIdentity};

    use super::*;
    use crate::{RuntimeIpcRecipientImportStepDisposition, RuntimeIpcRecipientImportStepReceipt};

    fn artifact(byte: u8) -> RuntimeIpcRecipientImportArtifactId {
        RuntimeIpcRecipientImportArtifactId::from_bytes([byte; 32])
            .unwrap_or_else(|error| unreachable!("nonzero test artifact must be valid: {error}"))
    }

    fn request_id(byte: u8) -> RuntimeIpcRequestId {
        format!("{byte:064x}")
            .parse()
            .unwrap_or_else(|error| unreachable!("test request ID must be valid: {error}"))
    }

    fn descriptor(identity: &DeviceIdentity, port: u16) -> Result<RuntimeIpcDescriptor> {
        RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port),
            [8_u8; 32],
            AccountId::from_bytes([7_u8; 32]),
            identity,
        )
    }

    fn memberships() -> Vec<RuntimeIpcRecipientImportMembershipReference> {
        vec![
            RuntimeIpcRecipientImportMembershipReference {
                source_ordinal: 2,
                authority_reference_id: artifact(20),
                membership_digest: artifact(21),
                authenticated_context_id: artifact(22),
            },
            RuntimeIpcRecipientImportMembershipReference {
                source_ordinal: 7,
                authority_reference_id: artifact(23),
                membership_digest: artifact(24),
                authenticated_context_id: artifact(25),
            },
        ]
    }

    fn execution_receipt() -> RuntimeIpcRecipientImportExecutionReceiptSummary {
        RuntimeIpcRecipientImportExecutionReceiptSummary {
            intent_id: artifact(1),
            execution_plan_id: artifact(2),
            confirmation_id: artifact(3),
            mutation_nonce: artifact(4),
            starting_completed_source_count: 0,
            final_completed_source_count: 2,
            total_source_count: 2,
            steps: vec![
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 1,
                    disposition: RuntimeIpcRecipientImportStepDisposition::Imported,
                    imported_event_count: 3,
                    coordinator_sequence: 10,
                },
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 2,
                    disposition: RuntimeIpcRecipientImportStepDisposition::AlreadyImported,
                    imported_event_count: 4,
                    coordinator_sequence: 11,
                },
            ],
            imported_now_count: 1,
            already_imported_count: 1,
            imported_event_count: 7,
            final_coordinator_plan_id: artifact(5),
            final_coordinator_sequence: 11,
            complete: true,
        }
    }

    fn evidence() -> Result<RuntimeIpcRecipientImportCompletedMutationEvidence> {
        RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt(
            artifact(6),
            recipient_import_membership_set_digest(&memberships())?,
            &execution_receipt(),
        )
    }

    fn plan() -> Result<RuntimeIpcRecipientImportConsumptionPlanSummary> {
        RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &evidence()?,
            artifact(7),
            1_000,
            120,
        )
    }

    fn consumption_receipt() -> Result<RuntimeIpcRecipientImportConsumptionReceiptSummary> {
        let plan = plan()?;
        Ok(RuntimeIpcRecipientImportConsumptionReceiptSummary {
            intent_id: plan.intent_id.clone(),
            consumption_plan_id: plan.consumption_plan_id.clone(),
            evidence_digest: plan.evidence_digest.clone(),
            consumption_nonce: plan.consumption_nonce.clone(),
            consumption_id: artifact(8),
            final_coordinator_plan_id: plan.final_coordinator_plan_id.clone(),
            final_coordinator_sequence: plan.final_coordinator_sequence,
            consumed_at_unix_seconds: 1_060,
            active_intent_cleared: true,
        })
    }

    #[test]
    fn plan_and_commit_requests_round_trip_canonically() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42101)?;
        let evidence = evidence()?;
        let plan_request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Plan {
                evidence: evidence.clone(),
                memberships: memberships(),
                consumption_nonce: artifact(7),
                requested_lifetime_seconds: 120,
            },
        )?;
        assert_eq!(
            RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
                &plan_request.encode()?,
                &descriptor,
            )?,
            plan_request
        );

        let plan = plan()?;
        let commit_request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(2),
            RuntimeIpcRecipientImportConsumptionCommand::Commit {
                evidence,
                memberships: memberships(),
                confirmation_phrase: plan.confirmation_phrase.clone(),
                explicit_approval:
                    RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                plan: Box::new(plan),
            },
        )?;
        assert_eq!(
            RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
                &commit_request.encode()?,
                &descriptor,
            )?,
            commit_request
        );
        Ok(())
    }

    #[test]
    fn evidence_requires_complete_exact_mutation_receipt() -> Result<(), Box<dyn Error>> {
        let mut incomplete = execution_receipt();
        incomplete.final_completed_source_count = 1;
        incomplete.steps.truncate(1);
        incomplete.already_imported_count = 0;
        incomplete.imported_event_count = 3;
        incomplete.complete = false;
        assert!(
            RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt(
                artifact(6),
                recipient_import_membership_set_digest(&memberships())?,
                &incomplete,
            )
            .is_err()
        );

        let mut overlapping = evidence()?;
        overlapping.completed_mutation_record_id =
            overlapping.completed_mutation_receipt_digest.clone();
        assert!(overlapping.validate().is_err());

        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42108)?;
        let mut substituted_memberships = memberships();
        substituted_memberships[0].membership_digest = artifact(99);
        assert!(
            RuntimeIpcRecipientImportConsumptionRequest::new(
                &descriptor,
                request_id(1),
                RuntimeIpcRecipientImportConsumptionCommand::Plan {
                    evidence: evidence()?,
                    memberships: substituted_memberships,
                    consumption_nonce: artifact(7),
                    requested_lifetime_seconds: 120,
                },
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn plan_exact_binds_evidence_nonce_and_short_expiry() -> Result<(), Box<dyn Error>> {
        let evidence = evidence()?;
        let plan = plan()?;
        assert_eq!(
            plan.confirmation_phrase,
            recipient_import_consumption_confirmation_phrase(&plan.consumption_plan_id)
        );
        assert!(!plan.expired(1_120));
        assert!(plan.expired(1_121));
        assert!(plan.validate_against_evidence(&evidence).is_ok());

        let mut changed = evidence;
        changed.final_coordinator_sequence += 1;
        assert!(plan.validate_against_evidence(&changed).is_err());
        assert!(
            RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
                &changed,
                artifact(7),
                1_000,
                MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS + 1,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn implicit_or_mismatched_commit_fails_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42102)?;
        let evidence = evidence()?;
        let plan = plan()?;
        for (approval, phrase) in [
            (
                RuntimeIpcRecipientImportConsumptionExplicitApproval::NotAuthorized,
                plan.confirmation_phrase.clone(),
            ),
            (
                RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                "CONSUME dead beef cafe babe".to_owned(),
            ),
        ] {
            assert!(
                RuntimeIpcRecipientImportConsumptionRequest::new(
                    &descriptor,
                    request_id(1),
                    RuntimeIpcRecipientImportConsumptionCommand::Commit {
                        evidence: evidence.clone(),
                        memberships: memberships(),
                        plan: Box::new(plan.clone()),
                        confirmation_phrase: phrase,
                        explicit_approval: approval,
                    },
                )
                .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn descriptor_substitution_fails_before_future_adapter() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let first = descriptor(&identity, 42103)?;
        let second = descriptor(&identity, 42104)?;
        let request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &first,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Plan {
                evidence: evidence()?,
                memberships: memberships(),
                consumption_nonce: artifact(7),
                requested_lifetime_seconds: 120,
            },
        )?;
        assert!(
            RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
                &request.encode()?,
                &second,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn signed_plan_response_exact_binds_request_descriptor_and_device() -> Result<(), Box<dyn Error>>
    {
        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42105)?;
        let request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Plan {
                evidence: evidence()?,
                memberships: memberships(),
                consumption_nonce: artifact(7),
                requested_lifetime_seconds: 120,
            },
        )?;
        let signed = SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
            &request,
            &descriptor,
            &identity,
            RuntimeIpcRecipientImportConsumptionOutcome::Planned(plan()?),
        )?;
        assert!(!signed.intent_consumed());
        assert!(signed.explicit_commit_required());
        assert!(!signed.automatic_consumption());
        assert_eq!(
            SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify(
                &signed.encode()?,
                &request,
                &descriptor,
            )?,
            signed
        );

        let other = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(2),
            request.command().clone(),
        )?;
        assert!(
            SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify(
                &signed.encode()?,
                &other,
                &descriptor,
            )
            .is_err()
        );
        let foreign = DeviceIdentity::generate()?;
        assert!(
            SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
                &request,
                &descriptor,
                &foreign,
                signed.outcome().clone(),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn commit_response_consumes_only_the_exact_explicit_plan() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42106)?;
        let evidence = evidence()?;
        let consume_plan = plan()?;
        let request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Commit {
                evidence,
                memberships: memberships(),
                confirmation_phrase: consume_plan.confirmation_phrase.clone(),
                explicit_approval:
                    RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                plan: Box::new(consume_plan),
            },
        )?;
        for outcome in [
            RuntimeIpcRecipientImportConsumptionOutcome::Consumed(consumption_receipt()?),
            RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed(consumption_receipt()?),
        ] {
            let signed = SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
                &request,
                &descriptor,
                &identity,
                outcome,
            )?;
            assert!(signed.intent_consumed());
            assert!(
                SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify(
                    &signed.encode()?,
                    &request,
                    &descriptor,
                )
                .is_ok()
            );
        }

        assert!(
            SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
                &request,
                &descriptor,
                &identity,
                RuntimeIpcRecipientImportConsumptionOutcome::Planned(plan()?),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn noncanonical_oversized_and_tampered_encodings_fail_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = descriptor(&identity, 42107)?;
        let request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Plan {
                evidence: evidence()?,
                memberships: memberships(),
                consumption_nonce: artifact(7),
                requested_lifetime_seconds: 120,
            },
        )?;
        let mut trailing = request.encode()?;
        trailing.push(0);
        assert!(
            RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
                &trailing,
                &descriptor,
            )
            .is_err()
        );
        assert!(
            RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate(
                &vec![0; MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_REQUEST_BYTES + 1],
                &descriptor,
            )
            .is_err()
        );

        let signed = SignedRuntimeIpcRecipientImportConsumptionResponse::issue(
            &request,
            &descriptor,
            &identity,
            RuntimeIpcRecipientImportConsumptionOutcome::Planned(plan()?),
        )?;
        let mut tampered = signed.encode()?;
        let last = tampered
            .last_mut()
            .ok_or("signed response unexpectedly encoded empty")?;
        *last ^= 1;
        assert!(
            SignedRuntimeIpcRecipientImportConsumptionResponse::decode_and_verify(
                &tampered,
                &request,
                &descriptor,
            )
            .is_err()
        );
        Ok(())
    }
}
