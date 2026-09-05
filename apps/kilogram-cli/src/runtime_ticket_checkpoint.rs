use std::{collections::BTreeSet, fmt};

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    runtime_endpoint_announcement::{
        AcceptedEndpointObservationId, SignedAcceptedEndpointObservation,
    },
    runtime_own_device_automation::{
        OwnDeviceAnnouncementAttemptId, OwnDeviceAnnouncementPolicyId,
        SignedOwnDeviceAnnouncementAttempt, SignedOwnDeviceAnnouncementPolicy,
    },
    runtime_own_device_discovery::{
        OwnDeviceTicketDiscoveryPolicyId, SignedOwnDeviceTicketDiscoveryPolicy,
    },
    runtime_own_device_roster::{OwnDeviceRosterPolicyId, SignedOwnDeviceRosterPolicy},
    runtime_publication::{
        SignedTicketPublication, SignedTicketPublicationObservation, TicketPublicationChannelId,
        TicketPublicationId, TicketPublicationObservationId,
    },
    runtime_queue::{MAX_RUNTIME_RECORD_BYTES, RuntimeContactId},
    runtime_ticket_automation::{
        SignedTicketAutomationAttempt, SignedTicketAutomationPolicy, TicketAutomationAction,
        TicketAutomationAttemptId, TicketAutomationPolicyId,
    },
};

const CHECKPOINT_VERSION: u8 = 1;
const CHECKPOINT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ticket-checkpoint:v1\0";
const CHECKPOINT_ID_DOMAIN: &[u8] = b"kilogram:runtime-ticket-checkpoint-id:v1\0";
const COMPACTED_DELTA_DIGEST_DOMAIN: &[u8] = b"kilogram:runtime-ticket-compacted-delta:v1\0";
const COMPACTED_HISTORY_DIGEST_DOMAIN: &[u8] = b"kilogram:runtime-ticket-compacted-history:v1\0";
pub const MAX_RUNTIME_TICKET_CHECKPOINT_ANCHORS: usize = 2_048;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeTicketCheckpointId([u8; 32]);

impl RuntimeTicketCheckpointId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for RuntimeTicketCheckpointId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeTicketChainAnchor {
    Publication {
        channel_id: TicketPublicationChannelId,
        generation: u64,
        record_id: TicketPublicationId,
    },
    Observation {
        channel_id: TicketPublicationChannelId,
        generation: u64,
        publication_generation: u64,
        record_id: TicketPublicationObservationId,
    },
    Policy {
        contact_id: RuntimeContactId,
        generation: u64,
        record_id: TicketAutomationPolicyId,
    },
    Attempt {
        contact_id: RuntimeContactId,
        action: TicketAutomationAction,
        generation: u64,
        policy_generation: u64,
        record_id: TicketAutomationAttemptId,
    },
    OwnDevicePolicy {
        recipient_device_id: DeviceId,
        generation: u64,
        record_id: OwnDeviceAnnouncementPolicyId,
    },
    OwnDeviceAttempt {
        recipient_device_id: DeviceId,
        generation: u64,
        policy_generation: u64,
        record_id: OwnDeviceAnnouncementAttemptId,
    },
    OwnDeviceTicketDiscoveryPolicy {
        recipient_device_id: DeviceId,
        generation: u64,
        record_id: OwnDeviceTicketDiscoveryPolicyId,
    },
    OwnDeviceRosterPolicy {
        generation: u64,
        record_id: OwnDeviceRosterPolicyId,
    },
    AcceptedEndpointObservation {
        channel_id: TicketPublicationChannelId,
        generation: u64,
        publication_id: TicketPublicationId,
        ticket_digest: [u8; 32],
        record_id: AcceptedEndpointObservationId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RuntimeTicketChainKey {
    Publication(TicketPublicationChannelId),
    Observation(TicketPublicationChannelId),
    Policy(RuntimeContactId),
    Attempt(RuntimeContactId, TicketAutomationAction),
    OwnDevicePolicy(DeviceId),
    OwnDeviceAttempt(DeviceId),
    OwnDeviceTicketDiscoveryPolicy(DeviceId),
    OwnDeviceRosterPolicy,
    AcceptedEndpointObservation(TicketPublicationChannelId),
}

impl RuntimeTicketChainAnchor {
    pub fn publication(value: &SignedTicketPublication) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::Publication {
            channel_id: value.channel_id(),
            generation: value.generation(),
            record_id: value.publication_id()?,
        })
    }

