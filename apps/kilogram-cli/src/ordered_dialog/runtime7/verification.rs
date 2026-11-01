//! Default-off S54 observations/pressure probes. No wire, Native or storage authority.
use super::*;
use owner::Owner7;
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
mod control;
use control::{Location, Phase, Schedule};
pub(super) mod retry;
#[cfg(test)]
mod tests;

pub(super) struct Observer {
    location: Option<Location>,
    incarnation: Id,
    owner: Id,
    sequence: u64,
    pages: [u64; 2],
    last_page: [Option<(u64, Id)>; 2],
}
impl Observer {
    pub fn new(root: &Path, owner: Id) -> Result<Self> {
        let location = std::env::var_os(control::ENV)
            .map(|p| Location::new(Path::new(&p), root, cfg!(debug_assertions)))
            .transpose()?;
        let incarnation = if location.is_some() {
            admin::random::<32>()?
        } else {
            [0; 32]
        };
        let value = Self {
            location,
            incarnation,
            owner,
            sequence: 0,
            pages: [0; 2],
            last_page: [None; 2],
        };
        if value.enabled() {
            eprintln!(
                "s54_observation {}",
                value.envelope(
                    "start",
                    json!({"counters":counters(&owner::Stats7::default())})
                )
            );
        }
        Ok(value)
    }
    pub fn enabled(&self) -> bool {
        self.location.is_some()
    }
    pub fn retry_nonce(&self) -> Id {
        self.incarnation
    }
    pub fn retry_completed(&self, ticket: &retry::Ticket, ids: &[Id], next: u64) -> Result<()> {
        ensure!(
            self.enabled() && state::parse_id(&ticket.request.incarnation)? == self.incarnation,
            "S54 completion observer incarnation"
        );
        retry::completed_ids(ticket, ids, next)?;
        eprintln!("s54_observation {}",self.envelope("live_retry_complete",json!({"context":state::hex(&ticket.context),"binding":state::hex(&ticket.binding),"lease_generation":ticket.request.lease_generation,"from":ticket.request.from,"normal_first":ticket.normal_first,"first_id":state::hex(&ticket.first_id),"ids":ids.iter().map(|id|state::hex(id)).collect::<Vec<_>>(),"next":next})));
        Ok(())
    }
    fn envelope(&self, kind: &str, facts: Json) -> Json {
        json!({"kind":kind,"pid":std::process::id(),"incarnation":state::hex(&self.incarnation),"owner":state::hex(&self.owner),"sequence":self.sequence,"facts":facts})
    }
    fn schedule(&self, context: Id, role: u8) -> Result<Option<Schedule>> {
        let Some(p) = &self.location else {
            return Ok(None);
        };
        Ok(Some(p.load()?).filter(|s| s.applies(context, role)))
    }
    pub fn native(&mut self, c: Id, report: db::CommitReport, stats: &owner::Stats7) -> Result<()> {
        if !self.enabled() {
            return Ok(());
        }
        ensure!(
            !report.duplicate && self.sequence < 4096,
            "S54 duplicate/bounded Native observation"
        );
        self.sequence += 1;
        eprintln!("s54_observation {}", self.envelope("native_commit", json!({"context":state::hex(&c),"generation":report.generation,"operation":state::hex(&report.commit_id),"counters":counters(stats)})));
        Ok(())
    }
    pub fn finish(&self, stats: &owner::Stats7, resolved: bool) {
        if self.enabled() {
            eprintln!(
                "s54_observation {}",
                self.envelope(
                    "final",
                    json!({"counters":counters(stats),"resolved":resolved})
                )
            );
        }
    }
    pub fn head(&self, context: Id, binding: Id, generation: u64, pre_q: bool) {
        if self.enabled() {
            eprintln!("s54_observation {}", self.envelope("validated_head", json!({"context":state::hex(&context),"binding":state::hex(&binding),"lease_generation":generation,"pre_q":pre_q})));
        }
    }
    pub fn pending(&self, context: Id, role: u8, n: u64, id: Id, report: db::CommitReport) {
        if self.enabled() {
            eprintln!("s54_observation {}", self.envelope("saved_pending", json!({"context":state::hex(&context),"role":role,"ordinal":n,"event":state::hex(&id),"generation":report.generation,"operation":state::hex(&report.commit_id)})));
        }
    }
    fn pause(&self, c: Id, role: u8, gate: PauseKind, facts: Json) -> Result<Option<Pause>> {
        let Some(schedule) = self.schedule(c, role)? else {
            return Ok(None);
        };
        if !gate.active(&schedule) {
            return Ok(None);
        }
        let event = self.envelope(gate.tag(), facts);
        let location = self.location.clone().context("S54 location")?;
        Ok(Some(Pause {
            location,
            event,
            context: c,
            role,
            gate,
            deadline: Instant::now() + Duration::from_millis(schedule.max_pause_ms),
        }))
    }
    pub fn page(&mut self, c: u8, context: Id, role: u8, page: Page) -> Result<Option<PageNotice>> {
        if !self.enabled() {
            return Ok(None);
        }
        ensure!(
            page.start < page.end
                && page.end - page.start <= 16
                && page.new_sources <= page.end - page.start
                && (!page.head || page.end - page.start == 1),
            "S54 actual prefix page"
        );
        let forward = !page.head && page.new_sources > 0;
        let kind = match (page.head, page.new_sources > 0) {
            (true, true) => "head_page",
            (true, false) => "replayed_head_page",
            (false, true) => "normal_page",
            (false, false) => "replayed_page",
        };
        if forward {
            ensure!(
                self.last_page[c as usize]
                    .is_none_or(|(g, id)| page.generation > g && page.operation != id),
                "S54 replayed page observation"
            );
            self.last_page[c as usize] = Some((page.generation, page.operation));
            self.pages[c as usize] = self.pages[c as usize]
                .checked_add(1)
                .context("S54 page overflow")?;
        }
        let number = self.pages[c as usize];
        let facts = json!({"context":state::hex(&context),"local_page":number,"head":page.head,"grant":page.grant.map(|id|state::hex(&id)),"start":page.start,"end":page.end,"new_sources":page.new_sources,"generation":page.generation,"operation":state::hex(&page.operation)});
        let event = self.envelope(kind, facts.clone());
        Ok(Some(PageNotice {
            event,
            pause: if forward {
                self.pause(context, role, PauseKind::Page(number), facts)?
            } else {
                None
            },
        }))
    }
}
fn counters(s: &owner::Stats7) -> Json {
    json!({"encrypt":s.committed_encrypt,"decrypt":s.committed_decrypt,"attempts":s.packet_attempts,"aborted":s.aborted_packet_calls,"ambiguous":s.ambiguous_packet_calls,"accounts":s.account_constructors,"outbound":s.outbound_constructors,"inbound":s.inbound_constructors})
}
fn ack_count(
    kind: kilogram_protocol::ordered_dialog_v2::FrameKind,
    body: &wire::Value,
) -> Result<Option<usize>> {
    use kilogram_protocol::ordered_dialog_v2 as v2;
    if kind != v2::FrameKind::AckBatch {
        return Ok(None);
    }
    let f = body.fields(8)?;
    ensure!(f[4].uint()? < 2, "S54 receipt lane");
    let items = v2::tagged_items(&f[6], 1024)?;
    ensure!(
        items.len() <= 16 && items.iter().all(|i| matches!(i.format, 1 | 2)),
        "S54 actual bounded tagged receipts"
    );
    Ok((!items.is_empty()).then_some(items.len()))
}
pub(super) struct Page {
    pub start: u64,
    pub end: u64,
    pub new_sources: u64,
    pub head: bool,
    pub grant: Option<Id>,
    pub generation: u64,
    pub operation: Id,
}
#[derive(Clone, Copy)]
enum PauseKind {
    Source64,
    Page(u64),
    Ack,
}
impl PauseKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Source64 => "source64",
            Self::Page(_) => "page",
            Self::Ack => "ack",
        }
    }
    fn active(self, s: &Schedule) -> bool {
        match self {
            Self::Source64 => s.hold_source64 && s.phase == Phase::Pressure,
            Self::Page(n) => s.pause_after_normal_page == Some(n),
            Self::Ack => s.hold_ack,
        }
    }
}
pub(super) struct PageNotice {
    event: Json,
    pub pause: Option<Pause>,
}
impl PageNotice {
    pub async fn after_reply(self, connection: &iroh::endpoint::Connection) -> Result<()> {
        eprintln!("s54_observation {}", self.event);
        if let Some(pause) = self.pause {
            pause.wait(connection).await?;
        }
        Ok(())
    }
}
pub(super) struct Pause {
    location: Location,
    event: Json,
    context: Id,
    role: u8,
    gate: PauseKind,
    deadline: Instant,
}
impl Pause {
    fn publish(&self) -> Result<()> {
        self.location.marker(self.gate.tag(), &self.event)?;
        eprintln!("s54_observation {}", self.event);
        Ok(())
    }
    pub async fn wait(self, connection: &iroh::endpoint::Connection) -> Result<()> {
        self.wait_until(async {
            let _ = connection.closed().await;
        })
        .await
    }
    async fn wait_until(self, closed: impl std::future::Future<Output = ()>) -> Result<()> {
        tokio::pin!(closed);
        self.publish()?;
        loop {
            ensure!(
                Instant::now() < self.deadline,
                "S54 bounded observation pause expired"
            );
            let s = self.location.load()?;
            if !s.applies(self.context, self.role) || !self.gate.active(&s) {
                return Ok(());
            }
            tokio::select! {
                _ = &mut closed => anyhow::bail!("S54 connection closed during pause"),
                _ = tokio::time::sleep(Duration::from_millis(50)) => (),
            }
        }
    }
}
// IDs are dropped before their permit. Ownership crosses the owner callback only once.
pub(super) struct Selection64 {
    pub ids: Vec<(u64, Id)>,
    _permit: window::RefPermit2,
    pub pause: Option<Pause>,
}
fn select64(
    window: &Arc<window::RefWindow2>,
    c: u8,
    mut read: impl FnMut(Option<u64>) -> Result<Vec<(u64, Id)>>,
    mut verify: impl FnMut(u64, Id) -> Result<()>,
) -> Result<Selection64> {
    // Never retain the references before their real profile share is owned.
    let permit = window.reserve(c, 64)?;
    let mut ids = Vec::with_capacity(64);
    let mut distinct = BTreeSet::new();
    let mut after = None;
    for _ in 0..2 {
        let rows = read(after)?;
        ensure!(rows.len() == 32, "S54 requires two actual debt pages32");
        for (n, id) in rows {
            ensure!(
                after.is_none_or(|previous| n > previous) && distinct.insert(id),
                "S54 debt order/duplicate"
            );
            verify(n, id)?;
            ids.push((n, id));
            after = Some(n);
        }
    }
    Ok(Selection64 {
        ids,
        _permit: permit,
        pause: None,
    })
}
impl Owner7 {
    pub(super) fn verification_selection(
        &mut self,
        c: u8,
        binding: Id,
    ) -> Result<Option<Selection64>> {
        if !self.verification.enabled() {
            return Ok(None);
        }
        let context = self.spec(c)?.c;
        let role = self.role(c)? as u8;
        let Some(plan) = self.verification.schedule(context, role)? else {
            return Ok(None);
        };
        if !plan.hold_source64 {
            return Ok(None);
        }
        self.check_existing_lease(c, binding)?;
        ensure!(
            self.enabled.len() == 2
                && self.repo()?.existing_state(c)?.mode == store::ExistingMode::Active,
            "S54 Active/two-C pressure only"
        );
        let window = self.window.clone();
        // Split the two repository callbacks through RefCell; neither escapes this turn.
        let owner = std::cell::RefCell::new(self);
        let mut selected = select64(
            &window,
            c,
            |after| Ok(owner.borrow_mut().repo()?.debt_page(c, after, 32)?),
            |n, id| {
                let mut o = owner.borrow_mut();
                let raw = o
                    .repo()?
                    .event_at(c, role, n)?
                    .context("S54 own immutable source")?;
                let (format, _, _) = data2::declared(&raw)?;
                let source = o.verify_source2(
                    c,
                    &kilogram_protocol::ordered_dialog_v2::TaggedItem { format, raw },
                    &std::collections::BTreeMap::new(),
                )?;
                let f = data2::facts(&source)?;
                ensure!(
                    f.id == id && f.meta.n == n && f.meta.role == role && f.meta.kind == 3,
                    "S54 debt/source/context association"
                );
                Ok(())
            },
        )?;
        let o = owner.into_inner();
        selected.pause = o.verification.pause(context, role, PauseKind::Source64, json!({"context":state::hex(&context),"count":selected.ids.len(),"ids_digest":blake3::hash(&serde_json::to_vec(&selected.ids)?).to_hex().to_string()}))?;
        Ok(Some(selected))
    }
    pub(super) fn verification_ack(
        &self,
        c: u8,
        frame: &kilogram_protocol::ordered_dialog_v2::ParsedFrame2,
        query_reply: bool,
    ) -> Result<Option<Pause>> {
        use kilogram_protocol::ordered_dialog_v2::FrameKind;
        if !self.verification.enabled() || frame.body().kind() != FrameKind::AckBatch {
            return Ok(None);
        }
        let Some(count) = ack_count(frame.body().kind(), frame.body().value())? else {
            return Ok(None);
        };
        let f = frame.body().value().fields(8)?;
        self.verification.pause(self.spec(c)?.c, self.role(c)? as u8, PauseKind::Ack, json!({"context":state::hex(&self.spec(c)?.c),"binding":state::hex(&frame.binding()),"request":state::hex(&frame.request()),"query_reply":query_reply,"receipt_count":count,"saved_receipts_digest":blake3::hash(&wire::encode(&f[6],16*1024)?).to_hex().to_string()}))
    }
    pub(super) fn observe_native(&mut self, c: u8, report: db::CommitReport) -> Result<()> {
        if !self.verification.enabled() {
            return Ok(());
        }
        self.verification
            .native(self.spec(c)?.c, report, &self.stats)
    }
    pub(super) fn verification_plan(&self, c: u8) -> Result<Option<Schedule>> {
        self.verification
            .schedule(self.spec(c)?.c, self.role(c)? as u8)
    }
}
