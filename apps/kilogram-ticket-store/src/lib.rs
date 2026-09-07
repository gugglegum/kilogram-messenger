use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    future::Future,
    hash::Hash,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use kilogram_mailbox::{
    BlindMailboxStore, DEFAULT_MAX_ITEMS_PER_MAILBOX, MAX_MAILBOX_ENVELOPE_BYTES,
    MailboxDeleteRequest, MailboxDeleteResponse, MailboxId, MailboxItemId, MailboxListRequest,
    MailboxListResponse, MailboxPeerOperation, MailboxPeerRejection, MailboxPeerRequest,
    MailboxPeerResponse, MailboxPutRequest, MailboxPutResponse, MailboxStoreConfig,
    MailboxStoreIdentity, MailboxStoreKey, SignedMailboxStorageOffer,
};
use kilogram_ticket_publication::{
    TicketPublicationChannelId, TicketPublicationWriteKey, WRITE_KEY_HEADER,
    WRITE_SIGNATURE_HEADER, decode_signature,
};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinSet,
    time::{MissedTickBehavior, timeout},
};
use zeroize::Zeroizing;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const DATABASE_FILE: &str = "ticket-publications.redb";
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("opaque-ticket-publications-v1");
const META_TABLE: TableDefinition<&str, u64> =
    TableDefinition::new("opaque-ticket-publication-meta-v1");
const TOTAL_BODY_BYTES_KEY: &str = "total-body-bytes";
const TRANSFER_WINDOW_SECONDS: u64 = 30 * 24 * 60 * 60;
const RECORD_VERSION: u8 = 1;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_HEADER_COUNT: usize = 64;
const MAX_REQUEST_TARGET_BYTES: usize = 512;
const MAX_ABSOLUTE_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MIN_RETENTION_SECONDS: u64 = 30;
const MAX_RETENTION_SECONDS: u64 = 60 * 60;
const MAX_RATE_LIMIT: u64 = 10_000_000;
const MAX_CHANNEL_LIMIT: u64 = 10_000_000;
const MAX_TOTAL_BYTES_LIMIT: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_TRANSFER_BYTES_LIMIT: u64 = 16 * 1024 * 1024 * 1024 * 1024;
const MAX_CONNECTION_LIMIT: usize = 4_096;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(20);
const RATE_WINDOW: Duration = Duration::from_secs(60);
const PUBLICATION_PATH_PREFIX: &str = "/v1/ticket-publications/";
const PUBLICATION_CONTENT_TYPE: &str = "application/vnd.kilogram.ticket-publication";
const MAILBOX_PATH_PREFIX: &str = "/v1/mailboxes/";
const MAILBOX_CONTENT_TYPE: &str = "application/vnd.kilogram.blind-mailbox-v1";
const MAILBOX_DATA_DIRECTORY: &str = "blind-mailbox";
const MAILBOX_IDENTITY_FILE: &str = "blind-mailbox-store-secret.key";

