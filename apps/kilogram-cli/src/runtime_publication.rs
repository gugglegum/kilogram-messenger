use std::{fmt, net::IpAddr, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use kilogram_crypto::{EncryptionPublicKey, SealedMessage};
use kilogram_identity::{AccountId, DeviceEncryptionIdentity, DeviceId, DeviceIdentity};
use kilogram_protocol::ConversationId;
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};

const PUBLICATION_VERSION: u8 = 1;
const ENVELOPE_VERSION: u8 = 1;
const OBSERVATION_VERSION: u8 = 1;
const PUBLICATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication:v1\0";
const OBSERVATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-observation:v1\0";
const PUBLICATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-id:v1\0";
const OBSERVATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-observation-id:v1\0";
const CHANNEL_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-channel:v1\0";
const TICKET_DIGEST_DOMAIN: &[u8] = b"kilogram:ticket-publication-ticket-digest:v1\0";
const RECIPIENT_SELECTOR_DOMAIN: &[u8] = b"kilogram:ticket-publication-recipient:v1\0";
const ENVELOPE_HPKE_INFO: &[u8] = b"kilogram:ticket-publication-envelope:v1";
const MAX_CLOCK_SKEW_SECONDS: u64 = 5 * 60;
pub const MIN_TICKET_PUBLICATION_TTL_SECONDS: u64 = 30;
pub const DEFAULT_TICKET_PUBLICATION_TTL_SECONDS: u64 = 15 * 60;
pub const MAX_TICKET_PUBLICATION_TTL_SECONDS: u64 = 60 * 60;
pub const MAX_TICKET_PUBLICATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TICKET_PUBLICATION_RECIPIENTS: usize = 64;
const MAX_TICKET_TEXT_BYTES: usize = 8 * 1024 * 1024;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationChannelId([u8; 32]);

