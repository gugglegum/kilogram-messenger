//! P2 IPC/binding predicates and a separate read-only stopped warm-A collector.
use super::*;
#[cfg(test)]
#[path = "migration_s54_mixed_boundary_tests.rs"]
pub(super) mod tests;
type Repo = db::migration7::live1::L1Repository;
use kilogram_state::indexed_dialog::migration7::live1 as store;

pub(in super::super) fn live(rows: &[[Value; 2]; 2], contexts: &[String; 2]) -> Result<bool> {
    for (c, context) in contexts.iter().enumerate() {
        let a = &rows[0][c];
        let b = &rows[1][c];
        if a["context"] != *context
            || b["context"] != *context
            || a["role"] != 0
            || a["mode"] != "L1"
            || a["phase"] != 3
            || b["phase"] != 3
            || a["next"]
                != if c == 0 {
                    json!([25, 16])
                } else {
                    json!([1, 1])
                }
            || a["counts"]["queued"] != 0
            || a["counts"]["debt"] != if c == 0 { 1 } else { 0 }
            || a["counts"]["pending"] != if c == 0 { 8 } else { 0 }
            || b["send_next"] != if c == 0 { 25 } else { 1 }
            || b["receive_next"] != if c == 0 { 25 } else { 1 }
            || b["unacked"] != if c == 0 { 9 } else { 0 }
            || b["pending"] != 0
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(in super::super) fn early_a(endpoint: &intervals::Endpoint, source: old::Id) -> Result<()> {
    endpoint.validate(0)?;
    ensure!(
        endpoint.source == source
            && endpoint.lanes[0].next == [17, 16]
            && endpoint.lanes[1].next == [1, 1],
        "P2/P3 retained A remains original17/16 migration"
    );
    Ok(())
}
pub(in super::super) fn bindings(diagnostic: &Value, expected: [Option<old::Id>; 2]) -> Result<()> {
    let rows = diagnostic["actual_lifetimes"]
        .as_array()
        .context("existing actual memory lifetimes")?;
    ensure!(!rows.is_empty(), "P2/P3 memory lifetimes present");
    let mut seen = [0usize; 2];
    for row in rows {
        let owner: usize = row["alias"]["owner"]
            .as_u64()
            .context("memory owner")?
            .try_into()?;
        ensure!(
            owner < 2 && !row["token"].is_null() && row["aborted"] == false,
            "registered healthy memory lifetime"
        );
        seen[owner] += 1;
        if let Some(source) = expected[owner] {
            ensure!(
                row["binding"]["owner"] == owner as u64
                    && serde_json::from_value::<old::Id>(row["binding"]["source"].clone())?
                        == source,
                "P2/P3 all owner lifetimes retain AN source"
            );
        } else {
            ensure!(
                row["binding"].is_null() && row["alias"]["current"] == false,
                "P2 legacy B remains unbound"
            );
        }
    }
    ensure!(
        seen.iter().all(|n| *n > 0),
        "both original owners in memory diagnostic"
    );
    Ok(())
}

#[derive(Clone)]
pub(super) struct OrdinaryPending {
    pub row: boundary::PendingRow,
    pub head: bool,
    pub quarantined: bool,
}
#[derive(Clone)]
pub(super) struct WarmLane {
    pub next: [u64; 2],
    pub counts: [u64; 4], // sources/queue/debt/pending, from authenticated roots
    pub lanes: [u64; 2],
    pub projections: u64,
    pub queues: usize,
    pub debt: Vec<(u64, old::Id)>,
    pub pending: Vec<OrdinaryPending>,
    pub repair_pending: u64,
    pub repair_head: Option<u64>,
    pub blocked: bool,
    pub a24: Option<Vec<u8>>,
    pub local_receipt: Option<Vec<u8>>,
    pub peer_receipts: Vec<Option<Vec<u8>>>,
    pub receipt_counts: [u64; 2],
}
// Same exact row predicate used by the collector; supplied rows in tests are
// observation injection, never positive warm-repository execution evidence.
pub(super) fn verify_rows(b: &Legacy, rows: &[WarmLane; 2]) -> Result<Value> {
    for (c, r) in rows.iter().enumerate() {
        let next = if c == 0 { [25, 16] } else { [1, 1] };
        ensure!(
            r.next == next
                && r.lanes == next
                && r.counts[0] == next.iter().sum::<u64>()
                && r.projections == next.iter().sum::<u64>() - 2
                && r.counts[1] == 0
                && r.queues == 0
                && r.counts[2] == r.debt.len() as u64
                && r.counts[2] == if c == 0 { 1 } else { 0 }
                && r.counts[3] == r.pending.len() as u64
                && r.counts[3] == if c == 0 { 8 } else { 0 }
                && r.repair_pending == r.counts[3]
                && r.repair_head.is_none()
                && !r.blocked,
            "warm A exact frontiers/source roots/queue/debt/pending/repair"
        );
        ensure!(
            r.pending.iter().all(|p| !p.head && !p.quarantined),
            "ordinary pending excludes Head/quarantine"
        );
        if c == 1 {
            ensure!(
                r.receipt_counts == [0, 0]
                    && r.a24.is_none()
                    && r.local_receipt.is_none()
                    && r.peer_receipts.is_empty(),
                "warm C2 initialized zero Text/receipts"
            );
        }
    }
    ensure!(
        rows[0].receipt_counts == [23, 15]
            && rows[0].peer_receipts.len() == 9
            && rows[0].peer_receipts.iter().all(Option::is_none),
        "B16..24 pending/held sources have no readable receipt and exact receipt roots"
    );
    boundary::verify_pending(
        b,
        &rows[0]
            .pending
            .iter()
            .map(|p| p.row.clone())
            .collect::<Vec<_>>(),
    )?;
    b.r1_rows(
        &rows[0].debt,
        rows[0].a24.as_ref(),
        rows[0].local_receipt.as_ref(),
    )
}
fn scope(repo: &Repo, endpoint: &intervals::Endpoint, contexts: &[String; 2]) -> Result<()> {
    early_a(endpoint, repo.source_id())?;
    ensure!(
        repo.contexts().len() == 2 && contexts[0] != contexts[1],
        "warm A exactly two mapped contexts"
    );
    for (i, lane) in endpoint.lanes.iter().enumerate() {
        ensure!(
            state::hex(&lane.context) == contexts[i]
                && repo.contexts().get(lane.slot as usize) == Some(&(lane.context, 0)),
            "warm A original role/context/physical mapping"
        );
    }
    Ok(())
}
pub(super) fn repository(
    repo: &mut Repo,
    endpoint: &intervals::Endpoint,
    contexts: &[String; 2],
    b: &Legacy,
) -> Result<Value> {
    scope(repo, endpoint, contexts)?;
    ensure!(
        repo.initialized()?,
        "warm A must already be initialized; no cold fallback"
    );
    let mut rows = Vec::new();
    for (i, lane) in endpoint.lanes.iter().enumerate() {
        let c = lane.slot;
        let snapshot = repo.snapshot(c)?;
        let spec = Spec::parse(&repo.spec(c)?)?;
        let head = state::Head::parse(&snapshot.head, lane.context, spec.cert_refs()?[0], 0)?;
        ensure!(
            head.phase == 3
                && snapshot.native.is_some()
                && repo.existing_state(c)?.mode == store::ExistingMode::L1,
            "warm A normal Ready L1 snapshot"
        );
        let counts = repo.counts(c)?;
        let roots = repo.source_counts(c)?;
        let repair = repo.repair_state(c)?;
        let mut pending = Vec::new();
        for p in repo.pending_page(c, None, 32)? {
            let exact = repo
                .pending(c, p.ordinal)?
                .context("pending page/point member")?;
            ensure!(
                p.raw == exact.raw
                    && p.state == exact.state
                    && p.head == exact.head
                    && p.quarantined == exact.quarantined
                    && p.original == exact.original
                    && p.admission == exact.admission,
                "same actual pending page/point record"
            );
            let event = old::Event::parse(&p.raw, spec.peers[1].cert.device_id())?;
            pending.push(OrdinaryPending {
                row: boundary::PendingRow {
                    ordinal: p.ordinal,
                    id: event.id()?,
                    raw: p.raw,
                },
                head: p.head,
                quarantined: p.quarantined,
            });
        }
        // Strict source counts bound the whole set; a capped page cannot hide extras.
        let peer_receipts = if i == 0 {
            (16..=24)
                .map(|n| repo.receipt(c, 1, n))
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            vec![]
        };
        rows.push(WarmLane {
            next: snapshot.meta.next,
            counts: [counts.sources, counts.queued, counts.debt, counts.pending],
            lanes: roots.lanes,
            projections: roots.projections,
            queues: repo.queue_page(c, None, 32)?.len(),
            debt: repo.debt_page(c, None, 32)?,
            pending,
            repair_pending: repair.pending,
            repair_head: repair.head,
            blocked: repo
                .control(c, db::migration7::live1::L1Control::BlockedFork)?
                .is_some(),
            a24: repo.event_at(c, 0, 24)?,
            local_receipt: repo.receipt(c, 0, 24)?,
            peer_receipts,
            receipt_counts: counts.receipts,
        });
    }
    let rows = rows
        .try_into()
        .map_err(|_| anyhow::anyhow!("two warm A contexts"))?;
    verify_rows(b, &rows)
}
pub(in super::super) fn collect(
    node: &Node,
    endpoint: &intervals::Endpoint,
    contexts: &[String; 2],
    b: &Legacy,
) -> Result<Value> {
    owned(node)?; // Admission and original-directory policy before private access.
    ensure!(
        node.indexed && node.slot == endpoint.lanes[0].slot,
        "warm A current physical view"
    );
    let profile = Profile3::load(&node.profile)?;
    ensure!(
        profile.account == node.branch.account && profile.device == node.branch.device,
        "warm A exact profile owner"
    );
    let mut repo = profile.open(&node.profile)?;
    repository(&mut repo, endpoint, contexts, b)
} // Exclusive repository/source handles are closed before B conversion.
