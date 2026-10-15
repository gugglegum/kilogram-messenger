//! Inactive deterministic-CBOR history recovery link, plan, and page exchange.
//!
//! The active recovery implementation remains Postcard encoded. This module
//! completes the candidate recovery control plane without defining the
//! separate opaque history-rewrap payload carried by a successful page.

use std::{net::SocketAddr, str::FromStr};

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        decode_and_verify_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        signed_envelope,
    },
    canonical_authority_v1::{
        CanonicalAuthorityError, CanonicalSignedDeviceCertificateV1,
        decode_and_verify_signed_device_certificate_v1, encode_signed_device_certificate_v1,
    },
    canonical_recovery_v1::{
        CanonicalHistoryRecoveryCheckpointContentV1, CanonicalRecoveryError, history_recovery_id_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RECOVERY_COORDINATES: usize = 16;
pub const MAX_CANONICAL_RECOVERY_RELAY_COORDINATE_BYTES: usize = 2048;
pub const MAX_CANONICAL_RECOVERY_DIRECT_COORDINATE_BYTES: usize = 128;
pub const MAX_CANONICAL_RECOVERY_LINK_VALIDITY_SECONDS: u64 = 3600;
pub const MAX_CANONICAL_RECOVERY_PLAN_VALIDITY_SECONDS: u64 = 604_800;
pub const MAX_CANONICAL_RECOVERY_INVENTORY_EVENTS: u64 = 4096;
pub const MAX_CANONICAL_RECOVERY_EXCHANGE_PAGE_SIZE: u64 = 256;
pub const MAX_CANONICAL_RECOVERY_NESTED_ARTIFACT_BYTES: usize = 1024 * 1024;
pub const MAX_CANONICAL_RECOVERY_OPAQUE_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANONICAL_RECOVERY_RESPONSE_BYTES: usize = 9 * 1024 * 1024;

pub const CANONICAL_HISTORY_RECOVERY_SAS_DOMAIN: &[u8] = b"kilogram:history-recovery-sas:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-link-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_LINK_ID_DOMAIN: &[u8] =
    b"kilogram:history-recovery-link-id:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-plan-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_PLAN_ID_DOMAIN: &[u8] =
    b"kilogram:history-recovery-plan-id:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_PAGE_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-page-request-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-response-signature:cbor-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalRecoveryCoordinateKindV1 {
    RelayUrl = 1,
    DirectSocketAddress = 2,
}

