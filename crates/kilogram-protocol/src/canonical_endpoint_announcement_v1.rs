//! Inactive deterministic-CBOR endpoint-announcement bundle candidate.
//!
//! This module covers the signed plaintext bundle and the portable accepted
//! observation carried by it. Recipient HPKE sealing and the session-bound
//! acknowledgement remain a separate follow-up. The active Postcard/HPKE
//! runtime is deliberately unchanged.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        decode_and_verify_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        verify_signed_account_device_list_v1,
    },
    canonical_publication_conflict_v1::{
        CanonicalPublicationConflictError, CanonicalSignedPublicationConflictProofV1,
        CanonicalSignedTicketPublicationObservationV1,
        decode_and_verify_signed_publication_conflict_proof_v1,
        decode_and_verify_signed_ticket_publication_observation_v1,
        encode_signed_publication_conflict_proof_v1,
        encode_signed_ticket_publication_observation_v1,
        verify_signed_publication_conflict_proof_v1,
        verify_signed_ticket_publication_observation_v1,
    },
    canonical_publication_resolution_v1::{
        CanonicalPublicationResolutionError, CanonicalRootSignedPublicationConflictResolutionV1,
        decode_and_verify_root_signed_publication_conflict_resolution_v1,
        encode_root_signed_publication_conflict_resolution_v1,
        publication_conflict_replacement_ticket_digest_v1,
        verify_root_signed_publication_conflict_resolution_v1,
    },
    canonical_ticket_publication_v1::ticket_publication_channel_id_v1,
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS: usize = 256;
pub const MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT: usize = 4;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_LABEL_BYTES: usize = 4096;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_TICKET_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS: u64 = 60 * 60;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CLOCK_SKEW_SECONDS: u64 = 5 * 60;

pub const CANONICAL_ENDPOINT_ANNOUNCEMENT_BUNDLE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:endpoint-announcement-bundle:cbor-v1\0";
pub const CANONICAL_ENDPOINT_ANNOUNCEMENT_BUNDLE_ID_DOMAIN: &[u8] =
    b"kilogram:endpoint-announcement-bundle-id:cbor-v1\0";
pub const CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:accepted-endpoint-observation:cbor-v1\0";
pub const CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_ID_DOMAIN: &[u8] =
    b"kilogram:accepted-endpoint-observation-id:cbor-v1\0";
pub const ENDPOINT_ANNOUNCEMENT_CONVERSATION_LABEL_DOMAIN: &[u8] =
    b"kilogram:conversation-label:v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalEndpointRoutePolicyV1 {
    Auto = 0,
    DirectOnly = 1,
    RelayOnly = 2,
}

