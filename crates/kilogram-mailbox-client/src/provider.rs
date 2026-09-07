use std::{collections::BTreeSet, fmt, path::PathBuf};

use anyhow::{Context, Result, ensure};
use kilogram_mailbox::{
    MAX_MAILBOX_STORAGE_OFFER_BYTES, MailboxStoragePolicyClass, MailboxStoreKey,
    SignedMailboxStorageOffer,
};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};

const DATABASE_FILE: &str = "mailbox-provider-registry.redb";
const OFFER_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-provider-offers-v1");
const RECORD_VERSION: u8 = 1;
const OFFER_ID_DOMAIN: &[u8] = b"kilogram:mailbox-provider-offer-id:v1\0";
const SELECTION_DOMAIN: &[u8] = b"kilogram:mailbox-provider-selection:v1\0";
const MAX_PROVIDER_RECORD_BYTES: usize = MAX_MAILBOX_STORAGE_OFFER_BYTES + 256;
const MAX_ABSOLUTE_PROVIDER_OFFERS: u64 = 4_096;

/// A conservative local default. The registry is discovery state, not an
/// unbounded cache of every volunteer ever observed.
pub const DEFAULT_MAX_PROVIDER_OFFERS: u64 = 256;

/// Replication fan-out remains deliberately small even if the registry is
/// full. A later policy may choose fewer providers for a particular item.
pub const MAX_PROVIDER_SELECTION: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MailboxProviderOfferId([u8; 32]);

impl MailboxProviderOfferId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
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

    fn into_public(self, now_unix_seconds: u64) -> Result<MailboxProviderOffer> {
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
        })
    }
}

pub struct MailboxProviderRegistry {
    database: Database,
    config: MailboxProviderRegistryConfig,
}

impl MailboxProviderRegistry {
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

        let expired_keys = {
            let table = write.open_table(OFFER_TABLE)?;
            let mut expired_keys = Vec::new();
            for entry in table.iter()? {
                let (candidate_key, candidate_value) = entry?;
                let candidate = ProviderOfferRecord::decode(candidate_value.value())?;
                ensure!(
                    candidate_key.value() == candidate.store_key.as_bytes(),
                    "mailbox provider registry key does not match its signed offer"
                );
                if candidate.expires_at_unix_seconds <= now_unix_seconds {
                    expired_keys.push(candidate_key.value().to_vec());
                }
            }
            expired_keys
        };
        {
            let mut table = write.open_table(OFFER_TABLE)?;
            for expired_key in &expired_keys {
                table.remove(expired_key.as_slice())?;
            }
        }

        let current = write
            .open_table(OFFER_TABLE)?
            .get(key.as_slice())?
            .map(|value| value.value().to_vec());
        let (outcome, effective_record) = if let Some(current) = current {
            let current = ProviderOfferRecord::decode(&current)?;
            if current.encoded_offer == record.encoded_offer
                && current.transport_identity == record.transport_identity
            {
                (MailboxProviderImportOutcome::AlreadyPresent, current)
            } else {
                ensure!(
                    record.issued_at_unix_seconds > current.issued_at_unix_seconds,
                    "mailbox provider offer does not advance the current signed offer"
                );
                write
                    .open_table(OFFER_TABLE)?
                    .insert(key.as_slice(), encoded_record.as_slice())?;
                (MailboxProviderImportOutcome::Replaced, record.clone())
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
            (MailboxProviderImportOutcome::Inserted, record.clone())
        };
        write
            .commit()
            .context("commit mailbox provider offer import")?;
        Ok((outcome, effective_record.into_public(now_unix_seconds)?))
    }

    pub fn active_offers(&self, now_unix_seconds: u64) -> Result<Vec<MailboxProviderOffer>> {
        let read = self
            .database
            .begin_read()
            .context("begin mailbox provider registry read")?;
        let table = read.open_table(OFFER_TABLE)?;
        let mut offers = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let record = ProviderOfferRecord::decode(value.value())?;
            ensure!(
                key.value() == record.store_key.as_bytes(),
                "mailbox provider registry key does not match its signed offer"
            );
            if record.expires_at_unix_seconds > now_unix_seconds {
                offers.push(record.into_public(now_unix_seconds)?);
            }
        }
        offers.sort_by_key(|offer| (offer.store_key(), offer.offer_id()));
        Ok(offers)
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
        let mut ranked = self
            .active_offers(now_unix_seconds)?
            .into_iter()
            .map(|offer| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(SELECTION_DOMAIN);
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

fn offer_id(encoded_offer: &[u8]) -> MailboxProviderOfferId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(OFFER_ID_DOMAIN);
    hasher.update(encoded_offer);
    MailboxProviderOfferId(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_mailbox::MailboxStoreIdentity;

    fn offer(store_secret: u8, endpoint: u8, issued_at: u64) -> Result<(Vec<u8>, [u8; 32])> {
        let identity = MailboxStoreIdentity::from_secret_bytes([store_secret; 32]);
        let offer = identity.storage_offer(
            vec![endpoint; 64],
            200 * 1024 * 1024,
            1024 * 1024,
            issued_at,
            300,
        )?;
        Ok((offer.encode(issued_at)?, [endpoint; 32]))
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
}
