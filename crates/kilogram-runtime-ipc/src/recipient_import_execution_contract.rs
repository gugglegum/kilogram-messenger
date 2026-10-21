//! Dormant cross-process contract for explicit recipient import-round execution.
//!
//! The contract is intentionally not registered in the active runtime command
//! enum. It transports authenticated references to caller-authorized live
//! membership context, never membership snapshots, message bodies, plaintext,
//! filesystem paths, or reusable transport authority.

use std::{fmt, str::FromStr};

use anyhow::{Context, Result, ensure};
use kilogram_identity::DeviceIdentity;
use serde::{Deserialize, Serialize};

use super::{RuntimeIpcDescriptor, RuntimeIpcRequestId};

pub const RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION: u8 = 1;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_RESPONSE_BYTES: usize = 16 * 1024;
pub const MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES: usize = 8;
pub const DEFAULT_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_MAX_STEPS: u8 = 1;

const REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-execution-request:v1\0";
const DESCRIPTOR_BINDING_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-execution-descriptor:v1\0";
const DEVICE_BINDING_DOMAIN: &[u8] = b"kilogram:runtime-ipc-recipient-import-execution-device:v1\0";
const MEMBERSHIP_SET_DOMAIN: &[u8] = b"kilogram:runtime-ipc-recipient-import-membership-set:v1\0";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-execution-response:v1\0";
const EXECUTION_RECEIPT_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recipient-import-execution-receipt:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIpcRecipientImportArtifactId(String);

