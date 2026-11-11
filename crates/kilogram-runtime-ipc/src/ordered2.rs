//! Small public file/IPC boundary for the fresh two-context desktop pilot.
use super::{RuntimeIpcCommand, RuntimeIpcDescriptor, RuntimeIpcResponse};
use anyhow::{Result, ensure};
use kilogram_identity::{AccountId, DeviceId};
use kilogram_protocol::ordered_dialog_v2::{self as wire, Id, Value, a, b, u};
use serde::{Deserialize, Deserializer, Serialize, de::IgnoredAny};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
pub const PROFILE_CAP: usize = 16 * 1024;
pub const LOCATOR_CAP: usize = 8192;
pub const STATUS_CAP: usize = 48 * 1024;

// A missing field remains None; an explicitly supplied null is rejected by T.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileRoutePolicy2 {
    Auto,
    DirectOnly,
    RelayOnly,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRoute2 {
    pub policy: ProfileRoutePolicy2,
    pub bind: Option<SocketAddr>,
    pub relay: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile2Document {
    pub version: u8,
    pub profile_mode: String,
    pub state_dir: PathBuf,
    pub account: String,
    pub claim: String,
    pub device: String,
    pub contexts: [String; 2],
    pub enabled: Vec<u8>,
    pub ipc_file: PathBuf,
    pub locator_directory: PathBuf,
    pub route: ProfileRoute2,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub logical_quota: Option<[u64; 2]>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub physical_quota: Option<u64>,
}
pub fn id(text: &str) -> Result<Id> {
    ensure!(
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid canonical ID"
    );
    let mut out = [0; 32];
    for (byte, pair) in out.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(out)
}
impl Profile2Document {
    pub fn decode(raw: &[u8]) -> Result<Self> {
        ensure!(raw.len() <= PROFILE_CAP, "profile size");
        let doc: Self = serde_json::from_slice(raw)?;
        ensure!(
            doc.version == 2 && doc.profile_mode == "ordered-dialog-v2",
            "unsupported profile"
        );
        for pin in [
            &doc.account,
            &doc.claim,
            &doc.device,
            &doc.contexts[0],
            &doc.contexts[1],
        ] {
            id(pin)?;
        }
        ensure!(doc.contexts[0] != doc.contexts[1], "duplicate contexts");
        ensure!(
            !doc.enabled.is_empty()
                && doc.enabled.len() <= 2
                && doc.enabled.iter().all(|c| *c < 2)
                && (doc.enabled.len() == 1 || doc.enabled[0] != doc.enabled[1]),
            "enabled slots"
        );
        Ok(doc)
    }
}

pub fn decode_reply(opcode: u64, response: RuntimeIpcResponse) -> Result<Value> {
    let RuntimeIpcResponse::OrderedDialog { response } = response else {
        anyhow::bail!("unexpected runtime response");
    };
    let v = wire::decode(&response, STATUS_CAP)?;
    let f = v.fields(3)?;
    ensure!(
        f[0].uint()? == 2 && f[1].uint()? == opcode,
        "runtime reply version/opcode"
    );
    Ok(f[2].clone())
}

#[derive(Clone)]
pub struct Client2 {
    descriptor: RuntimeIpcDescriptor,
    expected: (AccountId, DeviceId),
    contexts: [Id; 2],
}
impl std::fmt::Debug for Client2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Client2([local authenticated descriptor])")
    }
}
impl Client2 {
    pub fn new(
        descriptor: RuntimeIpcDescriptor,
        account: AccountId,
        device: DeviceId,
        contexts: [Id; 2],
    ) -> Result<Self> {
        descriptor.verify()?;
        ensure!(
            descriptor.account_id() == account && descriptor.device_id() == device,
            "runtime identity mismatch"
        );
        ensure!(contexts[0] != contexts[1], "duplicate client contexts");
        Ok(Self {
            descriptor,
            expected: (account, device),
            contexts,
        })
    }
    pub fn descriptor(&self) -> &RuntimeIpcDescriptor {
        &self.descriptor
    }
    async fn request(&self, opcode: u64, fields: Vec<Value>) -> Result<Value> {
        let mut request = vec![u(2), u(opcode)];
        request.extend(fields);
        let request = wire::encode(&a(request), 32 * 1024)?;
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            super::call_with_descriptor(
                &self.descriptor,
                self.expected,
                RuntimeIpcCommand::OrderedDialog { request },
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("runtime request timed out"))??;
        decode_reply(opcode, response)
    }
    pub async fn status(&self) -> Result<Status2> {
        let v = self.request(1, vec![]).await?;
        let status = Status2::decode(v.bytes()?)?;
        for c in &status.contexts {
            if let Context2::Installed(c) = c {
                ensure!(
                    id(&c.context)? == self.contexts[usize::from(c.slot)],
                    "status context binding"
                );
            }
        }
        Ok(status)
    }
    pub async fn export_locator(&self, slot: u8) -> Result<Vec<u8>> {
        ensure!(slot < 2, "context slot");
        let v = self.request(6, vec![u(u64::from(slot))]).await?;
        let raw = v.bytes()?;
        ensure!(raw.len() <= LOCATOR_CAP, "locator size");
        ensure!(
            wire::ParsedRuntimeLocator2::parse(raw)?.fields().c == self.contexts[usize::from(slot)],
            "export context binding"
        );
        Ok(raw.to_vec())
    }
    pub async fn import_locator(&self, slot: u8, raw: &[u8]) -> Result<()> {
        ensure!(slot < 2 && raw.len() <= LOCATOR_CAP, "locator slot/size");
        ensure!(
            wire::ParsedRuntimeLocator2::parse(raw)?.fields().c == self.contexts[usize::from(slot)],
            "import context binding"
        );
        let v = self.request(5, vec![u(u64::from(slot)), b(raw)]).await?;
        v.fields(0)?;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let v = tokio::time::timeout(
            Duration::from_secs(5),
            super::call_with_descriptor(
                &self.descriptor,
                self.expected,
                RuntimeIpcCommand::Shutdown,
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("shutdown timed out"))??;
        ensure!(
            matches!(v, RuntimeIpcResponse::ShutdownAccepted),
            "shutdown not acknowledged"
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint2 {
    pub id: String,
    pub ips: Vec<SocketAddr>,
    pub relays: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Details2 {
    #[serde(deserialize_with = "nullable")]
    pub current_consent: Option<String>,
    #[serde(rename = "native")]
    _native: IgnoredAny,
    #[serde(rename = "native_hash")]
    _native_hash: IgnoredAny,
    #[serde(rename = "offer")]
    _offer: IgnoredAny,
    #[serde(rename = "grant")]
    _grant: IgnoredAny,
    #[serde(rename = "readies")]
    _readies: IgnoredAny,
    #[serde(rename = "selected_hash")]
    _selected_hash: IgnoredAny,
    #[serde(rename = "policy_generation")]
    _policy_generation: IgnoredAny,
    #[serde(rename = "wall_high_water")]
    _wall_high_water: IgnoredAny,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed2 {
    pub slot: u8,
    pub context: String,
    pub role: u8,
    pub phase: u8,
    pub active: bool,
    #[serde(deserialize_with = "nullable")]
    pub d: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub e: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub sid: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub q: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub blocked: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub policy_error: Option<String>,
    pub own_locator_revision: u64,
    pub peer_locator_revision: u64,
    pub publication_ready: bool,
    pub file_exported: bool,
    pub live_admitted: bool,
    #[serde(deserialize_with = "nullable")]
    pub lease_expires: Option<u64>,
    pub details: Details2,
    #[serde(rename = "native_generation")]
    _native_generation: IgnoredAny,
    #[serde(rename = "counts")]
    _counts: IgnoredAny,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Missing2 {
    pub slot: u8,
    installed: bool,
    active: bool,
    publication_ready: bool,
    #[serde(rename = "counts")]
    _counts: IgnoredAny,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Context2 {
    Installed(Box<Installed2>),
    Missing(Missing2),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status2 {
    pub version: u8,
    pub runtime_pid: u32,
    pub runtime_instance: String,
    pub contexts: Vec<Context2>,
    #[serde(deserialize_with = "nullable")]
    pub endpoint: Option<Endpoint2>,
    pub route: ProfileRoute2,
    pub root_generation: u64,
    #[serde(rename = "stats")]
    _stats: IgnoredAny,
    #[serde(rename = "window")]
    _window: IgnoredAny,
    #[serde(rename = "resources")]
    _resources: IgnoredAny,
    #[serde(rename = "backend")]
    _backend: IgnoredAny,
}
impl Status2 {
    pub fn decode(raw: &[u8]) -> Result<Self> {
        ensure!(raw.len() <= STATUS_CAP, "status size");
        let status: Self = serde_json::from_slice(raw)?;
        ensure!(
            status.version == 2 && status.runtime_pid > 0 && status.contexts.len() == 2,
            "status scope"
        );
        id(&status.runtime_instance)?;
        if let Some(e) = &status.endpoint {
            id(&e.id)?;
            ensure!(e.ips.len() <= 16 && e.relays.len() <= 1, "endpoint size");
        }
        let mut seen = [false; 2];
        for (index, c) in status.contexts.iter().enumerate() {
            let slot = match c {
                Context2::Installed(c) => {
                    id(&c.context)?;
                    ensure!(c.role < 2 && c.phase <= 3, "status role/phase");
                    for pin in [&c.d, &c.e, &c.q, &c.details.current_consent]
                        .into_iter()
                        .flatten()
                    {
                        id(pin)?;
                    }
                    ensure!(
                        c.sid
                            .as_ref()
                            .is_none_or(|s| !s.is_empty() && s.len() <= 256),
                        "status SID"
                    );
                    ensure!(
                        !c.active
                            || (c.phase == 3
                                && c.d.is_some()
                                && c.e.is_some()
                                && c.q.is_some()
                                && c.sid.is_some()),
                        "active shape"
                    );
                    ensure!(c.live_admitted == c.lease_expires.is_some(), "lease shape");
                    c.slot
                }
                Context2::Missing(c) => {
                    ensure!(
                        !c.installed && !c.active && !c.publication_ready,
                        "missing shape"
                    );
                    c.slot
                }
            };
            ensure!(
                slot < 2 && usize::from(slot) == index && !seen[usize::from(slot)],
                "status slots"
            );
            seen[usize::from(slot)] = true;
        }
        Ok(status)
    }
    pub fn installed(&self, slot: usize) -> Result<&Installed2> {
        self.contexts
            .iter()
            .find_map(|c| match c {
                Context2::Installed(c) if usize::from(c.slot) == slot => Some(c.as_ref()),
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("context not installed"))
    }
}

#[cfg(test)]
mod tests;