impl TicketPublicationChannelId {
    pub fn derive(
        conversation_id: ConversationId,
        publisher_account_id: AccountId,
        publisher_device_id: DeviceId,
        recipient_account_id: AccountId,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHANNEL_ID_DOMAIN);
        hasher.update(conversation_id.as_bytes());
        hasher.update(publisher_account_id.as_bytes());
        hasher.update(publisher_device_id.as_bytes());
        hasher.update(recipient_account_id.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for TicketPublicationChannelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationId([u8; 32]);

impl fmt::Display for TicketPublicationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationObservationId([u8; 32]);

impl fmt::Display for TicketPublicationObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketPublicationContent {
    version: u8,
    channel_id: TicketPublicationChannelId,
    publisher_account_id: AccountId,
    publisher_device_id: DeviceId,
    recipient_account_id: AccountId,
    generation: u64,
    previous_publication_id: Option<TicketPublicationId>,
    published_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    ticket_digest: [u8; 32],
    ticket: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketPublication {
    content: TicketPublicationContent,
    signature: Vec<u8>,
}

impl SignedTicketPublication {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        channel_id: TicketPublicationChannelId,
        publisher_account_id: AccountId,
        recipient_account_id: AccountId,
        ticket: String,
        published_at_unix_seconds: u64,
        ttl_seconds: u64,
        previous: Option<&Self>,
    ) -> Result<Self> {
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&ttl_seconds),
            "ticket publication TTL must be between {MIN_TICKET_PUBLICATION_TTL_SECONDS} and {MAX_TICKET_PUBLICATION_TTL_SECONDS} seconds"
        );
        ensure!(
            !ticket.is_empty() && ticket.len() <= MAX_TICKET_TEXT_BYTES,
            "ticket publication contains an invalid ticket size"
        );
        let expires_at_unix_seconds = published_at_unix_seconds
            .checked_add(ttl_seconds)
            .context("ticket publication expiry overflow")?;
        let (generation, previous_publication_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.channel_id() == channel_id
                        && previous.publisher_account_id() == publisher_account_id
                        && previous.publisher_device_id() == identity.device_id()
                        && previous.recipient_account_id() == recipient_account_id,
                    "ticket publication chain changes channel identity"
                );
                ensure!(
                    published_at_unix_seconds >= previous.published_at_unix_seconds(),
                    "ticket publication chain moves publication time backwards"
                );
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("ticket publication generation overflow")?,
                    Some(previous.publication_id()?),
                )
            }
            None => (1, None),
        };
        let content = TicketPublicationContent {
            version: PUBLICATION_VERSION,
            channel_id,
            publisher_account_id,
            publisher_device_id: identity.device_id(),
            recipient_account_id,
            generation,
            previous_publication_id,
            published_at_unix_seconds,
            expires_at_unix_seconds,
            ticket_digest: ticket_digest(ticket.as_bytes()),
            ticket,
        };
        let signature = identity
            .sign(&signing_bytes(PUBLICATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let publication = Self { content, signature };
        publication.verify(previous)?;
        Ok(publication)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        postcard::to_allocvec(self).context("encode signed ticket publication")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "signed ticket publication is too large"
        );
        let publication: Self =
            postcard::from_bytes(bytes).context("decode signed ticket publication")?;
        publication.verify_signature()?;
        Ok(publication)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.channel_id() == previous.channel_id()
                        && self.publisher_account_id() == previous.publisher_account_id()
                        && self.publisher_device_id() == previous.publisher_device_id()
                        && self.recipient_account_id() == previous.recipient_account_id(),
                    "ticket publication chain changes channel identity"
                );
                ensure!(
                    self.generation()
                        == previous
                            .generation()
                            .checked_add(1)
                            .context("ticket publication generation overflow")?
                        && self.content.previous_publication_id == Some(previous.publication_id()?),
                    "ticket publication chain is not contiguous"
                );
                ensure!(
                    self.published_at_unix_seconds() >= previous.published_at_unix_seconds(),
                    "ticket publication chain moves publication time backwards"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_publication_id.is_none(),
                "ticket publication chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<()> {
        self.verify_signature()?;
        ensure!(
            self.published_at_unix_seconds()
                <= now_unix_seconds.saturating_add(MAX_CLOCK_SKEW_SECONDS),
            "ticket publication time is too far in the future"
        );
        ensure!(
            self.expires_at_unix_seconds() > now_unix_seconds,
            "ticket publication has expired"
        );
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == PUBLICATION_VERSION,
            "unsupported ticket publication version"
        );
        ensure!(
            !self.content.ticket.is_empty()
                && self.content.ticket.len() <= MAX_TICKET_TEXT_BYTES
                && self.content.ticket_digest == ticket_digest(self.content.ticket.as_bytes()),
            "ticket publication ticket digest is invalid"
        );
        let lifetime = self
            .content
            .expires_at_unix_seconds
            .checked_sub(self.content.published_at_unix_seconds)
            .context("ticket publication expiry precedes publication time")?;
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&lifetime),
            "ticket publication lifetime is outside protocol bounds"
        );
        ensure!(
            self.content.generation != 0,
            "ticket publication generation must be non-zero"
        );
        self.content
            .publisher_device_id
            .verify(
                &signing_bytes(PUBLICATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket publication device signature")
    }

    pub fn publication_id(&self) -> Result<TicketPublicationId> {
        Ok(TicketPublicationId(domain_hash(
            PUBLICATION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.channel_id
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.publisher_account_id
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.publisher_device_id
    }

    pub fn recipient_account_id(&self) -> AccountId {
        self.content.recipient_account_id
    }

    pub fn generation(&self) -> u64 {
        self.content.generation
    }

    pub fn published_at_unix_seconds(&self) -> u64 {
        self.content.published_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn ticket_digest(&self) -> [u8; 32] {
        self.content.ticket_digest
    }

    pub fn ticket(&self) -> &str {
        &self.content.ticket
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct EncryptedTicketPublicationSlot {
    selector: [u8; 32],
    sealed_publication: SealedMessage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncryptedTicketPublication {
    version: u8,
    channel_id: TicketPublicationChannelId,
    publication_id: TicketPublicationId,
    publication_generation: u64,
    slots: Vec<EncryptedTicketPublicationSlot>,
}

impl EncryptedTicketPublication {
    pub fn seal(
        publication: &SignedTicketPublication,
        recipients: &[(DeviceId, EncryptionPublicKey)],
    ) -> Result<Self> {
        publication.verify_signature()?;
        ensure!(
            !recipients.is_empty() && recipients.len() <= MAX_TICKET_PUBLICATION_RECIPIENTS,
            "ticket publication recipient count is outside protocol bounds"
        );
        let publication_id = publication.publication_id()?;
        let plaintext = publication.encode()?;
        let mut slots = Vec::with_capacity(recipients.len());
        for (device_id, encryption_key) in recipients {
            let selector = recipient_selector(publication.channel_id(), *device_id);
            ensure!(
                !slots
                    .iter()
                    .any(|slot: &EncryptedTicketPublicationSlot| slot.selector == selector),
                "ticket publication contains a duplicate recipient"
            );
            let aad = envelope_aad(
                publication.channel_id(),
                publication_id,
                publication.generation(),
                selector,
            )?;
            let sealed_publication = encryption_key
                .seal(&plaintext, ENVELOPE_HPKE_INFO, &aad)
                .context("encrypt ticket publication for recipient device")?;
            slots.push(EncryptedTicketPublicationSlot {
                selector,
                sealed_publication,
            });
        }
        slots.sort_by_key(|slot| slot.selector);
        let envelope = Self {
            version: ENVELOPE_VERSION,
            channel_id: publication.channel_id(),
            publication_id,
            publication_generation: publication.generation(),
            slots,
        };
        envelope.verify_outer()?;
        Ok(envelope)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_outer()?;
        let bytes = postcard::to_allocvec(self).context("encode encrypted ticket publication")?;
        ensure!(
            bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "encrypted ticket publication is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "encrypted ticket publication is too large"
        );
        let envelope: Self =
            postcard::from_bytes(bytes).context("decode encrypted ticket publication")?;
        envelope.verify_outer()?;
        Ok(envelope)
    }

    pub fn open(
        &self,
        recipient_device_id: DeviceId,
        recipient_encryption: &DeviceEncryptionIdentity,
        now_unix_seconds: u64,
    ) -> Result<SignedTicketPublication> {
        self.verify_outer()?;
        let selector = recipient_selector(self.channel_id, recipient_device_id);
        let slot = self
            .slots
            .iter()
            .find(|slot| slot.selector == selector)
            .context("encrypted ticket publication has no slot for this device")?;
        let aad = envelope_aad(
            self.channel_id,
            self.publication_id,
            self.publication_generation,
            selector,
        )?;
        let plaintext = recipient_encryption
            .open(&slot.sealed_publication, ENVELOPE_HPKE_INFO, &aad)
            .context("decrypt ticket publication for local device")?;
        let publication = SignedTicketPublication::decode(&plaintext)?;
        publication.verify_at(now_unix_seconds)?;
        ensure!(
            publication.channel_id() == self.channel_id
                && publication.publication_id()? == self.publication_id
                && publication.generation() == self.publication_generation,
            "encrypted ticket publication outer metadata is inconsistent"
        );
        Ok(publication)
    }

    fn verify_outer(&self) -> Result<()> {
        ensure!(
            self.version == ENVELOPE_VERSION,
            "unsupported encrypted ticket publication version"
        );
        ensure!(
            self.publication_generation != 0,
            "encrypted ticket publication generation must be non-zero"
        );
        ensure!(
            !self.slots.is_empty() && self.slots.len() <= MAX_TICKET_PUBLICATION_RECIPIENTS,
            "encrypted ticket publication slot count is outside protocol bounds"
        );
        let mut previous = None;
        for slot in &self.slots {
            ensure!(
                previous.is_none_or(|value| value < slot.selector),
                "encrypted ticket publication selectors are duplicated or non-canonical"
            );
            ensure!(
                slot.sealed_publication.encapsulated_key.len() <= 128
                    && slot.sealed_publication.ciphertext.len() <= MAX_TICKET_PUBLICATION_BYTES,
                "encrypted ticket publication slot is outside protocol bounds"
            );
            previous = Some(slot.selector);
        }
        Ok(())
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.channel_id
    }

    pub fn publication_generation(&self) -> u64 {
        self.publication_generation
    }

    pub fn recipient_count(&self) -> usize {
        self.slots.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketPublicationObservationContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    channel_id: TicketPublicationChannelId,
    publisher_account_id: AccountId,
    publisher_device_id: DeviceId,
    observation_generation: u64,
    previous_observation_id: Option<TicketPublicationObservationId>,
    publication_generation: u64,
    publication_id: TicketPublicationId,
    ticket_digest: [u8; 32],
    observed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketPublicationObservation {
    content: TicketPublicationObservationContent,
    signature: Vec<u8>,
}

impl SignedTicketPublicationObservation {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        publication: &SignedTicketPublication,
        observed_at_unix_seconds: u64,
        previous: Option<&Self>,
    ) -> Result<Self> {
        publication.verify_at(observed_at_unix_seconds)?;
        ensure!(
            publication.recipient_account_id() == local_account_id,
            "ticket publication is addressed to another account"
        );
        let (observation_generation, previous_observation_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.local_account_id() == local_account_id
                        && previous.local_device_id() == identity.device_id()
                        && previous.channel_id() == publication.channel_id()
                        && previous.publisher_account_id() == publication.publisher_account_id()
                        && previous.publisher_device_id() == publication.publisher_device_id(),
                    "ticket publication observation chain changes identity"
                );
                ensure!(
                    publication.generation() > previous.publication_generation(),
                    "ticket publication observation does not advance the peer high-water mark"
                );
                (
                    previous
                        .observation_generation()
                        .checked_add(1)
                        .context("ticket publication observation generation overflow")?,
                    Some(previous.observation_id()?),
                )
            }
            None => (1, None),
        };
        let content = TicketPublicationObservationContent {
            version: OBSERVATION_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            channel_id: publication.channel_id(),
            publisher_account_id: publication.publisher_account_id(),
            publisher_device_id: publication.publisher_device_id(),
            observation_generation,
            previous_observation_id,
            publication_generation: publication.generation(),
            publication_id: publication.publication_id()?,
            ticket_digest: publication.ticket_digest(),
            observed_at_unix_seconds,
        };
        let signature = identity
            .sign(&signing_bytes(OBSERVATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let observation = Self { content, signature };
        observation.verify(previous)?;
        Ok(observation)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        postcard::to_allocvec(self).context("encode ticket publication observation")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "ticket publication observation is too large"
        );
        let observation: Self =
            postcard::from_bytes(bytes).context("decode ticket publication observation")?;
        observation.verify_signature()?;
        Ok(observation)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id()
                        && self.channel_id() == previous.channel_id()
                        && self.publisher_account_id() == previous.publisher_account_id()
                        && self.publisher_device_id() == previous.publisher_device_id(),
                    "ticket publication observation chain changes identity"
                );
                ensure!(
                    self.observation_generation()
                        == previous
                            .observation_generation()
                            .checked_add(1)
                            .context("ticket publication observation generation overflow")?
                        && self.content.previous_observation_id == Some(previous.observation_id()?),
                    "ticket publication observation chain is not contiguous"
                );
                ensure!(
                    self.publication_generation() > previous.publication_generation()
                        && self.observed_at_unix_seconds() >= previous.observed_at_unix_seconds(),
                    "ticket publication observation rolls its high-water mark back"
                );
            }
            None => ensure!(
                self.observation_generation() == 1
                    && self.content.previous_observation_id.is_none()
                    && self.publication_generation() != 0,
                "ticket publication observation chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == OBSERVATION_VERSION,
            "unsupported ticket publication observation version"
        );
        ensure!(
            self.content.observation_generation != 0 && self.content.publication_generation != 0,
            "ticket publication observation generation must be non-zero"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(OBSERVATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket publication observation signature")
    }

    pub fn observation_id(&self) -> Result<TicketPublicationObservationId> {
        Ok(TicketPublicationObservationId(domain_hash(
            OBSERVATION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.channel_id
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.publisher_account_id
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.publisher_device_id
    }

    pub fn observation_generation(&self) -> u64 {
        self.content.observation_generation
    }

    pub fn publication_generation(&self) -> u64 {
        self.content.publication_generation
    }

    pub fn publication_id(&self) -> TicketPublicationId {
        self.content.publication_id
    }

    pub fn ticket_digest(&self) -> [u8; 32] {
        self.content.ticket_digest
    }

    pub fn observed_at_unix_seconds(&self) -> u64 {
        self.content.observed_at_unix_seconds
    }
}

#[derive(Clone)]
pub struct TicketPublicationStoreClient {
    client: Client,
    base_url: Url,
}

impl TicketPublicationStoreClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let mut base_url = Url::parse(base_url).context("parse ticket publication service URL")?;
        ensure!(
            base_url.username().is_empty()
                && base_url.password().is_none()
                && base_url.query().is_none()
                && base_url.fragment().is_none(),
            "ticket publication service URL must not contain credentials, query, or fragment"
        );
        match base_url.scheme() {
            "https" => {}
            "http" => {
                let host = base_url
                    .host_str()
                    .context("ticket publication HTTP URL has no host")?;
                let ip: IpAddr = host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse()
                    .context("plain HTTP ticket publication is allowed only for a loopback IP")?;
                ensure!(
                    ip.is_loopback(),
                    "plain HTTP ticket publication is allowed only for a loopback IP"
                );
            }
            _ => bail!("ticket publication service URL must use HTTPS"),
        }
        ensure!(
            base_url.path_segments().is_some(),
            "ticket publication service URL cannot be a base"
        );
        let normalized_path = format!("{}/", base_url.path().trim_end_matches('/'));
        base_url.set_path(&normalized_path);
        let client = Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .context("build ticket publication HTTPS client")?;
        Ok(Self { client, base_url })
    }

    pub async fn put(&self, envelope: &EncryptedTicketPublication) -> Result<usize> {
        let body = envelope.encode()?;
        let url = self.record_url(envelope.channel_id())?;
        let response = self
            .client
            .put(url)
            .header(
                "content-type",
                "application/vnd.kilogram.ticket-publication",
            )
            .header(
                "x-kilogram-publication-generation",
                envelope.publication_generation().to_string(),
            )
            .body(body.clone())
            .send()
            .await
            .context("upload encrypted ticket publication")?;
        ensure!(
            matches!(
                response.status(),
                StatusCode::OK | StatusCode::CREATED | StatusCode::NO_CONTENT
            ),
            "ticket publication service rejected upload with HTTP {}",
            response.status()
        );
        Ok(body.len())
    }

    pub async fn get(
        &self,
        channel_id: TicketPublicationChannelId,
    ) -> Result<EncryptedTicketPublication> {
        let url = self.record_url(channel_id)?;
        let mut response = self
            .client
            .get(url)
            .header("accept", "application/vnd.kilogram.ticket-publication")
            .send()
            .await
            .context("fetch encrypted ticket publication")?;
        ensure!(
            response.status() == StatusCode::OK,
            "ticket publication service returned HTTP {}",
            response.status()
        );
        if let Some(length) = response.content_length() {
            ensure!(
                length <= MAX_TICKET_PUBLICATION_BYTES as u64,
                "ticket publication HTTP response is too large"
            );
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("read ticket publication HTTP response")?
        {
            ensure!(
                body.len().saturating_add(chunk.len()) <= MAX_TICKET_PUBLICATION_BYTES,
                "ticket publication HTTP response is too large"
            );
            body.extend_from_slice(&chunk);
        }
        EncryptedTicketPublication::decode(&body)
    }

    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    fn record_url(&self, channel_id: TicketPublicationChannelId) -> Result<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("ticket publication service URL cannot be a base"))?
            .pop_if_empty()
            .extend(["v1", "ticket-publications", &channel_id.to_string()]);
        Ok(url)
    }
}

fn recipient_selector(
    channel_id: TicketPublicationChannelId,
    recipient_device_id: DeviceId,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RECIPIENT_SELECTOR_DOMAIN);
    hasher.update(channel_id.as_bytes());
    hasher.update(recipient_device_id.as_bytes());
    *hasher.finalize().as_bytes()
}

fn envelope_aad(
    channel_id: TicketPublicationChannelId,
    publication_id: TicketPublicationId,
    publication_generation: u64,
    selector: [u8; 32],
) -> Result<Vec<u8>> {
    postcard::to_allocvec(&(
        ENVELOPE_VERSION,
        channel_id,
        publication_id,
        publication_generation,
        selector,
    ))
    .context("encode ticket publication envelope AAD")
}

fn ticket_digest(ticket: &[u8]) -> [u8; 32] {
    domain_hash(TICKET_DIGEST_DOMAIN, ticket)
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut bytes = domain.to_vec();
    bytes.extend(postcard::to_allocvec(content).context("encode signed publication content")?);
    Ok(bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_crypto::DeviceEncryptionIdentity;
    use kilogram_identity::AccountRootState;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    async fn read_http_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>)> {
        let mut bytes = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 4_096];
            let read = stream.read(&mut chunk).await?;
            ensure!(
                read != 0,
                "mock publication client closed before HTTP headers"
            );
            bytes.extend_from_slice(&chunk[..read]);
            ensure!(
                bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
                "mock HTTP request is too large"
            );
            if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let headers = std::str::from_utf8(&bytes[..header_end])?;
        let mut lines = headers.split("\r\n");
        let request_line = lines.next().context("mock HTTP request line is missing")?;
        let mut request_parts = request_line.split_ascii_whitespace();
        let method = request_parts
            .next()
            .context("mock HTTP method is missing")?
            .to_owned();
        let path = request_parts
            .next()
            .context("mock HTTP path is missing")?
            .to_owned();
        let content_length = lines
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>())
            })
            .transpose()?
            .unwrap_or(0);
        ensure!(
            content_length <= MAX_TICKET_PUBLICATION_BYTES,
            "mock HTTP request body is too large"
        );
        while bytes.len() - header_end < content_length {
            let mut chunk = [0_u8; 4_096];
            let read = stream.read(&mut chunk).await?;
            ensure!(read != 0, "mock publication client closed during HTTP body");
            bytes.extend_from_slice(&chunk[..read]);
        }
        Ok((
            method,
            path,
            bytes[header_end..header_end + content_length].to_vec(),
        ))
    }

    #[test]
    fn encrypted_publication_and_observation_are_recipient_bound_and_monotonic()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let publisher_root = AccountRootState::create(directory.path().join("publisher-root"))?;
        let recipient_root = AccountRootState::create(directory.path().join("recipient-root"))?;
        let publisher = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let recipient_encryption = DeviceEncryptionIdentity::generate()?;
        let other_encryption = DeviceEncryptionIdentity::generate()?;
        let channel = TicketPublicationChannelId::derive(
            ConversationId::from_label("publication-test"),
            publisher_root.account_id(),
            publisher.device_id(),
            recipient_root.account_id(),
        );
        assert_ne!(
            channel,
            TicketPublicationChannelId::derive(
                ConversationId::from_label("publication-test"),
                publisher_root.account_id(),
                DeviceIdentity::generate()?.device_id(),
                recipient_root.account_id(),
            )
        );
        let first = SignedTicketPublication::sign(
            &publisher,
            channel,
            publisher_root.account_id(),
            recipient_root.account_id(),
            "signed-ticket-one".to_owned(),
            1_000,
            300,
            None,
        )?;
        let envelope = EncryptedTicketPublication::seal(
            &first,
            &[(recipient.device_id(), recipient_encryption.public_key())],
        )?;
        let restarted = EncryptedTicketPublication::decode(&envelope.encode()?)?;
        let opened = restarted.open(recipient.device_id(), &recipient_encryption, 1_001)?;
        assert_eq!(opened, first);
        assert!(
            restarted
                .open(recipient.device_id(), &other_encryption, 1_001)
                .is_err()
        );
        assert!(
            restarted
                .open(
                    DeviceIdentity::generate()?.device_id(),
                    &recipient_encryption,
                    1_001
                )
                .is_err()
        );

        let first_observation = SignedTicketPublicationObservation::sign(
            &recipient,
            recipient_root.account_id(),
            &first,
            1_001,
            None,
        )?;
        let second = SignedTicketPublication::sign(
            &publisher,
            channel,
            publisher_root.account_id(),
            recipient_root.account_id(),
            "signed-ticket-two".to_owned(),
            1_100,
            300,
            Some(&first),
        )?;
        let second_observation = SignedTicketPublicationObservation::sign(
            &recipient,
            recipient_root.account_id(),
            &second,
            1_101,
            Some(&first_observation),
        )?;
        second_observation.verify(Some(&first_observation))?;
        assert_eq!(second_observation.publication_generation(), 2);
        assert!(
            SignedTicketPublicationObservation::sign(
                &recipient,
                recipient_root.account_id(),
                &first,
                1_102,
                Some(&second_observation),
            )
            .is_err()
        );
        assert!(opened.verify_at(1_301).is_err());
        Ok(())
    }

    #[test]
    fn store_client_requires_https_except_for_numeric_loopback() {
        assert!(TicketPublicationStoreClient::new("https://example.invalid/base").is_ok());
        assert!(TicketPublicationStoreClient::new("http://127.0.0.1:8080/base").is_ok());
        assert!(TicketPublicationStoreClient::new("http://[::1]:8080/base").is_ok());
        assert!(TicketPublicationStoreClient::new("http://localhost:8080/base").is_err());
        assert!(TicketPublicationStoreClient::new("http://192.0.2.1/base").is_err());
        assert!(TicketPublicationStoreClient::new("file:///tmp/store").is_err());
        assert!(TicketPublicationStoreClient::new("https://user@example.invalid/base").is_err());
        assert!(TicketPublicationStoreClient::new("https://example.invalid/base?q=1").is_err());
    }

    #[tokio::test]
    async fn store_client_round_trips_only_the_opaque_envelope() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let publisher_root = AccountRootState::create(directory.path().join("publisher-root"))?;
        let recipient_root = AccountRootState::create(directory.path().join("recipient-root"))?;
        let publisher = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let recipient_encryption = DeviceEncryptionIdentity::generate()?;
        let channel = TicketPublicationChannelId::derive(
            ConversationId::from_label("publication-http-test"),
            publisher_root.account_id(),
            publisher.device_id(),
            recipient_root.account_id(),
        );
        let publication = SignedTicketPublication::sign(
            &publisher,
            channel,
            publisher_root.account_id(),
            recipient_root.account_id(),
            "opaque-ticket".to_owned(),
            2_000,
            300,
            None,
        )?;
        let envelope = EncryptedTicketPublication::seal(
            &publication,
            &[(recipient.device_id(), recipient_encryption.public_key())],
        )?;
        let expected_body = envelope.encode()?;
        let expected_path = format!("/fixture/v1/ticket-publications/{channel}");

        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let server_body = expected_body.clone();
        let server_path = expected_path.clone();
        let server = tokio::spawn(async move {
            let (mut put_stream, _) = listener.accept().await?;
            let (method, path, body) = read_http_request(&mut put_stream).await?;
            ensure!(
                method == "PUT" && path == server_path,
                "unexpected mock PUT request"
            );
            ensure!(
                body == server_body,
                "publication service received non-envelope bytes"
            );
            put_stream
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .await?;

            let (mut get_stream, _) = listener.accept().await?;
            let (method, path, body) = read_http_request(&mut get_stream).await?;
            ensure!(
                method == "GET" && path == server_path,
                "unexpected mock GET request"
            );
            ensure!(body.is_empty(), "mock GET unexpectedly contains a body");
            let response_headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.kilogram.ticket-publication\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                server_body.len()
            );
            get_stream.write_all(response_headers.as_bytes()).await?;
            get_stream.write_all(&server_body).await?;
            Ok::<(), anyhow::Error>(())
        });

        let client = TicketPublicationStoreClient::new(&format!("http://{address}/fixture"))?;
        assert_eq!(client.put(&envelope).await?, expected_body.len());
        assert_eq!(client.get(channel).await?, envelope);
        server.await??;
        Ok(())
    }
}
