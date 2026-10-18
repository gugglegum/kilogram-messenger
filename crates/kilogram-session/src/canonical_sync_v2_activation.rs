//! Device-bound durable authority for canonical sync v2 activation.
//!
//! The public runtime default is represented by the absence of records:
//! `legacy-only`, epoch zero, normal disposition. Every explicit change is a
//! signed, append-only successor. A separate signed high-water marker lets a
//! retained state directory detect a truncated tail after it has observed a
//! newer head. As with every local-only witness, a coordinated rollback of the
//! entire directory and its high-water marker still requires an external or
//! hardware witness to detect.

use kilogram_identity::{DeviceId, DeviceIdentity, IdentityError};
use thiserror::Error;

use crate::canonical_sync_v2_adapter::{
    CanonicalSyncV2ActivationDisposition, CanonicalSyncV2ActivationPermissions,
    CanonicalSyncV2MigrationState, evaluate_canonical_sync_v2_activation_permissions,
};

const ACTIVATION_RECORD_MAGIC: &[u8; 16] = b"KILOGRAM-CSAP-V1";
const ACTIVATION_HIGH_WATER_MAGIC: &[u8; 16] = b"KILOGRAM-CSHW-V1";
const ACTIVATION_RECORD_VERSION: u8 = 1;
const ACTIVATION_HIGH_WATER_VERSION: u8 = 1;
const ACTIVATION_RECORD_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:canonical-sync-v2:activation-record:v1\0";
const ACTIVATION_HIGH_WATER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:canonical-sync-v2:activation-high-water:v1\0";
const ACTIVATION_RECORD_ID_CONTEXT: &str = "kilogram canonical sync v2 activation record id v1";
const ACTIVATION_RECORD_FIXED_BYTES: usize = 192;
const ACTIVATION_HIGH_WATER_BYTES: usize = 153;

pub const MAX_CANONICAL_SYNC_V2_ACTIVATION_REASON_BYTES: usize = 256;
pub const MAX_CANONICAL_SYNC_V2_ACTIVATION_RECORD_BYTES: usize =
    ACTIVATION_RECORD_FIXED_BYTES + MAX_CANONICAL_SYNC_V2_ACTIVATION_REASON_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSyncV2ActivationActor {
    ExplicitLocal,
    ReleasePolicy,
}