impl CanonicalEndpointRoutePolicyV1 {
    pub const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalEndpointAnnouncementError> {
        match code {
            0 => Ok(Self::Auto),
            1 => Ok(Self::DirectOnly),
            2 => Ok(Self::RelayOnly),
            _ => Err(CanonicalEndpointAnnouncementError::InvalidRoutePolicy),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAcceptedEndpointObservationContentV1 {
    pub local_account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub source_bundle_id: [u8; 32],
    pub imported_at_unix_seconds: u64,
    pub source_observation: CanonicalSignedTicketPublicationObservationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAcceptedEndpointObservationV1 {
    pub content: CanonicalAcceptedEndpointObservationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalEndpointAnnouncementStateV1 {
    None,
    DirectObservation(Box<CanonicalSignedTicketPublicationObservationV1>),
    AcceptedObservation(Box<CanonicalSignedAcceptedEndpointObservationV1>),
    PublicationConflict(Box<CanonicalSignedPublicationConflictProofV1>),
    PublicationResolution(Box<CanonicalRootSignedPublicationConflictResolutionV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEndpointCandidateAnnouncementV1 {
    pub peer_device_id: [u8; 32],
    pub primary: bool,
    pub route_policy: CanonicalEndpointRoutePolicyV1,
    pub ticket_publication_write_key: [u8; 32],
    pub ticket: Vec<u8>,
    pub state: CanonicalEndpointAnnouncementStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalContactEndpointAnnouncementV1 {
    pub conversation_label: Vec<u8>,
    pub conversation_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub endpoints: Vec<CanonicalEndpointCandidateAnnouncementV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEndpointAnnouncementBundleContentV1 {
    pub account_device_list: CanonicalSignedAccountDeviceListV1,
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub contacts: Vec<CanonicalContactEndpointAnnouncementV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedEndpointAnnouncementBundleV1 {
    pub content: CanonicalEndpointAnnouncementBundleContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalEndpointAnnouncementError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Publication(#[from] CanonicalPublicationConflictError),
    #[error(transparent)]
    Resolution(#[from] CanonicalPublicationResolutionError),
    #[error("canonical endpoint-announcement {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical endpoint-announcement {object} contains an invalid Ed25519 key")]
    InvalidPublicKey { object: &'static str },
    #[error("canonical endpoint-announcement {object} contains an invalid signature")]
    InvalidSignature { object: &'static str },
    #[error("canonical endpoint-announcement route policy is unknown")]
    InvalidRoutePolicy,
    #[error("canonical accepted endpoint observation is invalid")]
    InvalidAcceptedObservation,
    #[error("canonical endpoint-announcement state variant is invalid")]
    InvalidState,
    #[error("canonical endpoint-announcement candidate is invalid")]
    InvalidCandidate,
    #[error("canonical endpoint-announcement contact is invalid")]
    InvalidContact,
    #[error("canonical endpoint-announcement bundle is invalid")]
    InvalidBundle,
    #[error("canonical endpoint-announcement label is not valid UTF-8")]
    InvalidConversationLabel,
    #[error("canonical endpoint-announcement is not valid at the supplied time")]
    BundleNotCurrent,
}

#[allow(clippy::too_many_arguments)]
pub fn sign_accepted_endpoint_observation_v1(
    signing_key: &SigningKey,
    local_account_id: [u8; 32],
    source_device_id: [u8; 32],
    source_bundle_id: [u8; 32],
    imported_at_unix_seconds: u64,
    source_observation: CanonicalSignedTicketPublicationObservationV1,
) -> Result<CanonicalSignedAcceptedEndpointObservationV1, CanonicalEndpointAnnouncementError> {
    let content = CanonicalAcceptedEndpointObservationContentV1 {
        local_account_id,
        local_device_id: signing_key.verifying_key().to_bytes(),
        source_device_id,
        source_bundle_id,
        imported_at_unix_seconds,
        source_observation,
    };
    let signature = signing_key
        .sign(&accepted_endpoint_observation_signing_bytes_v1(&content)?)
        .to_bytes();
    let evidence = CanonicalSignedAcceptedEndpointObservationV1 { content, signature };
    verify_signed_accepted_endpoint_observation_v1(&evidence)?;
    Ok(evidence)
}

pub fn encode_accepted_endpoint_observation_content_v1(
    content: &CanonicalAcceptedEndpointObservationContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    validate_accepted_observation_content(content)?;
    let observation = encode_signed_ticket_publication_observation_v1(&content.source_observation)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.source_device_id);
    push_bytes(&mut output, &content.source_bundle_id);
    push_unsigned(&mut output, content.imported_at_unix_seconds);
    push_bytes(&mut output, &observation);
    Ok(output)
}

pub fn accepted_endpoint_observation_signing_bytes_v1(
    content: &CanonicalAcceptedEndpointObservationContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    Ok(prefixed(
        CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_SIGNATURE_DOMAIN,
        &encode_accepted_endpoint_observation_content_v1(content)?,
    ))
}

pub fn encode_signed_accepted_endpoint_observation_v1(
    evidence: &CanonicalSignedAcceptedEndpointObservationV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    verify_signed_accepted_endpoint_observation_v1(evidence)?;
    let content = encode_accepted_endpoint_observation_content_v1(&evidence.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &evidence.signature);
    ensure_bound(
        &output,
        "signed accepted observation",
        MAX_CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_accepted_endpoint_observation_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAcceptedEndpointObservationV1, CanonicalEndpointAnnouncementError> {
    ensure_bound(
        bytes,
        "signed accepted observation",
        MAX_CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-accepted-endpoint-observation-v1", 3)?;
    decoder.expect_version("signed-accepted-endpoint-observation-v1.schema_version")?;
    let content = decode_accepted_endpoint_observation_content_v1(
        decoder.bytes("signed-accepted-endpoint-observation-v1.content")?,
    )?;
    let signature = decoder.fixed_bytes("signed-accepted-endpoint-observation-v1.signature")?;
    decoder.finish()?;
    let evidence = CanonicalSignedAcceptedEndpointObservationV1 { content, signature };
    verify_signed_accepted_endpoint_observation_v1(&evidence)?;
    if encode_signed_accepted_endpoint_observation_v1(&evidence)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(evidence)
}

pub fn verify_signed_accepted_endpoint_observation_v1(
    evidence: &CanonicalSignedAcceptedEndpointObservationV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    validate_accepted_observation_content(&evidence.content)?;
    verify_ed25519(
        &evidence.content.local_device_id,
        &accepted_endpoint_observation_signing_bytes_v1(&evidence.content)?,
        &evidence.signature,
        "signed-accepted-endpoint-observation-v1",
    )
}

pub fn accepted_endpoint_observation_id_v1(
    evidence: &CanonicalSignedAcceptedEndpointObservationV1,
) -> Result<[u8; 32], CanonicalEndpointAnnouncementError> {
    Ok(domain_hash(
        CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_ID_DOMAIN,
        &encode_signed_accepted_endpoint_observation_v1(evidence)?,
    ))
}

pub fn endpoint_announcement_conversation_id_v1(
    label: &[u8],
) -> Result<[u8; 32], CanonicalEndpointAnnouncementError> {
    validate_label(label)?;
    Ok(domain_hash(
        ENDPOINT_ANNOUNCEMENT_CONVERSATION_LABEL_DOMAIN,
        label,
    ))
}

pub fn encode_endpoint_candidate_announcement_v1(
    candidate: &CanonicalEndpointCandidateAnnouncementV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    validate_candidate(candidate)?;
    let state = encode_endpoint_announcement_state_v1(&candidate.state)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &candidate.peer_device_id);
    push_unsigned(&mut output, u64::from(candidate.primary));
    push_unsigned(&mut output, candidate.route_policy.code());
    push_bytes(&mut output, &candidate.ticket_publication_write_key);
    push_bytes(&mut output, &candidate.ticket);
    push_bytes(&mut output, &state);
    Ok(output)
}

pub fn decode_and_verify_endpoint_candidate_announcement_v1(
    bytes: &[u8],
) -> Result<CanonicalEndpointCandidateAnnouncementV1, CanonicalEndpointAnnouncementError> {
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("endpoint-candidate-announcement-v1", 7)?;
    decoder.expect_version("endpoint-candidate-announcement-v1.schema_version")?;
    let peer_device_id =
        decoder.fixed_bytes("endpoint-candidate-announcement-v1.peer_device_id")?;
    let primary = decode_boolean(decoder.unsigned("endpoint-candidate-announcement-v1.primary")?)?;
    let route_policy = CanonicalEndpointRoutePolicyV1::from_code(
        decoder.unsigned("endpoint-candidate-announcement-v1.route_policy")?,
    )?;
    let ticket_publication_write_key =
        decoder.fixed_bytes("endpoint-candidate-announcement-v1.write_key")?;
    let ticket = decoder
        .bytes("endpoint-candidate-announcement-v1.ticket")?
        .to_vec();
    let state = decode_endpoint_announcement_state_v1(
        decoder.bytes("endpoint-candidate-announcement-v1.state")?,
    )?;
    decoder.finish()?;
    let candidate = CanonicalEndpointCandidateAnnouncementV1 {
        peer_device_id,
        primary,
        route_policy,
        ticket_publication_write_key,
        ticket,
        state,
    };
    validate_candidate(&candidate)?;
    if encode_endpoint_candidate_announcement_v1(&candidate)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(candidate)
}

pub fn encode_contact_endpoint_announcement_v1(
    contact: &CanonicalContactEndpointAnnouncementV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    validate_contact(contact)?;
    let mut endpoints = Vec::with_capacity(contact.endpoints.len());
    for endpoint in &contact.endpoints {
        endpoints.push(encode_endpoint_candidate_announcement_v1(endpoint)?);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &contact.conversation_label);
    push_bytes(&mut output, &contact.conversation_id);
    push_bytes(&mut output, &contact.peer_account_id);
    push_array_header(&mut output, endpoints.len() as u64);
    for endpoint in endpoints {
        push_bytes(&mut output, &endpoint);
    }
    Ok(output)
}

pub fn decode_and_verify_contact_endpoint_announcement_v1(
    bytes: &[u8],
) -> Result<CanonicalContactEndpointAnnouncementV1, CanonicalEndpointAnnouncementError> {
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("contact-endpoint-announcement-v1", 5)?;
    decoder.expect_version("contact-endpoint-announcement-v1.schema_version")?;
    let conversation_label = decoder
        .bytes("contact-endpoint-announcement-v1.conversation_label")?
        .to_vec();
    let conversation_id =
        decoder.fixed_bytes("contact-endpoint-announcement-v1.conversation_id")?;
    let peer_account_id =
        decoder.fixed_bytes("contact-endpoint-announcement-v1.peer_account_id")?;
    let count = decoder.array("contact-endpoint-announcement-v1.endpoints")?;
    if count > MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT {
        return Err(CanonicalWireError::TooManyItems {
            field: "endpoint-announcement endpoints",
            actual: count,
            maximum: MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT,
        }
        .into());
    }
    let mut endpoints = Vec::with_capacity(count);
    for _ in 0..count {
        endpoints.push(decode_and_verify_endpoint_candidate_announcement_v1(
            decoder.bytes("contact-endpoint-announcement-v1.endpoint")?,
        )?);
    }
    decoder.finish()?;
    let contact = CanonicalContactEndpointAnnouncementV1 {
        conversation_label,
        conversation_id,
        peer_account_id,
        endpoints,
    };
    validate_contact(&contact)?;
    if encode_contact_endpoint_announcement_v1(&contact)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(contact)
}

pub fn sign_endpoint_announcement_bundle_v1(
    signing_key: &SigningKey,
    account_device_list: CanonicalSignedAccountDeviceListV1,
    recipient_device_id: [u8; 32],
    created_at_unix_seconds: u64,
    validity_seconds: u64,
    mut contacts: Vec<CanonicalContactEndpointAnnouncementV1>,
) -> Result<CanonicalSignedEndpointAnnouncementBundleV1, CanonicalEndpointAnnouncementError> {
    if !(1..=MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS).contains(&validity_seconds) {
        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
    }
    contacts.sort_by_key(contact_sort_key);
    let content = CanonicalEndpointAnnouncementBundleContentV1 {
        account_device_list,
        source_device_id: signing_key.verifying_key().to_bytes(),
        recipient_device_id,
        created_at_unix_seconds,
        expires_at_unix_seconds: created_at_unix_seconds
            .checked_add(validity_seconds)
            .ok_or(CanonicalEndpointAnnouncementError::InvalidBundle)?,
        contacts,
    };
    let signature = signing_key
        .sign(&endpoint_announcement_bundle_signing_bytes_v1(&content)?)
        .to_bytes();
    let bundle = CanonicalSignedEndpointAnnouncementBundleV1 { content, signature };
    verify_signed_endpoint_announcement_bundle_v1(&bundle)?;
    Ok(bundle)
}

pub fn encode_endpoint_announcement_bundle_content_v1(
    content: &CanonicalEndpointAnnouncementBundleContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    validate_bundle_content(content)?;
    let device_list = encode_signed_account_device_list_v1(&content.account_device_list)?;
    let mut contacts = Vec::with_capacity(content.contacts.len());
    for contact in &content.contacts {
        contacts.push(encode_contact_endpoint_announcement_v1(contact)?);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &device_list);
    push_bytes(&mut output, &content.source_device_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_unsigned(&mut output, content.created_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    push_array_header(&mut output, contacts.len() as u64);
    for contact in contacts {
        push_bytes(&mut output, &contact);
    }
    ensure_bound(
        &output,
        "bundle content",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES,
    )?;
    Ok(output)
}

pub fn endpoint_announcement_bundle_signing_bytes_v1(
    content: &CanonicalEndpointAnnouncementBundleContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    Ok(prefixed(
        CANONICAL_ENDPOINT_ANNOUNCEMENT_BUNDLE_SIGNATURE_DOMAIN,
        &encode_endpoint_announcement_bundle_content_v1(content)?,
    ))
}

pub fn encode_signed_endpoint_announcement_bundle_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    let content = encode_endpoint_announcement_bundle_content_v1(&bundle.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &bundle.signature);
    ensure_bound(
        &output,
        "signed bundle",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_endpoint_announcement_bundle_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedEndpointAnnouncementBundleV1, CanonicalEndpointAnnouncementError> {
    ensure_bound(
        bytes,
        "signed bundle",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES,
    )?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-endpoint-announcement-bundle-v1", 3)?;
    decoder.expect_version("signed-endpoint-announcement-bundle-v1.schema_version")?;
    let content = decode_endpoint_announcement_bundle_content_v1(
        decoder.bytes("signed-endpoint-announcement-bundle-v1.content")?,
    )?;
    let signature = decoder.fixed_bytes("signed-endpoint-announcement-bundle-v1.signature")?;
    decoder.finish()?;
    let bundle = CanonicalSignedEndpointAnnouncementBundleV1 { content, signature };
    verify_signed_endpoint_announcement_bundle_v1(&bundle)?;
    if encode_signed_endpoint_announcement_bundle_v1(&bundle)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(bundle)
}

pub fn verify_signed_endpoint_announcement_bundle_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    validate_bundle_content(&bundle.content)?;
    verify_ed25519(
        &bundle.content.source_device_id,
        &endpoint_announcement_bundle_signing_bytes_v1(&bundle.content)?,
        &bundle.signature,
        "signed-endpoint-announcement-bundle-v1",
    )
}

pub fn verify_endpoint_announcement_bundle_at_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    if bundle.content.created_at_unix_seconds
        > now_unix_seconds.saturating_add(MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CLOCK_SKEW_SECONDS)
        || now_unix_seconds > bundle.content.expires_at_unix_seconds
    {
        return Err(CanonicalEndpointAnnouncementError::BundleNotCurrent);
    }
    Ok(())
}

pub fn endpoint_announcement_bundle_id_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<[u8; 32], CanonicalEndpointAnnouncementError> {
    Ok(domain_hash(
        CANONICAL_ENDPOINT_ANNOUNCEMENT_BUNDLE_ID_DOMAIN,
        &encode_signed_endpoint_announcement_bundle_v1(bundle)?,
    ))
}

fn encode_endpoint_announcement_state_v1(
    state: &CanonicalEndpointAnnouncementStateV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementError> {
    let mut output = Vec::new();
    match state {
        CanonicalEndpointAnnouncementStateV1::None => {
            push_array_header(&mut output, 2);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 0);
        }
        CanonicalEndpointAnnouncementStateV1::DirectObservation(observation) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 1);
            push_bytes(
                &mut output,
                &encode_signed_ticket_publication_observation_v1(observation)?,
            );
        }
        CanonicalEndpointAnnouncementStateV1::AcceptedObservation(evidence) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 2);
            push_bytes(
                &mut output,
                &encode_signed_accepted_endpoint_observation_v1(evidence)?,
            );
        }
        CanonicalEndpointAnnouncementStateV1::PublicationConflict(proof) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 3);
            push_bytes(
                &mut output,
                &encode_signed_publication_conflict_proof_v1(proof)?,
            );
        }
        CanonicalEndpointAnnouncementStateV1::PublicationResolution(resolution) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 4);
            push_bytes(
                &mut output,
                &encode_root_signed_publication_conflict_resolution_v1(resolution)?,
            );
        }
    }
    Ok(output)
}

fn decode_endpoint_announcement_state_v1(
    bytes: &[u8],
) -> Result<CanonicalEndpointAnnouncementStateV1, CanonicalEndpointAnnouncementError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let length = decoder.array("endpoint-announcement-state-v1")?;
    decoder.expect_version("endpoint-announcement-state-v1.schema_version")?;
    let kind = decoder.unsigned("endpoint-announcement-state-v1.kind")?;
    let state = match (kind, length) {
        (0, 2) => CanonicalEndpointAnnouncementStateV1::None,
        (1, 3) => CanonicalEndpointAnnouncementStateV1::DirectObservation(Box::new(
            decode_and_verify_signed_ticket_publication_observation_v1(
                decoder.bytes("endpoint-announcement-state-v1.observation")?,
            )?,
        )),
        (2, 3) => CanonicalEndpointAnnouncementStateV1::AcceptedObservation(Box::new(
            decode_and_verify_signed_accepted_endpoint_observation_v1(
                decoder.bytes("endpoint-announcement-state-v1.accepted_observation")?,
            )?,
        )),
        (3, 3) => CanonicalEndpointAnnouncementStateV1::PublicationConflict(Box::new(
            decode_and_verify_signed_publication_conflict_proof_v1(
                decoder.bytes("endpoint-announcement-state-v1.conflict")?,
            )?,
        )),
        (4, 3) => CanonicalEndpointAnnouncementStateV1::PublicationResolution(Box::new(
            decode_and_verify_root_signed_publication_conflict_resolution_v1(
                decoder.bytes("endpoint-announcement-state-v1.resolution")?,
            )?,
        )),
        _ => return Err(CanonicalEndpointAnnouncementError::InvalidState),
    };
    decoder.finish()?;
    if encode_endpoint_announcement_state_v1(&state)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(state)
}

fn decode_accepted_endpoint_observation_content_v1(
    bytes: &[u8],
) -> Result<CanonicalAcceptedEndpointObservationContentV1, CanonicalEndpointAnnouncementError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("accepted-endpoint-observation-content-v1", 7)?;
    decoder.expect_version("accepted-endpoint-observation-content-v1.schema_version")?;
    let content = CanonicalAcceptedEndpointObservationContentV1 {
        local_account_id: decoder
            .fixed_bytes("accepted-endpoint-observation-content-v1.local_account_id")?,
        local_device_id: decoder
            .fixed_bytes("accepted-endpoint-observation-content-v1.local_device_id")?,
        source_device_id: decoder
            .fixed_bytes("accepted-endpoint-observation-content-v1.source_device_id")?,
        source_bundle_id: decoder
            .fixed_bytes("accepted-endpoint-observation-content-v1.source_bundle_id")?,
        imported_at_unix_seconds: decoder
            .unsigned("accepted-endpoint-observation-content-v1.imported_at_unix_seconds")?,
        source_observation: decode_and_verify_signed_ticket_publication_observation_v1(
            decoder.bytes("accepted-endpoint-observation-content-v1.source_observation")?,
        )?,
    };
    decoder.finish()?;
    validate_accepted_observation_content(&content)?;
    if encode_accepted_endpoint_observation_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn decode_endpoint_announcement_bundle_content_v1(
    bytes: &[u8],
) -> Result<CanonicalEndpointAnnouncementBundleContentV1, CanonicalEndpointAnnouncementError> {
    ensure_bound(
        bytes,
        "bundle content",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES,
    )?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("endpoint-announcement-bundle-content-v1", 7)?;
    decoder.expect_version("endpoint-announcement-bundle-content-v1.schema_version")?;
    let account_device_list = decode_and_verify_signed_account_device_list_v1(
        decoder.bytes("endpoint-announcement-bundle-content-v1.account_device_list")?,
    )?;
    let source_device_id =
        decoder.fixed_bytes("endpoint-announcement-bundle-content-v1.source_device_id")?;
    let recipient_device_id =
        decoder.fixed_bytes("endpoint-announcement-bundle-content-v1.recipient_device_id")?;
    let created_at_unix_seconds =
        decoder.unsigned("endpoint-announcement-bundle-content-v1.created_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("endpoint-announcement-bundle-content-v1.expires_at_unix_seconds")?;
    let count = decoder.array("endpoint-announcement-bundle-content-v1.contacts")?;
    if count > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS {
        return Err(CanonicalWireError::TooManyItems {
            field: "endpoint-announcement contacts",
            actual: count,
            maximum: MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS,
        }
        .into());
    }
    let mut contacts = Vec::with_capacity(count);
    for _ in 0..count {
        contacts.push(decode_and_verify_contact_endpoint_announcement_v1(
            decoder.bytes("endpoint-announcement-bundle-content-v1.contact")?,
        )?);
    }
    decoder.finish()?;
    let content = CanonicalEndpointAnnouncementBundleContentV1 {
        account_device_list,
        source_device_id,
        recipient_device_id,
        created_at_unix_seconds,
        expires_at_unix_seconds,
        contacts,
    };
    validate_bundle_content(&content)?;
    if encode_endpoint_announcement_bundle_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn validate_accepted_observation_content(
    content: &CanonicalAcceptedEndpointObservationContentV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    verify_key(&content.local_account_id, "accepted observation Account ID")?;
    verify_key(&content.local_device_id, "accepted observation Device ID")?;
    verify_key(
        &content.source_device_id,
        "accepted observation source Device ID",
    )?;
    verify_signed_ticket_publication_observation_v1(&content.source_observation)?;
    if content.imported_at_unix_seconds == 0
        || content.local_device_id == content.source_device_id
        || content.source_observation.content.local_account_id != content.local_account_id
        || content.source_observation.content.local_device_id != content.source_device_id
    {
        return Err(CanonicalEndpointAnnouncementError::InvalidAcceptedObservation);
    }
    Ok(())
}

fn validate_candidate(
    candidate: &CanonicalEndpointCandidateAnnouncementV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    verify_key(&candidate.peer_device_id, "endpoint peer Device ID")?;
    verify_key(
        &candidate.ticket_publication_write_key,
        "endpoint publication write key",
    )?;
    if candidate.ticket.is_empty()
        || candidate.ticket.len() > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_TICKET_BYTES
        || std::str::from_utf8(&candidate.ticket).is_err()
    {
        return Err(CanonicalEndpointAnnouncementError::InvalidCandidate);
    }
    let channel_id = ticket_publication_channel_id_v1(&candidate.ticket_publication_write_key);
    match &candidate.state {
        CanonicalEndpointAnnouncementStateV1::None => {}
        CanonicalEndpointAnnouncementStateV1::DirectObservation(observation) => {
            verify_signed_ticket_publication_observation_v1(observation)?;
            if observation.content.channel_id != channel_id
                || observation.content.publisher_device_id != candidate.peer_device_id
            {
                return Err(CanonicalEndpointAnnouncementError::InvalidCandidate);
            }
        }
        CanonicalEndpointAnnouncementStateV1::AcceptedObservation(evidence) => {
            verify_signed_accepted_endpoint_observation_v1(evidence)?;
            let observation = &evidence.content.source_observation;
            if observation.content.channel_id != channel_id
                || observation.content.publisher_device_id != candidate.peer_device_id
            {
                return Err(CanonicalEndpointAnnouncementError::InvalidCandidate);
            }
        }
        CanonicalEndpointAnnouncementStateV1::PublicationConflict(proof) => {
            verify_signed_publication_conflict_proof_v1(proof)?;
            if proof.content.channel_id != channel_id
                || proof.content.first_observation.content.publisher_device_id
                    != candidate.peer_device_id
            {
                return Err(CanonicalEndpointAnnouncementError::InvalidCandidate);
            }
        }
        CanonicalEndpointAnnouncementStateV1::PublicationResolution(resolution) => {
            verify_root_signed_publication_conflict_resolution_v1(resolution)?;
            if resolution.content.peer_device_id != candidate.peer_device_id
                || resolution.content.new_write_key != candidate.ticket_publication_write_key
                || resolution.content.route_policy.code() != candidate.route_policy.code()
                || publication_conflict_replacement_ticket_digest_v1(&candidate.ticket)
                    != resolution.content.replacement_ticket_digest
            {
                return Err(CanonicalEndpointAnnouncementError::InvalidCandidate);
            }
        }
    }
    Ok(())
}

fn validate_contact(
    contact: &CanonicalContactEndpointAnnouncementV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    validate_label(&contact.conversation_label)?;
    verify_key(&contact.peer_account_id, "endpoint peer Account ID")?;
    if contact.conversation_id
        != endpoint_announcement_conversation_id_v1(&contact.conversation_label)?
        || contact.endpoints.is_empty()
        || contact.endpoints.len() > MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT
        || contact
            .endpoints
            .iter()
            .filter(|value| value.primary)
            .count()
            != 1
        || contact
            .endpoints
            .windows(2)
            .any(|pair| pair[0].peer_device_id >= pair[1].peer_device_id)
    {
        return Err(CanonicalEndpointAnnouncementError::InvalidContact);
    }
    for endpoint in &contact.endpoints {
        validate_candidate(endpoint)?;
        let publisher_account_id = match &endpoint.state {
            CanonicalEndpointAnnouncementStateV1::None => None,
            CanonicalEndpointAnnouncementStateV1::DirectObservation(observation) => {
                Some(observation.content.publisher_account_id)
            }
            CanonicalEndpointAnnouncementStateV1::AcceptedObservation(evidence) => Some(
                evidence
                    .content
                    .source_observation
                    .content
                    .publisher_account_id,
            ),
            CanonicalEndpointAnnouncementStateV1::PublicationConflict(proof) => {
                Some(proof.content.first_observation.content.publisher_account_id)
            }
            CanonicalEndpointAnnouncementStateV1::PublicationResolution(resolution) => {
                Some(resolution.content.peer_account_id)
            }
        };
        if publisher_account_id.is_some_and(|value| value != contact.peer_account_id) {
            return Err(CanonicalEndpointAnnouncementError::InvalidContact);
        }
    }
    Ok(())
}

fn validate_bundle_content(
    content: &CanonicalEndpointAnnouncementBundleContentV1,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    verify_signed_account_device_list_v1(&content.account_device_list)?;
    if content.source_device_id == content.recipient_device_id
        || content.contacts.len() > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS
        || content.expires_at_unix_seconds <= content.created_at_unix_seconds
        || content
            .expires_at_unix_seconds
            .saturating_sub(content.created_at_unix_seconds)
            > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS
        || content
            .contacts
            .windows(2)
            .any(|pair| contact_sort_key(&pair[0]) >= contact_sort_key(&pair[1]))
    {
        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
    }
    let has_device = |device_id: [u8; 32]| {
        content
            .account_device_list
            .content
            .devices
            .iter()
            .any(|certificate| certificate.content.device_id == device_id)
    };
    if !has_device(content.source_device_id) || !has_device(content.recipient_device_id) {
        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
    }
    let authority = &content
        .account_device_list
        .content
        .authority_snapshot
        .content;
    for contact in &content.contacts {
        validate_contact(contact)?;
        for endpoint in &contact.endpoints {
            match &endpoint.state {
                CanonicalEndpointAnnouncementStateV1::None => {}
                CanonicalEndpointAnnouncementStateV1::DirectObservation(observation) => {
                    if observation.content.local_account_id != authority.account_id
                        || observation.content.local_device_id != content.source_device_id
                    {
                        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
                    }
                }
                CanonicalEndpointAnnouncementStateV1::AcceptedObservation(evidence) => {
                    if evidence.content.local_account_id != authority.account_id
                        || evidence.content.local_device_id != content.source_device_id
                    {
                        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
                    }
                }
                CanonicalEndpointAnnouncementStateV1::PublicationConflict(proof) => {
                    if proof.content.local_account_id != authority.account_id
                        || !has_device(proof.content.detector_device_id)
                    {
                        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
                    }
                }
                CanonicalEndpointAnnouncementStateV1::PublicationResolution(resolution) => {
                    if resolution.content.local_account_id != authority.account_id
                        || resolution.content.authority_revision != authority.revision
                    {
                        return Err(CanonicalEndpointAnnouncementError::InvalidBundle);
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_label(label: &[u8]) -> Result<(), CanonicalEndpointAnnouncementError> {
    if label.is_empty() || label.len() > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_LABEL_BYTES {
        return Err(CanonicalEndpointAnnouncementError::InvalidContact);
    }
    std::str::from_utf8(label)
        .map_err(|_| CanonicalEndpointAnnouncementError::InvalidConversationLabel)?;
    Ok(())
}

fn decode_boolean(value: u64) -> Result<bool, CanonicalEndpointAnnouncementError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CanonicalEndpointAnnouncementError::InvalidCandidate),
    }
}

fn contact_sort_key(contact: &CanonicalContactEndpointAnnouncementV1) -> ([u8; 32], [u8; 32]) {
    (contact.peer_account_id, contact.conversation_id)
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    if bytes.len() > maximum {
        return Err(CanonicalEndpointAnnouncementError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_key(
    key: &[u8; 32],
    object: &'static str,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    VerifyingKey::from_bytes(key)
        .map(|_| ())
        .map_err(|_| CanonicalEndpointAnnouncementError::InvalidPublicKey { object })
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalEndpointAnnouncementError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalEndpointAnnouncementError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalEndpointAnnouncementError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalEndpointAnnouncementError::InvalidSignature { object })
}

fn domain_hash(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(value);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EndpointAnnouncementVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: serde_json::Value,
        dependencies: Vec<String>,
        expected: Expected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        accepted_observation: SignedExpected,
        signed_bundle: BundleExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SignedExpected {
        #[serde(rename = "content_cbor_hex")]
        _content_cbor_hex: String,
        #[serde(rename = "signing_preimage_hex")]
        _signing_preimage_hex: String,
        #[serde(rename = "signature_hex")]
        _signature_hex: String,
        signed_cbor_hex: String,
        object_id_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct BundleExpected {
        #[serde(rename = "content_cbor_hex")]
        _content_cbor_hex: String,
        #[serde(rename = "signing_preimage_hex")]
        _signing_preimage_hex: String,
        #[serde(rename = "signature_hex")]
        _signature_hex: String,
        signed_cbor_hex: String,
        #[serde(rename = "object_id_hex")]
        bundle_id_hex: String,
        valid_at_unix_seconds: u64,
    }

    #[test]
    fn endpoint_announcement_vectors_decode_verify_and_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors: EndpointAnnouncementVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
        )))?;
        assert_eq!(vectors.schema_version, 1);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        assert!(vectors.input.is_object());
        assert_eq!(
            vectors.dependencies,
            ["CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"]
        );

        let accepted_bytes = decode_hex(&vectors.expected.accepted_observation.signed_cbor_hex)?;
        let accepted = decode_and_verify_signed_accepted_endpoint_observation_v1(&accepted_bytes)?;
        assert_eq!(
            encode_signed_accepted_endpoint_observation_v1(&accepted)?,
            accepted_bytes
        );
        assert_eq!(
            encode_hex(&accepted_endpoint_observation_id_v1(&accepted)?),
            vectors.expected.accepted_observation.object_id_hex
        );

        let bundle_bytes = decode_hex(&vectors.expected.signed_bundle.signed_cbor_hex)?;
        let bundle = decode_and_verify_signed_endpoint_announcement_bundle_v1(&bundle_bytes)?;
        assert_eq!(
            encode_signed_endpoint_announcement_bundle_v1(&bundle)?,
            bundle_bytes
        );
        assert_eq!(
            encode_hex(&endpoint_announcement_bundle_id_v1(&bundle)?),
            vectors.expected.signed_bundle.bundle_id_hex
        );
        verify_endpoint_announcement_bundle_at_v1(
            &bundle,
            vectors.expected.signed_bundle.valid_at_unix_seconds,
        )?;
        assert!(
            verify_endpoint_announcement_bundle_at_v1(
                &bundle,
                bundle.content.expires_at_unix_seconds + 1,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn endpoint_announcement_rejects_tampering_and_wrong_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors: EndpointAnnouncementVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
        )))?;
        let bundle_bytes = decode_hex(&vectors.expected.signed_bundle.signed_cbor_hex)?;
        let mut bundle = decode_and_verify_signed_endpoint_announcement_bundle_v1(&bundle_bytes)?;
        bundle.content.recipient_device_id = bundle.content.source_device_id;
        assert!(verify_signed_endpoint_announcement_bundle_v1(&bundle).is_err());

        let mut tampered = bundle_bytes;
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(decode_and_verify_signed_endpoint_announcement_bundle_v1(&tampered).is_err());
        Ok(())
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex value has odd length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn encode_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(output, "{byte:02x}");
        }
        output
    }
}