pub const DEFAULT_RETENTION_SECONDS: u64 = 15 * 60;
pub const DEFAULT_MAX_RECORD_BYTES: usize = MAX_ABSOLUTE_RECORD_BYTES;
pub const DEFAULT_MAX_CHANNELS: u64 = 100_000;
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_PER_IP_REQUESTS_PER_MINUTE: u64 = 120;
pub const DEFAULT_GLOBAL_REQUESTS_PER_MINUTE: u64 = 10_000;
pub const DEFAULT_MAX_CONCURRENT_CONNECTIONS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreConfig {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub retention_seconds: u64,
    pub max_record_bytes: usize,
    pub max_channels: u64,
    pub max_total_bytes: u64,
    pub per_ip_requests_per_minute: u64,
    pub global_requests_per_minute: u64,
    pub max_concurrent_connections: usize,
    pub trust_x_real_ip: bool,
    pub service_mode: StoreServiceMode,
    pub transfer_accounting_scope: Option<String>,
    pub max_transfer_bytes_per_30_days: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreServiceMode {
    Combined,
    MailboxOnly,
}

impl StoreConfig {
    pub fn local_test(data_dir: PathBuf) -> Self {
        Self {
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            data_dir,
            retention_seconds: DEFAULT_RETENTION_SECONDS,
            max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
            max_channels: DEFAULT_MAX_CHANNELS,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            per_ip_requests_per_minute: DEFAULT_PER_IP_REQUESTS_PER_MINUTE,
            global_requests_per_minute: DEFAULT_GLOBAL_REQUESTS_PER_MINUTE,
            max_concurrent_connections: DEFAULT_MAX_CONCURRENT_CONNECTIONS,
            trust_x_real_ip: false,
            service_mode: StoreServiceMode::Combined,
            transfer_accounting_scope: None,
            max_transfer_bytes_per_30_days: None,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.listen.ip().is_loopback(),
            "opaque ticket store must listen on loopback behind an HTTPS reverse proxy"
        );
        ensure!(
            (MIN_RETENTION_SECONDS..=MAX_RETENTION_SECONDS).contains(&self.retention_seconds),
            "ticket-store retention must be between {MIN_RETENTION_SECONDS} and {MAX_RETENTION_SECONDS} seconds"
        );
        ensure!(
            (1..=MAX_ABSOLUTE_RECORD_BYTES).contains(&self.max_record_bytes),
            "ticket-store record size limit is outside protocol bounds"
        );
        ensure!(
            (1..=MAX_CHANNEL_LIMIT).contains(&self.max_channels),
            "ticket-store channel limit is outside service bounds"
        );
        ensure!(
            (self.max_record_bytes as u64..=MAX_TOTAL_BYTES_LIMIT).contains(&self.max_total_bytes),
            "ticket-store total byte limit is outside service bounds"
        );
        ensure!(
            (1..=MAX_RATE_LIMIT).contains(&self.per_ip_requests_per_minute)
                && (self.per_ip_requests_per_minute..=MAX_RATE_LIMIT)
                    .contains(&self.global_requests_per_minute),
            "ticket-store request limits are invalid"
        );
        ensure!(
            (1..=MAX_CONNECTION_LIMIT).contains(&self.max_concurrent_connections),
            "ticket-store connection limit is outside service bounds"
        );
        ensure!(
            !self.data_dir.as_os_str().is_empty(),
            "ticket-store data directory is empty"
        );
        ensure!(
            self.transfer_accounting_scope.is_some()
                == self.max_transfer_bytes_per_30_days.is_some(),
            "ticket-store transfer accounting scope and limit must be configured together"
        );
        if let (Some(scope), Some(limit)) = (
            self.transfer_accounting_scope.as_deref(),
            self.max_transfer_bytes_per_30_days,
        ) {
            ensure!(
                !scope.is_empty()
                    && scope.len() <= 32
                    && scope.bytes().all(|byte| byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'),
                "ticket-store transfer accounting scope is invalid"
            );
            ensure!(
                (self.max_record_bytes as u64..=MAX_TRANSFER_BYTES_LIMIT).contains(&limit),
                "ticket-store 30-day transfer limit is outside service bounds"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredOpaquePublication {
    version: u8,
    generation: u64,
    stored_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    body: Vec<u8>,
}

impl StoredOpaquePublication {
    fn new(generation: u64, body: Vec<u8>, now: u64, retention_seconds: u64) -> Result<Self> {
        ensure!(generation != 0, "publication generation must be non-zero");
        ensure!(
            !body.is_empty() && body.len() <= MAX_ABSOLUTE_RECORD_BYTES,
            "opaque publication body is outside protocol bounds"
        );
        let expires_at_unix_seconds = now
            .checked_add(retention_seconds)
            .context("ticket-store retention expiry overflow")?;
        Ok(Self {
            version: RECORD_VERSION,
            generation,
            stored_at_unix_seconds: now,
            expires_at_unix_seconds,
            body,
        })
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        postcard::to_allocvec(self).context("encode opaque ticket-store record")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_ABSOLUTE_RECORD_BYTES.saturating_add(128),
            "opaque ticket-store record is too large"
        );
        let record: Self =
            postcard::from_bytes(bytes).context("decode opaque ticket-store record")?;
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported opaque ticket-store record version"
        );
        ensure!(
            self.generation != 0
                && self.stored_at_unix_seconds < self.expires_at_unix_seconds
                && !self.body.is_empty()
                && self.body.len() <= MAX_ABSOLUTE_RECORD_BYTES,
            "opaque ticket-store record is invalid"
        );
        Ok(())
    }

    fn expired_at(&self, now: u64) -> bool {
        self.expires_at_unix_seconds <= now
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PutOutcome {
    Created,
    Replaced,
    AlreadyPresent,
    Conflict,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredValue {
    pub generation: u64,
    pub expires_at_unix_seconds: u64,
    pub body: Vec<u8>,
}

struct OpaqueStore {
    database: Database,
    retention_seconds: u64,
    max_record_bytes: usize,
    max_channels: u64,
    max_total_bytes: u64,
}

impl OpaqueStore {
    fn open(config: &StoreConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "create ticket-store data directory {}",
                config.data_dir.display()
            )
        })?;
        let metadata = std::fs::symlink_metadata(&config.data_dir).with_context(|| {
            format!(
                "inspect ticket-store data directory {}",
                config.data_dir.display()
            )
        })?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "ticket-store data directory must be a real directory, not a symlink"
        );
        let canonical_data_dir = std::fs::canonicalize(&config.data_dir).with_context(|| {
            format!(
                "canonicalize ticket-store data directory {}",
                config.data_dir.display()
            )
        })?;
        let database_path = canonical_data_dir.join(DATABASE_FILE);
        let database = Database::create(&database_path)
            .with_context(|| format!("open ticket-store database {}", database_path.display()))?;
        let write = database
            .begin_write()
            .context("begin ticket-store initialization transaction")?;
        {
            write
                .open_table(RECORD_TABLE)
                .context("open ticket-store record table")?;
        }
        {
            let mut meta = write
                .open_table(META_TABLE)
                .context("open ticket-store metadata table")?;
            if meta
                .get(TOTAL_BODY_BYTES_KEY)
                .context("read ticket-store byte accounting")?
                .is_none()
            {
                meta.insert(TOTAL_BODY_BYTES_KEY, 0)
                    .context("initialize ticket-store byte accounting")?;
            }
        }
        write
            .commit()
            .context("commit ticket-store initialization transaction")?;
        Ok(Self {
            database,
            retention_seconds: config.retention_seconds,
            max_record_bytes: config.max_record_bytes,
            max_channels: config.max_channels,
            max_total_bytes: config.max_total_bytes,
        })
    }

    fn put(
        &self,
        channel: [u8; 32],
        generation: u64,
        body: Vec<u8>,
        now: u64,
    ) -> Result<PutOutcome> {
        ensure!(
            !body.is_empty() && body.len() <= self.max_record_bytes,
            "opaque publication body is outside configured bounds"
        );
        let incoming = StoredOpaquePublication::new(generation, body, now, self.retention_seconds)?;
        let encoded = incoming.encode()?;
        let mut write = self
            .database
            .begin_write()
            .context("begin ticket-store PUT transaction")?;
        write
            .set_durability(Durability::Immediate)
            .context("set ticket-store PUT durability")?;
        let current_total_bytes = {
            let meta = write
                .open_table(META_TABLE)
                .context("open ticket-store metadata for PUT")?;
            meta.get(TOTAL_BODY_BYTES_KEY)
                .context("read ticket-store byte total for PUT")?
                .map_or(0, |value| value.value())
        };
        let mut next_total_bytes = current_total_bytes;
        let outcome;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .context("open ticket-store record table for PUT")?;
            let current = table
                .get(channel.as_slice())
                .context("read current ticket-store value")?
                .map(|value| value.value().to_vec());
            outcome = match current {
                Some(current) => {
                    let current = StoredOpaquePublication::decode(&current)?;
                    if incoming.generation < current.generation
                        || (incoming.generation == current.generation
                            && incoming.body != current.body)
                    {
                        PutOutcome::Conflict
                    } else if incoming.generation == current.generation {
                        PutOutcome::AlreadyPresent
                    } else {
                        let candidate_total = current_total_bytes
                            .saturating_sub(current.body.len() as u64)
                            .saturating_add(incoming.body.len() as u64);
                        if candidate_total > self.max_total_bytes {
                            PutOutcome::CapacityExceeded
                        } else {
                            table
                                .insert(channel.as_slice(), encoded.as_slice())
                                .context("replace ticket-store value")?;
                            next_total_bytes = candidate_total;
                            PutOutcome::Replaced
                        }
                    }
                }
                None if table.len().context("count ticket-store channels")?
                    >= self.max_channels =>
                {
                    PutOutcome::CapacityExceeded
                }
                None if current_total_bytes.saturating_add(incoming.body.len() as u64)
                    > self.max_total_bytes =>
                {
                    PutOutcome::CapacityExceeded
                }
                None => {
                    table
                        .insert(channel.as_slice(), encoded.as_slice())
                        .context("insert ticket-store value")?;
                    next_total_bytes =
                        current_total_bytes.saturating_add(incoming.body.len() as u64);
                    PutOutcome::Created
                }
            };
        }
        if next_total_bytes != current_total_bytes {
            write
                .open_table(META_TABLE)
                .context("open ticket-store metadata for PUT update")?
                .insert(TOTAL_BODY_BYTES_KEY, next_total_bytes)
                .context("update ticket-store byte total")?;
        }
        write
            .commit()
            .context("commit ticket-store PUT transaction")?;
        Ok(outcome)
    }

    fn get(&self, channel: [u8; 32], now: u64) -> Result<Option<StoredValue>> {
        let current = {
            let read = self
                .database
                .begin_read()
                .context("begin ticket-store GET transaction")?;
            let table = read
                .open_table(RECORD_TABLE)
                .context("open ticket-store record table for GET")?;
            table
                .get(channel.as_slice())
                .context("read ticket-store value")?
                .map(|value| value.value().to_vec())
        };
        let Some(current) = current else {
            return Ok(None);
        };
        let current = StoredOpaquePublication::decode(&current)?;
        if current.expired_at(now) {
            self.remove_if_expired(channel, now)?;
            return Ok(None);
        }
        Ok(Some(StoredValue {
            generation: current.generation,
            expires_at_unix_seconds: current.expires_at_unix_seconds,
            body: current.body,
        }))
    }

    fn remove_if_expired(&self, channel: [u8; 32], now: u64) -> Result<()> {
        let mut write = self
            .database
            .begin_write()
            .context("begin expired ticket-store removal transaction")?;
        write
            .set_durability(Durability::Immediate)
            .context("set expired ticket-store removal durability")?;
        let mut removed_bytes = 0_u64;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .context("open ticket-store record table for expired removal")?;
            let current = table
                .get(channel.as_slice())
                .context("re-read possibly expired ticket-store value")?
                .map(|value| value.value().to_vec());
            let expired = current
                .as_deref()
                .and_then(|bytes| StoredOpaquePublication::decode(bytes).ok())
                .filter(|record| record.expired_at(now));
            if let Some(expired) = expired {
                removed_bytes = expired.body.len() as u64;
                table
                    .remove(channel.as_slice())
                    .context("remove expired ticket-store value")?;
            }
        }
        if removed_bytes != 0 {
            let mut meta = write
                .open_table(META_TABLE)
                .context("open ticket-store metadata for expired removal")?;
            let current_total = meta
                .get(TOTAL_BODY_BYTES_KEY)
                .context("read ticket-store byte total for expired removal")?
                .map_or(0, |value| value.value());
            meta.insert(
                TOTAL_BODY_BYTES_KEY,
                current_total.saturating_sub(removed_bytes),
            )
            .context("update ticket-store byte total after expired removal")?;
        }
        write
            .commit()
            .context("commit expired ticket-store removal transaction")
    }

    fn cleanup(&self, now: u64) -> Result<u64> {
        let mut write = self
            .database
            .begin_write()
            .context("begin ticket-store cleanup transaction")?;
        write
            .set_durability(Durability::Immediate)
            .context("set ticket-store cleanup durability")?;
        let removed;
        let mut retained_bytes = 0_u64;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .context("open ticket-store record table for cleanup")?;
            let before = table.len().context("count ticket-store records")?;
            table
                .retain(|_, value| {
                    StoredOpaquePublication::decode(value).is_ok_and(|record| {
                        if record.expired_at(now) {
                            false
                        } else {
                            retained_bytes =
                                retained_bytes.saturating_add(record.body.len() as u64);
                            true
                        }
                    })
                })
                .context("retain live ticket-store records")?;
            removed =
                before.saturating_sub(table.len().context("count retained ticket-store records")?);
        }
        write
            .open_table(META_TABLE)
            .context("open ticket-store metadata for cleanup")?
            .insert(TOTAL_BODY_BYTES_KEY, retained_bytes)
            .context("rebuild ticket-store byte total during cleanup")?;
        write
            .commit()
            .context("commit ticket-store cleanup transaction")?;
        Ok(removed)
    }

    fn live_usage(&self) -> Result<(u64, u64)> {
        let read = self
            .database
            .begin_read()
            .context("begin ticket-store usage transaction")?;
        let channels = read
            .open_table(RECORD_TABLE)
            .context("open ticket-store records for usage")?
            .len()
            .context("count live ticket-store channels")?;
        let total_bytes = read
            .open_table(META_TABLE)
            .context("open ticket-store metadata for usage")?
            .get(TOTAL_BODY_BYTES_KEY)
            .context("read ticket-store byte total for usage")?
            .map_or(0, |value| value.value());
        Ok((channels, total_bytes))
    }

    fn reserve_transfer_bytes(
        &self,
        scope: &str,
        limit: u64,
        bytes: u64,
        now: u64,
    ) -> Result<bool> {
        if bytes == 0 {
            return Ok(true);
        }
        let window = now / TRANSFER_WINDOW_SECONDS;
        let window_key = format!("transfer-{scope}-window");
        let bytes_key = format!("transfer-{scope}-bytes");
        let mut write = self
            .database
            .begin_write()
            .context("begin ticket-store transfer accounting transaction")?;
        write
            .set_durability(Durability::Immediate)
            .context("set ticket-store transfer accounting durability")?;
        let (stored_window, stored_bytes) = {
            let meta = write
                .open_table(META_TABLE)
                .context("open ticket-store transfer accounting metadata")?;
            (
                meta.get(window_key.as_str())
                    .context("read ticket-store transfer window")?
                    .map_or(window, |value| value.value()),
                meta.get(bytes_key.as_str())
                    .context("read ticket-store transfer bytes")?
                    .map_or(0, |value| value.value()),
            )
        };
        let current_bytes = if stored_window == window {
            stored_bytes
        } else {
            0
        };
        let next_bytes = current_bytes.saturating_add(bytes);
        let allowed = next_bytes <= limit;
        {
            let mut meta = write
                .open_table(META_TABLE)
                .context("open ticket-store transfer accounting update")?;
            meta.insert(window_key.as_str(), window)
                .context("update ticket-store transfer window")?;
            if allowed {
                meta.insert(bytes_key.as_str(), next_bytes)
                    .context("update ticket-store transfer bytes")?;
            } else if stored_window != window {
                meta.insert(bytes_key.as_str(), 0)
                    .context("reset ticket-store transfer bytes")?;
            }
        }
        write
            .commit()
            .context("commit ticket-store transfer accounting transaction")?;
        Ok(allowed)
    }
}

