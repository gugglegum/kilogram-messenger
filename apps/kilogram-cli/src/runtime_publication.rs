use std::{net::IpAddr, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use kilogram_crypto::{EncryptionPublicKey, SealedMessage};
use kilogram_identity::{DeviceEncryptionIdentity, DeviceId};
pub use kilogram_publication_conflict::{
    DEFAULT_TICKET_PUBLICATION_TTL_SECONDS, MAX_TICKET_PUBLICATION_BYTES,
    MAX_TICKET_PUBLICATION_TTL_SECONDS, MIN_TICKET_PUBLICATION_TTL_SECONDS,
    SignedTicketPublication, SignedTicketPublicationObservation, TicketPublicationId,
    TicketPublicationObservationId,
};
pub use kilogram_ticket_publication::{
    TicketPublicationChannelId, TicketPublicationWriteCapability, TicketPublicationWriteKey,
};
use kilogram_ticket_publication::{WRITE_KEY_HEADER, WRITE_SIGNATURE_HEADER, encode_signature};
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};

const ENVELOPE_VERSION: u8 = 1;
const RECIPIENT_SELECTOR_DOMAIN: &[u8] = b"kilogram:ticket-publication-recipient:v1\0";
const ENVELOPE_HPKE_INFO: &[u8] = b"kilogram:ticket-publication-envelope:v1";
pub const MAX_TICKET_PUBLICATION_RECIPIENTS: usize = 64;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

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

    pub async fn put(
        &self,
        envelope: &EncryptedTicketPublication,
        capability: &TicketPublicationWriteCapability,
    ) -> Result<usize> {
        let body = envelope.encode()?;
        let url = self.record_url(envelope.channel_id())?;
        ensure!(
            capability.write_key().channel_id() == envelope.channel_id(),
            "ticket publication write capability belongs to another channel"
        );
        let signature = capability.authorize(
            envelope.channel_id(),
            envelope.publication_generation(),
            &body,
        );
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
            .header(WRITE_KEY_HEADER, capability.write_key().to_string())
            .header(WRITE_SIGNATURE_HEADER, encode_signature(&signature))
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

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_crypto::DeviceEncryptionIdentity;
    use kilogram_identity::{AccountRootState, DeviceIdentity};
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
        let capability = TicketPublicationWriteCapability::derive(
            publisher.secret_bytes(),
            recipient_root.account_id().as_bytes(),
        );
        let channel = capability.write_key().channel_id();
        assert_ne!(
            channel,
            TicketPublicationWriteCapability::derive(
                DeviceIdentity::generate()?.secret_bytes(),
                recipient_root.account_id().as_bytes(),
            )
            .write_key()
            .channel_id()
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
        let capability = TicketPublicationWriteCapability::derive(
            publisher.secret_bytes(),
            recipient_root.account_id().as_bytes(),
        );
        let channel = capability.write_key().channel_id();
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
        assert_eq!(
            client.put(&envelope, &capability).await?,
            expected_body.len()
        );
        assert_eq!(client.get(channel).await?, envelope);
        server.await??;
        Ok(())
    }
}
