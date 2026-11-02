//! Fresh P11 observations; P10 delivery predicates remain unchanged.
use super::*;
use relay::{live, locators, proofs};

type PendingRows = BTreeMap<(u8, u8, u64), observations::Pending>;
pub(super) struct PendingHistory {
    keys: [oracle::Incarnation; 2],
    rows: [PendingRows; 2],
}
impl PendingHistory {
    pub fn capture(
        held: old::Id,
        keys: [oracle::Incarnation; 2],
        decoders: [&observations::Decoder; 2],
    ) -> Result<Self> {
        for (o, d) in decoders.iter().enumerate() {
            ensure!(
                d.incarnation()? == keys[o],
                "original owned pending-history source/PID/nonce"
            );
            if o == 0 {
                // receive2 always persists/emits Pending before drain2
                // decrypts the sole missing original B640 in this P10 lifetime.
                ensure!(
                    d.pending_rows.len() == 1,
                    "actual original B640 pending witness required"
                );
                let row = d
                    .pending_rows
                    .get(&(0, 1, 640))
                    .context("original A/C1/peer B640 pending observation")?;
                ensure!(
                    row.context == 0
                        && row.role == 1
                        && row.ordinal == 640
                        && row.event == held
                        && row.generation > 0
                        && row.operation != [0; 32],
                    "exact original historical B640 pending commit association"
                );
            } else {
                ensure!(
                    d.pending_rows.is_empty(),
                    "no historical pending receive work at original B"
                );
            }
        }
        Ok(Self {
            keys,
            rows: decoders.map(|d| d.pending_rows.clone()),
        })
    }
    pub fn check(&self, owner: usize, d: &observations::Decoder, restart: bool) -> Result<()> {
        let original = self
            .keys
            .get(owner)
            .context("original pending-history owner")?;
        let key = d.incarnation()?;
        if restart {
            ensure!(
                key.owner == original.owner
                    && key.pid > 0
                    && key.nonce != [0; 32]
                    && self.keys.iter().all(|p| p.nonce != key.nonce)
                    && d.pending_rows.is_empty(),
                "fresh zero-work lifetime has no pending observation history"
            );
        } else {
            ensure!(
                key == *original && d.pending_rows == self.rows[owner],
                "unchanged validated historical pending observations on original key"
            );
        }
        Ok(())
    }
}

