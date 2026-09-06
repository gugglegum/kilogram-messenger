use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId};
use kilogram_publication_conflict::{
    MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES, PUBLICATION_CONFLICT_CLAIM_URI_PREFIX,
    PublicationConflictQrClaim,
};

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
            && replacement.route_policy().as_str() == request.route_policy().as_str()
            && replacement.ticket_publication_write_key() == request.new_write_key()
            && replacement.ticket_publication_channel_epoch() == request.new_channel_epoch()
            && request.new_channel_epoch() > request.old_channel_epoch(),
        "request replacement ticket does not match the Device-signed rotation summary"
    );
    let request_id = request.request_id()?.to_string();
    let evidence_id = request.conflict_proof().evidence_id()?.to_string();
    let ticket_digest = encode_hex(&request.replacement_ticket_digest());
    let claim = PublicationConflictQrClaim::for_request(&request)?;
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
        confirmation_code: claim.confirmation_code().to_owned(),
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
    let claim = PublicationConflictQrClaim::for_response(&response)?;
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
        confirmation_code: claim.confirmation_code().to_owned(),
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
        PUBLICATION_CONFLICT_CLAIM_URI_PREFIX,
        MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES,
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
        PUBLICATION_CONFLICT_CLAIM_URI_PREFIX,
        MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES,
        "publication-conflict verification claim",
    )?;
    let claim = PublicationConflictQrClaim::decode_uri(&decoded.payload)?;
    ensure!(
        claim == *expected,
        "publication-conflict verification QR does not match the exact signed artifact"
    );
    Ok("exact-match".to_owned())
}
