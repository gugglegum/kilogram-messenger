use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
};

use anyhow::{Context, Result, ensure};
use kilogram_mailbox::{
    DEFAULT_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS,
    MAX_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS, MAX_MAILBOX_STORAGE_OFFER_BYTES,
    MailboxStoragePolicyClass, MailboxStoreKey, SignedMailboxStorageOffer,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};

const DATABASE_FILE: &str = "mailbox-provider-registry.redb";
const OFFER_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-provider-offers-v1");
const GOSSIP_HOP_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-provider-gossip-hops-v1");
const AUTHENTICATED_OBSERVATION_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-provider-authenticated-observations-v1");
const RECORD_VERSION: u8 = 1;
const AUTHENTICATED_OBSERVATION_RECORD_VERSION: u8 = 1;
const OFFER_ID_DOMAIN: &[u8] = b"kilogram:mailbox-provider-offer-id:v1\0";
const SELECTION_DOMAIN: &[u8] = b"kilogram:mailbox-provider-selection:v1\0";
const GOSSIP_SELECTION_DOMAIN: &[u8] = b"kilogram:mailbox-provider-gossip-selection:v1\0";
const GOSSIP_FRAME_ID_DOMAIN: &[u8] = b"kilogram:mailbox-provider-gossip-frame-id:v1\0";
const MAX_PROVIDER_RECORD_BYTES: usize = MAX_MAILBOX_STORAGE_OFFER_BYTES + 256;
const AUTHENTICATED_OBSERVATION_KEY_BYTES: usize = 64;
const MAX_AUTHENTICATED_OBSERVATION_RECORD_BYTES: usize = 160;
const MAX_ABSOLUTE_PROVIDER_OFFERS: u64 = 4_096;
const GOSSIP_FRAME_VERSION: u8 = 1;

/// A conservative local default. The registry is discovery state, not an
/// unbounded cache of every volunteer ever observed.
pub const DEFAULT_MAX_PROVIDER_OFFERS: u64 = 256;

/// Replication fan-out remains deliberately small even if the registry is
/// full. A later policy may choose fewer providers for a particular item.
pub const MAX_PROVIDER_SELECTION: u8 = 8;

/// Offers below this cheap-to-verify, bounded creation cost remain readable for
/// compatibility but are not selected for new volunteer replica sets or gossip.
pub const DEFAULT_PROVIDER_ADMISSION_WORK_BITS: u8 =
    DEFAULT_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS;

/// Only a bounded number of distinct authenticated peer observations is kept
/// for one exact signed offer. The opaque tags are local-only and never enter
/// the offer, gossip frame or IPC projection.
pub const MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER: u8 = 8;

/// One authenticated peer observation is enough to prefer an offer over a
/// locally imported bootstrap candidate. Additional observing Devices do not
/// improve its rank because Devices are not treated as independent operators.
pub const MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE: u8 = 1;

/// No authenticated session can carry more than this many provider offers.
pub const MAX_PROVIDER_GOSSIP_OFFERS: u8 = 8;

/// A provider offer may cross at most two authenticated peer edges. This is a
/// bandwidth/privacy bound for honest clients, not a Sybil-resistance proof.
pub const MAX_PROVIDER_GOSSIP_HOPS: u8 = 2;

/// Old but still cryptographically valid offers are not amplified further.
pub const MAX_PROVIDER_GOSSIP_AGE_SECONDS: u64 = 15 * 60;

/// Oversized but otherwise valid endpoint offers remain available for local
/// explicit selection, but are not amplified through peer gossip.
pub const MAX_PROVIDER_GOSSIP_OFFER_BYTES: usize = 2 * 1024;

/// A frame itself is deliberately much shorter-lived than its signed offers.
pub const MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS: u64 = 60;

pub const MAX_PROVIDER_GOSSIP_FRAME_BYTES: usize =
    MAX_PROVIDER_GOSSIP_OFFER_BYTES * MAX_PROVIDER_GOSSIP_OFFERS as usize + 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MailboxProviderOfferId([u8; 32]);

impl MailboxProviderOfferId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A pseudonymous, installation-local handle derived by the runtime from an
/// already authenticated peer session. It must never be serialized into a
/// provider offer, gossip frame or IPC response.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MailboxProviderLocalObserverTag([u8; 32]);

impl MailboxProviderLocalObserverTag {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxProviderObservationOutcome {
    Added,
    Refreshed,
    CapacityReached,
}

impl fmt::Display for MailboxProviderOfferId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxProviderOffer {
    offer_id: MailboxProviderOfferId,
    transport_identity: [u8; 32],
    offer: SignedMailboxStorageOffer,
    encoded_offer: Vec<u8>,
    observed_at_unix_seconds: u64,
    observed_gossip_hops: u8,
    authenticated_observation_count: u8,
}

impl MailboxProviderOffer {
    pub fn offer_id(&self) -> MailboxProviderOfferId {
        self.offer_id
    }

    /// Opaque identity supplied by the transport adapter after parsing the
    /// signed endpoint. It is intentionally unrelated to Account or Device.
    pub fn transport_identity(&self) -> &[u8; 32] {
        &self.transport_identity
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        self.offer.store_key()
    }

    pub fn policy_class(&self) -> MailboxStoragePolicyClass {
        self.offer.policy_class()
    }

    pub fn capacity_hint_bytes(&self) -> u64 {
        self.offer.capacity_hint_bytes()
    }

    pub fn max_record_bytes(&self) -> u64 {
        self.offer.max_record_bytes()
    }

    pub fn issued_at_unix_seconds(&self) -> u64 {
        self.offer.issued_at_unix_seconds()
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.offer.expires_at_unix_seconds()
    }

    pub fn observed_at_unix_seconds(&self) -> u64 {
        self.observed_at_unix_seconds
    }

    pub fn observed_gossip_hops(&self) -> u8 {
        self.observed_gossip_hops
    }

    pub fn admission_work_bits(&self) -> u16 {
        self.offer.admission_work_bits()
    }

    pub fn authenticated_observation_count(&self) -> u8 {
        self.authenticated_observation_count
    }

    /// Exact signed bytes are returned for the transport adapter to decode and
    /// dial. No mailbox read/write capability is part of this object.
    pub fn encoded_offer(&self) -> &[u8] {
        &self.encoded_offer
    }

    pub fn signed_offer(&self) -> &SignedMailboxStorageOffer {
        &self.offer
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxProviderImportOutcome {
    Inserted,
    Replaced,
    AlreadyPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxProviderRegistryConfig {
    pub data_dir: PathBuf,
    pub max_offers: u64,
}

impl MailboxProviderRegistryConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            max_offers: DEFAULT_MAX_PROVIDER_OFFERS,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.data_dir.as_os_str().is_empty(),
            "mailbox provider registry directory is empty"
        );
        ensure!(
            (1..=MAX_ABSOLUTE_PROVIDER_OFFERS).contains(&self.max_offers),
            "mailbox provider registry capacity is invalid"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProviderOfferRecord {
    version: u8,
    store_key: MailboxStoreKey,
    transport_identity: [u8; 32],
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    encoded_offer: Vec<u8>,
    observed_at_unix_seconds: u64,
}

impl ProviderOfferRecord {
    fn from_verified_offer(
        offer: &SignedMailboxStorageOffer,
        encoded_offer: Vec<u8>,
        transport_identity: [u8; 32],
        observed_at_unix_seconds: u64,
    ) -> Self {
        Self {
            version: RECORD_VERSION,
            store_key: offer.store_key(),
            transport_identity,
            issued_at_unix_seconds: offer.issued_at_unix_seconds(),
            expires_at_unix_seconds: offer.expires_at_unix_seconds(),
            encoded_offer,
            observed_at_unix_seconds,
        }
    }

    fn validate_and_offer(&self) -> Result<SignedMailboxStorageOffer> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported mailbox provider offer record"
        );
        ensure!(
            !self.encoded_offer.is_empty()
                && self.encoded_offer.len() <= MAX_MAILBOX_STORAGE_OFFER_BYTES,
            "stored mailbox provider offer size is invalid"
        );
        // Verify historical records at their signed issue instant so an
        // expired offer can still be authenticated before pruning.
        let offer = SignedMailboxStorageOffer::decode_and_verify(
            &self.encoded_offer,
            self.issued_at_unix_seconds,
        )
        .context("verify stored mailbox provider offer")?;
        ensure!(
            offer.store_key() == self.store_key
                && offer.issued_at_unix_seconds() == self.issued_at_unix_seconds
                && offer.expires_at_unix_seconds() == self.expires_at_unix_seconds
                && self.observed_at_unix_seconds >= self.issued_at_unix_seconds
                && self.observed_at_unix_seconds < self.expires_at_unix_seconds
                && offer.encode(self.issued_at_unix_seconds)? == self.encoded_offer,
            "stored mailbox provider offer metadata is inconsistent"
        );
        Ok(offer)
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate_and_offer()?;
        let bytes = postcard::to_allocvec(self).context("encode mailbox provider offer record")?;
        ensure!(
            bytes.len() <= MAX_PROVIDER_RECORD_BYTES,
            "mailbox provider offer record is too large"
        );
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_PROVIDER_RECORD_BYTES,
            "mailbox provider offer record is too large"
        );
        let record: Self =
            postcard::from_bytes(bytes).context("decode mailbox provider offer record")?;
        record.validate_and_offer()?;
        Ok(record)
    }