#[derive(Clone, Copy, Debug)]
struct WindowCounter {
    started: Instant,
    count: u64,
}

#[derive(Debug)]
struct RateLimiter<K> {
    global: WindowCounter,
    per_subject: HashMap<K, WindowCounter>,
    per_subject_limit: u64,
    global_limit: u64,
}

impl<K> RateLimiter<K>
where
    K: Eq + Hash,
{
    fn new(per_subject_limit: u64, global_limit: u64) -> Self {
        let now = Instant::now();
        Self {
            global: WindowCounter {
                started: now,
                count: 0,
            },
            per_subject: HashMap::new(),
            per_subject_limit,
            global_limit,
        }
    }

    fn allow(&mut self, subject: K) -> bool {
        let now = Instant::now();
        if now.duration_since(self.global.started) >= RATE_WINDOW {
            self.global = WindowCounter {
                started: now,
                count: 0,
            };
            self.per_subject.clear();
        } else {
            self.per_subject
                .retain(|_, counter| now.duration_since(counter.started) < RATE_WINDOW);
        }
        if self.global.count >= self.global_limit {
            return false;
        }
        let counter = self.per_subject.entry(subject).or_insert(WindowCounter {
            started: now,
            count: 0,
        });
        if now.duration_since(counter.started) >= RATE_WINDOW {
            *counter = WindowCounter {
                started: now,
                count: 0,
            };
        }
        if counter.count >= self.per_subject_limit {
            return false;
        }
        self.global.count += 1;
        counter.count += 1;
        true
    }
}

struct ServerState {
    store: OpaqueStore,
    mailbox_store: BlindMailboxStore,
    limiter: Mutex<RateLimiter<IpAddr>>,
    peer_limiter: Mutex<RateLimiter<String>>,
    permits: Arc<Semaphore>,
    max_record_bytes: usize,
    trust_x_real_ip: bool,
    service_mode: StoreServiceMode,
    transfer_accounting_scope: Option<String>,
    max_transfer_bytes_per_30_days: Option<u64>,
}

pub struct TicketStoreServer {
    listener: TcpListener,
    state: Arc<ServerState>,
    config: StoreConfig,
}

