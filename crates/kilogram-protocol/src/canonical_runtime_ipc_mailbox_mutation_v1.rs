//! Inactive deterministic-CBOR mailbox-capability mutations for local runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. Requests carry exact
//! Device-signed provider-selection evidence and, for rotation or revocation,
//! the exact acknowledged predecessor head. Responses expose only signed
//! capability updates and never serialize local mailbox bindings, read/write
//! secrets, or decrypted peer offers.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_mailbox_provisioning_v1::{
        CanonicalMailboxCapabilityActionV1, CanonicalMailboxProvisioningModeV1,
        CanonicalSignedMailboxCapabilityAcknowledgementV1,
        CanonicalSignedMailboxCapabilityUpdateV1, MAX_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS,
        MAX_CANONICAL_MAILBOX_REPLICA_STORES, MIN_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS,
        MIN_CANONICAL_MAILBOX_REPLICA_STORES,
        decode_and_verify_signed_mailbox_capability_acknowledgement_v1,
        decode_and_verify_signed_mailbox_capability_update_v1,
        encode_signed_mailbox_capability_acknowledgement_v1,
        encode_signed_mailbox_capability_update_v1,
        verify_mailbox_capability_acknowledgement_for_v1,
        verify_mailbox_capability_update_chain_link_v1, verify_signed_mailbox_capability_update_v1,
    },
    canonical_mailbox_v1::verify_mailbox_storage_offer_at_v1,
    canonical_runtime_ipc_mailbox_status_v1::CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    canonical_runtime_ipc_provider_v1::{
        CanonicalRuntimeIpcProviderResultV1, CanonicalRuntimeIpcVolunteerProviderV1,
        CanonicalSignedRuntimeIpcProviderResponseV1,
        decode_and_verify_signed_runtime_ipc_provider_response_v1,
        encode_signed_runtime_ipc_provider_response_v1, runtime_ipc_provider_response_id_v1,
        verify_signed_runtime_ipc_provider_response_v1,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_BYTES: usize = 512 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_BYTES: usize = 128 * 1024;

pub const CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-mutation-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-mutation-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-mutation-response-id:cbor-v1\0";

const COMMAND_CREATE: u64 = 0;
const COMMAND_ROTATE: u64 = 1;
const COMMAND_REVOKE: u64 = 2;
const RESULT_TRANSITION: u64 = 0;
const RESULT_ERROR: u64 = 1;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const DELIVERY_PENDING_RECIPIENT_ACKNOWLEDGEMENT: u64 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxMutationTargetV1 {
    pub contact_id: [u8; 32],
    pub recipient_account_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub recipient_device_list_digest: [u8; 32],
    pub scope: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxMutationCommandV1 {
    CreateMailboxCapability {
        target: CanonicalRuntimeIpcMailboxMutationTargetV1,
        provider_selection: CanonicalSignedRuntimeIpcProviderResponseV1,
        offer_valid_for_seconds: u64,
    },
    RotateMailboxCapability {
        target: CanonicalRuntimeIpcMailboxMutationTargetV1,
        previous_head: Box<CanonicalRuntimeIpcMailboxCapabilityHeadV1>,
        provider_selection: CanonicalSignedRuntimeIpcProviderResponseV1,
        offer_valid_for_seconds: u64,
    },
    RevokeMailboxCapability {
        target: CanonicalRuntimeIpcMailboxMutationTargetV1,
        previous_head: Box<CanonicalRuntimeIpcMailboxCapabilityHeadV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxMutationRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcMailboxMutationCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxMutationOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxMutationDeliveryStateV1 {
    PendingRecipientAcknowledgement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxCapabilityTransitionV1 {
    pub outcome: CanonicalRuntimeIpcMailboxMutationOutcomeV1,
    pub target: CanonicalRuntimeIpcMailboxMutationTargetV1,
    pub update: CanonicalSignedMailboxCapabilityUpdateV1,
    pub provider_selection_id: Option<[u8; 32]>,
    pub offer_expires_at_unix_seconds: Option<u64>,
    pub delivery_state: CanonicalRuntimeIpcMailboxMutationDeliveryStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxMutationResultV1 {
    MailboxCapabilityTransition(Box<CanonicalRuntimeIpcMailboxCapabilityTransitionV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxMutationResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcMailboxMutationResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcMailboxMutationResponseV1 {
    pub content: CanonicalRuntimeIpcMailboxMutationResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_mailbox_mutation_request_v1(
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_command(&mut output, &request.command)?;
    ensure_bound(
        &output,
        "mailbox mutation request",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcMailboxMutationRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "mailbox mutation request",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-mailbox-mutation-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-mailbox-mutation-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcMailboxMutationRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-mailbox-mutation-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-mailbox-mutation-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-mailbox-mutation-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_mailbox_mutation_request_digest_v1(
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_mailbox_mutation_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_request(request)?;
    if request.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || request.bearer_token != descriptor.content.bearer_token
        || request.issued_at_unix_seconds < descriptor.content.issued_at_unix_seconds
        || request.expires_at_unix_seconds > descriptor.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if request.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS)
        || request.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::RequestNotCurrent);
    }
    let (target, previous_head, provider_selection) = command_parts(&request.command);
    if target.recipient_account_id == descriptor.content.account_id
        || target.recipient_device_id == descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if let Some(previous_head) = previous_head {
        validate_previous_head_for_context(previous_head, target, descriptor)?;
    }
    if let Some(provider_selection) = provider_selection {
        if provider_selection.content.descriptor_id != request.descriptor_id
            || provider_selection.content.responder_device_id != descriptor.content.device_id
            || provider_selection.content.request_id == request.request_id
        {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
        selected_store_keys(provider_selection, Some(now_unix_seconds))?;
    }
    Ok(())
}

pub fn runtime_ipc_mailbox_mutation_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcMailboxMutationResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_mailbox_mutation_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcMailboxMutationResultV1,
) -> Result<CanonicalSignedRuntimeIpcMailboxMutationResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if let CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(transition) =
        &result
    {
        verify_transition_for_request(transition, request, descriptor)?;
    }
    let content = CanonicalRuntimeIpcMailboxMutationResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_mailbox_mutation_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_mailbox_mutation_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcMailboxMutationResponseV1 { content, signature };
    verify_signed_runtime_ipc_mailbox_mutation_response_v1(&response)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_mailbox_mutation_response_v1(
    response: &CanonicalSignedRuntimeIpcMailboxMutationResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_mailbox_mutation_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "mailbox mutation response",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcMailboxMutationResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "mailbox mutation response",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-mailbox-mutation-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-mailbox-mutation-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-mailbox-mutation-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcMailboxMutationResponseV1 { content, signature };
    verify_signed_runtime_ipc_mailbox_mutation_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_mailbox_mutation_response_v1(
    response: &CanonicalSignedRuntimeIpcMailboxMutationResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_mailbox_mutation_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_mailbox_mutation_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcMailboxMutationResponseV1,
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_mailbox_mutation_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_mailbox_mutation_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match &response.content.result {
        CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(transition) => {
            verify_transition_for_request(transition, request, descriptor)
        }
        CanonicalRuntimeIpcMailboxMutationResultV1::Error { .. } => Ok(()),
    }
}

pub fn runtime_ipc_mailbox_mutation_response_id_v1(
    response: &CanonicalSignedRuntimeIpcMailboxMutationResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_mailbox_mutation_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    if request.descriptor_id == [0; 32]
        || request.request_id == [0; 32]
        || request.bearer_token == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    match &request.command {
        CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
            target,
            provider_selection,
            offer_valid_for_seconds,
        } => {
            validate_target(target)?;
            validate_offer_validity(*offer_valid_for_seconds)?;
            selected_store_keys(provider_selection, None)?;
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
            target,
            previous_head,
            provider_selection,
            offer_valid_for_seconds,
        } => {
            validate_target(target)?;
            validate_previous_head(previous_head)?;
            validate_offer_validity(*offer_valid_for_seconds)?;
            selected_store_keys(provider_selection, None)?;
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
            target,
            previous_head,
        } => {
            validate_target(target)?;
            validate_previous_head(previous_head)?;
        }
    }
    Ok(())
}

fn validate_target(
    target: &CanonicalRuntimeIpcMailboxMutationTargetV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if target.contact_id == [0; 32]
        || target.recipient_account_id == [0; 32]
        || target.recipient_device_id == [0; 32]
        || target.recipient_device_list_digest == [0; 32]
        || target.scope == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_offer_validity(validity: u64) -> Result<(), CanonicalRuntimeIpcError> {
    if !(MIN_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS
        ..=MAX_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS)
        .contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_previous_head(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_mailbox_capability_update_v1(&head.update)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    if !matches!(
        head.update.content.action,
        CanonicalMailboxCapabilityActionV1::Activate { .. }
    ) {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let acknowledgement = head
        .acknowledgement
        .as_ref()
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    verify_mailbox_capability_acknowledgement_for_v1(
        acknowledgement,
        acknowledgement.content.session_binding,
        &head.update,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    if acknowledgement.content.revoked {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_previous_head_for_context(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    target: &CanonicalRuntimeIpcMailboxMutationTargetV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_previous_head(head)?;
    let content = &head.update.content;
    if content.owner_account_id != descriptor.content.account_id
        || content.owner_device_id != descriptor.content.device_id
        || content.recipient_account_id != target.recipient_account_id
        || content.recipient_device_id != target.recipient_device_id
        || content.scope != target.scope
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn selected_providers(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
) -> Result<&[CanonicalRuntimeIpcVolunteerProviderV1], CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_provider_response_v1(response)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    match &response.content.result {
        CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
            requested,
            providers,
            ..
        } if usize::from(*requested) == providers.len()
            && (MIN_CANONICAL_MAILBOX_REPLICA_STORES..=MAX_CANONICAL_MAILBOX_REPLICA_STORES)
                .contains(&providers.len()) =>
        {
            Ok(providers)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn selected_store_keys(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
    now_unix_seconds: Option<u64>,
) -> Result<Vec<[u8; 32]>, CanonicalRuntimeIpcError> {
    let providers = selected_providers(response)?;
    let mut transport_identities = BTreeSet::new();
    let mut keys = Vec::with_capacity(providers.len());
    for provider in providers {
        if !transport_identities.insert(provider.transport_identity) {
            return Err(CanonicalRuntimeIpcError::InvalidRequest);
        }
        if let Some(now) = now_unix_seconds {
            verify_mailbox_storage_offer_at_v1(&provider.offer, now)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            if provider.observed_at_unix_seconds > now {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
        }
        keys.push(provider.offer.content.store_key);
    }
    if !keys.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(keys)
}

fn command_parts(
    command: &CanonicalRuntimeIpcMailboxMutationCommandV1,
) -> (
    &CanonicalRuntimeIpcMailboxMutationTargetV1,
    Option<&CanonicalRuntimeIpcMailboxCapabilityHeadV1>,
    Option<&CanonicalSignedRuntimeIpcProviderResponseV1>,
) {
    match command {
        CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
            target,
            provider_selection,
            ..
        } => (target, None, Some(provider_selection)),
        CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
            target,
            previous_head,
            provider_selection,
            ..
        } => (target, Some(previous_head), Some(provider_selection)),
        CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
            target,
            previous_head,
        } => (target, Some(previous_head), None),
    }
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcMailboxMutationResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(transition) => {
            validate_transition(transition)
        }
        CanonicalRuntimeIpcMailboxMutationResultV1::Error { code, message_utf8 } => {
            if *code == 0
                || message_utf8.is_empty()
                || message_utf8.len() > MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES
                || std::str::from_utf8(message_utf8).is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
    }
}

fn validate_transition(
    transition: &CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_target(&transition.target).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    verify_signed_mailbox_capability_update_v1(&transition.update)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    match &transition.update.content.action {
        CanonicalMailboxCapabilityActionV1::Activate {
            mode, replica_set, ..
        } => {
            if *mode != CanonicalMailboxProvisioningModeV1::ExactVolunteer
                || replica_set.is_none()
                || transition.provider_selection_id.is_none()
                || transition.offer_expires_at_unix_seconds.is_none()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalMailboxCapabilityActionV1::Revoke { .. } => {
            if transition.provider_selection_id.is_some()
                || transition.offer_expires_at_unix_seconds.is_some()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
    }
    Ok(())
}

fn verify_transition_for_request(
    transition: &CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
    request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_transition(transition)?;
    let (target, _, _) = command_parts(&request.command);
    let content = &transition.update.content;
    if transition.target != *target
        || content.owner_account_id != descriptor.content.account_id
        || content.owner_device_id != descriptor.content.device_id
        || content.recipient_account_id != target.recipient_account_id
        || content.recipient_device_id != target.recipient_device_id
        || content.scope != target.scope
        || content.created_at_unix_seconds < request.issued_at_unix_seconds
        || content.created_at_unix_seconds > request.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match &request.command {
        CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
            provider_selection,
            offer_valid_for_seconds,
            ..
        } => {
            if content.generation != 1 || content.previous_update_id.is_some() {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            verify_activation_transition(transition, provider_selection, *offer_valid_for_seconds)
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
            previous_head,
            provider_selection,
            offer_valid_for_seconds,
            ..
        } => {
            verify_mailbox_capability_update_chain_link_v1(
                &previous_head.update,
                &transition.update,
            )
            .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)?;
            verify_activation_transition(transition, provider_selection, *offer_valid_for_seconds)
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
            previous_head,
            ..
        } => {
            verify_mailbox_capability_update_chain_link_v1(
                &previous_head.update,
                &transition.update,
            )
            .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)?;
            if !matches!(
                transition.update.content.action,
                CanonicalMailboxCapabilityActionV1::Revoke { .. }
            ) {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
    }
}

fn verify_activation_transition(
    transition: &CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
    provider_selection: &CanonicalSignedRuntimeIpcProviderResponseV1,
    offer_valid_for_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    let expected_keys = selected_store_keys(provider_selection, None)?;
    let replica_keys = match &transition.update.content.action {
        CanonicalMailboxCapabilityActionV1::Activate {
            mode: CanonicalMailboxProvisioningModeV1::ExactVolunteer,
            replica_set: Some(replica_set),
            ..
        } => &replica_set.store_keys,
        _ => return Err(CanonicalRuntimeIpcError::ContextMismatch),
    };
    let selection_id = runtime_ipc_provider_response_id_v1(provider_selection)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    let offer_expires_at = transition
        .update
        .content
        .created_at_unix_seconds
        .checked_add(offer_valid_for_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if replica_keys != &expected_keys
        || transition.provider_selection_id != Some(selection_id)
        || transition.offer_expires_at_unix_seconds != Some(offer_expires_at)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    for provider in selected_providers(provider_selection)? {
        if provider.offer.content.expires_at_unix_seconds < offer_expires_at {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    }
    Ok(())
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcMailboxMutationCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
            target,
            provider_selection,
            offer_valid_for_seconds,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_CREATE);
            encode_target(output, target);
            encode_provider_selection(output, provider_selection)?;
            push_unsigned(output, *offer_valid_for_seconds);
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
            target,
            previous_head,
            provider_selection,
            offer_valid_for_seconds,
        } => {
            push_array_header(output, 5);
            push_unsigned(output, COMMAND_ROTATE);
            encode_target(output, target);
            encode_head(output, previous_head, true)?;
            encode_provider_selection(output, provider_selection)?;
            push_unsigned(output, *offer_valid_for_seconds);
        }
        CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
            target,
            previous_head,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_REVOKE);
            encode_target(output, target);
            encode_head(output, previous_head, true)?;
        }
    }
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxMutationCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-mutation-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-mutation-command-v1.variant")?,
        length,
    ) {
        (COMMAND_CREATE, 4) => Ok(
            CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
                target: decode_target(decoder)?,
                provider_selection: decode_provider_selection(decoder)?,
                offer_valid_for_seconds: decoder
                    .unsigned("runtime-ipc-mailbox-mutation-command-v1.offer-valid-for-seconds")?,
            },
        ),
        (COMMAND_ROTATE, 5) => Ok(
            CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
                target: decode_target(decoder)?,
                previous_head: Box::new(decode_head(decoder, true)?),
                provider_selection: decode_provider_selection(decoder)?,
                offer_valid_for_seconds: decoder
                    .unsigned("runtime-ipc-mailbox-mutation-command-v1.offer-valid-for-seconds")?,
            },
        ),
        (COMMAND_REVOKE, 3) => Ok(
            CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
                target: decode_target(decoder)?,
                previous_head: Box::new(decode_head(decoder, true)?),
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_target(output: &mut Vec<u8>, target: &CanonicalRuntimeIpcMailboxMutationTargetV1) {
    push_array_header(output, 6);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &target.contact_id);
    push_bytes(output, &target.recipient_account_id);
    push_bytes(output, &target.recipient_device_id);
    push_bytes(output, &target.recipient_device_list_digest);
    push_bytes(output, &target.scope);
}

fn decode_target(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxMutationTargetV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-mutation-target-v1", 6)?;
    decoder.expect_version("runtime-ipc-mailbox-mutation-target-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcMailboxMutationTargetV1 {
        contact_id: decoder.fixed_bytes("runtime-ipc-mailbox-mutation-target-v1.contact-id")?,
        recipient_account_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-target-v1.recipient-account-id")?,
        recipient_device_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-target-v1.recipient-device-id")?,
        recipient_device_list_digest: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-target-v1.recipient-device-list-digest")?,
        scope: decoder.fixed_bytes("runtime-ipc-mailbox-mutation-target-v1.scope")?,
    })
}

fn encode_provider_selection(
    output: &mut Vec<u8>,
    selection: &CanonicalSignedRuntimeIpcProviderResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_bytes(
        output,
        &encode_signed_runtime_ipc_provider_response_v1(selection)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
    );
    Ok(())
}

fn decode_provider_selection(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSignedRuntimeIpcProviderResponseV1, CanonicalRuntimeIpcError> {
    decode_and_verify_signed_runtime_ipc_provider_response_v1(
        decoder.bytes("runtime-ipc-mailbox-mutation-command-v1.provider-selection")?,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)
}

fn encode_head(
    output: &mut Vec<u8>,
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    request_side: bool,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 3);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(
        output,
        &encode_signed_mailbox_capability_update_v1(&head.update).map_err(|_| {
            if request_side {
                CanonicalRuntimeIpcError::InvalidRequest
            } else {
                CanonicalRuntimeIpcError::InvalidResponse
            }
        })?,
    );
    encode_acknowledgement_option(output, head.acknowledgement.as_ref(), request_side)
}

fn decode_head(
    decoder: &mut AccountDecoder<'_>,
    request_side: bool,
) -> Result<CanonicalRuntimeIpcMailboxCapabilityHeadV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-capability-head-v1", 3)?;
    decoder.expect_version("runtime-ipc-mailbox-capability-head-v1.schema-version")?;
    let invalid = || {
        if request_side {
            CanonicalRuntimeIpcError::InvalidRequest
        } else {
            CanonicalRuntimeIpcError::InvalidResponse
        }
    };
    Ok(CanonicalRuntimeIpcMailboxCapabilityHeadV1 {
        update: decode_and_verify_signed_mailbox_capability_update_v1(
            decoder.bytes("runtime-ipc-mailbox-capability-head-v1.update")?,
        )
        .map_err(|_| invalid())?,
        acknowledgement: decode_acknowledgement_option(decoder, request_side)?,
    })
}

fn encode_acknowledgement_option(
    output: &mut Vec<u8>,
    acknowledgement: Option<&CanonicalSignedMailboxCapabilityAcknowledgementV1>,
    request_side: bool,
) -> Result<(), CanonicalRuntimeIpcError> {
    match acknowledgement {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(acknowledgement) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(
                output,
                &encode_signed_mailbox_capability_acknowledgement_v1(acknowledgement).map_err(
                    |_| {
                        if request_side {
                            CanonicalRuntimeIpcError::InvalidRequest
                        } else {
                            CanonicalRuntimeIpcError::InvalidResponse
                        }
                    },
                )?,
            );
        }
    }
    Ok(())
}

fn decode_acknowledgement_option(
    decoder: &mut AccountDecoder<'_>,
    request_side: bool,
) -> Result<Option<CanonicalSignedMailboxCapabilityAcknowledgementV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-capability-acknowledgement-option-v1")?;
    let invalid = || {
        if request_side {
            CanonicalRuntimeIpcError::InvalidRequest
        } else {
            CanonicalRuntimeIpcError::InvalidResponse
        }
    };
    match (
        decoder.unsigned("runtime-ipc-mailbox-capability-acknowledgement-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decode_and_verify_signed_mailbox_capability_acknowledgement_v1(
                decoder.bytes("runtime-ipc-mailbox-capability-acknowledgement-option-v1.value")?,
            )
            .map_err(|_| invalid())?,
        )),
        _ => Err(invalid()),
    }
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcMailboxMutationResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_result(&mut output, &content.result)?;
    Ok(output)
}

fn decode_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxMutationResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-mutation-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-mailbox-mutation-response-content-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcMailboxMutationResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-mutation-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    })
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcMailboxMutationResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(transition) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_TRANSITION);
            encode_transition(output, transition)?;
        }
        CanonicalRuntimeIpcMailboxMutationResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
    Ok(())
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxMutationResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-mutation-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-mutation-result-v1.variant")?,
        length,
    ) {
        (RESULT_TRANSITION, 2) => Ok(
            CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(Box::new(
                decode_transition(decoder)?,
            )),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcMailboxMutationResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-mailbox-mutation-result-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-mailbox-mutation-result-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_transition(
    output: &mut Vec<u8>,
    transition: &CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 7);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_unsigned(output, encode_outcome(transition.outcome));
    encode_target(output, &transition.target);
    push_bytes(
        output,
        &encode_signed_mailbox_capability_update_v1(&transition.update)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    encode_fixed_option(output, transition.provider_selection_id.as_ref());
    encode_unsigned_option(output, transition.offer_expires_at_unix_seconds);
    push_unsigned(output, encode_delivery_state(transition.delivery_state));
    Ok(())
}

fn decode_transition(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxCapabilityTransitionV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-capability-transition-v1", 7)?;
    decoder.expect_version("runtime-ipc-mailbox-capability-transition-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcMailboxCapabilityTransitionV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-mailbox-capability-transition-v1.outcome")?,
        )?,
        target: decode_target(decoder)?,
        update: decode_and_verify_signed_mailbox_capability_update_v1(
            decoder.bytes("runtime-ipc-mailbox-capability-transition-v1.update")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        provider_selection_id: decode_fixed_option(decoder)?,
        offer_expires_at_unix_seconds: decode_unsigned_option(decoder)?,
        delivery_state: decode_delivery_state(
            decoder.unsigned("runtime-ipc-mailbox-capability-transition-v1.delivery-state")?,
        )?,
    })
}

fn encode_fixed_option(output: &mut Vec<u8>, value: Option<&[u8; 32]>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, value);
        }
    }
}