impl CanonicalSyncV2ActivationActor {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitLocal => "explicit-local",
            Self::ReleasePolicy => "release-policy",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedCanonicalSyncV2ActivationRecord {
    local_device_id: DeviceId,
    migration_epoch: u64,
    previous_record_id: Option<[u8; 32]>,
    migration_state: CanonicalSyncV2MigrationState,
    disposition: CanonicalSyncV2ActivationDisposition,
    actor: CanonicalSyncV2ActivationActor,
    canonical_evidence_id: Option<[u8; 32]>,
    reason: String,
    signature: [u8; 64],
}

impl SignedCanonicalSyncV2ActivationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        identity: &DeviceIdentity,
        previous: Option<&Self>,
        migration_state: CanonicalSyncV2MigrationState,
        disposition: CanonicalSyncV2ActivationDisposition,
        actor: CanonicalSyncV2ActivationActor,
        canonical_evidence_id: Option<[u8; 32]>,
        reason: impl Into<String>,
    ) -> Result<Self, CanonicalSyncV2ActivationError> {
        let reason = reason.into();
        validate_reason(&reason)?;
        let migration_epoch = match previous {
            Some(previous) => previous
                .migration_epoch
                .checked_add(1)
                .ok_or(CanonicalSyncV2ActivationError::EpochOverflow)?,
            None => 1,
        };
        let previous_record_id = previous.map(Self::record_id).transpose()?;
        let mut record = Self {
            local_device_id: identity.device_id(),
            migration_epoch,
            previous_record_id,
            migration_state,
            disposition,
            actor,
            canonical_evidence_id,
            reason,
            signature: [0_u8; 64],
        };
        record.verify_transition(previous)?;
        record.signature = identity.sign(&record.signing_bytes()?);
        record.verify_signature()?;
        Ok(record)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        expected_device_id: DeviceId,
    ) -> Result<Self, CanonicalSyncV2ActivationError> {
        if encoded.len() < ACTIVATION_RECORD_FIXED_BYTES
            || encoded.len() > MAX_CANONICAL_SYNC_V2_ACTIVATION_RECORD_BYTES
        {
            return Err(CanonicalSyncV2ActivationError::RecordLength(encoded.len()));
        }
        if &encoded[..16] != ACTIVATION_RECORD_MAGIC {
            return Err(CanonicalSyncV2ActivationError::RecordMagic);
        }
        if encoded[16] != ACTIVATION_RECORD_VERSION {
            return Err(CanonicalSyncV2ActivationError::RecordVersion(encoded[16]));
        }
        let local_device_id = DeviceId::from_bytes(copy_array(&encoded[17..49])?);
        if local_device_id != expected_device_id {
            return Err(CanonicalSyncV2ActivationError::WrongDevice {
                expected: expected_device_id,
                found: local_device_id,
            });
        }
        let migration_epoch = u64::from_be_bytes(copy_array(&encoded[49..57])?);
        let previous_record_id = decode_optional_id(encoded[57], &encoded[58..90])?;
        let migration_state = migration_state_from_code(encoded[90])?;
        let disposition = disposition_from_code(encoded[91])?;
        let actor = actor_from_code(encoded[92])?;
        let canonical_evidence_id = decode_optional_id(encoded[93], &encoded[94..126])?;
        let reason_len = u16::from_be_bytes(copy_array(&encoded[126..128])?) as usize;
        let reason_end = 128_usize
            .checked_add(reason_len)
            .ok_or(CanonicalSyncV2ActivationError::RecordLength(encoded.len()))?;
        if reason_end + 64 != encoded.len() {
            return Err(CanonicalSyncV2ActivationError::RecordLength(encoded.len()));
        }
        let reason = std::str::from_utf8(&encoded[128..reason_end])
            .map_err(|_| CanonicalSyncV2ActivationError::ReasonUtf8)?
            .to_owned();
        validate_reason(&reason)?;
        let signature = copy_array(&encoded[reason_end..])?;
        let record = Self {
            local_device_id,
            migration_epoch,
            previous_record_id,
            migration_state,
            disposition,
            actor,
            canonical_evidence_id,
            reason,
            signature,
        };
        record.verify_signature()?;
        if record.encode()? != encoded {
            return Err(CanonicalSyncV2ActivationError::NonCanonicalRecord);
        }
        Ok(record)
    }

    pub fn encode(&self) -> Result<Vec<u8>, CanonicalSyncV2ActivationError> {
        self.verify_signature()?;
        let mut encoded = self.content_bytes()?;
        encoded.extend_from_slice(&self.signature);
        Ok(encoded)
    }

    pub fn verify_signature(&self) -> Result<(), CanonicalSyncV2ActivationError> {
        self.local_device_id
            .verify(&self.signing_bytes()?, &self.signature)
            .map_err(CanonicalSyncV2ActivationError::Signature)
    }

    pub fn verify_transition(
        &self,
        previous: Option<&Self>,
    ) -> Result<(), CanonicalSyncV2ActivationError> {
        validate_reason(&self.reason)?;
        evaluate_canonical_sync_v2_activation_permissions(
            self.migration_state,
            self.disposition,
            self.canonical_evidence_id.is_some(),
        )
        .map_err(|_| CanonicalSyncV2ActivationError::PermissionCombination)?;

        let (
            expected_epoch,
            expected_previous_id,
            previous_state,
            previous_disposition,
            old_evidence,
        ) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                if previous.local_device_id != self.local_device_id {
                    return Err(CanonicalSyncV2ActivationError::WrongDevice {
                        expected: previous.local_device_id,
                        found: self.local_device_id,
                    });
                }
                (
                    previous
                        .migration_epoch
                        .checked_add(1)
                        .ok_or(CanonicalSyncV2ActivationError::EpochOverflow)?,
                    Some(previous.record_id()?),
                    previous.migration_state,
                    previous.disposition,
                    previous.canonical_evidence_id,
                )
            }
            None => (
                1,
                None,
                CanonicalSyncV2MigrationState::LegacyOnly,
                CanonicalSyncV2ActivationDisposition::Normal,
                None,
            ),
        };

        if self.migration_epoch != expected_epoch {
            return Err(CanonicalSyncV2ActivationError::EpochGap {
                expected: expected_epoch,
                found: self.migration_epoch,
            });
        }
        if self.previous_record_id != expected_previous_id {
            return Err(CanonicalSyncV2ActivationError::PreviousRecordMismatch);
        }
        if let Some(old_evidence) = old_evidence
            && self.canonical_evidence_id != Some(old_evidence)
        {
            return Err(CanonicalSyncV2ActivationError::CanonicalEvidenceChanged);
        }

        let old_rank = migration_state_rank(previous_state);
        let new_rank = migration_state_rank(self.migration_state);
        let state_delta = i16::from(new_rank) - i16::from(old_rank);
        let disposition_changed = self.disposition != previous_disposition;
        let evidence_added = old_evidence.is_none() && self.canonical_evidence_id.is_some();

        match state_delta {
            1 => {
                if self.disposition != CanonicalSyncV2ActivationDisposition::Normal
                    || disposition_changed
                    || evidence_added
                {
                    return Err(CanonicalSyncV2ActivationError::InvalidTransition);
                }
            }
            -1 => {
                if old_evidence.is_some()
                    || self.canonical_evidence_id.is_some()
                    || self.disposition != CanonicalSyncV2ActivationDisposition::Normal
                {
                    return Err(CanonicalSyncV2ActivationError::RollbackAfterCanonicalEvidence);
                }
            }
            0 => {
                let containment_change = disposition_changed
                    && self.canonical_evidence_id.is_some()
                    && self.migration_state != CanonicalSyncV2MigrationState::LegacyOnly
                    && self.migration_state != CanonicalSyncV2MigrationState::AdvertiseCanonicalV2;
                let evidence_observation = !disposition_changed
                    && previous_disposition == CanonicalSyncV2ActivationDisposition::Normal
                    && evidence_added
                    && self.migration_state != CanonicalSyncV2MigrationState::LegacyOnly
                    && self.migration_state != CanonicalSyncV2MigrationState::AdvertiseCanonicalV2;
                if !containment_change && !evidence_observation {
                    return Err(CanonicalSyncV2ActivationError::NoEffectiveChange);
                }
            }
            _ => return Err(CanonicalSyncV2ActivationError::InvalidTransition),
        }
        Ok(())
    }

    pub fn record_id(&self) -> Result<[u8; 32], CanonicalSyncV2ActivationError> {
        Ok(blake3::derive_key(
            ACTIVATION_RECORD_ID_CONTEXT,
            &self.encode()?,
        ))
    }

    pub fn permissions(
        &self,
    ) -> Result<CanonicalSyncV2ActivationPermissions, CanonicalSyncV2ActivationError> {
        evaluate_canonical_sync_v2_activation_permissions(
            self.migration_state,
            self.disposition,
            self.canonical_evidence_id.is_some(),
        )
        .map_err(|_| CanonicalSyncV2ActivationError::PermissionCombination)
    }

    pub const fn local_device_id(&self) -> DeviceId {
        self.local_device_id
    }

    pub const fn migration_epoch(&self) -> u64 {
        self.migration_epoch
    }

    pub const fn previous_record_id(&self) -> Option<[u8; 32]> {
        self.previous_record_id
    }

    pub const fn migration_state(&self) -> CanonicalSyncV2MigrationState {
        self.migration_state
    }

    pub const fn disposition(&self) -> CanonicalSyncV2ActivationDisposition {
        self.disposition
    }

    pub const fn actor(&self) -> CanonicalSyncV2ActivationActor {
        self.actor
    }

    pub const fn canonical_evidence_id(&self) -> Option<[u8; 32]> {
        self.canonical_evidence_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, CanonicalSyncV2ActivationError> {
        let content = self.content_bytes()?;
        let mut signing =
            Vec::with_capacity(ACTIVATION_RECORD_SIGNATURE_DOMAIN.len() + content.len());
        signing.extend_from_slice(ACTIVATION_RECORD_SIGNATURE_DOMAIN);
        signing.extend_from_slice(&content);
        Ok(signing)
    }

    fn content_bytes(&self) -> Result<Vec<u8>, CanonicalSyncV2ActivationError> {
        validate_reason(&self.reason)?;
        let reason = self.reason.as_bytes();
        let reason_len = u16::try_from(reason.len())
            .map_err(|_| CanonicalSyncV2ActivationError::ReasonLength(reason.len()))?;
        let mut encoded = Vec::with_capacity(128 + reason.len());
        encoded.extend_from_slice(ACTIVATION_RECORD_MAGIC);
        encoded.push(ACTIVATION_RECORD_VERSION);
        encoded.extend_from_slice(self.local_device_id.as_bytes());
        encoded.extend_from_slice(&self.migration_epoch.to_be_bytes());
        encode_optional_id(self.previous_record_id, &mut encoded);
        encoded.push(migration_state_code(self.migration_state));
        encoded.push(disposition_code(self.disposition));
        encoded.push(actor_code(self.actor));
        encode_optional_id(self.canonical_evidence_id, &mut encoded);
        encoded.extend_from_slice(&reason_len.to_be_bytes());
        encoded.extend_from_slice(reason);
        Ok(encoded)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedCanonicalSyncV2ActivationHighWater {
    local_device_id: DeviceId,
    migration_epoch: u64,
    head_record_id: [u8; 32],
    signature: [u8; 64],
}

impl SignedCanonicalSyncV2ActivationHighWater {
    pub fn issue(
        identity: &DeviceIdentity,
        head: &SignedCanonicalSyncV2ActivationRecord,
    ) -> Result<Self, CanonicalSyncV2ActivationError> {
        if identity.device_id() != head.local_device_id {
            return Err(CanonicalSyncV2ActivationError::WrongDevice {
                expected: head.local_device_id,
                found: identity.device_id(),
            });
        }
        let mut value = Self {
            local_device_id: identity.device_id(),
            migration_epoch: head.migration_epoch,
            head_record_id: head.record_id()?,
            signature: [0_u8; 64],
        };
        value.signature = identity.sign(&value.signing_bytes());
        value.verify_signature()?;
        Ok(value)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        expected_device_id: DeviceId,
    ) -> Result<Self, CanonicalSyncV2ActivationError> {
        if encoded.len() != ACTIVATION_HIGH_WATER_BYTES {
            return Err(CanonicalSyncV2ActivationError::HighWaterLength(
                encoded.len(),
            ));
        }
        if &encoded[..16] != ACTIVATION_HIGH_WATER_MAGIC {
            return Err(CanonicalSyncV2ActivationError::HighWaterMagic);
        }
        if encoded[16] != ACTIVATION_HIGH_WATER_VERSION {
            return Err(CanonicalSyncV2ActivationError::HighWaterVersion(
                encoded[16],
            ));
        }
        let local_device_id = DeviceId::from_bytes(copy_array(&encoded[17..49])?);
        if local_device_id != expected_device_id {
            return Err(CanonicalSyncV2ActivationError::WrongDevice {
                expected: expected_device_id,
                found: local_device_id,
            });
        }
        let value = Self {
            local_device_id,
            migration_epoch: u64::from_be_bytes(copy_array(&encoded[49..57])?),
            head_record_id: copy_array(&encoded[57..89])?,
            signature: copy_array(&encoded[89..153])?,
        };
        value.verify_signature()?;
        if value.encode() != encoded {
            return Err(CanonicalSyncV2ActivationError::NonCanonicalHighWater);
        }
        Ok(value)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = self.content_bytes();
        encoded.extend_from_slice(&self.signature);
        encoded
    }

    pub fn verify_signature(&self) -> Result<(), CanonicalSyncV2ActivationError> {
        self.local_device_id
            .verify(&self.signing_bytes(), &self.signature)
            .map_err(CanonicalSyncV2ActivationError::Signature)
    }

    pub const fn local_device_id(&self) -> DeviceId {
        self.local_device_id
    }

    pub const fn migration_epoch(&self) -> u64 {
        self.migration_epoch
    }

    pub const fn head_record_id(&self) -> [u8; 32] {
        self.head_record_id
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let content = self.content_bytes();
        let mut signing =
            Vec::with_capacity(ACTIVATION_HIGH_WATER_SIGNATURE_DOMAIN.len() + content.len());
        signing.extend_from_slice(ACTIVATION_HIGH_WATER_SIGNATURE_DOMAIN);
        signing.extend_from_slice(&content);
        signing
    }

    fn content_bytes(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(89);
        encoded.extend_from_slice(ACTIVATION_HIGH_WATER_MAGIC);
        encoded.push(ACTIVATION_HIGH_WATER_VERSION);
        encoded.extend_from_slice(self.local_device_id.as_bytes());
        encoded.extend_from_slice(&self.migration_epoch.to_be_bytes());
        encoded.extend_from_slice(&self.head_record_id);
        encoded
    }
}

pub fn verify_canonical_sync_v2_activation_chain(
    records: &[SignedCanonicalSyncV2ActivationRecord],
    expected_device_id: DeviceId,
) -> Result<Option<SignedCanonicalSyncV2ActivationRecord>, CanonicalSyncV2ActivationError> {
    if records.is_empty() {
        return Ok(None);
    }
    let mut ordered = records.to_vec();
    ordered.sort_by_key(SignedCanonicalSyncV2ActivationRecord::migration_epoch);
    let mut previous: Option<&SignedCanonicalSyncV2ActivationRecord> = None;
    for record in &ordered {
        if record.local_device_id != expected_device_id {
            return Err(CanonicalSyncV2ActivationError::WrongDevice {
                expected: expected_device_id,
                found: record.local_device_id,
            });
        }
        record.verify_signature()?;
        record.verify_transition(previous)?;
        previous = Some(record);
    }
    Ok(ordered.last().cloned())
}

pub fn verify_canonical_sync_v2_activation_authority(
    records: &[SignedCanonicalSyncV2ActivationRecord],
    high_water: Option<&SignedCanonicalSyncV2ActivationHighWater>,
    expected_device_id: DeviceId,
) -> Result<Option<SignedCanonicalSyncV2ActivationRecord>, CanonicalSyncV2ActivationError> {
    let head = verify_canonical_sync_v2_activation_chain(records, expected_device_id)?;
    match (head.as_ref(), high_water) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(CanonicalSyncV2ActivationError::HighWaterWithoutChain),
        (Some(_), None) => Err(CanonicalSyncV2ActivationError::HighWaterMissing),
        (Some(head), Some(high_water)) => {
            high_water.verify_signature()?;
            if high_water.local_device_id != expected_device_id {
                return Err(CanonicalSyncV2ActivationError::WrongDevice {
                    expected: expected_device_id,
                    found: high_water.local_device_id,
                });
            }
            if high_water.migration_epoch != head.migration_epoch
                || high_water.head_record_id != head.record_id()?
            {
                return Err(CanonicalSyncV2ActivationError::HighWaterMismatch);
            }
            Ok(Some(head.clone()))
        }
    }
}

#[derive(Debug, Error)]
pub enum CanonicalSyncV2ActivationError {
    #[error("canonical sync v2 activation record has invalid length {0}")]
    RecordLength(usize),
    #[error("canonical sync v2 activation record has invalid magic")]
    RecordMagic,
    #[error("canonical sync v2 activation record version {0} is unsupported")]
    RecordVersion(u8),
    #[error("canonical sync v2 activation record is not canonically encoded")]
    NonCanonicalRecord,
    #[error("canonical sync v2 activation high-water has invalid length {0}")]
    HighWaterLength(usize),
    #[error("canonical sync v2 activation high-water has invalid magic")]
    HighWaterMagic,
    #[error("canonical sync v2 activation high-water version {0} is unsupported")]
    HighWaterVersion(u8),
    #[error("canonical sync v2 activation high-water is not canonically encoded")]
    NonCanonicalHighWater,
    #[error("canonical sync v2 activation metadata belongs to Device {found}, expected {expected}")]
    WrongDevice { expected: DeviceId, found: DeviceId },
    #[error("canonical sync v2 activation signature is invalid")]
    Signature(#[source] IdentityError),
    #[error("canonical sync v2 activation optional identifier flag {0} is invalid")]
    OptionalIdentifierFlag(u8),
    #[error("canonical sync v2 activation absent identifier contains non-zero bytes")]
    NonCanonicalAbsentIdentifier,
    #[error("canonical sync v2 activation migration state code {0} is invalid")]
    MigrationStateCode(u8),
    #[error("canonical sync v2 activation disposition code {0} is invalid")]
    DispositionCode(u8),
    #[error("canonical sync v2 activation actor code {0} is invalid")]
    ActorCode(u8),
    #[error("canonical sync v2 activation reason is not valid UTF-8")]
    ReasonUtf8,
    #[error("canonical sync v2 activation reason length {0} is outside 1..=256 bytes")]
    ReasonLength(usize),
    #[error(
        "canonical sync v2 activation reason must be trimmed and contain no control characters"
    )]
    ReasonInvalid,
    #[error("canonical sync v2 activation epoch overflowed")]
    EpochOverflow,
    #[error("canonical sync v2 activation epoch gap: expected {expected}, found {found}")]
    EpochGap { expected: u64, found: u64 },
    #[error("canonical sync v2 activation previous-record binding is not the exact prior head")]
    PreviousRecordMismatch,
    #[error("canonical sync v2 activation transition is not an exact one-step transition")]
    InvalidTransition,
    #[error("canonical sync v2 activation transition makes no effective policy change")]
    NoEffectiveChange,
    #[error("canonical sync v2 activation rollback is blocked after canonical evidence")]
    RollbackAfterCanonicalEvidence,
    #[error("canonical sync v2 activation canonical evidence binding changed or disappeared")]
    CanonicalEvidenceChanged,
    #[error("canonical sync v2 activation state/disposition/evidence combination is invalid")]
    PermissionCombination,
    #[error("canonical sync v2 activation chain exists but its signed high-water is missing")]
    HighWaterMissing,
    #[error("canonical sync v2 activation high-water exists without its append-only chain")]
    HighWaterWithoutChain,
    #[error("canonical sync v2 activation high-water does not match the exact chain head")]
    HighWaterMismatch,
    #[error("canonical sync v2 activation record contains a malformed fixed-width field")]
    FixedWidthField,
}

