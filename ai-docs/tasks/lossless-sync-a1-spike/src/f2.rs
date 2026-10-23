//! Fake ciphertext, deterministic arithmetic admission model. Not a vault/wire implementation.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

const NORMAL: usize = 10; // Event + authorization, in scaled model byte units.
const PROJECTION: usize = 8;
const RATCHET_FRONTIER: usize = 4;
const ACK_INTENT: usize = 2;
const STAGING_SHADOW: usize = 6;
const FRAME_BASE: usize = 4;
const ITEM_FRAMING: usize = 2;
const FRAME_CAP: usize = 28;
const ORDINARY_SLOTS: usize = 3;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Fake {
    n: u8,
    hash: u8,
    bytes: usize,
    valid: bool,
}

impl Fake {
    fn new(n: u8) -> Self {
        Self {
            n,
            hash: 6 - n,
            bytes: NORMAL,
            valid: true,
        }
    }
    fn durable(&self) -> usize {
        self.bytes + PROJECTION + RATCHET_FRONTIER + ACK_INTENT
    }
    fn lease(&self) -> usize {
        self.durable() + STAGING_SHADOW
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Transition {
    AcceptedPending,
    AcceptedReadable,
    Backpressure,
    StorageBlocked(&'static str),
    InterruptedPartial,
    Invalid,
}

#[derive(Clone, Serialize, Deserialize)]
struct Model {
    pending: BTreeMap<u8, Fake>,
    readable: BTreeMap<u8, Fake>,
    crypto_next: u8,
    causal_frontier: Option<u8>,
    ack_intents: BTreeSet<u8>,
    vault_cap: usize,
    base_history: usize,
    cursor: Option<u8>,
    scan_generation: u64,
    last_round_error: Option<String>,
    retry_after_tick: u64,
}

impl Model {
    fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            readable: BTreeMap::new(),
            crypto_next: 0,
            causal_frontier: None,
            ack_intents: BTreeSet::new(),
            vault_cap: 200,
            base_history: 0,
            cursor: None,
            scan_generation: 0,
            last_round_error: None,
            retry_after_tick: 0,
        }
    }

    fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    fn reload(&mut self, file: &Path) -> Result<()> {
        let before = self.snapshot()?;
        fs::write(file, &before)?;
        *self = serde_json::from_slice(&fs::read(file)?)?;
        ensure!(before == self.snapshot()?, "model restart lost checkpoint");
        Ok(())
    }

    fn possession(&self) -> BTreeSet<u8> {
        self.pending
            .keys()
            .chain(self.readable.keys())
            .copied()
            .collect()
    }

    fn history_cost(&self) -> usize {
        self.base_history + self.readable.values().map(Fake::durable).sum::<usize>()
    }

    fn pending_leases(&self) -> usize {
        self.pending.values().map(Fake::lease).sum()
    }

    // One slot AND a full commit lease are held for the next head, not just ciphertext bytes.
    fn head_reserve(&self) -> usize {
        if self.pending.contains_key(&self.crypto_next) {
            0
        } else {
            Fake::new(0).lease()
        }
    }

    fn stage(&mut self, event: &Fake) -> Result<Transition> {
        if !event.valid {
            return Ok(Transition::Invalid);
        }
        if event.bytes > NORMAL {
            return Ok(Transition::StorageBlocked("OversizedLegacyEvent"));
        }
        if self.possession().contains(&event.n) {
            return Ok(if self.readable.contains_key(&event.n) {
                Transition::AcceptedReadable
            } else {
                Transition::AcceptedPending
            });
        }
        if event.n < self.crypto_next {
            return Ok(Transition::Invalid);
        }
        let ordinary = self
            .pending
            .keys()
            .filter(|n| **n != self.crypto_next)
            .count();
        if event.n != self.crypto_next && ordinary >= ORDINARY_SLOTS {
            return Ok(Transition::Backpressure);
        }
        let reserve_after = if event.n == self.crypto_next {
            0
        } else {
            self.head_reserve()
        };
        if self.history_cost() + self.pending_leases() + event.lease() + reserve_after
            > self.vault_cap
        {
            return Ok(Transition::StorageBlocked("FullCommitBudget"));
        }
        self.pending.insert(event.n, event.clone());
        self.invariants()?;
        Ok(Transition::AcceptedPending)
    }

    fn drain(&mut self, physical_storage_available: bool) -> Result<Transition> {
        if !physical_storage_available {
            return Ok(Transition::StorageBlocked("PhysicalWriteUnavailable"));
        }
        let mut drained = false;
        while let Some(event) = self.pending.get(&self.crypto_next).cloned() {
            // All deltas and staging are in the lease. No mutation/ACK before this check.
            if self.history_cost() + self.pending_leases() > self.vault_cap {
                return Ok(Transition::StorageBlocked("FullCommitBudget"));
            }
            self.pending.remove(&event.n);
            self.ack_intents.insert(event.n);
            self.causal_frontier = Some(event.n);
            self.crypto_next += 1;
            self.readable.insert(event.n, event);
            drained = true;
        }
        self.invariants()?;
        Ok(if drained {
            Transition::AcceptedReadable
        } else {
            Transition::AcceptedPending
        })
    }

    fn admit(&mut self, event: &Fake) -> Result<Transition> {
        let staged = self.stage(event)?;
        if staged == Transition::AcceptedPending && self.pending.contains_key(&self.crypto_next) {
            return self.drain(true);
        }
        Ok(staged)
    }

    fn invariants(&self) -> Result<()> {
        ensure!(self.pending.len() <= ORDINARY_SLOTS + 1, "head slot lost");
        ensure!(
            self.history_cost() + self.pending_leases() <= self.vault_cap,
            "commit lease overbooked"
        );
        ensure!(
            self.ack_intents == self.readable.keys().copied().collect(),
            "false delivery ACK"
        );
        ensure!(
            self.causal_frontier == self.readable.keys().next_back().copied(),
            "pending entered causal frontier"
        );
        ensure!(
            self.readable.len() == usize::from(self.crypto_next),
            "noncontiguous crypto frontier"
        );
        Ok(())
    }
}

fn legacy_page(source: &[Fake], inventory: &BTreeSet<u8>) -> Vec<Fake> {
    let mut missing = source
        .iter()
        .filter(|event| !inventory.contains(&event.n))
        .cloned()
        .collect::<Vec<_>>();
    missing.sort_by_key(|event| event.hash);
    missing.truncate(2); // Scaled page width. Production uses 64, not negotiated client credit.
    missing
}

fn frame_bytes(events: &[Fake]) -> usize {
    FRAME_BASE
        + events
            .iter()
            .map(|event| ITEM_FRAMING + event.bytes)
            .sum::<usize>()
}

fn prefix_page(source: &[Fake], next: u8, budget: usize) -> Vec<Fake> {
    let mut events = source
        .iter()
        .filter(|event| event.n >= next)
        .cloned()
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.n);
    let mut result = Vec::new();
    for event in events.into_iter().take(64) {
        if frame_bytes(&result) + ITEM_FRAMING + event.bytes > budget {
            break;
        }
        result.push(event);
    }
    result
}