pub(super) struct Baseline {
    pub rows: [[Value; 2]; 2],
    pub controls: [[Value; 2]; 2],
    pub requests: old::Id,
    pub maps: [[BTreeMap<String, Value>; 2]; 2],
    pub old_keys: [oracle::Incarnation; 2],
    pub old_counts: [oracle::Counts; 2],
    pub old_packets: [Option<observations::PacketObservation>; 2],
    pub pending_history: PendingHistory,
    pub relay_pid: u32,
    pub url: String,
}
pub(super) fn controller(previous: &mut relay::Continuation) -> &mut control::Controller {
    &mut previous.previous.previous.previous.previous.controller
}
pub(super) fn relay_identity(s: &control::Controller, pid: u32, url: &str) -> Result<()> {
    let relay = s.relay.as_ref().context("original live relay")?;
    ensure!(
        relay.child.0.id() == pid && relay.url.as_deref() == Some(url) && s.relay_outcome.is_none(),
        "same original relay PID/URL and no previous finalization"
    );
    Ok(())
}
pub(super) fn fresh(old: [oracle::Incarnation; 2], new: [oracle::Incarnation; 2]) -> Result<()> {
    ensure!(
        new[0].pid != new[1].pid && new[0].owner != new[1].owner,
        "distinct owned pair"
    );
    for (a, b) in old.iter().zip(new) {
        ensure!(
            a.owner == b.owner
                && b.pid > 0
                && b.nonce != [0; 32]
                && old.iter().all(|p| p.nonce != b.nonce),
            "new original source/nonce; PID reuse is permitted"
        );
    }
    ensure!(new[0].nonce != new[1].nonce, "independent new nonces");
    Ok(())
}
pub(super) fn work(
    observed: oracle::Counts,
    packet: Option<observations::PacketObservation>,
    expected: oracle::Counts,
    expected_packet: Option<observations::PacketObservation>,
) -> Result<()> {
    ensure!(
        observed == expected && packet == expected_packet,
        "exact current incarnation work/packet attribution"
    );
    Ok(())
}
pub(super) fn entry(previous: &mut relay::Continuation) -> Result<Baseline> {
    let held = state::parse_id(
        previous.delivery["original_commitment"]["id"]
            .as_str()
            .context("original B640 EventID")?,
    )?;
    let maps = reader::expected(previous)?;
    let rows = serde_json::from_value(previous.relay["final"].clone())?;
    let (old_keys, old_counts, old_packets, relay_pid) = (
        previous.incarnations,
        previous.observed,
        previous.packets,
        previous.relay_pid,
    );
    let retained_url = previous.relay["startup"]["url"]
        .as_str()
        .context("original relay URL")?
        .to_owned();
    let s = controller(previous);
    s.check()?;
    ensure!(
        s.phases.next == 11 && !s.phases.complete() && s.running.iter().all(Option::is_some),
        "original running incomplete11/12"
    );
    let url = relay_owner::url(&retained_url)?;
    relay_identity(s, relay_pid, &url)?;
    ensure!(
        live::keys(s)? == old_keys,
        "original retained P10 running keys"
    );
    let controls = live::controls(s)?;
    proofs::controls(&controls, &controls, proofs::Stage::Final)?;
    for o in 0..2 {
        proofs::work(old_counts[o], o, proofs::Stage::Final, true)?;
        proofs::packet(old_packets[o], o, true)?;
    }
    let pending_history = PendingHistory::capture(
        held,
        old_keys,
        [
            &s.running[0]
                .as_ref()
                .context("original A pending history")?
                .decoder,
            &s.running[1]
                .as_ref()
                .context("original B pending history")?
                .decoder,
        ],
    )?;
    Ok(Baseline {
        rows,
        controls,
        requests: live::requests(s)?,
        maps,
        old_keys,
        old_counts,
        old_packets,
        pending_history,
        relay_pid,
        url,
    })
}
pub(super) fn unchanged(s: &mut control::Controller, base: &Baseline) -> Result<()> {
    s.check()?;
    relay_identity(s, base.relay_pid, &base.url)?;
    ensure!(
        live::controls(s)? == base.controls && live::requests(s)? == base.requests,
        "unchanged final schedules and all original request associations"
    );
    Ok(())
}
fn decoder(
    s: &control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    restart: bool,
) -> Result<()> {
    for (o, key) in keys.iter().enumerate() {
        let run = s.running[o].as_ref().context("same actual live owner")?;
        ensure!(
            run.child.0.id() == key.pid && run.decoder.incarnation()? == *key,
            "same source/PID/nonce"
        );
        base.pending_history.check(o, &run.decoder, restart)?;
        work(
            run.decoder.observed_counts()?,
            run.decoder.packet()?,
            if restart {
                Default::default()
            } else {
                base.old_counts[o]
            },
            if restart { None } else { base.old_packets[o] },
        )?;
    }
    Ok(())
}
pub(super) fn sample(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    restart: bool,
) -> Result<live::Sample> {
    unchanged(s, base)?;
    if restart {
        fresh(base.old_keys, keys)?;
    } else {
        ensure!(keys == base.old_keys, "P10 current keys");
    }
    decoder(s, base, keys, restart)?;
    let mut raw = Vec::new();
    let mut all = Vec::new();
    for o in 0..2 {
        let node = s.view(o, 0)?;
        // Node.status retains A7 sanitize-before-write and A8 around_status hooks.
        let status = node.status()?;
        let rows = [
            node.context(&status)?.clone(),
            s.view(o, 1)?.context(&status)?.clone(),
        ];
        s.original[o]
            .as_ref()
            .context("original owner")?
            .identity(&s.origin.pair.nodes[o], &rows)?;
        pressure::resources(&status, node.slot as usize, false)?;
        for (c, row) in rows.iter().enumerate() {
            let slot = s.view(o, c)?.slot as usize;
            ensure!(
                row["context"] == s.origin.contexts[c]
                    && row["role"] == o
                    && row["slot"] == slot
                    && status["routes"][slot]["slot"] == slot
                    && status["routes"][slot]["policy"] == "relay-only"
                    && (status["routes"][slot]["selected_path"].is_null()
                        || status["routes"][slot]["selected_path"] == "relay")
                    && row["cuts"]
                        == if c == 0 {
                            json!([25, 25])
                        } else {
                            json!([33, 33])
                        },
                "original source/Ready role/slot/current relay permissions and cuts"
            );
            proofs::same(row, &base.rows[o][c], o, c, proofs::Stage::Final)?;
        }
        let counts = observations::stats(&status["stats"])?;
        ensure!(
            counts
                == if restart {
                    Default::default()
                } else {
                    base.old_counts[o]
                },
            "exact last-live Stats work"
        );
        s.counters[o] = counts;
        raw.push(status);
        all.push(rows);
        s.check()?;
        decoder(s, base, keys, restart)?;
    }
    unchanged(s, base)?;
    decoder(s, base, keys, restart)?;
    Ok(live::Sample {
        raw: raw
            .try_into()
            .map_err(|_| anyhow::anyhow!("two statuses"))?,
        rows: all
            .try_into()
            .map_err(|_| anyhow::anyhow!("four logical rows"))?,
    })
}
fn idle(s: &control::Controller, facts: &live::Sample) -> Result<bool> {
    let mut idle = true;
    for o in 0..2 {
        for c in 0..2 {
            idle &= facts.raw[o]["window"][1][s.view(o, c)?.slot as usize] == 0;
        }
    }
    Ok(idle)
}
pub(super) fn admissions(
    facts: &live::Sample,
    pubs: &locators::Publications,
    slots: [[u8; 2]; 2],
    heads: [[Option<(old::Id, u64, u64)>; 2]; 2],
) -> Result<bool> {
    let mut ready = true;
    for o in 0..2 {
        for c in 0..2 {
            ready &= locators::admitted(
                &facts.rows[o][c],
                &facts.raw[o]["routes"][slots[o][c] as usize],
                &pubs.rows[o][c],
                &pubs.rows[1 - o][c],
                heads[o][c],
                true,
            )?;
        }
    }
    Ok(ready)
}
fn ready(
    s: &control::Controller,
    facts: &live::Sample,
    pubs: &locators::Publications,
) -> Result<bool> {
    let mut slots = [[0; 2]; 2];
    let mut heads = [[None; 2]; 2];
    for o in 0..2 {
        for c in 0..2 {
            slots[o][c] = s.view(o, c)?.slot;
            heads[o][c] = s.running[o]
                .as_ref()
                .context("actual current decoder")?
                .decoder
                .current_head(c as u8);
        }
    }
    admissions(facts, pubs, slots, heads)
}
pub(super) fn observe(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    restart: bool,
    pubs: &locators::Publications,
) -> Result<reader::Current> {
    s.wait(45, |s| {
        let facts = sample(s, base, keys, restart)?;
        Ok((idle(s, &facts)? && ready(s, &facts, pubs)?).then_some(()))
    })?;
    for (o, maps) in base.maps.iter().enumerate() {
        for (c, map) in maps.iter().enumerate() {
            sample(s, base, keys, restart)?;
            let rows = early::history(&s.view(o, c)?)?;
            proofs::history(&rows, map, if c == 0 { [641, 641] } else { [33, 33] })?;
            sample(s, base, keys, restart)?;
        }
    }
    s.frozen
        .as_ref()
        .context("original frozen23")?
        .verify(&s.view(0, 0)?)?;
    let facts = sample(s, base, keys, restart)?;
    ensure!(
        idle(s, &facts)? && ready(s, &facts, pubs)?,
        "four fresh current Head/Locator2 admissions and idle paths after histories"
    );
    let mut associations = Vec::new();
    for (o, key) in keys.iter().enumerate() {
        for c in 0..2 {
            let row = &facts.rows[o][c];
            let slot = s.view(o, c)?.slot;
            let (binding, generation, after) = s.running[o]
                .as_ref()
                .context("actual fresh Head owner")?
                .decoder
                .current_head(c as u8)
                .context("actual current admitted Head")?;
            associations.push(json!({"owner":o,"context":row["context"],"slot":slot,
                "source":state::hex(&key.owner),"pid":key.pid,"incarnation":state::hex(&key.nonce),
                "q":row["q"],"cuts":row["cuts"],"controls":row["controls"],
                "current_consent":row["current_consent"],"current_consent2":row["current_consent2"],
                "native":{"keys":row["native"]["keys"]},"next":row["next"],"counts":row["counts"],
                "lease_current":row["lease_current"],"lease_family":row["lease_family"],
                "lease_generation":row["lease_generation"],"own_locator_revision":row["own_locator_revision"],
                "peer_locator_revision":row["peer_locator_revision"],"route":facts.raw[o]["routes"][slot as usize],
                "head":{"binding":state::hex(&binding),"generation":generation,"after":after},
                "window":facts.raw[o]["window"],"resources":facts.raw[o]["resources"]}));
        }
    }
    let mut associations = json!(associations);
    public_fingerprints(&mut associations)?;
    Ok(reader::Current {
        keys,
        rows: facts.rows,
        associations,
    })
}
pub(super) fn publications(
    s: &mut control::Controller,
    base: &Baseline,
    prior: Option<&locators::Publications>,
) -> Result<locators::Publications> {
    s.wait(15, |s| {
        for o in 0..2 {
            for c in 0..2 {
                if !s
                    .view(o, c)?
                    .locators
                    .join(format!("{}.locator2", s.origin.contexts[c]))
                    .is_file()
                {
                    return Ok(None);
                }
            }
        }
        Ok(Some(()))
    })?;
    match prior {
        Some(p) => locators::read_same_route(s, &base.url, p),
        None => locators::read(s, Some(&base.url), None),
    }
}