impl TicketStoreServer {
    pub async fn bind(config: StoreConfig) -> Result<Self> {
        config.validate()?;
        let store = OpaqueStore::open(&config)?;
        store.cleanup(unix_time_now()?)?;
        let (live_channels, live_bytes) = store.live_usage()?;
        ensure!(
            live_channels <= config.max_channels && live_bytes <= config.max_total_bytes,
            "existing live ticket-store data exceeds the configured capacity"
        );
        let mailbox_identity = load_or_create_mailbox_identity(&config.data_dir)?;
        let mut mailbox_config =
            MailboxStoreConfig::new(config.data_dir.join(MAILBOX_DATA_DIRECTORY));
        mailbox_config.max_envelope_bytes = config.max_record_bytes.min(MAX_MAILBOX_ENVELOPE_BYTES);
        mailbox_config.max_items_per_mailbox =
            config.max_channels.min(DEFAULT_MAX_ITEMS_PER_MAILBOX);
        mailbox_config.max_total_items = config.max_channels;
        mailbox_config.max_total_bytes = config.max_total_bytes;
        let mailbox_store = BlindMailboxStore::open(mailbox_config, mailbox_identity)?;
        mailbox_store.cleanup(unix_time_now()?)?;
        let listener = TcpListener::bind(config.listen)
            .await
            .with_context(|| format!("bind opaque ticket store on {}", config.listen))?;
        let state = Arc::new(ServerState {
            store,
            mailbox_store,
            limiter: Mutex::new(RateLimiter::new(
                config.per_ip_requests_per_minute,
                config.global_requests_per_minute,
            )),
            peer_limiter: Mutex::new(RateLimiter::new(
                config.per_ip_requests_per_minute,
                config.global_requests_per_minute,
            )),
            permits: Arc::new(Semaphore::new(config.max_concurrent_connections)),
            max_record_bytes: config.max_record_bytes,
            trust_x_real_ip: config.trust_x_real_ip,
            service_mode: config.service_mode,
            transfer_accounting_scope: config.transfer_accounting_scope.clone(),
            max_transfer_bytes_per_30_days: config.max_transfer_bytes_per_30_days,
        });
        Ok(Self {
            listener,
            state,
            config,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap_or(self.config.listen)
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    pub fn mailbox_store_key(&self) -> MailboxStoreKey {
        self.state.mailbox_store.store_key()
    }

    pub fn mailbox_peer_service(&self) -> VolunteerMailboxService {
        VolunteerMailboxService {
            state: Arc::clone(&self.state),
        }
    }

    pub fn signed_mailbox_storage_offer(
        &self,
        provider_endpoint: Vec<u8>,
        issued_at_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<SignedMailboxStorageOffer> {
        self.state.mailbox_store.storage_offer(
            provider_endpoint,
            issued_at_unix_seconds,
            validity_seconds,
        )
    }

    pub async fn run_until<F>(self, shutdown: F) -> Result<()>
    where
        F: Future<Output = Result<()>>,
    {
        tokio::pin!(shutdown);
        let mut connections = JoinSet::new();
        let mut cleanup_interval =
            tokio::time::interval(Duration::from_secs(self.config.retention_seconds.min(60)));
        cleanup_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        cleanup_interval.tick().await;
        let result = loop {
            tokio::select! {
                shutdown_result = &mut shutdown => break shutdown_result,
                _ = cleanup_interval.tick() => {
                    self.state.store.cleanup(unix_time_now()?)?;
                    self.state.mailbox_store.cleanup(unix_time_now()?)?;
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = completed {
                        eprintln!("ticket_store_connection_task_error={error}");
                    }
                }
                accepted = self.listener.accept() => {
                    let (stream, peer) = accepted.context("accept ticket-store connection")?;
                    let permit = match Arc::clone(&self.state.permits).try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            connections.spawn(async move {
                                let mut stream = stream;
                                let _ = write_problem(&mut stream, HttpProblem::unavailable()).await;
                            });
                            continue;
                        }
                    };
                    let state = Arc::clone(&self.state);
                    connections.spawn(async move {
                        let _permit = permit;
                        if let Err(error) = handle_connection(stream, peer, state).await {
                            eprintln!("ticket_store_connection_error={error:#}");
                        }
                    });
                }
            }
        };
        connections.abort_all();
        while connections.join_next().await.is_some() {}
        result
    }
}

#[derive(Clone)]
pub struct VolunteerMailboxService {
    state: Arc<ServerState>,
}

impl VolunteerMailboxService {
    pub fn store_key(&self) -> MailboxStoreKey {
        self.state.mailbox_store.store_key()
    }

    pub fn handle_peer_request(
        &self,
        authenticated_peer_id: &str,
        request: &MailboxPeerRequest,
        now_unix_seconds: u64,
    ) -> Result<MailboxPeerResponse> {
        ensure!(
            !authenticated_peer_id.is_empty() && authenticated_peer_id.len() <= 128,
            "authenticated mailbox peer id is invalid"
        );
        if !self
            .state
            .peer_limiter
            .lock()
            .map_err(|_| anyhow::anyhow!("mailbox peer rate limiter lock poisoned"))?
            .allow(authenticated_peer_id.to_owned())
        {
            return MailboxPeerResponse::rejected(request, MailboxPeerRejection::RateLimited)
                .context("encode mailbox peer rate-limit response");
        }
        let request_bytes = request.encode()?.len() as u64;
        if !self.reserve_transfer(request_bytes, now_unix_seconds)? {
            return MailboxPeerResponse::rejected(
                request,
                MailboxPeerRejection::TransferBudgetExhausted,
            )
            .context("encode mailbox peer transfer-limit response");
        }
        let response = match request.operation() {
            MailboxPeerOperation::Put => self.handle_peer_put(request, now_unix_seconds),
            MailboxPeerOperation::List => self.handle_peer_list(request, now_unix_seconds),
            MailboxPeerOperation::Delete => self.handle_peer_delete(request, now_unix_seconds),
        }?;
        let response_bytes = response.encode(request)?.len() as u64;
        if !self.reserve_transfer(response_bytes, now_unix_seconds)? {
            return MailboxPeerResponse::rejected(
                request,
                MailboxPeerRejection::TransferBudgetExhausted,
            )
            .context("encode mailbox peer response transfer-limit response");
        }
        Ok(response)
    }

    fn reserve_transfer(&self, bytes: u64, now_unix_seconds: u64) -> Result<bool> {
        match (
            self.state.transfer_accounting_scope.as_deref(),
            self.state.max_transfer_bytes_per_30_days,
        ) {
            (Some(scope), Some(limit)) => {
                self.state
                    .store
                    .reserve_transfer_bytes(scope, limit, bytes, now_unix_seconds)
            }
            (None, None) => Ok(true),
            _ => anyhow::bail!("mailbox peer transfer accounting is partially configured"),
        }
    }

    fn handle_peer_put(
        &self,
        request: &MailboxPeerRequest,
        now_unix_seconds: u64,
    ) -> Result<MailboxPeerResponse> {
        let put = match MailboxPutRequest::decode_and_verify(request.payload()) {
            Ok(put) => put,
            Err(_) => {
                return MailboxPeerResponse::rejected(
                    request,
                    MailboxPeerRejection::InvalidCapability,
                )
                .context("encode invalid mailbox put response");
            }
        };
        let request_for_response = put.clone();
        let (address, authorization, envelope) = put.into_parts();
        let outcome = self
            .state
            .mailbox_store
            .put(address, &authorization, envelope, now_unix_seconds)
            .context("durable peer mailbox put")?;
        let payload = MailboxPutResponse::from_outcome(outcome)
            .encode(&request_for_response, self.store_key())?;
        MailboxPeerResponse::success(request, payload).context("encode mailbox peer put response")
    }

    fn handle_peer_list(
        &self,
        request: &MailboxPeerRequest,
        now_unix_seconds: u64,
    ) -> Result<MailboxPeerResponse> {
        let list = match MailboxListRequest::decode_and_verify(request.payload()) {
            Ok(list) => list,
            Err(_) => {
                return MailboxPeerResponse::rejected(
                    request,
                    MailboxPeerRejection::InvalidCapability,
                )
                .context("encode invalid mailbox list response");
            }
        };
        let page = self
            .state
            .mailbox_store
            .list_page(list.address(), list.authorization(), now_unix_seconds)
            .context("durable peer mailbox list")?;
        let payload = MailboxListResponse::new(page).encode(&list, self.store_key())?;
        MailboxPeerResponse::success(request, payload).context("encode mailbox peer list response")
    }

    fn handle_peer_delete(
        &self,
        request: &MailboxPeerRequest,
        now_unix_seconds: u64,
    ) -> Result<MailboxPeerResponse> {
        let delete = match MailboxDeleteRequest::decode_and_verify(request.payload()) {
            Ok(delete) => delete,
            Err(_) => {
                return MailboxPeerResponse::rejected(
                    request,
                    MailboxPeerRejection::InvalidCapability,
                )
                .context("encode invalid mailbox delete response");
            }
        };
        let outcome = self
            .state
            .mailbox_store
            .delete(delete.address(), delete.authorization(), now_unix_seconds)
            .context("durable peer mailbox delete")?;
        let payload =
            MailboxDeleteResponse::from_outcome(outcome).encode(&delete, self.store_key())?;
        MailboxPeerResponse::success(request, payload)
            .context("encode mailbox peer delete response")
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct HttpProblem {
    status: u16,
    reason: &'static str,
    message: &'static str,
    retry_after: Option<u64>,
}

impl HttpProblem {
    const fn bad_request(message: &'static str) -> Self {
        Self {
            status: 400,
            reason: "Bad Request",
            message,
            retry_after: None,
        }
    }

    const fn request_timeout() -> Self {
        Self {
            status: 408,
            reason: "Request Timeout",
            message: "request timed out",
            retry_after: None,
        }
    }

    const fn too_large() -> Self {
        Self {
            status: 413,
            reason: "Content Too Large",
            message: "opaque publication is too large",
            retry_after: None,
        }
    }

    const fn forbidden(message: &'static str) -> Self {
        Self {
            status: 403,
            reason: "Forbidden",
            message,
            retry_after: None,
        }
    }

    const fn internal(message: &'static str) -> Self {
        Self {
            status: 500,
            reason: "Internal Server Error",
            message,
            retry_after: None,
        }
    }

    const fn too_many_requests() -> Self {
        Self {
            status: 429,
            reason: "Too Many Requests",
            message: "request rate exceeded",
            retry_after: Some(60),
        }
    }

    const fn transfer_budget_exhausted() -> Self {
        Self {
            status: 429,
            reason: "Too Many Requests",
            message: "volunteer transfer budget exhausted",
            retry_after: Some(24 * 60 * 60),
        }
    }

    const fn unavailable() -> Self {
        Self {
            status: 503,
            reason: "Service Unavailable",
            message: "connection capacity exceeded",
            retry_after: Some(1),
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ServerState>,
) -> Result<()> {
    let request = match timeout(
        HTTP_IO_TIMEOUT,
        read_http_request(&mut stream, state.max_record_bytes),
    )
    .await
    {
        Ok(Ok(request)) => request,
        Ok(Err(problem)) => {
            write_problem(&mut stream, problem).await?;
            return Ok(());
        }
        Err(_) => {
            write_problem(&mut stream, HttpProblem::request_timeout()).await?;
            return Ok(());
        }
    };
    let rate_limit_ip = if state.trust_x_real_ip {
        match request.headers.get("x-real-ip") {
            Some(value) => match value.parse::<IpAddr>() {
                Ok(value) => value,
                Err(_) => {
                    write_problem(
                        &mut stream,
                        HttpProblem::bad_request("X-Real-IP is invalid"),
                    )
                    .await?;
                    return Ok(());
                }
            },
            None => peer.ip(),
        }
    } else {
        peer.ip()
    };
    let allowed = state
        .limiter
        .lock()
        .map_err(|_| anyhow::anyhow!("ticket-store rate limiter lock is poisoned"))?
        .allow(rate_limit_ip);
    if !allowed {
        write_problem(&mut stream, HttpProblem::too_many_requests()).await?;
        return Ok(());
    }
    let now = unix_time_now()?;
    if let (Some(scope), Some(limit)) = (
        state.transfer_accounting_scope.as_deref(),
        state.max_transfer_bytes_per_30_days,
    ) && !state
        .store
        .reserve_transfer_bytes(scope, limit, request.body.len() as u64, now)?
    {
        write_problem(&mut stream, HttpProblem::transfer_budget_exhausted()).await?;
        return Ok(());
    }
    let response = route_request(&state, request, now);
    match response {
        Ok(response) => {
            if let (Some(scope), Some(limit)) = (
                state.transfer_accounting_scope.as_deref(),
                state.max_transfer_bytes_per_30_days,
            ) && !state.store.reserve_transfer_bytes(
                scope,
                limit,
                response.body.len() as u64,
                now,
            )? {
                write_problem(&mut stream, HttpProblem::transfer_budget_exhausted()).await?;
                return Ok(());
            }
            write_response(&mut stream, response).await?
        }
        Err(problem) => write_problem(&mut stream, problem).await?,
    }
    Ok(())
}

fn route_request(
    state: &ServerState,
    request: HttpRequest,
    now: u64,
) -> std::result::Result<HttpResponse, HttpProblem> {
    if request.target == "/healthz" {
        if request.method != "GET" || !request.body.is_empty() {
            return Err(HttpProblem {
                status: 405,
                reason: "Method Not Allowed",
                message: "health endpoint requires GET",
                retry_after: None,
            });
        }
        return Ok(HttpResponse::text(200, "OK", b"ok\n".to_vec()));
    }
    if request.target.starts_with(MAILBOX_PATH_PREFIX) {
        return route_mailbox_request(&state.mailbox_store, request, now);
    }
    if state.service_mode == StoreServiceMode::MailboxOnly {
        return Err(HttpProblem {
            status: 404,
            reason: "Not Found",
            message: "resource not found",
            retry_after: None,
        });
    }
    let channel_text = request
        .target
        .strip_prefix(PUBLICATION_PATH_PREFIX)
        .ok_or(HttpProblem {
            status: 404,
            reason: "Not Found",
            message: "resource not found",
            retry_after: None,
        })?;
    let channel_id = channel_text
        .parse::<TicketPublicationChannelId>()
        .map_err(|_| HttpProblem::bad_request("invalid publication channel"))?;
    let channel = *channel_id.as_bytes();
    match request.method.as_str() {
        "PUT" => {
            if request.body.is_empty() {
                return Err(HttpProblem::bad_request("publication body is empty"));
            }
            let content_type = request.headers.get("content-type").ok_or(HttpProblem {
                status: 415,
                reason: "Unsupported Media Type",
                message: "publication content type is required",
                retry_after: None,
            })?;
            if content_type != PUBLICATION_CONTENT_TYPE {
                return Err(HttpProblem {
                    status: 415,
                    reason: "Unsupported Media Type",
                    message: "publication content type is unsupported",
                    retry_after: None,
                });
            }
            let generation = request
                .headers
                .get("x-kilogram-publication-generation")
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value != 0)
                .ok_or_else(|| {
                    HttpProblem::bad_request("valid publication generation is required")
                })?;
            let write_key = request
                .headers
                .get(WRITE_KEY_HEADER)
                .and_then(|value| value.parse::<TicketPublicationWriteKey>().ok())
                .ok_or_else(|| HttpProblem::forbidden("valid write capability is required"))?;
            let write_signature = request
                .headers
                .get(WRITE_SIGNATURE_HEADER)
                .and_then(|value| decode_signature(value).ok())
                .ok_or_else(|| HttpProblem::forbidden("valid write capability is required"))?;
            write_key
                .verify_authorization(channel_id, generation, &request.body, &write_signature)
                .map_err(|_| HttpProblem::forbidden("write capability authorization failed"))?;
            let outcome = state
                .store
                .put(channel, generation, request.body, now)
                .map_err(|_| HttpProblem {
                    status: 500,
                    reason: "Internal Server Error",
                    message: "durable publication write failed",
                    retry_after: None,
                })?;
            match outcome {
                PutOutcome::Created => Ok(HttpResponse::empty(201, "Created")),
                PutOutcome::Replaced => Ok(HttpResponse::empty(204, "No Content")),
                PutOutcome::AlreadyPresent => Ok(HttpResponse::empty(200, "OK")),
                PutOutcome::Conflict => Err(HttpProblem {
                    status: 409,
                    reason: "Conflict",
                    message: "publication generation is stale or conflicts",
                    retry_after: None,
                }),
                PutOutcome::CapacityExceeded => Err(HttpProblem {
                    status: 507,
                    reason: "Insufficient Storage",
                    message: "live channel capacity exceeded",
                    retry_after: None,
                }),
            }
        }
        "GET" => {
            if !request.body.is_empty() {
                return Err(HttpProblem::bad_request("GET request body is forbidden"));
            }
            let value = state.store.get(channel, now).map_err(|_| HttpProblem {
                status: 500,
                reason: "Internal Server Error",
                message: "durable publication read failed",
                retry_after: None,
            })?;
            match value {
                Some(value) => Ok(HttpResponse::publication(value)),
                None => Err(HttpProblem {
                    status: 404,
                    reason: "Not Found",
                    message: "publication is absent or expired",
                    retry_after: None,
                }),
            }
        }
        _ => Err(HttpProblem {
            status: 405,
            reason: "Method Not Allowed",
            message: "publication resource supports only GET and PUT",
            retry_after: None,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MailboxHttpResource {
    List(MailboxId),
    Item(MailboxId, MailboxItemId),
}

fn route_mailbox_request(
    store: &BlindMailboxStore,
    request: HttpRequest,
    now: u64,
) -> std::result::Result<HttpResponse, HttpProblem> {
    let resource = parse_mailbox_resource(&request.target)?;
    let content_type = request.headers.get("content-type").ok_or(HttpProblem {
        status: 415,
        reason: "Unsupported Media Type",
        message: "mailbox content type is required",
        retry_after: None,
    })?;
    if content_type != MAILBOX_CONTENT_TYPE {
        return Err(HttpProblem {
            status: 415,
            reason: "Unsupported Media Type",
            message: "mailbox content type is unsupported",
            retry_after: None,
        });
    }
    if request.body.is_empty() {
        return Err(HttpProblem::bad_request("mailbox request body is empty"));
    }
    let store_key = store.store_key();
    match (request.method.as_str(), resource) {
        ("PUT", MailboxHttpResource::Item(mailbox_id, item_id)) => {
            let mailbox_request = MailboxPutRequest::decode_and_verify(&request.body)
                .map_err(|_| HttpProblem::forbidden("mailbox write authorization failed"))?;
            if mailbox_request.mailbox_id() != mailbox_id || mailbox_request.item_id() != item_id {
                return Err(HttpProblem::bad_request(
                    "mailbox request does not match resource path",
                ));
            }
            let request_for_response = mailbox_request.clone();
            let (address, authorization, envelope) = mailbox_request.into_parts();
            let outcome = store
                .put(address, &authorization, envelope, now)
                .map_err(|_| HttpProblem::internal("durable mailbox write failed"))?;
            let response = MailboxPutResponse::from_outcome(outcome);
            let status = if matches!(response, MailboxPutResponse::Stored { created: true, .. }) {
                (201, "Created")
            } else {
                (200, "OK")
            };
            let body = response
                .encode(&request_for_response, store_key)
                .map_err(|_| HttpProblem::internal("mailbox put response encoding failed"))?;
            Ok(HttpResponse::mailbox(status.0, status.1, body))
        }
        ("POST", MailboxHttpResource::List(mailbox_id)) => {
            let mailbox_request = MailboxListRequest::decode_and_verify(&request.body)
                .map_err(|_| HttpProblem::forbidden("mailbox read authorization failed"))?;
            if mailbox_request.mailbox_id() != mailbox_id {
                return Err(HttpProblem::bad_request(
                    "mailbox request does not match resource path",
                ));
            }
            let page = store
                .list_page(
                    mailbox_request.address(),
                    mailbox_request.authorization(),
                    now,
                )
                .map_err(|_| HttpProblem::internal("durable mailbox list failed"))?;
            let response = MailboxListResponse::new(page);
            let body = response
                .encode(&mailbox_request, store_key)
                .map_err(|_| HttpProblem::internal("mailbox list response encoding failed"))?;
            Ok(HttpResponse::mailbox(200, "OK", body))
        }
        ("DELETE", MailboxHttpResource::Item(mailbox_id, item_id)) => {
            let mailbox_request = MailboxDeleteRequest::decode_and_verify(&request.body)
                .map_err(|_| HttpProblem::forbidden("mailbox delete authorization failed"))?;
            if mailbox_request.mailbox_id() != mailbox_id || mailbox_request.item_id() != item_id {
                return Err(HttpProblem::bad_request(
                    "mailbox request does not match resource path",
                ));
            }
            let outcome = store
                .delete(
                    mailbox_request.address(),
                    mailbox_request.authorization(),
                    now,
                )
                .map_err(|_| HttpProblem::internal("durable mailbox delete failed"))?;
            let response = MailboxDeleteResponse::from_outcome(outcome);
            let body = response
                .encode(&mailbox_request, store_key)
                .map_err(|_| HttpProblem::internal("mailbox delete response encoding failed"))?;
            Ok(HttpResponse::mailbox(200, "OK", body))
        }
        _ => Err(HttpProblem {
            status: 405,
            reason: "Method Not Allowed",
            message: "mailbox resource method is unsupported",
            retry_after: None,
        }),
    }
}

fn parse_mailbox_resource(target: &str) -> std::result::Result<MailboxHttpResource, HttpProblem> {
    let suffix = target
        .strip_prefix(MAILBOX_PATH_PREFIX)
        .ok_or(HttpProblem {
            status: 404,
            reason: "Not Found",
            message: "resource not found",
            retry_after: None,
        })?;
    let parts = suffix.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        [mailbox, "list"] => {
            Ok(MailboxHttpResource::List(mailbox.parse().map_err(
                |_| HttpProblem::bad_request("invalid mailbox ID"),
            )?))
        }
        [mailbox, "items", item] => Ok(MailboxHttpResource::Item(
            mailbox
                .parse()
                .map_err(|_| HttpProblem::bad_request("invalid mailbox ID"))?,
            item.parse()
                .map_err(|_| HttpProblem::bad_request("invalid mailbox item ID"))?,
        )),
        _ => Err(HttpProblem {
            status: 404,
            reason: "Not Found",
            message: "mailbox resource not found",
            retry_after: None,
        }),
    }
}

fn load_or_create_mailbox_identity(data_dir: &Path) -> Result<MailboxStoreIdentity> {
    fs::create_dir_all(data_dir)
        .with_context(|| format!("create opaque service directory {}", data_dir.display()))?;
    let path = data_dir.join(MAILBOX_IDENTITY_FILE);
    if let Some(identity) = load_mailbox_identity(&path)? {
        return Ok(identity);
    }
    let identity = MailboxStoreIdentity::generate()?;
    let secret = Zeroizing::new(identity.secret_bytes());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(secret.as_ref())
                .context("write blind mailbox store identity")?;
            file.sync_all()
                .context("sync blind mailbox store identity")?;
            Ok(identity)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            load_mailbox_identity(&path)?.context("mailbox identity appeared but is unreadable")
        }
        Err(error) => Err(error).context("create blind mailbox store identity"),
    }
}

fn load_mailbox_identity(path: &Path) -> Result<Option<MailboxStoreIdentity>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("inspect blind mailbox store identity"),
    };
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "blind mailbox store identity must be a regular file, not a symlink"
    );
    let mut file = fs::File::open(path).context("open blind mailbox store identity")?;
    let mut secret = Zeroizing::new([0_u8; 32]);
    file.read_exact(secret.as_mut())
        .context("read blind mailbox store identity")?;
    let mut trailing = [0_u8; 1];
    ensure!(
        file.read(&mut trailing)
            .context("check blind mailbox store identity length")?
            == 0,
        "blind mailbox store identity has trailing bytes"
    );
    Ok(Some(MailboxStoreIdentity::from_secret_bytes(*secret)))
}

async fn read_http_request(
    stream: &mut TcpStream,
    max_record_bytes: usize,
) -> std::result::Result<HttpRequest, HttpProblem> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
        if bytes.len() >= MAX_HEADER_BYTES {
            return Err(HttpProblem {
                status: 431,
                reason: "Request Header Fields Too Large",
                message: "request headers are too large",
                retry_after: None,
            });
        }
        let mut chunk = [0_u8; 4_096];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| HttpProblem::bad_request("request read failed"))?;
        if read == 0 {
            return Err(HttpProblem::bad_request("request ended before headers"));
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    if header_end > MAX_HEADER_BYTES {
        return Err(HttpProblem {
            status: 431,
            reason: "Request Header Fields Too Large",
            message: "request headers are too large",
            retry_after: None,
        });
    }
    let header_text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| HttpProblem::bad_request("request headers are not UTF-8"))?;
    let mut lines = header_text[..header_text.len().saturating_sub(4)].split("\r\n");
    let mut request_line = lines
        .next()
        .ok_or_else(|| HttpProblem::bad_request("request line is missing"))?
        .split_ascii_whitespace();
    let method = request_line
        .next()
        .ok_or_else(|| HttpProblem::bad_request("request method is missing"))?
        .to_owned();
    let target = request_line
        .next()
        .ok_or_else(|| HttpProblem::bad_request("request target is missing"))?
        .to_owned();
    let version = request_line
        .next()
        .ok_or_else(|| HttpProblem::bad_request("HTTP version is missing"))?;
    if request_line.next().is_some()
        || version != "HTTP/1.1"
        || method.len() > 16
        || target.is_empty()
        || target.len() > MAX_REQUEST_TARGET_BYTES
        || !target.starts_with('/')
        || target.contains(['?', '#'])
    {
        return Err(HttpProblem::bad_request("request line is invalid"));
    }
    let mut headers = BTreeMap::new();
    let mut header_count = 0_usize;
    for line in lines {
        header_count += 1;
        if header_count > MAX_HEADER_COUNT {
            return Err(HttpProblem {
                status: 431,
                reason: "Request Header Fields Too Large",
                message: "too many request headers",
                retry_after: None,
            });
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| HttpProblem::bad_request("request header is malformed"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(HttpProblem::bad_request("request header is invalid"));
        }
        let name = name.to_ascii_lowercase();
        if headers.insert(name, value.trim().to_owned()).is_some() {
            return Err(HttpProblem::bad_request(
                "duplicate request header is forbidden",
            ));
        }
    }
    if !headers.contains_key("host") {
        return Err(HttpProblem::bad_request("Host header is required"));
    }
    if headers.contains_key("transfer-encoding") || headers.contains_key("expect") {
        return Err(HttpProblem::bad_request(
            "streaming request bodies are unsupported",
        ));
    }
    let content_length = match headers.get("content-length") {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| HttpProblem::bad_request("Content-Length is invalid"))?,
        None if matches!(method.as_str(), "PUT" | "POST" | "DELETE") => {
            return Err(HttpProblem {
                status: 411,
                reason: "Length Required",
                message: "request method requires Content-Length",
                retry_after: None,
            });
        }
        None => 0,
    };
    if content_length > max_record_bytes {
        return Err(HttpProblem::too_large());
    }
    if bytes.len().saturating_sub(header_end) > content_length {
        return Err(HttpProblem::bad_request(
            "bytes after the declared request body are forbidden",
        ));
    }
    while bytes.len().saturating_sub(header_end) < content_length {
        let remaining = content_length - bytes.len().saturating_sub(header_end);
        let mut chunk = [0_u8; 8_192];
        let chunk_limit = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..chunk_limit])
            .await
            .map_err(|_| HttpProblem::bad_request("request body read failed"))?;
        if read == 0 {
            return Err(HttpProblem::bad_request(
                "request ended before the declared body",
            ));
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(HttpRequest {
        method,
        target,
        headers,
        body: bytes[header_end..].to_vec(),
    })
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl HttpResponse {
    fn empty(status: u16, reason: &'static str) -> Self {
        Self {
            status,
            reason,
            content_type: "application/octet-stream",
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn text(status: u16, reason: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            reason,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body,
        }
    }

    fn publication(value: StoredValue) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: PUBLICATION_CONTENT_TYPE,
            headers: vec![
                (
                    "X-Kilogram-Publication-Generation",
                    value.generation.to_string(),
                ),
                (
                    "X-Kilogram-Service-Expires-At",
                    value.expires_at_unix_seconds.to_string(),
                ),
            ],
            body: value.body,
        }
    }

    fn mailbox(status: u16, reason: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            reason,
            content_type: MAILBOX_CONTENT_TYPE,
            headers: Vec::new(),
            body,
        }
    }
}

