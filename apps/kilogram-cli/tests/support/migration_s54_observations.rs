//! Bounded decoder of actual child stderr, never expected operation counts.
use super::*;
use std::io::{Read, Seek, SeekFrom};
#[path = "migration_s54_pause_observations.rs"]
mod pauses;
#[path = "migration_s54_observation_types.rs"]
mod types;
pub(super) use pauses::Pending;
pub(super) use types::{AckObservation, PageObservation};

pub(super) fn stats(raw: &Value) -> Result<oracle::Counts> {
    let get = |key: &str| {
        raw.get(key)
            .and_then(Value::as_u64)
            .context("actual Stats7 integer")
    };
    Ok(oracle::Counts {
        encrypt: get("committed_encrypt")?,
        decrypt: get("committed_decrypt")?,
        attempts: get("packet_attempts")?,
        aborted: get("aborted_packet_calls")?,
        ambiguous: get("ambiguous_packet_calls")?,
        accounts: get("account_constructors")?,
        outbound: get("outbound_constructors")?,
        inbound: get("inbound_constructors")?,
    })
}
pub(super) struct Decoder {
    pid: u32,
    role: u8,
    owner: old::Id,
    contexts: [String; 2],
    offset: u64,
    fragment: Vec<u8>,
    key: Option<oracle::Incarnation>,
    sequence: u64,
    counts: oracle::Counts,
    final_seen: bool,
    lines: u64,
    pub pre_q_bindings: BTreeSet<(old::Id, u64)>,
    pub pending: BTreeSet<old::Id>,
    pub pending_rows: BTreeMap<(u8, u8, u64), Pending>,
    pub source64: Option<Value>,
    source_expected: Option<Value>,
    heads: BTreeMap<u8, (old::Id, u64, u64)>,
    normal_pages: BTreeMap<u8, (PageObservation, bool)>,
    page_pause: Option<PageObservation>,
    head_page: Option<PageObservation>,
    first_tail: Option<PageObservation>,
    pub ack_pause: Option<AckObservation>,
    ack_arm: Option<types::AckArm>,
    last_ack_request: Option<(old::Id, [u8; 16])>,
    pub max_packet_delta: u64,
    current_head: Option<(old::Id, u64)>,
    retry_expected: Option<(u64, u64, old::Id, old::Id)>,
    pub live_retry_complete: Option<Value>,
}
impl Decoder {
    pub fn observed_counts(&self) -> Result<oracle::Counts> {
        self.incarnation()?;
        Ok(self.counts)
    }
    pub fn new(pid: u32, owner: old::Id, contexts: [String; 2], role: u8) -> Self {
        Self {
            pid,
            role,
            owner,
            contexts,
            offset: 0,
            fragment: vec![],
            key: None,
            sequence: 0,
            counts: Default::default(),
            final_seen: false,
            lines: 0,
            pre_q_bindings: BTreeSet::new(),
            pending: BTreeSet::new(),
            pending_rows: BTreeMap::new(),
            source64: None,
            source_expected: None,
            heads: BTreeMap::new(),
            normal_pages: BTreeMap::new(),
            page_pause: None,
            head_page: None,
            first_tail: None,
            ack_pause: None,
            ack_arm: None,
            last_ack_request: None,
            max_packet_delta: 0,
            current_head: None,
            retry_expected: None,
            live_retry_complete: None,
        }
    }
    pub fn poll(
        &mut self,
        path: &Path,
        ops: &mut oracle::Operations,
        pages: &mut oracle::Pages,
        heads: &mut u64,
    ) -> Result<()> {
        let mut file = std::fs::File::open(path)?;
        let size = file.metadata()?.len();
        ensure!(
            size >= self.offset && size <= 32 * 1024 * 1024,
            "bounded append-only process log"
        );
        file.seek(SeekFrom::Start(self.offset))?;
        let mut chunk = vec![0; 64 * 1024];
        loop {
            let n = file.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            self.offset += n as u64;
            for byte in &chunk[..n] {
                if *byte == b'\n' {
                    let line = std::mem::take(&mut self.fragment);
                    self.line(
                        std::str::from_utf8(&line)?.trim_end_matches('\r'),
                        ops,
                        pages,
                        heads,
                    )?;
                } else {
                    ensure!(
                        self.fragment.len() < 16 * 1024,
                        "bounded complete observation line"
                    );
                    self.fragment.push(*byte);
                }
            }
        }
        Ok(())
    }
    pub fn line(
        &mut self,
        line: &str,
        ops: &mut oracle::Operations,
        pages: &mut oracle::Pages,
        heads: &mut u64,
    ) -> Result<()> {
        let Some(raw) = line.strip_prefix("s54_observation ") else {
            return Ok(());
        };
        self.lines += 1;
        ensure!(
            self.lines <= 16384 && raw.len() <= 16 * 1024,
            "bounded observer events"
        );
        let value: Value = serde_json::from_str(raw)?;
        let object = value.as_object().context("observation object")?;
        ensure!(object.len() == 6, "exact observation envelope");
        let pid = u32::try_from(value["pid"].as_u64().context("actual observation PID")?)?;
        let owner = state::parse_id(value["owner"].as_str().context("observation owner")?)?;
        let nonce = state::parse_id(value["incarnation"].as_str().context("incarnation nonce")?)?;
        ensure!(
            pid == self.pid && owner == self.owner && !self.final_seen,
            "owned PID/source and live stream"
        );
        let key = oracle::Incarnation { pid, owner, nonce };
        let sequence = value["sequence"].as_u64().context("sequence")?;
        let facts = &value["facts"];
        let kind = value["kind"].as_str().context("kind")?;
        if kind == "start" {
            ensure!(
                self.key.is_none()
                    && sequence == 0
                    && oracle::Counts::read(&facts["counters"])? == Default::default(),
                "unique zero start observation"
            );
            ops.start(key)?;
            pages.start(key)?;
            self.key = Some(key);
            return Ok(());
        }
        ensure!(self.key == Some(key), "stable incarnation, start required");
        if kind == "native_commit" {
            self.context(facts)?;
            let counts = oracle::Counts::read(&facts["counters"])?;
            let generation = facts["generation"]
                .as_u64()
                .context("actual committed generation")?;
            let operation =
                state::parse_id(facts["operation"].as_str().context("commit operation")?)?;
            let delta = counts.delta(self.counts)?;
            ops.commit(key, sequence, generation, operation, counts)?;
            self.max_packet_delta = self.max_packet_delta.max(
                delta
                    .encrypt
                    .checked_add(delta.decrypt)
                    .context("bounded packet delta")?,
            );
            self.sequence = sequence;
            self.counts = counts;
            return Ok(());
        }
        // Non-native observations retain the last native sequence; they cannot
        // silently fill a gap or become an extra Native operation.
        ensure!(
            sequence <= self.sequence,
            "non-native event cannot fill a sequence gap"
        );
        match kind {
            "final" => {
                ensure!(sequence == self.sequence, "exact final Native sequence");
                ensure!(
                    facts["resolved"] == true,
                    "explicit repository resolved outcome"
                );
                ops.finish(key, oracle::Counts::read(&facts["counters"])?)?;
                self.final_seen = true;
            }
            "validated_head" => {
                ensure!(
                    facts.as_object().is_some_and(|m| m.len() == 5),
                    "exact accepted Head facts"
                );
                self.context(facts)?;
                let binding = state::parse_id(facts["binding"].as_str().context("Head binding")?)?;
                let generation = facts["lease_generation"]
                    .as_u64()
                    .context("Head current lease")?;
                ensure!(generation > 0, "nonzero current Head generation");
                let received = facts["received"]
                    .as_u64()
                    .filter(|n| *n > 0 && *n <= 4097)
                    .context("actual accepted Head receive frontier")?;
                let context = self.context_index(facts)?;
                if let Some((old_binding, old_generation, old_received)) = self.heads.get(&context)
                {
                    ensure!(
                        generation > *old_generation
                            || (generation == *old_generation
                                && binding == *old_binding
                                && received >= *old_received),
                        "stale/contradictory Head observation"
                    );
                }
                self.heads.insert(context, (binding, generation, received));
                if facts["context"] == self.contexts[0] {
                    self.current_head = Some((binding, generation));
                }
                if facts["pre_q"] == true && facts["context"] == self.contexts[0] {
                    ensure!(
                        self.pre_q_bindings.len() < 32
                            || self.pre_q_bindings.contains(&(binding, generation)),
                        "bounded pre-Q connections"
                    );
                    self.pre_q_bindings.insert((binding, generation));
                    *heads = heads.checked_add(1).context("Head observations overflow")?;
                } else {
                    ensure!(facts["pre_q"].is_boolean(), "explicit pre-Q classification");
                }
            }
            "saved_pending" => {
                self.saved_pending(facts)?;
            }
            "live_retry_complete" => {
                self.context(facts)?;
                ensure!(
                    self.role == 1
                        && facts["context"] == self.contexts[0]
                        && self.live_retry_complete.is_none()
                        && facts.as_object().is_some_and(|m| m.len() == 8),
                    "one expected C1 B possession observation"
                );
                let (generation, from, first_id, binding) = self
                    .retry_expected
                    .context("unarmed live retry observation")?;
                ensure!(
                    facts["lease_generation"] == generation
                        && facts["from"] == from
                        && facts["first_id"] == state::hex(&first_id)
                        && facts["binding"] == state::hex(&binding)
                        && self.current_head == Some((binding, generation)),
                    "actual retry matches captured source/current lease"
                );
                let normal = facts["normal_first"]
                    .as_u64()
                    .context("actual pre-retry cursor")?;
                let next = facts["next"].as_u64().context("actual retry next")?;
                let ids = facts["ids"]
                    .as_array()
                    .context("actual Complete IDs")?
                    .iter()
                    .map(|id| state::parse_id(id.as_str().context("Complete ID")?))
                    .collect::<Result<Vec<_>>>()?;
                ensure!(
                    (1..512).contains(&from)
                        && normal > from
                        && next > from
                        && next <= from + 16
                        && !ids.is_empty()
                        && ids.len() <= 16
                        && ids[0] == first_id
                        && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
                    "actual bounded first-source possession, not a receipt or readability claim"
                );
                self.live_retry_complete = Some(facts.clone());
            }
            "normal_page" | "head_page" | "replayed_head_page" | "replayed_page" => {
                self.page_facts(kind, facts, sequence)?;
                if facts["context"] == self.contexts[0] {
                    let normal = kind == "normal_page";
                    let new = facts["new_sources"]
                        .as_u64()
                        .context("accepted page novelty")?;
                    ensure!(!normal || new > 0, "forward normal page");
                    pages.accepted(
                        key,
                        facts["local_page"].as_u64().context("local page")?,
                        state::parse_id(facts["operation"].as_str().context("page operation")?)?,
                        !normal,
                        new,
                    )?;
                }
            }
            "source64" => self.source_pause(facts)?,
            "page" => self.page_barrier(facts, sequence)?,
            "ack" => self.ack(facts, sequence)?,
            _ => anyhow::bail!("unknown actual S54 observation kind"),
        }
        Ok(())
    }
    fn context(&self, facts: &Value) -> Result<()> {
        let c = facts["context"]
            .as_str()
            .context("actual observation context")?;
        ensure!(
            self.contexts.iter().any(|v| v == c),
            "unexpected observation context"
        );
        Ok(())
    }
    pub fn has_current_lease(&self, generation: u64) -> bool {
        self.key.is_some() && self.current_head.is_some_and(|(_, g)| g == generation)
    }
    pub fn expect_live_retry(
        &mut self,
        generation: u64,
        from: u64,
        first_id: old::Id,
    ) -> Result<Value> {
        ensure!(
            self.role == 1
                && self.retry_expected.is_none()
                && self.live_retry_complete.is_none()
                && (1..512).contains(&from),
            "one C1 B expected retry"
        );
        let key = self.key.context("actual B start identity")?;
        let (binding, observed) = self.current_head.context("actual B Head required")?;
        ensure!(
            observed == generation,
            "request uses actual current B generation"
        );
        self.retry_expected = Some((generation, from, first_id, binding));
        Ok(json!({"incarnation":state::hex(&key.nonce),"lease_generation":generation,"from":from}))
    }
    pub fn close_killed(
        &mut self,
        ops: &mut oracle::Operations,
        start: (u64, u64),
        stopped: source::End,
        actual: oracle::Counts,
    ) -> Result<()> {
        ensure!(
            self.fragment.is_empty(),
            "incomplete kill-tail observation cannot be guessed"
        );
        let key = self.key.context("current owner start missing")?;
        ensure!(
            !self.final_seen && actual.resolved(),
            "killed owner requires real resolved outcome sample"
        );
        let before = (
            start
                .0
                .checked_add(self.counts.encrypt)
                .context("source sum")?,
            start
                .1
                .checked_add(self.counts.decrypt)
                .context("readable sum")?,
        );
        ops.reconcile_final(
            key,
            stopped.generation,
            stopped.operation,
            before,
            stopped.point,
            actual,
        )?;
        self.final_seen = true;
        Ok(())
    }
}