    fn into_public(
        self,
        now_unix_seconds: u64,
        observed_gossip_hops: u8,
        authenticated_observation_count: u8,
    ) -> Result<MailboxProviderOffer> {
        ensure!(
            observed_gossip_hops <= MAX_PROVIDER_GOSSIP_HOPS,
            "stored mailbox provider gossip hop count is invalid"
        );
        ensure!(
            authenticated_observation_count <= MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER,
            "stored mailbox provider authenticated observation count is invalid"
        );
        let offer =
            SignedMailboxStorageOffer::decode_and_verify(&self.encoded_offer, now_unix_seconds)
                .context("verify active mailbox provider offer")?;
        let offer_id = offer_id(&self.encoded_offer);
        Ok(MailboxProviderOffer {
            offer_id,
            transport_identity: self.transport_identity,
            offer,
            encoded_offer: self.encoded_offer,
            observed_at_unix_seconds: self.observed_at_unix_seconds,
            observed_gossip_hops,
            authenticated_observation_count,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AuthenticatedProviderObservationRecord {
    version: u8,
    offer_id: [u8; 32],
    observer_tag: [u8; 32],
    first_observed_at_unix_seconds: u64,
    last_observed_at_unix_seconds: u64,
}

impl AuthenticatedProviderObservationRecord {
    fn new(
        offer_id: MailboxProviderOfferId,
        observer_tag: MailboxProviderLocalObserverTag,
        observed_at_unix_seconds: u64,
    ) -> Self {
        Self {
            version: AUTHENTICATED_OBSERVATION_RECORD_VERSION,
            offer_id: *offer_id.as_bytes(),
            observer_tag: *observer_tag.as_bytes(),
            first_observed_at_unix_seconds: observed_at_unix_seconds,
            last_observed_at_unix_seconds: observed_at_unix_seconds,
        }
    }

    fn key(&self) -> [u8; AUTHENTICATED_OBSERVATION_KEY_BYTES] {
        authenticated_observation_key(self.offer_id, self.observer_tag)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == AUTHENTICATED_OBSERVATION_RECORD_VERSION
                && self.first_observed_at_unix_seconds <= self.last_observed_at_unix_seconds,
            "stored mailbox provider authenticated observation is invalid"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self)
            .context("encode mailbox provider authenticated observation")?;
        ensure!(
            bytes.len() <= MAX_AUTHENTICATED_OBSERVATION_RECORD_BYTES,
            "mailbox provider authenticated observation is too large"
        );
        Ok(bytes)
    }

    fn decode(key: &[u8], bytes: &[u8]) -> Result<Self> {
        ensure!(
            key.len() == AUTHENTICATED_OBSERVATION_KEY_BYTES
                && bytes.len() <= MAX_AUTHENTICATED_OBSERVATION_RECORD_BYTES,
            "mailbox provider authenticated observation encoding is invalid"
        );
        let record: Self = postcard::from_bytes(bytes)
            .context("decode mailbox provider authenticated observation")?;
        record.validate()?;
        ensure!(
            record.key().as_slice() == key,
            "mailbox provider authenticated observation key is inconsistent"
        );
        Ok(record)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxProviderGossipEntry {
    transmitted_hops: u8,
    encoded_offer: Vec<u8>,
}

impl MailboxProviderGossipEntry {
    pub fn transmitted_hops(&self) -> u8 {
        self.transmitted_hops
    }

    pub fn encoded_offer(&self) -> &[u8] {
        &self.encoded_offer
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxProviderGossipFrame {
    version: u8,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    entropy: [u8; 32],
    reply_to: Option<[u8; 32]>,
    entries: Vec<MailboxProviderGossipEntry>,
}

impl MailboxProviderGossipFrame {
    pub fn from_registry(
        registry: &MailboxProviderRegistry,
        entropy: [u8; 32],
        reply_to: Option<[u8; 32]>,
        now_unix_seconds: u64,
    ) -> Result<Self> {
        let entries = registry
            .select_for_gossip(entropy, MAX_PROVIDER_GOSSIP_OFFERS, now_unix_seconds)?
            .into_iter()
            .map(|offer| MailboxProviderGossipEntry {
                transmitted_hops: offer.observed_gossip_hops.saturating_add(1),
                encoded_offer: offer.encoded_offer,
            })
            .collect();
        let frame = Self {
            version: GOSSIP_FRAME_VERSION,
            created_at_unix_seconds: now_unix_seconds,
            expires_at_unix_seconds: now_unix_seconds
                .checked_add(MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS)
                .context("mailbox provider gossip expiry overflow")?,
            entropy,
            reply_to,
            entries,
        };
        frame.validate(now_unix_seconds)?;
        Ok(frame)
    }

    pub fn decode_and_verify(bytes: &[u8], now_unix_seconds: u64) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PROVIDER_GOSSIP_FRAME_BYTES,
            "mailbox provider gossip frame size is invalid"
        );
        let frame: Self =
            postcard::from_bytes(bytes).context("decode mailbox provider gossip frame")?;
        frame.validate(now_unix_seconds)?;
        ensure!(
            postcard::to_allocvec(&frame)? == bytes,
            "mailbox provider gossip frame encoding is not canonical"
        );
        Ok(frame)
    }

    pub fn encode(&self, now_unix_seconds: u64) -> Result<Vec<u8>> {
        self.validate(now_unix_seconds)?;
        let bytes = postcard::to_allocvec(self).context("encode mailbox provider gossip frame")?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PROVIDER_GOSSIP_FRAME_BYTES,
            "mailbox provider gossip frame size is invalid"
        );
        Ok(bytes)
    }

    pub fn frame_id(&self) -> Result<[u8; 32]> {
        let bytes =
            postcard::to_allocvec(self).context("encode mailbox provider gossip frame ID")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(GOSSIP_FRAME_ID_DOMAIN);
        hasher.update(&bytes);
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn reply_to(&self) -> Option<[u8; 32]> {
        self.reply_to
    }

    pub fn entries(&self) -> &[MailboxProviderGossipEntry] {
        &self.entries
    }

    fn validate(&self, now_unix_seconds: u64) -> Result<()> {
        ensure!(
            self.version == GOSSIP_FRAME_VERSION,
            "unsupported mailbox provider gossip frame"
        );
        ensure!(
            self.created_at_unix_seconds <= now_unix_seconds
                && now_unix_seconds < self.expires_at_unix_seconds
                && self.expires_at_unix_seconds - self.created_at_unix_seconds
                    <= MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS,
            "mailbox provider gossip frame is outside its validity window"
        );
        ensure!(
            self.entries.len() <= usize::from(MAX_PROVIDER_GOSSIP_OFFERS),
            "mailbox provider gossip frame contains too many offers"
        );
        let mut store_keys = BTreeSet::new();
        for entry in &self.entries {
            ensure!(
                (1..=MAX_PROVIDER_GOSSIP_HOPS).contains(&entry.transmitted_hops),
                "mailbox provider gossip hop count is invalid"
            );
            ensure!(
                !entry.encoded_offer.is_empty()
                    && entry.encoded_offer.len() <= MAX_PROVIDER_GOSSIP_OFFER_BYTES,
                "mailbox provider gossip offer is too large to amplify"
            );
            let offer = SignedMailboxStorageOffer::decode_and_verify(
                &entry.encoded_offer,
                now_unix_seconds,
            )
            .context("verify gossiped mailbox provider offer")?;
            ensure!(
                now_unix_seconds.saturating_sub(offer.issued_at_unix_seconds())
                    <= MAX_PROVIDER_GOSSIP_AGE_SECONDS,
                "mailbox provider gossip offer is too old to relay"
            );
            ensure!(
                offer.encode(now_unix_seconds)? == entry.encoded_offer,
                "gossiped mailbox provider offer encoding is not canonical"
            );
            ensure!(
                store_keys.insert(offer.store_key()),
                "mailbox provider gossip frame repeats a store key"
            );
        }
        Ok(())
    }
}

pub struct MailboxProviderRegistry {
    database: Database,
    config: MailboxProviderRegistryConfig,
}

impl MailboxProviderRegistry {
    /// Read provider discovery state without opening the redb file for write.
    /// Writable redb open/close bookkeeping may change database bytes even
    /// when no logical record changes, which would invalidate a byte-exact
    /// authenticated state-vault mirror.
    pub fn active_offers_read_only(
        config: MailboxProviderRegistryConfig,
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        config.validate()?;
        let Some(database) = open_provider_registry_read_only(&config)? else {
            return Ok(Vec::new());
        };
        active_offers_from_database(&database, now_unix_seconds)
    }

    /// Resolve an authenticated replica set by indexed store-key lookup while
    /// retaining the same no-write guarantee as `active_offers_read_only`.
    pub fn active_offers_for_store_keys_read_only(
        config: MailboxProviderRegistryConfig,
        store_keys: &[MailboxStoreKey],
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        validate_replica_set_lookup_keys(store_keys)?;
        config.validate()?;
        let Some(database) = open_provider_registry_read_only(&config)? else {
            return Ok(Vec::new());
        };
        active_offers_for_store_keys_from_database(&database, store_keys, now_unix_seconds)
    }

    pub fn open(config: MailboxProviderRegistryConfig) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "create mailbox provider registry directory {}",
                config.data_dir.display()
            )
        })?;
        let metadata = std::fs::symlink_metadata(&config.data_dir)
            .context("inspect mailbox provider registry directory")?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "mailbox provider registry directory must be a real directory, not a symlink"
        );
        let canonical = std::fs::canonicalize(&config.data_dir)
            .context("canonicalize mailbox provider registry directory")?;
        let database = Database::create(canonical.join(DATABASE_FILE))
            .context("open mailbox provider registry")?;
        let write = database
            .begin_write()
            .context("begin mailbox provider registry initialization")?;
        write.open_table(OFFER_TABLE)?;
        write.open_table(GOSSIP_HOP_TABLE)?;
        write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
        write
            .commit()
            .context("commit mailbox provider registry initialization")?;
        Ok(Self { database, config })
    }

