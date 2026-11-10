//! Public-only invite/reply. This file supplies no authenticated human consent.
use anyhow::{Result, ensure};
use kilogram_bootstrap_contract::parse_ordered2_id;
use kilogram_identity::{AccountAuthoritySnapshot, AccountId, DeviceId};
use kilogram_protocol::ordered_dialog_v2 as wire;
use serde::{Deserialize, Serialize};

pub(super) const FILE_CAP: usize = 512 * 1024;
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn id() -> Result<String> {
    let mut b = [0; 32];
    getrandom::fill(&mut b)?;
    Ok(hex(&b))
}
pub(super) fn canonical_id(s: &str) -> Result<()> {
    let _ = parse_ordered2_id(s)?;
    ensure!(
        s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "Noncanonical public ID"
    );
    Ok(())
}
pub(super) fn unhex(s: &str, cap: usize) -> Result<Vec<u8>> {
    ensure!(
        s.len() <= 2 * cap && s.len().is_multiple_of(2) && s.is_ascii(),
        "Public artifact byte cap"
    );
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            ensure!(
                p.iter()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c)),
                "Noncanonical artifact bytes"
            );
            Ok(u8::from_str_radix(std::str::from_utf8(p)?, 16)?)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub account: AccountId,
    pub device: DeviceId,
    pub credential: String,
    pub authority: String,
}
impl Identity {
    pub fn new(credential: &[u8], authority: &[u8]) -> Result<Self> {
        let c = wire::ParsedCredentialBundle::parse(credential)?.verify_root()?;
        let out = Self {
            account: c.certificate().account_id(),
            device: c.certificate().device_id(),
            credential: hex(credential),
            authority: hex(authority),
        };
        out.validate()?;
        Ok(out)
    }
    pub fn validate(&self) -> Result<()> {
        let raw = unhex(&self.credential, wire::MAX_AUTH)?;
        let c = wire::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
        let floor_raw = unhex(&self.authority, wire::MAX_AUTH)?;
        let floor = AccountAuthoritySnapshot::decode_and_verify(&floor_raw)?;
        ensure!(
            c.certificate().account_id() == self.account
                && c.certificate().device_id() == self.device
                && floor.account_id() == self.account
                && floor.encode()? == floor_raw
                && c.authority().revision() >= floor.revision()
                && (c.authority().revision() != floor.revision()
                    || c.authority().encode()? == floor_raw),
            "Public identity or authority binding mismatch"
        );
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    Invite,
    Reply,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipient {
    pub account: AccountId,
    pub device: DeviceId,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Artifact {
    pub version: u8,
    pub kind: Kind,
    pub mode: String,
    pub invitation: String,
    pub contexts: [String; 2],
    pub sender: Identity,
    pub recipient: Option<Recipient>,
}
impl Artifact {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.mode == "ordered-dialog-v2",
            "Unsupported invitation version or mode"
        );
        canonical_id(&self.invitation)?;
        for c in &self.contexts {
            canonical_id(c)?;
        }
        ensure!(
            self.contexts[0] != self.contexts[1],
            "Duplicate invitation contexts"
        );
        self.sender.validate()?;
        match self.kind {
            Kind::Invite => ensure!(
                self.recipient.is_none(),
                "An invite cannot select an unknown recipient"
            ),
            Kind::Reply => {
                let peer = self
                    .recipient
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Reply recipient is required"))?;
                ensure!(
                    peer.account != self.sender.account && peer.device != self.sender.device,
                    "Reply must name a distinct recipient"
                );
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= FILE_CAP, "Invitation too large");
        let out: Self = serde_json::from_slice(bytes)
            .map_err(|_| anyhow::anyhow!("Invalid public invitation file"))?;
        out.validate()?;
        Ok(out)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(bytes.len() <= FILE_CAP, "Invitation too large");
        Ok(bytes)
    }
}