    pub fn observation(value: &SignedTicketPublicationObservation) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::Observation {
            channel_id: value.channel_id(),
            generation: value.observation_generation(),
            publication_generation: value.publication_generation(),
            record_id: value.observation_id()?,
        })
    }

    pub fn policy(value: &SignedTicketAutomationPolicy) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::Policy {
            contact_id: value.contact_id(),
            generation: value.generation(),
            record_id: value.policy_id()?,
        })
    }

    pub fn attempt(value: &SignedTicketAutomationAttempt) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::Attempt {
            contact_id: value.contact_id(),
            action: value.action(),
            generation: value.generation(),
            policy_generation: value.policy_generation(),
            record_id: value.attempt_id()?,
        })
    }

    pub fn own_device_policy(value: &SignedOwnDeviceAnnouncementPolicy) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::OwnDevicePolicy {
            recipient_device_id: value.recipient_device_id(),
            generation: value.generation(),
            record_id: value.policy_id()?,
        })
    }

    pub fn own_device_attempt(value: &SignedOwnDeviceAnnouncementAttempt) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::OwnDeviceAttempt {
            recipient_device_id: value.recipient_device_id(),
            generation: value.generation(),
            policy_generation: value.policy_generation(),
            record_id: value.attempt_id()?,
        })
    }

    pub fn own_device_ticket_discovery_policy(
        value: &SignedOwnDeviceTicketDiscoveryPolicy,
    ) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::OwnDeviceTicketDiscoveryPolicy {
            recipient_device_id: value.recipient_device_id(),
            generation: value.generation(),
            record_id: value.policy_id()?,
        })
    }

    pub fn own_device_roster_policy(value: &SignedOwnDeviceRosterPolicy) -> Result<Self> {
        value.verify_signature()?;
        Ok(Self::OwnDeviceRosterPolicy {
            generation: value.generation(),
            record_id: value.policy_id()?,
        })
    }

    pub fn accepted_endpoint_observation(
        value: &SignedAcceptedEndpointObservation,
    ) -> Result<Self> {
        value.verify()?;
        Ok(Self::AcceptedEndpointObservation {
            channel_id: value.channel_id(),
            generation: value.publication_generation(),
            publication_id: value.publication_id(),
            ticket_digest: value.ticket_digest(),
            record_id: value.evidence_id()?,
        })
    }

    fn key(&self) -> RuntimeTicketChainKey {
        match self {
            Self::Publication { channel_id, .. } => RuntimeTicketChainKey::Publication(*channel_id),
            Self::Observation { channel_id, .. } => RuntimeTicketChainKey::Observation(*channel_id),
            Self::Policy { contact_id, .. } => RuntimeTicketChainKey::Policy(*contact_id),
            Self::Attempt {
                contact_id, action, ..
            } => RuntimeTicketChainKey::Attempt(*contact_id, *action),
            Self::OwnDevicePolicy {
                recipient_device_id,
                ..
            } => RuntimeTicketChainKey::OwnDevicePolicy(*recipient_device_id),
            Self::OwnDeviceAttempt {
                recipient_device_id,
                ..
            } => RuntimeTicketChainKey::OwnDeviceAttempt(*recipient_device_id),
            Self::OwnDeviceTicketDiscoveryPolicy {
                recipient_device_id,
                ..
            } => RuntimeTicketChainKey::OwnDeviceTicketDiscoveryPolicy(*recipient_device_id),
            Self::OwnDeviceRosterPolicy { .. } => RuntimeTicketChainKey::OwnDeviceRosterPolicy,
            Self::AcceptedEndpointObservation { channel_id, .. } => {
                RuntimeTicketChainKey::AcceptedEndpointObservation(*channel_id)
            }
        }
    }

    pub fn generation(&self) -> u64 {
        match self {
            Self::Publication { generation, .. }
            | Self::Observation { generation, .. }
            | Self::Policy { generation, .. }
            | Self::Attempt { generation, .. }
            | Self::OwnDevicePolicy { generation, .. }
            | Self::OwnDeviceAttempt { generation, .. }
            | Self::OwnDeviceTicketDiscoveryPolicy { generation, .. }
            | Self::OwnDeviceRosterPolicy { generation, .. }
            | Self::AcceptedEndpointObservation { generation, .. } => *generation,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeTicketCheckpointContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    generation: u64,
    previous_checkpoint_id: Option<RuntimeTicketCheckpointId>,
    compacted_at_unix_seconds: u64,
    compacted_delta_records: u64,
    compacted_total_records: u64,
    compacted_delta_digest: [u8; 32],
    compacted_history_digest: [u8; 32],
    anchors: Vec<RuntimeTicketChainAnchor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimeTicketCheckpoint {
    content: RuntimeTicketCheckpointContent,
    signature: Vec<u8>,
}

impl SignedRuntimeTicketCheckpoint {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        compacted_at_unix_seconds: u64,
        removed_records: &[(String, [u8; 32])],
        mut anchors: Vec<RuntimeTicketChainAnchor>,
        previous: Option<&Self>,
    ) -> Result<Self> {
        ensure!(
            !removed_records.is_empty(),
            "runtime ticket checkpoint cannot compact an empty delta"
        );
        anchors.sort();
        validate_anchors(&anchors)?;
        let (generation, previous_checkpoint_id, previous_history_digest, previous_total) =
            match previous {
                Some(previous) => {
                    previous.verify_signature()?;
                    ensure!(
                        previous.local_account_id() == local_account_id
                            && previous.local_device_id() == identity.device_id(),
                        "runtime ticket checkpoint chain changes local identity"
                    );
                    ensure!(
                        compacted_at_unix_seconds >= previous.compacted_at_unix_seconds(),
                        "runtime ticket checkpoint time moves backwards"
                    );
                    validate_anchor_progress(previous.anchors(), &anchors)?;
                    (
                        previous
                            .generation()
                            .checked_add(1)
                            .context("runtime ticket checkpoint generation overflow")?,
                        Some(previous.checkpoint_id()?),
                        previous.compacted_history_digest(),
                        previous.compacted_total_records(),
                    )
                }
                None => (1, None, [0_u8; 32], 0),
            };
        let delta_digest = compacted_delta_digest(removed_records)?;
        let compacted_delta_records = u64::try_from(removed_records.len())
            .context("runtime ticket compacted delta count overflow")?;
        let compacted_total_records = previous_total
            .checked_add(compacted_delta_records)
            .context("runtime ticket compacted record count overflow")?;
        let compacted_history_digest = compacted_history_digest(
            previous_history_digest,
            previous_checkpoint_id,
            delta_digest,
            compacted_delta_records,
        );
        let content = RuntimeTicketCheckpointContent {
            version: CHECKPOINT_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            generation,
            previous_checkpoint_id,
            compacted_at_unix_seconds,
            compacted_delta_records,
            compacted_total_records,
            compacted_delta_digest: delta_digest,
            compacted_history_digest,
            anchors,
        };
        let signature = identity
            .sign(&signing_bytes(CHECKPOINT_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let checkpoint = Self { content, signature };
        checkpoint.verify(previous)?;
        Ok(checkpoint)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes = postcard::to_allocvec(self).context("encode runtime ticket checkpoint")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime ticket checkpoint is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime ticket checkpoint is too large"
        );
        let checkpoint: Self =
            postcard::from_bytes(bytes).context("decode runtime ticket checkpoint")?;
        checkpoint.verify_signature()?;
        Ok(checkpoint)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id(),
                    "runtime ticket checkpoint chain changes local identity"
                );
                ensure!(
                    self.generation()
                        == previous
                            .generation()
                            .checked_add(1)
                            .context("runtime ticket checkpoint generation overflow")?
                        && self.previous_checkpoint_id() == Some(previous.checkpoint_id()?)
                        && self.compacted_at_unix_seconds() >= previous.compacted_at_unix_seconds()
                        && self.compacted_total_records()
                            == previous
                                .compacted_total_records()
                                .checked_add(self.compacted_delta_records())
                                .context("runtime ticket compacted record count overflow")?,
                    "runtime ticket checkpoint chain is not contiguous"
                );
                ensure!(
                    self.compacted_history_digest()
                        == compacted_history_digest(
                            previous.compacted_history_digest(),
                            self.previous_checkpoint_id(),
                            self.compacted_delta_digest(),
                            self.compacted_delta_records(),
                        ),
                    "runtime ticket checkpoint history digest is invalid"
                );
                validate_anchor_progress(previous.anchors(), self.anchors())?;
            }
            None => ensure!(
                self.generation() == 1
                    && self.previous_checkpoint_id().is_none()
                    && self.compacted_total_records() == self.compacted_delta_records(),
                "runtime ticket checkpoint has an invalid first generation"
            ),
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == CHECKPOINT_VERSION,
            "unsupported runtime ticket checkpoint version"
        );
        ensure!(
            self.content.generation != 0
                && self.content.compacted_delta_records != 0
                && self.content.compacted_total_records >= self.content.compacted_delta_records
                && self.content.compacted_delta_digest != [0_u8; 32]
                && self.content.compacted_history_digest != [0_u8; 32],
            "runtime ticket checkpoint counters are invalid"
        );
        ensure!(
            (self.content.generation == 1) == self.content.previous_checkpoint_id.is_none(),
            "runtime ticket checkpoint predecessor shape is invalid"
        );
        validate_anchors(&self.content.anchors)?;
        self.content
            .local_device_id
            .verify(
                &signing_bytes(CHECKPOINT_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify runtime ticket checkpoint signature")
    }

    pub fn checkpoint_id(&self) -> Result<RuntimeTicketCheckpointId> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHECKPOINT_ID_DOMAIN);
        hasher.update(&self.encode()?);
        Ok(RuntimeTicketCheckpointId(*hasher.finalize().as_bytes()))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
    pub fn generation(&self) -> u64 {
        self.content.generation
    }
    pub fn previous_checkpoint_id(&self) -> Option<RuntimeTicketCheckpointId> {
        self.content.previous_checkpoint_id
    }
    pub fn compacted_at_unix_seconds(&self) -> u64 {
        self.content.compacted_at_unix_seconds
    }
    pub fn compacted_delta_records(&self) -> u64 {
        self.content.compacted_delta_records
    }
    pub fn compacted_total_records(&self) -> u64 {
        self.content.compacted_total_records
    }
    pub fn compacted_delta_digest(&self) -> [u8; 32] {
        self.content.compacted_delta_digest
    }
    pub fn compacted_history_digest(&self) -> [u8; 32] {
        self.content.compacted_history_digest
    }
    pub fn anchors(&self) -> &[RuntimeTicketChainAnchor] {
        &self.content.anchors
    }

    pub fn publication_anchor(
        &self,
        expected_channel: TicketPublicationChannelId,
    ) -> Option<(u64, TicketPublicationId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::Publication {
                channel_id,
                generation,
                record_id,
            } if *channel_id == expected_channel => Some((*generation, *record_id)),
            _ => None,
        })
    }

    pub fn observation_anchor(
        &self,
        expected_channel: TicketPublicationChannelId,
    ) -> Option<(u64, u64, TicketPublicationObservationId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::Observation {
                channel_id,
                generation,
                publication_generation,
                record_id,
            } if *channel_id == expected_channel => {
                Some((*generation, *publication_generation, *record_id))
            }
            _ => None,
        })
    }

    pub fn policy_anchor(
        &self,
        expected_contact: RuntimeContactId,
    ) -> Option<(u64, TicketAutomationPolicyId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::Policy {
                contact_id,
                generation,
                record_id,
            } if *contact_id == expected_contact => Some((*generation, *record_id)),
            _ => None,
        })
    }

    pub fn attempt_anchor(
        &self,
        expected_contact: RuntimeContactId,
        expected_action: TicketAutomationAction,
    ) -> Option<(u64, u64, TicketAutomationAttemptId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::Attempt {
                contact_id,
                action,
                generation,
                policy_generation,
                record_id,
            } if *contact_id == expected_contact && *action == expected_action => {
                Some((*generation, *policy_generation, *record_id))
            }
            _ => None,
        })
    }

    pub fn own_device_policy_anchor(
        &self,
        expected_recipient: DeviceId,
    ) -> Option<(u64, OwnDeviceAnnouncementPolicyId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::OwnDevicePolicy {
                recipient_device_id,
                generation,
                record_id,
            } if *recipient_device_id == expected_recipient => Some((*generation, *record_id)),
            _ => None,
        })
    }

    pub fn own_device_attempt_anchor(
        &self,
        expected_recipient: DeviceId,
    ) -> Option<(u64, u64, OwnDeviceAnnouncementAttemptId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::OwnDeviceAttempt {
                recipient_device_id,
                generation,
                policy_generation,
                record_id,
            } if *recipient_device_id == expected_recipient => {
                Some((*generation, *policy_generation, *record_id))
            }
            _ => None,
        })
    }

    pub fn own_device_ticket_discovery_policy_anchor(
        &self,
        expected_recipient: DeviceId,
    ) -> Option<(u64, OwnDeviceTicketDiscoveryPolicyId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::OwnDeviceTicketDiscoveryPolicy {
                recipient_device_id,
                generation,
                record_id,
            } if *recipient_device_id == expected_recipient => Some((*generation, *record_id)),
            _ => None,
        })
    }

    pub fn own_device_roster_policy_anchor(&self) -> Option<(u64, OwnDeviceRosterPolicyId)> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::OwnDeviceRosterPolicy {
                generation,
                record_id,
            } => Some((*generation, *record_id)),
            _ => None,
        })
    }

    pub fn accepted_endpoint_observation_anchor(
        &self,
        expected_channel: TicketPublicationChannelId,
    ) -> Option<(
        u64,
        TicketPublicationId,
        [u8; 32],
        AcceptedEndpointObservationId,
    )> {
        self.content.anchors.iter().find_map(|anchor| match anchor {
            RuntimeTicketChainAnchor::AcceptedEndpointObservation {
                channel_id,
                generation,
                publication_id,
                ticket_digest,
                record_id,
            } if *channel_id == expected_channel => {
                Some((*generation, *publication_id, *ticket_digest, *record_id))
            }
            _ => None,
        })
    }
}