    /// Import a signed offer after the transport adapter has parsed its
    /// endpoint and supplied the stable endpoint identity. Callers must derive
    /// `transport_identity` from that endpoint, never from Account or Device.
    pub fn import_offer(
        &self,
        encoded_offer: &[u8],
        transport_identity: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<(MailboxProviderImportOutcome, MailboxProviderOffer)> {
        let (outcome, observation, offer) = self.import_offer_with_gossip_hops(
            encoded_offer,
            transport_identity,
            0,
            None,
            now_unix_seconds,
        )?;
        ensure!(
            observation.is_none(),
            "direct mailbox provider import created authenticated peer provenance"
        );
        Ok((outcome, offer))
    }

    pub fn import_gossiped_offer(
        &self,
        encoded_offer: &[u8],
        transport_identity: [u8; 32],
        transmitted_hops: u8,
        authenticated_observer: MailboxProviderLocalObserverTag,
        now_unix_seconds: u64,
    ) -> Result<(
        MailboxProviderImportOutcome,
        MailboxProviderObservationOutcome,
        MailboxProviderOffer,
    )> {
        ensure!(
            (1..=MAX_PROVIDER_GOSSIP_HOPS).contains(&transmitted_hops),
            "mailbox provider gossip hop count is invalid"
        );
        let (outcome, observation, offer) = self.import_offer_with_gossip_hops(
            encoded_offer,
            transport_identity,
            transmitted_hops,
            Some(authenticated_observer),
            now_unix_seconds,
        )?;
        Ok((
            outcome,
            observation.context("authenticated gossip import omitted local provenance")?,
            offer,
        ))
    }

    fn import_offer_with_gossip_hops(
        &self,
        encoded_offer: &[u8],
        transport_identity: [u8; 32],
        observed_gossip_hops: u8,
        authenticated_observer: Option<MailboxProviderLocalObserverTag>,
        now_unix_seconds: u64,
    ) -> Result<(
        MailboxProviderImportOutcome,
        Option<MailboxProviderObservationOutcome>,
        MailboxProviderOffer,
    )> {
        ensure!(
            observed_gossip_hops <= MAX_PROVIDER_GOSSIP_HOPS,
            "mailbox provider gossip hop count is invalid"
        );
        let offer = SignedMailboxStorageOffer::decode_and_verify(encoded_offer, now_unix_seconds)
            .context("verify imported mailbox provider offer")?;
        let canonical = offer
            .encode(now_unix_seconds)
            .context("canonicalize imported mailbox provider offer")?;
        ensure!(
            canonical == encoded_offer,
            "mailbox provider offer encoding is not canonical"
        );
        let record = ProviderOfferRecord::from_verified_offer(
            &offer,
            canonical,
            transport_identity,
            now_unix_seconds,
        );
        let encoded_record = record.encode()?;
        let key = record.store_key.as_bytes();

        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox provider offer import")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox provider import durability")?;

        let expired = {
            let table = write.open_table(OFFER_TABLE)?;
            let mut expired = Vec::new();
            for entry in table.iter()? {
                let (candidate_key, candidate_value) = entry?;
                let candidate = ProviderOfferRecord::decode(candidate_value.value())?;
                ensure!(
                    candidate_key.value() == candidate.store_key.as_bytes(),
                    "mailbox provider registry key does not match its signed offer"
                );
                if candidate.expires_at_unix_seconds <= now_unix_seconds {
                    expired.push((
                        candidate_key.value().to_vec(),
                        *offer_id(&candidate.encoded_offer).as_bytes(),
                    ));
                }
            }
            expired
        };
        {
            let mut table = write.open_table(OFFER_TABLE)?;
            for (expired_key, _) in &expired {
                table.remove(expired_key.as_slice())?;
            }
        }
        {
            let mut table = write.open_table(GOSSIP_HOP_TABLE)?;
            for (expired_key, _) in &expired {
                table.remove(expired_key.as_slice())?;
            }
        }
        let expired_offer_ids = expired
            .iter()
            .map(|(_, offer_id)| *offer_id)
            .collect::<BTreeSet<_>>();
        if !expired_offer_ids.is_empty() {
            let observation_keys = {
                let table = write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
                let mut keys = Vec::new();
                for entry in table.iter()? {
                    let (observation_key, observation_value) = entry?;
                    let observation = AuthenticatedProviderObservationRecord::decode(
                        observation_key.value(),
                        observation_value.value(),
                    )?;
                    if expired_offer_ids.contains(&observation.offer_id) {
                        keys.push(observation_key.value().to_vec());
                    }
                }
                keys
            };
            let mut table = write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
            for observation_key in observation_keys {
                table.remove(observation_key.as_slice())?;
            }
        }

        let current = write
            .open_table(OFFER_TABLE)?
            .get(key.as_slice())?
            .map(|value| value.value().to_vec());
        let current_gossip_hops = write
            .open_table(GOSSIP_HOP_TABLE)?
            .get(key.as_slice())?
            .map(|value| decode_gossip_hops(value.value()))
            .transpose()?
            .unwrap_or(0);
        let (outcome, effective_record, effective_gossip_hops, replaced_offer_id) =
            if let Some(current) = current {
                let current = ProviderOfferRecord::decode(&current)?;
                if current.encoded_offer == record.encoded_offer
                    && current.transport_identity == record.transport_identity
                {
                    (
                        MailboxProviderImportOutcome::AlreadyPresent,
                        current,
                        if observed_gossip_hops == 0 {
                            0
                        } else {
                            current_gossip_hops
                        },
                        None,
                    )
                } else {
                    ensure!(
                        record.issued_at_unix_seconds > current.issued_at_unix_seconds,
                        "mailbox provider offer does not advance the current signed offer"
                    );
                    write
                        .open_table(OFFER_TABLE)?
                        .insert(key.as_slice(), encoded_record.as_slice())?;
                    (
                        MailboxProviderImportOutcome::Replaced,
                        record.clone(),
                        observed_gossip_hops,
                        Some(*offer_id(&current.encoded_offer).as_bytes()),
                    )
                }
            } else {
                let count = write.open_table(OFFER_TABLE)?.len()?;
                ensure!(
                    count < self.config.max_offers,
                    "mailbox provider registry capacity exceeded"
                );
                write
                    .open_table(OFFER_TABLE)?
                    .insert(key.as_slice(), encoded_record.as_slice())?;
                (
                    MailboxProviderImportOutcome::Inserted,
                    record.clone(),
                    observed_gossip_hops,
                    None,
                )
            };
        if let Some(replaced_offer_id) = replaced_offer_id {
            let observation_keys = {
                let table = write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
                let mut keys = Vec::new();
                for entry in table.iter()? {
                    let (observation_key, observation_value) = entry?;
                    let observation = AuthenticatedProviderObservationRecord::decode(
                        observation_key.value(),
                        observation_value.value(),
                    )?;
                    if observation.offer_id == replaced_offer_id {
                        keys.push(observation_key.value().to_vec());
                    }
                }
                keys
            };
            let mut table = write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
            for observation_key in observation_keys {
                table.remove(observation_key.as_slice())?;
            }
        }
        let encoded_gossip_hops = [effective_gossip_hops];
        write
            .open_table(GOSSIP_HOP_TABLE)?
            .insert(key.as_slice(), encoded_gossip_hops.as_slice())?;
        let effective_offer_id = offer_id(&effective_record.encoded_offer);
        let (observation_outcome, authenticated_observation_count) = {
            let mut table = write.open_table(AUTHENTICATED_OBSERVATION_TABLE)?;
            let mut count = 0_u8;
            for entry in table.iter()? {
                let (observation_key, observation_value) = entry?;
                let observation = AuthenticatedProviderObservationRecord::decode(
                    observation_key.value(),
                    observation_value.value(),
                )?;
                if observation.offer_id == *effective_offer_id.as_bytes() {
                    count = count
                        .checked_add(1)
                        .context("mailbox provider authenticated observation count overflow")?;
                }
            }
            ensure!(
                count <= MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER,
                "mailbox provider authenticated observation capacity exceeded"
            );
            if let Some(observer_tag) = authenticated_observer {
                let observation = AuthenticatedProviderObservationRecord::new(
                    effective_offer_id,
                    observer_tag,
                    now_unix_seconds,
                );
                let observation_key = observation.key();
                let existing = table
                    .get(observation_key.as_slice())?
                    .map(|value| value.value().to_vec());
                if let Some(existing) = existing {
                    let mut existing = AuthenticatedProviderObservationRecord::decode(
                        observation_key.as_slice(),
                        &existing,
                    )?;
                    ensure!(
                        existing.offer_id == *effective_offer_id.as_bytes()
                            && existing.observer_tag == *observer_tag.as_bytes(),
                        "mailbox provider authenticated observation identity is inconsistent"
                    );
                    existing.last_observed_at_unix_seconds =
                        existing.last_observed_at_unix_seconds.max(now_unix_seconds);
                    table.insert(observation_key.as_slice(), existing.encode()?.as_slice())?;
                    (Some(MailboxProviderObservationOutcome::Refreshed), count)
                } else if count >= MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER {
                    (
                        Some(MailboxProviderObservationOutcome::CapacityReached),
                        count,
                    )
                } else {
                    table.insert(observation_key.as_slice(), observation.encode()?.as_slice())?;
                    (Some(MailboxProviderObservationOutcome::Added), count + 1)
                }
            } else {
                (None, count)
            }
        };
        write
            .commit()
            .context("commit mailbox provider offer import")?;
        Ok((
            outcome,
            observation_outcome,
            effective_record.into_public(
                now_unix_seconds,
                effective_gossip_hops,
                authenticated_observation_count,
            )?,
        ))
    }

