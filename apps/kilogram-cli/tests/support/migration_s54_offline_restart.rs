//! Owned stopped snapshots only. No lease, secret output or runtime substitute.
use super::*;
use kilogram_cli::ordered_dialog::{handshake::Descriptor, runtime::Profile};
#[cfg(test)]
#[path = "migration_s54_offline_restart_tests.rs"]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct Position {
    next: [u64; 2],
    generations: [u64; 2],
    native_generation: u64,
    sid: String,
}
#[derive(Clone)]
struct Lane {
    position: Position,
    local: Vec<u8>,
    peer: Vec<u8>,
    pending: usize,
    outbox: usize,
}
// Deliberately no Debug/Serialize. Proof and private material remain internal.
#[derive(Clone)]
pub(in super::super) struct Snapshot {
    proof: boundary::Proof,
    lanes: Vec<Lane>,
    profile: Vec<u8>,
    unchanged: BTreeMap<String, (old::Id, u64)>,
}
fn head(ctx: &SavedContext) -> Result<state::Head> {
    let spec = Spec::parse(&ctx.spec)?;
    state::Head::parse(
        &ctx.head,
        ctx.c,
        spec.cert_refs()?[ctx.role as usize],
        ctx.role as u64,
    )
}
impl Snapshot {
    pub fn capture(node: &Node, contexts: &[String; 2]) -> Result<Self> {
        Legacy::with_source(node, |purpose, source| {
            Self::capture_source(
                node,
                contexts,
                purpose,
                source,
                &Profile::load(&node.profile)?,
            )
        })
    }
    pub(super) fn capture_source(
        node: &Node,
        contexts: &[String; 2],
        purpose: Purpose,
        source: &db::migration7::V4Source,
        profile: &Profile,
    ) -> Result<Self> {
        profile.validate()?;
        ensure!(
            profile.enabled_contexts.len() == 2
                && profile.enabled_contexts.iter().collect::<BTreeSet<_>>()
                    == contexts.iter().collect()
                && profile.state_dir.canonicalize()? == node.branch.state.canonicalize()?
                && profile.ipc_file == node.ipc
                && profile.locator_directory == node.locators
                && profile
                    .bind
                    .parse::<std::net::SocketAddr>()?
                    .ip()
                    .is_loopback(),
            "offline exact two-context local profile"
        );
        let proof = boundary::Proof::capture_source(node, contexts, purpose, source)?;
        let witness = &proof.witness;
        let device = DeviceEncryptionIdentity::from_secret_bytes(*witness.encryption);
        let claim = old::decode(&source.required("runtime/od1/profile-claim")?, 1024)?.fields(5)?
            [2]
        .fixed()?;
        let mut lanes = Vec::new();
        for ctx in &witness.contexts {
            let h = head(ctx)?;
            let private = state::PrivateState::open(&h, claim, &device, &ctx.native)?;
            let guard = private.guard.as_ref().context("offline genuine Guard1")?;
            let local = source
                .required(&format!("{}local-locator", engine::prefix(ctx.c)))?
                .to_vec();
            let peer = source
                .required(&format!("{}peer-locator", engine::prefix(ctx.c)))?
                .to_vec();
            let spec = Spec::parse(&ctx.spec)?;
            Descriptor::parse(&spec, ctx.role as u64, &local)?;
            Descriptor::parse(&spec, 1 - ctx.role as u64, &peer)?;
            ensure!(
                kilogram_cli::ordered_dialog::admin::read_public(
                    &node
                        .locators
                        .join(format!("{}.locator", state::hex(&ctx.c))),
                    16 * 1024
                )? == local,
                "offline latest published local descriptor equals stopped storage"
            );
            let entries = source.entries().iter().filter(|e| e.context == Some(ctx.c));
            lanes.push(Lane {
                position: Position {
                    next: [guard.own.next, guard.peer.next],
                    generations: [guard.own.g, guard.peer.g],
                    native_generation: h.native_generation,
                    sid: h.sid.context("offline Ready SID")?,
                },
                local,
                peer,
                pending: entries
                    .clone()
                    .filter(|e| matches!(e.kind, 14 | 15))
                    .count(),
                outbox: entries.filter(|e| e.kind == 19).count(),
            });
        }
        let unchanged = source
            .entries()
            .iter()
            .filter(|e| {
                !(e.kind == 9
                    || e.kind == 13
                    || (e.kind == 23 && e.path.ends_with("/local-locator")))
            })
            .map(|e| (e.path.clone(), (e.content_hash, e.length)))
            .collect();
        Ok(Self {
            proof,
            lanes,
            profile: serde_json::to_vec(profile)?,
            unchanged,
        })
    }
    pub fn witness(&self) -> &Legacy {
        &self.proof.witness
    }
    fn a_state(&self) -> Result<()> {
        self.witness().role(0)?;
        ensure!(
            self.lanes.len() == 2 && self.witness().contexts.len() == 2,
            "offline both contexts"
        );
        for (i, (ctx, lane)) in self.witness().contexts.iter().zip(&self.lanes).enumerate() {
            ensure!(
                head(ctx)?.phase == 3
                    && lane.position.next == if i == 0 { [17, 16] } else { [1, 1] }
                    && lane.pending == 0
                    && (i == 0 || lane.outbox == 0),
                "offline A phase/frontiers/pending/C2 outbox"
            );
        }
        ensure!(self.proof.exact.debt.is_empty(), "offline A empty own debt");
        Ok(())
    }
    fn descriptors(&self, b: &Self, now: u64) -> Result<()> {
        self.witness().role(0)?;
        b.witness().role(1)?;
        ensure!(
            self.lanes.len() == 2 && b.lanes.len() == 2,
            "offline descriptor contexts"
        );
        for i in 0..2 {
            let ctx = &self.witness().contexts[i];
            ensure!(
                ctx.c == b.witness().contexts[i].c && self.lanes[i].peer == b.lanes[i].local,
                "offline peer must be latest actual B local descriptor"
            );
            let spec = Spec::parse(&ctx.spec)?;
            Descriptor::parse(&spec, 1, &self.lanes[i].peer)?.valid_now(now)?;
            let bctx = &b.witness().contexts[i];
            Descriptor::parse(&Spec::parse(&bctx.spec)?, 1, &b.lanes[i].local)?.valid_now(now)?;
            Descriptor::parse(&spec, 0, &self.lanes[i].local)?.valid_now(now)?;
        }
        Ok(())
    }
    pub fn initial(&self, b: &Self, now: u64) -> Result<()> {
        self.a_state()?;
        ensure!(
            self.witness().contexts.iter().all(|c| c.queue.is_empty())
                && self.proof.exact.queue.is_empty(),
            "offline initial queue0"
        );
        self.descriptors(b, now)
    }
    // Legacy schema only; no queued field or current-format key requirement.
    pub fn ready(&self, rows: &[Value; 2], b: &Self, now: u64) -> Result<bool> {
        self.initial(b, now)?;
        for (i, row) in rows.iter().enumerate() {
            let p = &self.lanes[i].position;
            if row["context"] != state::hex(&self.witness().contexts[i].c)
                || row["phase"] != 3
                || row["native_generation"] != p.native_generation
                || row["session_id"] != p.sid
                || row["send_next"] != p.next[0]
                || row["receive_next"] != p.next[1]
                || row["own_generation"] != p.generations[0]
                || row["peer_generation"] != p.generations[1]
                || row["pending"] != 0
                || row["unacked"] != 0
                || !row["status"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("ConnectionBackoff: "))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn finish(
        &self,
        after: Self,
        b: &Self,
        now: u64,
        contexts: &[String; 2],
    ) -> Result<boundary::Checked> {
        self.initial(b, now)?;
        after.a_state()?;
        after.descriptors(b, now)?;
        self.witness().same_identity(after.witness())?;
        ensure!(
            self.profile == after.profile && self.unchanged == after.unchanged,
            "offline unchanged profile and all other durable records"
        );
        for i in 0..2 {
            let a = &self.witness().contexts[i];
            let z = &after.witness().contexts[i];
            let old_head = head(a)?;
            let mut new_head = head(z)?;
            ensure!(
                new_head.generation > old_head.generation,
                "offline ordinary Head renewal"
            );
            // Comparison copy only; never stamp/persist a target Head.
            new_head.generation = old_head.generation;
            ensure!(
                new_head.encode()? == old_head.encode()?
                    && a.native == z.native
                    && a.events == z.events
                    && a.history == z.history
                    && a.receipts == z.receipts
                    && self.lanes[i].position == after.lanes[i].position
                    && self.lanes[i].outbox == after.lanes[i].outbox
                    && self.lanes[i].peer == after.lanes[i].peer,
                "offline unchanged Native/Guard/Head semantics/packets/history/receipts/frontiers"
            );
            let spec = Spec::parse(&a.spec)?;
            let old = Descriptor::parse(&spec, 0, &self.lanes[i].local)?;
            let new = Descriptor::parse(&spec, 0, &after.lanes[i].local)?;
            ensure!(
                new.revision > old.revision && new.raw != old.raw,
                "offline genuine local descriptor renewal"
            );
        }
        // Consume this exact final source proof; the Controller must not recapture.
        after.proof.prepare(boundary::Phase::FirstA, 0, contexts)
    }
}

// Small association to the existing owned records, not a lifecycle registry.
pub(in super::super) struct Life {
    start: usize,
    record: intervals::Started,
    handle: usize,
    old_wait: intervals::Waited,
}
impl Life {
    pub fn new(
        ledger: &intervals::Ledger,
        outcomes: &[Value],
        old: usize,
        start: usize,
        handle: usize,
        until: Instant,
    ) -> Result<Self> {
        let record = ledger
            .starts
            .get(start)
            .context("offline fresh owned start")?;
        let prior = ledger.starts.get(old).context("offline old owned start")?;
        let wait = ledger
            .waits
            .iter()
            .find(|w| w.start == old)
            .context("offline old actual wait")?;
        let outcome = outcomes
            .get(wait.outcome)
            .context("offline actual waited outcome")?;
        ensure!(
            record.owner == 0
                && prior.owner == 0
                && record.endpoint.is_none()
                && prior.endpoint.is_none()
                && record.lifetime == prior.lifetime + 1
                && start > old
                && handle != 0
                && wait.key.is_none()
                && wait.kind == intervals::WaitKind::Killed
                && outcome["owner"] == 0
                && outcome["pid"] == prior.pid
                && outcome["waited"] == true
                && outcome["code"] == wait.code
                && outcome["counter_outcome"] == "frozen process; outside current ledger",
            "offline old wait and fresh legacy owned lifetime association"
        );
        let life = Self {
            start,
            record: record.clone(),
            handle,
            old_wait: wait.clone(),
        };
        life.check(ledger, start, handle, Instant::now(), until)?;
        Ok(life)
    }
    pub fn check(
        &self,
        ledger: &intervals::Ledger,
        start: usize,
        handle: usize,
        now: Instant,
        until: Instant,
    ) -> Result<()> {
        ensure!(
            now < until
                && start == self.start
                && handle == self.handle
                && ledger.starts.get(start) == Some(&self.record)
                && ledger
                    .starts
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, s)| s.owner == 0)
                    .map(|(i, _)| i)
                    == Some(start)
                && !ledger.waits.iter().any(|w| w.start == start)
                && ledger.waits.iter().any(|w| w == &self.old_wait),
            "offline fresh handle/start/wait association and original deadline"
        );
        Ok(())
    }
}
