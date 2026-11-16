//! Public signed locators only. Local paths/claim/IPC never enter this schema.
use super::{artifact, consent, model::Setup};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v2 as wire;
use serde::{Deserialize, Serialize};

pub(super) const CAP: usize = 48 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    account: String,
    device: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    version: u8,
    kind: String,
    profile_mode: String,
    invitation: String,
    contexts: [String; 2],
    sender: Pins,
    recipient: Pins,
    locators: [String; 2],
}
pub(super) fn peers(setup: &Setup) -> Result<[wire::RootVerifiedCredential; 2]> {
    let info = setup.record.public.as_ref().context("Account")?;
    let own = Setup::identity(info)?;
    let peer = setup.record.peer.as_ref().context("Peer")?;
    [own.credential.as_str(), peer.credential.as_str()]
        .map(|s| {
            wire::ParsedCredentialBundle::parse(&artifact::unhex(s, wire::MAX_AUTH)?)
                .and_then(|p| p.verify_root())
                .map_err(anyhow::Error::from)
        })
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Credential pair"))
}
impl Envelope {
    pub fn decode(raw: &[u8]) -> Result<Self> {
        ensure!(raw.len() <= CAP, "Locator exchange size");
        let out: Self = serde_json::from_slice(raw)
            .map_err(|_| anyhow::anyhow!("Invalid public locator exchange"))?;
        out.shape()?;
        Ok(out)
    }
    fn shape(&self) -> Result<()> {
        ensure!(
            self.version == 1
                && self.kind == "locators"
                && self.profile_mode == "ordered-dialog-v2",
            "Unsupported locator exchange"
        );
        for s in [
            &self.invitation,
            &self.contexts[0],
            &self.contexts[1],
            &self.sender.account,
            &self.sender.device,
            &self.recipient.account,
            &self.recipient.device,
        ] {
            artifact::canonical_id(s)?;
        }
        ensure!(
            self.contexts[0] != self.contexts[1] && self.locators.iter().all(|s| s.len() <= 16384),
            "Locator slots/field size"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= CAP,
            "Locator aggregate size"
        );
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.shape()?;
        Ok(serde_json::to_vec(self)?)
    }
    pub fn create(setup: &Setup, raw: [Vec<u8>; 2]) -> Result<Self> {
        let info = setup.record.public.as_ref().context("Account")?;
        let peer = setup.record.peer.as_ref().context("Peer")?;
        let out = Self {
            version: 1,
            kind: "locators".into(),
            profile_mode: "ordered-dialog-v2".into(),
            invitation: setup.record.invitation.clone(),
            contexts: setup.record.contexts.clone(),
            sender: Pins {
                account: info.account_id.to_string(),
                device: info.device_id.to_string(),
            },
            recipient: Pins {
                account: peer.account.to_string(),
                device: peer.device.to_string(),
            },
            locators: raw.map(|b| artifact::hex(&b)),
        };
        out.validate(setup, false)?;
        Ok(out)
    }
    pub fn validate(&self, setup: &Setup, incoming: bool) -> Result<[Vec<u8>; 2]> {
        self.shape()?;
        let info = setup.record.public.as_ref().context("Account")?;
        let peer = setup.record.peer.as_ref().context("Peer")?;
        let own = Pins {
            account: info.account_id.to_string(),
            device: info.device_id.to_string(),
        };
        let peer = Pins {
            account: peer.account.to_string(),
            device: peer.device.to_string(),
        };
        let (sender, recipient) = if incoming { (peer, own) } else { (own, peer) };
        ensure!(
            self.invitation == setup.record.invitation
                && self.contexts == setup.record.contexts
                && self.sender == sender
                && self.recipient == recipient,
            "Locator sender/recipient/invitation/context mismatch"
        );
        let credentials = peers(setup)?;
        let wall = consent::now()?;
        let raw = [
            artifact::unhex(&self.locators[0], 8192)?,
            artifact::unhex(&self.locators[1], 8192)?,
        ];
        let mut endpoint = None;
        let lan = super::connection::lan_settings(setup);
        let mut lan_socket = None;
        for (slot, blob) in raw.iter().enumerate() {
            let locator = wire::ParsedRuntimeLocator2::parse(blob)?;
            let (own, other) = if incoming {
                (&credentials[1], &credentials[0])
            } else {
                (&credentials[0], &credentials[1])
            };
            locator.verify(
                kilogram_runtime_ipc::ordered2::id(&self.contexts[slot])?,
                own,
                other,
            )?;
            let f = locator.fields();
            ensure!(
                f.not_before <= wall && wall < f.expires,
                "Locator expired or not yet valid"
            );
            if let Some(lan) = lan {
                let expected = if incoming { lan.peer } else { lan.local };
                let socket = locator.direct_lan_socket(expected)?;
                ensure!(
                    lan_socket.is_none_or(|prior| prior == socket),
                    "LAN locator socket pair mismatch"
                );
                lan_socket = Some(socket);
            } else {
                ensure!(
                    f.relay.is_none()
                        && !f.ips.is_empty()
                        && f.ips.iter().all(|a| a.ip().is_loopback()),
                    "Locator outside local pilot route"
                );
            }
            ensure!(
                endpoint.is_none_or(|e| e == f.endpoint),
                "Locator endpoint pair mismatch"
            );
            endpoint = Some(f.endpoint);
        }
        Ok(raw)
    }
}
