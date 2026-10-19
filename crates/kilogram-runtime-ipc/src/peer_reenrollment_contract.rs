//! Dormant, privacy-minimizing local IPC contract for explicit peer re-enrollment.
//!
//! This codec deliberately is not a variant of [`super::RuntimeIpcCommand`].
//! It defines the future adapter boundary without activating a mutation path in
//! the production runtime mutation surface. M1.2.15 carries this codec only
//! through the v32 read-only adapter; plan/approve/accept remain rejected.

use std::{fmt, str::FromStr};

use anyhow::{Context, Result, bail, ensure};
use kilogram_identity::DeviceIdentity;
use serde::{Deserialize, Serialize};

use super::{
    RuntimeIpcDescriptor, RuntimeIpcPeerCapabilityQuarantineReason, RuntimeIpcRequestId,
    validate_opaque_contact_id,
};

pub const RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION: u8 = 1;
pub const MAX_RUNTIME_IPC_PEER_REENROLLMENT_REQUEST_BYTES: usize = 4 * 1024;
pub const MAX_RUNTIME_IPC_PEER_REENROLLMENT_RESPONSE_BYTES: usize = 16 * 1024;
pub const MAX_RUNTIME_IPC_PEER_REENROLLMENT_QUEUE_COUNT: u16 = 4_096;

const REQUEST_DIGEST_DOMAIN: &[u8] = b"kilogram:runtime-ipc-peer-reenrollment-request:v1\0";
const DESCRIPTOR_BINDING_DOMAIN: &[u8] = b"kilogram:runtime-ipc-peer-reenrollment-descriptor:v1\0";
const DEVICE_BINDING_DOMAIN: &[u8] = b"kilogram:runtime-ipc-peer-reenrollment-device:v1\0";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ipc-peer-reenrollment-response:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIpcPeerReenrollmentContactId(String);

impl RuntimeIpcPeerReenrollmentContactId {
    pub fn new(value: String) -> Result<Self> {
        validate_opaque_contact_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<()> {
        validate_opaque_contact_id(&self.0)
    }
}

impl fmt::Display for RuntimeIpcPeerReenrollmentContactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RuntimeIpcPeerReenrollmentContactId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value.to_owned())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIpcPeerReenrollmentArtifactId(String);

