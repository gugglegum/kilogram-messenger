use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use kilogram_identity::{AccountId, DeviceId};
use serde::{Deserialize, Serialize};

use crate::{
    ConnectionTicket, encode_hex, read_bounded_regular_file,
    recovery_qr::{
        RecoveryQrRenderReport, decode_bounded_ascii_qr_image, render_bounded_ascii_qr_png,
    },
    runtime_publication_resolution::{
        MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
        MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES, PublicationConflictResolutionResponse,
        SignedPublicationConflictResolutionRequest,
    },
};

const CLAIM_VERSION: u8 = 1;
const CLAIM_URI_PREFIX: &str = "kilogram://publication-conflict/v1/";
const MAX_CLAIM_URI_BYTES: usize = 3_072;
const CONFIRMATION_CODE_DOMAIN: &[u8] = b"kilogram:publication-conflict-confirmation:v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PublicationConflictClaimKind {
    Request,
    Response,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationConflictQrClaim {
    version: u8,
    kind: PublicationConflictClaimKind,
    artifact_id: String,
    request_id: String,
    local_account_id: AccountId,
    authority_revision: u64,
    conflict_evidence_id: String,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    old_publication_channel_id: String,
    new_publication_channel_id: String,
    replacement_ticket_digest: String,
    confirmation_code: String,
}

impl PublicationConflictQrClaim {
    fn verify(&self) -> Result<()> {
        ensure!(
            self.version == CLAIM_VERSION,
            "unsupported publication-conflict QR claim version"
        );
        ensure!(
            !self.artifact_id.is_empty()
                && !self.request_id.is_empty()
                && !self.conflict_evidence_id.is_empty()
                && !self.old_publication_channel_id.is_empty()
                && !self.new_publication_channel_id.is_empty()
                && !self.replacement_ticket_digest.is_empty(),
            "publication-conflict QR claim has an empty security field"
        );
        ensure!(
            self.confirmation_code == confirmation_code(self),
            "publication-conflict QR confirmation code is invalid"
        );
        Ok(())
    }