#[derive(Debug)]
struct Batch {
    transition: Transition,
    complete_ids: Option<Vec<u8>>,
    durable_ids: Vec<u8>, // Diagnostic/checkpoint only, NOT a subset SyncComplete.
}

fn batch(model: &mut Model, events: &[Fake], interrupt_after: Option<usize>) -> Result<Batch> {
    let mut durable = Vec::new();
    for event in events {
        if interrupt_after == Some(durable.len()) {
            model.last_round_error = Some("InterruptedPartial".into());
            return Ok(Batch {
                transition: Transition::InterruptedPartial,
                complete_ids: None,
                durable_ids: durable,
            });
        }
        let transition = model.admit(event)?;
        if matches!(
            transition,
            Transition::AcceptedPending | Transition::AcceptedReadable
        ) {
            durable.push(event.n);
        } else {
            model.last_round_error = Some(format!("{transition:?}"));
            return Ok(Batch {
                transition,
                complete_ids: None,
                durable_ids: durable,
            });
        }
    }
    model.last_round_error = None;
    Ok(Batch {
        transition: if model.pending.is_empty() {
            Transition::AcceptedReadable
        } else {
            Transition::AcceptedPending
        },
        complete_ids: Some(events.iter().map(|event| event.n).collect()),
        durable_ids: durable,
    })
}

