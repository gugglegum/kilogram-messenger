use std::{net::IpAddr, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use kilogram_mailbox::{
    MAX_MAILBOX_WIRE_RESPONSE_BYTES, MailboxDeleteRequest, MailboxDeleteResponse,
    MailboxListRequest, MailboxListResponse, MailboxPutRequest, MailboxPutResponse,
    MailboxStoreKey,
};
use reqwest::{Client, Response, StatusCode, Url, header::CONTENT_TYPE, redirect::Policy};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SMALL_RESPONSE_LIMIT: usize = 4 * 1024;

pub const MAILBOX_HTTP_CONTENT_TYPE: &str = "application/vnd.kilogram.blind-mailbox-v1";

#[derive(Clone)]
pub struct MailboxHttpClient {
    client: Client,
    base_url: Url,
    expected_store_key: MailboxStoreKey,
}

impl MailboxHttpClient {
    pub fn new(base_url: &str, expected_store_key: MailboxStoreKey) -> Result<Self> {
        let mut base_url = Url::parse(base_url).context("parse mailbox service URL")?;
        ensure!(
            base_url.username().is_empty()
                && base_url.password().is_none()
                && base_url.query().is_none()
                && base_url.fragment().is_none(),
            "mailbox service URL must not contain credentials, query, or fragment"
        );
        match base_url.scheme() {
            "https" => {}
            "http" => {
                let host = base_url
                    .host_str()
                    .context("mailbox HTTP URL has no host")?;
                let ip: IpAddr = host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse()
                    .context("plain HTTP mailbox is allowed only for a loopback IP")?;
                ensure!(
                    ip.is_loopback(),
                    "plain HTTP mailbox is allowed only for a loopback IP"
                );
            }
            _ => bail!("mailbox service URL must use HTTPS"),
        }
        ensure!(
            base_url.path_segments().is_some(),
            "mailbox service URL cannot be a base"
        );
        let normalized_path = format!("{}/", base_url.path().trim_end_matches('/'));
        base_url.set_path(&normalized_path);
        let client = Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .context("build blind mailbox HTTPS client")?;
        Ok(Self {
            client,
            base_url,
            expected_store_key,
        })
    }

    pub async fn put(&self, request: &MailboxPutRequest) -> Result<MailboxPutResponse> {
        let body = request.encode()?;
        let response = self
            .client
            .put(self.item_url(request.mailbox_id(), request.item_id())?)
            .header(CONTENT_TYPE, MAILBOX_HTTP_CONTENT_TYPE)
            .header("accept", MAILBOX_HTTP_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .context("upload blind mailbox item")?;
        let body = read_protocol_response(response, SMALL_RESPONSE_LIMIT).await?;
        MailboxPutResponse::decode_and_verify(&body, request, self.expected_store_key)
            .context("verify blind mailbox put response")
    }

    pub async fn list(&self, request: &MailboxListRequest) -> Result<MailboxListResponse> {
        let body = request.encode()?;
        let response = self
            .client
            .post(self.list_url(request.mailbox_id())?)
            .header(CONTENT_TYPE, MAILBOX_HTTP_CONTENT_TYPE)
            .header("accept", MAILBOX_HTTP_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .context("list blind mailbox items")?;
        let body = read_protocol_response(response, MAX_MAILBOX_WIRE_RESPONSE_BYTES).await?;
        MailboxListResponse::decode_and_verify(&body, request, self.expected_store_key)
            .context("verify blind mailbox list response")
    }

    pub async fn delete(&self, request: &MailboxDeleteRequest) -> Result<MailboxDeleteResponse> {
        let body = request.encode()?;
        let response = self
            .client
            .delete(self.item_url(request.mailbox_id(), request.item_id())?)
            .header(CONTENT_TYPE, MAILBOX_HTTP_CONTENT_TYPE)
            .header("accept", MAILBOX_HTTP_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .context("delete blind mailbox item")?;
        let body = read_protocol_response(response, SMALL_RESPONSE_LIMIT).await?;
        MailboxDeleteResponse::decode_and_verify(&body, request, self.expected_store_key)
            .context("verify blind mailbox delete response")
    }

    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    pub fn expected_store_key(&self) -> MailboxStoreKey {
        self.expected_store_key
    }

    fn list_url(&self, mailbox_id: kilogram_mailbox::MailboxId) -> Result<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("mailbox service URL cannot be a base"))?
            .pop_if_empty()
            .extend(["v1", "mailboxes", &mailbox_id.to_string(), "list"]);
        Ok(url)
    }

    fn item_url(
        &self,
        mailbox_id: kilogram_mailbox::MailboxId,
        item_id: kilogram_mailbox::MailboxItemId,
    ) -> Result<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("mailbox service URL cannot be a base"))?
            .pop_if_empty()
            .extend([
                "v1",
                "mailboxes",
                &mailbox_id.to_string(),
                "items",
                &item_id.to_string(),
            ]);
        Ok(url)
    }
}

async fn read_protocol_response(mut response: Response, maximum: usize) -> Result<Vec<u8>> {
    ensure!(
        matches!(response.status(), StatusCode::OK | StatusCode::CREATED),
        "mailbox service returned HTTP {}",
        response.status()
    );
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .context("mailbox response has no Content-Type")?
        .to_str()
        .context("mailbox response Content-Type is invalid")?;
    ensure!(
        content_type == MAILBOX_HTTP_CONTENT_TYPE,
        "mailbox response Content-Type is unsupported"
    );
    if let Some(length) = response.content_length() {
        ensure!(length <= maximum as u64, "mailbox response is too large");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.context("read mailbox response")? {
        ensure!(
            body.len().saturating_add(chunk.len()) <= maximum,
            "mailbox response is too large"
        );
        body.extend_from_slice(&chunk);
    }
    ensure!(!body.is_empty(), "mailbox response body is empty");
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_mailbox::MailboxStoreIdentity;

    #[test]
    fn client_requires_https_except_for_numeric_loopback() {
        let key = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]).store_key();
        assert!(MailboxHttpClient::new("https://example.invalid/base", key).is_ok());
        assert!(MailboxHttpClient::new("http://127.0.0.1:8787/base", key).is_ok());
        assert!(MailboxHttpClient::new("http://[::1]:8787/base", key).is_ok());
        assert!(MailboxHttpClient::new("http://localhost:8787/base", key).is_err());
        assert!(MailboxHttpClient::new("http://192.0.2.1/base", key).is_err());
        assert!(MailboxHttpClient::new("file:///tmp/store", key).is_err());
        assert!(MailboxHttpClient::new("https://user@example.invalid/base", key).is_err());
        assert!(MailboxHttpClient::new("https://example.invalid/base?q=1", key).is_err());
    }
}