    pub fn active_offers(&self, now_unix_seconds: u64) -> Result<Vec<MailboxProviderOffer>> {
        active_offers_from_database(&self.database, now_unix_seconds)
    }

    /// Resolve a recipient-authenticated, bounded replica-set commitment by
    /// exact store key. This performs indexed lookups instead of sampling or
    /// scanning the provider registry. Missing/expired providers are omitted
    /// so the caller can wait for a fresh signed offer without changing the
    /// committed set.
    pub fn active_offers_for_store_keys(
        &self,
        store_keys: &[MailboxStoreKey],
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        validate_replica_set_lookup_keys(store_keys)?;
        active_offers_for_store_keys_from_database(&self.database, store_keys, now_unix_seconds)
    }

    /// Deterministic rendezvous-style selection. The caller supplies a fresh,
    /// opaque per-item salt; using an Account, Device, conversation or mailbox
    /// capability as the salt is forbidden by the protocol boundary.
    pub fn select(
        &self,
        selection_salt: [u8; 32],
        requested: u8,
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        ensure!(
            (1..=MAX_PROVIDER_SELECTION).contains(&requested),
            "mailbox provider selection size is invalid"
        );
        select_active_offers(
            self.active_offers(now_unix_seconds)?,
            selection_salt,
            requested,
        )
    }

    /// Apply the same bounded rendezvous selection to an already authenticated
    /// read-only snapshot. This lets long-running runtimes avoid reopening the
    /// provider registry in writable mode merely to choose peers.
    pub fn select_from_active_offers(
        active_offers: Vec<MailboxProviderOffer>,
        selection_salt: [u8; 32],
        requested: u8,
    ) -> Result<Vec<MailboxProviderOffer>> {
        select_active_offers(active_offers, selection_salt, requested)
    }

    /// Select admission-qualified offers with a bootstrap-safe binary local
    /// corroboration preference. Authenticated observation count above one
    /// adds no rank, and unobserved offers fill every remaining slot.
    pub fn select_bootstrap_safe(
        &self,
        selection_salt: [u8; 32],
        requested: u8,
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        select_bootstrap_safe_active_offers(
            self.active_offers(now_unix_seconds)?,
            selection_salt,
            requested,
            DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
        )
    }