    fn encode_uri(&self) -> Result<String> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self)
            .context("encode publication-conflict QR verification claim")?;
        let uri = format!("{CLAIM_URI_PREFIX}{}", URL_SAFE_NO_PAD.encode(encoded));
        ensure!(
            uri.len() <= MAX_CLAIM_URI_BYTES,
            "publication-conflict QR verification claim is too large"
        );
        Ok(uri)
    }

    fn decode_uri(uri: &str) -> Result<Self> {
        ensure!(
            uri.is_ascii() && uri.starts_with(CLAIM_URI_PREFIX) && uri.len() <= MAX_CLAIM_URI_BYTES,
            "publication-conflict QR URI shape is invalid"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(&uri[CLAIM_URI_PREFIX.len()..])
            .context("decode publication-conflict QR claim as base64url")?;
        let claim: Self = postcard::from_bytes(&bytes)
            .context("decode publication-conflict QR verification claim")?;
        claim.verify()?;
        Ok(claim)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublicationConflictRequestInspection {
    pub request_file: PathBuf,
    pub artifact_bytes: u64,
    pub artifact_digest: String,
    pub request_id: String,
    pub local_account_id: AccountId,
    pub requester_device_id: DeviceId,
    pub authority_revision: u64,
    pub conflict_proof_id: String,
    pub conflict_evidence_id: String,
    pub detector_device_id: DeviceId,
    pub detected_at_unix_seconds: u64,
    pub publication_generation: u64,
    pub peer_account_id: AccountId,
    pub peer_device_id: DeviceId,
    pub old_publication_channel_id: String,
    pub new_publication_channel_id: String,
    pub old_channel_epoch: u64,
    pub new_channel_epoch: u64,
    pub route_policy: String,
    pub replacement_ticket_digest: String,
    pub replacement_endpoint_id: String,
    pub replacement_peer_authority_revision: u64,
    pub replacement_peer_device_count: usize,
    pub requested_at_unix_seconds: u64,
    pub confirmation_code: String,
    pub verification_qr_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublicationConflictResponseInspection {
    pub response_file: PathBuf,
    pub artifact_bytes: u64,
    pub artifact_digest: String,
    pub request_id: String,
    pub resolution_id: String,
    pub local_account_id: AccountId,
    pub authority_revision: u64,
    pub conflict_evidence_id: String,
    pub peer_account_id: AccountId,
    pub peer_device_id: DeviceId,
    pub old_publication_channel_id: String,
    pub new_publication_channel_id: String,
    pub new_channel_epoch: u64,
    pub route_policy: String,
    pub replacement_ticket_digest: String,
    pub replacement_endpoint_id: String,
    pub replacement_peer_authority_revision: u64,
    pub replacement_peer_device_count: usize,
    pub authorized_at_unix_seconds: u64,
    pub confirmation_code: String,
    pub verification_qr_status: String,
}

pub(crate) struct InspectedPublicationConflictRequest {
    request: SignedPublicationConflictResolutionRequest,
    report: PublicationConflictRequestInspection,
    claim: PublicationConflictQrClaim,
}

impl InspectedPublicationConflictRequest {
    pub fn request(&self) -> &SignedPublicationConflictResolutionRequest {
        &self.request
    }

    pub fn report(&self) -> &PublicationConflictRequestInspection {
        &self.report
    }
}

pub(crate) struct InspectedPublicationConflictResponse {
    report: PublicationConflictResponseInspection,
    claim: PublicationConflictQrClaim,
}

impl InspectedPublicationConflictResponse {
    pub fn report(&self) -> &PublicationConflictResponseInspection {
        &self.report
    }
}

pub(crate) fn inspect_publication_conflict_request(
    request_file: &Path,
    verification_qr_file: Option<&Path>,
) -> Result<InspectedPublicationConflictRequest> {
    let bytes = read_bounded_regular_file(
        request_file,
        MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES as u64,
        "publication conflict resolution request",
    )?;
    let request = SignedPublicationConflictResolutionRequest::decode(&bytes)?;
    let replacement = ConnectionTicket::decode(
        std::str::from_utf8(request.replacement_ticket())
            .context("request replacement peer ticket is not UTF-8")?,
    )
    .context("verify request replacement peer ticket")?;
    ensure!(
        replacement.listener_account_id() == request.peer_account_id()
            && replacement.listener_device_id() == request.peer_device_id()
            && replacement.allowed_requester_account_id() == request.local_account_id()
            && replacement.route_policy() == request.route_policy()
            && replacement.ticket_publication_write_key() == request.new_write_key()
            && replacement.ticket_publication_channel_epoch() == request.new_channel_epoch()
            && request.new_channel_epoch() > request.old_channel_epoch(),
        "request replacement ticket does not match the Device-signed rotation summary"
    );
    let request_id = request.request_id()?.to_string();
    let evidence_id = request.conflict_proof().evidence_id()?.to_string();
    let ticket_digest = encode_hex(&request.replacement_ticket_digest());
    let mut claim = PublicationConflictQrClaim {
        version: CLAIM_VERSION,
        kind: PublicationConflictClaimKind::Request,
        artifact_id: request_id.clone(),
        request_id: request_id.clone(),
        local_account_id: request.local_account_id(),
        authority_revision: request.authority_revision(),
        conflict_evidence_id: evidence_id.clone(),
        peer_account_id: request.peer_account_id(),
        peer_device_id: request.peer_device_id(),
        old_publication_channel_id: request.old_write_key().channel_id().to_string(),
        new_publication_channel_id: request.new_write_key().channel_id().to_string(),
        replacement_ticket_digest: ticket_digest.clone(),
        confirmation_code: String::new(),
    };
    claim.confirmation_code = confirmation_code(&claim);
    claim.verify()?;
    let verification_qr_status = verify_optional_qr(verification_qr_file, &claim)?;
    let report = PublicationConflictRequestInspection {
        request_file: fs::canonicalize(request_file)
            .context("resolve inspected publication-conflict request")?,
        artifact_bytes: bytes.len() as u64,
        artifact_digest: encode_hex(blake3::hash(&bytes).as_bytes()),
        request_id,
        local_account_id: request.local_account_id(),
        requester_device_id: request.requester_device_id(),
        authority_revision: request.authority_revision(),
        conflict_proof_id: request.conflict_proof().proof_id()?.to_string(),
        conflict_evidence_id: evidence_id,
        detector_device_id: request.conflict_proof().detector_device_id(),
        detected_at_unix_seconds: request.conflict_proof().detected_at_unix_seconds(),
        publication_generation: request.conflict_proof().publication_generation(),
        peer_account_id: request.peer_account_id(),
        peer_device_id: request.peer_device_id(),
        old_publication_channel_id: request.old_write_key().channel_id().to_string(),
        new_publication_channel_id: request.new_write_key().channel_id().to_string(),
        old_channel_epoch: request.old_channel_epoch(),
        new_channel_epoch: request.new_channel_epoch(),
        route_policy: request.route_policy().as_str().to_owned(),
        replacement_ticket_digest: ticket_digest,
        replacement_endpoint_id: replacement.endpoint().id.to_string(),
        replacement_peer_authority_revision: replacement.listener_authority_snapshot().revision(),
        replacement_peer_device_count: replacement
            .listener_directory()
            .device_list()
            .devices()
            .len(),
        requested_at_unix_seconds: request.requested_at_unix_seconds(),
        confirmation_code: claim.confirmation_code.clone(),
        verification_qr_status,
    };
    Ok(InspectedPublicationConflictRequest {
        request,
        report,
        claim,
    })
}

pub(crate) fn inspect_publication_conflict_response(
    response_file: &Path,
    verification_qr_file: Option<&Path>,
) -> Result<InspectedPublicationConflictResponse> {
    let bytes = read_bounded_regular_file(
        response_file,
        MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES as u64,
        "publication conflict resolution response",
    )?;
    let response = PublicationConflictResolutionResponse::decode(&bytes)?;
    let resolution = response.resolution();
    let replacement = ConnectionTicket::decode(
        std::str::from_utf8(response.replacement_ticket())
            .context("response replacement peer ticket is not UTF-8")?,
    )
    .context("verify response replacement peer ticket")?;
    ensure!(
        replacement.listener_account_id() == resolution.peer_account_id()
            && replacement.listener_device_id() == resolution.peer_device_id()
            && replacement.allowed_requester_account_id() == resolution.local_account_id()
            && replacement.ticket_publication_write_key() == resolution.new_write_key()
            && encode_hex(blake3::hash(response.replacement_ticket()).as_bytes())
                == encode_hex(&resolution.replacement_ticket_digest()),
        "response replacement ticket does not match the Root-signed resolution"
    );
    let request_id = response.request_id().to_string();
    let resolution_id = resolution.resolution_id()?.to_string();
    let evidence_id = resolution.conflict_evidence_id().to_string();
    let ticket_digest = encode_hex(&resolution.replacement_ticket_digest());
    let mut claim = PublicationConflictQrClaim {
        version: CLAIM_VERSION,
        kind: PublicationConflictClaimKind::Response,
        artifact_id: resolution_id.clone(),
        request_id: request_id.clone(),
        local_account_id: resolution.local_account_id(),
        authority_revision: resolution.authority_revision(),
        conflict_evidence_id: evidence_id.clone(),
        peer_account_id: resolution.peer_account_id(),
        peer_device_id: resolution.peer_device_id(),
        old_publication_channel_id: resolution.old_write_key().channel_id().to_string(),
        new_publication_channel_id: resolution.new_write_key().channel_id().to_string(),
        replacement_ticket_digest: ticket_digest.clone(),
        confirmation_code: String::new(),
    };
    claim.confirmation_code = confirmation_code(&claim);
    claim.verify()?;
    let verification_qr_status = verify_optional_qr(verification_qr_file, &claim)?;
    let report = PublicationConflictResponseInspection {
        response_file: fs::canonicalize(response_file)
            .context("resolve inspected publication-conflict response")?,
        artifact_bytes: bytes.len() as u64,
        artifact_digest: encode_hex(blake3::hash(&bytes).as_bytes()),
        request_id,
        resolution_id,
        local_account_id: resolution.local_account_id(),
        authority_revision: resolution.authority_revision(),
        conflict_evidence_id: evidence_id,
        peer_account_id: resolution.peer_account_id(),
        peer_device_id: resolution.peer_device_id(),
        old_publication_channel_id: resolution.old_write_key().channel_id().to_string(),
        new_publication_channel_id: resolution.new_write_key().channel_id().to_string(),
        new_channel_epoch: replacement.ticket_publication_channel_epoch(),
        route_policy: replacement.route_policy().as_str().to_owned(),
        replacement_ticket_digest: ticket_digest,
        replacement_endpoint_id: replacement.endpoint().id.to_string(),
        replacement_peer_authority_revision: replacement.listener_authority_snapshot().revision(),
        replacement_peer_device_count: replacement
            .listener_directory()
            .device_list()
            .devices()
            .len(),
        authorized_at_unix_seconds: resolution.authorized_at_unix_seconds(),
        confirmation_code: claim.confirmation_code.clone(),
        verification_qr_status,
    };
    Ok(InspectedPublicationConflictResponse { report, claim })
}

pub(crate) fn render_request_verification_qr(
    inspected: &InspectedPublicationConflictRequest,
    output_file: &Path,
) -> Result<RecoveryQrRenderReport> {
    render_claim_qr(&inspected.claim, output_file)
}

pub(crate) fn render_response_verification_qr(
    inspected: &InspectedPublicationConflictResponse,
    output_file: &Path,
) -> Result<RecoveryQrRenderReport> {
    render_claim_qr(&inspected.claim, output_file)
}

fn render_claim_qr(
    claim: &PublicationConflictQrClaim,
    output_file: &Path,
) -> Result<RecoveryQrRenderReport> {
    render_bounded_ascii_qr_png(
        &claim.encode_uri()?,
        output_file,
        CLAIM_URI_PREFIX,
        MAX_CLAIM_URI_BYTES,
        "publication-conflict verification claim",
    )
}

fn verify_optional_qr(
    verification_qr_file: Option<&Path>,
    expected: &PublicationConflictQrClaim,
) -> Result<String> {
    let Some(path) = verification_qr_file else {
        return Ok("not-provided".to_owned());
    };
    let decoded = decode_bounded_ascii_qr_image(
        path,
        CLAIM_URI_PREFIX,
        MAX_CLAIM_URI_BYTES,
        "publication-conflict verification claim",
    )?;
    let claim = PublicationConflictQrClaim::decode_uri(&decoded.payload)?;
    ensure!(
        claim == *expected,
        "publication-conflict verification QR does not match the exact signed artifact"
    );
    Ok("exact-match".to_owned())
}

fn confirmation_code(claim: &PublicationConflictQrClaim) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIRMATION_CODE_DOMAIN);
    let authority_revision = claim.authority_revision.to_be_bytes();
    for value in [
        claim.request_id.as_bytes(),
        claim.local_account_id.as_bytes(),
        authority_revision.as_slice(),
        claim.conflict_evidence_id.as_bytes(),
        claim.peer_account_id.as_bytes(),
        claim.peer_device_id.as_bytes(),
        claim.old_publication_channel_id.as_bytes(),
        claim.new_publication_channel_id.as_bytes(),
        claim.replacement_ticket_digest.as_bytes(),
    ] {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    let digest = hasher.finalize();
    let mut code = String::from("KPC1");
    for chunk in digest.as_bytes()[..12].as_chunks::<2>().0 {
        let _ = write!(code, "-{:02X}{:02X}", chunk[0], chunk[1]);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_code_is_common_to_request_and_response_but_claims_are_distinct() -> Result<()> {
        let common = PublicationConflictQrClaim {
            version: CLAIM_VERSION,
            kind: PublicationConflictClaimKind::Request,
            artifact_id: "request".to_owned(),
            request_id: "11".repeat(32),
            local_account_id: AccountId::from_bytes([1; 32]),
            authority_revision: 7,
            conflict_evidence_id: "22".repeat(32),
            peer_account_id: AccountId::from_bytes([2; 32]),
            peer_device_id: DeviceId::from_bytes([3; 32]),
            old_publication_channel_id: "33".repeat(32),
            new_publication_channel_id: "44".repeat(32),
            replacement_ticket_digest: "55".repeat(32),
            confirmation_code: String::new(),
        };
        let mut request = common.clone();
        request.confirmation_code = confirmation_code(&request);
        request.verify()?;
        let mut response = common;
        response.kind = PublicationConflictClaimKind::Response;
        response.artifact_id = "response".to_owned();
        response.confirmation_code = confirmation_code(&response);
        response.verify()?;
        assert_eq!(request.confirmation_code, response.confirmation_code);
        assert_ne!(request.encode_uri()?, response.encode_uri()?);
        assert_eq!(
            PublicationConflictQrClaim::decode_uri(&request.encode_uri()?)?,
            request
        );
        Ok(())
    }
}
