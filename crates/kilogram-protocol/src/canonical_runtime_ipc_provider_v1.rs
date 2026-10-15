//! Inactive deterministic-CBOR volunteer-provider control surface for local
//! runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. Provider results carry
//! the original signed offer rather than trusting duplicated string fields;
//! no mailbox read or write capability is exposed by this surface.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_mailbox_gossip_v1::{
        DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS, mailbox_provider_offer_id_v1,
    },
    canonical_mailbox_v1::{
        CanonicalSignedMailboxStorageOfferV1, decode_and_verify_signed_mailbox_storage_offer_v1,
        encode_signed_mailbox_storage_offer_v1, verify_mailbox_storage_offer_admission_work_v1,
        verify_mailbox_storage_offer_at_v1, verify_signed_mailbox_storage_offer_v1,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_BYTES: usize = 256 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_ACTIVE_PROVIDERS: u32 = 4_096;
pub const MAX_CANONICAL_RUNTIME_IPC_SELECTED_PROVIDERS: u8 = 8;

pub const CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-provider-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-provider-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-provider-response-id:cbor-v1\0";

const COMMAND_IMPORT_OFFER: u64 = 0;
const COMMAND_SELECT_PROVIDERS: u64 = 1;
const RESULT_OFFER_IMPORTED: u64 = 0;
const RESULT_PROVIDERS_SELECTED: u64 = 1;
const RESULT_ERROR: u64 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcProviderCommandV1 {
    ImportVolunteerStorageOffer {
        offer: CanonicalSignedMailboxStorageOfferV1,
    },
    SelectVolunteerStorageProviders {
        selection_salt: [u8; 32],
        requested: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcProviderRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcProviderCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcProviderImportOutcomeV1 {
    Inserted,
    Replaced,
    AlreadyPresent,
}

/// Capability-free projection. Every public property is either authenticated
/// by `offer` or local-only observation metadata explicitly named below.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcVolunteerProviderV1 {
    pub offer: CanonicalSignedMailboxStorageOfferV1,
    pub transport_identity: [u8; 32],
    pub observed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcProviderResultV1 {
    VolunteerStorageOfferImported {
        outcome: CanonicalRuntimeIpcProviderImportOutcomeV1,
        active_provider_count: u32,
        provider: CanonicalRuntimeIpcVolunteerProviderV1,
    },
    VolunteerStorageProvidersSelected {
        selection_salt: [u8; 32],
        requested: u8,
        active_provider_count: u32,
        providers: Vec<CanonicalRuntimeIpcVolunteerProviderV1>,
    },
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcProviderResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcProviderResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcProviderResponseV1 {
    pub content: CanonicalRuntimeIpcProviderResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_provider_request_v1(
    request: &CanonicalRuntimeIpcProviderRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_provider_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_provider_command(&mut output, &request.command)?;
    ensure_bound(
        &output,
        "provider request",
        MAX_CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_provider_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcProviderRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "provider request",
        MAX_CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-provider-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-provider-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcProviderRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-provider-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-provider-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-provider-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-provider-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-provider-request-v1.expires-at-unix-seconds")?,
        command: decode_provider_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_provider_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_provider_request_digest_v1(
    request: &CanonicalRuntimeIpcProviderRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_provider_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_provider_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcProviderRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_provider_request(request)?;
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
    if let CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer { offer } =
        &request.command
    {
        verify_mailbox_storage_offer_at_v1(offer, now_unix_seconds)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
        verify_mailbox_storage_offer_admission_work_v1(
            offer,
            DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    }
    Ok(())
}

pub fn runtime_ipc_provider_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcProviderResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_SIGNATURE_DOMAIN,
        &encode_provider_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_provider_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcProviderRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcProviderResultV1,
) -> Result<CanonicalSignedRuntimeIpcProviderResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_provider_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcProviderResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_provider_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_provider_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcProviderResponseV1 { content, signature };
    verify_runtime_ipc_provider_response_for_request_v1(
        &response,
        request,
        descriptor,
        now_unix_seconds,
    )?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_provider_response_v1(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_provider_response_v1(response)?;
    let output = signed_envelope(
        &encode_provider_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "provider response",
        MAX_CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_provider_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcProviderResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "provider response",
        MAX_CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-provider-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-provider-response-v1.schema-version")?;
    let content = decode_provider_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-provider-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcProviderResponseV1 { content, signature };
    verify_signed_runtime_ipc_provider_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_provider_response_v1(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_provider_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_provider_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_provider_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
    request: &CanonicalRuntimeIpcProviderRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_provider_response_v1(response)?;
    verify_runtime_ipc_provider_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_provider_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcProviderResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer { offer },
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
                active_provider_count,
                provider,
                ..
            },
        ) => {
            if provider.offer != *offer || *active_provider_count == 0 {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            validate_provider_at(provider, now_unix_seconds)?;
            if provider.observed_at_unix_seconds < request.issued_at_unix_seconds
                || provider.observed_at_unix_seconds > request.expires_at_unix_seconds
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
                selection_salt,
                requested,
            },
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                selection_salt: result_salt,
                requested: result_requested,
                providers,
                ..
            },
        ) => {
            if result_salt != selection_salt || result_requested != requested {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            for provider in providers {
                validate_provider_at(provider, now_unix_seconds)?;
            }
            Ok(())
        }
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

pub fn runtime_ipc_provider_response_id_v1(
    response: &CanonicalSignedRuntimeIpcProviderResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_provider_response_v1(response)?,
    ))
}

fn validate_provider_request(
    request: &CanonicalRuntimeIpcProviderRequestV1,
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
        CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer { offer } => {
            verify_signed_mailbox_storage_offer_v1(offer)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            verify_mailbox_storage_offer_admission_work_v1(
                offer,
                DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)
        }
        CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
            selection_salt,
            requested,
        } => {
            if *selection_salt == [0; 32]
                || !(1..=MAX_CANONICAL_RUNTIME_IPC_SELECTED_PROVIDERS).contains(requested)
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            Ok(())
        }
    }
}

fn validate_provider_response_content(
    content: &CanonicalRuntimeIpcProviderResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
            active_provider_count,
            provider,
            ..
        } => {
            validate_active_count(*active_provider_count)?;
            if *active_provider_count == 0 {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_provider(provider)
        }
        CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
            selection_salt,
            requested,
            active_provider_count,
            providers,
        } => {
            validate_active_count(*active_provider_count)?;
            if *selection_salt == [0; 32]
                || !(1..=MAX_CANONICAL_RUNTIME_IPC_SELECTED_PROVIDERS).contains(requested)
                || providers.len() > usize::from(*requested)
                || providers.len() > *active_provider_count as usize
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_provider_set(providers)
        }
        CanonicalRuntimeIpcProviderResultV1::Error { code, message_utf8 } => {
            if *code == 0 {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_text(message_utf8, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES)
        }
    }
}

fn validate_active_count(count: u32) -> Result<(), CanonicalRuntimeIpcError> {
    if count > MAX_CANONICAL_RUNTIME_IPC_ACTIVE_PROVIDERS {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_provider(
    provider: &CanonicalRuntimeIpcVolunteerProviderV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_mailbox_storage_offer_v1(&provider.offer)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    verify_mailbox_storage_offer_admission_work_v1(
        &provider.offer,
        DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if provider.transport_identity == [0; 32]
        || provider.observed_at_unix_seconds < provider.offer.content.issued_at_unix_seconds
        || provider.observed_at_unix_seconds >= provider.offer.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_provider_set(
    providers: &[CanonicalRuntimeIpcVolunteerProviderV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    let mut store_keys = BTreeSet::new();
    let mut offer_ids = BTreeSet::new();
    for provider in providers {
        validate_provider(provider)?;
        let offer_id = mailbox_provider_offer_id_v1(&provider.offer)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
        if !store_keys.insert(provider.offer.content.store_key) || !offer_ids.insert(offer_id) {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
    }
    Ok(())
}

fn validate_provider_at(
    provider: &CanonicalRuntimeIpcVolunteerProviderV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_provider(provider)?;
    verify_mailbox_storage_offer_at_v1(&provider.offer, now_unix_seconds)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if provider.observed_at_unix_seconds > now_unix_seconds {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn encode_provider_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcProviderCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer { offer } => {
            push_array_header(output, 2);
            push_unsigned(output, COMMAND_IMPORT_OFFER);
            let encoded = encode_signed_mailbox_storage_offer_v1(offer)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            push_bytes(output, &encoded);
        }
        CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
            selection_salt,
            requested,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_SELECT_PROVIDERS);
            push_bytes(output, selection_salt);
            push_unsigned(output, u64::from(*requested));
        }
    }
    Ok(())
}

fn decode_provider_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcProviderCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-provider-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-provider-command-v1.variant")?,
        length,
    ) {
        (COMMAND_IMPORT_OFFER, 2) => {
            let encoded = decoder.bytes("runtime-ipc-provider-command-v1.offer")?;
            let offer = decode_and_verify_signed_mailbox_storage_offer_v1(encoded)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            Ok(CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer { offer })
        }
        (COMMAND_SELECT_PROVIDERS, 3) => Ok(
            CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
                selection_salt: decoder
                    .fixed_bytes("runtime-ipc-provider-command-v1.selection-salt")?,
                requested: u8::try_from(
                    decoder.unsigned("runtime-ipc-provider-command-v1.requested")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_provider_response_content(
    content: &CanonicalRuntimeIpcProviderResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_provider_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_provider_result(&mut output, &content.result)?;
    Ok(output)
}

fn decode_provider_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcProviderResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-provider-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-provider-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcProviderResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-provider-response-content-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-provider-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-provider-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-provider-response-content-v1.responder-device-id")?,
        result: decode_provider_result(decoder)?,
    };
    validate_provider_response_content(&content)?;
    Ok(content)
}

fn encode_provider_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcProviderResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
            outcome,
            active_provider_count,
            provider,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, RESULT_OFFER_IMPORTED);
            push_unsigned(output, encode_import_outcome(*outcome));
            push_unsigned(output, u64::from(*active_provider_count));
            encode_provider(output, provider)?;
        }
        CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
            selection_salt,
            requested,
            active_provider_count,
            providers,
        } => {
            push_array_header(output, 5);
            push_unsigned(output, RESULT_PROVIDERS_SELECTED);
            push_bytes(output, selection_salt);
            push_unsigned(output, u64::from(*requested));
            push_unsigned(output, u64::from(*active_provider_count));
            push_array_header(output, providers.len() as u64);
            for provider in providers {
                encode_provider(output, provider)?;
            }
        }
        CanonicalRuntimeIpcProviderResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
    Ok(())
}

fn decode_provider_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcProviderResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-provider-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-provider-result-v1.variant")?,
        length,
    ) {
        (RESULT_OFFER_IMPORTED, 4) => Ok(
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
                outcome: decode_import_outcome(decoder)?,
                active_provider_count: u32::try_from(
                    decoder.unsigned("runtime-ipc-provider-result-v1.active-provider-count")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
                provider: decode_provider(decoder)?,
            },
        ),
        (RESULT_PROVIDERS_SELECTED, 5) => {
            let selection_salt =
                decoder.fixed_bytes("runtime-ipc-provider-result-v1.selection-salt")?;
            let requested =
                u8::try_from(decoder.unsigned("runtime-ipc-provider-result-v1.requested")?)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let active_provider_count = u32::try_from(
                decoder.unsigned("runtime-ipc-provider-result-v1.active-provider-count")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let count = decoder.array("runtime-ipc-provider-result-v1.providers")?;
            if count > usize::from(MAX_CANONICAL_RUNTIME_IPC_SELECTED_PROVIDERS) {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            let providers = (0..count)
                .map(|_| decode_provider(decoder))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(
                CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                    selection_salt,
                    requested,
                    active_provider_count,
                    providers,
                },
            )
        }
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcProviderResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-provider-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-provider-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_provider(
    output: &mut Vec<u8>,
    provider: &CanonicalRuntimeIpcVolunteerProviderV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 3);
    let encoded = encode_signed_mailbox_storage_offer_v1(&provider.offer)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    push_bytes(output, &encoded);
    push_bytes(output, &provider.transport_identity);
    push_unsigned(output, provider.observed_at_unix_seconds);
    Ok(())
}

fn decode_provider(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcVolunteerProviderV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-volunteer-provider-v1", 3)?;
    let encoded = decoder.bytes("runtime-ipc-volunteer-provider-v1.offer")?;
    Ok(CanonicalRuntimeIpcVolunteerProviderV1 {
        offer: decode_and_verify_signed_mailbox_storage_offer_v1(encoded)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        transport_identity: decoder
            .fixed_bytes("runtime-ipc-volunteer-provider-v1.transport-identity")?,
        observed_at_unix_seconds: decoder
            .unsigned("runtime-ipc-volunteer-provider-v1.observed-at-unix-seconds")?,
    })
}

fn encode_import_outcome(outcome: CanonicalRuntimeIpcProviderImportOutcomeV1) -> u64 {
    match outcome {
        CanonicalRuntimeIpcProviderImportOutcomeV1::Inserted => 0,
        CanonicalRuntimeIpcProviderImportOutcomeV1::Replaced => 1,
        CanonicalRuntimeIpcProviderImportOutcomeV1::AlreadyPresent => 2,
    }
}

fn decode_import_outcome(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcProviderImportOutcomeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-provider-result-v1.import-outcome")? {
        0 => Ok(CanonicalRuntimeIpcProviderImportOutcomeV1::Inserted),
        1 => Ok(CanonicalRuntimeIpcProviderImportOutcomeV1::Replaced),
        2 => Ok(CanonicalRuntimeIpcProviderImportOutcomeV1::AlreadyPresent),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn validate_text(bytes: &[u8], maximum: usize) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.is_empty() || bytes.len() > maximum || std::str::from_utf8(bytes).is_err() {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
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
    let mut input = Vec::with_capacity(domain.len() + bytes.len());
    input.extend_from_slice(domain);
    input.extend_from_slice(bytes);
    *blake3::hash(&input).as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};

    use super::*;
    use crate::{
        canonical_mailbox_v1::{
            CanonicalMailboxStorageOfferContentV1, CanonicalMailboxStoragePolicyClassV1,
            mailbox_storage_offer_admission_work_bits_v1, mailbox_storage_offer_signing_bytes_v1,
        },
        canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        offer: CanonicalSignedMailboxStorageOfferV1,
        store_secret_key: [u8; 32],
        transport_identity: [u8; 32],
        selection_salt: [u8; 32],
        request_id: [u8; 32],
        selection_request_id: [u8; 32],
        issued_at: u64,
        expires_at: u64,
    }

    fn bytes(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}"))?;
        if value.len() % 2 != 0 {
            return Err(format!("odd hex length at {pointer}").into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        Ok(bytes(document, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}"))?)
    }

    fn fixture() -> TestResult<Fixture> {
        let core: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
        )))?;
        let mailbox: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-VECTORS.json"
        )))?;
        let gossip: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-GOSSIP-VECTORS.json"
        )))?;
        let store_secret_key = fixed(&mailbox, "/input/store_secret_key_hex")?;
        let store_key = SigningKey::from_bytes(&store_secret_key);
        let mut content = CanonicalMailboxStorageOfferContentV1 {
            provider_endpoint: b"iroh-ticket:canonical-runtime-provider".to_vec(),
            store_key: store_key.verifying_key().to_bytes(),
            policy_class: CanonicalMailboxStoragePolicyClassV1::BoundedVolunteer,
            capacity_hint_bytes: 200 * 1024 * 1024,
            max_record_bytes: 1024 * 1024,
            issued_at_unix_seconds: 1_800_002_000,
            expires_at_unix_seconds: 1_800_002_300,
            nonce: [0; 32],
        };
        for value in 0_u64..100_000 {
            content.nonce[24..].copy_from_slice(&value.to_be_bytes());
            if mailbox_storage_offer_admission_work_bits_v1(&content)?
                >= DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS as u16
            {
                break;
            }
        }
        let signature = store_key
            .sign(&mailbox_storage_offer_signing_bytes_v1(&content)?)
            .to_bytes();
        let offer = CanonicalSignedMailboxStorageOfferV1 { content, signature };
        verify_mailbox_storage_offer_admission_work_v1(
            &offer,
            DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS,
        )?;
        Ok(Fixture {
            descriptor: decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
                &core,
                "/expected/descriptor/signed_cbor_hex",
            )?)?,
            device_key: SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?),
            offer,
            store_secret_key,
            transport_identity: fixed(&gossip, "/input/transport_identity_hex")?,
            selection_salt: fixed(&gossip, "/input/selection_salt_hex")?,
            request_id: [0x92; 32],
            selection_request_id: [0x93; 32],
            issued_at: 1_800_002_100,
            expires_at: 1_800_002_130,
        })
    }

    fn request(
        fixture: &Fixture,
        command: CanonicalRuntimeIpcProviderCommandV1,
    ) -> TestResult<CanonicalRuntimeIpcProviderRequestV1> {
        Ok(CanonicalRuntimeIpcProviderRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&fixture.descriptor)?,
            request_id: fixture.request_id,
            bearer_token: fixture.descriptor.content.bearer_token,
            issued_at_unix_seconds: fixture.issued_at,
            expires_at_unix_seconds: fixture.expires_at,
            command,
        })
    }

    fn provider(fixture: &Fixture) -> CanonicalRuntimeIpcVolunteerProviderV1 {
        CanonicalRuntimeIpcVolunteerProviderV1 {
            offer: fixture.offer.clone(),
            transport_identity: fixture.transport_identity,
            observed_at_unix_seconds: fixture.issued_at,
        }
    }

    fn selection_request(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcProviderRequestV1> {
        let mut request = request(
            fixture,
            CanonicalRuntimeIpcProviderCommandV1::SelectVolunteerStorageProviders {
                selection_salt: fixture.selection_salt,
                requested: 2,
            },
        )?;
        request.request_id = fixture.selection_request_id;
        Ok(request)
    }

    #[test]
    fn canonical_runtime_ipc_provider_round_trips_and_binds_offer() -> TestResult {
        let fixture = fixture()?;
        let request = request(
            &fixture,
            CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer {
                offer: fixture.offer.clone(),
            },
        )?;
        let encoded = encode_runtime_ipc_provider_request_v1(&request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_provider_request_v1(&encoded)?,
            request
        );
        let response = sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
                outcome: CanonicalRuntimeIpcProviderImportOutcomeV1::Inserted,
                active_provider_count: 1,
                provider: provider(&fixture),
            },
        )?;
        let encoded = encode_signed_runtime_ipc_provider_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_provider_response_v1(&encoded)?,
            response
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_provider_selection_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let request = selection_request(&fixture)?;
        let mut previously_observed = provider(&fixture);
        previously_observed.observed_at_unix_seconds = fixture.offer.content.issued_at_unix_seconds;
        sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                selection_salt: fixture.selection_salt,
                requested: 2,
                active_provider_count: 1,
                providers: vec![previously_observed],
            },
        )?;
        let mut wrong_salt = fixture.selection_salt;
        wrong_salt[0] ^= 1;
        assert!(
            sign_runtime_ipc_provider_response_v1(
                &fixture.device_key,
                &request,
                &fixture.descriptor,
                fixture.issued_at,
                CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                    selection_salt: wrong_salt,
                    requested: 2,
                    active_provider_count: 1,
                    providers: vec![provider(&fixture)],
                },
            )
            .is_err()
        );
        let duplicate = provider(&fixture);
        let result = CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
            selection_salt: fixture.selection_salt,
            requested: 2,
            active_provider_count: 2,
            providers: vec![duplicate.clone(), duplicate],
        };
        assert!(
            sign_runtime_ipc_provider_response_v1(
                &fixture.device_key,
                &request,
                &fixture.descriptor,
                fixture.issued_at,
                result,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_provider_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json"
        )))?;
        let request = request(
            &fixture,
            CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer {
                offer: fixture.offer.clone(),
            },
        )?;
        let response = sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
                outcome: CanonicalRuntimeIpcProviderImportOutcomeV1::Inserted,
                active_provider_count: 1,
                provider: provider(&fixture),
            },
        )?;
        assert_eq!(
            encode_runtime_ipc_provider_request_v1(&request)?,
            bytes(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_provider_response_v1(&response)?,
            bytes(&vectors, "/expected/response/signed_cbor_hex")?
        );
        let selection_request = selection_request(&fixture)?;
        let mut selected_provider = provider(&fixture);
        selected_provider.observed_at_unix_seconds = fixture.offer.content.issued_at_unix_seconds;
        let selection_response = sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &selection_request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                selection_salt: fixture.selection_salt,
                requested: 2,
                active_provider_count: 1,
                providers: vec![selected_provider],
            },
        )?;
        assert_eq!(
            encode_runtime_ipc_provider_request_v1(&selection_request)?,
            bytes(&vectors, "/expected/selection_request/cbor_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_provider_response_v1(&selection_response)?,
            bytes(&vectors, "/expected/selection_response/signed_cbor_hex")?
        );
        Ok(())
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_provider_vectors() -> TestResult {
        let fixture = fixture()?;
        let request = request(
            &fixture,
            CanonicalRuntimeIpcProviderCommandV1::ImportVolunteerStorageOffer {
                offer: fixture.offer.clone(),
            },
        )?;
        let request_bytes = encode_runtime_ipc_provider_request_v1(&request)?;
        let request_digest = runtime_ipc_provider_request_digest_v1(&request)?;
        let provider = provider(&fixture);
        let response = sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageOfferImported {
                outcome: CanonicalRuntimeIpcProviderImportOutcomeV1::Inserted,
                active_provider_count: 1,
                provider: provider.clone(),
            },
        )?;
        let content_bytes = encode_provider_response_content(&response.content)?;
        let signed_bytes = encode_signed_runtime_ipc_provider_response_v1(&response)?;
        let response_id = runtime_ipc_provider_response_id_v1(&response)?;
        let selection_request = selection_request(&fixture)?;
        let selection_request_bytes = encode_runtime_ipc_provider_request_v1(&selection_request)?;
        let selection_request_digest = runtime_ipc_provider_request_digest_v1(&selection_request)?;
        let mut selected_provider = provider.clone();
        selected_provider.observed_at_unix_seconds = fixture.offer.content.issued_at_unix_seconds;
        let selection_response = sign_runtime_ipc_provider_response_v1(
            &fixture.device_key,
            &selection_request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcProviderResultV1::VolunteerStorageProvidersSelected {
                selection_salt: fixture.selection_salt,
                requested: 2,
                active_provider_count: 1,
                providers: vec![selected_provider],
            },
        )?;
        let selection_content_bytes =
            encode_provider_response_content(&selection_response.content)?;
        let selection_signed_bytes =
            encode_signed_runtime_ipc_provider_response_v1(&selection_response)?;
        let selection_response_id = runtime_ipc_provider_response_id_v1(&selection_response)?;
        let offer_bytes = encode_signed_mailbox_storage_offer_v1(&fixture.offer)?;
        let offer_id = mailbox_provider_offer_id_v1(&fixture.offer)?;
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-MAILBOX-VECTORS.json",
                "CANONICAL-MAILBOX-GOSSIP-VECTORS.json"
            ],
            "input": {
                "request_id_hex": lower_hex(&fixture.request_id),
                "selection_request_id_hex": lower_hex(&fixture.selection_request_id),
                "request_issued_at_unix_seconds": fixture.issued_at,
                "request_expires_at_unix_seconds": fixture.expires_at,
                "transport_identity_hex": lower_hex(&fixture.transport_identity),
                "selection_salt_hex": lower_hex(&fixture.selection_salt),
                "store_secret_key_hex": lower_hex(&fixture.store_secret_key),
                "provider_endpoint_hex": lower_hex(&fixture.offer.content.provider_endpoint),
                "capacity_hint_bytes": fixture.offer.content.capacity_hint_bytes,
                "max_record_bytes": fixture.offer.content.max_record_bytes,
                "offer_issued_at_unix_seconds": fixture.offer.content.issued_at_unix_seconds,
                "offer_expires_at_unix_seconds": fixture.offer.content.expires_at_unix_seconds,
                "offer_nonce_hex": lower_hex(&fixture.offer.content.nonce),
                "offer_signed_cbor_hex": lower_hex(&offer_bytes),
                "observed_at_unix_seconds": provider.observed_at_unix_seconds,
                "selection_observed_at_unix_seconds": fixture.offer.content.issued_at_unix_seconds
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "offer_id_hex": lower_hex(&offer_id),
                "request": {
                    "cbor_hex": lower_hex(&request_bytes),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN, &request_bytes)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&content_bytes),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_provider_response_signing_bytes_v1(&response.content)?),
                    "signature_hex": lower_hex(&response.signature),
                    "signed_cbor_hex": lower_hex(&signed_bytes),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_ID_DOMAIN, &signed_bytes)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "selection_request": {
                    "cbor_hex": lower_hex(&selection_request_bytes),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_DIGEST_DOMAIN, &selection_request_bytes)),
                    "object_id_hex": lower_hex(&selection_request_digest)
                },
                "selection_response": {
                    "content_cbor_hex": lower_hex(&selection_content_bytes),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_provider_response_signing_bytes_v1(&selection_response.content)?),
                    "signature_hex": lower_hex(&selection_response.signature),
                    "signed_cbor_hex": lower_hex(&selection_signed_bytes),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_ID_DOMAIN, &selection_signed_bytes)),
                    "object_id_hex": lower_hex(&selection_response_id)
                },
                "context": {
                    "valid_at_unix_seconds": fixture.issued_at,
                    "provider_properties_derive_from_signed_offer": true,
                    "mailbox_capabilities_in_payload": false,
                    "response_binds_complete_request_digest": true,
                    "active_runtime_changed": false
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
