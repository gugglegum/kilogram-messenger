use super::*;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq)]
pub enum LocatorCommand {
    Export,
    Import(PathBuf),
    Retry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorState {
    Exported,
    Imported,
    Partial,
    Unconfirmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorAttempt {
    NotAttempted,
    Unconfirmed,
    Confirmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct LocatorSlot {
    pub attempt: LocatorAttempt,
    pub retained_confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocatorImport {
    pub digest: String,
    pub retained: Option<bool>,
    pub slots: [LocatorSlot; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocatorReport {
    pub state: LocatorState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(flatten)]
    pub import: Option<LocatorImport>,
}

pub type LocatorResult = std::result::Result<LocatorReport, &'static str>;

fn admit(cancel: &AtomicBool, op: Option<&OperationControl>, report: &LocatorReport) -> bool {
    !cancel.load(Ordering::Acquire) && op.is_none_or(|op| op.admit_locator(report))
}

fn witness(op: Option<&OperationControl>, report: &LocatorReport) {
    if let Some(op) = op {
        op.locator_progress(report);
    }
}

fn locator_directory(setup: &Setup) -> Result<PathBuf> {
    let path = setup.home.join("connection");
    check_artifact_path(setup, &path)?;
    ensure!(
        fs::canonicalize(&path)? == path && path.is_dir(),
        "Missing connection directory"
    );
    Ok(path)
}

fn publish_export(
    path: &Path,
    raw: &[u8],
    cancel: &AtomicBool,
    op: Option<&OperationControl>,
    after_publish: impl FnOnce(),
) -> LocatorResult {
    let mut report = LocatorReport {
        state: LocatorState::Unconfirmed,
        path: Some(path.to_str().ok_or("locator_unavailable")?.to_owned()),
        import: None,
    };
    if !admit(cancel, op, &report) {
        return Err("cancelled");
    }
    if model::write(path, raw, false).is_ok() {
        after_publish();
        report.state = LocatorState::Exported;
        witness(op, &report);
    }
    Ok(report)
}

impl Session {
    pub(super) fn locator(
        &mut self,
        setup: &mut Setup,
        command: LocatorCommand,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
    ) -> LocatorResult {
        match command {
            LocatorCommand::Export => self.export_report(setup, cancel, op),
            LocatorCommand::Import(path) => self.import_file(setup, &path, cancel, op, |_| Ok(())),
            LocatorCommand::Retry => self.retry_locator(setup, cancel, op),
        }
    }

    fn export_report(
        &mut self,
        setup: &Setup,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
    ) -> LocatorResult {
        let (raw, path) = self
            .prepare_export(setup, cancel, op)
            .map_err(|_| "locator_unavailable")?;
        publish_export(&path, &raw, cancel, op, || {})
    }

    fn prepare_export(
        &mut self,
        setup: &Setup,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
    ) -> Result<(Vec<u8>, PathBuf)> {
        self.poll_controlled(cancel, op)?;
        let client = self.client.as_ref().context("Owned client")?;
        let first = operation::wait(&self.rt, Some(cancel), op, client.export_locator(0))??;
        let second = operation::wait(&self.rt, Some(cancel), op, client.export_locator(1))??;
        let envelope = locator_artifact::Envelope::create(setup, [first, second])?;
        let blobs = envelope.validate(setup, false)?;
        self.poll_controlled(cancel, op)?;
        let status = self.last_status.as_ref().context("Status")?;
        let endpoint = status.endpoint.as_ref().context("Endpoint")?;
        for (slot, raw) in blobs.iter().enumerate() {
            let loc = kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(raw)?;
            ensure!(
                artifact::hex(&loc.fields().endpoint) == endpoint.id
                    && loc.fields().revision == status.installed(slot)?.own_locator_revision
                    && (self.doc.route.lan_peer_ipv4.is_none() || loc.fields().ips == endpoint.ips),
                "Own locators changed; export again"
            );
        }
        let path = locator_directory(setup)?.join(format!("public-{}.json", artifact::id()?));
        check_artifact_path(setup, &path)?;
        Ok((envelope.encode()?, path))
    }

    pub(super) fn import_file(
        &mut self,
        setup: &mut Setup,
        path: &Path,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
        after_ack: impl FnMut(usize) -> Result<()>,
    ) -> LocatorResult {
        let raw = model::read(path, locator_artifact::CAP).map_err(|_| "invalid_locator_bundle")?;
        self.import_bytes(setup, &raw, false, cancel, op, after_ack)
    }

    fn retry_locator(
        &mut self,
        setup: &mut Setup,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
    ) -> LocatorResult {
        let state = setup
            .record
            .connection
            .as_ref()
            .ok_or("no_retained_locator")?;
        let name = state.incoming.as_ref().ok_or("no_retained_locator")?;
        let expected = state
            .incoming_digest
            .as_ref()
            .ok_or("no_retained_locator")?;
        if name != &format!("peer-{expected}.json") {
            return Err("retained_locator_changed");
        }
        let raw = (|| {
            let path = locator_directory(setup)?.join(name);
            check_artifact_path(setup, &path)?;
            let raw = model::read(&path, locator_artifact::CAP)?;
            ensure!(&digest(&raw) == expected, "Retained locator changed");
            Ok::<_, anyhow::Error>(raw)
        })()
        .map_err(|_| "retained_locator_changed")?;
        self.import_bytes(setup, &raw, true, cancel, op, |_| Ok(()))
    }

    fn import_bytes(
        &mut self,
        setup: &mut Setup,
        raw: &[u8],
        already_retained: bool,
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
        mut after_ack: impl FnMut(usize) -> Result<()>,
    ) -> LocatorResult {
        let envelope =
            locator_artifact::Envelope::decode(raw).map_err(|_| "invalid_locator_bundle")?;
        let blobs = envelope
            .validate(setup, true)
            .map_err(|_| "invalid_locator_bundle")?;
        self.poll_controlled(cancel, op)
            .map_err(|_| "locator_unavailable")?;
        let status = self.last_status.as_ref().ok_or("locator_unavailable")?;
        for (slot, blob) in blobs.iter().enumerate() {
            let loc = kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(blob)
                .map_err(|_| "invalid_locator_bundle")?;
            let installed = status.installed(slot).map_err(|_| "locator_unavailable")?;
            if loc.fields().revision < installed.peer_locator_revision {
                return Err("stale_locator_bundle");
            }
        }

        let hash = digest(raw);
        let name = format!("peer-{hash}.json");
        let path = locator_directory(setup)
            .map_err(|_| "locator_unavailable")?
            .join(&name);
        let exists = if already_retained {
            true
        } else {
            check_artifact_path(setup, &path).map_err(|_| "retained_locator_changed")?;
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    if model::read(&path, locator_artifact::CAP).as_deref().ok() != Some(raw) {
                        return Err("retained_locator_changed");
                    }
                    true
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(_) => return Err("retained_locator_changed"),
            }
        };
        let same = setup.record.connection.as_ref().filter(|state| {
            state.incoming_digest.as_ref() == Some(&hash) && state.incoming.as_ref() == Some(&name)
        });
        let slots = std::array::from_fn(|slot| LocatorSlot {
            attempt: LocatorAttempt::NotAttempted,
            retained_confirmed: same.is_some_and(|s| s.imports[slot] == consent::Outcome::Accepted),
        });
        let same = same.is_some();
        let mut report = LocatorReport {
            state: LocatorState::Unconfirmed,
            path: None,
            import: Some(LocatorImport {
                digest: hash.clone(),
                retained: (same && exists).then_some(true),
                slots,
            }),
        };

        if !admit(cancel, op, &report) {
            return Err("cancelled");
        }
        if !exists && model::write(&path, raw, false).is_err() {
            return Ok(report);
        }
        if change(setup, |s| {
            if !same {
                s.imports = [consent::Outcome::NotStarted; 2];
            }
            s.incoming = Some(name);
            s.incoming_digest = Some(hash);
        })
        .is_err()
        {
            return Ok(report);
        }
        report.import.as_mut().unwrap().retained = Some(true);
        witness(op, &report);

        for (slot, blob) in blobs.iter().enumerate() {
            if cancel.load(Ordering::Acquire)
                || op.is_some_and(OperationControl::stopped)
                || envelope.validate(setup, true).is_err()
            {
                return Ok(report);
            }
            if change(setup, |s| {
                if s.imports[slot] != consent::Outcome::Accepted {
                    s.imports[slot] = consent::Outcome::Started;
                }
            })
            .is_err()
            {
                return Ok(report);
            }

            let mut sending = report.clone();
            sending.import.as_mut().unwrap().slots[slot].attempt = LocatorAttempt::Unconfirmed;
            if !admit(cancel, op, &sending) {
                return Ok(report);
            }
            report = sending;
            let ack = self.import_locator_ack(slot, blob, cancel, op);
            if ack.is_err() {
                let _ = change(setup, |s| {
                    if s.imports[slot] != consent::Outcome::Accepted {
                        s.imports[slot] = consent::Outcome::Failed;
                    }
                    s.last_outcome = Some("locator-partial".into());
                });
                return Ok(report);
            }
            if after_ack(slot).is_err() {
                return Ok(report);
            }

            report.state = if slot == 0 {
                LocatorState::Partial
            } else {
                LocatorState::Unconfirmed
            };
            report.import.as_mut().unwrap().slots[slot].attempt = LocatorAttempt::Confirmed;
            witness(op, &report);
            if change(setup, |s| {
                s.imports[slot] = consent::Outcome::Accepted;
                s.last_outcome = Some(
                    if slot == 0 {
                        "locator-partial"
                    } else {
                        "locator-imported"
                    }
                    .into(),
                );
            })
            .is_err()
            {
                return Ok(report);
            }
            report.import.as_mut().unwrap().slots[slot].retained_confirmed = true;
            if slot == 1 {
                report.state = LocatorState::Imported;
            }
            witness(op, &report);
        }
        Ok(report)
    }

    fn import_locator_ack(
        &mut self,
        slot: usize,
        blob: &[u8],
        cancel: &AtomicBool,
        op: Option<&OperationControl>,
    ) -> Result<()> {
        let client = self.client.as_ref().context("Owned client")?;
        operation::wait(
            &self.rt,
            Some(cancel),
            op,
            client.import_locator(slot as u8, blob),
        )??;
        ensure!(
            self.child.try_wait()?.is_none()
                && RuntimeIpcDescriptor::load(&self.doc.ipc_file)
                    .is_ok_and(|d| &d == client.descriptor()),
            "Locator acknowledgement from stale runtime"
        );
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn export(&mut self, setup: &Setup, cancel: &AtomicBool) -> Result<PathBuf> {
        let report = self
            .export_report(setup, cancel, None)
            .map_err(anyhow::Error::msg)?;
        ensure!(
            report.state == LocatorState::Exported,
            "Locator publication unconfirmed"
        );
        Ok(PathBuf::from(report.path.context("Export path")?))
    }

    #[cfg(test)]
    pub(super) fn import(
        &mut self,
        setup: &mut Setup,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<()> {
        self.import_with(setup, path, cancel, |_| Ok(()))
    }

    #[cfg(test)]
    pub(super) fn import_with(
        &mut self,
        setup: &mut Setup,
        path: &Path,
        cancel: &AtomicBool,
        after_ack: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        let report = self
            .import_file(setup, path, cancel, None, after_ack)
            .map_err(anyhow::Error::msg)?;
        ensure!(
            report.state == LocatorState::Imported,
            "Locator import unconfirmed"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::OperationKind;

    #[test]
    fn t11d_export_cancellation_snapshot_and_no_clobber() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("public-owned.json");
        let candidate = || {
            OperationControl::new(
                OperationKind::Locator,
                Instant::now() + Duration::from_secs(10),
                None,
            )
        };
        let stopped = AtomicBool::new(false);
        let before = candidate();
        before.cancel(Instant::now());
        ensure!(
            publish_export(&file, b"public fixture", &stopped, Some(&before), || {}).err()
                == Some("cancelled")
                && !file.exists()
        );
        ensure!(before.finish(false).locator.is_none());

        let after = candidate();
        let mut frozen = None;
        let complete = publish_export(&file, b"public fixture", &stopped, Some(&after), || {
            after.cancel(Instant::now());
            frozen = after.finish(true).locator;
        })
        .map_err(anyhow::Error::msg)?;
        let frozen = frozen.context("Publication snapshot")?;
        ensure!(frozen.state == LocatorState::Unconfirmed && frozen.path == complete.path);
        ensure!(complete.state == LocatorState::Exported && fs::read(&file)? == b"public fixture");

        let collision = publish_export(&file, b"replacement", &stopped, None, || {
            panic!("Published over a collision");
        })
        .map_err(anyhow::Error::msg)?;
        ensure!(
            collision.state == LocatorState::Unconfirmed && fs::read(file)? == b"public fixture"
        );
        Ok(())
    }
}