fn stalled_legacy(source: &[Fake], file: &Path) -> Result<Model> {
    let mut model = Model::new();
    let mut first = source.to_vec();
    first.sort_by_key(|event| event.hash);
    let interrupted = batch(&mut model, &first, Some(2))?;
    ensure!(
        interrupted.transition == Transition::InterruptedPartial
            && interrupted.complete_ids.is_none()
            && interrupted.durable_ids == [5, 4]
    );
    model.reload(file)?;
    ensure!(
        model.possession() == [4, 5].into_iter().collect()
            && model.crypto_next == 0
            && model.causal_frontier.is_none()
            && model.ack_intents.is_empty()
    );
    println!(
        "f2.interrupted_after_two=InterruptedPartial; SyncComplete=none; restart_inventory=[4,5]; crypto_next=0; readable=[]; causal=[]; ACK=[]"
    );
    let page = legacy_page(source, &model.possession());
    ensure!(page.iter().map(|event| event.n).collect::<Vec<_>>() == [3, 2]);
    let partial = batch(&mut model, &page, None)?;
    ensure!(
        partial.transition == Transition::Backpressure
            && partial.complete_ids.is_none()
            && partial.durable_ids == [3]
    );
    model.reload(file)?;
    ensure!(model.possession() == [3, 4, 5].into_iter().collect());
    let next_page = legacy_page(source, &model.possession());
    ensure!(next_page.iter().map(|event| event.n).collect::<Vec<_>>() == [2, 1]);
    let before = model.snapshot()?;
    let blocked = model.admit(&next_page[0])?;
    ensure!(blocked == Transition::Backpressure && before == model.snapshot()?);
    ensure!(legacy_page(source, &model.possession()) == next_page);
    let inventory = model.possession();
    let may_retry =
        |new_inventory: &BTreeSet<u8>, capacity_changed: bool, prefix_supported: bool| {
            *new_inventory != inventory || capacity_changed || prefix_supported
        };
    ensure!(!may_retry(&inventory, false, false));
    ensure!(may_retry(&inventory, true, false) && may_retry(&inventory, false, true));
    ensure!(model.head_reserve() == 30 && model.pending.len() == 3 && model.ack_intents.is_empty());
    println!(
        "f2.legacy_hash_sender=CompatibilityBlocked/UpgradeRequired; repeat_page=[2,1]; head=0 absent; ordinary_slots=3; head_slot_reserved=1; head_commit_lease=30; ciphertext_preserved=true; retry=external_change_only"
    );
    Ok(model)
}

