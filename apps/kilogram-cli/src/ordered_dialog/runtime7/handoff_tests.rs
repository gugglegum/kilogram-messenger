//! Charged component cases. Genuine enrollment, old Engine, Native, migration
//! and the one production owner; component gates make no TLS/process claim.
use super::super::{repair_tests::Pair, storage_migration};
use super::control::*;
use super::owner::Owner7;
use super::*;
use kilogram_identity::{AccountId, AccountRootState};
use kilogram_protocol::ordered_dialog_v2 as v2;
use kilogram_ratchet::ordered::native_call_counts;
use kilogram_state::StateDirectoryLock;
use serde_json::{Value as Json, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
#[path = "../../../tests/support/attempt_result.rs"]
mod attempt_result;
#[path = "handoff_integration_tests.rs"]
mod integration;

struct Binding {
    case: String,
    run: PathBuf,
    provenance: attempt_result::Provenance,
}
impl Binding {
    fn managed(case: &str) -> Result<Self> {
        if std::env::var_os("KILOGRAM_S53_CASE").is_none() {
            // The shared legacy helper can label unavailable runner provenance;
            // this new managed caller refuses it before creating any fixture.
            let unavailable = attempt_result::Provenance::UnavailableLegacyRunner;
            anyhow::bail!(
                "S5.3 requires bound managed provenance: {}",
                serde_json::to_string(&unavailable)?
            );
        }
        ensure!(
            std::env::var("KILOGRAM_S53_CASE")? == case,
            "managed case identity"
        );
        let path = PathBuf::from(
            std::env::var_os("KILOGRAM_S53_INVENTORY").context("pre-run inventory required")?,
        );
        let id = std::env::var("KILOGRAM_S53_INVENTORY_ID")?;
        ensure!(
            path.is_absolute() && std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "bounded absolute inventory"
        );
        let raw = std::fs::read(&path)?;
        let inventory: Json = serde_json::from_slice(&raw)?;
        ensure!(
            inventory["classification"] == "bound_pre_run_inventory"
                && inventory["case"] == case
                && inventory["inventory_identity"] == id
                && inventory["baseline"] == "64c2359788ead360314793e47741adacb83b8d4f"
                && inventory["binding_sha256"]
                    .as_str()
                    .is_some_and(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                && inventory["sources"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty())
                && inventory["images"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty()),
            "actual pre-run binding"
        );
        let run = PathBuf::from(inventory["run"].as_str().context("managed run directory")?);
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        ensure!(
            run.is_absolute()
                && run.canonicalize()?.starts_with(workspace.join(".tmp"))
                && run.is_dir(),
            "fresh owned workspace run"
        );
        Ok(Self {
            case: case.to_owned(),
            run,
            provenance: attempt_result::Provenance::BoundPreRunInventory {
                path,
                blake3: blake3::hash(&raw).to_hex().to_string(),
                identity: id,
            },
        })
    }
    fn finish(self, observations: Json, tail: impl FnOnce() -> Result<()>) -> Result<()> {
        let output = self.run.join("assertions.json");
        attempt_result::persist_before_tail(
            &output,
            &attempt_result::Envelope {
                case: &self.case,
                run: &self.run,
                output: &output,
                provenance: &self.provenance,
                observations: &observations,
            },
            tail,
        )
    }
}

fn controls_phase(
    timing: Option<(&'static str, Instant)>,
    phase: &'static str,
    event: &'static str,
    completed_pairs: u16,
) {
    if let Some((scenario, started)) = timing {
        println!(
            "s53_phase scenario={scenario} phase={phase} event={event} elapsed_ms={} completed_pairs={completed_pairs}",
            started.elapsed().as_millis()
        );
    }
}

struct Fixture {
    owners: [Owner7; 2],
    profiles: [PathBuf; 2],
    roots: [PathBuf; 2],
    consent: Vec<u8>,
    c: Id,
    timing: Option<(&'static str, Instant)>,
}
fn send_old(p: &mut Pair, role: usize, request: u8, deliver: bool) -> Result<Event> {
    let (sender, receiver) = if role == 0 {
        (&mut p.a, &mut p.b)
    } else {
        (&mut p.b, &mut p.a)
    };
    sender.queue(
        p.c,
        [request; 16],
        &format!("genuine text {role}/{request}"),
    )?;
    let event = sender
        .materialize_one(p.c, p.binding)?
        .context("real old materialization")?;
    if deliver {
        let frame =
            sender.deliver_frame(p.c, std::slice::from_ref(&event), p.binding, [request; 16])?;
        let result = receiver.accept_data_frame(p.c, &frame)?;
        ensure!(
            result.error.is_none() && result.readable == 1,
            "genuine old Native receive"
        );
        let (ack, ordinals) = receiver
            .ack_intents(p.c, p.binding)?
            .context("real old ACK")?;
        sender.accept_ack_frame(p.c, &ack)?;
        receiver.mark_ack_intents(p.c, p.binding, &ordinals, 2)?;
    }
    Ok(event)
}
fn renew_legacy_authority(p: &mut Pair) -> Result<()> {
    // A real additional certificate advances Root authority. The original C's
    // Device/certificate is retained and never revoked or re-enrolled.
    let root = AccountRootState::load(&p.accounts[0])?;
    let extra = kilogram_identity::DeviceIdentity::generate()?;
    let encryption = kilogram_crypto::DeviceEncryptionIdentity::generate()?;
    root.issue_device_certificate(
        extra.device_id(),
        encryption.public_key(),
        &kilogram_identity::DeviceCapability::MESSAGING,
    )?;
    let authority = root.authority_snapshot()?;
    let before = p.a.own_credentials()?;
    assert!(authority.revision() > before.authority.revision());
    let heads = [p.a.head(p.c)?, p.b.head(p.c)?];
    let sessions = [p.a.native(&heads[0])?, p.b.native(&heads[1])?];
    let calls = native_call_counts();
    for engine in [&mut p.a, &mut p.b] {
        engine.pin_authority(&authority.encode()?)?;
    }
    let after = p.a.own_credentials()?;
    assert_ne!(before.reference(), after.reference());
    assert_eq!(before.cert_ref()?, after.cert_ref()?);
    v2::ParsedCredentialBundle::parse(&after.raw)?.verify_root()?;
    for (role, engine) in [&p.a, &p.b].into_iter().enumerate() {
        let head = engine.head(p.c)?;
        let native = engine.native(&head)?;
        assert_eq!(
            (
                head.c,
                head.d,
                head.e,
                &head.sid,
                head.local_cert,
                head.native_generation
            ),
            (
                heads[role].c,
                heads[role].d,
                heads[role].e,
                &heads[role].sid,
                heads[role].local_cert,
                heads[role].native_generation
            )
        );
        assert_eq!(
            native
                .session
                .as_ref()
                .context("same renewal session")?
                .keys(),
            sessions[role]
                .session
                .as_ref()
                .context("original renewal session")?
                .keys()
        );
        assert_eq!(native.reservation, sessions[role].reservation);
    }
    assert_eq!(
        calls,
        native_call_counts(),
        "authority renewal performs no Native operation"
    );
    // Pin updates invalidate the old component leases. Re-admit through the
    // existing fixture path; do not call any bootstrap/session constructor.
    p.admit()?;
    let event = send_old(p, 0, 16, true)?;
    assert_eq!(event.core.author_auth, after.reference());
    assert_ne!(event.core.author_auth, before.reference());
    let text_calls = native_call_counts();
    assert_eq!(
        (
            text_calls.fresh_account,
            text_calls.outbound_session,
            text_calls.inbound_session
        ),
        (
            calls.fresh_account,
            calls.outbound_session,
            calls.inbound_session
        )
    );
    assert_eq!(
        (text_calls.encrypt, text_calls.decrypt),
        (calls.encrypt + 1, calls.decrypt + 1),
        "one renewed-authority Text1, without account/session constructors"
    );
    for (role, engine) in [&p.a, &p.b].into_iter().enumerate() {
        let head = engine.head(p.c)?;
        let native = engine.native(&head)?;
        assert_eq!(
            (head.c, head.d, head.e, &head.sid, head.local_cert),
            (
                heads[role].c,
                heads[role].d,
                heads[role].e,
                &heads[role].sid,
                heads[role].local_cert
            )
        );
        assert_eq!(
            native.account.curve_key(),
            sessions[role].account.curve_key()
        );
        assert_eq!(
            native
                .session
                .as_ref()
                .context("renewed Text1 same session")?
                .keys(),
            sessions[role]
                .session
                .as_ref()
                .context("original session before renewed Text1")?
                .keys()
        );
        assert_eq!(*native.key, *sessions[role].key);
        assert_eq!(native.reservation, sessions[role].reservation);
    }
    Ok(())
}
impl Fixture {
    fn new(parent: &Path, gap: bool) -> Result<Self> {
        Self::new_with_timing(parent, gap, None)
    }
    fn new_with_timing(
        parent: &Path,
        gap: bool,
        timing: Option<(&'static str, Instant)>,
    ) -> Result<Self> {
        Self::new_geometry(parent, gap, timing, None)
    }
    fn new_history(parent: &Path, slot: u8) -> Result<Self> {
        ensure!(slot < 2, "history fixture slot");
        Self::new_geometry(parent, false, None, Some(slot))
    }
    fn new_geometry(
        parent: &Path,
        gap: bool,
        timing: Option<(&'static str, Instant)>,
        history_slot: Option<u8>,
    ) -> Result<Self> {
        controls_phase(timing, "pair-enrollment", "start", 0);
        let mut p = Pair::new_in(parent)?;
        let mut enabled = vec![p.c];
        if let Some(slot) = history_slot {
            let ca = admin::credentials(&p.a.store)?;
            let cb = admin::credentials(&p.b.store)?;
            let wall = now()?;
            let (c2, invitation) = admin::create_context(
                &p.a.store,
                &p.accounts[0],
                &cb.raw,
                cb.cert.account_id(),
                wall,
            )?;
            let (_, complete) = admin::import_context(
                &mut p.b.store,
                Some(&p.accounts[1]),
                &invitation,
                ca.cert.account_id(),
                wall,
            )?;
            admin::import_context(&mut p.a.store, None, &complete, cb.cert.account_id(), wall)?;
            ensure!(c2 != p.c, "two genuine contexts");
            // Pair constructed its Engines before this second real import.
            // Refresh only the cached Specs from committed storage, as Engine::new does.
            for engine in [&mut p.a, &mut p.b] {
                engine.contexts = admin::specs(&engine.store)?
                    .into_iter()
                    .map(|s| (s.c, s))
                    .collect();
                ensure!(
                    engine.contexts.len() == 2,
                    "two committed legacy Specs loaded"
                );
            }
            enabled.push(c2);
            enabled.sort();
            let selected = enabled[usize::from(slot)];
            let mut selected_binding = None;
            for context in &enabled {
                p.c = *context;
                let pair = p.admit()?;
                p.activate(&pair)?;
                if p.c == selected {
                    selected_binding = Some(p.binding);
                }
            }
            p.c = selected;
            p.binding = selected_binding.context("actual selected legacy lease")?;
        }
        controls_phase(timing, "pair-enrollment", "ok", 0);
        controls_phase(timing, "old-message-preparation", "start", 0);
        if history_slot.is_none() {
            let pair = p.admit()?;
            p.activate(&pair)?;
        }
        for (role, request) in [(0, 10), (1, 11), (0, 12), (1, 13)] {
            send_old(&mut p, role, request, true)?;
        }
        if history_slot.is_some() {
            for request in [14, 15] {
                p.a.queue(p.c, [request; 16], "genuine queued migration source")?;
            }
        }
        if gap {
            renew_legacy_authority(&mut p)?;
            send_old(&mut p, 0, 14, false)?;
            let later = send_old(&mut p, 0, 15, false)?;
            let frame = p.a.deliver_frame(p.c, &[later], p.binding, [15; 16])?;
            ensure!(
                p.b.accept_data_frame(p.c, &frame)?.readable == 0,
                "real indexed pending gap source"
            );
        }
        controls_phase(timing, "old-message-preparation", "ok", 0);
        let c = p.c;
        let root_paths = p.accounts.clone();
        let fixture = p._tmp.keep();
        let profiles = [fixture.join("a.profile"), fixture.join("b.profile")];
        let mut owners = Vec::new();
        for (role, engine) in [p.a, p.b].into_iter().enumerate() {
            let path = engine.store.root().to_path_buf();
            let account = AccountId::from_bytes(
                *admin::credentials(&engine.store)?
                    .cert
                    .account_id()
                    .as_bytes(),
            );
            let device = engine.device.identity().device_id();
            drop(engine);
            let config = db::migration7::MigrationConfig {
                logical_quota: [512 * 1024 * 1024; 2],
                physical_capacity: 2 * 1024 * 1024 * 1024,
                output_profile: profiles[role].clone(),
                control_ipc_file: fixture.join(format!("{role}.ipc")),
                locator_directory: fixture.join(format!("{role}.locators")),
            };
            let migration_phase = if role == 0 {
                "migration-a"
            } else {
                "migration-b"
            };
            controls_phase(timing, migration_phase, "start", 0);
            storage_migration::upgrade(
                StateDirectoryLock::acquire(&path)?,
                account,
                device,
                &config,
                false,
                None,
            )?;
            controls_phase(timing, migration_phase, "ok", 0);
            let open_phase = if role == 0 {
                "owner-open-a"
            } else {
                "owner-open-b"
            };
            controls_phase(timing, open_phase, "start", 0);
            let profile = Profile3::load(&profiles[role])?;
            let mut repository = profile.open(&profiles[role])?;
            if let Some(slot) = history_slot {
                ensure!(
                    repository.contexts().len() == 2
                        && repository.contexts()[usize::from(slot)] == (c, role as u8),
                    "actual committed history mapping"
                );
                for physical in 0..2 {
                    repository.cold_ready_snapshot(physical)?;
                    ensure!(
                        repository.source_counts(physical)?.lanes
                            == if physical == slot { [3, 3] } else { [1, 1] },
                        "cold authenticated source frontiers"
                    );
                }
            }
            let mut owner = Owner7::new(repository, &enabled)?;
            if let Some(slot) = history_slot {
                let counts = owner.repo()?.counts(slot)?;
                ensure!(
                    owner.repo()?.metadata(slot)?.next == [3, 3]
                        && counts.queued == if role == 0 { 2 } else { 0 }
                        && counts.pending == 0
                        && counts.debt == 0,
                    "warm migration preserves queued A3/4"
                );
                ensure!(
                    owner.window.snapshot().0 == [64, 64],
                    "two actual enabled shares"
                );
            }
            owners.push(owner);
            controls_phase(timing, open_phase, "ok", 0);
        }
        let mut owners: [Owner7; 2] = owners
            .try_into()
            .map_err(|_| anyhow::anyhow!("two owners"))?;
        controls_phase(timing, "consent-preparation", "start", 0);
        let slot = owners[0]
            .specs
            .iter()
            .position(|s| s.c == c)
            .context("fixture resolved scope")? as u8;
        let scope = owners[0].existing(slot, |s| s.scope(slot))?;
        let now = now()?;
        let certs = scope.certs();
        let origin = scope.origin();
        let origin = match origin {
            v2::Origin::Existing { d, e, sid } => a([u(1), b(d), b(e), b(sid)]),
            _ => anyhow::bail!("genuine existing origin"),
        };
        let body = a([
            u(2),
            b(c),
            b(certs[0]),
            b(certs[1]),
            u(1),
            u(2),
            origin,
            u(1),
            u(now - 1),
            u(now + 86400),
        ]);
        let raw = wire::encode(&body, 4096)?;
        let roots = [
            AccountRootState::load(&root_paths[0])?,
            AccountRootState::load(&root_paths[1])?,
        ];
        let consent = wire::encode(
            &a([
                body,
                b(roots[0].sign_ordered_context_consent2(&raw)),
                b(roots[1].sign_ordered_context_consent2(&raw)),
            ]),
            4096,
        )?;
        for owner in &mut owners {
            owner.existing(slot, |s| {
                s.import_consent(slot, None, consent.clone(), false)
            })?;
        }
        controls_phase(timing, "consent-preparation", "ok", 0);
        Ok(Self {
            owners,
            profiles,
            roots: root_paths,
            consent,
            c,
            timing,
        })
    }
    fn reopen(&mut self, role: usize) -> Result<()> {
        let phase = if role == 0 { "reopen-a" } else { "reopen-b" };
        controls_phase(self.timing, phase, "start", 0);
        let profile = Profile3::load(&self.profiles[role])?;
        let old = self.owners[role]
            .repository
            .take()
            .context("owner repository")?;
        drop(old);
        let open_phase = if role == 0 {
            "owner-open-a"
        } else {
            "owner-open-b"
        };
        controls_phase(self.timing, open_phase, "start", 0);
        self.owners[role] = Owner7::new(profile.open(&self.profiles[role])?, &[self.c])?;
        controls_phase(self.timing, open_phase, "ok", 0);
        controls_phase(self.timing, phase, "ok", 0);
        Ok(())
    }
    fn notify(&mut self, request: [u8; 16], binding: Id) -> Result<v2::VerifiedFrame2> {
        controls_phase(self.timing, "notification-handoff", "start", 0);
        let staged = self.owners[1]
            .existing(0, |s| s.stage_notification(0))?
            .context("B staged notification")?;
        let body = self.owners[1]
            .existing(0, |s| s.handoff_notification(staged, &mut || Ok(())))?
            .context("B handoff")?;
        let notification =
            v2::ParsedFrame2::sign(body, binding, request, self.owners[1].device.identity())?
                .verify(binding, self.owners[1].device.identity().device_id())?;
        controls_phase(self.timing, "notification-handoff", "ok", 0);
        Ok(notification)
    }
    fn complete(&mut self) -> Result<[Vec<u8>; 4]> {
        controls_phase(self.timing, "control-handoff", "start", 0);
        let p = self.owners[0].existing(0, |s| s.propose(0, &mut || Ok(())))?;
        let a = self.owners[1].existing(0, |s| s.prepare(0, &p, &self.consent, &mut || Ok(())))?;
        let k = self.owners[0].existing(0, |s| s.select(0, &a, &mut || Ok(())))?;
        let committed = self.owners[1].existing(0, |s| s.commit_b(0, &k, &mut || Ok(())))?;
        self.owners[0].existing(0, |s| s.activate(0, &committed, &mut || Ok(())))?;
        controls_phase(self.timing, "control-handoff", "ok", 0);
        Ok([p, a, k, committed])
    }
    fn expired_consent(&self) -> Result<Vec<u8>> {
        let v = wire::decode(&self.consent, 4096)?;
        let mut fields = v.fields(3)?[0].fields(10)?.to_vec();
        fields[8] = u(0);
        fields[9] = u(1);
        let body = a(fields);
        let raw = wire::encode(&body, 4096)?;
        let roots = [
            AccountRootState::load(&self.roots[0])?,
            AccountRootState::load(&self.roots[1])?,
        ];
        Ok(wire::encode(
            &a([
                body,
                b(roots[0].sign_ordered_context_consent2(&raw)),
                b(roots[1].sign_ordered_context_consent2(&raw)),
            ]),
            4096,
        )?)
    }
}
fn observed(owner: &mut Owner7) -> Result<Json> {
    observed_at(owner, 0)
}
fn observed_at(owner: &mut Owner7, slot: u8) -> Result<Json> {
    let head = owner.head(slot)?;
    let repo = owner.repo()?;
    let snapshot = repo.snapshot(slot)?;
    let mode = repo.existing_state(slot)?;
    Ok(
        json!({"root":repo.generation(),"head":state::hex(&wire::hash(b"",&head.encode()?)),
        "native":snapshot.native.as_ref().map(|v|state::hex(&wire::hash(b"",v))),
        "meta":snapshot.meta,"counts":repo.counts(slot)?,"state":mode,
        "blocked":repo.control(slot,store::L1Control::BlockedFork)?.map(|v|state::hex(&wire::hash(b"",&v)))}),
    )
}
fn stale_purposes(owner: &mut Owner7) -> Result<()> {
    let head = owner.repo()?.head(0)?;
    let meta = owner.repo()?.metadata(0)?;
    let base = store::ExistingOperation7 {
        context: 0,
        expected_root: owner.repo()?.generation(),
        expected_head: wire::hash(b"", &head),
        policy_generation: meta.policy_generation,
        wall_time: now()?,
        action: store::ExistingAction7::Intent { request: [77; 16] },
    };
    for variant in 0..4 {
        let before = observed(owner)?;
        let mut op = base.clone();
        match variant {
            0 => op.expected_root -= 1,
            1 => op.expected_head = [99; 32],
            2 => op.policy_generation -= 1,
            _ => op.context = 2,
        }
        assert!(
            owner
                .repo()?
                .commit_existing(op, Some(&mut || Ok(())))
                .is_err()
        );
        assert_eq!(
            before,
            observed(owner)?,
            "stale root/head/policy or purpose before retry"
        );
    }
    Ok(())
}
fn notification_negatives(f: &mut Fixture, body: &v2::FrameBody2, request: [u8; 16]) -> Result<()> {
    for field in 0..6 {
        let before = observed(&mut f.owners[0])?;
        let mut fields = body.value().fields(6)?.to_vec();
        fields[field] = match field {
            0 => b([99; 32]),
            1 => u(2),
            _ => opt(None),
        };
        let malformed =
            v2::FrameBody2::parse(v2::FrameKind::OpenIntent, &wire::encode(&a(fields), 512)?)?;
        let frame =
            v2::ParsedFrame2::sign(malformed, [3; 32], request, f.owners[1].device.identity())?
                .verify([3; 32], f.owners[1].device.identity().device_id())?;
        assert!(
            f.owners[0]
                .existing(0, |s| s.accept_peer_notification(0, &frame, &mut || Ok(())))
                .is_err()
        );
        assert_eq!(
            before,
            observed(&mut f.owners[0])?,
            "malformed original scope/ref/phase"
        );
    }
    let before = observed(&mut f.owners[0])?;
    let frame = v2::ParsedFrame2::sign(
        body.clone(),
        [3; 32],
        request,
        f.owners[0].device.identity(),
    )?;
    assert!(
        frame
            .clone()
            .verify([4; 32], f.owners[0].device.identity().device_id())
            .is_err()
    );
    let wrong_role = frame.verify([3; 32], f.owners[0].device.identity().device_id())?;
    assert!(
        f.owners[0]
            .existing(0, |s| s.accept_peer_notification(
                0,
                &wrong_role,
                &mut || Ok(())
            ))
            .is_err()
    );
    assert_eq!(before, observed(&mut f.owners[0])?);
    Ok(())
}
fn scheduler_cases() -> Result<()> {
    let t = Instant::now();
    let at = |n| t + Duration::from_secs(n);
    let mut s = ExistingScheduler7::default();
    s.eligibility(t, true)?;
    assert_eq!(
        s.decision(at(4), store::ExistingMode::L1, false, false, false),
        ScheduleDecision7::Wait
    );
    let p = s.start(at(5), store::ExistingMode::L1, false, false, false)?;
    assert_eq!(p.deadline, at(50));
    assert_eq!(s.quiet_deadline(p, at(40))?, at(50));
    assert!(s.gate(p, at(50), store::ExistingMode::L1).is_err());
    assert!(s.resolved(p, at(50), store::ExistingMode::L1)?);
    assert_eq!(
        s.decision(at(65), store::ExistingMode::L1, false, true, false),
        ScheduleDecision7::Ordinary
    );
    s.ordinary_finished(at(51))?;
    assert_eq!(
        s.decision(at(79), store::ExistingMode::L1, false, true, false),
        ScheduleDecision7::Wait
    );
    let p2 = s.start(at(80), store::ExistingMode::L1, false, true, false)?;
    assert!(!s.resolved(p, at(81), store::ExistingMode::L1)?);
    assert!(s.gate(p2, at(126), store::ExistingMode::Offering).is_ok());
    s.resolved(p2, at(126), store::ExistingMode::Offering)?;
    assert_eq!(
        s.decision(at(500), store::ExistingMode::Offering, false, true, false),
        ScheduleDecision7::Existing
    );
    let mut slow = ExistingScheduler7::default();
    slow.eligibility(t, true)?;
    assert_eq!(
        slow.decision(at(5), store::ExistingMode::L1, false, true, false),
        ScheduleDecision7::Ordinary
    );
    assert_eq!(
        slow.decision(at(90), store::ExistingMode::L1, false, true, false),
        ScheduleDecision7::ResolveOrdinary
    );
    assert_eq!(
        slow.decision(at(90), store::ExistingMode::L1, true, true, true),
        ScheduleDecision7::Wait
    );
    slow.ordinary_finished(at(90))?;
    let q = slow.start(at(90), store::ExistingMode::L1, true, true, false)?;
    slow.resolved(q, at(91), store::ExistingMode::L1)?;
    assert_eq!(
        slow.decision(at(92), store::ExistingMode::L1, true, true, false),
        ScheduleDecision7::Ordinary
    );
    slow.ordinary_finished(at(92))?;
    assert_eq!(
        slow.decision(at(120), store::ExistingMode::L1, true, true, false),
        ScheduleDecision7::Wait
    );
    let mut fair = ExistingFairTurn7::default();
    assert_eq!(
        [
            fair.choose([true, true]),
            fair.choose([true, true]),
            fair.choose([true, true])
        ],
        [Some(0), Some(1), Some(0)]
    );
    Ok(())
}

fn controls_scenario(logical_case: &'static str, initiation: &'static str) -> Result<()> {
    let binding = Binding::managed(logical_case)?;
    let timing = Some((initiation, Instant::now()));
    controls_phase(timing, "scenario", "start", 0);
    scheduler_cases()?;
    let mut retained = Vec::new();
    let mut traces = Vec::new();
    let mut f = Fixture::new_with_timing(&binding.run, false, timing)?;
    let initial_calls = native_call_counts();
    let none = observed(&mut f.owners[0])?;
    assert!(
        f.owners[0]
            .existing(0, |s| s.propose(0, &mut || Ok(())))
            .is_err()
    );
    assert_eq!(
        none,
        observed(&mut f.owners[0])?,
        "Consent2 creates no intent/P"
    );
    stale_purposes(&mut f.owners[0])?;
    let expired = f.expired_consent()?;
    let consent_ref = v2::ParsedConsent2::parse(&f.consent)?.reference();
    let before = observed(&mut f.owners[0])?;
    assert!(
        f.owners[0]
            .existing(0, |s| s.import_consent(
                0,
                Some(consent_ref),
                expired,
                false
            ))
            .is_err()
    );
    assert_eq!(
        before,
        observed(&mut f.owners[0])?,
        "expired current signed permission"
    );
    if initiation == "A-only" {
        f.owners[0]
            .repo()?
            .verification_existing_io_fault("after_sync", 0)?;
        assert!(
            f.owners[0]
                .existing(0, |s| s.request_upgrade(0, [200; 16]))
                .is_err()
        );
        assert!(
            f.owners[0].repo()?.requires_reopen(),
            "actual I/O ambiguity poisons owner"
        );
        f.owners[0].recover()?;
        f.owners[0].existing(0, |s| s.request_upgrade(0, [200; 16]))?;
        f.owners[0].existing(0, |s| s.cancel(0, [200; 16]))?;
        // All outcomes are paid and bounded. The final live request reaches
        // exactly128 retained IDs; its exact retry remains usable.
        for n in 0..126u16 {
            if n % 16 == 0 {
                controls_phase(timing, "outcome-pairs", "start", n);
            }
            let mut request = [50; 16];
            request[..2].copy_from_slice(&n.to_be_bytes());
            f.owners[0].existing(0, |s| s.request_upgrade(0, request))?;
            f.owners[0].existing(0, |s| s.cancel(0, request))?;
            if (n + 1) % 16 == 0 || n == 125 {
                controls_phase(timing, "outcome-pairs", "ok", n + 1);
            }
        }
    }
    if initiation != "B-only" {
        f.owners[0].existing(0, |s| s.request_upgrade(0, [1; 16]))?;
    }
    if initiation != "A-only" {
        f.owners[1].existing(0, |s| s.request_upgrade(0, [2; 16]))?;
        let unsent = f.owners[1]
            .existing(0, |s| s.stage_notification(0))?
            .context("unsent token")?;
        f.owners[1].existing(0, |s| s.cancel(0, [2; 16]))?;
        assert!(
            f.owners[1]
                .existing(0, |s| s.handoff_notification(unsent, &mut || Ok(())))?
                .is_none()
        );
        f.owners[1].existing(0, |s| s.request_upgrade(0, [22; 16]))?;
        // A cancelled ID remains exact even after a later local request.
        let old = f.owners[1].existing(0, |s| s.cancel(0, [2; 16]))?;
        assert_eq!(
            old.local_intent.context("new local intent")?.request,
            [22; 16]
        );
        let before = observed(&mut f.owners[1])?;
        assert!(
            f.owners[1]
                .existing(0, |s| s.request_upgrade(0, [3; 16]))
                .is_err()
        );
        assert_eq!(before, observed(&mut f.owners[1])?);
        // Lost notification: no peer write, then genuine Root7 cold reopen.
        let staged = f.owners[1]
            .existing(0, |s| s.stage_notification(0))?
            .context("B notification")?;
        notification_negatives(&mut f, staged.body(), [22; 16])?;
        let lost = f.notify([22; 16], [1; 32])?;
        drop(lost);
        f.reopen(1)?;
        assert_eq!(
            f.owners[1].existing(0, |s| s.status(0))?,
            UpgradeStatus7::NotificationSentUnconfirmed
        );
        let notification = f.notify([22; 16], [2; 32])?;
        let unchanged = observed(&mut f.owners[0])?;
        assert!(
            f.owners[0]
                .existing(0, |s| s.accept_peer_notification(
                    0,
                    &notification,
                    &mut || Err(db::Error::PolicyRejected("stale component lease"))
                ))
                .is_err()
        );
        assert_eq!(unchanged, observed(&mut f.owners[0])?);
        f.owners[0].existing(0, |s| {
            s.accept_peer_notification(0, &notification, &mut || Ok(()))
        })?;
        f.reopen(0)?;
        assert!(f.owners[0].repo()?.existing_state(0)?.peer_intent.is_some());
        let repeated = f.notify([22; 16], [4; 32])?;
        let before = observed(&mut f.owners[0])?;
        f.owners[0].existing(0, |s| {
            s.accept_peer_notification(0, &repeated, &mut || Ok(()))
        })?;
        assert_eq!(
            before,
            observed(&mut f.owners[0])?,
            "exact peer retry on a new binding is read-only"
        );
        notification_negatives(&mut f, repeated.frame().body(), [22; 16])?;
        f.owners[1].existing(0, |s| s.cancel(0, [22; 16]))?;
        assert!(
            f.owners[1]
                .existing(0, |s| s.stage_notification(0))?
                .is_none()
        );
    }
    if initiation == "simultaneous" {
        f.owners[0].existing(0, |s| s.cancel(0, [1; 16]))?;
    }
    if initiation == "A-only" {
        assert_eq!(
            f.owners[0].repo()?.existing_state(0)?.local_outcomes.len(),
            128
        );
        let before = observed(&mut f.owners[0])?;
        let mut count = 0;
        assert!(
            f.owners[0]
                .existing(0, |s| s.propose(0, &mut || {
                    count += 1;
                    if count == 3 {
                        Err(db::Error::PolicyRejected("late final component gate"))
                    } else {
                        Ok(())
                    }
                }))
                .is_err()
        );
        assert_eq!(count, 3);
        assert!(!f.owners[0].repo()?.requires_reopen());
        assert_eq!(
            before,
            observed(&mut f.owners[0])?,
            "clean tx abort at final gate"
        );
    }
    let controls = f.complete()?;
    if initiation != "A-only" {
        let scope = f.owners[1].existing(0, |s| s.scope(0))?;
        let ids = scope.old_control_ids();
        let body = v2::FrameBody2::parse(
            v2::FrameKind::OpenIntent,
            &wire::encode(
                &a([
                    b(scope.c()),
                    u(3),
                    opt(Some(b(ids[0]))),
                    opt(Some(b(ids[1]))),
                    opt(Some(b(ids[2]))),
                    opt(Some(b(ids[3]))),
                ]),
                512,
            )?,
        )?;
        let repeat =
            v2::ParsedFrame2::sign(body, [6; 32], [22; 16], f.owners[1].device.identity())?
                .verify([6; 32], f.owners[1].device.identity().device_id())?;
        let before = observed(&mut f.owners[0])?;
        f.owners[0].existing(0, |s| {
            s.accept_peer_notification(0, &repeat, &mut || Ok(()))
        })?;
        assert_eq!(
            before,
            observed(&mut f.owners[0])?,
            "selected-stage notification coalesces without recut/reseal/write"
        );
    }
    let wrong_role_before = [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?];
    assert!(
        f.owners[1]
            .existing(0, |s| s.propose(0, &mut || Ok(())))
            .is_err()
    );
    assert!(
        f.owners[0]
            .existing(0, |s| s.prepare(
                0,
                &controls[0],
                &f.consent,
                &mut || Ok(())
            ))
            .is_err()
    );
    assert!(
        f.owners[1]
            .existing(0, |s| s.select(0, &controls[1], &mut || Ok(())))
            .is_err()
    );
    assert!(
        f.owners[0]
            .existing(0, |s| s.commit_b(0, &controls[2], &mut || Ok(())))
            .is_err()
    );
    assert!(
        f.owners[1]
            .existing(0, |s| s.activate(0, &controls[3], &mut || Ok(())))
            .is_err()
    );
    for owner in &mut f.owners {
        assert!(
            owner
                .repo()?
                .commit(store::L1Operation::InitializeHistory { context: 0 }, None)
                .is_err(),
            "L1-only purpose cannot operate on Selected/Active Root7"
        );
    }
    assert_eq!(
        wrong_role_before,
        [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?],
        "role/purpose checks precede even valid saved-control lookup"
    );
    assert_eq!(
        initial_calls,
        native_call_counts(),
        "controls/conversion create no Native operations"
    );
    for role in 0..2 {
        assert_eq!(
            f.owners[role].repo()?.existing_state(0)?.mode,
            store::ExistingMode::Active
        );
        assert!(f.owners[role].current(0).is_err());
        let before = observed(&mut f.owners[role])?;
        assert!(
            f.owners[role]
                .existing(0, |s| s.cancel(0, [99; 16]))
                .is_err()
        );
        assert_eq!(before, observed(&mut f.owners[role])?);
        f.reopen(role)?;
    }
    let before = [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?];
    assert_eq!(
        f.owners[0].existing(0, |s| s.propose(0, &mut || Ok(())))?,
        controls[0]
    );
    assert_eq!(
        f.owners[1].existing(0, |s| s.prepare(
            0,
            &controls[0],
            &f.consent,
            &mut || Ok(())
        ))?,
        controls[1]
    );
    assert_eq!(
        f.owners[0].existing(0, |s| s.select(0, &controls[1], &mut || Ok(())))?,
        controls[2]
    );
    assert_eq!(
        f.owners[1].existing(0, |s| s.commit_b(0, &controls[2], &mut || Ok(())))?,
        controls[3]
    );
    f.owners[0].existing(0, |s| s.activate(0, &controls[3], &mut || Ok(())))?;
    assert_eq!(
        before,
        [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?],
        "saved retries never recut/reseal/write"
    );
    if initiation != "A-only" {
        let cancelled = f.owners[1].existing(0, |s| s.cancel(0, [22; 16]))?;
        assert_eq!(cancelled.mode, store::ExistingMode::Active);
        assert!(
            cancelled
                .local_outcomes
                .iter()
                .any(|i| i.request == [22; 16] && i.cancelled)
        );
    }
    traces.push(json!({"initiation":initiation,"control_ids":controls.iter().map(|v|state::hex(&wire::hash(b"",v))).collect::<Vec<_>>(),"a":before[0],"b":before[1]}));
    if initiation == "A-only" {
        let root = AccountRootState::load(&f.roots[0])?;
        root.revoke_device(f.owners[0].device.identity().device_id())?;
        let authority = root.authority_snapshot()?.encode()?;
        for owner in &mut f.owners {
            owner.pin_authority(&authority)?;
        }
        let before = observed(&mut f.owners[0])?;
        assert!(
            f.owners[0]
                .existing(0, |s| s.propose(0, &mut || Ok(())))
                .is_err()
        );
        assert_eq!(
            before,
            observed(&mut f.owners[0])?,
            "saved bytes cannot bypass current Root revocation"
        );
        f.reopen(0)?;
    } else {
        let parsed = v2::ParsedControl2::parse(&controls[0])?;
        let mut fields = parsed.body().fields(8)?.to_vec();
        fields[5] = b([99; 32]);
        let fork = v2::ParsedControl2::sign(
            18,
            &wire::encode(&a(fields), 4096)?,
            f.owners[0].device.identity(),
        )?;
        let head = f.owners[1].head(0)?;
        assert!(
            f.owners[1]
                .existing(0, |s| s.prepare(0, fork.raw(), &f.consent, &mut || Ok(())))
                .is_err()
        );
        assert!(
            f.owners[1]
                .repo()?
                .control(0, store::L1Control::BlockedFork)?
                .is_some()
        );
        assert_eq!(
            f.owners[1].head(0)?.encode()?,
            head.encode()?,
            "signed fork preserves mode/Native and blocks C"
        );
        f.reopen(1)?;
    }
    retained.push(f);
    controls_phase(timing, "scenario", "ok", 0);
    binding.finish(json!({"scope":"genuine controls/native component; no TLS","scheduler":"5/60/45/15/30/90 production constants","traces":traces}),|| {drop(retained);Ok(())})
}

#[test]
fn s53_controls_a_only_core() -> Result<()> {
    controls_scenario("C-s53-controls-a-only-01", "A-only")
}

#[test]
fn s53_controls_b_only_core() -> Result<()> {
    controls_scenario("C-s53-controls-b-only-01", "B-only")
}

#[test]
fn s53_controls_simultaneous_core() -> Result<()> {
    controls_scenario("C-s53-controls-simultaneous-01", "simultaneous")
}

#[test]
fn s53_native_core() -> Result<()> {
    let binding = Binding::managed("C-s53-native-01")?;
    let mut f = Fixture::new(&binding.run, true)?;
    let scope = f.owners[0].existing(0, |s| s.scope(0))?;
    let initial_raw = f.owners[0]
        .repo()?
        .event_at(0, 0, 1)?
        .context("initial-authority real Text1")?;
    let renewed_raw = f.owners[0]
        .repo()?
        .event_at(0, 0, 3)?
        .context("renewed-authority real Text1")?;
    let renewed = Event::parse(&renewed_raw, scope.peers()[0].certificate().device_id())?;
    let initial = scope.verify_event(&initial_raw, &scope.peers()[0])?;
    assert_ne!(initial.event().core.author_auth, renewed.core.author_auth);
    let author_raw = f.owners[0]
        .repo()?
        .auth(0, renewed.core.author_auth)?
        .context("retained exact renewed O")?;
    let author = v2::ParsedCredentialBundle::parse(&author_raw)?.verify_root()?;
    assert_eq!(author.cert_ref(), scope.peers()[0].cert_ref());
    assert!(author.authority().revision() > scope.peers()[0].authority().revision());
    let unchanged = [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?];
    scope.verify_event(&renewed_raw, &author)?;
    assert!(
        scope.verify_event(&renewed_raw, &scope.peers()[0]).is_err(),
        "initial O cannot substitute for renewed O"
    );
    assert!(
        scope.verify_event(&initial_raw, &author).is_err(),
        "current same-cert O cannot substitute for initial O"
    );
    let mut signed_mismatch = renewed.clone();
    signed_mismatch.core.author_auth = [99; 32];
    signed_mismatch.sign(f.owners[0].device.identity())?;
    let negative_raw = signed_mismatch.raw()?;
    Event::parse(&negative_raw, f.owners[0].device.identity().device_id())?;
    assert!(
        f.owners[0]
            .repo()?
            .auth(0, signed_mismatch.core.author_auth)?
            .is_none()
    );
    assert!(
        scope.verify_event(&negative_raw, &author).is_err(),
        "genuine Device signature does not authorize a different O"
    );
    assert_eq!(
        unchanged,
        [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?],
        "negative verifier inputs never enter storage or change state"
    );
    f.owners[0].queue(
        0,
        [8; 16],
        "queued through the same-Native handoff".to_owned(),
    )?;
    let a_meta = f.owners[0].repo()?.metadata(0)?;
    let b_meta = f.owners[1].repo()?.metadata(0)?;
    let previous = [
        a_meta.tails[0].context("actual A tail")?,
        b_meta.tails[1].context("actual B tail")?,
    ];
    let cuts = [a_meta.next[0], b_meta.next[1]];
    let pending_ordinal = cuts[0] - 1;
    let pending = f.owners[1]
        .repo()?
        .pending(0, pending_ordinal)?
        .context("actual migrated pending after authority renewal")?;
    assert!(!pending.quarantined);
    f.owners[1].repo()?.commit(
        store::L1Operation::Quarantine {
            context: 0,
            ordinal: pending_ordinal,
        },
        Some(&mut || Ok(())),
    )?;
    assert!(
        f.owners[1]
            .repo()?
            .pending(0, pending_ordinal)?
            .context("same quarantined source")?
            .quarantined
    );
    // The source watermark is the real opposite owner's committed frontier.
    f.owners[1].repo()?.commit(
        store::L1Operation::ObserveHead {
            context: 0,
            next: cuts[0],
            last: Some(previous[0]),
        },
        Some(&mut || Ok(())),
    )?;
    for role in 0..2 {
        f.owners[role]
            .repo()?
            .check_existing_cuts(0, cuts, previous)?;
        let mut own_beyond = cuts;
        own_beyond[role] += 1;
        for invalid in [
            [0, cuts[1]],
            [513, cuts[1]],
            own_beyond,
            [pending_ordinal, cuts[1]],
        ] {
            let before = observed(&mut f.owners[role])?;
            assert!(
                f.owners[role]
                    .repo()?
                    .check_existing_cuts(0, invalid, previous)
                    .is_err()
            );
            assert_eq!(
                before,
                observed(&mut f.owners[role])?,
                "own/peer/pending/quarantine/watermark bound"
            );
        }
        let before = observed(&mut f.owners[role])?;
        let mut wrong = previous;
        wrong[0] = [99; 32];
        assert!(
            f.owners[role]
                .repo()?
                .check_existing_cuts(0, cuts, wrong)
                .is_err()
        );
        assert_eq!(
            before,
            observed(&mut f.owners[role])?,
            "exact equal-next watermark tail"
        );
    }
    let mut facts = Vec::new();
    for role in 0..2 {
        let head = f.owners[role].head(0)?;
        let native = f.owners[role].native(0, &head)?;
        let guard = native.guard.as_ref().context("authentic current Guard1")?;
        assert!(
            guard.own.g > 0
                && guard.peer.g > 0
                && guard.own.seen.len() > 1
                && guard.peer.seen.len() > 1,
            "real rotations and retained Seen keys, including the current open tail"
        );
        facts.push((
            head,
            native.session.as_ref().context("genuine session")?.keys(),
            *native.key,
            native.reservation,
            f.owners[role].repo()?.history(0, None, 1)?,
            f.owners[role].repo()?.history(0, None, 32)?,
        ));
    }
    let calls = native_call_counts();
    let t = Instant::now();
    let mut scheduler = ExistingScheduler7::default();
    scheduler.eligibility(t, true)?;
    let token = scheduler.start(t, store::ExistingMode::L1, true, true, false)?;
    f.owners[0].existing(0, |s| s.request_upgrade(0, [4; 16]))?;
    let p = f.owners[0].existing(0, |s| s.propose(0, &mut || Ok(())))?;
    let durable = f.owners[0].repo()?.existing_state(0)?.mode;
    let frozen_before = observed(&mut f.owners[0])?;
    assert!(
        f.owners[0].existing(0, |s| s.cancel(0, [4; 16])).is_err(),
        "first cancel after P is TooLate"
    );
    assert_eq!(frozen_before, observed(&mut f.owners[0])?);
    assert!(
        !f.owners[0].materialize(0)?,
        "durable P freezes a real accepted queue before Native/lease work"
    );
    scheduler.gate(token, t + Duration::from_secs(46), durable)?;
    scheduler.resolved(token, t + Duration::from_secs(46), durable)?;
    assert_eq!(
        scheduler.decision(t + Duration::from_secs(90), durable, false, true, false),
        ScheduleDecision7::Existing,
        "real durable P wins the stale probe timer"
    );
    let accept = f.owners[1].existing(0, |s| s.prepare(0, &p, &f.consent, &mut || Ok(())))?;
    assert_eq!(
        f.owners[1].repo()?.existing_state(0)?.q,
        None,
        "Prepared B has no synthetic Q"
    );
    let parsed_accept = v2::ParsedControl2::parse(&accept)?;
    let mut wrong = parsed_accept.body().fields(4)?.to_vec();
    wrong[2] = u(1);
    wrong[3] = b([99; 32]);
    let bad = v2::ParsedControl2::sign(
        19,
        &wire::encode(&a(wrong), 4096)?,
        f.owners[1].device.identity(),
    )?;
    let before = observed(&mut f.owners[0])?;
    assert!(
        f.owners[0]
            .existing(0, |s| s.select(0, bad.raw(), &mut || Ok(())))
            .is_err()
    );
    assert_eq!(
        before,
        observed(&mut f.owners[0])?,
        "bad cut before conversion/writes"
    );
    let k = f.owners[0].existing(0, |s| s.select(0, &accept, &mut || Ok(())))?;
    assert_eq!(
        f.owners[0].repo()?.existing_state(0)?.mode,
        store::ExistingMode::Selected
    );
    assert_eq!(
        f.owners[1].repo()?.existing_state(0)?.mode,
        store::ExistingMode::Prepared
    );
    f.reopen(0)?;
    f.reopen(1)?;
    let committed = f.owners[1].existing(0, |s| s.commit_b(0, &k, &mut || Ok(())))?;
    f.reopen(1)?;
    f.owners[0].existing(0, |s| s.activate(0, &committed, &mut || Ok(())))?;
    let mut observations = Vec::new();
    for (role, (old, keys, key, reservation, history, full_history)) in
        facts.into_iter().enumerate()
    {
        f.reopen(role)?;
        let owner = &mut f.owners[role];
        let state = owner.repo()?.existing_state(0)?;
        let head = owner.head(0)?;
        let material = owner.repo()?.owner_material()?;
        let identity = super::native::MixedIdentity7 {
            incarnation: material.incarnation,
            claim: material.nonce,
            source: owner.repo()?.source_id(),
        };
        let raw = owner.repo()?.snapshot(0)?.native.context("Mixed7 bytes")?;
        let mixed = super::native::MixedNative7::open(
            &identity,
            &head,
            state.q.context("Q")?,
            owner.device.encryption(),
            &raw,
        )?;
        assert_eq!(mixed.session.keys(), keys);
        assert_eq!(*mixed.key, key);
        assert_eq!(mixed.reservation, reservation);
        assert_eq!(head.sid, old.sid);
        assert_eq!(head.d, old.d);
        assert_eq!(head.e, old.e);
        assert_eq!(head.native_generation, old.native_generation + 1);
        assert_eq!(head.generation, old.generation + 1);
        assert_eq!(
            owner.repo()?.history(0, None, 1)?,
            history,
            "unchanged HVI1/cursor/history across conversion/reopen"
        );
        assert_eq!(
            owner.repo()?.history(0, None, 32)?,
            full_history,
            "complete original and renewed-authority history survives conversion/reopen"
        );
        let reopened_scope = owner.existing(0, |s| s.scope(0))?;
        for (ordinal, expected) in [(1, &initial_raw), (3, &renewed_raw)] {
            let source = owner
                .repo()?
                .event_at(0, 0, ordinal)?
                .context("immutable historical source after reopen")?;
            assert_eq!(&source, expected);
            let event = Event::parse(&source, reopened_scope.peers()[0].certificate().device_id())?;
            let retained = owner
                .repo()?
                .auth(0, event.core.author_auth)?
                .context("exact historical O after reopen")?;
            let historical = v2::ParsedCredentialBundle::parse(&retained)?.verify_root()?;
            reopened_scope.verify_event(&source, &historical)?;
        }
        owner.existing(0, |s| s.validate_mixed(0))?;
        observations.push(json!({"role":role,"encoding_bumps":1,"same_sid_keys_guard_seen":true,"state":observed(owner)?}));
    }
    assert_eq!(calls, native_call_counts(), "encoding-only Native handoff");
    assert_eq!(
        f.owners[0].repo()?.counts(0)?.queued,
        1,
        "same accepted queue retained across both conversions/reopens"
    );
    let before = [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?];
    assert_eq!(
        f.owners[0].existing(0, |s| s.select(0, &accept, &mut || Ok(())))?,
        k
    );
    assert_eq!(
        f.owners[1].existing(0, |s| s.commit_b(0, &k, &mut || Ok(())))?,
        committed
    );
    assert_eq!(
        before,
        [observed(&mut f.owners[0])?, observed(&mut f.owners[1])?]
    );
    binding.finish(json!({"scope":"same genuine Native component with pending old gap; no TLS","observations":observations,"zero_native_calls":true,
        "renewal":{"initial_O":state::hex(&initial.event().core.author_auth),"renewed_O":state::hex(&renewed.core.author_auth),
            "initial_event":state::hex(&initial.event().id()?),"renewed_event":state::hex(&renewed.id()?),
            "same_certificate":true,"exact_historical_O_verified_after_reopen":true,"signed_mismatch_refused_without_storage":true}}),|| {drop(f);Ok(())})
}
