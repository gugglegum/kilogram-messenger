//! One optional owned transport child; node Native streams remain independent.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
pub(super) fn url(raw: &str) -> Result<String> {
    kilogram_protocol::ordered_dialog_v2::canonical_runtime_relay(raw)?;
    let port = raw
        .strip_prefix("http://127.0.0.1:")
        .and_then(|v| v.strip_suffix('/'))
        .context("exact owned loopback relay URL")?;
    let number = port.parse::<u16>()?;
    ensure!(
        number > 0 && number.to_string() == port,
        "canonical nonzero owned relay port"
    );
    Ok(raw.into())
}
pub(super) fn ready_until(start: Instant, whole: Instant) -> Result<Instant> {
    ensure!(
        start < whole,
        "relay startup retains original remaining deadline"
    );
    Ok((start + Duration::from_secs(15)).min(whole))
}
pub(super) fn unique(present: bool) -> Result<()> {
    ensure!(!present, "one original owned relay");
    Ok(())
}
pub(super) fn image() -> Result<Value> {
    let path =
        workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe");
    let mut file = std::fs::File::open(&path)?;
    ensure!(
        file.metadata()?.len() == 10460160,
        "exact accepted relay image size"
    );
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0; 65536];
    loop {
        s53_deadline::check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        ensure!(bytes <= 10460160, "bounded relay image");
        digest.update(&buffer[..n]);
    }
    let hash = state::hex(&digest.finalize());
    ensure!(
        bytes == 10460160
            && hash == "cf9d029cebf1d04745c90a9712401d70e823ecf4af5a082fe7269649df358e5e",
        "exact accepted unchanged relay image hash"
    );
    Ok(json!({"path":path,"bytes":bytes,"sha256":hash}))
}
pub(super) struct Relay {
    pub child: OwnedChild,
    pub ready: PathBuf,
    pub stop: PathBuf,
    pub log: PathBuf,
    pub url: Option<String>,
    pub image: Value,
    cleanup: PathBuf,
}
impl Relay {
    pub fn facts(&self) -> Value {
        json!({"pid":self.child.0.id(),"url":self.url,"ready":self.ready,"stop":self.stop,"stderr":self.log,"image":self.image})
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        // No deadline check here: expiration must not skip known-handle cleanup.
        let pid = self.child.0.id();
        let mut kill_error = None;
        let outcome = match self.child.0.try_wait() {
            Ok(Some(exit)) => Ok(exit),
            _ => {
                if let Err(error) = self.child.0.kill() {
                    kill_error = Some(error.to_string());
                }
                self.child.0.wait()
            }
        };
        let row = match outcome {
            Ok(exit) => {
                json!({"classification":"owned relay waited outcome; no node packet accounting","pid":pid,"waited":true,"success":exit.success(),"code":exit.code(),"kill_error":kill_error,"url":self.url})
            }
            Err(error) => {
                json!({"classification":"unresolved owned relay cleanup","pid":pid,"waited":false,"error":error.to_string(),"kill_error":kill_error,"url":self.url})
            }
        };
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.cleanup)
            && let Ok(raw) = serde_json::to_vec(&row)
        {
            let _ = file.write_all(&raw);
            let _ = file.sync_all();
        }
    }
}
pub(super) fn start(s: &mut control::Controller) -> Result<Value> {
    s.stopped_reconciled()?;
    unique(s.relay.is_some())?;
    s.check()?;
    let image = image()?;
    let until = ready_until(Instant::now(), s.until)?;
    let _phase = s53_deadline::Phase::until(until)?;
    let base = s.origin.pair.base.join("p10-relay");
    std::fs::create_dir(&base)?;
    let ready = base.join("relay.ready");
    let stop = base.join("relay.stop");
    let log = base.join("relay.stderr.log");
    ensure!(
        !ready.exists() && !stop.exists() && !ready.with_extension("pending").exists(),
        "fresh actual relay control files"
    );
    let stderr = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&log)?;
    let child = OwnedChild(
        command(
            &workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe"),
        )
        .arg(&ready)
        .arg(&stop)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?,
    );
    s.relay = Some(Relay {
        child,
        ready: ready.clone(),
        stop,
        log,
        url: None,
        image,
        cleanup: base.join("relay-cleanup.json"),
    });
    s.wait(15, |s| {
        s.relay
            .as_mut()
            .context("same actual relay")?
            .child
            .alive()?;
        Ok(ready.is_file().then_some(()))
    })?;
    let raw = kilogram_cli::ordered_dialog::admin::read_public(&ready, 2048)?;
    let value = url(std::str::from_utf8(&raw)?)?;
    let relay = s
        .relay
        .as_mut()
        .context("same owned relay after readiness")?;
    relay.child.alive()?;
    relay.url = Some(value);
    Ok(relay.facts())
}
