use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use kilogram_crypto::SealedMessage;
use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, AccountRootState, DeviceCapability, DeviceCertificate,
    DeviceId, DeviceState, EncryptionPublicKey,
};
use kilogram_ratchet::{
    DEFAULT_PREKEY_POOL_SIZE, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS, RatchetState, SignedPrekeyPool,
};
use kilogram_state::{
    DeviceIdentityStateRepository, EncryptedStateVault, StateDirectoryLock, StateMirrorRepository,
    StateTransaction, TrustStateRepository, VaultPrimaryWriteRepository,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const REQUEST_VERSION: u8 = 1;
const RESPONSE_VERSION: u8 = 1;
const RECEIPT_VERSION: u8 = 1;
const REQUEST_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-link-request-signature:v1\0";
const REQUEST_ID_DOMAIN: &[u8] = b"kilogram:device-link-request-id:v1\0";
const RESPONSE_INFO: &[u8] = b"kilogram:device-link-response-hpke:v1\0";
const RESPONSE_AAD_DOMAIN: &[u8] = b"kilogram:device-link-response-aad:v1\0";
const MAX_ARTIFACT_BYTES: usize = 256 * 1024;
const DEFAULT_REQUEST_VALIDITY_SECONDS: u64 = 600;
const MAX_REQUEST_VALIDITY_SECONDS: u64 = 1_800;
const CLOCK_SKEW_SECONDS: u64 = 120;
const DEVICE_STATE_DIRECTORY: &str = "device";
const PUBLIC_DIRECTORY: &str = "public";
const DEVICE_LINK_DIRECTORY: &str = "device-link";
const REQUEST_FILE: &str = "request.kdl";
const DEVICE_CERTIFICATE_FILE: &str = "device-certificate.cert";
const ACCOUNT_DEVICE_LIST_FILE: &str = "account-device-list.snapshot";
const PREKEY_POOL_FILE: &str = "prekey-pool.bin";
const REQUEST_RECEIPT_FILE: &str = "request-receipt.json";
const ACCEPT_RECEIPT_FILE: &str = "accept-receipt.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceLinkRequestContent {
    version: u8,
    account_id: AccountId,
    nonce: [u8; 32],
    device_id: DeviceId,
    encryption_public_key: EncryptionPublicKey,
    prekey_pool: Vec<u8>,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SignedDeviceLinkRequest {
    content: DeviceLinkRequestContent,
    signature: Vec<u8>,
}

impl SignedDeviceLinkRequest {
    fn issue(
        device: &DeviceState,
        account_id: AccountId,
        prekey_pool: Vec<u8>,
        now: u64,
        validity_seconds: u64,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_REQUEST_VALIDITY_SECONDS).contains(&validity_seconds),
            "device-link request validity must be between 1 and {MAX_REQUEST_VALIDITY_SECONDS} seconds"
        );
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).context("generate device-link request nonce")?;
        let content = DeviceLinkRequestContent {
            version: REQUEST_VERSION,
            account_id,
            nonce,
            device_id: device.identity().device_id(),
            encryption_public_key: device.encryption().public_key(),
            prekey_pool,
            issued_at_unix_seconds: now,
            expires_at_unix_seconds: now
                .checked_add(validity_seconds)
                .context("device-link request expiry overflow")?,
        };
        let signature = device
            .identity()
            .sign(&request_signing_bytes(&content)?)
            .to_vec();
        let request = Self { content, signature };
        request.verify(None)?;
        Ok(request)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "device-link request is too large"
        );
        let request: Self = postcard::from_bytes(bytes).context("decode device-link request")?;
        request.verify(None)?;
        Ok(request)
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.verify(None)?;
        let bytes = postcard::to_allocvec(self).context("encode device-link request")?;
        ensure!(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "device-link request is too large"
        );
        Ok(bytes)
    }

    fn verify(&self, now: Option<u64>) -> Result<()> {
        ensure!(
            self.content.version == REQUEST_VERSION,
            "unsupported device-link request version"
        );
        ensure!(
            self.content.issued_at_unix_seconds < self.content.expires_at_unix_seconds,
            "device-link request has an empty validity interval"
        );
        ensure!(
            self.content
                .expires_at_unix_seconds
                .saturating_sub(self.content.issued_at_unix_seconds)
                <= MAX_REQUEST_VALIDITY_SECONDS,
            "device-link request validity is too long"
        );
        if let Some(now) = now {
            ensure!(
                now.saturating_add(CLOCK_SKEW_SECONDS) >= self.content.issued_at_unix_seconds,
                "device-link request is not yet valid"
            );
            ensure!(
                now <= self
                    .content
                    .expires_at_unix_seconds
                    .saturating_add(CLOCK_SKEW_SECONDS),
                "device-link request has expired"
            );
        }
        let pool = SignedPrekeyPool::decode(&self.content.prekey_pool)
            .context("verify requested device prekey pool")?;
        if let Some(now) = now {
            pool.verify_at(now)
                .context("verify requested device prekey pool freshness")?;
        }
        ensure!(
            pool.device_id() == self.content.device_id,
            "prekey pool belongs to another device"
        );
        self.content
            .device_id
            .verify(&request_signing_bytes(&self.content)?, &self.signature)
            .context("verify device-link request signature")?;
        Ok(())
    }

    fn request_id(&self) -> Result<[u8; 32]> {
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(REQUEST_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    fn request_id_hex(&self) -> Result<String> {
        Ok(hex(&self.request_id()?))
    }

    fn sas(&self) -> Result<String> {
        let digest = self.request_id()?;
        let value = u64::from_le_bytes(digest[..8].try_into().context("read SAS digest")?)
            % 1_000_000_000_000;
        let digits = format!("{value:012}");
        Ok(format!(
            "{}-{}-{}",
            &digits[..4],
            &digits[4..8],
            &digits[8..]
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceLinkAuthorizationContent {
    version: u8,
    account_id: AccountId,
    request_id: [u8; 32],
    device_certificate: DeviceCertificate,
    device_list: AccountDeviceListSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SignedDeviceLinkAuthorization {
    content: DeviceLinkAuthorizationContent,
    signature: Vec<u8>,
}

impl SignedDeviceLinkAuthorization {
    fn issue(
        root: &AccountRootState,
        request: &SignedDeviceLinkRequest,
        certificate: DeviceCertificate,
        device_list: AccountDeviceListSnapshot,
    ) -> Result<Self> {
        let content = DeviceLinkAuthorizationContent {
            version: RESPONSE_VERSION,
            account_id: root.account_id(),
            request_id: request.request_id()?,
            device_certificate: certificate,
            device_list,
        };
        let canonical = authorization_signing_bytes(&content)?;
        let authorization = Self {
            content,
            signature: root.sign_device_link_authorization(&canonical),
        };
        authorization.verify(request)?;
        Ok(authorization)
    }

    fn verify(&self, request: &SignedDeviceLinkRequest) -> Result<()> {
        ensure!(
            self.content.version == RESPONSE_VERSION,
            "unsupported device-link response version"
        );
        ensure!(
            self.content.account_id == request.content.account_id,
            "response account does not match request"
        );
        ensure!(
            self.content.request_id == request.request_id()?,
            "response is bound to another request"
        );
        let certificate = &self.content.device_certificate;
        certificate
            .verify_for_account(self.content.account_id)
            .context("verify enrolled device certificate")?;
        ensure!(
            certificate.device_id() == request.content.device_id,
            "response certificate belongs to another device"
        );
        ensure!(
            certificate.encryption_public_key() == request.content.encryption_public_key,
            "response certificate has another encryption key"
        );
        ensure!(
            certificate.capabilities() == DeviceCapability::MESSAGING,
            "response certificate has unexpected capabilities"
        );
        self.content
            .device_list
            .verify_for_account(self.content.account_id)
            .context("verify response device list")?;
        ensure!(
            self.content
                .device_list
                .certificate_for(certificate.device_id())
                == Some(certificate),
            "response device list does not contain the exact certificate"
        );
        self.content
            .account_id
            .verify_device_link_authorization(
                &authorization_signing_bytes(&self.content)?,
                &self.signature,
            )
            .context("verify Account Root device-link authorization")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct EncryptedDeviceLinkResponse {
    version: u8,
    account_id: AccountId,
    request_id: [u8; 32],
    sealed: SealedMessage,
}

impl EncryptedDeviceLinkResponse {
    fn issue(
        request: &SignedDeviceLinkRequest,
        authorization: &SignedDeviceLinkAuthorization,
    ) -> Result<Self> {
        let account_id = request.content.account_id;
        let request_id = request.request_id()?;
        let aad = response_aad(account_id, request_id)?;
        let plaintext =
            postcard::to_allocvec(authorization).context("encode device-link authorization")?;
        let sealed = request
            .content
            .encryption_public_key
            .seal(&plaintext, RESPONSE_INFO, &aad)
            .context("encrypt device-link authorization for requested device")?;
        Ok(Self {
            version: RESPONSE_VERSION,
            account_id,
            request_id,
            sealed,
        })
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "device-link response is too large"
        );
        let response = postcard::from_bytes(bytes).context("decode device-link response")?;
        Ok(response)
    }

    fn encode(&self) -> Result<Vec<u8>> {
        ensure!(
            self.version == RESPONSE_VERSION,
            "unsupported device-link response version"
        );
        let bytes = postcard::to_allocvec(self).context("encode encrypted device-link response")?;
        ensure!(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "device-link response is too large"
        );
        Ok(bytes)
    }

    fn open(
        &self,
        request: &SignedDeviceLinkRequest,
        device: &DeviceState,
    ) -> Result<SignedDeviceLinkAuthorization> {
        ensure!(
            self.version == RESPONSE_VERSION,
            "unsupported device-link response version"
        );
        ensure!(
            self.account_id == request.content.account_id,
            "response account does not match local request"
        );
        ensure!(
            self.request_id == request.request_id()?,
            "response is bound to another local request"
        );
        let aad = response_aad(self.account_id, self.request_id)?;
        let plaintext = device
            .encryption()
            .open(&self.sealed, RESPONSE_INFO, &aad)
            .context("decrypt device-link response with local device key")?;
        let authorization: SignedDeviceLinkAuthorization = postcard::from_bytes(&plaintext)
            .context("decode decrypted device-link authorization")?;
        authorization.verify(request)?;
        Ok(authorization)
    }
}

#[derive(Debug, Serialize)]
pub struct DeviceLinkRequestOutput {
    status: &'static str,
    account_id: String,
    device_id: String,
    request_id: String,
    sas: String,
    expires_at_unix_seconds: u64,
    workspace_dir: PathBuf,
    state_dir: PathBuf,
    request_file: PathBuf,
    prekey_pool_file: PathBuf,
    vault_key_protection: String,
}

impl DeviceLinkRequestOutput {
    pub fn sas(&self) -> &str {
        &self.sas
    }

    pub fn request_file(&self) -> &Path {
        &self.request_file
    }
}

#[derive(Debug, Serialize)]
pub struct DeviceLinkInspectOutput {
    status: &'static str,
    account_id: String,
    device_id: String,
    request_id: String,
    sas: String,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    request_fresh: bool,
}

#[derive(Debug, Serialize)]
pub struct DeviceLinkAuthorizeOutput {
    status: &'static str,
    account_id: String,
    device_id: String,
    request_id: String,
    authority_revision: u64,
    response_file: PathBuf,
    device_list_file: PathBuf,
    response_encrypted_for_device: bool,
}

#[derive(Debug, Serialize)]
pub struct DeviceLinkAcceptOutput {
    status: &'static str,
    account_id: String,
    device_id: String,
    request_id: String,
    authority_revision: u64,
    workspace_dir: PathBuf,
    state_dir: PathBuf,
    certificate_file: PathBuf,
    device_list_file: PathBuf,
    prekey_pool_file: PathBuf,
    vault_key_protection: String,
    history_recovery: &'static str,
}

impl DeviceLinkAcceptOutput {
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RequestReceipt {
    version: u8,
    account_id: String,
    device_id: String,
    request_id: String,
    expires_at_unix_seconds: u64,
    recovery_scope: &'static str,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptReceipt {
    version: u8,
    account_id: String,
    device_id: String,
    request_id: String,
    authority_revision: u64,
    history_recovery: &'static str,
}

pub fn create_request(
    workspace_dir: impl AsRef<Path>,
    account_id: AccountId,
) -> Result<DeviceLinkRequestOutput> {
    let workspace_dir = super::resolve_new_workspace(workspace_dir.as_ref())?;
    let parent = workspace_dir
        .parent()
        .context("device-link workspace has no parent")?;
    let staging = tempfile::Builder::new()
        .prefix(".kilogram-device-link-")
        .tempdir_in(parent)
        .context("create device-link staging directory")?;
    let state_dir = staging.path().join(DEVICE_STATE_DIRECTORY);
    let public_dir = staging.path().join(PUBLIC_DIRECTORY);
    let link_dir = staging.path().join(DEVICE_LINK_DIRECTORY);
    fs::create_dir(&public_dir).context("create device-link public directory")?;
    fs::create_dir(&link_dir).context("create device-link metadata directory")?;

    let device = DeviceState::load_or_create(&state_dir).context("create joining device")?;
    let mut ratchet =
        RatchetState::load_or_create(&state_dir).context("create joining device ratchet state")?;
    let now = unix_time_now()?;
    let pool = ratchet
        .prekey_pool(
            device.identity(),
            DEFAULT_PREKEY_POOL_SIZE,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )
        .context("create joining device prekey pool")?;
    let pool_bytes = pool.encode().context("encode joining device prekey pool")?;
    let request = SignedDeviceLinkRequest::issue(
        &device,
        account_id,
        pool_bytes.clone(),
        now,
        DEFAULT_REQUEST_VALIDITY_SECONDS,
    )?;
    let request_bytes = request.encode()?;
    super::write_new_file(&public_dir.join(PREKEY_POOL_FILE), &pool_bytes)?;
    super::write_new_file(&link_dir.join(REQUEST_FILE), &request_bytes)?;

    let receipt = RequestReceipt {
        version: RECEIPT_VERSION,
        account_id: account_id.to_string(),
        device_id: device.identity().device_id().to_string(),
        request_id: request.request_id_hex()?,
        expires_at_unix_seconds: request.content.expires_at_unix_seconds,
        recovery_scope: "authority-first-then-recipient-bound-history-plans",
    };
    write_json_new(&link_dir.join(REQUEST_RECEIPT_FILE), &receipt)?;
    drop(ratchet);
    let vault = EncryptedStateVault::open_or_create(&state_dir)
        .context("create joining device encrypted vault")?;
    let vault_key_protection = vault.key_protection().as_str().to_owned();
    vault
        .migrate_legacy_snapshot()
        .context("commit provisional joining device state")?;
    vault
        .verify()
        .context("verify provisional joining device vault")?;
    drop(vault);

    let final_state = workspace_dir.join(DEVICE_STATE_DIRECTORY);
    let final_request = workspace_dir.join(DEVICE_LINK_DIRECTORY).join(REQUEST_FILE);
    let final_pool = workspace_dir.join(PUBLIC_DIRECTORY).join(PREKEY_POOL_FILE);
    let output = DeviceLinkRequestOutput {
        status: "device-link-request-created",
        account_id: account_id.to_string(),
        device_id: device.identity().device_id().to_string(),
        request_id: request.request_id_hex()?,
        sas: request.sas()?,
        expires_at_unix_seconds: request.content.expires_at_unix_seconds,
        workspace_dir: workspace_dir.clone(),
        state_dir: final_state,
        request_file: final_request,
        prekey_pool_file: final_pool,
        vault_key_protection,
    };
    fs::rename(staging.path(), &workspace_dir).with_context(|| {
        format!(
            "publish provisional device-link workspace at {}",
            workspace_dir.display()
        )
    })?;
    Ok(output)
}

pub fn inspect_request(request_file: impl AsRef<Path>) -> Result<DeviceLinkInspectOutput> {
    let request = load_request(request_file.as_ref())?;
    let now = unix_time_now()?;
    let request_fresh = request.verify(Some(now)).is_ok();
    Ok(DeviceLinkInspectOutput {
        status: "device-link-request-verified",
        account_id: request.content.account_id.to_string(),
        device_id: request.content.device_id.to_string(),
        request_id: request.request_id_hex()?,
        sas: request.sas()?,
        issued_at_unix_seconds: request.content.issued_at_unix_seconds,
        expires_at_unix_seconds: request.content.expires_at_unix_seconds,
        request_fresh,
    })
}

pub fn authorize_request(
    account_root_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    confirm_sas: &str,
    response_file: impl AsRef<Path>,
    device_list_file: impl AsRef<Path>,
) -> Result<DeviceLinkAuthorizeOutput> {
    let now = unix_time_now()?;
    let request = load_request(request_file.as_ref())?;
    request.verify(Some(now))?;
    ensure!(
        request.sas()? == confirm_sas.trim(),
        "device-link SAS confirmation does not match"
    );
    let root = AccountRootState::load(account_root_dir.as_ref()).context("load Account Root")?;
    ensure!(
        root.account_id() == request.content.account_id,
        "device-link request targets another account"
    );
    let (certificate, device_list) = root
        .enroll_device(
            request.content.device_id,
            request.content.encryption_public_key,
            &DeviceCapability::MESSAGING,
        )
        .context("transactionally enroll requested device")?;
    let authorization =
        SignedDeviceLinkAuthorization::issue(&root, &request, certificate, device_list.clone())?;
    match fs::read(response_file.as_ref()) {
        Ok(existing) => {
            let existing = EncryptedDeviceLinkResponse::decode(&existing)
                .context("verify existing encrypted device-link response envelope")?;
            ensure!(
                existing.account_id == request.content.account_id
                    && existing.request_id == request.request_id()?,
                "existing response file is bound to another device-link request"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let response = EncryptedDeviceLinkResponse::issue(&request, &authorization)?;
            write_idempotent(response_file.as_ref(), &response.encode()?)?;
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "read existing device-link response {}",
                    response_file.as_ref().display()
                )
            });
        }
    }
    publish_device_list_file(device_list_file.as_ref(), &device_list)?;
    Ok(DeviceLinkAuthorizeOutput {
        status: "device-link-authorized",
        account_id: root.account_id().to_string(),
        device_id: request.content.device_id.to_string(),
        request_id: request.request_id_hex()?,
        authority_revision: device_list.revision(),
        response_file: absolute_existing_path(response_file.as_ref())?,
        device_list_file: absolute_existing_path(device_list_file.as_ref())?,
        response_encrypted_for_device: true,
    })
}

pub fn accept_response(
    workspace_dir: impl AsRef<Path>,
    response_file: impl AsRef<Path>,
) -> Result<DeviceLinkAcceptOutput> {
    let workspace_dir = fs::canonicalize(workspace_dir.as_ref()).with_context(|| {
        format!(
            "resolve joining workspace {}",
            workspace_dir.as_ref().display()
        )
    })?;
    let state_dir = workspace_dir.join(DEVICE_STATE_DIRECTORY);
    let _lock = StateDirectoryLock::acquire(&state_dir).context("lock joining device state")?;
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open joining device encrypted vault")?;
    vault
        .recover_primary_shadow()
        .context("recover interrupted retained trust shadow")?;
    vault
        .recover_pending_dual_write()
        .context("recover interrupted joining-device vault write")?;
    let identity = vault
        .read_primary_device_identity()
        .context("read authenticated joining-device identity")?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let request_file = workspace_dir.join(DEVICE_LINK_DIRECTORY).join(REQUEST_FILE);
    let request = load_request(&request_file)?;
    ensure!(
        request.content.device_id == device.identity().device_id(),
        "local joining device does not match stored request"
    );
    ensure!(
        request.content.encryption_public_key == device.encryption().public_key(),
        "local encryption key does not match stored request"
    );
    let response_bytes = fs::read(response_file.as_ref()).with_context(|| {
        format!(
            "read device-link response {}",
            response_file.as_ref().display()
        )
    })?;
    let response = EncryptedDeviceLinkResponse::decode(&response_bytes)?;
    let authorization = response.open(&request, &device)?;
    let certificate = &authorization.content.device_certificate;
    let device_list = &authorization.content.device_list;
    vault
        .begin_dual_write()
        .context("prepare joining-device vault transaction")?;
    let enrollment =
        install_enrollment_primary(&state_dir, &vault, &device, certificate, device_list);
    let mirror = vault
        .finish_dual_write()
        .context("complete joining-device vault transaction");
    match (enrollment, mirror) {
        (Ok(()), Ok(_)) => {}
        (Err(error), Ok(_)) => return Err(error),
        (Ok(()), Err(error)) => return Err(error),
        (Err(error), Err(mirror)) => {
            return Err(error.context(format!(
                "enrollment failed and vault cleanup also failed: {mirror:#}"
            )));
        }
    }

    let public_dir = workspace_dir.join(PUBLIC_DIRECTORY);
    let certificate_file = public_dir.join(DEVICE_CERTIFICATE_FILE);
    let device_list_file = public_dir.join(ACCOUNT_DEVICE_LIST_FILE);
    write_idempotent(&certificate_file, &certificate.encode()?)?;
    publish_device_list_file(&device_list_file, device_list)?;
    let receipt = AcceptReceipt {
        version: RECEIPT_VERSION,
        account_id: request.content.account_id.to_string(),
        device_id: request.content.device_id.to_string(),
        request_id: request.request_id_hex()?,
        authority_revision: device_list.revision(),
        history_recovery: "required-use-one-or-more-recipient-bound-recovery-plans",
    };
    write_json_idempotent(
        &workspace_dir
            .join(DEVICE_LINK_DIRECTORY)
            .join(ACCEPT_RECEIPT_FILE),
        &receipt,
    )?;
    let vault_key_protection = vault.key_protection().as_str().to_owned();
    vault
        .verify()
        .context("verify accepted joining device vault")?;

    Ok(DeviceLinkAcceptOutput {
        status: "device-link-accepted",
        account_id: request.content.account_id.to_string(),
        device_id: request.content.device_id.to_string(),
        request_id: request.request_id_hex()?,
        authority_revision: device_list.revision(),
        workspace_dir: workspace_dir.clone(),
        state_dir,
        certificate_file,
        device_list_file,
        prekey_pool_file: public_dir.join(PREKEY_POOL_FILE),
        vault_key_protection,
        history_recovery: "ready-for-recipient-bound-multi-source-plans",
    })
}

fn install_enrollment_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    certificate: &DeviceCertificate,
    device_list: &AccountDeviceListSnapshot,
) -> Result<()> {
    let trust = vault
        .read_primary_trust()
        .context("read current authenticated trust repository")?;
    let mut transaction = StateTransaction::begin(state_dir)
        .context("prepare crash-consistent enrollment transaction")?;
    if let Err(error) = transaction.prepare_trust_workspace(&trust) {
        let _ = transaction.rollback();
        return Err(error).context("prepare DB-primary trust workspace");
    }
    let operation = device
        .install_certificate(certificate)
        .context("install enrolled device certificate")
        .and_then(|()| {
            device
                .install_own_authority_snapshot(device_list.authority_snapshot())
                .context("install current account authority")
        });
    if let Err(error) = operation {
        return match transaction.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!(
                "enrollment failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    let _primary = match vault.commit_primary_transaction(&transaction) {
        Ok(primary) => primary,
        Err(error) => {
            return match transaction.rollback() {
                Ok(()) => Err(error).context("commit enrollment to encrypted vault"),
                Err(rollback) => Err(anyhow::Error::new(error).context(format!(
                    "vault enrollment failed and trust rollback also failed: {rollback}"
                ))),
            };
        }
    };
    transaction
        .commit()
        .context("commit retained enrollment trust shadow")?;
    vault
        .confirm_primary_shadow()
        .context("confirm retained enrollment trust shadow")?;
    Ok(())
}

fn load_request(path: &Path) -> Result<SignedDeviceLinkRequest> {
    let bytes =
        fs::read(path).with_context(|| format!("read device-link request {}", path.display()))?;
    SignedDeviceLinkRequest::decode(&bytes)
}

fn request_signing_bytes(content: &DeviceLinkRequestContent) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode device-link request content")?;
    let mut bytes = Vec::with_capacity(REQUEST_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(REQUEST_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn authorization_signing_bytes(content: &DeviceLinkAuthorizationContent) -> Result<Vec<u8>> {
    postcard::to_allocvec(content).context("encode device-link authorization content")
}

fn response_aad(account_id: AccountId, request_id: [u8; 32]) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(&(account_id, request_id))
        .context("encode device-link response associated data")?;
    let mut aad = Vec::with_capacity(RESPONSE_AAD_DOMAIN.len() + encoded.len());
    aad.extend_from_slice(RESPONSE_AAD_DOMAIN);
    aad.extend_from_slice(&encoded);
    Ok(aad)
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).context("encode device-link receipt")?;
    bytes.push(b'\n');
    super::write_new_file(path, &bytes)
}

fn write_json_idempotent(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).context("encode device-link receipt")?;
    bytes.push(b'\n');
    write_idempotent(path, &bytes)
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::read(path) {
        Ok(existing) => {
            ensure!(
                existing == bytes,
                "refusing to replace a different device-link artifact at {}",
                path.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    }
    let parent = path.parent().context("device-link output has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary file in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("write {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync {}", path.display()))?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing =
                fs::read(path).with_context(|| format!("read raced output {}", path.display()))?;
            ensure!(
                existing == bytes,
                "another process published a different artifact at {}",
                path.display()
            );
            Ok(())
        }
        Err(error) => Err(error.error).with_context(|| format!("publish {}", path.display())),
    }
}

fn publish_device_list_file(path: &Path, list: &AccountDeviceListSnapshot) -> Result<()> {
    let bytes = list.encode().context("encode account device list")?;
    match fs::read(path) {
        Ok(existing) => {
            let existing = AccountDeviceListSnapshot::decode_and_verify(&existing)
                .context("verify existing public account device list")?;
            ensure!(
                existing.account_id() == list.account_id(),
                "existing public device list belongs to another account"
            );
            ensure!(
                existing.revision() <= list.revision(),
                "refusing account device-list rollback"
            );
            if existing.revision() == list.revision() {
                ensure!(
                    existing == *list,
                    "different device list already exists at the same revision"
                );
                return Ok(());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    }
    let parent = path.parent().context("device-list output has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary device-list file in {}", parent.display()))?;
    temporary
        .write_all(&bytes)
        .with_context(|| format!("write {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish {}", path.display()))?;
    Ok(())
}

fn absolute_existing_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("resolve {}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn full_device_link_is_recipient_bound_encrypted_and_idempotent() -> Result<(), Box<dyn Error>>
    {
        let parent = tempfile::tempdir()?;
        let owner = parent.path().join("owner");
        let first = crate::create_account(&owner)?;
        let joining = parent.path().join("joining");
        let request_output = create_request(&joining, first.account_id())?;
        let inspect = inspect_request(&request_output.request_file)?;
        assert!(inspect.request_fresh);
        assert_eq!(inspect.sas, request_output.sas);
        let response = parent.path().join("authorization.kdl");
        let updated_list = owner.join("public").join(ACCOUNT_DEVICE_LIST_FILE);
        let authorized = authorize_request(
            first.account_root_dir(),
            &request_output.request_file,
            &request_output.sas,
            &response,
            &updated_list,
        )?;
        assert!(authorized.response_encrypted_for_device);
        assert_eq!(authorized.authority_revision, 2);
        let response_bytes = fs::read(&response)?;
        let signed_request = load_request(&request_output.request_file)?;
        assert!(
            !response_bytes
                .windows(32)
                .any(|window| window == signed_request.content.device_id.as_bytes())
        );
        let accepted = accept_response(&joining, &response)?;
        assert_eq!(accepted.authority_revision, 2);
        assert_eq!(accepted.device_id, request_output.device_id);
        assert_eq!(
            AccountDeviceListSnapshot::decode_and_verify(&fs::read(&accepted.device_list_file)?)?
                .devices()
                .len(),
            2
        );
        let repeated = authorize_request(
            first.account_root_dir(),
            &request_output.request_file,
            &request_output.sas,
            &response,
            &updated_list,
        )?;
        assert_eq!(repeated.authority_revision, 2);
        assert_eq!(accept_response(&joining, &response)?.authority_revision, 2);
        Ok(())
    }

    #[test]
    fn rejects_wrong_sas_tampering_and_wrong_recipient() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let owner = parent.path().join("owner");
        let first = crate::create_account(&owner)?;
        let joining = parent.path().join("joining");
        let request = create_request(&joining, first.account_id())?;
        let response = parent.path().join("response.kdl");
        let updated_list = owner.join("public").join(ACCOUNT_DEVICE_LIST_FILE);
        assert!(
            authorize_request(
                first.account_root_dir(),
                &request.request_file,
                "0000-0000-0000",
                &response,
                &updated_list,
            )
            .is_err()
        );
        authorize_request(
            first.account_root_dir(),
            &request.request_file,
            &request.sas,
            &response,
            &updated_list,
        )?;
        let other = parent.path().join("other");
        create_request(&other, first.account_id())?;
        assert!(accept_response(&other, &response).is_err());
        let mut tampered = fs::read(&response)?;
        let last = tampered.len().saturating_sub(1);
        tampered[last] ^= 1;
        let tampered_path = parent.path().join("tampered.kdl");
        fs::write(&tampered_path, tampered)?;
        assert!(accept_response(&joining, &tampered_path).is_err());
        Ok(())
    }
}