fn validate_anchors(anchors: &[RuntimeTicketChainAnchor]) -> Result<()> {
    ensure!(
        !anchors.is_empty() && anchors.len() <= MAX_RUNTIME_TICKET_CHECKPOINT_ANCHORS,
        "runtime ticket checkpoint anchor count is invalid"
    );
    ensure!(
        anchors.windows(2).all(|pair| pair[0] < pair[1]),
        "runtime ticket checkpoint anchors are not canonical"
    );
    let mut keys = BTreeSet::new();
    for anchor in anchors {
        ensure!(
            anchor.generation() != 0 && keys.insert(anchor.key()),
            "runtime ticket checkpoint contains an invalid or duplicate chain anchor"
        );
        match anchor {
            RuntimeTicketChainAnchor::Observation {
                publication_generation,
                ..
            }
            | RuntimeTicketChainAnchor::Attempt {
                policy_generation: publication_generation,
                ..
            }
            | RuntimeTicketChainAnchor::OwnDeviceAttempt {
                policy_generation: publication_generation,
                ..
            } => ensure!(
                *publication_generation != 0,
                "runtime ticket checkpoint contains a zero high-water generation"
            ),
            RuntimeTicketChainAnchor::Publication { .. }
            | RuntimeTicketChainAnchor::Policy { .. }
            | RuntimeTicketChainAnchor::OwnDevicePolicy { .. }
            | RuntimeTicketChainAnchor::OwnDeviceTicketDiscoveryPolicy { .. }
            | RuntimeTicketChainAnchor::OwnDeviceRosterPolicy { .. }
            | RuntimeTicketChainAnchor::AcceptedEndpointObservation { .. } => {}
        }
    }
    Ok(())
}

