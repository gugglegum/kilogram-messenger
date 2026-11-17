//! Durable two-slot offline CS1 adapter. No runtime/profile/network operations.
use super::{
    artifact::{self, Identity, Recipient},
    consent_artifact::{BLOB_CAP, Envelope, Kind},
    helper,
    model::{self, Setup},
};
use anyhow::{Context, Result, ensure};
use kilogram_bootstrap_contract::parse_ordered2_id;
use kilogram_protocol::ordered_dialog_v2 as wire;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const INPUT_CAP: usize = 160 * 1024;
pub(super) const TTL: u64 = 30 * 86400;

#[derive(Clone, Copy, Debug)]
pub enum Failure {
    InvalidArtifact,
    Mismatch,
    WrongRole,
    NotReady,
    NotCurrent,
    Conflict,
    ExportConflict,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidArtifact => "Invalid signed consent artifact",
            Self::Mismatch => "Consent differs from the selected peer, contexts or proposal",
            Self::WrongRole => "Wrong canonical consent role",
            Self::NotReady => "Complete the required consent step first",
            Self::NotCurrent => {
                "Consent expired or not yet valid; existing artifacts were preserved"
            }
            Self::Conflict => "Consent output conflict; existing files were preserved",
            Self::ExportConflict => "Exchange output already has different bytes",
        })
    }
}
impl std::error::Error for Failure {}