impl CanonicalRecoveryCoordinateKindV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalRecoveryExchangeError> {
        match code {
            1 => Ok(Self::RelayUrl),
            2 => Ok(Self::DirectSocketAddress),
            other => Err(CanonicalRecoveryExchangeError::UnknownCoordinateKind(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRecoveryCoordinateV1 {
    pub kind: CanonicalRecoveryCoordinateKindV1,
    pub value: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalRecoveryRoutePolicyV1 {
    Auto = 1,
    DirectOnly = 2,
    RelayOnly = 3,
}

impl CanonicalRecoveryRoutePolicyV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalRecoveryExchangeError> {
        match code {
            1 => Ok(Self::Auto),
            2 => Ok(Self::DirectOnly),
            3 => Ok(Self::RelayOnly),
            other => Err(CanonicalRecoveryExchangeError::UnknownRoutePolicy(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryLinkContentV1 {
    pub endpoint_id: [u8; 32],
    pub coordinates: Vec<CanonicalRecoveryCoordinateV1>,
    pub source_certificate: CanonicalSignedDeviceCertificateV1,
    pub account_device_list: CanonicalSignedAccountDeviceListV1,
    pub recipient_device_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub approved_range_start: u64,
    pub approved_range_end: u64,
    pub page_size: u64,
    pub route_policy: CanonicalRecoveryRoutePolicyV1,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRecoveryLinkV1 {
    pub content: CanonicalHistoryRecoveryLinkContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRecoveryExecutionPolicyV1 {
    pub allow_ethernet: bool,
    pub allow_wifi: bool,
    pub allow_mobile: bool,
    pub allow_unknown_network: bool,
    pub require_external_power: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryPlanContentV1 {
    pub signed_link: CanonicalSignedHistoryRecoveryLinkV1,
    pub sas: [u8; 32],
    pub execution_policy: CanonicalRecoveryExecutionPolicyV1,
    pub approved_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRecoveryPlanV1 {
    pub content: CanonicalHistoryRecoveryPlanContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryPageRequestContentV1 {
    pub plan_id: [u8; 32],
    pub recovery_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub range_start: u64,
    pub maximum_event_count: u64,
    pub sas: [u8; 32],
    pub previous_checkpoint_id: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRecoveryPageRequestV1 {
    pub content: CanonicalHistoryRecoveryPageRequestContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryPageV1 {
    pub signed_request: CanonicalSignedHistoryRecoveryPageRequestV1,
    pub inventory_event_count: u64,
    pub inventory_digest: [u8; 32],
    pub range_start: u64,
    pub range_end: u64,
    pub opaque_rewrapped_payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalHistoryRecoveryRejectionReasonV1 {
    NotApproved = 1,
    ApprovalMismatch = 2,
    PageTooLarge = 3,
}

impl CanonicalHistoryRecoveryRejectionReasonV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalRecoveryExchangeError> {
        match code {
            1 => Ok(Self::NotApproved),
            2 => Ok(Self::ApprovalMismatch),
            3 => Ok(Self::PageTooLarge),
            other => Err(CanonicalRecoveryExchangeError::UnknownRejectionReason(
                other,
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryRejectedV1 {
    pub signed_request: CanonicalSignedHistoryRecoveryPageRequestV1,
    pub reason: CanonicalHistoryRecoveryRejectionReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalHistoryRecoveryResponseV1 {
    Page(CanonicalHistoryRecoveryPageV1),
    Rejected(CanonicalHistoryRecoveryRejectedV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRecoveryResponseV1 {
    pub content: CanonicalHistoryRecoveryResponseV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalRecoveryExchangeError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    Authority(#[from] CanonicalAuthorityError),

    #[error(transparent)]
    Recovery(#[from] CanonicalRecoveryError),

    #[error("canonical recovery {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },

    #[error(
        "canonical recovery link must contain between one and {MAX_CANONICAL_RECOVERY_COORDINATES} coordinates"
    )]
    InvalidCoordinateCount,

    #[error("unknown canonical recovery coordinate kind {0}")]
    UnknownCoordinateKind(u64),

    #[error("canonical recovery coordinates must be strictly ordered and duplicate free")]
    NonCanonicalCoordinateOrder,

    #[error("canonical recovery coordinate is malformed")]
    InvalidCoordinate,

    #[error("unknown canonical recovery route policy {0}")]
    UnknownRoutePolicy(u64),

    #[error("canonical recovery coordinates do not satisfy the selected route policy")]
    RoutePolicyMismatch,

    #[error("recovery source and recipient Device IDs must differ")]
    SameDevice,

    #[error(
        "source certificate is not the exact authorized source entry in the Account device list"
    )]
    SourceCertificateMismatch,

    #[error("recovery recipient is not authorized by the Account device list")]
    RecipientNotAuthorized,

    #[error("canonical recovery approved range is invalid")]
    InvalidApprovedRange,

    #[error("canonical recovery page size {actual} is outside 1..={maximum}")]
    InvalidPageSize { actual: u64, maximum: u64 },

    #[error(
        "canonical recovery link validity is invalid or exceeds {MAX_CANONICAL_RECOVERY_LINK_VALIDITY_SECONDS} seconds"
    )]
    InvalidLinkValidity,

    #[error("canonical recovery execution policy permits no network")]
    NoPermittedNetwork,

    #[error("canonical recovery plan approval is outside the link validity interval")]
    ApprovalOutsideLinkValidity,

    #[error(
        "canonical recovery plan validity is invalid or exceeds {MAX_CANONICAL_RECOVERY_PLAN_VALIDITY_SECONDS} seconds"
    )]
    InvalidPlanValidity,

    #[error("canonical recovery SAS does not match the exact link roles and Account device list")]
    SasMismatch,

    #[error("canonical recovery {object} contains an invalid Ed25519 key")]
    InvalidPublicKey { object: &'static str },

    #[error("canonical recovery {object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },

    #[error("canonical recovery request names a different signed plan")]
    PlanMismatch,

    #[error("canonical recovery request names a different recovery identity")]
    RecoveryIdentityMismatch,

    #[error("canonical recovery request changes an approved role or conversation")]
    RequestIdentityMismatch,

    #[error("canonical recovery request is bound to a different live session")]
    SessionBindingMismatch,

    #[error("canonical recovery request range is outside the approved plan")]
    RequestRangeOutsidePlan,

    #[error("canonical recovery page range does not satisfy its exact signed request")]
    PageRangeMismatch,

    #[error("canonical recovery page inventory claim does not cover its returned range")]
    InventoryClaimMismatch,

    #[error(
        "canonical recovery opaque payload exceeds {MAX_CANONICAL_RECOVERY_OPAQUE_PAYLOAD_BYTES} bytes"
    )]
    OpaquePayloadTooLarge,

    #[error("unknown canonical recovery response variant {0}")]
    UnknownResponseVariant(u64),

    #[error("unknown canonical recovery rejection reason {0}")]
    UnknownRejectionReason(u64),

    #[error("{field} contains integer {actual}; expected canonical boolean 0 or 1")]
    InvalidBoolean { field: &'static str, actual: u64 },
}

pub fn encode_history_recovery_link_content_v1(
    content: &CanonicalHistoryRecoveryLinkContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    validate_link_content(content)?;
    let certificate = encode_signed_device_certificate_v1(&content.source_certificate)?;
    let device_list = encode_signed_account_device_list_v1(&content.account_device_list)?;
    ensure_nested(&certificate, "source certificate")?;
    ensure_nested(&device_list, "Account device list")?;
    let mut output = Vec::new();
    push_array_header(&mut output, 13);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.endpoint_id);
    push_array_header(&mut output, content.coordinates.len() as u64);
    for coordinate in &content.coordinates {
        push_array_header(&mut output, 2);
        push_unsigned(&mut output, coordinate.kind.code());
        push_bytes(&mut output, &coordinate.value);
    }
    push_bytes(&mut output, &certificate);
    push_bytes(&mut output, &device_list);
    push_bytes(&mut output, &content.recipient_device_id);
    push_bytes(&mut output, &content.conversation_id);
    push_unsigned(&mut output, content.approved_range_start);
    push_unsigned(&mut output, content.approved_range_end);
    push_unsigned(&mut output, content.page_size);
    push_unsigned(&mut output, content.route_policy.code());
    push_unsigned(&mut output, content.issued_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    ensure_artifact(output, "link content")
}

pub fn history_recovery_link_signing_bytes_v1(
    content: &CanonicalHistoryRecoveryLinkContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    Ok(prefixed(
        CANONICAL_HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN,
        &encode_history_recovery_link_content_v1(content)?,
    ))
}

pub fn encode_signed_history_recovery_link_v1(
    link: &CanonicalSignedHistoryRecoveryLinkV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_link_v1(link)?;
    let content = encode_history_recovery_link_content_v1(&link.content)?;
    ensure_artifact(signed_envelope(&content, &link.signature), "signed link")
}

pub fn decode_signed_history_recovery_link_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryLinkV1, CanonicalRecoveryExchangeError> {
    validate_artifact_input(bytes, "signed link")?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-recovery-link-v1", 3)?;
    decoder.expect_version("signed-history-recovery-link-v1.schema_version")?;
    let content = decode_link_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-recovery-link-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedHistoryRecoveryLinkV1 { content, signature })
}

pub fn decode_and_verify_signed_history_recovery_link_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryLinkV1, CanonicalRecoveryExchangeError> {
    let link = decode_signed_history_recovery_link_v1(bytes)?;
    verify_signed_history_recovery_link_v1(&link)?;
    Ok(link)
}

pub fn verify_signed_history_recovery_link_v1(
    link: &CanonicalSignedHistoryRecoveryLinkV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    validate_link_content(&link.content)?;
    verify_ed25519(
        &link.content.source_certificate.content.device_id,
        &history_recovery_link_signing_bytes_v1(&link.content)?,
        &link.signature,
        "signed link",
    )
}

pub fn history_recovery_link_id_v1(
    link: &CanonicalSignedHistoryRecoveryLinkV1,
) -> Result<[u8; 32], CanonicalRecoveryExchangeError> {
    domain_hash(
        CANONICAL_HISTORY_RECOVERY_LINK_ID_DOMAIN,
        &encode_signed_history_recovery_link_v1(link)?,
    )
}

pub fn history_recovery_sas_v1(
    device_list: &CanonicalSignedAccountDeviceListV1,
    source_device_id: [u8; 32],
    recipient_device_id: [u8; 32],
) -> Result<[u8; 32], CanonicalRecoveryExchangeError> {
    if source_device_id == recipient_device_id {
        return Err(CanonicalRecoveryExchangeError::SameDevice);
    }
    let encoded_list = encode_signed_account_device_list_v1(device_list)?;
    ensure_nested(&encoded_list, "Account device list")?;
    let mut input = Vec::new();
    push_array_header(&mut input, 4);
    push_unsigned(&mut input, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut input, &encoded_list);
    push_bytes(&mut input, &source_device_id);
    push_bytes(&mut input, &recipient_device_id);
    domain_hash(CANONICAL_HISTORY_RECOVERY_SAS_DOMAIN, &input)
}

pub fn encode_recovery_execution_policy_v1(
    policy: &CanonicalRecoveryExecutionPolicyV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    validate_execution_policy(policy)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, u64::from(policy.allow_ethernet));
    push_unsigned(&mut output, u64::from(policy.allow_wifi));
    push_unsigned(&mut output, u64::from(policy.allow_mobile));
    push_unsigned(&mut output, u64::from(policy.allow_unknown_network));
    push_unsigned(&mut output, u64::from(policy.require_external_power));
    Ok(output)
}

pub fn encode_history_recovery_plan_content_v1(
    content: &CanonicalHistoryRecoveryPlanContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    validate_plan_content(content)?;
    let link = encode_signed_history_recovery_link_v1(&content.signed_link)?;
    let policy = encode_recovery_execution_policy_v1(&content.execution_policy)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &link);
    push_bytes(&mut output, &content.sas);
    output.extend_from_slice(&policy);
    push_unsigned(&mut output, content.approved_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    ensure_artifact(output, "plan content")
}

pub fn history_recovery_plan_signing_bytes_v1(
    content: &CanonicalHistoryRecoveryPlanContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    Ok(prefixed(
        CANONICAL_HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN,
        &encode_history_recovery_plan_content_v1(content)?,
    ))
}

pub fn encode_signed_history_recovery_plan_v1(
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_plan_v1(plan)?;
    let content = encode_history_recovery_plan_content_v1(&plan.content)?;
    ensure_artifact(signed_envelope(&content, &plan.signature), "signed plan")
}

pub fn decode_signed_history_recovery_plan_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryPlanV1, CanonicalRecoveryExchangeError> {
    validate_artifact_input(bytes, "signed plan")?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-recovery-plan-v1", 3)?;
    decoder.expect_version("signed-history-recovery-plan-v1.schema_version")?;
    let content = decode_plan_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-recovery-plan-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedHistoryRecoveryPlanV1 { content, signature })
}

pub fn decode_and_verify_signed_history_recovery_plan_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryPlanV1, CanonicalRecoveryExchangeError> {
    let plan = decode_signed_history_recovery_plan_v1(bytes)?;
    verify_signed_history_recovery_plan_v1(&plan)?;
    Ok(plan)
}

pub fn verify_signed_history_recovery_plan_v1(
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    validate_plan_content(&plan.content)?;
    verify_ed25519(
        &plan.content.signed_link.content.recipient_device_id,
        &history_recovery_plan_signing_bytes_v1(&plan.content)?,
        &plan.signature,
        "signed plan",
    )
}

pub fn history_recovery_plan_id_v1(
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<[u8; 32], CanonicalRecoveryExchangeError> {
    domain_hash(
        CANONICAL_HISTORY_RECOVERY_PLAN_ID_DOMAIN,
        &encode_signed_history_recovery_plan_v1(plan)?,
    )
}

pub fn history_recovery_id_for_plan_v1(
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<[u8; 32], CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_plan_v1(plan)?;
    let link = &plan.content.signed_link.content;
    Ok(history_recovery_id_v1(
        &CanonicalHistoryRecoveryCheckpointContentV1 {
            account_id: link
                .account_device_list
                .content
                .authority_snapshot
                .content
                .account_id,
            conversation_id: link.conversation_id,
            source_device_id: link.source_certificate.content.device_id,
            recipient_device_id: link.recipient_device_id,
            sas: plan.content.sas,
            approved_range_start: link.approved_range_start,
            approved_range_end: link.approved_range_end,
            page_size: link.page_size,
            inventory_claim: None,
            next_range_start: link.approved_range_start,
            previous_checkpoint_id: None,
        },
    )?)
}

pub fn encode_history_recovery_page_request_content_v1(
    content: &CanonicalHistoryRecoveryPageRequestContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    validate_request_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 11);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.plan_id);
    push_bytes(&mut output, &content.recovery_id);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.source_device_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_bytes(&mut output, &content.session_binding);
    push_unsigned(&mut output, content.range_start);
    push_unsigned(&mut output, content.maximum_event_count);
    push_bytes(&mut output, &content.sas);
    encode_optional_id(&mut output, content.previous_checkpoint_id);
    ensure_artifact(output, "page request content")
}

pub fn history_recovery_page_request_signing_bytes_v1(
    content: &CanonicalHistoryRecoveryPageRequestContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    Ok(prefixed(
        CANONICAL_HISTORY_RECOVERY_PAGE_REQUEST_SIGNATURE_DOMAIN,
        &encode_history_recovery_page_request_content_v1(content)?,
    ))
}

pub fn encode_signed_history_recovery_page_request_v1(
    request: &CanonicalSignedHistoryRecoveryPageRequestV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_page_request_v1(request)?;
    let content = encode_history_recovery_page_request_content_v1(&request.content)?;
    ensure_artifact(
        signed_envelope(&content, &request.signature),
        "signed page request",
    )
}

pub fn decode_signed_history_recovery_page_request_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryPageRequestV1, CanonicalRecoveryExchangeError> {
    validate_artifact_input(bytes, "signed page request")?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-recovery-page-request-v1", 3)?;
    decoder.expect_version("signed-history-recovery-page-request-v1.schema_version")?;
    let content = decode_request_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-recovery-page-request-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedHistoryRecoveryPageRequestV1 { content, signature })
}

pub fn decode_and_verify_signed_history_recovery_page_request_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryPageRequestV1, CanonicalRecoveryExchangeError> {
    let request = decode_signed_history_recovery_page_request_v1(bytes)?;
    verify_signed_history_recovery_page_request_v1(&request)?;
    Ok(request)
}

pub fn verify_signed_history_recovery_page_request_v1(
    request: &CanonicalSignedHistoryRecoveryPageRequestV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    validate_request_content(&request.content)?;
    verify_ed25519(
        &request.content.recipient_device_id,
        &history_recovery_page_request_signing_bytes_v1(&request.content)?,
        &request.signature,
        "signed page request",
    )
}

pub fn verify_history_recovery_page_request_for_plan_v1(
    request: &CanonicalSignedHistoryRecoveryPageRequestV1,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
    expected_session_binding: [u8; 32],
) -> Result<(), CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_page_request_v1(request)?;
    verify_signed_history_recovery_plan_v1(plan)?;
    let link = &plan.content.signed_link.content;
    if request.content.plan_id != history_recovery_plan_id_v1(plan)? {
        return Err(CanonicalRecoveryExchangeError::PlanMismatch);
    }
    if request.content.recovery_id != history_recovery_id_for_plan_v1(plan)? {
        return Err(CanonicalRecoveryExchangeError::RecoveryIdentityMismatch);
    }
    if request.content.conversation_id != link.conversation_id
        || request.content.source_device_id != link.source_certificate.content.device_id
        || request.content.recipient_device_id != link.recipient_device_id
        || request.content.sas != plan.content.sas
    {
        return Err(CanonicalRecoveryExchangeError::RequestIdentityMismatch);
    }
    if request.content.session_binding != expected_session_binding {
        return Err(CanonicalRecoveryExchangeError::SessionBindingMismatch);
    }
    let request_end = request
        .content
        .range_start
        .checked_add(request.content.maximum_event_count)
        .ok_or(CanonicalRecoveryExchangeError::RequestRangeOutsidePlan)?;
    if request.content.range_start < link.approved_range_start
        || request.content.range_start >= link.approved_range_end
        || request_end > link.approved_range_end
        || request.content.maximum_event_count > link.page_size
    {
        return Err(CanonicalRecoveryExchangeError::RequestRangeOutsidePlan);
    }
    Ok(())
}

pub fn encode_history_recovery_response_content_v1(
    content: &CanonicalHistoryRecoveryResponseV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    validate_response_content(content)?;
    let (variant, payload) = match content {
        CanonicalHistoryRecoveryResponseV1::Page(page) => (1, encode_page_v1(page)?),
        CanonicalHistoryRecoveryResponseV1::Rejected(rejected) => {
            (2, encode_rejected_v1(rejected)?)
        }
    };
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, variant);
    push_bytes(&mut output, &payload);
    ensure_response(output)
}

pub fn history_recovery_response_signing_bytes_v1(
    content: &CanonicalHistoryRecoveryResponseV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    Ok(prefixed(
        CANONICAL_HISTORY_RECOVERY_RESPONSE_SIGNATURE_DOMAIN,
        &encode_history_recovery_response_content_v1(content)?,
    ))
}

pub fn encode_signed_history_recovery_response_v1(
    response: &CanonicalSignedHistoryRecoveryResponseV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_response_v1(response)?;
    let content = encode_history_recovery_response_content_v1(&response.content)?;
    ensure_response(signed_envelope(&content, &response.signature))
}

pub fn decode_signed_history_recovery_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryResponseV1, CanonicalRecoveryExchangeError> {
    validate_response_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-recovery-response-v1", 3)?;
    decoder.expect_version("signed-history-recovery-response-v1.schema_version")?;
    let content = decode_response_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-recovery-response-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedHistoryRecoveryResponseV1 { content, signature })
}

pub fn decode_and_verify_signed_history_recovery_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryResponseV1, CanonicalRecoveryExchangeError> {
    let response = decode_signed_history_recovery_response_v1(bytes)?;
    verify_signed_history_recovery_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_history_recovery_response_v1(
    response: &CanonicalSignedHistoryRecoveryResponseV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response_request(&response.content).content.source_device_id,
        &history_recovery_response_signing_bytes_v1(&response.content)?,
        &response.signature,
        "signed response",
    )
}

pub fn verify_history_recovery_response_for_plan_v1(
    response: &CanonicalSignedHistoryRecoveryResponseV1,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
    expected_session_binding: [u8; 32],
) -> Result<(), CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_response_v1(response)?;
    verify_history_recovery_page_request_for_plan_v1(
        response_request(&response.content),
        plan,
        expected_session_binding,
    )
}

fn validate_link_content(
    content: &CanonicalHistoryRecoveryLinkContentV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    encode_signed_device_certificate_v1(&content.source_certificate)?;
    encode_signed_account_device_list_v1(&content.account_device_list)?;
    let source = content.source_certificate.content.device_id;
    if source == content.recipient_device_id {
        return Err(CanonicalRecoveryExchangeError::SameDevice);
    }
    if !content
        .account_device_list
        .content
        .devices
        .iter()
        .any(|certificate| certificate == &content.source_certificate)
    {
        return Err(CanonicalRecoveryExchangeError::SourceCertificateMismatch);
    }
    if !content
        .account_device_list
        .content
        .devices
        .iter()
        .any(|certificate| certificate.content.device_id == content.recipient_device_id)
    {
        return Err(CanonicalRecoveryExchangeError::RecipientNotAuthorized);
    }
    validate_coordinates(&content.coordinates, content.route_policy)?;
    if content.approved_range_start >= content.approved_range_end
        || content.approved_range_end - content.approved_range_start
            > MAX_CANONICAL_RECOVERY_INVENTORY_EVENTS
    {
        return Err(CanonicalRecoveryExchangeError::InvalidApprovedRange);
    }
    if content.page_size == 0 || content.page_size > MAX_CANONICAL_RECOVERY_EXCHANGE_PAGE_SIZE {
        return Err(CanonicalRecoveryExchangeError::InvalidPageSize {
            actual: content.page_size,
            maximum: MAX_CANONICAL_RECOVERY_EXCHANGE_PAGE_SIZE,
        });
    }
    if content.expires_at_unix_seconds <= content.issued_at_unix_seconds
        || content.expires_at_unix_seconds - content.issued_at_unix_seconds
            > MAX_CANONICAL_RECOVERY_LINK_VALIDITY_SECONDS
    {
        return Err(CanonicalRecoveryExchangeError::InvalidLinkValidity);
    }
    Ok(())
}

fn validate_coordinates(
    coordinates: &[CanonicalRecoveryCoordinateV1],
    route_policy: CanonicalRecoveryRoutePolicyV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    if coordinates.is_empty() || coordinates.len() > MAX_CANONICAL_RECOVERY_COORDINATES {
        return Err(CanonicalRecoveryExchangeError::InvalidCoordinateCount);
    }
    if coordinates.windows(2).any(|pair| {
        (pair[0].kind.code(), pair[0].value.as_slice())
            >= (pair[1].kind.code(), pair[1].value.as_slice())
    }) {
        return Err(CanonicalRecoveryExchangeError::NonCanonicalCoordinateOrder);
    }
    for coordinate in coordinates {
        let limit = match coordinate.kind {
            CanonicalRecoveryCoordinateKindV1::RelayUrl => {
                MAX_CANONICAL_RECOVERY_RELAY_COORDINATE_BYTES
            }
            CanonicalRecoveryCoordinateKindV1::DirectSocketAddress => {
                MAX_CANONICAL_RECOVERY_DIRECT_COORDINATE_BYTES
            }
        };
        if coordinate.value.is_empty()
            || coordinate.value.len() > limit
            || !coordinate.value.iter().all(|byte| byte.is_ascii_graphic())
        {
            return Err(CanonicalRecoveryExchangeError::InvalidCoordinate);
        }
        let text = std::str::from_utf8(&coordinate.value)
            .map_err(|_| CanonicalRecoveryExchangeError::InvalidCoordinate)?;
        match coordinate.kind {
            CanonicalRecoveryCoordinateKindV1::RelayUrl => {
                if !text.starts_with("https://") || !text.ends_with('/') {
                    return Err(CanonicalRecoveryExchangeError::InvalidCoordinate);
                }
            }
            CanonicalRecoveryCoordinateKindV1::DirectSocketAddress => {
                let address = SocketAddr::from_str(text)
                    .map_err(|_| CanonicalRecoveryExchangeError::InvalidCoordinate)?;
                if address.to_string() != text {
                    return Err(CanonicalRecoveryExchangeError::InvalidCoordinate);
                }
            }
        }
    }
    let valid = match route_policy {
        CanonicalRecoveryRoutePolicyV1::Auto => true,
        CanonicalRecoveryRoutePolicyV1::DirectOnly => coordinates.iter().all(|coordinate| {
            coordinate.kind == CanonicalRecoveryCoordinateKindV1::DirectSocketAddress
        }),
        CanonicalRecoveryRoutePolicyV1::RelayOnly => coordinates
            .iter()
            .all(|coordinate| coordinate.kind == CanonicalRecoveryCoordinateKindV1::RelayUrl),
    };
    if valid {
        Ok(())
    } else {
        Err(CanonicalRecoveryExchangeError::RoutePolicyMismatch)
    }
}

fn validate_execution_policy(
    policy: &CanonicalRecoveryExecutionPolicyV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    if policy.allow_ethernet
        || policy.allow_wifi
        || policy.allow_mobile
        || policy.allow_unknown_network
    {
        Ok(())
    } else {
        Err(CanonicalRecoveryExchangeError::NoPermittedNetwork)
    }
}

fn validate_plan_content(
    content: &CanonicalHistoryRecoveryPlanContentV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    verify_signed_history_recovery_link_v1(&content.signed_link)?;
    validate_execution_policy(&content.execution_policy)?;
    if content.sas
        != history_recovery_sas_v1(
            &content.signed_link.content.account_device_list,
            content
                .signed_link
                .content
                .source_certificate
                .content
                .device_id,
            content.signed_link.content.recipient_device_id,
        )?
    {
        return Err(CanonicalRecoveryExchangeError::SasMismatch);
    }
    let link = &content.signed_link.content;
    if content.approved_at_unix_seconds < link.issued_at_unix_seconds
        || content.approved_at_unix_seconds > link.expires_at_unix_seconds
    {
        return Err(CanonicalRecoveryExchangeError::ApprovalOutsideLinkValidity);
    }
    if content.expires_at_unix_seconds <= content.approved_at_unix_seconds
        || content.expires_at_unix_seconds - content.approved_at_unix_seconds
            > MAX_CANONICAL_RECOVERY_PLAN_VALIDITY_SECONDS
    {
        return Err(CanonicalRecoveryExchangeError::InvalidPlanValidity);
    }
    Ok(())
}

fn validate_request_content(
    content: &CanonicalHistoryRecoveryPageRequestContentV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    if content.source_device_id == content.recipient_device_id {
        return Err(CanonicalRecoveryExchangeError::SameDevice);
    }
    if content.maximum_event_count == 0
        || content.maximum_event_count > MAX_CANONICAL_RECOVERY_EXCHANGE_PAGE_SIZE
    {
        return Err(CanonicalRecoveryExchangeError::InvalidPageSize {
            actual: content.maximum_event_count,
            maximum: MAX_CANONICAL_RECOVERY_EXCHANGE_PAGE_SIZE,
        });
    }
    content
        .range_start
        .checked_add(content.maximum_event_count)
        .ok_or(CanonicalRecoveryExchangeError::RequestRangeOutsidePlan)?;
    Ok(())
}

fn validate_response_content(
    content: &CanonicalHistoryRecoveryResponseV1,
) -> Result<(), CanonicalRecoveryExchangeError> {
    match content {
        CanonicalHistoryRecoveryResponseV1::Page(page) => {
            verify_signed_history_recovery_page_request_v1(&page.signed_request)?;
            if page.range_start != page.signed_request.content.range_start
                || page.range_end <= page.range_start
                || page.range_end
                    > page
                        .range_start
                        .checked_add(page.signed_request.content.maximum_event_count)
                        .ok_or(CanonicalRecoveryExchangeError::PageRangeMismatch)?
            {
                return Err(CanonicalRecoveryExchangeError::PageRangeMismatch);
            }
            if page.inventory_event_count > MAX_CANONICAL_RECOVERY_INVENTORY_EVENTS
                || page.inventory_event_count < page.range_end
            {
                return Err(CanonicalRecoveryExchangeError::InventoryClaimMismatch);
            }
            if page.opaque_rewrapped_payload.len() > MAX_CANONICAL_RECOVERY_OPAQUE_PAYLOAD_BYTES {
                return Err(CanonicalRecoveryExchangeError::OpaquePayloadTooLarge);
            }
        }
        CanonicalHistoryRecoveryResponseV1::Rejected(rejected) => {
            verify_signed_history_recovery_page_request_v1(&rejected.signed_request)?;
        }
    }
    Ok(())
}

fn decode_link_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRecoveryLinkContentV1, CanonicalRecoveryExchangeError> {
    decoder.expect_array("history-recovery-link-content-v1", 13)?;
    decoder.expect_version("history-recovery-link-content-v1.schema_version")?;
    let endpoint_id = decoder.fixed_bytes("history-recovery-link-content-v1.endpoint_id")?;
    let count = decoder.array("history-recovery-link-content-v1.coordinates")?;
    if count == 0 || count > MAX_CANONICAL_RECOVERY_COORDINATES {
        return Err(CanonicalRecoveryExchangeError::InvalidCoordinateCount);
    }
    let mut coordinates = Vec::with_capacity(count);
    for _ in 0..count {
        decoder.expect_array("recovery-coordinate-v1", 2)?;
        let kind = CanonicalRecoveryCoordinateKindV1::from_code(
            decoder.unsigned("recovery-coordinate-v1.kind")?,
        )?;
        let value = decoder.bytes("recovery-coordinate-v1.value")?.to_vec();
        coordinates.push(CanonicalRecoveryCoordinateV1 { kind, value });
    }
    let certificate_bytes = decoder.bytes("history-recovery-link-content-v1.source_certificate")?;
    ensure_nested(certificate_bytes, "source certificate")?;
    let source_certificate = decode_and_verify_signed_device_certificate_v1(certificate_bytes)?;
    let list_bytes = decoder.bytes("history-recovery-link-content-v1.account_device_list")?;
    ensure_nested(list_bytes, "Account device list")?;
    let account_device_list = decode_and_verify_signed_account_device_list_v1(list_bytes)?;
    let recipient_device_id =
        decoder.fixed_bytes("history-recovery-link-content-v1.recipient_device_id")?;
    let conversation_id =
        decoder.fixed_bytes("history-recovery-link-content-v1.conversation_id")?;
    let approved_range_start =
        decoder.unsigned("history-recovery-link-content-v1.approved_range_start")?;
    let approved_range_end =
        decoder.unsigned("history-recovery-link-content-v1.approved_range_end")?;
    let page_size = decoder.unsigned("history-recovery-link-content-v1.page_size")?;
    let route_policy = CanonicalRecoveryRoutePolicyV1::from_code(
        decoder.unsigned("history-recovery-link-content-v1.route_policy")?,
    )?;
    let issued_at_unix_seconds =
        decoder.unsigned("history-recovery-link-content-v1.issued_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("history-recovery-link-content-v1.expires_at_unix_seconds")?;
    let content = CanonicalHistoryRecoveryLinkContentV1 {
        endpoint_id,
        coordinates,
        source_certificate,
        account_device_list,
        recipient_device_id,
        conversation_id,
        approved_range_start,
        approved_range_end,
        page_size,
        route_policy,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    };
    validate_link_content(&content)?;
    Ok(content)
}

fn decode_plan_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRecoveryPlanContentV1, CanonicalRecoveryExchangeError> {
    decoder.expect_array("history-recovery-plan-content-v1", 6)?;
    decoder.expect_version("history-recovery-plan-content-v1.schema_version")?;
    let link_bytes = decoder.bytes("history-recovery-plan-content-v1.signed_link")?;
    ensure_nested(link_bytes, "signed link")?;
    let signed_link = decode_and_verify_signed_history_recovery_link_v1(link_bytes)?;
    let sas = decoder.fixed_bytes("history-recovery-plan-content-v1.sas")?;
    let execution_policy = decode_execution_policy_from(decoder)?;
    let approved_at_unix_seconds =
        decoder.unsigned("history-recovery-plan-content-v1.approved_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("history-recovery-plan-content-v1.expires_at_unix_seconds")?;
    let content = CanonicalHistoryRecoveryPlanContentV1 {
        signed_link,
        sas,
        execution_policy,
        approved_at_unix_seconds,
        expires_at_unix_seconds,
    };
    validate_plan_content(&content)?;
    Ok(content)
}

fn decode_execution_policy_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRecoveryExecutionPolicyV1, CanonicalRecoveryExchangeError> {
    decoder.expect_array("recovery-execution-policy-v1", 6)?;
    decoder.expect_version("recovery-execution-policy-v1.schema_version")?;
    let policy = CanonicalRecoveryExecutionPolicyV1 {
        allow_ethernet: decode_bool(decoder, "recovery-execution-policy-v1.allow_ethernet")?,
        allow_wifi: decode_bool(decoder, "recovery-execution-policy-v1.allow_wifi")?,
        allow_mobile: decode_bool(decoder, "recovery-execution-policy-v1.allow_mobile")?,
        allow_unknown_network: decode_bool(
            decoder,
            "recovery-execution-policy-v1.allow_unknown_network",
        )?,
        require_external_power: decode_bool(
            decoder,
            "recovery-execution-policy-v1.require_external_power",
        )?,
    };
    validate_execution_policy(&policy)?;
    Ok(policy)
}

fn decode_request_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRecoveryPageRequestContentV1, CanonicalRecoveryExchangeError> {
    decoder.expect_array("history-recovery-page-request-content-v1", 11)?;
    decoder.expect_version("history-recovery-page-request-content-v1.schema_version")?;
    let content = CanonicalHistoryRecoveryPageRequestContentV1 {
        plan_id: decoder.fixed_bytes("history-recovery-page-request-content-v1.plan_id")?,
        recovery_id: decoder.fixed_bytes("history-recovery-page-request-content-v1.recovery_id")?,
        conversation_id: decoder
            .fixed_bytes("history-recovery-page-request-content-v1.conversation_id")?,
        source_device_id: decoder
            .fixed_bytes("history-recovery-page-request-content-v1.source_device_id")?,
        recipient_device_id: decoder
            .fixed_bytes("history-recovery-page-request-content-v1.recipient_device_id")?,
        session_binding: decoder
            .fixed_bytes("history-recovery-page-request-content-v1.session_binding")?,
        range_start: decoder.unsigned("history-recovery-page-request-content-v1.range_start")?,
        maximum_event_count: decoder
            .unsigned("history-recovery-page-request-content-v1.maximum_event_count")?,
        sas: decoder.fixed_bytes("history-recovery-page-request-content-v1.sas")?,
        previous_checkpoint_id: decode_optional_id(
            decoder,
            "history-recovery-page-request-content-v1.previous_checkpoint_id",
        )?,
    };
    validate_request_content(&content)?;
    Ok(content)
}

fn encode_page_v1(
    page: &CanonicalHistoryRecoveryPageV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    let request = encode_signed_history_recovery_page_request_v1(&page.signed_request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request);
    push_unsigned(&mut output, page.inventory_event_count);
    push_bytes(&mut output, &page.inventory_digest);
    push_unsigned(&mut output, page.range_start);
    push_unsigned(&mut output, page.range_end);
    push_bytes(&mut output, &page.opaque_rewrapped_payload);
    ensure_response(output)
}

fn decode_page_v1(
    bytes: &[u8],
) -> Result<CanonicalHistoryRecoveryPageV1, CanonicalRecoveryExchangeError> {
    validate_response_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("history-recovery-page-v1", 7)?;
    decoder.expect_version("history-recovery-page-v1.schema_version")?;
    let request_bytes = decoder.bytes("history-recovery-page-v1.signed_request")?;
    ensure_nested(request_bytes, "signed page request")?;
    let page = CanonicalHistoryRecoveryPageV1 {
        signed_request: decode_and_verify_signed_history_recovery_page_request_v1(request_bytes)?,
        inventory_event_count: decoder
            .unsigned("history-recovery-page-v1.inventory_event_count")?,
        inventory_digest: decoder.fixed_bytes("history-recovery-page-v1.inventory_digest")?,
        range_start: decoder.unsigned("history-recovery-page-v1.range_start")?,
        range_end: decoder.unsigned("history-recovery-page-v1.range_end")?,
        opaque_rewrapped_payload: decoder
            .bytes("history-recovery-page-v1.opaque_rewrapped_payload")?
            .to_vec(),
    };
    decoder.finish()?;
    validate_response_content(&CanonicalHistoryRecoveryResponseV1::Page(page.clone()))?;
    Ok(page)
}

fn encode_rejected_v1(
    rejected: &CanonicalHistoryRecoveryRejectedV1,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    let request = encode_signed_history_recovery_page_request_v1(&rejected.signed_request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request);
    push_unsigned(&mut output, rejected.reason.code());
    ensure_artifact(output, "rejected response")
}

fn decode_rejected_v1(
    bytes: &[u8],
) -> Result<CanonicalHistoryRecoveryRejectedV1, CanonicalRecoveryExchangeError> {
    validate_artifact_input(bytes, "rejected response")?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("history-recovery-rejected-v1", 3)?;
    decoder.expect_version("history-recovery-rejected-v1.schema_version")?;
    let request_bytes = decoder.bytes("history-recovery-rejected-v1.signed_request")?;
    ensure_nested(request_bytes, "signed page request")?;
    let rejected = CanonicalHistoryRecoveryRejectedV1 {
        signed_request: decode_and_verify_signed_history_recovery_page_request_v1(request_bytes)?,
        reason: CanonicalHistoryRecoveryRejectionReasonV1::from_code(
            decoder.unsigned("history-recovery-rejected-v1.reason")?,
        )?,
    };
    decoder.finish()?;
    Ok(rejected)
}

fn decode_response_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRecoveryResponseV1, CanonicalRecoveryExchangeError> {
    decoder.expect_array("history-recovery-response-content-v1", 3)?;
    decoder.expect_version("history-recovery-response-content-v1.schema_version")?;
    let variant = decoder.unsigned("history-recovery-response-content-v1.variant")?;
    let payload = decoder.bytes("history-recovery-response-content-v1.payload")?;
    let content = match variant {
        1 => CanonicalHistoryRecoveryResponseV1::Page(decode_page_v1(payload)?),
        2 => CanonicalHistoryRecoveryResponseV1::Rejected(decode_rejected_v1(payload)?),
        other => {
            return Err(CanonicalRecoveryExchangeError::UnknownResponseVariant(
                other,
            ));
        }
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn response_request(
    response: &CanonicalHistoryRecoveryResponseV1,
) -> &CanonicalSignedHistoryRecoveryPageRequestV1 {
    match response {
        CanonicalHistoryRecoveryResponseV1::Page(page) => &page.signed_request,
        CanonicalHistoryRecoveryResponseV1::Rejected(rejected) => &rejected.signed_request,
    }
}

fn encode_optional_id(output: &mut Vec<u8>, id: Option<[u8; 32]>) {
    match id {
        None => push_array_header(output, 0),
        Some(id) => {
            push_array_header(output, 1);
            push_bytes(output, &id);
        }
    }
}

fn decode_optional_id(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Option<[u8; 32]>, CanonicalRecoveryExchangeError> {
    match decoder.array(field)? {
        0 => Ok(None),
        1 => Ok(Some(decoder.fixed_bytes(field)?)),
        actual => Err(CanonicalAccountError::WrongArrayLength {
            object: field,
            expected: 1,
            actual,
        }
        .into()),
    }
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalRecoveryExchangeError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalRecoveryExchangeError::InvalidBoolean { field, actual }),
    }
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalRecoveryExchangeError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalRecoveryExchangeError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalRecoveryExchangeError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalRecoveryExchangeError::InvalidSignature { object })
}

fn domain_hash(domain: &[u8], content: &[u8]) -> Result<[u8; 32], CanonicalRecoveryExchangeError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(content);
    Ok(*hasher.finalize().as_bytes())
}

fn ensure_nested(bytes: &[u8], object: &'static str) -> Result<(), CanonicalRecoveryExchangeError> {
    if bytes.len() > MAX_CANONICAL_RECOVERY_NESTED_ARTIFACT_BYTES {
        Err(CanonicalRecoveryExchangeError::ObjectTooLarge {
            object,
            maximum: MAX_CANONICAL_RECOVERY_NESTED_ARTIFACT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn ensure_artifact(
    bytes: Vec<u8>,
    object: &'static str,
) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    ensure_nested(&bytes, object)?;
    Ok(bytes)
}

fn ensure_response(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalRecoveryExchangeError> {
    if bytes.len() > MAX_CANONICAL_RECOVERY_RESPONSE_BYTES {
        Err(CanonicalRecoveryExchangeError::ObjectTooLarge {
            object: "response",
            maximum: MAX_CANONICAL_RECOVERY_RESPONSE_BYTES,
        })
    } else {
        Ok(bytes)
    }
}

fn validate_artifact_input(
    bytes: &[u8],
    object: &'static str,
) -> Result<(), CanonicalRecoveryExchangeError> {
    ensure_nested(bytes, object)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn validate_response_input(bytes: &[u8]) -> Result<(), CanonicalRecoveryExchangeError> {
    if bytes.len() > MAX_CANONICAL_RECOVERY_RESPONSE_BYTES {
        return Err(CanonicalRecoveryExchangeError::ObjectTooLarge {
            object: "response",
            maximum: MAX_CANONICAL_RECOVERY_RESPONSE_BYTES,
        });
    }
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;

    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;

    use super::*;

    fn vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RECOVERY-EXCHANGE-VECTORS.json"
        )))?)
    }

    fn text_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, io::Error> {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::other(format!("missing string vector field {pointer}")))
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err(io::Error::other("odd hexadecimal length").into());
        }
        let mut output = Vec::with_capacity(value.len() / 2);
        for pair in value.as_bytes().as_chunks::<2>().0 {
            let text = std::str::from_utf8(pair)?;
            output.push(u8::from_str_radix(text, 16)?);
        }
        Ok(output)
    }

    fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        decode_hex(value)?
            .try_into()
            .map_err(|_| io::Error::other(format!("expected {N} hexadecimal bytes")).into())
    }

    fn signing_key(
        vectors: &Value,
        pointer: &str,
    ) -> Result<SigningKey, Box<dyn std::error::Error>> {
        Ok(SigningKey::from_bytes(&fixed_hex(text_at(
            vectors, pointer,
        )?)?))
    }

    fn resign_request(
        request: &mut CanonicalSignedHistoryRecoveryPageRequestV1,
        key: &SigningKey,
    ) -> Result<(), CanonicalRecoveryExchangeError> {
        request.signature = key
            .sign(&history_recovery_page_request_signing_bytes_v1(
                &request.content,
            )?)
            .to_bytes();
        Ok(())
    }

    fn resign_response(
        response: &mut CanonicalSignedHistoryRecoveryResponseV1,
        key: &SigningKey,
    ) -> Result<(), CanonicalRecoveryExchangeError> {
        response.signature = key
            .sign(&history_recovery_response_signing_bytes_v1(
                &response.content,
            )?)
            .to_bytes();
        Ok(())
    }

    #[test]
    fn golden_recovery_exchange_matches_exact_bytes_signatures_and_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let link_bytes = decode_hex(text_at(&vectors, "/expected/link/signed_cbor_hex")?)?;
        let link = decode_and_verify_signed_history_recovery_link_v1(&link_bytes)?;
        assert_eq!(encode_signed_history_recovery_link_v1(&link)?, link_bytes);
        assert_eq!(
            history_recovery_link_signing_bytes_v1(&link.content)?,
            decode_hex(text_at(&vectors, "/expected/link/signing_preimage_hex")?)?
        );
        assert_eq!(
            history_recovery_link_id_v1(&link)?,
            fixed_hex(text_at(&vectors, "/expected/link/id_hex")?)?
        );
        assert_eq!(
            history_recovery_sas_v1(
                &link.content.account_device_list,
                link.content.source_certificate.content.device_id,
                link.content.recipient_device_id,
            )?,
            fixed_hex(text_at(&vectors, "/expected/sas_hex")?)?
        );

        let plan_bytes = decode_hex(text_at(&vectors, "/expected/plan/signed_cbor_hex")?)?;
        let plan = decode_and_verify_signed_history_recovery_plan_v1(&plan_bytes)?;
        assert_eq!(encode_signed_history_recovery_plan_v1(&plan)?, plan_bytes);
        assert_eq!(
            history_recovery_plan_signing_bytes_v1(&plan.content)?,
            decode_hex(text_at(&vectors, "/expected/plan/signing_preimage_hex")?)?
        );
        assert_eq!(
            history_recovery_plan_id_v1(&plan)?,
            fixed_hex(text_at(&vectors, "/expected/plan/id_hex")?)?
        );
        assert_eq!(
            history_recovery_id_for_plan_v1(&plan)?,
            fixed_hex(text_at(&vectors, "/expected/recovery_id_hex")?)?
        );

        let request_bytes = decode_hex(text_at(&vectors, "/expected/request/signed_cbor_hex")?)?;
        let request = decode_and_verify_signed_history_recovery_page_request_v1(&request_bytes)?;
        assert_eq!(
            encode_signed_history_recovery_page_request_v1(&request)?,
            request_bytes
        );
        assert_eq!(
            history_recovery_page_request_signing_bytes_v1(&request.content)?,
            decode_hex(text_at(&vectors, "/expected/request/signing_preimage_hex")?)?
        );
        let session = fixed_hex(text_at(&vectors, "/input/session_binding_hex")?)?;
        verify_history_recovery_page_request_for_plan_v1(&request, &plan, session)?;

        for pointer in [
            "/expected/page_response/signed_cbor_hex",
            "/expected/rejected_response/signed_cbor_hex",
        ] {
            let bytes = decode_hex(text_at(&vectors, pointer)?)?;
            let response = decode_and_verify_signed_history_recovery_response_v1(&bytes)?;
            assert_eq!(
                encode_signed_history_recovery_response_v1(&response)?,
                bytes
            );
            verify_history_recovery_response_for_plan_v1(&response, &plan, session)?;
        }
        Ok(())
    }

    #[test]
    fn recovery_exchange_plan_session_and_ranges_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let plan = decode_and_verify_signed_history_recovery_plan_v1(&decode_hex(text_at(
            &vectors,
            "/expected/plan/signed_cbor_hex",
        )?)?)?;
        let mut request = decode_and_verify_signed_history_recovery_page_request_v1(&decode_hex(
            text_at(&vectors, "/expected/request/signed_cbor_hex")?,
        )?)?;
        let recipient = signing_key(&vectors, "/input/recipient_device_secret_key_hex")?;
        let source = signing_key(&vectors, "/input/source_device_secret_key_hex")?;
        let session = fixed_hex(text_at(&vectors, "/input/session_binding_hex")?)?;

        assert!(matches!(
            verify_history_recovery_page_request_for_plan_v1(&request, &plan, [0x55; 32]),
            Err(CanonicalRecoveryExchangeError::SessionBindingMismatch)
        ));
        request.content.plan_id[0] ^= 1;
        resign_request(&mut request, &recipient)?;
        assert!(matches!(
            verify_history_recovery_page_request_for_plan_v1(&request, &plan, session),
            Err(CanonicalRecoveryExchangeError::PlanMismatch)
        ));

        request = decode_and_verify_signed_history_recovery_page_request_v1(&decode_hex(
            text_at(&vectors, "/expected/request/signed_cbor_hex")?,
        )?)?;
        request.content.range_start = plan.content.signed_link.content.approved_range_end - 1;
        resign_request(&mut request, &recipient)?;
        assert!(matches!(
            verify_history_recovery_page_request_for_plan_v1(&request, &plan, session),
            Err(CanonicalRecoveryExchangeError::RequestRangeOutsidePlan)
        ));

        let mut response = decode_and_verify_signed_history_recovery_response_v1(&decode_hex(
            text_at(&vectors, "/expected/page_response/signed_cbor_hex")?,
        )?)?;
        let CanonicalHistoryRecoveryResponseV1::Page(page) = &mut response.content else {
            return Err(io::Error::other("golden response is not a page").into());
        };
        page.signed_request.content.session_binding[0] ^= 1;
        resign_request(&mut page.signed_request, &recipient)?;
        resign_response(&mut response, &source)?;
        assert!(matches!(
            verify_history_recovery_response_for_plan_v1(&response, &plan, session),
            Err(CanonicalRecoveryExchangeError::SessionBindingMismatch)
        ));

        let mut link = plan.content.signed_link.clone();
        link.content.coordinates.reverse();
        assert!(matches!(
            encode_signed_history_recovery_link_v1(&link),
            Err(CanonicalRecoveryExchangeError::NonCanonicalCoordinateOrder)
        ));
        link = plan.content.signed_link;
        link.content.recipient_device_id = link.content.source_certificate.content.device_id;
        assert!(matches!(
            encode_signed_history_recovery_link_v1(&link),
            Err(CanonicalRecoveryExchangeError::SameDevice)
        ));
        Ok(())
    }

    #[test]
    fn malformed_recovery_exchange_variants_booleans_and_signatures_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let mut plan = decode_hex(text_at(&vectors, "/expected/plan/signed_cbor_hex")?)?;
        let policy = [0x86, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00];
        let offset = plan
            .windows(policy.len())
            .position(|window| window == policy)
            .ok_or_else(|| io::Error::other("execution policy not found"))?;
        plan[offset + 4] = 2;
        assert!(matches!(
            decode_signed_history_recovery_plan_v1(&plan),
            Err(CanonicalRecoveryExchangeError::InvalidBoolean { .. })
        ));

        let mut response = decode_hex(text_at(
            &vectors,
            "/expected/page_response/signed_cbor_hex",
        )?)?;
        let content_prefix = [0x83, 0x01, 0x01];
        let offset = response
            .windows(content_prefix.len())
            .position(|window| window == content_prefix)
            .ok_or_else(|| io::Error::other("response content not found"))?;
        response[offset + 2] = 9;
        assert!(matches!(
            decode_signed_history_recovery_response_v1(&response),
            Err(CanonicalRecoveryExchangeError::UnknownResponseVariant(9))
        ));

        let mut request = decode_hex(text_at(&vectors, "/expected/request/signed_cbor_hex")?)?;
        let last = request
            .last_mut()
            .ok_or_else(|| io::Error::other("empty signed request"))?;
        *last ^= 1;
        assert!(matches!(
            decode_and_verify_signed_history_recovery_page_request_v1(&request),
            Err(CanonicalRecoveryExchangeError::InvalidSignature { .. })
        ));
        Ok(())
    }
}
