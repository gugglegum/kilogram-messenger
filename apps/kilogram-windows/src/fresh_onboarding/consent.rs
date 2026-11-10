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
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const INPUT_CAP: usize = 160 * 1024;
pub(super) const TTL: u64 = 30 * 86400;
pub(super) fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn digest(raw: &[u8]) -> String {
    artifact::hex(blake3::hash(raw).as_bytes())
}
pub(super) fn valid_interval(nb: u64, end: u64, wall: u64) -> Result<()> {
    ensure!(
        end > nb && end - nb <= TTL && wall.saturating_add(120) >= nb && wall < end,
        "Consent expired or not yet valid; existing artifacts were preserved"
    );
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
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
        let raw = model::read(&setup.home.join("consent").join(&self.name), cap)?;
        ensure!(
            digest(&raw) == self.digest,
            "Recorded consent artifact changed"
        );
        Ok(raw)
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum Outcome {
    #[default]
    NotStarted,
    Started,
    Accepted,
    Failed,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slot {
    pub output: Option<Reference>,
    pub signing: Outcome,
    pub importing: Outcome,
    pub last_output: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
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
pub(super) fn role(setup: &Setup) -> Result<usize> {
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
    let state = setup
        .record
        .consent
        .as_ref()
        .context("Consent not started")?;
    state.validate()?;
    let inputs: Inputs = serde_json::from_slice(&state.inputs.read(setup, INPUT_CAP)?)?;
    inputs.check(setup, time)?;
    Ok(inputs)
}
fn put(setup: &Setup, name: &str, raw: &[u8]) -> Result<Reference> {
    let reference = Reference {
        name: name.into(),
        digest: digest(raw),
    };
    reference.valid()?;
    let parent = setup.home.join("consent");
    fs::create_dir_all(&parent)?;
    let path = parent.join(name);
    if let Err(e) = model::write(&path, raw, false) {
        ensure!(
            model::read(&path, artifact::FILE_CAP)? == raw,
            "Consent output conflict; existing files preserved: {e}"
        );
    }
    Ok(reference)
}
fn freeze(setup: &mut Setup, nb: u64, ttl: u64) -> Result<Inputs> {
    if setup.record.consent.is_some() {
        return get_inputs(setup, true);
    }
    let info = setup.ready()?;
    let mut inputs = Inputs {
        version: 1,
        local_id: setup.record.local_id.clone(),
        invitation: setup.record.invitation.clone(),
        contexts: setup.record.contexts.clone(),
        own: Setup::identity(&info)?,
        peer: setup.record.peer.clone().context("Confirm peer first")?,
        not_before: nb,
        ttl,
    };
    let path = setup.home.join("consent/inputs.json");
    if path.try_exists()? {
        inputs = serde_json::from_slice(&model::read(&path, INPUT_CAP)?)?;
    }
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
        "Consent sender/recipient/invitation/context pins differ"
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
pub(super) fn period(setup: &Setup) -> Result<(u64, u64)> {
    Ok(get_inputs(setup, false)?.interval())
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
pub(super) fn select(setup: &mut Setup, e: &Envelope) -> Result<()> {
    check_envelope(setup, e, true)?;
    let r = role(setup)?;
    let blobs = e.payloads()?;
    match e.kind {
        Kind::Proposal => {
            ensure!(r == 1, "The canonical proposer must create the proposal");
            let body = wire::decode(&blobs[0], BLOB_CAP)?.fields(7)?[4].clone();
            let b = body.fields(10)?;
            let nb = b[8].uint()?;
            let ttl = b[9].uint()?.checked_sub(nb).context("Proposal interval")?;
            // Verify BOTH public proposals before freezing selection or signing.
            let info = setup.ready()?;
            let candidate = Inputs {
                version: 1,
                local_id: setup.record.local_id.clone(),
                invitation: setup.record.invitation.clone(),
                contexts: setup.record.contexts.clone(),
                own: Setup::identity(&info)?,
                peer: setup.record.peer.clone().context("Peer")?,
                not_before: nb,
                ttl,
            };
            candidate.check(setup, true)?;
            proposals(&candidate, &blobs, now()?)?;
            let inputs = freeze(setup, nb, ttl)?;
            proposals(&inputs, &blobs, now()?)?;
            let reference = put(setup, "received-proposal.kilogram", &e.encode()?)?;
            if let Some(old) = &setup.record.consent.as_ref().context("Consent")?.proposal {
                ensure!(
                    old == &reference,
                    "A different proposal was already selected"
                );
            }
            change(setup, |s| s.proposal = Some(reference))?;
        }
        Kind::Final => {
            ensure!(
                r == 0,
                "Only the proposer receives the cosigner's final file"
            );
            let inputs = get_inputs(setup, true)?;
            finals(&inputs, &blobs, Some(now()?))?;
            lineage(setup, &inputs, &blobs)?;
            let reference = put(setup, "final.kilogram", &e.encode()?)?;
            if let Some(old) = &setup
                .record
                .consent
                .as_ref()
                .context("Consent")?
                .final_artifact
            {
                ensure!(old == &reference, "Different finals already selected");
            }
            change(setup, |s| s.final_artifact = Some(reference))?;
        }
    }
    setup.consent_confirmed = false;
    Ok(())
}
/// Check references and crypto on reopen, but do not claim repository installation.
pub(super) fn review(setup: &Setup, time: bool) -> Result<()> {
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
#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    Propose,
    Cosign,
    Import,
}
fn stopped(cancel: &AtomicBool) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Consent action cancelled; saved progress retained"
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
    let path = setup.home.join("consent").join(name);
    if let Err(e) = model::write(&path, raw, false) {
        ensure!(
            model::read(&path, artifact::FILE_CAP)? == raw,
            "Existing public input conflict: {e}"
        );
    }
    Ok(path)
}
pub(super) fn execute(
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
    if matches!(action, Action::Propose) {
        ensure!(r == 0, "Only canonical role0 proposes");
        freeze(setup, now()?, TTL)?;
    }
    let inputs = get_inputs(setup, true)?;
    review(setup, true)?;
    if matches!(action, Action::Import) {
        setup.consent_confirmed = false;
        let state = setup.record.consent.as_ref().context("Consent")?;
        let e = Envelope::decode(
            &state
                .final_artifact
                .as_ref()
                .context("Select both final contexts first")?
                .read(setup, artifact::FILE_CAP)?,
        )?;
        let blobs = e.payloads()?;
        finals(&inputs, &blobs, Some(now()?))?; // both checked before either import
        lineage(setup, &inputs, &blobs)?;
        for (slot, raw) in blobs.iter().enumerate() {
            stopped(&cancel)?;
            finals(&inputs, &blobs, Some(now()?))?;
            let path = public_file(setup, &format!("import-{slot}.context"), raw)?;
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
            if helper::public_command(exe, &args, cancel.clone()).is_err() {
                change(setup, |s| s.slots[slot].importing = Outcome::Failed)?;
                anyhow::bail!(
                    "Local import not confirmed. Stop the messaging runtime if it holds this account; retry the same finals. Existing files were preserved."
                );
            }
            after(slot)?; // controlled test interruption can model lost result after commit
            change(setup, |s| s.slots[slot].importing = Outcome::Accepted)?;
        }
        review(setup, true)?;
        setup.consent_confirmed = true;
        return Ok(());
    }
    let cosign = matches!(action, Action::Cosign);
    ensure!(r == usize::from(cosign), "Wrong canonical signing role");
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
    if cosign {
        let state = setup.record.consent.as_ref().context("Consent")?;
        let e = Envelope::decode(
            &state
                .proposal
                .as_ref()
                .context("Inspect and select a proposal first")?
                .read(setup, artifact::FILE_CAP)?,
        )?;
        let blobs = e.payloads()?;
        proposals(&inputs, &blobs, now()?)?;
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
        let path = setup.home.join("consent").join(&name);
        change(setup, |s| {
            s.slots[slot].signing = Outcome::Started;
            s.slots[slot].last_output = Some(name.clone());
        })?;
        let args = argv(&inputs, setup, slot, &path, cosign)?;
        if helper::public_command(exe, &args, cancel.clone()).is_err() {
            change(setup, |s| s.slots[slot].signing = Outcome::Failed)?;
            anyhow::bail!(
                "Signing not confirmed. Its output may be empty; files retained. Retry uses the frozen inputs and a fresh output name."
            );
        }
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
pub(super) fn export(setup: &Setup) -> Result<PathBuf> {
    review(setup, true)?;
    let inputs = get_inputs(setup, true)?;
    let state = setup.record.consent.as_ref().context("Consent")?;
    let (name, raw) = if role(setup)? == 0 {
        let blobs = [
            state.slots[0]
                .output
                .as_ref()
                .context("Proposal slot0 incomplete")?
                .read(setup, BLOB_CAP)?,
            state.slots[1]
                .output
                .as_ref()
                .context("Proposal slot1 incomplete")?
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
                .context("Cosign both contexts first")?
                .read(setup, artifact::FILE_CAP)?,
        )
    };
    let parent = setup.home.join("exchange");
    fs::create_dir_all(&parent)?;
    let path = parent.join(name);
    if let Err(e) = model::write(&path, &raw, false) {
        ensure!(
            model::read(&path, artifact::FILE_CAP)? == raw,
            "Exchange output already has different bytes: {e}"
        );
    }
    Ok(path)
}

#[cfg(test)]
#[path = "consent_tests.rs"]
mod tests;
