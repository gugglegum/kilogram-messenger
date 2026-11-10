//! Separate one-shot output for direct Runtime6 creation; no legacy prekey paths.
use super::*;
use std::path::Component;

pub const ORDERED2_BOOTSTRAP_VERSION: u8 = 1;
pub const ORDERED2_BOOTSTRAP_MODE: &str = "ordered-dialog-v2";
pub const ORDERED2_RECOVERY_SCOPE: &str = "root-key-only-authority-history-required";

pub fn parse_ordered2_id(text: &str) -> Result<[u8; 32]> {
    ensure!(
        text.len() == 64 && text.is_ascii(),
        "expected 64 hexadecimal characters"
    );
    let mut id = [0; 32];
    for (out, pair) in id.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *out = u8::from_str_radix(std::str::from_utf8(pair)?, 16)
            .map_err(|_| anyhow::anyhow!("invalid hexadecimal ID"))?;
    }
    Ok(id)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ordered2BootstrapPublic {
    pub account_id: AccountId,
    pub device_id: DeviceId,
    pub claim: String,
    pub contexts: [String; 2],
    pub workspace_dir: PathBuf,
    pub account_root_dir: PathBuf,
    pub state_dir: PathBuf,
    pub credential_file: PathBuf,
    pub certificate_file: PathBuf,
    pub authority_file: PathBuf,
    pub device_list_file: PathBuf,
    pub receipt_file: PathBuf,
    pub root_key_protection: String,
}
impl Ordered2BootstrapPublic {
    pub fn validate(&self) -> Result<()> {
        let claim = parse_ordered2_id(&self.claim)?;
        let contexts = [
            parse_ordered2_id(&self.contexts[0])?,
            parse_ordered2_id(&self.contexts[1])?,
        ];
        ensure!(
            claim != [0; 32] && contexts[0] != contexts[1],
            "invalid owner/context claim"
        );
        for id in [&self.claim, &self.contexts[0], &self.contexts[1]] {
            ensure!(
                id.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "canonical lowercase ID"
            );
        }
        ensure!(
            self.workspace_dir.is_absolute()
                && self.workspace_dir.as_os_str().len() <= 4096
                && !self
                    .workspace_dir
                    .components()
                    .any(|c| matches!(c, Component::CurDir | Component::ParentDir)),
            "absolute bounded workspace path"
        );
        for (path, suffix) in [
            (&self.account_root_dir, "account-root"),
            (&self.state_dir, "device"),
            (&self.credential_file, "public/credential.bin"),
            (&self.certificate_file, "public/device-certificate.cert"),
            (&self.authority_file, "public/authority.snapshot"),
            (&self.device_list_file, "public/device-list.snapshot"),
            (&self.receipt_file, "bootstrap-ready.json"),
        ] {
            ensure!(
                path == &self.workspace_dir.join(suffix),
                "bootstrap output path role mismatch"
            );
        }
        ensure!(
            matches!(
                self.root_key_protection.as_str(),
                "windows-dpapi-current-user" | "plaintext-development"
            ),
            "unknown Root key protection"
        );
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Ordered2BootstrapOutput {
    #[zeroize(skip)]
    version: u8,
    profile_mode: String,
    #[zeroize(skip)]
    public: Ordered2BootstrapPublic,
    recovery_phrase: String,
    recovery_scope: String,
}
impl fmt::Debug for Ordered2BootstrapOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ordered2BootstrapOutput")
            .field("version", &self.version)
            .field("profile_mode", &self.profile_mode)
            .field("public", &self.public)
            .field("recovery_phrase", &"[REDACTED]")
            .field("recovery_scope", &self.recovery_scope)
            .finish()
    }
}
impl Ordered2BootstrapOutput {
    pub fn new(public: Ordered2BootstrapPublic, recovery_phrase: String) -> Result<Self> {
        let out = Self {
            version: ORDERED2_BOOTSTRAP_VERSION,
            profile_mode: ORDERED2_BOOTSTRAP_MODE.into(),
            public,
            recovery_phrase,
            recovery_scope: ORDERED2_RECOVERY_SCOPE.into(),
        };
        out.validate()?;
        Ok(out)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == ORDERED2_BOOTSTRAP_VERSION
                && self.profile_mode == ORDERED2_BOOTSTRAP_MODE
                && self.recovery_scope == ORDERED2_RECOVERY_SCOPE,
            "unsupported Ordered2 bootstrap result"
        );
        self.public.validate()?;
        let phrase = AccountRecoveryPhrase::parse(&self.recovery_phrase)
            .map_err(|_| anyhow::anyhow!("invalid bootstrap recovery phrase"))?;
        ensure!(
            phrase.account_id()? == self.public.account_id,
            "recovery phrase/account mismatch"
        );
        Ok(())
    }
    pub fn public(&self) -> &Ordered2BootstrapPublic {
        &self.public
    }
    pub fn recovery_phrase(&self) -> &str {
        &self.recovery_phrase
    }
    pub fn take_recovery_phrase(&mut self) -> Zeroizing<String> {
        Zeroizing::new(std::mem::take(&mut self.recovery_phrase))
    }
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        let raw = Zeroizing::new(serde_json::to_vec(self)?);
        ensure!(
            raw.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
            "bootstrap output byte cap"
        );
        Ok(raw)
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        ensure!(
            raw.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
            "bootstrap output byte cap"
        );
        let out: Self = serde_json::from_slice(raw)
            .map_err(|_| anyhow::anyhow!("invalid Ordered2 bootstrap result JSON"))?;
        out.validate()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cs2_output_roundtrip_redaction_and_shape() -> Result<()> {
        let root = tempfile::tempdir()?;
        let workspace = root.path().join("new-workspace");
        let phrase = ["abandon"; 23]
            .into_iter()
            .chain(["art"])
            .collect::<Vec<_>>()
            .join(" ");
        let account = AccountRecoveryPhrase::parse(&phrase)?.account_id()?;
        let info = Ordered2BootstrapPublic {
            account_id: account,
            device_id: DeviceId::from_bytes([2; 32]),
            claim: "01".repeat(32),
            contexts: ["02".repeat(32), "03".repeat(32)],
            account_root_dir: workspace.join("account-root"),
            state_dir: workspace.join("device"),
            credential_file: workspace.join("public/credential.bin"),
            certificate_file: workspace.join("public/device-certificate.cert"),
            authority_file: workspace.join("public/authority.snapshot"),
            device_list_file: workspace.join("public/device-list.snapshot"),
            receipt_file: workspace.join("bootstrap-ready.json"),
            workspace_dir: workspace,
            root_key_protection: "plaintext-development".into(),
        };
        let output = Ordered2BootstrapOutput::new(info, phrase)?;
        let bytes = output.encode()?;
        ensure!(
            Ordered2BootstrapOutput::decode(&bytes)?.public() == output.public(),
            "structured roundtrip"
        );
        ensure!(
            DesktopBootstrapOutput::decode(&bytes).is_err(),
            "fresh result cannot masquerade as legacy desktop output"
        );
        ensure!(
            !format!("{output:?}").contains(output.recovery_phrase()),
            "Debug leaked phrase"
        );
        for case in 0..6 {
            let mut v: serde_json::Value = serde_json::from_slice(&bytes)?;
            match case {
                0 => v["version"] = 2.into(),
                1 => v["profile_mode"] = "legacy".into(),
                2 => v["public"]["contexts"][1] = v["public"]["contexts"][0].clone(),
                3 => v["public"]["credential_file"] = "relative".into(),
                4 => v["recovery_phrase"] = "invalid".into(),
                _ => v["extra"] = true.into(),
            };
            let raw = Zeroizing::new(serde_json::to_vec(&v)?);
            // Test data is a fixed synthetic phrase; never print serialized values.
            ensure!(
                Ordered2BootstrapOutput::decode(&raw).is_err(),
                "invalid output case {case}"
            );
        }
        ensure!(
            Ordered2BootstrapOutput::decode(&vec![0; MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES + 1])
                .is_err(),
            "oversize output"
        );
        Ok(())
    }
}