fn validate_reason(reason: &str) -> Result<(), CanonicalSyncV2ActivationError> {
    let len = reason.len();
    if !(1..=MAX_CANONICAL_SYNC_V2_ACTIVATION_REASON_BYTES).contains(&len) {
        return Err(CanonicalSyncV2ActivationError::ReasonLength(len));
    }
    if reason.trim() != reason || reason.chars().any(char::is_control) {
        return Err(CanonicalSyncV2ActivationError::ReasonInvalid);
    }
    Ok(())
}

fn copy_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], CanonicalSyncV2ActivationError> {
    bytes
        .try_into()
        .map_err(|_| CanonicalSyncV2ActivationError::FixedWidthField)
}

fn encode_optional_id(value: Option<[u8; 32]>, encoded: &mut Vec<u8>) {
    match value {
        Some(value) => {
            encoded.push(1);
            encoded.extend_from_slice(&value);
        }
        None => {
            encoded.push(0);
            encoded.extend_from_slice(&[0_u8; 32]);
        }
    }
}

fn decode_optional_id(
    flag: u8,
    bytes: &[u8],
) -> Result<Option<[u8; 32]>, CanonicalSyncV2ActivationError> {
    let value = copy_array(bytes)?;
    match flag {
        0 if value == [0_u8; 32] => Ok(None),
        0 => Err(CanonicalSyncV2ActivationError::NonCanonicalAbsentIdentifier),
        1 => Ok(Some(value)),
        _ => Err(CanonicalSyncV2ActivationError::OptionalIdentifierFlag(flag)),
    }
}