impl RuntimeIpcPeerReenrollmentArtifactId {
    pub fn new(value: String) -> Result<Self> {
        ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "runtime IPC peer re-enrollment artifact ID must be 64 lowercase hexadecimal characters"
        );
        ensure!(
            value.bytes().any(|byte| byte != b'0'),
            "runtime IPC peer re-enrollment artifact ID must not be zero"
        );
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        ensure!(
            bytes.iter().any(|byte| *byte != 0),
            "runtime IPC peer re-enrollment artifact ID must not be zero"
        );
        let mut encoded = String::with_capacity(64);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}")
                .context("encode runtime IPC peer re-enrollment artifact ID")?;
        }
        Ok(Self(encoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<()> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

impl fmt::Display for RuntimeIpcPeerReenrollmentArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RuntimeIpcPeerReenrollmentArtifactId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value.to_owned())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentExplicitApproval {
    NotAuthorized,
    UserConfirmed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentCommand {
    Inspect {
        contact_id: RuntimeIpcPeerReenrollmentContactId,
        staged_invitation_id: Option<RuntimeIpcPeerReenrollmentArtifactId>,
    },
    Plan {
        contact_id: RuntimeIpcPeerReenrollmentContactId,
        invitation_id: RuntimeIpcPeerReenrollmentArtifactId,
        quarantine_record_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    Approve {
        contact_id: RuntimeIpcPeerReenrollmentContactId,
        plan_id: RuntimeIpcPeerReenrollmentArtifactId,
        confirmation_phrase: String,
        explicit_approval: RuntimeIpcPeerReenrollmentExplicitApproval,
    },
    Accept {
        contact_id: RuntimeIpcPeerReenrollmentContactId,
        tombstone_id: RuntimeIpcPeerReenrollmentArtifactId,
        quarantine_record_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    Status {
        contact_id: RuntimeIpcPeerReenrollmentContactId,
    },
}

impl RuntimeIpcPeerReenrollmentCommand {
    pub fn contact_id(&self) -> &RuntimeIpcPeerReenrollmentContactId {
        match self {
            Self::Inspect { contact_id, .. }
            | Self::Plan { contact_id, .. }
            | Self::Approve { contact_id, .. }
            | Self::Accept { contact_id, .. }
            | Self::Status { contact_id } => contact_id,
        }
    }

    fn validate(&self) -> Result<()> {
        self.contact_id().validate()?;
        match self {
            Self::Inspect {
                staged_invitation_id,
                ..
            } => {
                if let Some(id) = staged_invitation_id {
                    id.validate()?;
                }
            }
            Self::Plan {
                invitation_id,
                quarantine_record_id,
                ..
            } => {
                invitation_id.validate()?;
                quarantine_record_id.validate()?;
            }
            Self::Approve {
                plan_id,
                confirmation_phrase,
                explicit_approval,
                ..
            } => {
                plan_id.validate()?;
                ensure!(
                    *explicit_approval == RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
                    "runtime IPC peer re-enrollment approval must be explicit"
                );
                validate_confirmation_phrase(confirmation_phrase)?;
                ensure!(
                    confirmation_phrase == &confirmation_phrase_for(plan_id),
                    "runtime IPC peer re-enrollment confirmation phrase does not match the plan ID"
                );
            }
            Self::Accept {
                tombstone_id,
                quarantine_record_id,
                ..
            } => {
                tombstone_id.validate()?;
                quarantine_record_id.validate()?;
            }
            Self::Status { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcPeerReenrollmentRequestWire {
    version: u8,
    request_id: RuntimeIpcRequestId,
    runtime_descriptor_binding: [u8; 32],
    command: RuntimeIpcPeerReenrollmentCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIpcPeerReenrollmentRequest {
    wire: RuntimeIpcPeerReenrollmentRequestWire,
}

impl RuntimeIpcPeerReenrollmentRequest {
    pub fn new(
        descriptor: &RuntimeIpcDescriptor,
        request_id: RuntimeIpcRequestId,
        command: RuntimeIpcPeerReenrollmentCommand,
    ) -> Result<Self> {
        descriptor.verify()?;
        command.validate()?;
        let request = Self {
            wire: RuntimeIpcPeerReenrollmentRequestWire {
                version: RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION,
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
            !encoded.is_empty() && encoded.len() <= MAX_RUNTIME_IPC_PEER_REENROLLMENT_REQUEST_BYTES,
            "runtime IPC peer re-enrollment request length is invalid"
        );
        let request = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode runtime IPC peer re-enrollment request")?,
        };
        request.validate_for_descriptor(descriptor)?;
        ensure!(
            request.encode()? == encoded,
            "runtime IPC peer re-enrollment request is not canonical"
        );
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode runtime IPC peer re-enrollment request")?;
        ensure!(
            !encoded.is_empty() && encoded.len() <= MAX_RUNTIME_IPC_PEER_REENROLLMENT_REQUEST_BYTES,
            "runtime IPC peer re-enrollment request length is invalid"
        );
        Ok(encoded)
    }

    pub fn request_id(&self) -> RuntimeIpcRequestId {
        self.wire.request_id
    }

    pub fn command(&self) -> &RuntimeIpcPeerReenrollmentCommand {
        &self.wire.command
    }

    pub fn request_digest(&self) -> Result<[u8; 32]> {
        Ok(domain_digest(REQUEST_DIGEST_DOMAIN, &self.encode()?))
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.wire.version == RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION,
            "unsupported runtime IPC peer re-enrollment request version"
        );
        ensure!(
            self.wire.runtime_descriptor_binding != [0_u8; 32],
            "runtime IPC peer re-enrollment request has no descriptor binding"
        );
        ensure!(
            self.wire
                .request_id
                .as_bytes()
                .iter()
                .any(|byte| *byte != 0),
            "runtime IPC peer re-enrollment request ID must not be zero"
        );
        self.wire.command.validate()
    }

    fn validate_for_descriptor(&self, descriptor: &RuntimeIpcDescriptor) -> Result<()> {
        self.validate()?;
        descriptor.verify()?;
        ensure!(
            self.wire.runtime_descriptor_binding == descriptor_binding(descriptor)?,
            "runtime IPC peer re-enrollment request names another runtime descriptor"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentStatus {
    MissingInvitation,
    UnsafeOrDivergent,
    Ready,
    ApprovalRequired,
    ApprovedPrepared,
    AcceptedRestartRequired,
    Consumed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentHazard {
    MissingDurableQuarantine,
    InvitationExpired,
    InvitationDoesNotMatchQuarantine,
    AuthorityDiverged,
    ArtifactReplay,
    UnsupportedCapability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcPeerReenrollmentInvitationSummary {
    pub invitation_id: RuntimeIpcPeerReenrollmentArtifactId,
    pub replacement_device_fingerprint: RuntimeIpcPeerReenrollmentArtifactId,
    pub required_capability_id: RuntimeIpcPeerReenrollmentArtifactId,
    pub authority_revision: u64,
    pub publication_epoch: u64,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

impl RuntimeIpcPeerReenrollmentInvitationSummary {
    fn validate(&self) -> Result<()> {
        self.invitation_id.validate()?;
        self.replacement_device_fingerprint.validate()?;
        self.required_capability_id.validate()?;
        ensure!(
            self.authority_revision != 0
                && self.publication_epoch != 0
                && self.issued_at_unix_seconds < self.expires_at_unix_seconds,
            "runtime IPC peer re-enrollment invitation summary is invalid"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcPeerReenrollmentQuarantineSummary {
    pub record_id: RuntimeIpcPeerReenrollmentArtifactId,
    pub sequence: u64,
    pub reason: RuntimeIpcPeerCapabilityQuarantineReason,
}

impl RuntimeIpcPeerReenrollmentQuarantineSummary {
    fn validate(&self) -> Result<()> {
        self.record_id.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcPeerReenrollmentPlanSummary {
    pub plan_id: RuntimeIpcPeerReenrollmentArtifactId,
    pub expires_at_unix_seconds: u64,
    pub confirmation_phrase: String,
}

impl RuntimeIpcPeerReenrollmentPlanSummary {
    pub fn new(
        plan_id: RuntimeIpcPeerReenrollmentArtifactId,
        expires_at_unix_seconds: u64,
    ) -> Result<Self> {
        let summary = Self {
            confirmation_phrase: confirmation_phrase_for(&plan_id),
            plan_id,
            expires_at_unix_seconds,
        };
        summary.validate()?;
        Ok(summary)
    }

    fn validate(&self) -> Result<()> {
        self.plan_id.validate()?;
        ensure!(
            self.expires_at_unix_seconds != 0,
            "runtime IPC peer re-enrollment plan expiry is invalid"
        );
        validate_confirmation_phrase(&self.confirmation_phrase)?;
        ensure!(
            self.confirmation_phrase == confirmation_phrase_for(&self.plan_id),
            "runtime IPC peer re-enrollment confirmation phrase does not match the plan ID"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentDurableOutcome {
    Unchanged,
    PlanPrepared {
        plan_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    ApprovalPrepared {
        plan_id: RuntimeIpcPeerReenrollmentArtifactId,
        tombstone_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    AcceptanceCommitted {
        acceptance_receipt_id: RuntimeIpcPeerReenrollmentArtifactId,
        runtime_transition_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    TransitionConsumed {
        acceptance_receipt_id: RuntimeIpcPeerReenrollmentArtifactId,
        runtime_transition_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
}

impl RuntimeIpcPeerReenrollmentDurableOutcome {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Unchanged => Ok(()),
            Self::PlanPrepared { plan_id } => plan_id.validate(),
            Self::ApprovalPrepared {
                plan_id,
                tombstone_id,
            } => {
                plan_id.validate()?;
                tombstone_id.validate()
            }
            Self::AcceptanceCommitted {
                acceptance_receipt_id,
                runtime_transition_id,
            }
            | Self::TransitionConsumed {
                acceptance_receipt_id,
                runtime_transition_id,
            } => {
                acceptance_receipt_id.validate()?;
                runtime_transition_id.validate()
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcPeerReenrollmentHistoryRecovery {
    NotEvaluated,
    SeparateApprovalRequired,
    ApprovedRewrapComplete {
        source_inventory_id: RuntimeIpcPeerReenrollmentArtifactId,
        event_count: u32,
    },
}

impl RuntimeIpcPeerReenrollmentHistoryRecovery {
    fn validate(&self) -> Result<()> {
        if let Self::ApprovedRewrapComplete {
            source_inventory_id,
            event_count,
        } = self
        {
            source_inventory_id.validate()?;
            ensure!(
                *event_count != 0,
                "runtime IPC peer re-enrollment history rewrap cannot claim an empty inventory"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcPeerReenrollmentSuppressionSummary {
    pub durable_quarantine_present: bool,
    pub process_local_sync_suppressed: bool,
    pub process_local_queue_suppressed_count: u16,
}

impl RuntimeIpcPeerReenrollmentSuppressionSummary {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.process_local_queue_suppressed_count
                <= MAX_RUNTIME_IPC_PEER_REENROLLMENT_QUEUE_COUNT,
            "runtime IPC peer re-enrollment suppressed queue count exceeds its bound"
        );
        ensure!(
            self.process_local_queue_suppressed_count == 0 || self.process_local_sync_suppressed,
            "runtime IPC peer re-enrollment queue suppression lacks process-local sync suppression"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIpcPeerReenrollmentResponse {
    pub contact_id: RuntimeIpcPeerReenrollmentContactId,
    pub status: RuntimeIpcPeerReenrollmentStatus,
    pub hazard: Option<RuntimeIpcPeerReenrollmentHazard>,
    pub invitation: Option<RuntimeIpcPeerReenrollmentInvitationSummary>,
    pub quarantine: Option<RuntimeIpcPeerReenrollmentQuarantineSummary>,
    pub plan: Option<RuntimeIpcPeerReenrollmentPlanSummary>,
    pub durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome,
    pub suppression: RuntimeIpcPeerReenrollmentSuppressionSummary,
    pub history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery,
    pub restart_required: bool,
}

impl RuntimeIpcPeerReenrollmentResponse {
    pub fn validate(&self) -> Result<()> {
        self.contact_id.validate()?;
        if let Some(invitation) = &self.invitation {
            invitation.validate()?;
        }
        if let Some(quarantine) = &self.quarantine {
            quarantine.validate()?;
        }
        if let Some(plan) = &self.plan {
            plan.validate()?;
        }
        self.durable_outcome.validate()?;
        self.suppression.validate()?;
        self.history_recovery.validate()?;

        let has_invitation = self.invitation.is_some();
        let has_quarantine = self.quarantine.is_some();
        let has_plan = self.plan.is_some();
        match self.status {
            RuntimeIpcPeerReenrollmentStatus::MissingInvitation => {
                ensure!(
                    !has_invitation
                        && has_quarantine
                        && !has_plan
                        && self.hazard.is_none()
                        && self.durable_outcome
                            == RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged
                        && !self.restart_required,
                    "runtime IPC missing-invitation response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent => {
                ensure!(
                    self.hazard.is_some()
                        && !has_plan
                        && self.durable_outcome
                            == RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged
                        && !self.restart_required,
                    "runtime IPC unsafe peer re-enrollment response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::Ready => {
                ensure!(
                    has_invitation
                        && has_quarantine
                        && !has_plan
                        && self.hazard.is_none()
                        && self.durable_outcome
                            == RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged
                        && !self.restart_required,
                    "runtime IPC ready peer re-enrollment response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovalRequired => {
                let Some(plan) = &self.plan else {
                    bail!("runtime IPC approval-required response has no plan");
                };
                let RuntimeIpcPeerReenrollmentDurableOutcome::PlanPrepared { plan_id } =
                    &self.durable_outcome
                else {
                    bail!("runtime IPC approval-required response has no prepared plan");
                };
                ensure!(
                    has_invitation
                        && has_quarantine
                        && self.hazard.is_none()
                        && plan.plan_id == *plan_id
                        && !self.restart_required,
                    "runtime IPC approval-required response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared => {
                let Some(plan) = &self.plan else {
                    bail!("runtime IPC approved response has no plan");
                };
                let RuntimeIpcPeerReenrollmentDurableOutcome::ApprovalPrepared { plan_id, .. } =
                    &self.durable_outcome
                else {
                    bail!("runtime IPC approved response has no prepared approval");
                };
                ensure!(
                    has_invitation
                        && has_quarantine
                        && self.hazard.is_none()
                        && plan.plan_id == *plan_id
                        && !self.restart_required,
                    "runtime IPC approved peer re-enrollment response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired => {
                ensure!(
                    has_invitation
                        && has_quarantine
                        && has_plan
                        && self.hazard.is_none()
                        && matches!(
                            self.durable_outcome,
                            RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceCommitted { .. }
                        )
                        && self.restart_required,
                    "runtime IPC accepted peer re-enrollment response is inconsistent"
                );
            }
            RuntimeIpcPeerReenrollmentStatus::Consumed => {
                ensure!(
                    has_invitation
                        && has_quarantine
                        && self.hazard.is_none()
                        && matches!(
                            self.durable_outcome,
                            RuntimeIpcPeerReenrollmentDurableOutcome::TransitionConsumed { .. }
                        )
                        && !self.restart_required,
                    "runtime IPC consumed peer re-enrollment response is inconsistent"
                );
            }
        }
        ensure!(
            !self.suppression.durable_quarantine_present || self.quarantine.is_some(),
            "runtime IPC durable quarantine flag lacks durable quarantine summary"
        );
        ensure!(
            self.quarantine.is_none() || self.suppression.durable_quarantine_present,
            "runtime IPC durable quarantine summary was mislabeled as process-local state"
        );
        Ok(())
    }

    fn validate_for_command(&self, command: &RuntimeIpcPeerReenrollmentCommand) -> Result<()> {
        self.validate()?;
        ensure!(
            &self.contact_id == command.contact_id(),
            "runtime IPC peer re-enrollment response names another contact"
        );
        let allowed = match command {
            RuntimeIpcPeerReenrollmentCommand::Inspect { .. } => matches!(
                self.status,
                RuntimeIpcPeerReenrollmentStatus::MissingInvitation
                    | RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent
                    | RuntimeIpcPeerReenrollmentStatus::Ready
            ),
            RuntimeIpcPeerReenrollmentCommand::Plan { .. } => matches!(
                self.status,
                RuntimeIpcPeerReenrollmentStatus::MissingInvitation
                    | RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent
                    | RuntimeIpcPeerReenrollmentStatus::ApprovalRequired
            ),
            RuntimeIpcPeerReenrollmentCommand::Approve { .. } => matches!(
                self.status,
                RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent
                    | RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared
            ),
            RuntimeIpcPeerReenrollmentCommand::Accept { .. } => matches!(
                self.status,
                RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent
                    | RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired
            ),
            RuntimeIpcPeerReenrollmentCommand::Status { .. } => true,
        };
        ensure!(
            allowed,
            "runtime IPC peer re-enrollment response skips or rewinds the requested stage"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcPeerReenrollmentSignedResponseContent {
    version: u8,
    request_digest: [u8; 32],
    runtime_descriptor_binding: [u8; 32],
    current_device_binding: [u8; 32],
    response: RuntimeIpcPeerReenrollmentResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIpcPeerReenrollmentSignedResponseWire {
    content: RuntimeIpcPeerReenrollmentSignedResponseContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRuntimeIpcPeerReenrollmentResponse {
    wire: RuntimeIpcPeerReenrollmentSignedResponseWire,
}

impl SignedRuntimeIpcPeerReenrollmentResponse {
    pub fn issue(
        request: &RuntimeIpcPeerReenrollmentRequest,
        descriptor: &RuntimeIpcDescriptor,
        identity: &DeviceIdentity,
        response: RuntimeIpcPeerReenrollmentResponse,
    ) -> Result<Self> {
        request.validate_for_descriptor(descriptor)?;
        response.validate_for_command(request.command())?;
        ensure!(
            descriptor.content.device_id == identity.device_id(),
            "runtime IPC peer re-enrollment response signer is not the descriptor Device"
        );
        let content = RuntimeIpcPeerReenrollmentSignedResponseContent {
            version: RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION,
            request_digest: request.request_digest()?,
            runtime_descriptor_binding: descriptor_binding(descriptor)?,
            current_device_binding: device_binding(descriptor.content.device_id.as_bytes()),
            response,
        };
        let signature = identity.sign(&response_signing_bytes(&content)?).to_vec();
        let signed = Self {
            wire: RuntimeIpcPeerReenrollmentSignedResponseWire { content, signature },
        };
        signed.verify(request, descriptor)?;
        Ok(signed)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        request: &RuntimeIpcPeerReenrollmentRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<Self> {
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_PEER_REENROLLMENT_RESPONSE_BYTES,
            "runtime IPC peer re-enrollment response length is invalid"
        );
        let signed = Self {
            wire: postcard::from_bytes(encoded)
                .context("decode signed runtime IPC peer re-enrollment response")?,
        };
        signed.verify(request, descriptor)?;
        ensure!(
            signed.encode()? == encoded,
            "runtime IPC peer re-enrollment response is not canonical"
        );
        Ok(signed)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        ensure!(
            self.wire.content.version == RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION,
            "unsupported runtime IPC peer re-enrollment response version"
        );
        self.wire.content.response.validate()?;
        let encoded = postcard::to_allocvec(&self.wire)
            .context("encode signed runtime IPC peer re-enrollment response")?;
        ensure!(
            !encoded.is_empty()
                && encoded.len() <= MAX_RUNTIME_IPC_PEER_REENROLLMENT_RESPONSE_BYTES,
            "runtime IPC peer re-enrollment response length is invalid"
        );
        Ok(encoded)
    }

    pub fn response(&self) -> &RuntimeIpcPeerReenrollmentResponse {
        &self.wire.content.response
    }

    pub fn request_digest(&self) -> [u8; 32] {
        self.wire.content.request_digest
    }

    fn verify(
        &self,
        request: &RuntimeIpcPeerReenrollmentRequest,
        descriptor: &RuntimeIpcDescriptor,
    ) -> Result<()> {
        request.validate_for_descriptor(descriptor)?;
        ensure!(
            self.wire.content.version == RUNTIME_IPC_PEER_REENROLLMENT_CONTRACT_VERSION,
            "unsupported runtime IPC peer re-enrollment response version"
        );
        self.wire
            .content
            .response
            .validate_for_command(request.command())?;
        ensure!(
            self.wire.content.request_digest == request.request_digest()?,
            "runtime IPC peer re-enrollment response names another request"
        );
        ensure!(
            self.wire.content.runtime_descriptor_binding == descriptor_binding(descriptor)?,
            "runtime IPC peer re-enrollment response names another runtime descriptor"
        );
        ensure!(
            self.wire.content.current_device_binding
                == device_binding(descriptor.content.device_id.as_bytes()),
            "runtime IPC peer re-enrollment response names another current Device"
        );
        descriptor
            .content
            .device_id
            .verify(
                &response_signing_bytes(&self.wire.content)?,
                &self.wire.signature,
            )
            .context("verify runtime IPC peer re-enrollment response signature")?;
        Ok(())
    }
}

fn validate_confirmation_phrase(value: &str) -> Result<()> {
    let chunks: Vec<&str> = value.split(' ').collect();
    ensure!(
        chunks.len() == 5
            && chunks[0] == "REENROLL"
            && chunks[1..].iter().all(|chunk| {
                chunk.len() == 4
                    && chunk
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }),
        "runtime IPC peer re-enrollment confirmation phrase is invalid"
    );
    Ok(())
}

fn confirmation_phrase_for(plan_id: &RuntimeIpcPeerReenrollmentArtifactId) -> String {
    let encoded = plan_id.as_str();
    format!(
        "REENROLL {} {} {} {}",
        &encoded[0..4],
        &encoded[4..8],
        &encoded[8..12],
        &encoded[12..16]
    )
}

fn descriptor_binding(descriptor: &RuntimeIpcDescriptor) -> Result<[u8; 32]> {
    descriptor.verify()?;
    let encoded =
        postcard::to_allocvec(descriptor).context("encode runtime IPC descriptor binding")?;
    Ok(domain_digest(DESCRIPTOR_BINDING_DOMAIN, &encoded))
}

fn device_binding(device_id: &[u8; 32]) -> [u8; 32] {
    domain_digest(DEVICE_BINDING_DOMAIN, device_id)
}

fn response_signing_bytes(
    content: &RuntimeIpcPeerReenrollmentSignedResponseContent,
) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content)
        .context("encode runtime IPC peer re-enrollment response content")?;
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

    fn artifact(byte: u8) -> RuntimeIpcPeerReenrollmentArtifactId {
        RuntimeIpcPeerReenrollmentArtifactId::from_bytes([byte; 32])
            .unwrap_or_else(|error| unreachable!("nonzero test artifact must be valid: {error}"))
    }

    fn contact(byte: u8) -> RuntimeIpcPeerReenrollmentContactId {
        format!("{byte:064x}")
            .parse()
            .unwrap_or_else(|error| unreachable!("test contact must be valid: {error}"))
    }

    fn test_descriptor(identity: &DeviceIdentity, port: u16) -> Result<RuntimeIpcDescriptor> {
        RuntimeIpcDescriptor::new(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port),
            [9_u8; 32],
            AccountId::from_bytes([7_u8; 32]),
            identity,
        )
    }

    fn request_id(byte: u8) -> RuntimeIpcRequestId {
        format!("{byte:064x}")
            .parse()
            .unwrap_or_else(|error| unreachable!("test request ID must be valid: {error}"))
    }

    fn invitation() -> RuntimeIpcPeerReenrollmentInvitationSummary {
        RuntimeIpcPeerReenrollmentInvitationSummary {
            invitation_id: artifact(2),
            replacement_device_fingerprint: artifact(3),
            required_capability_id: artifact(4),
            authority_revision: 5,
            publication_epoch: 6,
            issued_at_unix_seconds: 100,
            expires_at_unix_seconds: 200,
        }
    }

    fn quarantine() -> RuntimeIpcPeerReenrollmentQuarantineSummary {
        RuntimeIpcPeerReenrollmentQuarantineSummary {
            record_id: artifact(5),
            sequence: 7,
            reason: RuntimeIpcPeerCapabilityQuarantineReason::PinnedCanonicalCapabilityMissing,
        }
    }

    fn plan() -> RuntimeIpcPeerReenrollmentPlanSummary {
        RuntimeIpcPeerReenrollmentPlanSummary {
            plan_id: artifact(6),
            expires_at_unix_seconds: 190,
            confirmation_phrase: "REENROLL 0606 0606 0606 0606".to_owned(),
        }
    }

    fn suppression() -> RuntimeIpcPeerReenrollmentSuppressionSummary {
        RuntimeIpcPeerReenrollmentSuppressionSummary {
            durable_quarantine_present: true,
            process_local_sync_suppressed: true,
            process_local_queue_suppressed_count: 1,
        }
    }

    #[test]
    fn all_five_request_stages_round_trip_canonically() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 41001)?;
        let commands = [
            RuntimeIpcPeerReenrollmentCommand::Inspect {
                contact_id: contact(1),
                staged_invitation_id: Some(artifact(2)),
            },
            RuntimeIpcPeerReenrollmentCommand::Plan {
                contact_id: contact(1),
                invitation_id: artifact(2),
                quarantine_record_id: artifact(5),
            },
            RuntimeIpcPeerReenrollmentCommand::Approve {
                contact_id: contact(1),
                plan_id: artifact(6),
                confirmation_phrase: "REENROLL 0606 0606 0606 0606".to_owned(),
                explicit_approval: RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
            },
            RuntimeIpcPeerReenrollmentCommand::Accept {
                contact_id: contact(1),
                tombstone_id: artifact(7),
                quarantine_record_id: artifact(5),
            },
            RuntimeIpcPeerReenrollmentCommand::Status {
                contact_id: contact(1),
            },
        ];
        for (index, command) in commands.into_iter().enumerate() {
            let request = RuntimeIpcPeerReenrollmentRequest::new(
                &descriptor,
                request_id(u8::try_from(index + 1)?),
                command,
            )?;
            let encoded = request.encode()?;
            assert_eq!(
                RuntimeIpcPeerReenrollmentRequest::decode_and_validate(&encoded, &descriptor)?,
                request
            );
        }
        Ok(())
    }

    #[test]
    fn implicit_approval_and_malformed_confirmation_fail_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 41002)?;
        for (approval, phrase) in [
            (
                RuntimeIpcPeerReenrollmentExplicitApproval::NotAuthorized,
                "REENROLL 0606 0606 0606 0606",
            ),
            (
                RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
                "reenroll 0606 0606 0606 0606",
            ),
            (
                RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
                "REENROLL 0707 0707 0707 0707",
            ),
        ] {
            assert!(
                RuntimeIpcPeerReenrollmentRequest::new(
                    &descriptor,
                    request_id(1),
                    RuntimeIpcPeerReenrollmentCommand::Approve {
                        contact_id: contact(1),
                        plan_id: artifact(6),
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
    fn descriptor_substitution_rejects_request_before_adapter_dispatch()
    -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let first = test_descriptor(&identity, 41003)?;
        let second = test_descriptor(&identity, 41004)?;
        let request = RuntimeIpcPeerReenrollmentRequest::new(
            &first,
            request_id(1),
            RuntimeIpcPeerReenrollmentCommand::Status {
                contact_id: contact(1),
            },
        )?;
        let encoded = request.encode()?;
        assert!(RuntimeIpcPeerReenrollmentRequest::decode_and_validate(&encoded, &second).is_err());
        Ok(())
    }

    #[test]
    fn signed_response_exact_binds_request_descriptor_and_current_device()
    -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 41005)?;
        let request = RuntimeIpcPeerReenrollmentRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcPeerReenrollmentCommand::Inspect {
                contact_id: contact(1),
                staged_invitation_id: Some(artifact(2)),
            },
        )?;
        let response = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::Ready,
            hazard: None,
            invitation: Some(invitation()),
            quarantine: Some(quarantine()),
            plan: None,
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged,
            suppression: suppression(),
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: false,
        };
        let signed = SignedRuntimeIpcPeerReenrollmentResponse::issue(
            &request,
            &descriptor,
            &identity,
            response,
        )?;
        let encoded = signed.encode()?;
        assert_eq!(
            SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify(
                &encoded,
                &request,
                &descriptor,
            )?,
            signed
        );

        let other_request = RuntimeIpcPeerReenrollmentRequest::new(
            &descriptor,
            request_id(2),
            RuntimeIpcPeerReenrollmentCommand::Inspect {
                contact_id: contact(1),
                staged_invitation_id: Some(artifact(2)),
            },
        )?;
        assert!(
            SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify(
                &encoded,
                &other_request,
                &descriptor,
            )
            .is_err()
        );
        let replacement_descriptor = test_descriptor(&identity, 41006)?;
        assert!(
            SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify(
                &encoded,
                &request,
                &replacement_descriptor,
            )
            .is_err()
        );
        let foreign_identity = DeviceIdentity::generate()?;
        assert!(
            SignedRuntimeIpcPeerReenrollmentResponse::issue(
                &request,
                &descriptor,
                &foreign_identity,
                signed.response().clone(),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn stage_skips_and_inconsistent_durable_claims_fail_closed() -> Result<(), Box<dyn Error>> {
        let identity = DeviceIdentity::generate()?;
        let descriptor = test_descriptor(&identity, 41007)?;
        let inspect = RuntimeIpcPeerReenrollmentRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcPeerReenrollmentCommand::Inspect {
                contact_id: contact(1),
                staged_invitation_id: Some(artifact(2)),
            },
        )?;
        let accepted = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired,
            hazard: None,
            invitation: Some(invitation()),
            quarantine: Some(quarantine()),
            plan: Some(plan()),
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceCommitted {
                acceptance_receipt_id: artifact(8),
                runtime_transition_id: artifact(9),
            },
            suppression: suppression(),
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: true,
        };
        assert!(
            SignedRuntimeIpcPeerReenrollmentResponse::issue(
                &inspect,
                &descriptor,
                &identity,
                accepted,
            )
            .is_err()
        );

        let mut ready = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::Ready,
            hazard: None,
            invitation: Some(invitation()),
            quarantine: Some(quarantine()),
            plan: None,
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged,
            suppression: suppression(),
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: false,
        };
        ready.suppression.durable_quarantine_present = false;
        assert!(ready.validate().is_err());
        ready.suppression.durable_quarantine_present = true;
        ready.restart_required = true;
        assert!(ready.validate().is_err());
        Ok(())
    }

    #[test]
    fn process_local_suppression_never_substitutes_for_durable_quarantine() {
        let response = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent,
            hazard: Some(RuntimeIpcPeerReenrollmentHazard::MissingDurableQuarantine),
            invitation: Some(invitation()),
            quarantine: None,
            plan: None,
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged,
            suppression: RuntimeIpcPeerReenrollmentSuppressionSummary {
                durable_quarantine_present: false,
                process_local_sync_suppressed: true,
                process_local_queue_suppressed_count: 1,
            },
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::NotEvaluated,
            restart_required: false,
        };
        assert!(response.validate().is_ok());
        let mut false_authority = response;
        false_authority.quarantine = Some(quarantine());
        assert!(false_authority.validate().is_err());
    }

    #[test]
    fn history_rewrap_is_explicit_and_independent_from_reenrollment() -> Result<(), Box<dyn Error>>
    {
        let mut response = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::Consumed,
            hazard: None,
            invitation: Some(invitation()),
            quarantine: Some(quarantine()),
            plan: None,
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::TransitionConsumed {
                acceptance_receipt_id: artifact(8),
                runtime_transition_id: artifact(9),
            },
            suppression: suppression(),
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: false,
        };
        response.validate()?;
        response.history_recovery =
            RuntimeIpcPeerReenrollmentHistoryRecovery::ApprovedRewrapComplete {
                source_inventory_id: artifact(10),
                event_count: 0,
            };
        assert!(response.validate().is_err());
        Ok(())
    }

    #[test]
    fn public_projection_contains_no_raw_identity_transport_or_message_fields()
    -> Result<(), Box<dyn Error>> {
        let response = RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact(1),
            status: RuntimeIpcPeerReenrollmentStatus::ApprovalRequired,
            hazard: None,
            invitation: Some(invitation()),
            quarantine: Some(quarantine()),
            plan: Some(plan()),
            durable_outcome: RuntimeIpcPeerReenrollmentDurableOutcome::PlanPrepared {
                plan_id: artifact(6),
            },
            suppression: suppression(),
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: false,
        };
        response.validate()?;
        let projection = serde_json::to_string(&response)?;
        for forbidden in [
            "account_id",
            "device_id",
            "root_secret",
            "device_secret",
            "ticket",
            "endpoint",
            "queue_id",
            "message",
            "capability_bytes",
        ] {
            assert!(!projection.contains(forbidden), "leaked field: {forbidden}");
        }
        Ok(())
    }
}