    pub fn select_bootstrap_safe_from_active_offers(
        active_offers: Vec<MailboxProviderOffer>,
        selection_salt: [u8; 32],
        requested: u8,
    ) -> Result<Vec<MailboxProviderOffer>> {
        select_bootstrap_safe_active_offers(
            active_offers,
            selection_salt,
            requested,
            DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
        )
    }

    pub fn select_for_gossip(
        &self,
        selection_salt: [u8; 32],
        requested: u8,
        now_unix_seconds: u64,
    ) -> Result<Vec<MailboxProviderOffer>> {
        ensure!(
            (1..=MAX_PROVIDER_GOSSIP_OFFERS).contains(&requested),
            "mailbox provider gossip selection size is invalid"
        );
        let mut ranked = self
            .active_offers(now_unix_seconds)?
            .into_iter()
            .filter(|offer| {
                offer.admission_work_bits() >= u16::from(DEFAULT_PROVIDER_ADMISSION_WORK_BITS)
                    && offer.observed_gossip_hops < MAX_PROVIDER_GOSSIP_HOPS
                    && offer.encoded_offer.len() <= MAX_PROVIDER_GOSSIP_OFFER_BYTES
                    && now_unix_seconds.saturating_sub(offer.issued_at_unix_seconds())
                        <= MAX_PROVIDER_GOSSIP_AGE_SECONDS
            })
            .map(|offer| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(GOSSIP_SELECTION_DOMAIN);
                hasher.update(&selection_salt);
                hasher.update(offer.store_key().as_bytes());
                hasher.update(offer.transport_identity());
                hasher.update(offer.offer_id().as_bytes());
                (*hasher.finalize().as_bytes(), offer)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.store_key().cmp(&right.1.store_key()))
        });

        let mut selected_identities = BTreeSet::new();
        let mut selected = Vec::new();
        for (_, offer) in ranked {
            if selected_identities.insert(*offer.transport_identity()) {
                selected.push(offer);
                if selected.len() == usize::from(requested) {
                    break;
                }
            }
        }
        Ok(selected)
    }

    pub fn count(&self) -> Result<u64> {
        let read = self
            .database
            .begin_read()
            .context("begin mailbox provider registry count")?;
        Ok(read.open_table(OFFER_TABLE)?.len()?)
    }
}

fn open_provider_registry_read_only(
    config: &MailboxProviderRegistryConfig,
) -> Result<Option<ReadOnlyDatabase>> {
    let directory_metadata = match std::fs::symlink_metadata(&config.data_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspect mailbox provider registry directory {}",
                    config.data_dir.display()
                )
            });
        }
    };
    ensure!(
        directory_metadata.is_dir() && !directory_metadata.file_type().is_symlink(),
        "mailbox provider registry directory must be a real directory, not a symlink"
    );
    let canonical = std::fs::canonicalize(&config.data_dir)
        .context("canonicalize mailbox provider registry directory")?;
    let database_path = canonical.join(DATABASE_FILE);
    let database_metadata = match std::fs::symlink_metadata(&database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspect mailbox provider registry {}",
                    database_path.display()
                )
            });
        }
    };
    ensure!(
        database_metadata.is_file() && !database_metadata.file_type().is_symlink(),
        "mailbox provider registry must be a real file, not a symlink"
    );
    Ok(Some(
        ReadOnlyDatabase::open(&database_path)
            .context("open mailbox provider registry read-only")?,
    ))
}

fn validate_replica_set_lookup_keys(store_keys: &[MailboxStoreKey]) -> Result<()> {
    ensure!(
        !store_keys.is_empty() && store_keys.len() <= usize::from(MAX_PROVIDER_SELECTION),
        "mailbox replica-set lookup size is invalid"
    );
    ensure!(
        store_keys.windows(2).all(|pair| pair[0] < pair[1]),
        "mailbox replica-set lookup keys are not canonical and distinct"
    );
    Ok(())
}

fn active_offers_for_store_keys_from_database(
    database: &impl ReadableDatabase,
    store_keys: &[MailboxStoreKey],
    now_unix_seconds: u64,
) -> Result<Vec<MailboxProviderOffer>> {
    let read = database
        .begin_read()
        .context("begin exact mailbox provider lookup")?;
    let table = read.open_table(OFFER_TABLE)?;
    let gossip_hops = read.open_table(GOSSIP_HOP_TABLE)?;
    let mut observation_counts = BTreeMap::<[u8; 32], u8>::new();
    match read.open_table(AUTHENTICATED_OBSERVATION_TABLE) {
        Ok(observations) => {
            for entry in observations.iter()? {
                let (key, value) = entry?;
                let observation =
                    AuthenticatedProviderObservationRecord::decode(key.value(), value.value())?;
                let count = observation_counts.entry(observation.offer_id).or_default();
                *count = count
                    .checked_add(1)
                    .context("mailbox provider authenticated observation count overflow")?;
                ensure!(
                    *count <= MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER,
                    "mailbox provider authenticated observation capacity exceeded"
                );
            }
        }
        Err(redb::TableError::TableDoesNotExist(_)) => {}
        Err(error) => return Err(error.into()),
    }
    let mut offers = Vec::new();
    for store_key in store_keys {
        let Some(value) = table.get(store_key.as_bytes().as_slice())? else {
            continue;
        };
        let record = ProviderOfferRecord::decode(value.value())?;
        ensure!(
            record.store_key == *store_key,
            "mailbox provider registry key does not match its signed offer"
        );
        if record.expires_at_unix_seconds <= now_unix_seconds {
            continue;
        }
        let observed_gossip_hops = gossip_hops
            .get(store_key.as_bytes().as_slice())?
            .map(|value| decode_gossip_hops(value.value()))
            .transpose()?
            .unwrap_or(0);
        let authenticated_observation_count = observation_counts
            .get(offer_id(&record.encoded_offer).as_bytes())
            .copied()
            .unwrap_or(0);
        offers.push(record.into_public(
            now_unix_seconds,
            observed_gossip_hops,
            authenticated_observation_count,
        )?);
    }
    Ok(offers)
}

fn active_offers_from_database(
    database: &impl ReadableDatabase,
    now_unix_seconds: u64,
) -> Result<Vec<MailboxProviderOffer>> {
    let read = database
        .begin_read()
        .context("begin mailbox provider registry read")?;
    let table = read.open_table(OFFER_TABLE)?;
    let gossip_hops = read.open_table(GOSSIP_HOP_TABLE)?;
    let mut observation_counts = BTreeMap::<[u8; 32], u8>::new();
    match read.open_table(AUTHENTICATED_OBSERVATION_TABLE) {
        Ok(observations) => {
            for entry in observations.iter()? {
                let (key, value) = entry?;
                let observation =
                    AuthenticatedProviderObservationRecord::decode(key.value(), value.value())?;
                let count = observation_counts.entry(observation.offer_id).or_default();
                *count = count
                    .checked_add(1)
                    .context("mailbox provider authenticated observation count overflow")?;
                ensure!(
                    *count <= MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER,
                    "mailbox provider authenticated observation capacity exceeded"
                );
            }
        }
        Err(redb::TableError::TableDoesNotExist(_)) => {}
        Err(error) => return Err(error.into()),
    }
    let mut offers = Vec::new();
    for entry in table.iter()? {
        let (key, value) = entry?;
        let record = ProviderOfferRecord::decode(value.value())?;
        ensure!(
            key.value() == record.store_key.as_bytes(),
            "mailbox provider registry key does not match its signed offer"
        );
        if record.expires_at_unix_seconds > now_unix_seconds {
            let observed_gossip_hops = gossip_hops
                .get(key.value())?
                .map(|value| decode_gossip_hops(value.value()))
                .transpose()?
                .unwrap_or(0);
            let authenticated_observation_count = observation_counts
                .get(offer_id(&record.encoded_offer).as_bytes())
                .copied()
                .unwrap_or(0);
            offers.push(record.into_public(
                now_unix_seconds,
                observed_gossip_hops,
                authenticated_observation_count,
            )?);
        }
    }
    offers.sort_by_key(|offer| (offer.store_key(), offer.offer_id()));
    Ok(offers)
}

