//! Inactive deterministic-CBOR ticket-publication write capability.
//!
//! The active HTTPS store still uses lowercase hexadecimal headers and the
//! legacy raw authorization preimage. This module defines a bounded,
//! cross-language migration target without changing that runtime contract.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_TICKET_PUBLICATION_CAPABILITY_BYTES: usize = 128;
pub const MAX_CANONICAL_TICKET_PUBLICATION_AUTHORIZATION_BYTES: usize = 1024;
pub const MAX_CANONICAL_TICKET_PUBLICATION_BODY_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_CANONICAL_TICKET_PUBLICATION_PEER_SCOPE_BYTES: usize = 4096;

/// Preserved from the active capability so an independently derived public key
/// continues to name the same pseudonymous store channel.
pub const CANONICAL_TICKET_PUBLICATION_CHANNEL_ID_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-capability-channel:v1\0";
pub const CANONICAL_TICKET_PUBLICATION_WRITE_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-write-authorization:cbor-v1\0";
pub const CANONICAL_TICKET_PUBLICATION_WRITE_CAPABILITY_DERIVATION_CONTEXT: &str =
    "kilogram ticket publication per-peer write capability v1";
pub const CANONICAL_TICKET_PUBLICATION_ROTATED_WRITE_CAPABILITY_DERIVATION_CONTEXT: &str =
    "kilogram ticket publication per-peer rotated write capability v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTicketPublicationCapabilityV1 {
    pub write_key: [u8; 32],
    pub channel_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTicketPublicationWriteAuthorizationContentV1 {
    pub capability: CanonicalTicketPublicationCapabilityV1,
    pub generation: u64,
    pub body_length: u64,
    pub body_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedTicketPublicationWriteAuthorizationV1 {
    pub content: CanonicalTicketPublicationWriteAuthorizationContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalTicketPublicationError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error("canonical ticket-publication {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical ticket-publication peer scope is outside protocol bounds")]
    InvalidPeerScope,
    #[error("canonical ticket-publication capability contains an invalid Ed25519 key")]
    InvalidWriteKey,
    #[error("canonical ticket-publication channel does not match its write key")]
    ChannelMismatch,
    #[error("canonical ticket-publication authorization metadata is outside protocol bounds")]
    InvalidAuthorization,
    #[error("canonical ticket-publication authorization contains an invalid signature")]
    InvalidSignature,
    #[error("canonical ticket-publication authorization does not match the supplied body")]
    BodyMismatch,
}

pub fn derive_ticket_publication_capability_v1(
    device_secret: [u8; 32],
    peer_scope: &[u8],
    epoch: u64,
) -> Result<CanonicalTicketPublicationCapabilityV1, CanonicalTicketPublicationError> {
    let signing_key = derive_write_signing_key(device_secret, peer_scope, epoch)?;
    let write_key = signing_key.verifying_key().to_bytes();
    Ok(CanonicalTicketPublicationCapabilityV1 {
        write_key,
        channel_id: ticket_publication_channel_id_v1(&write_key),
    })
}

pub fn ticket_publication_channel_id_v1(write_key: &[u8; 32]) -> [u8; 32] {
    domain_hash(CANONICAL_TICKET_PUBLICATION_CHANNEL_ID_DOMAIN, write_key)
}

