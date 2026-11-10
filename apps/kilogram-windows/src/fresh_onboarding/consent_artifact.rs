use super::artifact::{self, Identity, Recipient};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub(super) const BLOB_CAP: usize = 96 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    Proposal,
    Final,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub version: u8,
    pub kind: Kind,
    pub mode: String,
    pub invitation: String,
    pub contexts: [String; 2],
    pub sender: Identity,
    pub recipient: Recipient,
    pub blobs: [String; 2],
}
impl Envelope {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.mode == "ordered-dialog-v2",
            "Unsupported consent exchange"
        );
        artifact::canonical_id(&self.invitation)?;
        for c in &self.contexts {
            artifact::canonical_id(c)?;
        }
        ensure!(
            self.contexts[0] != self.contexts[1],
            "Duplicate consent contexts"
        );
        // Check every encoded allocation budget before decoding any blob/identity.
        ensure!(
            self.sender.credential.len() <= 32 * 1024
                && self.sender.authority.len() <= 32 * 1024
                && self.blobs.iter().all(|b| b.len() <= 2 * BLOB_CAP),
            "Consent field byte cap"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= artifact::FILE_CAP,
            "Aggregate consent envelope cap"
        );
        self.sender.validate()?;
        ensure!(
            self.recipient.account != self.sender.account
                && self.recipient.device != self.sender.device,
            "Distinct consent recipient required"
        );
        Ok(())
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        ensure!(raw.len() <= artifact::FILE_CAP, "Consent envelope cap");
        let out: Self = serde_json::from_slice(raw)
            .map_err(|_| anyhow::anyhow!("Invalid proposal/final exchange file"))?;
        out.validate()?;
        Ok(out)
    }
    pub fn payloads(&self) -> Result<[Vec<u8>; 2]> {
        self.validate()?;
        Ok([
            artifact::unhex(&self.blobs[0], BLOB_CAP)?,
            artifact::unhex(&self.blobs[1], BLOB_CAP)?,
        ])
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }
}