async fn write_problem(stream: &mut TcpStream, problem: HttpProblem) -> Result<()> {
    let mut response = HttpResponse::text(
        problem.status,
        problem.reason,
        format!("{}\n", problem.message).into_bytes(),
    );
    if let Some(seconds) = problem.retry_after {
        response.headers.push(("Retry-After", seconds.to_string()));
    }
    write_response(stream, response).await
}

async fn write_response(stream: &mut TcpStream, response: HttpResponse) -> Result<()> {
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    for (name, value) in response.headers {
        headers.push_str(name);
        headers.push_str(": ");
        headers.push_str(&value);
        headers.push_str("\r\n");
    }
    headers.push_str("\r\n");
    timeout(HTTP_IO_TIMEOUT, async {
        stream.write_all(headers.as_bytes()).await?;
        stream.write_all(&response.body).await?;
        stream.shutdown().await
    })
    .await
    .context("ticket-store response timed out")?
    .context("write ticket-store response")
}

#[cfg(test)]
fn parse_channel_id(value: &str) -> Option<[u8; 32]> {
    value
        .parse::<TicketPublicationChannelId>()
        .ok()
        .map(|channel| *channel.as_bytes())
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::{Client, StatusCode};
    use tokio::sync::oneshot;

    #[test]
    fn durable_store_is_monotonic_idempotent_bounded_and_expiring() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = StoreConfig::local_test(directory.path().to_path_buf());
        config.retention_seconds = 30;
        config.max_channels = 1;
        let channel = [7_u8; 32];
        let other_channel = [8_u8; 32];
        {
            let store = OpaqueStore::open(&config)?;
            assert_eq!(
                store.put(channel, 4, b"opaque-four".to_vec(), 1_000)?,
                PutOutcome::Created
            );
            assert_eq!(
                store.put(channel, 4, b"opaque-four".to_vec(), 1_001)?,
                PutOutcome::AlreadyPresent
            );
            assert_eq!(
                store.put(channel, 4, b"equivocation".to_vec(), 1_001)?,
                PutOutcome::Conflict
            );
            assert_eq!(
                store.put(channel, 3, b"rollback".to_vec(), 1_001)?,
                PutOutcome::Conflict
            );
            assert_eq!(
                store.put(other_channel, 1, b"capacity".to_vec(), 1_001)?,
                PutOutcome::CapacityExceeded
            );
            assert_eq!(
                store.put(channel, 9, b"opaque-nine".to_vec(), 1_002)?,
                PutOutcome::Replaced
            );
        }
        let reopened = OpaqueStore::open(&config)?;
        assert_eq!(
            reopened.get(channel, 1_003)?,
            Some(StoredValue {
                generation: 9,
                expires_at_unix_seconds: 1_032,
                body: b"opaque-nine".to_vec(),
            })
        );
        assert_eq!(reopened.get(channel, 1_032)?, None);
        assert_eq!(
            reopened.put(other_channel, 1, b"after-expiry".to_vec(), 1_032)?,
            PutOutcome::Created
        );
        Ok(())
    }

    #[test]
    fn total_opaque_body_bytes_are_transactionally_bounded() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = StoreConfig::local_test(directory.path().to_path_buf());
        config.max_record_bytes = 8;
        config.max_total_bytes = 12;
        config.max_channels = 2;
        let store = OpaqueStore::open(&config)?;
        assert_eq!(
            store.put([1_u8; 32], 1, vec![1_u8; 8], 1_000)?,
            PutOutcome::Created
        );
        assert_eq!(
            store.put([2_u8; 32], 1, vec![2_u8; 5], 1_000)?,
            PutOutcome::CapacityExceeded
        );
        assert_eq!(
            store.put([1_u8; 32], 2, vec![3_u8; 4], 1_001)?,
            PutOutcome::Replaced
        );
        assert_eq!(
            store.put([2_u8; 32], 1, vec![4_u8; 8], 1_001)?,
            PutOutcome::Created
        );
        assert_eq!(
            store.put([3_u8; 32], 1, vec![5_u8; 1], 1_001)?,
            PutOutcome::CapacityExceeded
        );
        Ok(())
    }

    #[test]
    fn transfer_budget_is_durable_and_resets_after_thirty_day_window() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = StoreConfig::local_test(directory.path().to_path_buf());
        config.max_record_bytes = 8;
        {
            let store = OpaqueStore::open(&config)?;
            assert!(store.reserve_transfer_bytes("wifi", 10, 7, 1_000)?);
        }
        let reopened = OpaqueStore::open(&config)?;
        assert!(!reopened.reserve_transfer_bytes("wifi", 10, 4, 1_001)?);
        assert!(reopened.reserve_transfer_bytes(
            "wifi",
            10,
            10,
            TRANSFER_WINDOW_SECONDS + 1_001,
        )?);
        assert!(!reopened.reserve_transfer_bytes(
            "wifi",
            10,
            1,
            TRANSFER_WINDOW_SECONDS + 1_002,
        )?);
        assert!(reopened.reserve_transfer_bytes("ethernet", 10, 10, 1_002)?);
        Ok(())
    }

    #[test]
    fn mailbox_only_mode_fails_closed_for_ticket_publication_routes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = StoreConfig::local_test(directory.path().to_path_buf());
        config.service_mode = StoreServiceMode::MailboxOnly;
        let store = OpaqueStore::open(&config)?;
        let mailbox_identity = load_or_create_mailbox_identity(&config.data_dir)?;
        let mailbox_store = BlindMailboxStore::open(
            MailboxStoreConfig::new(config.data_dir.join(MAILBOX_DATA_DIRECTORY)),
            mailbox_identity,
        )?;
        let state = ServerState {
            store,
            mailbox_store,
            limiter: Mutex::new(RateLimiter::new(10, 10)),
            peer_limiter: Mutex::new(RateLimiter::new(10, 10)),
            permits: Arc::new(Semaphore::new(1)),
            max_record_bytes: config.max_record_bytes,
            trust_x_real_ip: false,
            service_mode: StoreServiceMode::MailboxOnly,
            transfer_accounting_scope: None,
            max_transfer_bytes_per_30_days: None,
        };
        let health = route_request(
            &state,
            HttpRequest {
                method: "GET".to_owned(),
                target: "/healthz".to_owned(),
                headers: BTreeMap::new(),
                body: Vec::new(),
            },
            1_000,
        )
        .map_err(|problem| anyhow::anyhow!("health route failed: {problem:?}"))?;
        assert_eq!(health.status, 200);
        let publication = match route_request(
            &state,
            HttpRequest {
                method: "GET".to_owned(),
                target: format!("{PUBLICATION_PATH_PREFIX}{}", "00".repeat(32)),
                headers: BTreeMap::new(),
                body: Vec::new(),
            },
            1_000,
        ) {
            Err(problem) => problem,
            Ok(_) => anyhow::bail!("mailbox-only mode accepted ticket publication"),
        };
        assert_eq!(publication.status, 404);
        let mailbox = match route_request(
            &state,
            HttpRequest {
                method: "POST".to_owned(),
                target: format!("{MAILBOX_PATH_PREFIX}{}/list", "00".repeat(32)),
                headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    MAILBOX_CONTENT_TYPE.to_owned(),
                )]),
                body: vec![0],
            },
            1_000,
        ) {
            Err(problem) => problem,
            Ok(_) => anyhow::bail!("malformed mailbox request was accepted"),
        };
        assert_ne!(mailbox.status, 404, "mailbox route must remain reachable");
        Ok(())
    }

    #[test]
    fn peer_service_executes_capability_request_and_rate_limits_endpoint_identity() -> Result<()> {
        use kilogram_crypto::DeviceEncryptionIdentity;
        use kilogram_mailbox::{
            MailboxAddress, MailboxEnvelope, MailboxPeerRejection, MailboxPeerRequest,
            MailboxReadCapability, MailboxWriteCapability,
        };

        let directory = tempfile::tempdir()?;
        let config = StoreConfig::local_test(directory.path().to_path_buf());
        let opaque_store = OpaqueStore::open(&config)?;
        let mailbox_identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let store_key = mailbox_identity.store_key();
        let mailbox_store = BlindMailboxStore::open(
            MailboxStoreConfig::new(config.data_dir.join(MAILBOX_DATA_DIRECTORY)),
            mailbox_identity,
        )?;
        let service = VolunteerMailboxService {
            state: Arc::new(ServerState {
                store: opaque_store,
                mailbox_store,
                limiter: Mutex::new(RateLimiter::new(10, 10)),
                peer_limiter: Mutex::new(RateLimiter::new(1, 10)),
                permits: Arc::new(Semaphore::new(1)),
                max_record_bytes: config.max_record_bytes,
                trust_x_real_ip: false,
                service_mode: StoreServiceMode::MailboxOnly,
                transfer_accounting_scope: None,
                max_transfer_bytes_per_30_days: None,
            }),
        };
        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        let item_id = MailboxItemId::from_bytes([4_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item_id,
            1_000,
            1_600,
            recipient.public_key(),
            b"peer-service-opaque-event",
        )?
        .encode()?;
        let put = MailboxPutRequest::new(
            address,
            write.authorize(address, item_id, 600, &envelope)?,
            envelope,
        )?;
        let request = MailboxPeerRequest::put(&put)?;
        let response = service.handle_peer_request("iroh-endpoint-a", &request, 1_000)?;
        let payload = response
            .success_payload()
            .context("peer mailbox PUT was not successful")?;
        assert!(matches!(
            MailboxPutResponse::decode_and_verify(payload, &put, store_key)?,
            MailboxPutResponse::Stored { created: true, .. }
        ));

        let limited = service.handle_peer_request("iroh-endpoint-a", &request, 1_001)?;
        assert_eq!(limited.rejection(), Some(MailboxPeerRejection::RateLimited));
        let idempotent = service.handle_peer_request("iroh-endpoint-b", &request, 1_001)?;
        assert!(matches!(
            MailboxPutResponse::decode_and_verify(
                idempotent
                    .success_payload()
                    .context("idempotent peer mailbox PUT was not successful")?,
                &put,
                store_key,
            )?,
            MailboxPutResponse::Stored { created: false, .. }
        ));
        Ok(())
    }

    #[test]
    fn blind_mailbox_routes_preserve_signed_wire_contract_without_network() -> Result<()> {
        use kilogram_crypto::DeviceEncryptionIdentity;
        use kilogram_mailbox::{
            MailboxAddress, MailboxEnvelope, MailboxReadCapability, MailboxRequestNonce,
            MailboxWriteCapability,
        };

        let directory = tempfile::tempdir()?;
        let identity_directory = directory.path().join("service");
        let identity = load_or_create_mailbox_identity(&identity_directory)?;
        let store_key = identity.store_key();
        assert_eq!(
            load_or_create_mailbox_identity(&identity_directory)?.store_key(),
            store_key
        );
        let store = BlindMailboxStore::open(
            MailboxStoreConfig::new(identity_directory.join("mailbox")),
            identity,
        )?;
        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        let item_id = MailboxItemId::from_bytes([4_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item_id,
            1_000,
            1_600,
            recipient.public_key(),
            b"opaque-event",
        )?
        .encode()?;
        let put = MailboxPutRequest::new(
            address,
            write.authorize(address, item_id, 600, &envelope)?,
            envelope,
        )?;
        let response = route_mailbox_request(
            &store,
            mailbox_test_request(
                "PUT",
                format!(
                    "{MAILBOX_PATH_PREFIX}{}/items/{item_id}",
                    address.mailbox_id()
                ),
                put.encode()?,
            ),
            1_000,
        )
        .map_err(|problem| anyhow::anyhow!("mailbox PUT failed: {problem:?}"))?;
        assert_eq!(response.status, 201);
        let put_response = MailboxPutResponse::decode_and_verify(&response.body, &put, store_key)?;
        let receipt = put_response
            .stored_receipt()
            .context("mailbox PUT response has no receipt")?;

        let list = MailboxListRequest::new(
            address,
            read.authorize_list_page(
                address,
                MailboxRequestNonce::from_bytes([5_u8; 32]),
                None,
                1,
            )?,
        )?;
        let response = route_mailbox_request(
            &store,
            mailbox_test_request(
                "POST",
                format!("{MAILBOX_PATH_PREFIX}{}/list", address.mailbox_id()),
                list.encode()?,
            ),
            1_001,
        )
        .map_err(|problem| anyhow::anyhow!("mailbox LIST failed: {problem:?}"))?;
        let list_response =
            MailboxListResponse::decode_and_verify(&response.body, &list, store_key)?;
        assert_eq!(list_response.page().items.len(), 1);

        let delete = MailboxDeleteRequest::new(
            address,
            read.authorize_delete(address, item_id, receipt.receipt_id()?)?,
        )?;
        let response = route_mailbox_request(
            &store,
            mailbox_test_request(
                "DELETE",
                format!(
                    "{MAILBOX_PATH_PREFIX}{}/items/{item_id}",
                    address.mailbox_id()
                ),
                delete.encode()?,
            ),
            1_002,
        )
        .map_err(|problem| anyhow::anyhow!("mailbox DELETE failed: {problem:?}"))?;
        assert!(matches!(
            MailboxDeleteResponse::decode_and_verify(&response.body, &delete, store_key)?,
            MailboxDeleteResponse::Deleted {
                newly_deleted: true,
                ..
            }
        ));
        Ok(())
    }

    fn mailbox_test_request(method: &str, target: String, body: Vec<u8>) -> HttpRequest {
        HttpRequest {
            method: method.to_owned(),
            target,
            headers: BTreeMap::from([("content-type".to_owned(), MAILBOX_CONTENT_TYPE.to_owned())]),
            body,
        }
    }

    #[tokio::test]
    async fn http_service_enforces_generation_content_type_path_and_rate_limits() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = StoreConfig::local_test(directory.path().to_path_buf());
        config.max_record_bytes = 32;
        config.per_ip_requests_per_minute = 10;
        config.global_requests_per_minute = 10;
        config.trust_x_real_ip = true;
        let server = TicketStoreServer::bind(config).await?;
        let address = server.local_addr();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let task = tokio::spawn(server.run_until(async {
            shutdown_receiver
                .await
                .map_err(|_| anyhow::anyhow!("test shutdown sender dropped"))
        }));
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let capability = kilogram_ticket_publication::TicketPublicationWriteCapability::derive(
            [7_u8; 32],
            b"http-store-test",
        );
        let channel_id = capability.write_key().channel_id();
        let url = format!("http://{address}{PUBLICATION_PATH_PREFIX}{channel_id}");
        let put = |generation: u64, body: &'static [u8]| {
            let signature = capability.authorize(channel_id, generation, body);
            client
                .put(&url)
                .header("content-type", PUBLICATION_CONTENT_TYPE)
                .header("x-kilogram-publication-generation", generation)
                .header(WRITE_KEY_HEADER, capability.write_key().to_string())
                .header(
                    WRITE_SIGNATURE_HEADER,
                    kilogram_ticket_publication::encode_signature(&signature),
                )
                .header("x-real-ip", "198.51.100.7")
                .body(body)
        };
        assert_eq!(
            put(1, b"opaque-one").send().await?.status(),
            StatusCode::CREATED
        );
        assert_eq!(put(1, b"opaque-one").send().await?.status(), StatusCode::OK);
        assert_eq!(
            put(1, b"conflict").send().await?.status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            put(2, b"opaque-two").send().await?.status(),
            StatusCode::NO_CONTENT
        );
        let get = client
            .get(&url)
            .header("x-real-ip", "198.51.100.7")
            .send()
            .await?;
        assert_eq!(get.status(), StatusCode::OK);
        assert_eq!(
            get.headers()
                .get("x-kilogram-publication-generation")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(get.bytes().await?.as_ref(), b"opaque-two");
        let attacker = kilogram_ticket_publication::TicketPublicationWriteCapability::derive(
            [8_u8; 32],
            b"http-store-test",
        );
        let attacker_body = b"arbitrary-high-generation";
        let attacker_signature = attacker.authorize(channel_id, u64::MAX, attacker_body);
        assert_eq!(
            client
                .put(&url)
                .header("content-type", PUBLICATION_CONTENT_TYPE)
                .header("x-kilogram-publication-generation", u64::MAX)
                .header(WRITE_KEY_HEADER, attacker.write_key().to_string())
                .header(
                    WRITE_SIGNATURE_HEADER,
                    kilogram_ticket_publication::encode_signature(&attacker_signature),
                )
                .header("x-real-ip", "198.51.100.7")
                .body(attacker_body.as_slice())
                .send()
                .await?
                .status(),
            StatusCode::FORBIDDEN
        );
        let retained = client
            .get(&url)
            .header("x-real-ip", "198.51.100.7")
            .send()
            .await?;
        assert_eq!(retained.status(), StatusCode::OK);
        assert_eq!(
            retained
                .headers()
                .get("x-kilogram-publication-generation")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(retained.bytes().await?.as_ref(), b"opaque-two");
        assert_eq!(
            client
                .put(&url)
                .header("content-type", PUBLICATION_CONTENT_TYPE)
                .header("x-kilogram-publication-generation", 3)
                .header("x-real-ip", "198.51.100.7")
                .body("unsigned")
                .send()
                .await?
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            client
                .put(&url)
                .header("content-type", "application/octet-stream")
                .header("x-kilogram-publication-generation", 3)
                .header("x-real-ip", "198.51.100.7")
                .body("opaque-three")
                .send()
                .await?
                .status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            client
                .put(&url)
                .header("content-type", PUBLICATION_CONTENT_TYPE)
                .header("x-kilogram-publication-generation", 3)
                .body(vec![0_u8; 33])
                .send()
                .await?
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            client
                .get(format!("http://{address}/healthz"))
                .header("x-real-ip", "198.51.100.7")
                .send()
                .await?
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .get(format!("http://{address}/healthz"))
                .header("x-real-ip", "198.51.100.7")
                .send()
                .await?
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        let _ = shutdown_sender.send(());
        task.await.context("join ticket-store test server")??;
        Ok(())
    }

    #[test]
    fn config_and_channel_parsing_fail_closed() {
        let mut config = StoreConfig::local_test(PathBuf::from("store"));
        assert!(config.validate().is_ok());
        config.listen = "0.0.0.0:8787".parse().unwrap_or(config.listen);
        assert!(config.validate().is_err());
        assert_eq!(parse_channel_id(&"00".repeat(32)), Some([0_u8; 32]));
        assert!(parse_channel_id(&"AA".repeat(32)).is_none());
        assert!(parse_channel_id("short").is_none());
    }
}