fn migration_state_rank(state: CanonicalSyncV2MigrationState) -> u8 {
    match state {
        CanonicalSyncV2MigrationState::LegacyOnly => 0,
        CanonicalSyncV2MigrationState::AdvertiseCanonicalV2 => 1,
        CanonicalSyncV2MigrationState::DualReaderPreferLegacy => 2,
        CanonicalSyncV2MigrationState::DualReaderPreferCanonical => 3,
        CanonicalSyncV2MigrationState::CanonicalRequired => 4,
    }
}

fn migration_state_code(state: CanonicalSyncV2MigrationState) -> u8 {
    migration_state_rank(state) + 1
}

fn migration_state_from_code(
    code: u8,
) -> Result<CanonicalSyncV2MigrationState, CanonicalSyncV2ActivationError> {
    match code {
        1 => Ok(CanonicalSyncV2MigrationState::LegacyOnly),
        2 => Ok(CanonicalSyncV2MigrationState::AdvertiseCanonicalV2),
        3 => Ok(CanonicalSyncV2MigrationState::DualReaderPreferLegacy),
        4 => Ok(CanonicalSyncV2MigrationState::DualReaderPreferCanonical),
        5 => Ok(CanonicalSyncV2MigrationState::CanonicalRequired),
        _ => Err(CanonicalSyncV2ActivationError::MigrationStateCode(code)),
    }
}