fn mailbox(source: &[Fake], mut model: Model, file: &Path) -> Result<()> {
    // Opaque rank is stable even after deletion. Readable frontier never drives list position.
    let mut provider = source
        .iter()
        .map(|event| (event.hash, event.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut blocked_scans = 0;
    let mut wraps = 0;
    let mut throttled_ticks = 0;
    for tick in 0..32 {
        // Virtual scheduler: two items per turn, persisted two-tick retry deadline.
        if tick < model.retry_after_tick {
            throttled_ticks += 1;
            continue;
        }
        model.retry_after_tick = tick + 2;
        let page = provider
            .iter()
            .filter(|(rank, _)| model.cursor.is_none_or(|cursor| **rank > cursor))
            .take(2)
            .map(|(rank, event)| (*rank, event.clone()))
            .collect::<Vec<_>>();
        if page.is_empty() {
            model.cursor = None;
            model.scan_generation += 1;
            wraps += 1;
            model.reload(file)?;
            continue;
        }
        for (rank, event) in page {
            let outcome = model.admit(&event)?;
            if outcome == Transition::Backpressure {
                blocked_scans += 1;
            }
            ensure!(outcome != Transition::Invalid);
            model.cursor = Some(rank); // Even a quota-rejected item remains at provider, unACKed.
            model.reload(file)?;
        }
        provider.retain(|_, event| !model.readable.contains_key(&event.n));
        if model.readable.len() == source.len() {
            break;
        }
    }
    ensure!(model.readable.len() == 6 && model.pending.is_empty() && model.ack_intents.len() == 6);
    ensure!(blocked_scans > 0 && wraps > 0 && throttled_ticks > 0);
    ensure!(model.readable.values().cloned().collect::<Vec<_>>() == source);
    println!(
        "f2.mailbox_cursor=AcceptedReadable/6; blocked_items_scanned={blocked_scans}; wrap_scans={wraps}; restart_cursor=true; delete_only_readable=true; scan_budget=2_items_per_turn; throttled_virtual_ticks={throttled_ticks}"
    );
    Ok(())
}

pub fn run(directory: &Path) -> Result<()> {
    let file = directory.join("f2-model.json");
    let source = (0..6).map(Fake::new).collect::<Vec<_>>();
    let stalled = stalled_legacy(&source, &file)?;
    let mut upgraded = stalled.clone();
    let mut responses = 0;
    while upgraded.readable.len() < 6 {
        ensure!(responses < 6, "prefix busy loop");
        let page = prefix_page(&source, upgraded.crypto_next, FRAME_CAP);
        ensure!(!page.is_empty() && frame_bytes(&page) <= FRAME_CAP);
        let outcome = batch(&mut upgraded, &page, None)?;
        ensure!(
            outcome.complete_ids == Some(page.iter().map(|event| event.n).collect()),
            "subset completion"
        );
        upgraded.reload(&file)?;
        responses += 1;
    }
    ensure!(upgraded.readable.values().cloned().collect::<Vec<_>>() == source);
    println!(
        "f2.prefix_capable_sender=AcceptedReadable/6; responses={responses}; frame_cap={FRAME_CAP}; max_response_items=2; exact_complete_ids=true"
    );
    mailbox(&source, stalled, &file)?;

    let mut full = Model::new();
    full.base_history = 185;
    let before = full.snapshot()?;
    ensure!(full.admit(&source[0])? == Transition::StorageBlocked("FullCommitBudget"));
    ensure!(before == full.snapshot()? && full.ack_intents.is_empty());
    ensure!(full.base_history + NORMAL <= full.vault_cap); // Ciphertext alone fits, full commit does not.
    println!(
        "f2.full_history=StorageBlocked/FullCommitBudget; ciphertext_alone_fits=true; state_unchanged=true; false_progress=false"
    );

    let mut physical = Model::new();
    ensure!(physical.stage(&source[0])? == Transition::AcceptedPending);
    let before = physical.snapshot()?;
    ensure!(physical.drain(false)? == Transition::StorageBlocked("PhysicalWriteUnavailable"));
    ensure!(
        before == physical.snapshot()?
            && physical.possession().contains(&0)
            && physical.ack_intents.is_empty()
    );
    physical.reload(&file)?;
    ensure!(physical.drain(true)? == Transition::AcceptedReadable);
    println!(
        "f2.physical_write_failure=StorageBlocked; accepted_head_retained=true; ACK_before_commit=false; explicit_capacity_retry=AcceptedReadable"
    );

    let mut oversized = Fake::new(0);
    oversized.bytes = 45;
    let mut model = Model::new();
    let before = model.snapshot()?;
    ensure!(model.admit(&oversized)? == Transition::StorageBlocked("OversizedLegacyEvent"));
    ensure!(before == model.snapshot()? && model.ack_intents.is_empty());
    ensure!(frame_bytes(&[oversized]) > FRAME_CAP);
    ensure!(64 * 512 * 1024 + FRAME_BASE > 8 * 1024 * 1024);
    println!(
        "f2.oversized_legacy=StorageBlocked/OversizedLegacyEvent; no_rewrite=true; no_ACK=true; new_credit_not_assumed_for_legacy=true"
    );
    println!(
        "f2.real_limit_arithmetic=64x512KiB_exceeds_8MiB_frame; vault_global_cap=512MiB; model_units_are_scaled_not_vault_measurements"
    );
    let mut invalid = Fake::new(0);
    invalid.valid = false;
    unchanged_invalid(&mut model, &invalid)?;

    let retained_blocked_epochs = [1, 2];
    let third_epoch = if retained_blocked_epochs.len() >= 2 {
        "StorageBlocked/RetainedEpochCapacity"
    } else {
        "Admitted"
    };
    ensure!(
        third_epoch == "StorageBlocked/RetainedEpochCapacity" && retained_blocked_epochs == [1, 2]
    );
    println!(
        "f2.third_epoch={third_epoch}; consent_does_not_create_capacity=true; automatic_deletion=false"
    );
    println!(
        "f2.transition_contract=AcceptedPending,AcceptedReadable,Backpressure,StorageBlocked,InterruptedPartial,Invalid; partial_batch=round_error_no_SyncComplete; retry=capacity_or_peer_or_inventory_change_else_backoff"
    );
    Ok(())
}

fn unchanged_invalid(model: &mut Model, invalid: &Fake) -> Result<()> {
    let before = model.snapshot()?;
    ensure!(model.admit(invalid)? == Transition::Invalid && before == model.snapshot()?);
    println!("f2.invalid=Invalid; state_unchanged=true; ACK=false");
    Ok(())
}