impl RuntimeIpcRecipientImportArtifactId {
    pub fn new(value: String) -> Result<Self> {
        ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "runtime IPC recipient import artifact ID must be 64 lowercase hexadecimal characters"
        );
        ensure!(
            value.bytes().any(|byte| byte != b'0'),
            "runtime IPC recipient import artifact ID must not be zero"
        );
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        ensure!(
            bytes.iter().any(|byte| *byte != 0),
            "runtime IPC recipient import artifact ID must not be zero"
        );
        let mut encoded = String::with_capacity(64);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}")
                .context("encode runtime IPC recipient import artifact ID")?;
        }
        Ok(Self(encoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_bytes(&self) -> Result<[u8; 32]> {
        self.validate()?;
        let mut bytes = [0_u8; 32];
        for (index, pair) in self.0.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let encoded = std::str::from_utf8(pair)
                .context("decode runtime IPC recipient import artifact ID")?;
            bytes[index] = u8::from_str_radix(encoded, 16)
                .context("decode runtime IPC recipient import artifact ID")?;
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<()> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

impl fmt::Display for RuntimeIpcRecipientImportArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RuntimeIpcRecipientImportArtifactId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value.to_owned())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportMembershipReference {
    pub source_ordinal: u16,
    pub authority_reference_id: RuntimeIpcRecipientImportArtifactId,
    pub membership_digest: RuntimeIpcRecipientImportArtifactId,
    pub authenticated_context_id: RuntimeIpcRecipientImportArtifactId,
}

impl RuntimeIpcRecipientImportMembershipReference {
    fn validate(&self) -> Result<()> {
        self.authority_reference_id.validate()?;
        self.membership_digest.validate()?;
        self.authenticated_context_id.validate()?;
        ensure!(
            self.authority_reference_id != self.membership_digest
                && self.authority_reference_id != self.authenticated_context_id
                && self.membership_digest != self.authenticated_context_id,
            "runtime IPC recipient import membership reference domains overlap"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportExplicitApproval {
    NotAuthorized,
    UserConfirmed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportExecutionCommand {
    Plan {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_sequence: u64,
        max_steps: u8,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
    },
    Status {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        execution_plan_id: Option<RuntimeIpcRecipientImportArtifactId>,
    },
    Confirm {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        execution_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_sequence: u64,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
        confirmation_nonce: RuntimeIpcRecipientImportArtifactId,
        confirmation_phrase: String,
        explicit_approval: RuntimeIpcRecipientImportExplicitApproval,
    },
    ApplyNext {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        execution_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_sequence: u64,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
        confirmation_id: RuntimeIpcRecipientImportArtifactId,
        mutation_nonce: RuntimeIpcRecipientImportArtifactId,
    },
    ApplyBounded {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        execution_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
        coordinator_sequence: u64,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
        confirmation_id: RuntimeIpcRecipientImportArtifactId,
        mutation_nonce: RuntimeIpcRecipientImportArtifactId,
        max_steps: u8,
    },
}

impl RuntimeIpcRecipientImportExecutionCommand {
    pub fn intent_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        match self {
            Self::Plan { intent_id, .. }
            | Self::Status { intent_id, .. }
            | Self::Confirm { intent_id, .. }
            | Self::ApplyNext { intent_id, .. }
            | Self::ApplyBounded { intent_id, .. } => intent_id,
        }
    }

    pub fn memberships(&self) -> Option<&[RuntimeIpcRecipientImportMembershipReference]> {
        match self {
            Self::Plan { memberships, .. }
            | Self::Confirm { memberships, .. }
            | Self::ApplyNext { memberships, .. }
            | Self::ApplyBounded { memberships, .. } => Some(memberships),
            Self::Status { .. } => None,
        }
    }

    pub fn membership_set_digest(&self) -> Result<Option<RuntimeIpcRecipientImportArtifactId>> {
        self.memberships().map(membership_set_digest).transpose()
    }

    fn validate(&self) -> Result<()> {
        self.intent_id().validate()?;
        match self {
            Self::Plan {
                coordinator_plan_id,
                max_steps,
                memberships,
                ..
            } => {
                coordinator_plan_id.validate()?;
                validate_step_budget(*max_steps)?;
                validate_memberships(memberships)?;
            }
            Self::Status {
                execution_plan_id, ..
            } => {
                if let Some(plan_id) = execution_plan_id {
                    plan_id.validate()?;
                }
            }
            Self::Confirm {
                execution_plan_id,
                coordinator_plan_id,
                memberships,
                confirmation_nonce,
                confirmation_phrase,
                explicit_approval,
                ..
            } => {
                execution_plan_id.validate()?;
                coordinator_plan_id.validate()?;
                validate_memberships(memberships)?;
                confirmation_nonce.validate()?;
                ensure!(
                    *explicit_approval == RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
                    "runtime IPC recipient import confirmation must be explicit"
                );
                validate_confirmation_phrase(confirmation_phrase)?;
                ensure!(
                    confirmation_phrase == &confirmation_phrase_for(execution_plan_id),
                    "runtime IPC recipient import confirmation phrase does not match the execution plan ID"
                );
            }
            Self::ApplyNext {
                execution_plan_id,
                coordinator_plan_id,
                memberships,
                confirmation_id,
                mutation_nonce,
                ..
            }
            | Self::ApplyBounded {
                execution_plan_id,
                coordinator_plan_id,
                memberships,
                confirmation_id,
                mutation_nonce,
                ..
            } => {
                execution_plan_id.validate()?;
                coordinator_plan_id.validate()?;
                validate_memberships(memberships)?;
                confirmation_id.validate()?;
                mutation_nonce.validate()?;
                ensure!(
                    confirmation_id != mutation_nonce,
                    "runtime IPC recipient import mutation nonce reuses the confirmation ID"
                );
                if let Self::ApplyBounded { max_steps, .. } = self {
                    validate_step_budget(*max_steps)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportExecutionRequestWire {
    version: u8,
    request_id: RuntimeIpcRequestId,
    runtime_descriptor_binding: [u8; 32],
    command: RuntimeIpcRecipientImportExecutionCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIpcRecipientImportExecutionRequest {
    wire: RuntimeIpcRecipientImportExecutionRequestWire,
}

impl RuntimeIpcRecipientImportExecutionRequest {
    pub fn new(
        descriptor: &RuntimeIpcDescriptor,
        request_id: RuntimeIpcRequestId,
        command: RuntimeIpcRecipientImportExecutionCommand,
    ) -> Result<Self> {
        descriptor.verify()?;
        command.validate()?;
        let request = Self {
            wire: RuntimeIpcRecipientImportExecutionRequestWire {
                version: RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION,
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
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_REQUEST_BYTES,
            "runtime IPC recipient import execution request length is invalid"
        );
        let request = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode runtime IPC recipient import execution request")?,
        };
        request.validate_for_descriptor(descriptor)?;
        ensure!(
            request.encode()? == encoded,
            "runtime IPC recipient import execution request is not canonical"
        );
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode runtime IPC recipient import execution request")?;
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_REQUEST_BYTES,
            "runtime IPC recipient import execution request length is invalid"
        );
        Ok(encoded)
    }

    pub fn request_id(&self) -> RuntimeIpcRequestId {
        self.wire.request_id
    }

    pub fn command(&self) -> &RuntimeIpcRecipientImportExecutionCommand {
        &self.wire.command
    }

    pub fn request_digest(&self) -> Result<[u8; 32]> {
        Ok(domain_digest(REQUEST_DIGEST_DOMAIN, &self.encode()?))
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.wire.version == RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION,
            "unsupported runtime IPC recipient import execution request version"
        );
        ensure!(
            self.wire.runtime_descriptor_binding != [0_u8; 32],
            "runtime IPC recipient import execution request has no descriptor binding"
        );
        ensure!(
            self.wire
                .request_id
                .as_bytes()
                .iter()
                .any(|byte| *byte != 0),
            "runtime IPC recipient import execution request ID must not be zero"
        );
        self.wire.command.validate()
    }

    fn validate_for_descriptor(&self, descriptor: &RuntimeIpcDescriptor) -> Result<()> {
        self.validate()?;
        descriptor.verify()?;
        ensure!(
            self.wire.runtime_descriptor_binding == descriptor_binding(descriptor)?,
            "runtime IPC recipient import execution request names another runtime descriptor"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportIntentState {
    ActiveUnplanned,
    AwaitingConfirmation,
    ReadyToExecute,
    InProgress,
    CompleteAwaitingSeparateConsume,
    Consumed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportStatusSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub execution_plan_id: Option<RuntimeIpcRecipientImportArtifactId>,
    pub coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub coordinator_sequence: u64,
    pub completed_source_count: u8,
    pub total_source_count: u8,
    pub state: RuntimeIpcRecipientImportIntentState,
}

impl RuntimeIpcRecipientImportStatusSummary {
    fn validate(&self) -> Result<()> {
        self.intent_id.validate()?;
        self.coordinator_plan_id.validate()?;
        if let Some(plan_id) = &self.execution_plan_id {
            plan_id.validate()?;
        }
        validate_counts(self.completed_source_count, self.total_source_count)?;
        match self.state {
            RuntimeIpcRecipientImportIntentState::ActiveUnplanned => {
                ensure!(
                    self.execution_plan_id.is_none(),
                    "unplanned recipient import status unexpectedly contains an execution plan"
                );
            }
            RuntimeIpcRecipientImportIntentState::AwaitingConfirmation
            | RuntimeIpcRecipientImportIntentState::ReadyToExecute
            | RuntimeIpcRecipientImportIntentState::InProgress
            | RuntimeIpcRecipientImportIntentState::CompleteAwaitingSeparateConsume => {
                ensure!(
                    self.execution_plan_id.is_some(),
                    "planned recipient import status is missing its execution plan"
                );
            }
            RuntimeIpcRecipientImportIntentState::Consumed => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportExecutionPlanSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub execution_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub coordinator_sequence: u64,
    pub membership_set_digest: RuntimeIpcRecipientImportArtifactId,
    pub completed_source_count: u8,
    pub total_source_count: u8,
    pub max_steps: u8,
    pub planned_step_count: u8,
    pub next_source_ordinal: Option<u16>,
    pub coordinator_refresh_required: bool,
    pub confirmation_phrase: String,
}

impl RuntimeIpcRecipientImportExecutionPlanSummary {
    fn validate(&self) -> Result<()> {
        self.intent_id.validate()?;
        self.execution_plan_id.validate()?;
        self.coordinator_plan_id.validate()?;
        self.membership_set_digest.validate()?;
        validate_counts(self.completed_source_count, self.total_source_count)?;
        validate_step_budget(self.max_steps)?;
        let remaining = self.total_source_count - self.completed_source_count;
        ensure!(
            self.planned_step_count <= self.max_steps
                && self.planned_step_count <= remaining
                && ((self.planned_step_count == 0 && self.next_source_ordinal.is_none())
                    || (self.planned_step_count != 0 && self.next_source_ordinal.is_some())),
            "runtime IPC recipient import execution plan bounds are inconsistent"
        );
        validate_confirmation_phrase(&self.confirmation_phrase)?;
        ensure!(
            self.confirmation_phrase == confirmation_phrase_for(&self.execution_plan_id),
            "runtime IPC recipient import execution plan confirmation phrase is inconsistent"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportExecutionConfirmationSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub execution_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub coordinator_sequence: u64,
    pub membership_set_digest: RuntimeIpcRecipientImportArtifactId,
    pub confirmation_id: RuntimeIpcRecipientImportArtifactId,
    pub confirmation_nonce: RuntimeIpcRecipientImportArtifactId,
    pub expires_at_unix_seconds: u64,
}

impl RuntimeIpcRecipientImportExecutionConfirmationSummary {
    fn validate(&self) -> Result<()> {
        self.intent_id.validate()?;
        self.execution_plan_id.validate()?;
        self.coordinator_plan_id.validate()?;
        self.membership_set_digest.validate()?;
        self.confirmation_id.validate()?;
        self.confirmation_nonce.validate()?;
        ensure!(
            self.confirmation_id != self.confirmation_nonce && self.expires_at_unix_seconds != 0,
            "runtime IPC recipient import execution confirmation is invalid"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportStepDisposition {
    Imported,
    AlreadyImported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportStepReceipt {
    pub source_ordinal: u16,
    pub disposition: RuntimeIpcRecipientImportStepDisposition,
    pub imported_event_count: u64,
    pub coordinator_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcRecipientImportExecutionReceiptSummary {
    pub intent_id: RuntimeIpcRecipientImportArtifactId,
    pub execution_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub confirmation_id: RuntimeIpcRecipientImportArtifactId,
    pub mutation_nonce: RuntimeIpcRecipientImportArtifactId,
    pub starting_completed_source_count: u8,
    pub final_completed_source_count: u8,
    pub total_source_count: u8,
    pub steps: Vec<RuntimeIpcRecipientImportStepReceipt>,
    pub imported_now_count: u8,
    pub already_imported_count: u8,
    pub imported_event_count: u64,
    pub final_coordinator_plan_id: RuntimeIpcRecipientImportArtifactId,
    pub final_coordinator_sequence: u64,
    pub complete: bool,
}

impl RuntimeIpcRecipientImportExecutionReceiptSummary {
    pub(crate) fn validate(&self) -> Result<()> {
        self.intent_id.validate()?;
        self.execution_plan_id.validate()?;
        self.confirmation_id.validate()?;
        self.mutation_nonce.validate()?;
        self.final_coordinator_plan_id.validate()?;
        ensure!(
            self.confirmation_id != self.mutation_nonce,
            "runtime IPC recipient import execution receipt reuses its confirmation ID"
        );
        validate_counts(
            self.starting_completed_source_count,
            self.total_source_count,
        )?;
        validate_counts(self.final_completed_source_count, self.total_source_count)?;
        ensure!(
            self.starting_completed_source_count <= self.final_completed_source_count,
            "runtime IPC recipient import execution receipt rewinds progress"
        );
        let step_count = u8::try_from(self.steps.len())
            .context("runtime IPC recipient import execution receipt has too many steps")?;
        ensure!(
            usize::from(step_count) <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES
                && self.final_completed_source_count - self.starting_completed_source_count
                    == step_count
                && self
                    .imported_now_count
                    .checked_add(self.already_imported_count)
                    == Some(step_count)
                && self.complete == (self.final_completed_source_count == self.total_source_count),
            "runtime IPC recipient import execution receipt counts are inconsistent"
        );
        let event_count = self.steps.iter().try_fold(0_u64, |total, step| {
            total.checked_add(step.imported_event_count)
        });
        ensure!(
            event_count == Some(self.imported_event_count),
            "runtime IPC recipient import execution receipt event count is inconsistent"
        );
        Ok(())
    }

    pub fn receipt_digest(&self) -> Result<RuntimeIpcRecipientImportArtifactId> {
        self.validate()?;
        let encoded = postcard::to_allocvec(self)
            .context("encode runtime IPC recipient import execution receipt digest")?;
        RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(
            EXECUTION_RECEIPT_DIGEST_DOMAIN,
            &encoded,
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportExecutionHazard {
    NoActiveIntent,
    IntentConsumed,
    PlanChanged,
    CoordinatorChanged,
    MembershipReferenceChanged,
    MembershipReloadFailed,
    ConfirmationRequired,
    ConfirmationExpired,
    MutationReplay,
    ImportRejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcRecipientImportExecutionOutcome {
    Status(RuntimeIpcRecipientImportStatusSummary),
    Planned(RuntimeIpcRecipientImportExecutionPlanSummary),
    Confirmed {
        plan: RuntimeIpcRecipientImportExecutionPlanSummary,
        confirmation: RuntimeIpcRecipientImportExecutionConfirmationSummary,
    },
    Executed(RuntimeIpcRecipientImportExecutionReceiptSummary),
    Rejected {
        intent_id: RuntimeIpcRecipientImportArtifactId,
        hazard: RuntimeIpcRecipientImportExecutionHazard,
    },
}

impl RuntimeIpcRecipientImportExecutionOutcome {
    pub fn intent_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        match self {
            Self::Status(summary) => &summary.intent_id,
            Self::Planned(summary) => &summary.intent_id,
            Self::Confirmed { plan, .. } => &plan.intent_id,
            Self::Executed(summary) => &summary.intent_id,
            Self::Rejected { intent_id, .. } => intent_id,
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Status(summary) => summary.validate(),
            Self::Planned(summary) => summary.validate(),
            Self::Confirmed { plan, confirmation } => {
                plan.validate()?;
                confirmation.validate()?;
                ensure!(
                    plan.intent_id == confirmation.intent_id
                        && plan.execution_plan_id == confirmation.execution_plan_id
                        && plan.coordinator_plan_id == confirmation.coordinator_plan_id
                        && plan.coordinator_sequence == confirmation.coordinator_sequence
                        && plan.membership_set_digest == confirmation.membership_set_digest,
                    "runtime IPC recipient import confirmation names another execution plan"
                );
                Ok(())
            }
            Self::Executed(summary) => summary.validate(),
            Self::Rejected { intent_id, .. } => intent_id.validate(),
        }
    }

    fn validate_for_command(
        &self,
        command: &RuntimeIpcRecipientImportExecutionCommand,
    ) -> Result<()> {
        self.validate()?;
        ensure!(
            self.intent_id() == command.intent_id(),
            "runtime IPC recipient import execution response names another intent"
        );
        if matches!(self, Self::Rejected { .. }) {
            return Ok(());
        }
        match (command, self) {
            (
                RuntimeIpcRecipientImportExecutionCommand::Status {
                    execution_plan_id, ..
                },
                Self::Status(summary),
            ) => {
                ensure!(
                    execution_plan_id
                        .as_ref()
                        .is_none_or(|expected| summary.execution_plan_id.as_ref() == Some(expected)),
                    "runtime IPC recipient import status names another execution plan"
                );
            }
            (
                RuntimeIpcRecipientImportExecutionCommand::Plan {
                    coordinator_plan_id,
                    coordinator_sequence,
                    max_steps,
                    memberships,
                    ..
                },
                Self::Planned(plan),
            ) => {
                ensure!(
                    &plan.coordinator_plan_id == coordinator_plan_id
                        && plan.coordinator_sequence == *coordinator_sequence
                        && plan.max_steps == *max_steps
                        && plan.membership_set_digest == membership_set_digest(memberships)?,
                    "runtime IPC recipient import plan does not match its request"
                );
            }
            (
                RuntimeIpcRecipientImportExecutionCommand::Confirm {
                    execution_plan_id,
                    coordinator_plan_id,
                    coordinator_sequence,
                    memberships,
                    confirmation_nonce,
                    ..
                },
                Self::Confirmed { plan, confirmation },
            ) => {
                let memberships_digest = membership_set_digest(memberships)?;
                ensure!(
                    &plan.execution_plan_id == execution_plan_id
                        && &plan.coordinator_plan_id == coordinator_plan_id
                        && plan.coordinator_sequence == *coordinator_sequence
                        && plan.membership_set_digest == memberships_digest
                        && &confirmation.confirmation_nonce == confirmation_nonce,
                    "runtime IPC recipient import confirmation does not match its request"
                );
            }
            (
                RuntimeIpcRecipientImportExecutionCommand::ApplyNext {
                    execution_plan_id,
                    confirmation_id,
                    mutation_nonce,
                    ..
                },
                Self::Executed(receipt),
            ) => validate_execution_binding(
                receipt,
                execution_plan_id,
                confirmation_id,
                mutation_nonce,
                1,
            )?,
            (
                RuntimeIpcRecipientImportExecutionCommand::ApplyBounded {
                    execution_plan_id,
                    confirmation_id,
                    mutation_nonce,
                    max_steps,
                    ..
                },
                Self::Executed(receipt),
            ) => validate_execution_binding(
                receipt,
                execution_plan_id,
                confirmation_id,
                mutation_nonce,
                *max_steps,
            )?,
            _ => {
                anyhow::bail!(
                    "runtime IPC recipient import execution response skips or rewinds the requested stage"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportExecutionSignedResponseContent {
    version: u8,
    request_digest: [u8; 32],
    runtime_descriptor_binding: [u8; 32],
    current_device_binding: [u8; 32],
    membership_reload_required: bool,
    explicit_execution_required: bool,
    intent_consumed: bool,
    outcome: RuntimeIpcRecipientImportExecutionOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcRecipientImportExecutionSignedResponseWire {
    content: RuntimeIpcRecipientImportExecutionSignedResponseContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRuntimeIpcRecipientImportExecutionResponse {
    wire: RuntimeIpcRecipientImportExecutionSignedResponseWire,
}

impl SignedRuntimeIpcRecipientImportExecutionResponse {
    pub fn issue(
        request: &RuntimeIpcRecipientImportExecutionRequest,
        descriptor: &RuntimeIpcDescriptor,
        identity: &DeviceIdentity,
        outcome: RuntimeIpcRecipientImportExecutionOutcome,
    ) -> Result<Self> {
        request.validate_for_descriptor(descriptor)?;
        outcome.validate_for_command(request.command())?;
        ensure!(
            descriptor.content.device_id == identity.device_id(),
            "runtime IPC recipient import execution response signer is not the descriptor Device"
        );
        let content = RuntimeIpcRecipientImportExecutionSignedResponseContent {
            version: RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION,
            request_digest: request.request_digest()?,
            runtime_descriptor_binding: descriptor_binding(descriptor)?,
            current_device_binding: device_binding(descriptor.content.device_id.as_bytes()),
            membership_reload_required: true,
            explicit_execution_required: true,
            intent_consumed: false,
            outcome,
        };
        let signature = identity.sign(&response_signing_bytes(&content)?).to_vec();
        let signed = Self {
            wire: RuntimeIpcRecipientImportExecutionSignedResponseWire { content, signature },
        };
        signed.verify(request, descriptor)?;
        Ok(signed)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        request: &RuntimeIpcRecipientImportExecutionRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<Self> {
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_RESPONSE_BYTES,
            "runtime IPC recipient import execution response length is invalid"
        );
        let signed = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode signed runtime IPC recipient import execution response")?,
        };
        signed.verify(request, descriptor)?;
        ensure!(
            signed.encode()? == encoded,
            "runtime IPC recipient import execution response is not canonical"
        );
        Ok(signed)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode signed runtime IPC recipient import execution response")?;
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_RESPONSE_BYTES,
            "runtime IPC recipient import execution response length is invalid"
        );
        Ok(encoded)
    }

    pub fn outcome(&self) -> &RuntimeIpcRecipientImportExecutionOutcome {
        &self.wire.content.outcome
    }

    pub fn request_digest(&self) -> [u8; 32] {
        self.wire.content.request_digest
    }

    pub fn membership_reload_required(&self) -> bool {
        self.wire.content.membership_reload_required
    }

    pub fn explicit_execution_required(&self) -> bool {
        self.wire.content.explicit_execution_required
    }

    pub fn intent_consumed(&self) -> bool {
        self.wire.content.intent_consumed
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.wire.content.version == RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_CONTRACT_VERSION,
            "unsupported runtime IPC recipient import execution response version"
        );
        ensure!(
            self.wire.content.membership_reload_required
                && self.wire.content.explicit_execution_required
                && !self.wire.content.intent_consumed,
            "runtime IPC recipient import execution response grants implicit authority"
        );
        self.wire.content.outcome.validate()
    }

    fn verify(
        &self,
        request: &RuntimeIpcRecipientImportExecutionRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<()> {
        request.validate_for_descriptor(descriptor)?;
        self.validate()?;
        self.wire
            .content
            .outcome
            .validate_for_command(request.command())?;
        ensure!(
            self.wire.content.request_digest == request.request_digest()?,
            "runtime IPC recipient import execution response names another request"
        );
        ensure!(
            self.wire.content.runtime_descriptor_binding == descriptor_binding(descriptor)?,
            "runtime IPC recipient import execution response names another runtime descriptor"
        );
        ensure!(
            self.wire.content.current_device_binding
                == device_binding(descriptor.content.device_id.as_bytes()),
            "runtime IPC recipient import execution response names another current Device"
        );
        descriptor
            .content
            .device_id
            .verify(
                &response_signing_bytes(&self.wire.content)?,
                &self.wire.signature,
            )
            .context("verify runtime IPC recipient import execution response signature")?;
        Ok(())
    }
}

fn validate_step_budget(max_steps: u8) -> Result<()> {
    ensure!(
        max_steps != 0
            && usize::from(max_steps) <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES,
        "runtime IPC recipient import execution step budget is invalid"
    );
    Ok(())
}

fn validate_counts(completed: u8, total: u8) -> Result<()> {
    ensure!(
        total != 0
            && usize::from(total) <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES
            && completed <= total,
        "runtime IPC recipient import execution progress counts are invalid"
    );
    Ok(())
}

fn validate_memberships(
    memberships: &[RuntimeIpcRecipientImportMembershipReference],
) -> Result<()> {
    ensure!(
        !memberships.is_empty()
            && memberships.len() <= MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES,
        "runtime IPC recipient import membership reference count is invalid"
    );
    let mut previous = None;
    for membership in memberships {
        membership.validate()?;
        ensure!(
            previous.is_none_or(|ordinal| membership.source_ordinal > ordinal),
            "runtime IPC recipient import membership references are reordered or duplicated"
        );
        previous = Some(membership.source_ordinal);
    }
    Ok(())
}

fn membership_set_digest(
    memberships: &[RuntimeIpcRecipientImportMembershipReference],
) -> Result<RuntimeIpcRecipientImportArtifactId> {
    validate_memberships(memberships)?;
    let encoded = postcard::to_allocvec(memberships)
        .context("encode runtime IPC recipient import membership references")?;
    RuntimeIpcRecipientImportArtifactId::from_bytes(domain_digest(MEMBERSHIP_SET_DOMAIN, &encoded))
}

/// Computes the canonical digest for an ordered set of authenticated
/// membership references. Later dormant ceremony contracts use this helper so
/// that every stage binds the same caller-owned live-membership context rather
/// than relying on an out-of-band agreement.
pub fn recipient_import_membership_set_digest(
    memberships: &[RuntimeIpcRecipientImportMembershipReference],
) -> Result<RuntimeIpcRecipientImportArtifactId> {
    membership_set_digest(memberships)
}

fn validate_confirmation_phrase(value: &str) -> Result<()> {
    let chunks: Vec<&str> = value.split(' ').collect();
    ensure!(
        chunks.len() == 5
            && chunks[0] == "IMPORT"
            && chunks[1..].iter().all(|chunk| {
                chunk.len() == 4
                    && chunk
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }),
        "runtime IPC recipient import confirmation phrase is invalid"
    );
    Ok(())
}

pub fn recipient_import_confirmation_phrase(
    plan_id: &RuntimeIpcRecipientImportArtifactId,
) -> String {
    confirmation_phrase_for(plan_id)
}

fn confirmation_phrase_for(plan_id: &RuntimeIpcRecipientImportArtifactId) -> String {
    let encoded = plan_id.as_str();
    format!(
        "IMPORT {} {} {} {}",
        &encoded[0..4],
        &encoded[4..8],
        &encoded[8..12],
        &encoded[12..16]
    )
}

fn validate_execution_binding(
    receipt: &RuntimeIpcRecipientImportExecutionReceiptSummary,
    execution_plan_id: &RuntimeIpcRecipientImportArtifactId,
    confirmation_id: &RuntimeIpcRecipientImportArtifactId,
    mutation_nonce: &RuntimeIpcRecipientImportArtifactId,
    max_steps: u8,
) -> Result<()> {
    ensure!(
        &receipt.execution_plan_id == execution_plan_id
            && &receipt.confirmation_id == confirmation_id
            && &receipt.mutation_nonce == mutation_nonce
            && receipt.steps.len() <= usize::from(max_steps),
        "runtime IPC recipient import execution receipt does not match its mutation request"
    );
    Ok(())
}

fn descriptor_binding(descriptor: &RuntimeIpcDescriptor) -> Result<[u8; 32]> {
    descriptor.verify()?;
    let encoded = postcard::to_allocvec(descriptor)
        .context("encode runtime IPC recipient import execution descriptor binding")?;
    Ok(domain_digest(DESCRIPTOR_BINDING_DOMAIN, &encoded))
}

fn device_binding(device_id: &[u8; 32]) -> [u8; 32] {
    domain_digest(DEVICE_BINDING_DOMAIN, device_id)
}

fn response_signing_bytes(
    content: &RuntimeIpcRecipientImportExecutionSignedResponseContent,
) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content)
        .context("encode runtime IPC recipient import execution response content")?;
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

    fn artifact(byte: u8) -> RuntimeIpcRecipientImportArtifactId {
        RuntimeIpcRecipientImportArtifactId::from_bytes([byte; 32])
            .unwrap_or_else(|error| unreachable!("nonzero test artifact must be valid: {error}"))
    }

    fn request_id(byte: u8) -> RuntimeIpcRequestId {
        format!("{byte:064x}")
            .parse()
            .unwrap_or_else(|error| unreachable!("test request ID must be valid: {error}"))
    }

    fn test_descriptor(identity: &DeviceIdentity, port: u16) -> Result<RuntimeIpcDescriptor> {
        RuntimeIpcDescriptor::new(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port),
            [9_u8; 32],
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

    fn plan_summary() -> Result<RuntimeIpcRecipientImportExecutionPlanSummary> {
        Ok(RuntimeIpcRecipientImportExecutionPlanSummary {
            intent_id: artifact(1),
            execution_plan_id: artifact(2),
            coordinator_plan_id: artifact(3),
            coordinator_sequence: 4,
            membership_set_digest: membership_set_digest(&memberships())?,
            completed_source_count: 0,
            total_source_count: 2,
            max_steps: 2,
            planned_step_count: 2,
            next_source_ordinal: Some(2),
            coordinator_refresh_required: false,
            confirmation_phrase: "IMPORT 0202 0202 0202 0202".to_owned(),
        })
    }

    fn confirmation_summary() -> Result<RuntimeIpcRecipientImportExecutionConfirmationSummary> {
        Ok(RuntimeIpcRecipientImportExecutionConfirmationSummary {
            intent_id: artifact(1),
            execution_plan_id: artifact(2),
            coordinator_plan_id: artifact(3),
            coordinator_sequence: 4,
            membership_set_digest: membership_set_digest(&memberships())?,
            confirmation_id: artifact(4),
            confirmation_nonce: artifact(5),
            expires_at_unix_seconds: 500,
        })
    }

    fn receipt() -> RuntimeIpcRecipientImportExecutionReceiptSummary {
        RuntimeIpcRecipientImportExecutionReceiptSummary {
            intent_id: artifact(1),
            execution_plan_id: artifact(2),
            confirmation_id: artifact(4),
            mutation_nonce: artifact(6),
            starting_completed_source_count: 0,
            final_completed_source_count: 2,
            total_source_count: 2,
            steps: vec![
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 2,
                    disposition: RuntimeIpcRecipientImportStepDisposition::Imported,
                    imported_event_count: 3,
                    coordinator_sequence: 5,
                },
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 7,
                    disposition: RuntimeIpcRecipientImportStepDisposition::AlreadyImported,
                    imported_event_count: 4,
                    coordinator_sequence: 6,
                },
            ],
            imported_now_count: 1,
            already_imported_count: 1,
            imported_event_count: 7,
            final_coordinator_plan_id: artifact(7),
            final_coordinator_sequence: 6,
            complete: true,
        }
    }

    #[test]
    fn all_five_request_stages_round_trip_canonically() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42001)?;
        let commands = [
            RuntimeIpcRecipientImportExecutionCommand::Plan {
                intent_id: artifact(1),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                max_steps: 2,
                memberships: memberships(),
            },
            RuntimeIpcRecipientImportExecutionCommand::Status {
                intent_id: artifact(1),
                execution_plan_id: Some(artifact(2)),
            },
            RuntimeIpcRecipientImportExecutionCommand::Confirm {
                intent_id: artifact(1),
                execution_plan_id: artifact(2),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                memberships: memberships(),
                confirmation_nonce: artifact(5),
                confirmation_phrase: "IMPORT 0202 0202 0202 0202".to_owned(),
                explicit_approval: RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
            },
            RuntimeIpcRecipientImportExecutionCommand::ApplyNext {
                intent_id: artifact(1),
                execution_plan_id: artifact(2),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                memberships: memberships(),
                confirmation_id: artifact(4),
                mutation_nonce: artifact(6),
            },
            RuntimeIpcRecipientImportExecutionCommand::ApplyBounded {
                intent_id: artifact(1),
                execution_plan_id: artifact(2),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                memberships: memberships(),
                confirmation_id: artifact(4),
                mutation_nonce: artifact(6),
                max_steps: 2,
            },
        ];
        for (index, command) in commands.into_iter().enumerate() {
            let request = RuntimeIpcRecipientImportExecutionRequest::new(
                &descriptor,
                request_id(u8::try_from(index + 1)?),
                command,
            )?;
            let encoded = request.encode()?;
            assert_eq!(
                RuntimeIpcRecipientImportExecutionRequest::decode_and_validate(
                    &encoded,
                    &descriptor,
                )?,
                request
            );
        }
        Ok(())
    }

    #[test]
    fn reordered_duplicated_or_oversized_membership_references_fail_closed()
    -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42002)?;
        let mut reordered = memberships();
        reordered.swap(0, 1);
        let mut duplicated = memberships();
        duplicated[1].source_ordinal = duplicated[0].source_ordinal;
        let oversized = (0..=MAX_RUNTIME_IPC_RECIPIENT_IMPORT_EXECUTION_SOURCES)
            .map(|index| RuntimeIpcRecipientImportMembershipReference {
                source_ordinal: u16::try_from(index).unwrap_or(u16::MAX),
                authority_reference_id: artifact(u8::try_from(30 + index * 3).unwrap_or(250)),
                membership_digest: artifact(u8::try_from(31 + index * 3).unwrap_or(251)),
                authenticated_context_id: artifact(u8::try_from(32 + index * 3).unwrap_or(252)),
            })
            .collect::<Vec<_>>();
        for invalid in [reordered, duplicated, oversized] {
            assert!(
                RuntimeIpcRecipientImportExecutionRequest::new(
                    &descriptor,
                    request_id(1),
                    RuntimeIpcRecipientImportExecutionCommand::Plan {
                        intent_id: artifact(1),
                        coordinator_plan_id: artifact(3),
                        coordinator_sequence: 4,
                        max_steps: 1,
                        memberships: invalid,
                    },
                )
                .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn implicit_or_mismatched_confirmation_fails_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42003)?;
        for (approval, phrase) in [
            (
                RuntimeIpcRecipientImportExplicitApproval::NotAuthorized,
                "IMPORT 0202 0202 0202 0202",
            ),
            (
                RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
                "IMPORT 0303 0303 0303 0303",
            ),
        ] {
            assert!(
                RuntimeIpcRecipientImportExecutionRequest::new(
                    &descriptor,
                    request_id(1),
                    RuntimeIpcRecipientImportExecutionCommand::Confirm {
                        intent_id: artifact(1),
                        execution_plan_id: artifact(2),
                        coordinator_plan_id: artifact(3),
                        coordinator_sequence: 4,
                        memberships: memberships(),
                        confirmation_nonce: artifact(5),
                        confirmation_phrase: phrase.to_owned(),
                        explicit_approval: approval,
                    },
                )
                .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn descriptor_substitution_rejects_before_adapter_dispatch() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let first = test_descriptor(&identity, 42004)?;
        let second = test_descriptor(&identity, 42005)?;
        let request = RuntimeIpcRecipientImportExecutionRequest::new(
            &first,
            request_id(1),
            RuntimeIpcRecipientImportExecutionCommand::Status {
                intent_id: artifact(1),
                execution_plan_id: None,
            },
        )?;
        assert!(
            RuntimeIpcRecipientImportExecutionRequest::decode_and_validate(
                &request.encode()?,
                &second,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn signed_response_exact_binds_request_descriptor_and_device() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42006)?;
        let request = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportExecutionCommand::Plan {
                intent_id: artifact(1),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                max_steps: 2,
                memberships: memberships(),
            },
        )?;
        let signed = SignedRuntimeIpcRecipientImportExecutionResponse::issue(
            &request,
            &descriptor,
            &identity,
            RuntimeIpcRecipientImportExecutionOutcome::Planned(plan_summary()?),
        )?;
        let encoded = signed.encode()?;
        assert_eq!(
            SignedRuntimeIpcRecipientImportExecutionResponse::decode_and_verify(
                &encoded,
                &request,
                &descriptor,
            )?,
            signed
        );
        let other = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            request_id(2),
            request.command().clone(),
        )?;
        assert!(
            SignedRuntimeIpcRecipientImportExecutionResponse::decode_and_verify(
                &encoded,
                &other,
                &descriptor,
            )
            .is_err()
        );
        let foreign_identity = DeviceIdentity::generate()?;
        assert!(
            SignedRuntimeIpcRecipientImportExecutionResponse::issue(
                &request,
                &descriptor,
                &foreign_identity,
                signed.outcome().clone(),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn response_stage_skip_and_binding_substitution_fail_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42007)?;
        let plan_request = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportExecutionCommand::Plan {
                intent_id: artifact(1),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                max_steps: 2,
                memberships: memberships(),
            },
        )?;
        assert!(
            SignedRuntimeIpcRecipientImportExecutionResponse::issue(
                &plan_request,
                &descriptor,
                &identity,
                RuntimeIpcRecipientImportExecutionOutcome::Executed(receipt()),
            )
            .is_err()
        );
        let mut changed_plan = plan_summary()?;
        changed_plan.membership_set_digest = artifact(99);
        assert!(
            SignedRuntimeIpcRecipientImportExecutionResponse::issue(
                &plan_request,
                &descriptor,
                &identity,
                RuntimeIpcRecipientImportExecutionOutcome::Planned(changed_plan),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn mutation_nonce_confirmation_and_step_budget_are_exact_bound() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42008)?;
        let request = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportExecutionCommand::ApplyBounded {
                intent_id: artifact(1),
                execution_plan_id: artifact(2),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                memberships: memberships(),
                confirmation_id: artifact(4),
                mutation_nonce: artifact(6),
                max_steps: 2,
            },
        )?;
        let signed = SignedRuntimeIpcRecipientImportExecutionResponse::issue(
            &request,
            &descriptor,
            &identity,
            RuntimeIpcRecipientImportExecutionOutcome::Executed(receipt()),
        )?;
        assert!(!signed.intent_consumed());
        assert!(signed.membership_reload_required());
        assert!(signed.explicit_execution_required());

        let mut changed = receipt();
        changed.mutation_nonce = artifact(8);
        assert!(
            SignedRuntimeIpcRecipientImportExecutionResponse::issue(
                &request,
                &descriptor,
                &identity,
                RuntimeIpcRecipientImportExecutionOutcome::Executed(changed),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn confirmation_is_separate_and_cannot_consume_the_intent() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 42009)?;
        let request = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportExecutionCommand::Confirm {
                intent_id: artifact(1),
                execution_plan_id: artifact(2),
                coordinator_plan_id: artifact(3),
                coordinator_sequence: 4,
                memberships: memberships(),
                confirmation_nonce: artifact(5),
                confirmation_phrase: "IMPORT 0202 0202 0202 0202".to_owned(),
                explicit_approval: RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
            },
        )?;
        let signed = SignedRuntimeIpcRecipientImportExecutionResponse::issue(
            &request,
            &descriptor,
            &identity,
            RuntimeIpcRecipientImportExecutionOutcome::Confirmed {
                plan: plan_summary()?,
                confirmation: confirmation_summary()?,
            },
        )?;
        assert!(!signed.intent_consumed());
        assert!(matches!(
            signed.outcome(),
            RuntimeIpcRecipientImportExecutionOutcome::Confirmed { .. }
        ));
        Ok(())
    }
}
