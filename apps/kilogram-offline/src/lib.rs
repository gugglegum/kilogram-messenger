#![forbid(unsafe_code)]

use std::{
    fs::{self, File},
    io::{BufReader, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use image::{
    ColorType, ImageEncoder, ImageFormat, ImageReader, Limits, Luma, codecs::png::PngEncoder,
};
use kilogram_identity::{
    AccountId, AccountRootState, DeviceCapability, DeviceCertificate, DeviceId,
    verify_device_authorization_with_snapshot,
};
use kilogram_publication_conflict::{
    MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES, MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
    MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES, PUBLICATION_CONFLICT_CLAIM_URI_PREFIX,
    PublicationConflictQrClaim, PublicationConflictResolutionResponse,
    PublicationConflictRoutePolicy, RootSignedPublicationConflictResolution,
    SignedPublicationConflictResolutionRequest,
};
use kilogram_ratchet::AccountPrekeyDirectory;
use kilogram_ticket_publication::TicketPublicationWriteKey;
use qrcode::{EcLevel, QrCode, Version};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tempfile::NamedTempFile;

const TICKET_VERSION: u8 = 11;
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v11\0";
const QR_MODULE_PIXELS: u32 = 4;
const MAX_QR_IMAGE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_QR_IMAGE_DIMENSION: u32 = 4_096;
const MAX_QR_IMAGE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TICKET_ENDPOINT_ADDRESSES: usize = 64;
const MAX_TICKET_ENDPOINT_TEXT_BYTES: usize = 2_048;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointSummary {
    id: String,
    addrs: Vec<EndpointAddressSummary>,
}

#[derive(Debug, Deserialize)]
enum EndpointAddressSummary {
    Relay(String),
    Ip(String),
}

#[derive(Debug, Deserialize, Serialize)]
struct OfflineConnectionTicketContent {
    version: u8,
    endpoint: Box<RawValue>,
    listener_certificate: DeviceCertificate,
    listener_directory: AccountPrekeyDirectory,
    allowed_requester_account_id: AccountId,
    ticket_publication_write_key: TicketPublicationWriteKey,
    ticket_publication_channel_epoch: u64,
    route_policy: PublicationConflictRoutePolicy,
}

#[derive(Debug, Deserialize, Serialize)]
struct OfflineConnectionTicket {
    content: OfflineConnectionTicketContent,
    signature: Vec<u8>,
}

impl OfflineConnectionTicket {
    fn decode(encoded: &str) -> Result<Self> {
        ensure!(
            !encoded.trim().is_empty()
                && encoded.trim().len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
            "connection ticket text size is invalid"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode connection ticket as base64url")?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
            "connection ticket payload size is invalid"
        );
        let ticket: Self =
            serde_json::from_slice(&bytes).context("decode connection ticket payload")?;
        ticket.verify()?;
        Ok(ticket)
    }

    fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == TICKET_VERSION,
            "unsupported connection ticket version: {}",
            self.content.version
        );
        self.endpoint_summary()?;
        self.content
            .ticket_publication_write_key
            .verify()
            .context("verify ticket publication write key")?;
        self.content.listener_certificate.verify()?;
        self.content.listener_directory.verify()?;
        self.content
            .listener_directory
            .verify_at(unix_time_now()?)?;
        ensure!(
            self.content.listener_directory.account_id()
                == self.content.listener_certificate.account_id(),
            "connection ticket directory does not belong to the listener account"
        );
        ensure!(
            self.content
                .listener_directory
                .certificate_for(self.content.listener_certificate.device_id())
                == Some(&self.content.listener_certificate),
            "connection ticket listener certificate is absent from its signed device list"
        );
        self.content
            .listener_certificate
            .device_id()
            .verify(&ticket_signing_bytes(&self.content)?, &self.signature)
            .context("verify listener signature on connection ticket")
    }

    fn endpoint_summary(&self) -> Result<EndpointSummary> {
        let endpoint: EndpointSummary = serde_json::from_str(self.content.endpoint.get())
            .context("decode public endpoint summary")?;
        ensure!(
            endpoint.id.len() == 64 && endpoint.id.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "connection ticket endpoint ID shape is invalid"
        );
        ensure!(
            endpoint.addrs.len() <= MAX_TICKET_ENDPOINT_ADDRESSES,
            "connection ticket contains too many endpoint addresses"
        );
        for address in &endpoint.addrs {
            match address {
                EndpointAddressSummary::Relay(url) => ensure!(
                    url.is_ascii()
                        && !url.is_empty()
                        && url.len() <= MAX_TICKET_ENDPOINT_TEXT_BYTES
                        && (url.starts_with("https://") || url.starts_with("http://")),
                    "connection ticket relay URL shape is invalid"
                ),
                EndpointAddressSummary::Ip(address) => {
                    ensure!(
                        address.len() <= MAX_TICKET_ENDPOINT_TEXT_BYTES,
                        "connection ticket IP endpoint is too large"
                    );
                    address
                        .parse::<SocketAddr>()
                        .context("parse connection ticket IP endpoint")?;
                }
            }
        }
        Ok(endpoint)
    }

    fn listener_account_id(&self) -> AccountId {
        self.content.listener_certificate.account_id()
    }

    fn listener_device_id(&self) -> DeviceId {
        self.content.listener_certificate.device_id()
    }

    fn allowed_requester_account_id(&self) -> AccountId {
        self.content.allowed_requester_account_id
    }

    fn publication_write_key(&self) -> TicketPublicationWriteKey {
        self.content.ticket_publication_write_key
    }

    fn publication_channel_epoch(&self) -> u64 {
        self.content.ticket_publication_channel_epoch
    }

    fn route_policy(&self) -> PublicationConflictRoutePolicy {
        self.content.route_policy
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationConflictRequestInspection {
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
pub struct PublicationConflictResponseInspection {
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

pub struct InspectedPublicationConflictRequest {
    request: SignedPublicationConflictResolutionRequest,
    report: PublicationConflictRequestInspection,
    claim: PublicationConflictQrClaim,
}

impl InspectedPublicationConflictRequest {
    pub fn report(&self) -> &PublicationConflictRequestInspection {
        &self.report
    }
}

pub struct InspectedPublicationConflictResponse {
    report: PublicationConflictResponseInspection,
    claim: PublicationConflictQrClaim,
}

impl InspectedPublicationConflictResponse {
    pub fn report(&self) -> &PublicationConflictResponseInspection {
        &self.report
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationConflictAuthorizationReport {
    pub request_id: String,
    pub resolution_id: String,
    pub conflict_evidence_id: String,
    pub old_publication_channel_id: String,
    pub new_publication_channel_id: String,
    pub authority_revision: u64,
    pub confirmation_code: String,
    pub verification_qr_status: String,
    pub authorized_at_unix_seconds: u64,
    pub response_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrRenderReport {
    pub qr_version: i16,
    pub module_count: usize,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub png_bytes: usize,
}

pub fn inspect_publication_conflict_request(
    request_file: &Path,
    verification_qr_file: Option<&Path>,
) -> Result<InspectedPublicationConflictRequest> {
    let bytes = read_bounded_regular_file(
        request_file,
        MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES as u64,
        "publication conflict resolution request",
    )?;
    let request = SignedPublicationConflictResolutionRequest::decode(&bytes)?;
    let replacement = OfflineConnectionTicket::decode(
        std::str::from_utf8(request.replacement_ticket())
            .context("request replacement peer ticket is not UTF-8")?,
    )
    .context("verify request replacement peer ticket")?;
    ensure!(
        replacement.listener_account_id() == request.peer_account_id()
            && replacement.listener_device_id() == request.peer_device_id()
            && replacement.allowed_requester_account_id() == request.local_account_id()
            && replacement.route_policy() == request.route_policy()
            && replacement.publication_write_key() == request.new_write_key()
            && replacement.publication_channel_epoch() == request.new_channel_epoch()
            && request.new_channel_epoch() > request.old_channel_epoch(),
        "request replacement ticket does not match the Device-signed rotation summary"
    );
    let endpoint = replacement.endpoint_summary()?;
    let request_id = request.request_id()?.to_string();
    let evidence_id = request.conflict_proof().evidence_id()?.to_string();
    let ticket_digest = encode_hex(&request.replacement_ticket_digest());
    let claim = PublicationConflictQrClaim::for_request(&request)?;
    let verification_qr_status = verify_optional_qr(verification_qr_file, &claim)?;
    let proof = request.conflict_proof();
    let report = PublicationConflictRequestInspection {
        request_file: fs::canonicalize(request_file)
            .context("resolve inspected publication-conflict request")?,
        artifact_bytes: bytes.len() as u64,
        artifact_digest: encode_hex(blake3::hash(&bytes).as_bytes()),
        request_id,
        local_account_id: request.local_account_id(),
        requester_device_id: request.requester_device_id(),
        authority_revision: request.authority_revision(),
        conflict_proof_id: proof.proof_id()?.to_string(),
        conflict_evidence_id: evidence_id,
        detector_device_id: proof.detector_device_id(),
        detected_at_unix_seconds: proof.detected_at_unix_seconds(),
        publication_generation: proof.publication_generation(),
        peer_account_id: request.peer_account_id(),
        peer_device_id: request.peer_device_id(),
        old_publication_channel_id: request.old_write_key().channel_id().to_string(),
        new_publication_channel_id: request.new_write_key().channel_id().to_string(),
        old_channel_epoch: request.old_channel_epoch(),
        new_channel_epoch: request.new_channel_epoch(),
        route_policy: request.route_policy().as_str().to_owned(),
        replacement_ticket_digest: ticket_digest,
        replacement_endpoint_id: endpoint.id,
        replacement_peer_authority_revision: replacement
            .content
            .listener_directory
            .authority_snapshot()
            .revision(),
        replacement_peer_device_count: replacement
            .content
            .listener_directory
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

pub fn inspect_publication_conflict_response(
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
    let replacement = OfflineConnectionTicket::decode(
        std::str::from_utf8(response.replacement_ticket())
            .context("response replacement peer ticket is not UTF-8")?,
    )
    .context("verify response replacement peer ticket")?;
    ensure!(
        replacement.listener_account_id() == resolution.peer_account_id()
            && replacement.listener_device_id() == resolution.peer_device_id()
            && replacement.allowed_requester_account_id() == resolution.local_account_id()
            && replacement.publication_write_key() == resolution.new_write_key()
            && *blake3::hash(response.replacement_ticket()).as_bytes()
                == resolution.replacement_ticket_digest(),
        "response replacement ticket does not match the Root-signed resolution"
    );
    let endpoint = replacement.endpoint_summary()?;
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
        new_channel_epoch: replacement.publication_channel_epoch(),
        route_policy: replacement.route_policy().as_str().to_owned(),
        replacement_ticket_digest: ticket_digest,
        replacement_endpoint_id: endpoint.id,
        replacement_peer_authority_revision: replacement
            .content
            .listener_directory
            .authority_snapshot()
            .revision(),
        replacement_peer_device_count: replacement
            .content
            .listener_directory
            .device_list()
            .devices()
            .len(),
        authorized_at_unix_seconds: resolution.authorized_at_unix_seconds(),
        confirmation_code: claim.confirmation_code().to_owned(),
        verification_qr_status,
    };
    Ok(InspectedPublicationConflictResponse { report, claim })
}

pub fn authorize_publication_conflict(
    account_directory: &Path,
    request_file: &Path,
    confirm_code: &str,
    verification_qr_file: Option<&Path>,
    output_file: &Path,
) -> Result<PublicationConflictAuthorizationReport> {
    let inspected = inspect_publication_conflict_request(request_file, verification_qr_file)?;
    ensure!(
        confirm_code
            .trim()
            .eq_ignore_ascii_case(&inspected.report.confirmation_code),
        "operator confirmation code does not match the exact inspected request; Account Root was not loaded"
    );
    let canonical_account_directory =
        fs::canonicalize(account_directory).context("resolve offline Account Root directory")?;
    ensure!(
        !inspected
            .report
            .request_file
            .starts_with(&canonical_account_directory),
        "untrusted conflict request must live outside the offline Account Root directory"
    );
    let output_file = absolute_new_external_path(&canonical_account_directory, output_file)?;
    let root = AccountRootState::load(&canonical_account_directory)
        .context("load offline Account Root for publication conflict resolution")?;
    let request = &inspected.request;
    let authority = root.authority_snapshot()?;
    let devices = root.published_device_list()?;
    ensure!(
        request.local_account_id() == root.account_id()
            && request.authority_revision() == authority.revision()
            && devices.authority_snapshot() == &authority,
        "resolution request is not for the exact current offline Root authority"
    );
    let requester_certificate = devices
        .certificate_for(request.requester_device_id())
        .context("resolution requester is not an exact-current Account Device")?;
    verify_device_authorization_with_snapshot(
        root.account_id(),
        requester_certificate,
        &authority,
        &DeviceCapability::MESSAGING,
    )
    .context("resolution requester is not currently authorized for messaging")?;
    let detector_id = request.conflict_proof().detector_device_id();
    let detector_certificate = devices
        .certificate_for(detector_id)
        .context("conflict detector is not an exact-current Account Device")?;
    verify_device_authorization_with_snapshot(
        root.account_id(),
        detector_certificate,
        &authority,
        &DeviceCapability::MESSAGING,
    )
    .context("conflict detector is not currently authorized for messaging")?;
    let authorized_at_unix_seconds = unix_time_now()?;
    let resolution = RootSignedPublicationConflictResolution::sign(
        &root,
        request.request_id()?,
        authority.revision(),
        request.conflict_proof().evidence_id()?,
        request.peer_account_id(),
        request.peer_device_id(),
        request.old_write_key(),
        request.new_write_key(),
        request.replacement_ticket_digest(),
        authorized_at_unix_seconds,
    )?;
    let response = PublicationConflictResolutionResponse::new(request, resolution.clone())?;
    write_new_file(&output_file, &response.encode()?)?;
    Ok(PublicationConflictAuthorizationReport {
        request_id: request.request_id()?.to_string(),
        resolution_id: resolution.resolution_id()?.to_string(),
        conflict_evidence_id: request.conflict_proof().evidence_id()?.to_string(),
        old_publication_channel_id: request.old_write_key().channel_id().to_string(),
        new_publication_channel_id: request.new_write_key().channel_id().to_string(),
        authority_revision: authority.revision(),
        confirmation_code: inspected.report.confirmation_code,
        verification_qr_status: inspected.report.verification_qr_status,
        authorized_at_unix_seconds,
        response_file: fs::canonicalize(&output_file)
            .context("resolve new publication conflict response")?,
    })
}

pub fn render_request_verification_qr(
    inspected: &InspectedPublicationConflictRequest,
    output_file: &Path,
) -> Result<QrRenderReport> {
    render_claim_qr(&inspected.claim, output_file)
}

pub fn render_response_verification_qr(
    inspected: &InspectedPublicationConflictResponse,
    output_file: &Path,
) -> Result<QrRenderReport> {
    render_claim_qr(&inspected.claim, output_file)
}

fn render_claim_qr(claim: &PublicationConflictQrClaim, output: &Path) -> Result<QrRenderReport> {
    let payload = claim.encode_uri()?;
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::L)
        .context("encode publication-conflict verification claim as QR")?;
    let qr_version = match code.version() {
        Version::Normal(version) => version,
        Version::Micro(_) => bail!("publication-conflict claim unexpectedly encoded as Micro QR"),
    };
    let module_count = code.width();
    let image = code
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(QR_MODULE_PIXELS, QR_MODULE_PIXELS)
        .build();
    let pixel_width = image.width();
    let pixel_height = image.height();
    ensure!(
        pixel_width <= MAX_QR_IMAGE_DIMENSION && pixel_height <= MAX_QR_IMAGE_DIMENSION,
        "rendered publication-conflict QR exceeds the image dimension limit"
    );
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        image.as_raw(),
        pixel_width,
        pixel_height,
        ColorType::L8.into(),
    )?;
    ensure!(
        png.len() as u64 <= MAX_QR_IMAGE_FILE_BYTES,
        "rendered publication-conflict QR exceeds the PNG size limit"
    );
    persist_noclobber(output, &png)?;
    Ok(QrRenderReport {
        qr_version,
        module_count,
        pixel_width,
        pixel_height,
        png_bytes: png.len(),
    })
}

fn verify_optional_qr(
    verification_qr_file: Option<&Path>,
    expected: &PublicationConflictQrClaim,
) -> Result<String> {
    let Some(path) = verification_qr_file else {
        return Ok("not-provided".to_owned());
    };
    let payload = decode_qr(path)?;
    let claim = PublicationConflictQrClaim::decode_uri(&payload)?;
    ensure!(
        claim == *expected,
        "publication-conflict verification QR does not match the exact signed artifact"
    );
    Ok("exact-match".to_owned())
}

fn decode_qr(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "read publication-conflict QR metadata from {}",
            path.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "publication-conflict QR input is not a regular non-symlink file"
    );
    ensure!(
        metadata.len() > 0 && metadata.len() <= MAX_QR_IMAGE_FILE_BYTES,
        "publication-conflict QR image size is invalid"
    );
    let file = File::open(path)
        .with_context(|| format!("open publication-conflict QR image {}", path.display()))?;
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .context("detect publication-conflict QR image format")?;
    ensure!(
        matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)),
        "publication-conflict QR must be PNG or JPEG"
    );
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_QR_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_QR_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_QR_IMAGE_ALLOC_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .context("decode bounded publication-conflict QR image")?;
    let mut prepared = rqrr::PreparedImage::prepare(image.to_luma8());
    let grids = prepared.detect_grids();
    ensure!(
        grids.len() == 1,
        "publication-conflict QR image must contain exactly one QR code; detected {}",
        grids.len()
    );
    let (_, payload) = grids[0]
        .decode()
        .context("decode publication-conflict QR payload")?;
    ensure!(
        payload.is_ascii()
            && payload.starts_with(PUBLICATION_CONFLICT_CLAIM_URI_PREFIX)
            && payload.len() <= MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES,
        "publication-conflict QR payload shape is invalid"
    );
    Ok(payload)
}

fn ticket_signing_bytes(content: &OfflineConnectionTicketContent) -> Result<Vec<u8>> {
    let encoded = serde_json::to_vec(content).context("serialize connection ticket content")?;
    let mut bytes = Vec::with_capacity(TICKET_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(TICKET_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn read_bounded_regular_file(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() > 0
            && metadata.len() <= max_bytes,
        "{label} must be a non-empty bounded regular non-symlink file"
    );
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}

fn absolute_new_external_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for offline output")?
            .join(path)
    };
    let parent = absolute.parent().context("offline output has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create offline output directory {}", parent.display()))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("resolve offline output directory {}", parent.display()))?;
    let file_name = absolute
        .file_name()
        .context("offline output has no file name")?;
    let resolved = parent.join(file_name);
    ensure!(
        !resolved.starts_with(root),
        "offline response output must live outside the Account Root directory"
    );
    ensure!(
        !resolved.exists(),
        "offline response output already exists: {}",
        resolved.display()
    );
    Ok(resolved)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("offline output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary offline response in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .context("write temporary offline response")?;
    temporary
        .as_file()
        .sync_all()
        .context("flush temporary offline response")?;
    match temporary.persist_noclobber(path) {
        Ok(file) => file
            .sync_all()
            .with_context(|| format!("flush offline response {}", path.display())),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("offline response output already exists: {}", path.display())
        }
        Err(error) => {
            Err(error.error).with_context(|| format!("publish offline response {}", path.display()))
        }
    }
}

fn persist_noclobber(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create offline QR directory {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary offline QR in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .context("write temporary offline QR")?;
    temporary
        .as_file()
        .sync_all()
        .context("flush temporary offline QR")?;
    match temporary.persist_noclobber(path) {
        Ok(file) => file
            .sync_all()
            .with_context(|| format!("flush offline QR {}", path.display())),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("offline QR output already exists: {}", path.display())
        }
        Err(error) => {
            Err(error.error).with_context(|| format!("publish offline QR {}", path.display()))
        }
    }
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time precedes Unix epoch")?
        .as_secs())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