pub fn encode_ticket_publication_capability_v1(
    capability: &CanonicalTicketPublicationCapabilityV1,
) -> Result<Vec<u8>, CanonicalTicketPublicationError> {
    validate_capability(capability)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &capability.write_key);
    push_bytes(&mut output, &capability.channel_id);
    ensure_bound(
        &output,
        "capability",
        MAX_CANONICAL_TICKET_PUBLICATION_CAPABILITY_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_ticket_publication_capability_v1(
    bytes: &[u8],
) -> Result<CanonicalTicketPublicationCapabilityV1, CanonicalTicketPublicationError> {
    ensure_bound(
        bytes,
        "capability",
        MAX_CANONICAL_TICKET_PUBLICATION_CAPABILITY_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("ticket-publication-capability-v1", 3)?;
    decoder.expect_version("ticket-publication-capability-v1.schema_version")?;
    let capability = CanonicalTicketPublicationCapabilityV1 {
        write_key: decoder.fixed_bytes("ticket-publication-capability-v1.write_key")?,
        channel_id: decoder.fixed_bytes("ticket-publication-capability-v1.channel_id")?,
    };
    decoder.finish()?;
    validate_capability(&capability)?;
    if encode_ticket_publication_capability_v1(&capability)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(capability)
}

pub fn ticket_publication_write_authorization_signing_bytes_v1(
    content: &CanonicalTicketPublicationWriteAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalTicketPublicationError> {
    validate_authorization_content(content)?;
    Ok(prefixed(
        CANONICAL_TICKET_PUBLICATION_WRITE_AUTHORIZATION_SIGNATURE_DOMAIN,
        &encode_authorization_content(content)?,
    ))
}

pub fn sign_ticket_publication_write_authorization_v1(
    device_secret: [u8; 32],
    peer_scope: &[u8],
    epoch: u64,
    generation: u64,
    body: &[u8],
) -> Result<CanonicalSignedTicketPublicationWriteAuthorizationV1, CanonicalTicketPublicationError> {
    let body_length = u64::try_from(body.len())
        .map_err(|_| CanonicalTicketPublicationError::InvalidAuthorization)?;
    let signing_key = derive_write_signing_key(device_secret, peer_scope, epoch)?;
    let write_key = signing_key.verifying_key().to_bytes();
    let content = CanonicalTicketPublicationWriteAuthorizationContentV1 {
        capability: CanonicalTicketPublicationCapabilityV1 {
            write_key,
            channel_id: ticket_publication_channel_id_v1(&write_key),
        },
        generation,
        body_length,
        body_digest: *blake3::hash(body).as_bytes(),
    };
    let signature = signing_key
        .sign(&ticket_publication_write_authorization_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let authorization = CanonicalSignedTicketPublicationWriteAuthorizationV1 { content, signature };
    verify_signed_ticket_publication_write_authorization_v1(&authorization)?;
    Ok(authorization)
}

pub fn encode_signed_ticket_publication_write_authorization_v1(
    authorization: &CanonicalSignedTicketPublicationWriteAuthorizationV1,
) -> Result<Vec<u8>, CanonicalTicketPublicationError> {
    verify_signed_ticket_publication_write_authorization_v1(authorization)?;
    let content = encode_authorization_content(&authorization.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &authorization.signature);
    ensure_bound(
        &output,
        "signed authorization",
        MAX_CANONICAL_TICKET_PUBLICATION_AUTHORIZATION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_ticket_publication_write_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedTicketPublicationWriteAuthorizationV1, CanonicalTicketPublicationError> {
    ensure_bound(
        bytes,
        "signed authorization",
        MAX_CANONICAL_TICKET_PUBLICATION_AUTHORIZATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-ticket-publication-write-authorization-v1", 3)?;
    decoder.expect_version("signed-ticket-publication-write-authorization-v1.schema_version")?;
    let content = decode_authorization_content(
        decoder.bytes("signed-ticket-publication-write-authorization-v1.content")?,
    )?;
    let signature =
        decoder.fixed_bytes("signed-ticket-publication-write-authorization-v1.signature")?;
    decoder.finish()?;
    let authorization = CanonicalSignedTicketPublicationWriteAuthorizationV1 { content, signature };
    verify_signed_ticket_publication_write_authorization_v1(&authorization)?;
    if encode_signed_ticket_publication_write_authorization_v1(&authorization)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(authorization)
}

pub fn verify_signed_ticket_publication_write_authorization_v1(
    authorization: &CanonicalSignedTicketPublicationWriteAuthorizationV1,
) -> Result<(), CanonicalTicketPublicationError> {
    validate_authorization_content(&authorization.content)?;
    let verifying_key = VerifyingKey::from_bytes(&authorization.content.capability.write_key)
        .map_err(|_| CanonicalTicketPublicationError::InvalidWriteKey)?;
    verifying_key
        .verify_strict(
            &ticket_publication_write_authorization_signing_bytes_v1(&authorization.content)?,
            &Signature::from_bytes(&authorization.signature),
        )
        .map_err(|_| CanonicalTicketPublicationError::InvalidSignature)
}

pub fn verify_ticket_publication_write_authorization_for_body_v1(
    authorization: &CanonicalSignedTicketPublicationWriteAuthorizationV1,
    body: &[u8],
) -> Result<(), CanonicalTicketPublicationError> {
    verify_signed_ticket_publication_write_authorization_v1(authorization)?;
    let body_length =
        u64::try_from(body.len()).map_err(|_| CanonicalTicketPublicationError::BodyMismatch)?;
    if authorization.content.body_length != body_length
        || authorization.content.body_digest != *blake3::hash(body).as_bytes()
    {
        return Err(CanonicalTicketPublicationError::BodyMismatch);
    }
    Ok(())
}

fn encode_authorization_content(
    content: &CanonicalTicketPublicationWriteAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalTicketPublicationError> {
    validate_authorization_content(content)?;
    let capability = encode_ticket_publication_capability_v1(&content.capability)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &capability);
    push_unsigned(&mut output, content.generation);
    push_unsigned(&mut output, content.body_length);
    push_bytes(&mut output, &content.body_digest);
    Ok(output)
}

fn decode_authorization_content(
    bytes: &[u8],
) -> Result<CanonicalTicketPublicationWriteAuthorizationContentV1, CanonicalTicketPublicationError>
{
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("ticket-publication-write-authorization-content-v1", 5)?;
    decoder.expect_version("ticket-publication-write-authorization-content-v1.schema_version")?;
    let capability = decode_and_verify_ticket_publication_capability_v1(
        decoder.bytes("ticket-publication-write-authorization-content-v1.capability")?,
    )?;
    let content = CanonicalTicketPublicationWriteAuthorizationContentV1 {
        capability,
        generation: decoder
            .unsigned("ticket-publication-write-authorization-content-v1.generation")?,
        body_length: decoder
            .unsigned("ticket-publication-write-authorization-content-v1.body_length")?,
        body_digest: decoder
            .fixed_bytes("ticket-publication-write-authorization-content-v1.body_digest")?,
    };
    decoder.finish()?;
    validate_authorization_content(&content)?;
    if encode_authorization_content(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn derive_write_signing_key(
    device_secret: [u8; 32],
    peer_scope: &[u8],
    epoch: u64,
) -> Result<SigningKey, CanonicalTicketPublicationError> {
    if peer_scope.is_empty() || peer_scope.len() > MAX_CANONICAL_TICKET_PUBLICATION_PEER_SCOPE_BYTES
    {
        return Err(CanonicalTicketPublicationError::InvalidPeerScope);
    }
    let device_secret = Zeroizing::new(device_secret);
    let mut material = Zeroizing::new(Vec::with_capacity(
        device_secret.len() + peer_scope.len() + usize::from(epoch != 0) * 9,
    ));
    material.extend_from_slice(device_secret.as_ref());
    material.extend_from_slice(peer_scope);
    let context = if epoch == 0 {
        CANONICAL_TICKET_PUBLICATION_WRITE_CAPABILITY_DERIVATION_CONTEXT
    } else {
        material.push(0);
        material.extend_from_slice(&epoch.to_be_bytes());
        CANONICAL_TICKET_PUBLICATION_ROTATED_WRITE_CAPABILITY_DERIVATION_CONTEXT
    };
    let seed = Zeroizing::new(blake3::derive_key(context, material.as_slice()));
    Ok(SigningKey::from_bytes(&seed))
}

fn validate_capability(
    capability: &CanonicalTicketPublicationCapabilityV1,
) -> Result<(), CanonicalTicketPublicationError> {
    VerifyingKey::from_bytes(&capability.write_key)
        .map_err(|_| CanonicalTicketPublicationError::InvalidWriteKey)?;
    if capability.channel_id != ticket_publication_channel_id_v1(&capability.write_key) {
        return Err(CanonicalTicketPublicationError::ChannelMismatch);
    }
    Ok(())
}

fn validate_authorization_content(
    content: &CanonicalTicketPublicationWriteAuthorizationContentV1,
) -> Result<(), CanonicalTicketPublicationError> {
    validate_capability(&content.capability)?;
    if content.generation == 0
        || !(1..=MAX_CANONICAL_TICKET_PUBLICATION_BODY_BYTES).contains(&content.body_length)
    {
        return Err(CanonicalTicketPublicationError::InvalidAuthorization);
    }
    Ok(())
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalTicketPublicationError> {
    if bytes.len() > maximum {
        return Err(CanonicalTicketPublicationError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(value);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vectors {
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Input {
        device_secret_hex: String,
        peer_scope_hex: String,
        rotated_epoch: u64,
        generation: u64,
        body_hex: String,
    }

    #[derive(Deserialize)]
    struct Expected {
        legacy_capability: CapabilityExpected,
        rotated_capability: CapabilityExpected,
        authorization: AuthorizationExpected,
    }

    #[derive(Deserialize)]
    struct CapabilityExpected {
        write_key_hex: String,
        channel_id_hex: String,
        cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct AuthorizationExpected {
        content_cbor_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
        body_digest_hex: String,
    }

    fn vectors() -> Result<Vectors, serde_json::Error> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-TICKET-PUBLICATION-VECTORS.json"
        )))
    }

    fn bytes(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("odd hexadecimal length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(bytes(value)?
            .try_into()
            .map_err(|_| "unexpected vector length")?)
    }

    fn hex(value: &[u8]) -> String {
        value.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn derivation_and_authorization_match_golden_vectors() -> Result<(), Box<dyn std::error::Error>>
    {
        let vectors = vectors()?;
        let device_secret = fixed(&vectors.input.device_secret_hex)?;
        let peer_scope = bytes(&vectors.input.peer_scope_hex)?;
        let legacy = derive_ticket_publication_capability_v1(device_secret, &peer_scope, 0)?;
        let rotated = derive_ticket_publication_capability_v1(
            device_secret,
            &peer_scope,
            vectors.input.rotated_epoch,
        )?;
        assert_eq!(
            hex(&legacy.write_key),
            vectors.expected.legacy_capability.write_key_hex
        );
        assert_eq!(
            hex(&legacy.channel_id),
            vectors.expected.legacy_capability.channel_id_hex
        );
        assert_eq!(
            hex(&encode_ticket_publication_capability_v1(&legacy)?),
            vectors.expected.legacy_capability.cbor_hex
        );
        assert_eq!(
            hex(&rotated.write_key),
            vectors.expected.rotated_capability.write_key_hex
        );
        assert_eq!(
            hex(&rotated.channel_id),
            vectors.expected.rotated_capability.channel_id_hex
        );
        assert_eq!(
            hex(&encode_ticket_publication_capability_v1(&rotated)?),
            vectors.expected.rotated_capability.cbor_hex
        );

        let body = bytes(&vectors.input.body_hex)?;
        let authorization = sign_ticket_publication_write_authorization_v1(
            device_secret,
            &peer_scope,
            vectors.input.rotated_epoch,
            vectors.input.generation,
            &body,
        )?;
        assert_eq!(authorization.content.capability, rotated);
        assert_eq!(
            hex(&encode_authorization_content(&authorization.content)?),
            vectors.expected.authorization.content_cbor_hex
        );
        assert_eq!(
            hex(&ticket_publication_write_authorization_signing_bytes_v1(
                &authorization.content
            )?),
            vectors.expected.authorization.signing_preimage_hex
        );
        assert_eq!(
            hex(&authorization.signature),
            vectors.expected.authorization.signature_hex
        );
        assert_eq!(
            hex(&authorization.content.body_digest),
            vectors.expected.authorization.body_digest_hex
        );
        let encoded = encode_signed_ticket_publication_write_authorization_v1(&authorization)?;
        assert_eq!(
            hex(&encoded),
            vectors.expected.authorization.signed_cbor_hex
        );
        assert_eq!(
            decode_and_verify_signed_ticket_publication_write_authorization_v1(&encoded)?,
            authorization
        );
        verify_ticket_publication_write_authorization_for_body_v1(&authorization, &body)?;
        Ok(())
    }

    #[test]
    fn substitutions_and_wrong_body_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let device_secret = fixed(&vectors.input.device_secret_hex)?;
        let peer_scope = bytes(&vectors.input.peer_scope_hex)?;
        let body = bytes(&vectors.input.body_hex)?;
        let authorization = sign_ticket_publication_write_authorization_v1(
            device_secret,
            &peer_scope,
            vectors.input.rotated_epoch,
            vectors.input.generation,
            &body,
        )?;
        assert!(
            verify_ticket_publication_write_authorization_for_body_v1(
                &authorization,
                b"changed body"
            )
            .is_err()
        );
        let mut changed = authorization.clone();
        changed.content.generation += 1;
        assert!(verify_signed_ticket_publication_write_authorization_v1(&changed).is_err());
        let mut changed = authorization;
        changed.content.capability.channel_id[0] ^= 1;
        assert!(verify_signed_ticket_publication_write_authorization_v1(&changed).is_err());
        Ok(())
    }
}