fn select_active_offers(
    active_offers: Vec<MailboxProviderOffer>,
    selection_salt: [u8; 32],
    requested: u8,
) -> Result<Vec<MailboxProviderOffer>> {
    ensure!(
        (1..=MAX_PROVIDER_SELECTION).contains(&requested),
        "mailbox provider selection size is invalid"
    );
    let mut ranked = active_offers
        .into_iter()
        .map(|offer| (provider_selection_rank(&offer, selection_salt), offer))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.store_key().cmp(&right.1.store_key()))
    });

    let mut selected_identities = BTreeSet::new();
    let mut selected = Vec::new();
    for (_, offer) in ranked {
        if selected_identities.insert(*offer.transport_identity()) {
            selected.push(offer);
            if selected.len() == usize::from(requested) {
                break;
            }
        }
    }
    Ok(selected)
}

fn select_bootstrap_safe_active_offers(
    active_offers: Vec<MailboxProviderOffer>,
    selection_salt: [u8; 32],
    requested: u8,
    minimum_work_bits: u8,
) -> Result<Vec<MailboxProviderOffer>> {
    ensure!(
        (1..=MAX_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS).contains(&minimum_work_bits),
        "mailbox provider admission work is outside protocol bounds"
    );
    ensure!(
        (1..=MAX_PROVIDER_SELECTION).contains(&requested),
        "mailbox provider selection size is invalid"
    );
    let mut ranked = active_offers
        .into_iter()
        .filter(|offer| offer.admission_work_bits() >= u16::from(minimum_work_bits))
        .map(|offer| {
            let bootstrap_fallback = offer.authenticated_observation_count()
                < MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE;
            (
                bootstrap_fallback,
                provider_selection_rank(&offer, selection_salt),
                offer,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.store_key().cmp(&right.2.store_key()))
    });

    let mut selected_identities = BTreeSet::new();
    let mut selected = Vec::new();
    for (_, _, offer) in ranked {
        if selected_identities.insert(*offer.transport_identity()) {
            selected.push(offer);
            if selected.len() == usize::from(requested) {
                break;
            }
        }
    }
    Ok(selected)
}

fn provider_selection_rank(offer: &MailboxProviderOffer, selection_salt: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SELECTION_DOMAIN);
    hasher.update(&selection_salt);
    hasher.update(offer.store_key().as_bytes());
    hasher.update(offer.transport_identity());
    hasher.update(offer.offer_id().as_bytes());
    *hasher.finalize().as_bytes()
}

fn decode_gossip_hops(bytes: &[u8]) -> Result<u8> {
    ensure!(
        bytes.len() == 1,
        "stored mailbox provider gossip hop count is invalid"
    );
    ensure!(
        bytes[0] <= MAX_PROVIDER_GOSSIP_HOPS,
        "stored mailbox provider gossip hop count is invalid"
    );
    Ok(bytes[0])
}

fn offer_id(encoded_offer: &[u8]) -> MailboxProviderOfferId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(OFFER_ID_DOMAIN);
    hasher.update(encoded_offer);
    MailboxProviderOfferId(*hasher.finalize().as_bytes())
}