pub fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn digest(raw: &[u8]) -> String {
    artifact::hex(blake3::hash(raw).as_bytes())
}
pub fn valid_interval(nb: u64, end: u64, wall: u64) -> Result<()> {
    ensure!(
        end > nb && end - nb <= TTL && wall.saturating_add(120) >= nb && wall < end,
        Failure::NotCurrent
    );
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub name: String,
    pub digest: String,
}
impl Reference {
    fn valid(&self) -> Result<()> {
        artifact::canonical_id(&self.digest)?;
        let named = matches!(
            self.name.as_str(),
            "inputs.json" | "received-proposal.kilogram" | "final.kilogram"
        );
        let attempt = self
            .name
            .strip_prefix("attempt-")
            .and_then(|s| s.strip_suffix(".bin"))
            .and_then(|s| s.rsplit_once('-'));
        let generated = attempt.is_some_and(|(id, slot)| {
            artifact::canonical_id(id).is_ok() && matches!(slot, "0" | "1")
        });
        ensure!(named || generated, "Unexpected local consent artifact path");
        Ok(())
    }
    fn read(&self, setup: &Setup, cap: usize) -> Result<Vec<u8>> {
        self.valid()?;
        let path = local_path(setup, "consent", &self.name)?;
        let raw = model::read(&path, cap)?;
        ensure!(
            digest(&raw) == self.digest,
            "Recorded consent artifact changed"
        );
        Ok(raw)
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Outcome {
    #[default]
    NotStarted,
    Started,
    Accepted,
    Failed,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub output: Option<Reference>,
    pub signing: Outcome,
    pub importing: Outcome,
    pub last_output: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u8,
    pub inputs: Reference,
    pub proposal: Option<Reference>,
    pub final_artifact: Option<Reference>,
    pub slots: [Slot; 2],
}
impl State {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.inputs.name == "inputs.json",
            "Unsupported consent progress"
        );
        self.inputs.valid()?;
        if let Some(r) = &self.proposal {
            ensure!(
                r.name == "received-proposal.kilogram",
                "Proposal reference role"
            );
            r.valid()?;
        }
        if let Some(r) = &self.final_artifact {
            ensure!(r.name == "final.kilogram", "Final reference role");
            r.valid()?;
        }
        for (slot, s) in self.slots.iter().enumerate() {
            if let Some(r) = &s.output {
                r.valid()?;
                ensure!(
                    r.name.starts_with("attempt-") && r.name.ends_with(&format!("-{slot}.bin")),
                    "Slot output path role"
                );
                ensure!(
                    s.signing == Outcome::Accepted,
                    "Output without accepted signing result"
                );
            }
            ensure!(
                s.signing != Outcome::Accepted || s.output.is_some(),
                "Accepted signing result missing its artifact"
            );
            ensure!(
                s.importing == Outcome::NotStarted || self.final_artifact.is_some(),
                "Import outcome missing exact finals"
            );
            if let Some(n) = &s.last_output {
                Reference {
                    name: n.clone(),
                    digest: "00".repeat(32),
                }
                .valid()?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    version: u8,
    local_id: String,
    invitation: String,
    contexts: [String; 2],
    own: Identity,
    peer: Identity,
    not_before: u64,
    ttl: u64,
}
impl Inputs {
    fn peers(&self) -> Result<[wire::RootVerifiedCredential; 2]> {
        self.own.validate()?;
        self.peer.validate()?;
        let mut peers = [
            wire::ParsedCredentialBundle::parse(&artifact::unhex(
                &self.own.credential,
                16 * 1024,
            )?)?
            .verify_root()?,
            wire::ParsedCredentialBundle::parse(&artifact::unhex(
                &self.peer.credential,
                16 * 1024,
            )?)?
            .verify_root()?,
        ];
        ensure!(
            peers[0].certificate().account_id() != peers[1].certificate().account_id()
                && peers[0].certificate().device_id() != peers[1].certificate().device_id(),
            "Distinct consent peers required"
        );
        peers.sort_by_key(|p| *p.certificate().device_id().as_bytes());
        Ok(peers)
    }
    fn check(&self, setup: &Setup, time: bool) -> Result<()> {
        let info = setup.record.public.as_ref().context("Account not ready")?;
        ensure!(
            self.version == 1
                && self.local_id == setup.record.local_id
                && self.invitation == setup.record.invitation
                && self.contexts == setup.record.contexts
                && setup.record.acknowledged
                && setup.phrase.is_none()
                && self.own == Setup::identity(info)?
                && Some(&self.peer) == setup.record.peer.as_ref(),
            "Frozen consent input pins differ"
        );
        ensure!((1..=TTL).contains(&self.ttl), "Frozen consent lifetime");
        let end = self
            .not_before
            .checked_add(self.ttl)
            .context("Consent expiry overflow")?;
        let _ = self.peers()?;
        if time {
            valid_interval(self.not_before, end, now()?)?;
        }
        Ok(())
    }
    fn interval(&self) -> (u64, u64) {
        (self.not_before, self.not_before + self.ttl)
    }
}
pub fn role(setup: &Setup) -> Result<usize> {
    let info = setup.record.public.as_ref().context("Account not ready")?;
    let peer = setup
        .record
        .peer
        .as_ref()
        .context("Confirm the peer identity first")?;
    ensure!(
        setup.record.acknowledged
            && !setup.incomplete
            && setup.phrase.is_none()
            && info.account_id != peer.account
            && info.device_id != peer.device,
        "Save recovery and confirm a distinct peer before consent"
    );
    Ok(usize::from(
        info.device_id.as_bytes() > peer.device.as_bytes(),
    ))
}
fn get_inputs(setup: &Setup, time: bool) -> Result<Inputs> {
    let state = setup.record.consent.as_ref().ok_or(Failure::NotReady)?;
    state.validate()?;
    let inputs: Inputs = serde_json::from_slice(&state.inputs.read(setup, INPUT_CAP)?)?;
    inputs.check(setup, time)?;
    Ok(inputs)
}
fn local_path(setup: &Setup, directory: &str, name: &str) -> Result<PathBuf> {
    let path = setup.home.join(directory).join(name);
    super::connection::validate_artifact_path(&setup.home, &setup.record, &path)?;
    Ok(path)
}

fn check_target(path: &Path, raw: &[u8], conflict: Failure) -> Result<()> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            ensure!(
                meta.len() == raw.len() as u64 && model::read(path, artifact::FILE_CAP)? == raw,
                conflict
            );
        }
        Ok(_) => return Err(conflict.into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn write_exact(path: &Path, raw: &[u8], conflict: Failure) -> Result<()> {
    if let Err(error) = model::write(path, raw, false) {
        if !path.try_exists()? {
            return Err(error);
        }
        check_target(path, raw, conflict)?;
    }
    Ok(())
}

fn put(setup: &Setup, name: &str, raw: &[u8]) -> Result<Reference> {
    let reference = Reference {
        name: name.into(),
        digest: digest(raw),
    };
    reference.valid()?;
    let path = local_path(setup, "consent", name)?;
    fs::create_dir_all(setup.home.join("consent"))?;
    write_exact(&path, raw, Failure::Conflict)?;
    Ok(reference)
}
fn candidate_inputs(setup: &Setup, nb: u64, ttl: u64) -> Result<Inputs> {
    let info = setup.ready()?;
    let inputs = Inputs {
        version: 1,
        local_id: setup.record.local_id.clone(),
        invitation: setup.record.invitation.clone(),
        contexts: setup.record.contexts.clone(),
        own: Setup::identity(&info)?,
        peer: setup.record.peer.clone().context("Confirm peer first")?,
        not_before: nb,
        ttl,
    };
    inputs.check(setup, true)?;
    Ok(inputs)
}

fn freeze(setup: &mut Setup, nb: u64, ttl: u64) -> Result<Inputs> {
    if setup.record.consent.is_some() {
        return get_inputs(setup, true);
    }
    let path = local_path(setup, "consent", "inputs.json")?;
    let inputs = if path.try_exists()? {
        serde_json::from_slice(&model::read(&path, INPUT_CAP)?)?
    } else {
        candidate_inputs(setup, nb, ttl)?
    };
    inputs.check(setup, true)?;
    let raw = serde_json::to_vec(&inputs)?;
    ensure!(raw.len() <= INPUT_CAP, "Frozen inputs cap");
    let reference = put(setup, "inputs.json", &raw)?;
    setup.update(|r| {
        r.consent = Some(State {
            version: 1,
            inputs: reference,
            proposal: None,
            final_artifact: None,
            slots: Default::default(),
        })
    })?;
    Ok(inputs)
}
fn change(setup: &mut Setup, f: impl FnOnce(&mut State)) -> Result<()> {
    ensure!(setup.record.consent.is_some(), "Consent not started");
    setup.update(|r| {
        if let Some(s) = &mut r.consent {
            f(s);
        }
    })
}
fn check_envelope(setup: &Setup, e: &Envelope, incoming: bool) -> Result<()> {
    e.validate()?;
    let info = setup.record.public.as_ref().context("Account not ready")?;
    let own = Setup::identity(info)?;
    let peer = setup.record.peer.as_ref().context("Confirm peer first")?;
    let (sender, recipient) = if incoming { (peer, &own) } else { (&own, peer) };
    ensure!(
        setup.record.acknowledged
            && setup.phrase.is_none()
            && e.sender == *sender
            && e.recipient
                == Recipient {
                    account: recipient.account,
                    device: recipient.device
                }
            && e.invitation == setup.record.invitation
            && e.contexts == setup.record.contexts,
        Failure::Mismatch
    );
    Ok(())
}
fn proposals(inputs: &Inputs, blobs: &[Vec<u8>; 2], wall: u64) -> Result<()> {
    let peers = inputs.peers()?;
    for (slot, raw) in blobs.iter().enumerate() {
        let v = wire::verify_fresh_proposal2(
            raw,
            parse_ordered2_id(&inputs.contexts[slot])?,
            &peers,
            wall,
        )?;
        let f = v.fields(7)?;
        let body = f[4].fields(10)?;
        ensure!(
            (body[8].uint()?, body[9].uint()?) == inputs.interval(),
            "Proposal differs from frozen validity"
        );
    }
    Ok(())
}
fn finals(inputs: &Inputs, blobs: &[Vec<u8>; 2], wall: Option<u64>) -> Result<()> {
    for (slot, raw) in blobs.iter().enumerate() {
        final_blob(inputs, slot, raw, wall)?;
    }
    Ok(())
}
fn final_blob(inputs: &Inputs, slot: usize, raw: &[u8], wall: Option<u64>) -> Result<()> {
    let peers = inputs.peers()?;
    let value = wire::decode(raw, BLOB_CAP)?;
    let f = value.fields(6)?;
    let c = parse_ordered2_id(&inputs.contexts[slot])?;
    ensure!(
        f[0].uint()? == 2
            && f[1].fixed::<32>()? == c
            && f[2].bytes()? == peers[0].raw()
            && f[3].bytes()? == peers[1].raw(),
        "Final ordered context/credential pins differ"
    );
    wire::verify_membership(c, &peers, f[4].bytes()?)?;
    let consent = wire::ParsedConsent2::parse(f[5].bytes()?)?;
    consent.verify(&peers)?;
    ensure!(
        consent.c() == c
            && consent.origin() == &wire::Origin::Fresh
            && consent.interval() == inputs.interval(),
        "Final Fresh body/validity differs from selected proposal"
    );
    if let Some(wall) = wall {
        let (nb, end) = consent.interval();
        valid_interval(nb, end, wall)?;
    }
    Ok(())
}
// Bind the returned finals to the exact retained administrative proposal,
// including membership and role0 signature, not just the same peer identities.
fn lineage(setup: &Setup, inputs: &Inputs, blobs: &[Vec<u8>; 2]) -> Result<()> {
    let state = setup.record.consent.as_ref().context("Consent")?;
    let proposed = if role(setup)? == 0 {
        [
            state.slots[0]
                .output
                .as_ref()
                .context("Complete proposal slot1 first")?
                .read(setup, BLOB_CAP)?,
            state.slots[1]
                .output
                .as_ref()
                .context("Complete proposal slot2 first")?
                .read(setup, BLOB_CAP)?,
        ]
    } else {
        Envelope::decode(
            &state
                .proposal
                .as_ref()
                .context("Retained proposal missing")?
                .read(setup, artifact::FILE_CAP)?,
        )?
        .payloads()?
    };
    let peers = inputs.peers()?;
    for slot in 0..2 {
        let p = wire::verify_fresh_proposal2(
            &proposed[slot],
            parse_ordered2_id(&inputs.contexts[slot])?,
            &peers,
            inputs.not_before,
        )?;
        let p = p.fields(7)?;
        let f = wire::decode(&blobs[slot], BLOB_CAP)?;
        let f = f.fields(6)?;
        let signed = wire::decode(f[5].bytes()?, 4096)?;
        let signed = signed.fields(3)?;
        ensure!(
            f[2] == p[1] && f[3] == p[2] && f[4] == p[3] && signed[0] == p[4] && signed[1] == p[5],
            "Final differs from the exact selected proposal"
        );
    }
    Ok(())
}
pub fn period(setup: &Setup) -> Result<(u64, u64)> {
    Ok(get_inputs(setup, false)?.interval())
}

/// Exact accepted final references for the connection adapter; no DB claim.
pub(super) fn connection_refs(setup: &Setup) -> Result<[String; 2]> {
    review(setup, true)?;
    let state = setup.record.consent.as_ref().context("Consent")?;
    let envelope = Envelope::decode(
        &state
            .final_artifact
            .as_ref()
            .context("Finals")?
            .read(setup, artifact::FILE_CAP)?,
    )?;
    let blobs = envelope.payloads()?;
    let mut refs = [String::new(), String::new()];
    for (slot, raw) in blobs.iter().enumerate() {
        let value = wire::decode(raw, BLOB_CAP)?;
        refs[slot] =
            artifact::hex(&wire::ParsedConsent2::parse(value.fields(6)?[5].bytes()?)?.reference());
    }
    Ok(refs)
}
fn envelope(setup: &Setup, kind: Kind, inputs: &Inputs, blobs: [Vec<u8>; 2]) -> Envelope {
    Envelope {
        version: 1,
        kind,
        mode: "ordered-dialog-v2".into(),
        invitation: setup.record.invitation.clone(),
        contexts: setup.record.contexts.clone(),
        sender: inputs.own.clone(),
        recipient: Recipient {
            account: inputs.peer.account,
            device: inputs.peer.device,
        },
        blobs: blobs.each_ref().map(|b| artifact::hex(b)),
    }
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct PublicArtifact {
    pub kind: Kind,
    pub invitation_id: String,
    pub contexts: [String; 2],
    pub sender: PublicIdentity,
    pub recipient: PublicIdentity,
    pub not_before: u64,
    pub expires_at: u64,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PublicIdentity {
    pub account_id: String,
    pub device_id: String,
}

impl PublicArtifact {
    fn new(e: &Envelope, inputs: &Inputs, raw: &[u8]) -> Self {
        let (not_before, expires_at) = inputs.interval();
        Self {
            kind: e.kind,
            invitation_id: e.invitation.clone(),
            contexts: e.contexts.clone(),
            sender: PublicIdentity {
                account_id: e.sender.account.to_string(),
                device_id: e.sender.device.to_string(),
            },
            recipient: PublicIdentity {
                account_id: e.recipient.account.to_string(),
                device_id: e.recipient.device.to_string(),
            },
            not_before,
            expires_at,
            artifact_digest: digest(raw),
        }
    }
}

fn proposal_period(blob: &[u8]) -> Result<(u64, u64)> {
    let value = wire::decode(blob, BLOB_CAP)?;
    let body = value.fields(7)?[4].fields(10)?;
    let nb = body[8].uint()?;
    let ttl = body[9]
        .uint()?
        .checked_sub(nb)
        .context("Proposal interval")?;
    ensure!((1..=TTL).contains(&ttl), Failure::InvalidArtifact);
    Ok((nb, ttl))
}

fn selection(setup: &Setup, e: &Envelope) -> Result<(Inputs, Vec<u8>, &'static str)> {
    check_envelope(setup, e, true)?;
    let r = role(setup)?;
    let blobs = e.payloads().context(Failure::InvalidArtifact)?;
    let raw = e.encode()?;
    let wall = now()?;
    let (inputs, name, prior) = match e.kind {
        Kind::Proposal => {
            ensure!(r == 1, Failure::WrongRole);
            let (nb, ttl) = proposal_period(&blobs[0]).context(Failure::InvalidArtifact)?;
            let candidate = candidate_inputs(setup, nb, ttl)?;
            proposals(&candidate, &blobs, wall).context(Failure::InvalidArtifact)?;
            if setup.record.consent.is_some() {
                ensure!(get_inputs(setup, false)? == candidate, Failure::Conflict);
            } else {
                let path = local_path(setup, "consent", "inputs.json")?;
                check_target(&path, &serde_json::to_vec(&candidate)?, Failure::Conflict)?;
            }
            let prior = setup
                .record
                .consent
                .as_ref()
                .and_then(|s| s.proposal.as_ref());
            (candidate, "received-proposal.kilogram", prior)
        }
        Kind::Final => {
            ensure!(r == 0, Failure::WrongRole);
            let inputs = get_inputs(setup, true)?;
            let state = setup.record.consent.as_ref().ok_or(Failure::NotReady)?;
            ensure!(
                state.slots.iter().all(|s| s.output.is_some()),
                Failure::NotReady
            );
            valid_interval(inputs.not_before, inputs.interval().1, wall)?;
            finals(&inputs, &blobs, None).context(Failure::InvalidArtifact)?;
            lineage(setup, &inputs, &blobs).context(Failure::Mismatch)?;
            (inputs, "final.kilogram", state.final_artifact.as_ref())
        }
    };
    if let Some(prior) = prior {
        ensure!(
            prior.read(setup, artifact::FILE_CAP)? == raw,
            Failure::Conflict
        );
    }
    let path = local_path(setup, "consent", name)?;
    check_target(&path, &raw, Failure::Conflict)?;
    Ok((inputs, raw, name))
}

/// Validate incoming consent and return public metadata without changing Setup.
pub fn preview(setup: &Setup, e: &Envelope) -> Result<PublicArtifact> {
    let (inputs, raw, _) = selection(setup, e)?;
    Ok(PublicArtifact::new(e, &inputs, &raw))
}

pub fn select(setup: &mut Setup, e: &Envelope) -> Result<()> {
    let (inputs, raw, name) = selection(setup, e)?;
    if e.kind == Kind::Proposal {
        freeze(setup, inputs.not_before, inputs.ttl)?;
    }
    let reference = put(setup, name, &raw)?;
    match e.kind {
        Kind::Proposal => change(setup, |s| s.proposal = Some(reference))?,
        Kind::Final => change(setup, |s| s.final_artifact = Some(reference))?,
    }
    setup.consent_confirmed = false;
    Ok(())
}
/// Check references and crypto on reopen, but do not claim repository installation.
pub fn review(setup: &Setup, time: bool) -> Result<()> {
    if setup.record.consent.is_none() {
        return Ok(());
    }
    let inputs = get_inputs(setup, time)?;
    let state = setup.record.consent.as_ref().context("Consent")?;
    if let Some(reference) = &state.proposal {
        let e = Envelope::decode(&reference.read(setup, artifact::FILE_CAP)?)?;
        ensure!(e.kind == Kind::Proposal, "Selected proposal kind");
        check_envelope(setup, &e, true)?;
        // For expired saved data, verify its signatures at its own not-before;
        // current permission is separately blocked by get_inputs(time=true).
        proposals(
            &inputs,
            &e.payloads()?,
            if time { now()? } else { inputs.not_before },
        )?;
    }
    for (slot, s) in state.slots.iter().enumerate() {
        if let Some(reference) = &s.output {
            let raw = reference.read(setup, BLOB_CAP)?;
            if role(setup)? == 0 {
                let peers = inputs.peers()?;
                let v = wire::verify_fresh_proposal2(
                    &raw,
                    parse_ordered2_id(&inputs.contexts[slot])?,
                    &peers,
                    if time { now()? } else { inputs.not_before },
                )?;
                let body = v.fields(7)?[4].fields(10)?;
                ensure!(
                    (body[8].uint()?, body[9].uint()?) == inputs.interval(),
                    "Saved proposal interval"
                );
            } else {
                final_blob(&inputs, slot, &raw, if time { Some(now()?) } else { None })?;
            }
        }
    }
    if let Some(reference) = &state.final_artifact {
        let e = Envelope::decode(&reference.read(setup, artifact::FILE_CAP)?)?;
        ensure!(e.kind == Kind::Final, "Selected final kind");
        check_envelope(setup, &e, role(setup)? == 0)?;
        finals(
            &inputs,
            &e.payloads()?,
            if time { Some(now()?) } else { None },
        )?;
        lineage(setup, &inputs, &e.payloads()?)?;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Propose,
    Cosign,
    Import,
}
fn stopped(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        helper::HelperError::Cancelled
    );
    Ok(())
}
fn argv(
    inputs: &Inputs,
    setup: &Setup,
    slot: usize,
    path: &Path,
    cosign: bool,
) -> Result<Vec<OsString>> {
    let info = setup.record.public.as_ref().context("Account not ready")?;
    let mut args: Vec<OsString> = vec![
        "ordered".into(),
        if cosign {
            "context2-cosign".into()
        } else {
            "context2-propose".into()
        },
    ];
    for (key, value) in [
        (
            "--account-dir",
            info.account_root_dir.clone().into_os_string(),
        ),
        ("--expect-account", inputs.own.account.to_string().into()),
        ("--expect-device", inputs.own.device.to_string().into()),
        (
            "--expect-peer-account",
            inputs.peer.account.to_string().into(),
        ),
        (
            "--expect-peer-device",
            inputs.peer.device.to_string().into(),
        ),
        ("--context", inputs.contexts[slot].clone().into()),
        ("--output-file", path.as_os_str().to_owned()),
    ] {
        args.push(key.into());
        args.push(value);
    }
    args.push("--credentials-file".into());
    for n in ["own.credential", "peer.credential"] {
        args.push(setup.home.join("consent").join(n).into_os_string());
    }
    args.push("--minimum-authority-file".into());
    for n in ["own.authority", "peer.authority"] {
        args.push(setup.home.join("consent").join(n).into_os_string());
    }
    if cosign {
        args.push("--proposal-file".into());
        args.push(
            setup
                .home
                .join("consent")
                .join(format!("input-{slot}.proposal"))
                .into_os_string(),
        );
    } else {
        args.extend([
            "--not-before".into(),
            inputs.not_before.to_string().into(),
            "--valid-for-seconds".into(),
            inputs.ttl.to_string().into(),
        ]);
    }
    Ok(args)
}
fn public_file(setup: &Setup, name: &str, raw: &[u8]) -> Result<PathBuf> {
    let path = local_path(setup, "consent", name)?;
    write_exact(&path, raw, Failure::Conflict)?;
    Ok(path)
}

fn failed_helper(
    result: Result<()>,
    record: impl FnOnce() -> Result<()>,
    message: &'static str,
) -> Result<()> {
    if let Err(error) = result {
        let mut error = error.context(message);
        if record().is_err() {
            error = error.context("Could not record failed consent attempt");
        }
        return Err(error);
    }
    Ok(())
}
pub fn execute(
    setup: &mut Setup,
    exe: &Path,
    action: Action,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    execute_with(setup, exe, action, cancel, |_| Ok(()))
}
fn execute_with(
    setup: &mut Setup,
    exe: &Path,
    action: Action,
    cancel: Arc<AtomicBool>,
    mut after: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let r = role(setup)?;
    stopped(&cancel)?;
    let _ = setup.ready()?;
    match action {
        Action::Propose => {
            ensure!(r == 0, Failure::WrongRole);
            freeze(setup, now()?, TTL)?;
        }
        Action::Cosign => ensure!(r == 1, Failure::WrongRole),
        Action::Import => setup.consent_confirmed = false,
    }
    let inputs = get_inputs(setup, true)?;
    review(setup, true)?;
    if matches!(action, Action::Import) {
        let state = setup.record.consent.as_ref().context("Consent")?;
        let e = Envelope::decode(
            &state
                .final_artifact
                .as_ref()
                .ok_or(Failure::NotReady)?
                .read(setup, artifact::FILE_CAP)?,
        )?;
        let blobs = e.payloads()?;
        finals(&inputs, &blobs, Some(now()?))?; // both checked before either import
        lineage(setup, &inputs, &blobs)?;
        for (slot, raw) in blobs.iter().enumerate() {
            stopped(&cancel)?;
            finals(&inputs, &blobs, Some(now()?))?;
            let path = match public_file(setup, &format!("import-{slot}.context"), raw) {
                Ok(path) => path,
                Err(error) => {
                    change(setup, |s| s.slots[slot].importing = Outcome::Failed)?;
                    return Err(error);
                }
            };
            change(setup, |s| s.slots[slot].importing = Outcome::Started)?;
            let info = setup.record.public.as_ref().context("Account")?;
            let args = vec![
                "ordered".into(),
                "context2-import".into(),
                "--state-dir".into(),
                info.state_dir.clone().into_os_string(),
                "--expect-account".into(),
                info.account_id.to_string().into(),
                "--expect-claim".into(),
                info.claim.clone().into(),
                "--invitation-file".into(),
                path.into_os_string(),
            ];
            failed_helper(
                helper::public_command(exe, &args, cancel.clone()),
                || change(setup, |s| s.slots[slot].importing = Outcome::Failed),
                "Local import not confirmed. Stop the messaging runtime if it holds this account; retry the same finals. Existing files were preserved.",
            )?;
            after(slot)?; // controlled test interruption can model lost result after commit
            change(setup, |s| s.slots[slot].importing = Outcome::Accepted)?;
        }
        review(setup, true)?;
        setup.consent_confirmed = true;
        return Ok(());
    }
    let cosign = matches!(action, Action::Cosign);
    let proposal = if cosign {
        let state = setup.record.consent.as_ref().context("Consent")?;
        let e = Envelope::decode(
            &state
                .proposal
                .as_ref()
                .ok_or(Failure::NotReady)?
                .read(setup, artifact::FILE_CAP)?,
        )?;
        let blobs = e.payloads()?;
        proposals(&inputs, &blobs, now()?)?;
        Some(blobs)
    } else {
        None
    };
    for (name, raw) in [
        (
            "own.credential",
            artifact::unhex(&inputs.own.credential, 16 * 1024)?,
        ),
        (
            "peer.credential",
            artifact::unhex(&inputs.peer.credential, 16 * 1024)?,
        ),
        (
            "own.authority",
            artifact::unhex(&inputs.own.authority, 16 * 1024)?,
        ),
        (
            "peer.authority",
            artifact::unhex(&inputs.peer.authority, 16 * 1024)?,
        ),
    ] {
        let _ = public_file(setup, name, &raw)?;
    }
    if let Some(blobs) = proposal {
        for (slot, raw) in blobs.iter().enumerate() {
            let _ = public_file(setup, &format!("input-{slot}.proposal"), raw)?;
        }
    }
    for slot in 0..2 {
        stopped(&cancel)?;
        review(setup, true)?;
        if setup.record.consent.as_ref().context("Consent")?.slots[slot]
            .output
            .is_some()
        {
            continue;
        }
        let name = format!("attempt-{}-{slot}.bin", artifact::id()?);
        let path = local_path(setup, "consent", &name)?;
        change(setup, |s| {
            s.slots[slot].signing = Outcome::Started;
            s.slots[slot].last_output = Some(name.clone());
        })?;
        let args = argv(&inputs, setup, slot, &path, cosign)?;
        failed_helper(
            helper::public_command(exe, &args, cancel.clone()),
            || change(setup, |s| s.slots[slot].signing = Outcome::Failed),
            "Signing not confirmed. Its output may be empty; files retained. Retry uses the frozen inputs and a fresh output name.",
        )?;
        let raw = model::read(&path, BLOB_CAP)?;
        if cosign {
            let value = wire::decode(&raw, BLOB_CAP)?;
            ensure!(
                value.fields(6)?[1].fixed::<32>()? == parse_ordered2_id(&inputs.contexts[slot])?,
                "Signed final context"
            );
        } else {
            let peers = inputs.peers()?;
            let _ = wire::verify_fresh_proposal2(
                &raw,
                parse_ordered2_id(&inputs.contexts[slot])?,
                &peers,
                now()?,
            )?;
        }
        after(slot)?;
        change(setup, |s| {
            s.slots[slot].output = Some(Reference {
                name,
                digest: digest(&raw),
            });
            s.slots[slot].signing = Outcome::Accepted;
        })?;
    }
    let state = setup.record.consent.as_ref().context("Consent")?;
    let blobs = [
        state.slots[0]
            .output
            .as_ref()
            .context("Slot0 output")?
            .read(setup, BLOB_CAP)?,
        state.slots[1]
            .output
            .as_ref()
            .context("Slot1 output")?
            .read(setup, BLOB_CAP)?,
    ];
    if cosign {
        finals(&inputs, &blobs, Some(now()?))?;
        lineage(setup, &inputs, &blobs)?;
        let e = envelope(setup, Kind::Final, &inputs, blobs);
        let reference = put(setup, "final.kilogram", &e.encode()?)?;
        change(setup, |s| s.final_artifact = Some(reference))?;
    } else {
        proposals(&inputs, &blobs, now()?)?;
    }
    Ok(())
}
pub fn export(setup: &Setup) -> Result<PathBuf> {
    Ok(export_public(setup)?.0)
}

pub fn export_public(setup: &Setup) -> Result<(PathBuf, PublicArtifact)> {
    review(setup, true)?;
    let inputs = get_inputs(setup, true)?;
    let state = setup.record.consent.as_ref().context("Consent")?;
    let (name, raw) = if role(setup)? == 0 {
        let blobs = [
            state.slots[0]
                .output
                .as_ref()
                .ok_or(Failure::NotReady)?
                .read(setup, BLOB_CAP)?,
            state.slots[1]
                .output
                .as_ref()
                .ok_or(Failure::NotReady)?
                .read(setup, BLOB_CAP)?,
        ];
        proposals(&inputs, &blobs, now()?)?;
        (
            "proposal.kilogram",
            envelope(setup, Kind::Proposal, &inputs, blobs).encode()?,
        )
    } else {
        (
            "final.kilogram",
            state
                .final_artifact
                .as_ref()
                .ok_or(Failure::NotReady)?
                .read(setup, artifact::FILE_CAP)?,
        )
    };
    let path = local_path(setup, "exchange", name)?;
    let public = PublicArtifact::new(&Envelope::decode(&raw)?, &inputs, &raw);
    fs::create_dir_all(setup.home.join("exchange"))?;
    write_exact(&path, &raw, Failure::ExportConflict)?;
    Ok((path, public))
}

#[cfg(test)]
#[path = "consent_tests.rs"]
pub(super) mod tests;