fn validate_anchor_progress(
    previous: &[RuntimeTicketChainAnchor],
    current: &[RuntimeTicketChainAnchor],
) -> Result<()> {
    for old in previous {
        let new = current
            .iter()
            .find(|candidate| candidate.key() == old.key())
            .context("runtime ticket checkpoint drops an authenticated chain")?;
        ensure!(
            new.generation() > old.generation()
                || (new.generation() == old.generation() && new == old),
            "runtime ticket checkpoint rolls back an authenticated chain"
        );
    }
    Ok(())
}

fn compacted_delta_digest(records: &[(String, [u8; 32])]) -> Result<[u8; 32]> {
    let mut records = records.to_vec();
    records.sort();
    ensure!(
        records.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "runtime ticket compacted delta contains duplicate or unordered paths"
    );
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTED_DELTA_DIGEST_DOMAIN);
    for (path, digest) in records {
        ensure!(
            !path.is_empty() && path.len() <= 4_096,
            "runtime ticket compacted path is invalid"
        );
        hasher.update(&(path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&digest);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn compacted_history_digest(
    previous_history_digest: [u8; 32],
    previous_checkpoint_id: Option<RuntimeTicketCheckpointId>,
    delta_digest: [u8; 32],
    delta_records: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTED_HISTORY_DIGEST_DOMAIN);
    hasher.update(&previous_history_digest);
    match previous_checkpoint_id {
        Some(id) => hasher.update(id.as_bytes()),
        None => hasher.update(&[0_u8; 32]),
    };
    hasher.update(&delta_digest);
    hasher.update(&delta_records.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut encoded = Vec::from(domain);
    encoded.extend(postcard::to_allocvec(content).context("encode runtime ticket signed content")?);
    Ok(encoded)
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