fn decode_fixed_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-mutation-fixed-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-mutation-fixed-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-mailbox-mutation-fixed-option-v1.value")?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_unsigned_option(output: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_unsigned(output, value);
        }
    }
}

fn decode_unsigned_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<u64>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-mutation-unsigned-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-mutation-unsigned-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder.unsigned("runtime-ipc-mailbox-mutation-unsigned-option-v1.value")?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(outcome: CanonicalRuntimeIpcMailboxMutationOutcomeV1) -> u64 {
    match outcome {
        CanonicalRuntimeIpcMailboxMutationOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcMailboxMutationOutcomeV1::AlreadyCommitted => OUTCOME_ALREADY_COMMITTED,
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcMailboxMutationOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcMailboxMutationOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcMailboxMutationOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_delivery_state(state: CanonicalRuntimeIpcMailboxMutationDeliveryStateV1) -> u64 {
    match state {
        CanonicalRuntimeIpcMailboxMutationDeliveryStateV1::PendingRecipientAcknowledgement => {
            DELIVERY_PENDING_RECIPIENT_ACKNOWLEDGEMENT
        }
    }
}

fn decode_delivery_state(
    value: u64,
) -> Result<CanonicalRuntimeIpcMailboxMutationDeliveryStateV1, CanonicalRuntimeIpcError> {
    match value {
        DELIVERY_PENDING_RECIPIENT_ACKNOWLEDGEMENT => {
            Ok(CanonicalRuntimeIpcMailboxMutationDeliveryStateV1::PendingRecipientAcknowledgement)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.len() > maximum {
        return Err(CanonicalRuntimeIpcError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};

    use super::*;
    use crate::{
        canonical_mailbox_gossip_v1::DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS,
        canonical_mailbox_provisioning_v1::{
            CanonicalMailboxCapabilityUpdateContentV1, mailbox_capability_update_id_v1,
            sign_mailbox_capability_update_v1,
        },
        canonical_mailbox_v1::{
            CanonicalMailboxStorageOfferContentV1, CanonicalMailboxStoragePolicyClassV1,
            CanonicalSignedMailboxStorageOfferV1, mailbox_storage_offer_admission_work_bits_v1,
            mailbox_storage_offer_signing_bytes_v1,
        },
        canonical_runtime_ipc_provider_v1::{
            CanonicalRuntimeIpcProviderCommandV1, CanonicalRuntimeIpcProviderRequestV1,
            sign_runtime_ipc_provider_response_v1,
        },
        canonical_runtime_ipc_v1::{
            CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
            encode_signed_runtime_ipc_descriptor_v1, sign_runtime_ipc_descriptor_v1,
        },
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        signing_key: SigningKey,
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        target: CanonicalRuntimeIpcMailboxMutationTargetV1,
        provider_selection: CanonicalSignedRuntimeIpcProviderResponseV1,
        activation: CanonicalSignedMailboxCapabilityUpdateV1,
        rotation: CanonicalSignedMailboxCapabilityUpdateV1,
        revocation: CanonicalSignedMailboxCapabilityUpdateV1,
        create_request: CanonicalRuntimeIpcMailboxMutationRequestV1,
        rotate_request: CanonicalRuntimeIpcMailboxMutationRequestV1,
        revoke_request: CanonicalRuntimeIpcMailboxMutationRequestV1,
    }

    fn provisioning_vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"
        )))?)
    }

    fn mutation_vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json"
        )))?)
    }

    fn string<'a>(value: &'a Value, pointer: &str) -> TestResult<&'a str> {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}").into())
    }

    fn bytes(value: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let encoded = string(value, pointer)?;
        let (pairs, remainder) = encoded.as_bytes().as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(format!("odd hex length at {pointer}").into());
        }
        pairs
            .iter()
            .map(|pair| {
                let pair = std::str::from_utf8(pair)?;
                Ok(u8::from_str_radix(pair, 16)?)
            })
            .collect()
    }

    fn fixed<const N: usize>(value: &Value, pointer: &str) -> TestResult<[u8; N]> {
        bytes(value, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}").into())
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn provider_offer(
        seed_start: u8,
        provider_endpoint: &[u8],
    ) -> TestResult<CanonicalSignedMailboxStorageOfferV1> {
        let mut seed = [0_u8; 32];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = seed_start.wrapping_add(u8::try_from(index)?);
        }
        let signing_key = SigningKey::from_bytes(&seed);
        let mut content = CanonicalMailboxStorageOfferContentV1 {
            provider_endpoint: provider_endpoint.to_vec(),
            store_key: signing_key.verifying_key().to_bytes(),
            policy_class: CanonicalMailboxStoragePolicyClassV1::BoundedVolunteer,
            capacity_hint_bytes: 200 * 1024 * 1024,
            max_record_bytes: 1024 * 1024,
            issued_at_unix_seconds: 1_800_000_990,
            expires_at_unix_seconds: 1_800_004_590,
            nonce: [0; 32],
        };
        let mut found = false;
        for value in 0_u64..1_000_000 {
            content.nonce[24..].copy_from_slice(&value.to_be_bytes());
            if mailbox_storage_offer_admission_work_bits_v1(&content)?
                >= DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS as u16
            {
                found = true;
                break;
            }
        }
        if !found {
            return Err("provider admission nonce was not found".into());
        }
        let signature = signing_key
            .sign(&mailbox_storage_offer_signing_bytes_v1(&content)?)
            .to_bytes();
        Ok(CanonicalSignedMailboxStorageOfferV1 { content, signature })
    }

    fn request(
        descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
        request_id: [u8; 32],
        issued_at_unix_seconds: u64,
        command: CanonicalRuntimeIpcMailboxMutationCommandV1,
    ) -> TestResult<CanonicalRuntimeIpcMailboxMutationRequestV1> {
        Ok(CanonicalRuntimeIpcMailboxMutationRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(descriptor)?,
            request_id,
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds,
            expires_at_unix_seconds: issued_at_unix_seconds + 30,
            command,
        })
    }

    fn fixture() -> TestResult<Fixture> {
        let provisioning = provisioning_vectors()?;
        let signing_key =
            SigningKey::from_bytes(&fixed(&provisioning, "/input/owner_secret_key_hex")?);
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &signing_key,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: 45136,
                bearer_token: [0x71; 32],
                account_id: fixed(&provisioning, "/input/owner_account_id_hex")?,
                device_id: signing_key.verifying_key().to_bytes(),
                server_instance_id: [0x72; 32],
                issued_at_unix_seconds: 1_800_000_900,
                expires_at_unix_seconds: 1_800_005_000,
            },
        )?;
        let activation = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &provisioning,
            "/expected/activation_update/signed_cbor_hex",
        )?)?;
        let acknowledgement =
            decode_and_verify_signed_mailbox_capability_acknowledgement_v1(&bytes(
                &provisioning,
                "/expected/activation_acknowledgement/signed_cbor_hex",
            )?)?;
        let revocation = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &provisioning,
            "/expected/revocation_update/signed_cbor_hex",
        )?)?;
        let previous_head = CanonicalRuntimeIpcMailboxCapabilityHeadV1 {
            update: activation.clone(),
            acknowledgement: Some(acknowledgement),
        };
        let target = CanonicalRuntimeIpcMailboxMutationTargetV1 {
            contact_id: [0x73; 32],
            recipient_account_id: activation.content.recipient_account_id,
            recipient_device_id: activation.content.recipient_device_id,
            recipient_device_list_digest: [0x74; 32],
            scope: activation.content.scope,
        };
        let provider_inputs = [
            (
                0_u8,
                b"iroh-ticket:mailbox-mutation-provider-1".as_slice(),
                [0x81; 32],
            ),
            (
                64_u8,
                b"iroh-ticket:mailbox-mutation-provider-2".as_slice(),
                [0x82; 32],
            ),
            (
                32_u8,
                b"iroh-ticket:mailbox-mutation-provider-3".as_slice(),
                [0x83; 32],
            ),
        ];
        let providers = provider_inputs
            .into_iter()
            .map(|(seed, endpoint, transport_identity)| {
                Ok(CanonicalRuntimeIpcVolunteerProviderV1 {
                    offer: provider_offer(seed, endpoint)?,
                    transport_identity,
                    observed_at_unix_seconds: 1_800_001_000,
                })
            })
            .collect::<TestResult<Vec<_>>>()?;
        let expected_keys = match &activation.content.action {
            CanonicalMailboxCapabilityActionV1::Activate {
                replica_set: Some(replica_set),
                ..
            } => replica_set.store_keys.clone(),
            _ => return Err("activation fixture has no exact replica set".into()),
        };
        if providers
            .iter()
            .map(|provider| provider.offer.content.store_key)
            .collect::<Vec<_>>()
            != expected_keys
        {
            return Err("provider fixture does not match activation replica set".into());
        }
        let selection_request = CanonicalRuntimeIpcProviderRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0x75; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: 1_800_000_995,
            expires_at_unix_seconds: 1_800_001_025,
            command: CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
                selection_salt: [0x76; 32],
                requested: 3,
            },
        };
        let provider_selection = sign_runtime_ipc_provider_response_v1(
            &signing_key,
            &selection_request,
            &descriptor,
            1_800_001_000,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                selection_salt: [0x76; 32],
                requested: 3,
                active_provider_count: 3,
                providers,
            },
        )?;
        let mut rotation_content: CanonicalMailboxCapabilityUpdateContentV1 =
            activation.content.clone();
        rotation_content.generation = 2;
        rotation_content.previous_update_id = Some(mailbox_capability_update_id_v1(&activation)?);
        rotation_content.created_at_unix_seconds = 1_800_001_010;
        match &mut rotation_content.action {
            CanonicalMailboxCapabilityActionV1::Activate {
                binding_id,
                sealed_peer_offer,
                ..
            } => {
                *binding_id = [0x77; 32];
                sealed_peer_offer.binding_id = [0x77; 32];
            }
            CanonicalMailboxCapabilityActionV1::Revoke { .. } => {
                return Err("activation fixture unexpectedly revoked".into());
            }
        }
        let rotation = sign_mailbox_capability_update_v1(&signing_key, rotation_content)?;
        let create_request = request(
            &descriptor,
            [0x78; 32],
            1_800_000_995,
            CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
                target,
                provider_selection: provider_selection.clone(),
                offer_valid_for_seconds: 3_500,
            },
        )?;
        let rotate_request = request(
            &descriptor,
            [0x79; 32],
            1_800_001_005,
            CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
                target,
                previous_head: Box::new(previous_head.clone()),
                provider_selection: provider_selection.clone(),
                offer_valid_for_seconds: 3_500,
            },
        )?;
        let revoke_request = request(
            &descriptor,
            [0x7a; 32],
            1_800_001_995,
            CanonicalRuntimeIpcMailboxMutationCommandV1::RevokeMailboxCapability {
                target,
                previous_head: Box::new(previous_head.clone()),
            },
        )?;
        Ok(Fixture {
            signing_key,
            descriptor,
            target,
            provider_selection,
            activation,
            rotation,
            revocation,
            create_request,
            rotate_request,
            revoke_request,
        })
    }

    fn transition(
        fixture: &Fixture,
        update: CanonicalSignedMailboxCapabilityUpdateV1,
        active: bool,
    ) -> TestResult<CanonicalRuntimeIpcMailboxCapabilityTransitionV1> {
        let provider_selection_id = if active {
            Some(runtime_ipc_provider_response_id_v1(
                &fixture.provider_selection,
            )?)
        } else {
            None
        };
        let offer_expires_at_unix_seconds = if active {
            Some(update.content.created_at_unix_seconds + 3_500)
        } else {
            None
        };
        Ok(CanonicalRuntimeIpcMailboxCapabilityTransitionV1 {
            outcome: CanonicalRuntimeIpcMailboxMutationOutcomeV1::Committed,
            target: fixture.target,
            update,
            provider_selection_id,
            offer_expires_at_unix_seconds,
            delivery_state:
                CanonicalRuntimeIpcMailboxMutationDeliveryStateV1::PendingRecipientAcknowledgement,
        })
    }

    fn response(
        fixture: &Fixture,
        request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
        transition: CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
        now_unix_seconds: u64,
    ) -> Result<CanonicalSignedRuntimeIpcMailboxMutationResponseV1, CanonicalRuntimeIpcError> {
        sign_runtime_ipc_mailbox_mutation_response_v1(
            &fixture.signing_key,
            request,
            &fixture.descriptor,
            now_unix_seconds,
            CanonicalRuntimeIpcMailboxMutationResultV1::MailboxCapabilityTransition(Box::new(
                transition,
            )),
        )
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_mutations_round_trip() -> TestResult {
        let fixture = fixture()?;
        let cases = [
            (
                &fixture.create_request,
                transition(&fixture, fixture.activation.clone(), true)?,
                1_800_001_000,
            ),
            (
                &fixture.rotate_request,
                transition(&fixture, fixture.rotation.clone(), true)?,
                1_800_001_010,
            ),
            (
                &fixture.revoke_request,
                transition(&fixture, fixture.revocation.clone(), false)?,
                1_800_002_000,
            ),
        ];
        for (request, transition, now) in cases {
            let encoded_request = encode_runtime_ipc_mailbox_mutation_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(&encoded_request)?,
                *request
            );
            verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                now,
            )?;
            let response = response(&fixture, request, transition, now)?;
            let encoded_response =
                encode_signed_runtime_ipc_mailbox_mutation_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(
                    &encoded_response
                )?,
                response
            );
            verify_runtime_ipc_mailbox_mutation_response_for_request_v1(
                &response,
                request,
                &fixture.descriptor,
            )?;
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_mutation_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let mut missing_ack = fixture.rotate_request.clone();
        if let CanonicalRuntimeIpcMailboxMutationCommandV1::RotateMailboxCapability {
            previous_head,
            ..
        } = &mut missing_ack.command
        {
            previous_head.acknowledgement = None;
        }
        assert!(encode_runtime_ipc_mailbox_mutation_request_v1(&missing_ack).is_err());

        let mut reused_selection_id = fixture.create_request.clone();
        if let CanonicalRuntimeIpcMailboxMutationCommandV1::CreateMailboxCapability {
            provider_selection,
            ..
        } = &mut reused_selection_id.command
        {
            reused_selection_id.request_id = provider_selection.content.request_id;
        }
        assert!(
            verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1(
                &reused_selection_id,
                &fixture.descriptor,
                1_800_001_000,
            )
            .is_err()
        );

        let mut wrong_transition = transition(&fixture, fixture.activation.clone(), true)?;
        wrong_transition.provider_selection_id = Some([0xaa; 32]);
        assert!(
            response(
                &fixture,
                &fixture.create_request,
                wrong_transition,
                1_800_001_000,
            )
            .is_err()
        );

        let mut wrong_target = transition(&fixture, fixture.revocation.clone(), false)?;
        wrong_target.target.recipient_device_list_digest[0] ^= 1;
        assert!(
            response(
                &fixture,
                &fixture.revoke_request,
                wrong_target,
                1_800_002_000,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_mutation_vectors_match() -> TestResult {
        let vectors = mutation_vectors()?;
        let fixture = fixture()?;
        let cases = [
            (
                "create",
                &fixture.create_request,
                transition(&fixture, fixture.activation.clone(), true)?,
                1_800_001_000,
            ),
            (
                "rotate",
                &fixture.rotate_request,
                transition(&fixture, fixture.rotation.clone(), true)?,
                1_800_001_010,
            ),
            (
                "revoke",
                &fixture.revoke_request,
                transition(&fixture, fixture.revocation.clone(), false)?,
                1_800_002_000,
            ),
        ];
        for (name, request, transition, now) in cases {
            let response = response(&fixture, request, transition, now)?;
            assert_eq!(
                lower_hex(&encode_runtime_ipc_mailbox_mutation_request_v1(request)?),
                string(&vectors, &format!("/expected/{name}/request/cbor_hex"))?
            );
            assert_eq!(
                lower_hex(&encode_signed_runtime_ipc_mailbox_mutation_response_v1(
                    &response
                )?),
                string(
                    &vectors,
                    &format!("/expected/{name}/response/signed_cbor_hex")
                )?
            );
        }
        Ok(())
    }

    fn vector_case(
        fixture: &Fixture,
        request: &CanonicalRuntimeIpcMailboxMutationRequestV1,
        transition: CanonicalRuntimeIpcMailboxCapabilityTransitionV1,
        now_unix_seconds: u64,
    ) -> TestResult<Value> {
        let request_bytes = encode_runtime_ipc_mailbox_mutation_request_v1(request)?;
        let request_digest = runtime_ipc_mailbox_mutation_request_digest_v1(request)?;
        let response = response(fixture, request, transition, now_unix_seconds)?;
        let response_content = encode_response_content(&response.content)?;
        let response_bytes = encode_signed_runtime_ipc_mailbox_mutation_response_v1(&response)?;
        let response_id = runtime_ipc_mailbox_mutation_response_id_v1(&response)?;
        Ok(json!({
            "request": {
                "cbor_hex": lower_hex(&request_bytes),
                "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_DIGEST_DOMAIN),
                "digest_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_DIGEST_DOMAIN, &request_bytes)),
                "object_id_hex": lower_hex(&request_digest)
            },
            "response": {
                "content_cbor_hex": lower_hex(&response_content),
                "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_SIGNATURE_DOMAIN),
                "signing_preimage_hex": lower_hex(&runtime_ipc_mailbox_mutation_response_signing_bytes_v1(&response.content)?),
                "signature_hex": lower_hex(&response.signature),
                "signed_cbor_hex": lower_hex(&response_bytes),
                "object_id_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_ID_DOMAIN),
                "object_id_hex": lower_hex(&response_id)
            }
        }))
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_mailbox_mutation_vectors() -> TestResult {
        let fixture = fixture()?;
        let descriptor = encode_signed_runtime_ipc_descriptor_v1(&fixture.descriptor)?;
        let provider_selection =
            encode_signed_runtime_ipc_provider_response_v1(&fixture.provider_selection)?;
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json",
                "CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"
            ],
            "input": {
                "owner_secret_key_hex": lower_hex(&fixture.signing_key.to_bytes()),
                "contact_id_hex": lower_hex(&fixture.target.contact_id),
                "recipient_device_list_digest_hex": lower_hex(&fixture.target.recipient_device_list_digest),
                "offer_valid_for_seconds": 3_500,
                "create_request_id_hex": lower_hex(&fixture.create_request.request_id),
                "rotate_request_id_hex": lower_hex(&fixture.rotate_request.request_id),
                "revoke_request_id_hex": lower_hex(&fixture.revoke_request.request_id)
            },
            "expected": {
                "descriptor": {
                    "signed_cbor_hex": lower_hex(&descriptor),
                    "object_id_hex": lower_hex(&runtime_ipc_descriptor_id_v1(&fixture.descriptor)?)
                },
                "provider_selection": {
                    "signed_cbor_hex": lower_hex(&provider_selection),
                    "object_id_hex": lower_hex(&runtime_ipc_provider_response_id_v1(&fixture.provider_selection)?)
                },
                "create": vector_case(
                    &fixture,
                    &fixture.create_request,
                    transition(&fixture, fixture.activation.clone(), true)?,
                    1_800_001_000,
                )?,
                "rotate": vector_case(
                    &fixture,
                    &fixture.rotate_request,
                    transition(&fixture, fixture.rotation.clone(), true)?,
                    1_800_001_010,
                )?,
                "revoke": vector_case(
                    &fixture,
                    &fixture.revoke_request,
                    transition(&fixture, fixture.revocation.clone(), false)?,
                    1_800_002_000,
                )?
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