fn disposition_code(disposition: CanonicalSyncV2ActivationDisposition) -> u8 {
    match disposition {
        CanonicalSyncV2ActivationDisposition::Normal => 1,
        CanonicalSyncV2ActivationDisposition::EmergencyContainment => 2,
    }
}

fn disposition_from_code(
    code: u8,
) -> Result<CanonicalSyncV2ActivationDisposition, CanonicalSyncV2ActivationError> {
    match code {
        1 => Ok(CanonicalSyncV2ActivationDisposition::Normal),
        2 => Ok(CanonicalSyncV2ActivationDisposition::EmergencyContainment),
        _ => Err(CanonicalSyncV2ActivationError::DispositionCode(code)),
    }
}

fn actor_code(actor: CanonicalSyncV2ActivationActor) -> u8 {
    match actor {
        CanonicalSyncV2ActivationActor::ExplicitLocal => 1,
        CanonicalSyncV2ActivationActor::ReleasePolicy => 2,
    }
}

fn actor_from_code(
    code: u8,
) -> Result<CanonicalSyncV2ActivationActor, CanonicalSyncV2ActivationError> {
    match code {
        1 => Ok(CanonicalSyncV2ActivationActor::ExplicitLocal),
        2 => Ok(CanonicalSyncV2ActivationActor::ReleasePolicy),
        _ => Err(CanonicalSyncV2ActivationError::ActorCode(code)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity(seed: u8) -> DeviceIdentity {
        DeviceIdentity::from_secret_bytes([seed; 32])
    }

    fn issue_forward_chain(
        identity: &DeviceIdentity,
    ) -> Result<Vec<SignedCanonicalSyncV2ActivationRecord>, CanonicalSyncV2ActivationError> {
        let advertise = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            None,
            CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "publish only after listener registration",
        )?;
        let dual = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            Some(&advertise),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "accept selected canonical sessions",
        )?;
        Ok(vec![advertise, dual])
    }

    #[test]
    fn signed_chain_round_trips_and_survives_restart() -> Result<(), CanonicalSyncV2ActivationError>
    {
        let identity = test_identity(7);
        let records = issue_forward_chain(&identity)?;
        let encoded: Vec<Vec<u8>> = records
            .iter()
            .map(SignedCanonicalSyncV2ActivationRecord::encode)
            .collect::<Result<_, _>>()?;
        let decoded: Vec<_> = encoded
            .iter()
            .map(|bytes| {
                SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
                    bytes,
                    identity.device_id(),
                )
            })
            .collect::<Result<_, _>>()?;
        let head = verify_canonical_sync_v2_activation_chain(&decoded, identity.device_id())?
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        let expected = records
            .last()
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        assert_eq!(&head, expected);
        assert_eq!(head.migration_epoch(), 2);
        Ok(())
    }

    #[test]
    fn authority_rejects_interrupted_tail_until_exact_high_water_is_published()
    -> Result<(), CanonicalSyncV2ActivationError> {
        let identity = test_identity(8);
        let records = issue_forward_chain(&identity)?;
        assert!(matches!(
            verify_canonical_sync_v2_activation_authority(&records, None, identity.device_id()),
            Err(CanonicalSyncV2ActivationError::HighWaterMissing)
        ));
        let expected = records
            .last()
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        let high_water = SignedCanonicalSyncV2ActivationHighWater::issue(&identity, expected)?;
        let verified = verify_canonical_sync_v2_activation_authority(
            &records,
            Some(&high_water),
            identity.device_id(),
        )?
        .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        assert_eq!(&verified, expected);
        Ok(())
    }

    #[test]
    fn chain_rejects_replay_epoch_skip_fork_and_wrong_device()
    -> Result<(), CanonicalSyncV2ActivationError> {
        let identity = test_identity(9);
        let other = test_identity(10);
        let records = issue_forward_chain(&identity)?;
        let first = records
            .first()
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        let second = records
            .last()
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;

        let replay = vec![first.clone(), first.clone()];
        assert!(matches!(
            verify_canonical_sync_v2_activation_chain(&replay, identity.device_id()),
            Err(CanonicalSyncV2ActivationError::EpochGap { .. })
        ));

        let fork = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            Some(first),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ReleasePolicy,
            None,
            "second valid successor creates a fork",
        )?;
        assert!(matches!(
            verify_canonical_sync_v2_activation_chain(
                &[first.clone(), second.clone(), fork],
                identity.device_id()
            ),
            Err(CanonicalSyncV2ActivationError::EpochGap { .. })
        ));

        let wrong = SignedCanonicalSyncV2ActivationRecord::issue(
            &other,
            None,
            CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "wrong device record",
        )?;
        let wrong_encoded = wrong.encode()?;
        assert!(matches!(
            SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
                &wrong_encoded,
                identity.device_id()
            ),
            Err(CanonicalSyncV2ActivationError::WrongDevice { .. })
        ));

        let mut skipped = second.clone();
        skipped.migration_epoch = 4;
        skipped.signature = identity.sign(&skipped.signing_bytes()?);
        assert!(matches!(
            verify_canonical_sync_v2_activation_chain(
                &[first.clone(), skipped],
                identity.device_id()
            ),
            Err(CanonicalSyncV2ActivationError::EpochGap {
                expected: 2,
                found: 4
            })
        ));
        Ok(())
    }

    #[test]
    fn high_water_rejects_truncated_or_replaced_chain_head()
    -> Result<(), CanonicalSyncV2ActivationError> {
        let identity = test_identity(11);
        let records = issue_forward_chain(&identity)?;
        let head = records
            .last()
            .ok_or(CanonicalSyncV2ActivationError::HighWaterWithoutChain)?;
        let high_water = SignedCanonicalSyncV2ActivationHighWater::issue(&identity, head)?;
        let restarted = SignedCanonicalSyncV2ActivationHighWater::decode_and_verify(
            &high_water.encode(),
            identity.device_id(),
        )?;
        assert!(matches!(
            verify_canonical_sync_v2_activation_authority(
                &records[..1],
                Some(&restarted),
                identity.device_id()
            ),
            Err(CanonicalSyncV2ActivationError::HighWaterMismatch)
        ));
        Ok(())
    }

    #[test]
    fn containment_preserves_evidence_and_blocks_rollback()
    -> Result<(), CanonicalSyncV2ActivationError> {
        let identity = test_identity(12);
        let mut records = issue_forward_chain(&identity)?;
        let selected = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            records.last(),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ReleasePolicy,
            Some([4_u8; 32]),
            "bind authenticated canonical selection",
        )?;
        records.push(selected);
        let containment = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            records.last(),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::EmergencyContainment,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            Some([4_u8; 32]),
            "contain new sessions during incident response",
        )?;
        let permissions = containment.permissions()?;
        assert!(permissions.resume_selected_canonical_session());
        assert!(!permissions.initiate_new_canonical_session());
        assert!(matches!(
            SignedCanonicalSyncV2ActivationRecord::issue(
                &identity,
                Some(&containment),
                CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
                CanonicalSyncV2ActivationDisposition::Normal,
                CanonicalSyncV2ActivationActor::ExplicitLocal,
                None,
                "forbidden rollback",
            ),
            Err(CanonicalSyncV2ActivationError::CanonicalEvidenceChanged)
                | Err(CanonicalSyncV2ActivationError::RollbackAfterCanonicalEvidence)
        ));
        Ok(())
    }
}