fn authenticated_observation_key(
    offer_id: [u8; 32],
    observer_tag: [u8; 32],
) -> [u8; AUTHENTICATED_OBSERVATION_KEY_BYTES] {
    let mut key = [0_u8; AUTHENTICATED_OBSERVATION_KEY_BYTES];
    key[..32].copy_from_slice(&offer_id);
    key[32..].copy_from_slice(&observer_tag);
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_mailbox::MailboxStoreIdentity;

    fn offer(store_secret: u8, endpoint: u8, issued_at: u64) -> Result<(Vec<u8>, [u8; 32])> {
        let identity = MailboxStoreIdentity::from_secret_bytes([store_secret; 32]);
        let offer = identity.storage_offer_with_admission_work(
            vec![endpoint; 64],
            200 * 1024 * 1024,
            1024 * 1024,
            issued_at,
            300,
            DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
        )?;
        Ok((offer.encode(issued_at)?, [endpoint; 32]))
    }

    fn unqualified_offer(
        store_secret: u8,
        endpoint: u8,
        issued_at: u64,
    ) -> Result<(Vec<u8>, [u8; 32])> {
        let identity = MailboxStoreIdentity::from_secret_bytes([store_secret; 32]);
        for _ in 0..32 {
            let offer = identity.storage_offer(
                vec![endpoint; 64],
                200 * 1024 * 1024,
                1024 * 1024,
                issued_at,
                300,
            )?;
            if offer.admission_work_bits() < u16::from(DEFAULT_PROVIDER_ADMISSION_WORK_BITS) {
                return Ok((offer.encode(issued_at)?, [endpoint; 32]));
            }
        }
        anyhow::bail!("failed to construct an admission-unqualified test offer")
    }

    fn observer(value: u8) -> MailboxProviderLocalObserverTag {
        MailboxProviderLocalObserverTag::from_bytes([value; 32])
    }

    #[test]
    fn read_only_discovery_does_not_create_or_rewrite_registry() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("providers");
        let config = MailboxProviderRegistryConfig::new(data_dir.clone());
        assert!(MailboxProviderRegistry::active_offers_read_only(config.clone(), 1)?.is_empty());
        assert!(!data_dir.exists());

        let registry = MailboxProviderRegistry::open(config.clone())?;
        let (encoded, endpoint) = offer(1, 11, 1_000)?;
        registry.import_offer(&encoded, endpoint, 1_000)?;
        drop(registry);
        let database_path = data_dir.join(DATABASE_FILE);
        let before = blake3::hash(&std::fs::read(&database_path)?);
        let active = MailboxProviderRegistry::active_offers_read_only(config, 1_001)?;
        assert_eq!(active.len(), 1);
        let exact = MailboxProviderRegistry::active_offers_for_store_keys_read_only(
            MailboxProviderRegistryConfig::new(data_dir.clone()),
            &[active[0].store_key()],
            1_001,
        )?;
        assert_eq!(exact.len(), 1);
        assert_eq!(blake3::hash(&std::fs::read(database_path)?), before);

        let legacy_data_dir = directory.path().join("legacy-providers");
        std::fs::create_dir_all(&legacy_data_dir)?;
        let legacy_database = Database::create(legacy_data_dir.join(DATABASE_FILE))?;
        let signed = SignedMailboxStorageOffer::decode_and_verify(&encoded, 1_000)?;
        let record = ProviderOfferRecord::from_verified_offer(&signed, encoded, endpoint, 1_000);
        let write = legacy_database.begin_write()?;
        {
            let mut table = write.open_table(OFFER_TABLE)?;
            table.insert(
                record.store_key.as_bytes().as_slice(),
                record.encode()?.as_slice(),
            )?;
        }
        {
            let mut table = write.open_table(GOSSIP_HOP_TABLE)?;
            table.insert(record.store_key.as_bytes().as_slice(), [0_u8].as_slice())?;
        }
        write.commit()?;
        drop(legacy_database);
        let legacy = MailboxProviderRegistry::active_offers_read_only(
            MailboxProviderRegistryConfig::new(legacy_data_dir),
            1_001,
        )?;
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].authenticated_observation_count(), 0);
        Ok(())
    }

    #[test]
    fn registry_is_bounded_monotonic_and_prunes_expired_offers() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = MailboxProviderRegistryConfig::new(directory.path().join("providers"));
        config.max_offers = 2;
        let registry = MailboxProviderRegistry::open(config)?;
        let (first, first_endpoint) = offer(1, 11, 1_000)?;
        let (second, second_endpoint) = offer(2, 22, 1_000)?;
        let (third, third_endpoint) = offer(3, 33, 1_000)?;

        assert_eq!(
            registry.import_offer(&first, first_endpoint, 1_000)?.0,
            MailboxProviderImportOutcome::Inserted
        );
        assert_eq!(
            registry.import_offer(&first, first_endpoint, 1_001)?.0,
            MailboxProviderImportOutcome::AlreadyPresent
        );
        assert_eq!(
            registry.import_offer(&second, second_endpoint, 1_000)?.0,
            MailboxProviderImportOutcome::Inserted
        );
        assert!(
            registry
                .import_offer(&third, third_endpoint, 1_000)
                .is_err()
        );

        let (older_first, _) = offer(1, 44, 900)?;
        assert!(
            registry
                .import_offer(&older_first, [44; 32], 1_000)
                .is_err()
        );
        let (newer_first, newer_endpoint) = offer(1, 44, 1_100)?;
        assert_eq!(
            registry
                .import_offer(&newer_first, newer_endpoint, 1_100)?
                .0,
            MailboxProviderImportOutcome::Replaced
        );
        assert_eq!(registry.count()?, 2);

        // Both previous offers expire at 1_400 or earlier. Importing at that
        // boundary prunes them before applying the capacity check.
        let (fresh_third, fresh_endpoint) = offer(3, 33, 1_400)?;
        assert_eq!(
            registry
                .import_offer(&fresh_third, fresh_endpoint, 1_400)?
                .0,
            MailboxProviderImportOutcome::Inserted
        );
        assert_eq!(registry.count()?, 1);
        Ok(())
    }

    #[test]
    fn deterministic_selection_deduplicates_transport_identities() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        for (store, endpoint) in [(1, 11), (2, 22), (3, 11), (4, 44)] {
            let (encoded, _) = offer(store, endpoint, 1_000)?;
            registry.import_offer(&encoded, [endpoint; 32], 1_000)?;
        }
        let first = registry.select([91; 32], 3, 1_001)?;
        let second = registry.select([91; 32], 3, 1_001)?;
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert_eq!(
            first
                .iter()
                .map(|offer| *offer.transport_identity())
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        assert!(registry.select([1; 32], 0, 1_001).is_err());
        assert!(
            registry
                .select([1; 32], MAX_PROVIDER_SELECTION + 1, 1_001)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn admission_qualified_selection_and_gossip_exclude_cheap_identities() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        let (cheap, cheap_endpoint) = unqualified_offer(9, 99, 1_000)?;
        registry.import_offer(&cheap, cheap_endpoint, 1_000)?;
        for (store, endpoint) in [(1, 11), (2, 22)] {
            let (encoded, transport) = offer(store, endpoint, 1_000)?;
            registry.import_offer(&encoded, transport, 1_000)?;
        }

        assert_eq!(registry.active_offers(1_001)?.len(), 3);
        let selected = registry.select_bootstrap_safe([73; 32], 3, 1_001)?;
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|offer| {
            offer.admission_work_bits() >= u16::from(DEFAULT_PROVIDER_ADMISSION_WORK_BITS)
        }));
        let gossip = MailboxProviderGossipFrame::from_registry(&registry, [74; 32], None, 1_001)?;
        assert_eq!(gossip.entries().len(), 2);
        assert!(
            gossip
                .entries()
                .iter()
                .all(|entry| entry.encoded_offer() != cheap)
        );
        Ok(())
    }

    #[test]
    fn authenticated_observations_are_local_bounded_deduplicated_and_offer_bound() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("providers");
        let registry =
            MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(data_dir.clone()))?;
        let (encoded, endpoint) = offer(1, 11, 1_000)?;
        let (import, observation, observed) =
            registry.import_gossiped_offer(&encoded, endpoint, 1, observer(1), 1_001)?;
        assert_eq!(import, MailboxProviderImportOutcome::Inserted);
        assert_eq!(observation, MailboxProviderObservationOutcome::Added);
        assert_eq!(observed.authenticated_observation_count(), 1);

        let (_, observation, observed) =
            registry.import_gossiped_offer(&encoded, endpoint, 1, observer(1), 1_002)?;
        assert_eq!(observation, MailboxProviderObservationOutcome::Refreshed);
        assert_eq!(observed.authenticated_observation_count(), 1);
        let (_, observation, observed) =
            registry.import_gossiped_offer(&encoded, endpoint, 1, observer(2), 1_003)?;
        assert_eq!(observation, MailboxProviderObservationOutcome::Added);
        assert_eq!(observed.authenticated_observation_count(), 2);

        for value in 3..=MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER {
            let (_, observation, _) = registry.import_gossiped_offer(
                &encoded,
                endpoint,
                1,
                observer(value),
                1_003 + u64::from(value),
            )?;
            assert_eq!(observation, MailboxProviderObservationOutcome::Added);
        }
        let (_, observation, observed) =
            registry.import_gossiped_offer(&encoded, endpoint, 1, observer(99), 1_020)?;
        assert_eq!(
            observation,
            MailboxProviderObservationOutcome::CapacityReached
        );
        assert_eq!(
            observed.authenticated_observation_count(),
            MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER
        );

        let control_directory = tempfile::tempdir()?;
        let control = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            control_directory.path().join("providers"),
        ))?;
        control.import_gossiped_offer(&encoded, endpoint, 1, observer(77), 1_001)?;
        let observed_frame =
            MailboxProviderGossipFrame::from_registry(&registry, [81; 32], None, 1_021)?;
        let control_frame =
            MailboxProviderGossipFrame::from_registry(&control, [81; 32], None, 1_021)?;
        assert_eq!(observed_frame.encode(1_021)?, control_frame.encode(1_021)?);

        let (replacement, replacement_endpoint) = offer(1, 12, 1_100)?;
        let (import, observation, replacement) = registry.import_gossiped_offer(
            &replacement,
            replacement_endpoint,
            1,
            observer(9),
            1_100,
        )?;
        assert_eq!(import, MailboxProviderImportOutcome::Replaced);
        assert_eq!(observation, MailboxProviderObservationOutcome::Added);
        assert_eq!(replacement.authenticated_observation_count(), 1);
        drop(registry);

        let active = MailboxProviderRegistry::active_offers_read_only(
            MailboxProviderRegistryConfig::new(data_dir),
            1_101,
        )?;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].authenticated_observation_count(), 1);
        Ok(())
    }

    #[test]
    fn bootstrap_safe_selection_prefers_binary_corroboration_and_fills_fallback() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        let mut observed_store_keys = BTreeSet::new();
        let mut first_observed = None;
        for (store, endpoint) in [(1, 11), (2, 22), (3, 33), (4, 44)] {
            let (encoded, transport) = offer(store, endpoint, 1_000)?;
            if store <= 2 {
                registry.import_gossiped_offer(&encoded, transport, 1, observer(store), 1_001)?;
                observed_store_keys.insert(
                    SignedMailboxStorageOffer::decode_and_verify(&encoded, 1_001)?.store_key(),
                );
                if store == 1 {
                    first_observed = Some((encoded, transport));
                }
            } else {
                registry.import_offer(&encoded, transport, 1_001)?;
            }
        }

        let salt = [85_u8; 32];
        let preferred = registry.select_bootstrap_safe(salt, 2, 1_002)?;
        assert_eq!(preferred.len(), 2);
        assert!(
            preferred
                .iter()
                .all(|offer| observed_store_keys.contains(&offer.store_key()))
        );

        let with_fallback = registry.select_bootstrap_safe(salt, 4, 1_002)?;
        assert_eq!(with_fallback.len(), 4);
        assert!(with_fallback[..2].iter().all(|offer| {
            offer.authenticated_observation_count()
                >= MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE
        }));
        assert!(
            with_fallback[2..]
                .iter()
                .all(|offer| offer.authenticated_observation_count() == 0)
        );

        let (encoded, transport) = first_observed.context("missing observed test offer")?;
        registry.import_gossiped_offer(&encoded, transport, 1, observer(99), 1_003)?;
        let after_extra_observer = registry.select_bootstrap_safe(salt, 4, 1_003)?;
        assert_eq!(
            with_fallback
                .iter()
                .map(MailboxProviderOffer::offer_id)
                .collect::<Vec<_>>(),
            after_extra_observer
                .iter()
                .map(MailboxProviderOffer::offer_id)
                .collect::<Vec<_>>()
        );

        let (cheap, cheap_transport) = unqualified_offer(9, 99, 1_000)?;
        registry.import_gossiped_offer(&cheap, cheap_transport, 1, observer(9), 1_003)?;
        let selected = registry.select_bootstrap_safe(salt, 8, 1_003)?;
        assert_eq!(selected.len(), 4);
        assert!(selected.iter().all(|offer| {
            offer.admission_work_bits() >= u16::from(DEFAULT_PROVIDER_ADMISSION_WORK_BITS)
        }));
        Ok(())
    }

    #[test]
    fn exact_replica_set_lookup_is_indexed_bounded_and_expiry_aware() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        let mut committed = Vec::new();
        for (store, endpoint) in [(1, 11), (2, 22), (3, 33)] {
            let (encoded, _) = offer(store, endpoint, 1_000)?;
            let (_, imported) = registry.import_offer(&encoded, [endpoint; 32], 1_000)?;
            if store != 2 {
                committed.push(imported.store_key());
            }
        }
        committed.sort_unstable();
        let resolved = registry.active_offers_for_store_keys(&committed, 1_001)?;
        assert_eq!(
            resolved
                .iter()
                .map(MailboxProviderOffer::store_key)
                .collect::<Vec<_>>(),
            committed
        );
        assert!(
            registry
                .active_offers_for_store_keys(&committed, 1_300)?
                .is_empty()
        );
        let mut reversed = committed.clone();
        reversed.reverse();
        assert!(
            registry
                .active_offers_for_store_keys(&reversed, 1_001)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn import_rejects_tamper_expiry_and_noncanonical_identity_replay() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        let (encoded, endpoint) = offer(9, 99, 1_000)?;
        let mut tampered = encoded.clone();
        tampered[10] ^= 1;
        assert!(registry.import_offer(&tampered, endpoint, 1_000).is_err());
        assert!(registry.import_offer(&encoded, endpoint, 1_300).is_err());
        registry.import_offer(&encoded, endpoint, 1_001)?;
        // The same signed offer cannot be rebound locally to another parsed
        // transport identity without advancing the provider signature.
        assert!(registry.import_offer(&encoded, [7; 32], 1_001).is_err());
        Ok(())
    }

    #[test]
    fn gossip_frame_is_short_lived_bounded_and_reply_bound() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let registry = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            directory.path().join("providers"),
        ))?;
        for value in 1..=12 {
            let (encoded, _) = offer(value, value, 1_000)?;
            registry.import_offer(&encoded, [value; 32], 1_000)?;
        }
        let request = MailboxProviderGossipFrame::from_registry(&registry, [31; 32], None, 1_001)?;
        assert_eq!(
            request.entries().len(),
            usize::from(MAX_PROVIDER_GOSSIP_OFFERS)
        );
        assert!(
            request
                .entries()
                .iter()
                .all(|entry| entry.transmitted_hops() == 1)
        );
        let encoded = request.encode(1_001)?;
        assert!(encoded.len() <= MAX_PROVIDER_GOSSIP_FRAME_BYTES);
        assert_eq!(
            MailboxProviderGossipFrame::decode_and_verify(&encoded, 1_001)?,
            request
        );
        assert!(MailboxProviderGossipFrame::decode_and_verify(&encoded, 1_061).is_err());

        let reply = MailboxProviderGossipFrame::from_registry(
            &registry,
            [41; 32],
            Some(request.frame_id()?),
            1_001,
        )?;
        assert_eq!(reply.reply_to(), Some(request.frame_id()?));
        assert_ne!(reply.frame_id()?, request.frame_id()?);
        Ok(())
    }

    #[test]
    fn gossip_hops_and_age_stop_amplification() -> Result<()> {
        let first_directory = tempfile::tempdir()?;
        let first = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            first_directory.path().join("providers"),
        ))?;
        let (encoded, endpoint) = offer(7, 77, 1_000)?;
        let (_, observation, once_relayed) =
            first.import_gossiped_offer(&encoded, endpoint, 1, observer(1), 1_001)?;
        assert_eq!(observation, MailboxProviderObservationOutcome::Added);
        assert_eq!(once_relayed.authenticated_observation_count(), 1);
        assert_eq!(once_relayed.observed_gossip_hops(), 1);
        let frame = MailboxProviderGossipFrame::from_registry(&first, [5; 32], None, 1_001)?;
        assert_eq!(frame.entries().len(), 1);
        assert_eq!(frame.entries()[0].transmitted_hops(), 2);

        let second_directory = tempfile::tempdir()?;
        let second = MailboxProviderRegistry::open(MailboxProviderRegistryConfig::new(
            second_directory.path().join("providers"),
        ))?;
        second.import_gossiped_offer(
            frame.entries()[0].encoded_offer(),
            endpoint,
            frame.entries()[0].transmitted_hops(),
            observer(2),
            1_001,
        )?;
        assert!(
            MailboxProviderGossipFrame::from_registry(&second, [6; 32], None, 1_001)?
                .entries()
                .is_empty()
        );

        // Another gossip frame cannot reset already retained provenance by
        // merely claiming a shorter path for the exact same signed offer.
        let (_, observation, replayed) =
            second.import_gossiped_offer(&encoded, endpoint, 1, observer(2), 1_002)?;
        assert_eq!(observation, MailboxProviderObservationOutcome::Refreshed);
        assert_eq!(replayed.authenticated_observation_count(), 1);
        assert_eq!(replayed.observed_gossip_hops(), 2);

        // A later direct observation lowers the retained hop count and makes
        // the exact signed offer eligible for one bounded gossip edge again.
        let (_, direct) = second.import_offer(&encoded, endpoint, 1_002)?;
        assert_eq!(direct.observed_gossip_hops(), 0);
        assert_eq!(
            MailboxProviderGossipFrame::from_registry(&second, [7; 32], None, 1_002)?
                .entries()
                .len(),
            1
        );

        let stale_identity = MailboxStoreIdentity::from_secret_bytes([8; 32]);
        let stale = stale_identity.storage_offer_with_admission_work(
            vec![88; 64],
            200 * 1024 * 1024,
            1024 * 1024,
            1_000,
            3_600,
            DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
        )?;
        let stale = stale.encode(1_000)?;
        second.import_offer(&stale, [88; 32], 1_000)?;
        let later = 1_000 + MAX_PROVIDER_GOSSIP_AGE_SECONDS + 1;
        let later_frame = MailboxProviderGossipFrame::from_registry(&second, [8; 32], None, later)?;
        assert!(later_frame.entries().is_empty());

        let oversized_directory = tempfile::tempdir()?;
        let oversized_registry = MailboxProviderRegistry::open(
            MailboxProviderRegistryConfig::new(oversized_directory.path().join("providers")),
        )?;
        let oversized_identity = MailboxStoreIdentity::from_secret_bytes([9; 32]);
        let oversized = oversized_identity.storage_offer_with_admission_work(
            vec![99; MAX_PROVIDER_GOSSIP_OFFER_BYTES + 1],
            200 * 1024 * 1024,
            1024 * 1024,
            1_000,
            3_600,
            DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
        )?;
        oversized_registry.import_offer(&oversized.encode(1_000)?, [99; 32], 1_000)?;
        assert!(
            MailboxProviderGossipFrame::from_registry(&oversized_registry, [9; 32], None, 1_001,)?
                .entries()
                .is_empty()
        );
        Ok(())
    }
}
