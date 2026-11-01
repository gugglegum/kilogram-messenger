//! G3 fixed write-set predicates. Every persisted byte identity comes from fixed_records.
use super::*;
use serde::de::DeserializeOwned;

#[cfg(test)]
#[path = "migration_s53_two_context_persistence_tests.rs"]
mod tests;

pub(super) fn context<'a>(report: &'a Value, c: &str) -> Result<&'a Value> {
    oracle::field(report, "contexts")?
        .get(c)
        .context("G3 stopped context")
}
fn typed<T: DeserializeOwned>(row: &Value, key: &str) -> Result<T> {
    Ok(serde_json::from_value(oracle::field(row, key)?.clone())?)
}
fn packed<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    // Same production envelope and postcard serializer; state-local fixture checks this premise.
    let mut out = b"L1M7".to_vec();
    out.extend(postcard::to_allocvec(value)?);
    Ok(out)
}
fn fingerprint(raw: &[u8]) -> store::L1RecordFingerprint7 {
    store::L1RecordFingerprint7 {
        length: raw.len() as u64,
        digest: *blake3::hash(raw).as_bytes(),
    }
}
pub(super) fn fixed(row: &Value) -> Result<store::L1FixedRecords7> {
    typed(row, "fixed")
}
fn current_lengths(r: &store::L1FixedRecords7) -> Result<u64> {
    [
        (&r.own_locator, 8192),
        (&r.peer_locator, 8192),
        (&r.existing, 24 * 1024),
        (&r.metadata, 32 * 1024),
    ]
    .into_iter()
    .try_fold(0u64, |sum, (v, cap)| {
        let v = v.as_ref().context("G3 required fixed Current record")?;
        ensure!(v.length <= cap, "G3 fixed Current cap");
        sum.checked_add(v.length)
            .context("G3 fixed length sum overflow")
    })
}
pub(super) fn bind(row: &Value) -> Result<()> {
    let r = fixed(row)?;
    current_lengths(&r)?;
    ensure!(
        r.head.length <= 8192 && r.legacy_budget.length <= 4096,
        "G3 fixed Head/budget cap"
    );
    let e: store::ExistingState7 = typed(row, "existing")?;
    let m: store::L1Metadata = typed(row, "metadata")?;
    ensure!(
        r.existing == Some(fingerprint(&packed(&e)?))
            && r.metadata == Some(fingerprint(&packed(&m)?)),
        "G3 actual stored Current encoding identity"
    );
    if !row["routing"].is_null() {
        let routes: routes::Stored = typed(row, "routing")?;
        ensure!(
            r.own_locator == Some(fingerprint(&routes.own))
                && r.peer_locator == Some(fingerprint(&routes.peer)),
            "G3 actual stored locator encoding identity"
        );
    }
    Ok(())
}
fn accounting(report: &Value) -> Result<store::L1Accounting> {
    typed(report, "accounting")
}
fn pair(v: &Value, key: &str) -> Result<[u64; 2]> {
    typed(v, key)
}
pub(super) fn reconcile(report: &Value) -> Result<()> {
    let a = accounting(report)?;
    let credits = oracle::field(report, "credits")?;
    for lane in 0..2 {
        ensure!(
            a.runtime.used[lane].checked_add(a.prepaid_metadata.used[lane])
                == Some(pair(credits, "used")?[lane])
                && a.runtime.reserved[lane].checked_add(a.prepaid_metadata.reserved[lane])
                    == Some(pair(credits, "reserved")?[lane]),
            "G3 lane{lane} runtime/prepaid credit reconciliation"
        );
    }
    Ok(())
}
fn funding(a: &store::L1AccountingGroup, b: &store::L1AccountingGroup) -> Result<()> {
    ensure!(a.records == b.records, "G3 accounting record count");
    for lane in 0..2 {
        let before = a.used[lane]
            .checked_add(a.reserved[lane])
            .context("G3 prior funding overflow")?;
        let after = b.used[lane]
            .checked_add(b.reserved[lane])
            .context("G3 current funding overflow")?;
        ensure!(before == after, "G3 lane{lane} paid funding exact");
    }
    Ok(())
}
fn ledger_fields(a: &Value, b: &Value) -> Result<()> {
    for key in ["repair_free", "grant_unissued", "pending", "head_in_use"] {
        oracle::exact(
            &format!("credits/{key}"),
            oracle::field(oracle::field(a, "credits")?, key)?,
            oracle::field(oracle::field(b, "credits")?, key)?,
        )?;
    }
    Ok(())
}
pub(super) fn data(a: &Value, b: &Value, c2: &str, routing: bool) -> Result<()> {
    for key in ["owner", "authority", "next_author", "initialized"] {
        oracle::exact(key, oracle::field(a, key)?, oracle::field(b, key)?)?;
    }
    let before = oracle::field(a, "contexts")?
        .as_object()
        .context("G3 context object")?;
    let after = oracle::field(b, "contexts")?
        .as_object()
        .context("G3 context object")?;
    ensure!(
        before.keys().eq(after.keys()),
        "G3 stopped context set exact"
    );
    for (c, old) in before {
        let new = &after[c];
        for key in [
            "mapping",
            "identity",
            "policy_generation",
            "repair",
            "history_initialized",
            "stable",
            "sources",
        ] {
            oracle::exact(
                &format!("{c}/{key}"),
                oracle::field(old, key)?,
                oracle::field(new, key)?,
            )?;
        }
        bind(old)?;
        bind(new)?;
        let prior = fixed(old)?;
        let current = fixed(new)?;
        ensure!(
            prior.head == current.head && prior.legacy_budget == current.legacy_budget,
            "G3 {c} raw Head7/LegacyBudget exact"
        );
        let mut e = oracle::field(new, "existing")?.clone();
        let mut m = oracle::field(new, "metadata")?.clone();
        if routing && c == c2 {
            e["locator_revision"] =
                oracle::field(oracle::field(old, "existing")?, "locator_revision")?.clone();
            ensure!(
                oracle::number(&m, "wall_high_water")?
                    >= oracle::number(oracle::field(old, "metadata")?, "wall_high_water")?,
                "G3 {c} nonregressing wall high water"
            );
            m["wall_high_water"] = old["metadata"]["wall_high_water"].clone();
        } else {
            oracle::exact(
                &format!("{c}/fixed"),
                &serde_json::to_value(prior)?,
                &serde_json::to_value(current)?,
            )?;
        }
        oracle::exact(
            &format!("{c}/existing whitelist"),
            oracle::field(old, "existing")?,
            &e,
        )?;
        oracle::exact(
            &format!("{c}/metadata whitelist"),
            oracle::field(old, "metadata")?,
            &m,
        )?;
    }
    ledger_fields(a, b)?;
    reconcile(a)?;
    reconcile(b)
}
pub(super) fn routing(a: &Value, b: &Value, c2: &str, profile_exact: bool) -> Result<()> {
    data(a, b, c2, true)?;
    oracle::exact(
        "routing/config",
        oracle::field(a, "config")?,
        oracle::field(b, "config")?,
    )?;
    oracle::exact(
        "routing/capacity",
        oracle::field(a, "capacity")?,
        oracle::field(b, "capacity")?,
    )?;
    let mut after = oracle::field(b, "profile")?.clone();
    if !profile_exact {
        after["route"] = a["profile"]["route"].clone();
    }
    oracle::exact("routing/profile", oracle::field(a, "profile")?, &after)?;
    ensure!(
        a["backend"]["physical_quota"] == b["backend"]["physical_quota"],
        "G3 route physical quota exact"
    );
    let old = accounting(a)?;
    let new = accounting(b)?;
    ensure!(
        old.prepaid_metadata == new.prepaid_metadata,
        "G3 routing prepaid metadata exact"
    );
    funding(&old.runtime, &new.runtime)?;
    let slot: usize =
        oracle::number(oracle::field(context(a, c2)?, "mapping")?, "slot")?.try_into()?;
    ensure!(slot < 2, "G3 actual routing slot");
    let delta = i128::from(current_lengths(&fixed(context(b, c2)?)?)?)
        - i128::from(current_lengths(&fixed(context(a, c2)?)?)?);
    for lane in 0..2 {
        let wanted = if lane == slot { delta } else { 0 };
        ensure!(
            i128::from(new.runtime.used[lane]) - i128::from(old.runtime.used[lane]) == wanted
                && i128::from(new.runtime.reserved[lane]) - i128::from(old.runtime.reserved[lane])
                    == -wanted,
            "G3 lane{lane} exact four-record routing delta"
        );
    }
    Ok(())
}
pub(super) fn settled(reports: &[Value; 3], maps: &[oracle::Mapping], c2: &str) -> Result<()> {
    for owner in [1, 2] {
        let other = 3 - owner;
        let own = mapping(maps, owner, c2)?;
        let remote = mapping(maps, other, c2)?;
        ensure!(own.role == 1 - remote.role, "G3 settled opposite roles");
        let row = context(&reports[owner], c2)?;
        let peer = context(&reports[other], c2)?;
        ensure!(
            reports[owner]["initialized"] == true && row["history_initialized"] == true,
            "G3 initialized restart domain"
        );
        let meta: store::L1Metadata = typed(row, "metadata")?;
        let other_meta: store::L1Metadata = typed(peer, "metadata")?;
        ensure!(
            meta.request_index_ready && row["existing"]["control_debt"] == false,
            "G3 complete request index/no control debt"
        );
        let role = usize::from(own.role);
        let incoming = 1 - role;
        ensure!(
            meta.next == other_meta.next && meta.tails == other_meta.tails,
            "G3 actual full source frontiers/tails agree"
        );
        let sources = oracle::field(row, "sources")?
            .as_array()
            .context("G3 signed source frontiers")?;
        ensure!(sources.len() == 2, "G3 two source lanes");
        let history: oracle::History = typed(oracle::field(row, "stable")?, "history")?;
        for (lane, source) in sources.iter().enumerate() {
            let next = meta.next[lane];
            ensure!(
                (1..=6).contains(&next)
                    && oracle::number(source, "role")? == lane as u64
                    && oracle::number(source, "next")? == next
                    && oracle::number(source, "ordinal")? == next - 1,
                "G3 source includes ordinal-zero control, not just text history"
            );
            let id = state::parse_id(source["last"].as_str().context("G3 source tail ID")?)?;
            ensure!(
                meta.tails[lane] == Some(id),
                "G3 actual signed source/meta tail"
            );
            let mut ordinals = history
                .values()
                .filter(|r| usize::from(r.role) == lane)
                .map(|r| r.ordinal)
                .collect::<Vec<_>>();
            ordinals.sort_unstable();
            ensure!(
                ordinals == (1..next).collect::<Vec<_>>(),
                "G3 complete text lane after control ordinal0"
            );
            if next == 1 {
                ensure!(oracle::number(source, "kind")? != 3, "G3 control-only lane");
            } else {
                ensure!(
                    history
                        .get(&state::hex(&id))
                        .is_some_and(|r| r.ordinal == next - 1),
                    "G3 terminal text source/history"
                );
            }
        }
        // ACK starts at1: genesis/control ordinal0 has no message receipt.
        ensure!(
            meta.ack_next == meta.next[incoming],
            "G3 unsettled ACK progress owner{owner}"
        );
        ensure!(
            meta.peer_source_next == other_meta.next[usize::from(remote.role)]
                && meta.peer_source_last == other_meta.tails[usize::from(remote.role)],
            "G3 unsettled peer source watermark/tail owner{owner}"
        );
    }
    Ok(())
}
pub(super) fn no_work(live: &Live) -> Result<()> {
    for (owner, status) in live
        .status
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|s| (i, s)))
    {
        for name in [
            "packet_attempts",
            "committed_encrypt",
            "committed_decrypt",
            "materialized",
            "readable",
            "aborted_packet_calls",
            "ambiguous_packet_calls",
            "account_constructors",
            "outbound_constructors",
            "inbound_constructors",
        ] {
            ensure!(
                oracle::number(oracle::field(status, "stats")?, name)? == 0,
                "G3 no replay/constructor owner{owner}/{name}"
            );
        }
    }
    Ok(())
}
pub(super) fn candidate(report: &Value) -> Result<db::migration7::MigrationConfig> {
    let mut c: db::migration7::MigrationConfig = typed(report, "config")?;
    for value in &mut c.logical_quota {
        *value = value
            .checked_add(128 * 1024 * 1024)
            .context("G3 capacity quota overflow")?;
    }
    c.physical_capacity = c
        .physical_capacity
        .checked_add(512 * 1024 * 1024)
        .context("G3 capacity physical overflow")?;
    Ok(c)
}
pub(super) fn desired_profile(
    before: &Value,
    candidate: &db::migration7::MigrationConfig,
) -> Result<Value> {
    let mut out = oracle::field(before, "profile")?.clone();
    out["logical_quota"] = json!(candidate.logical_quota);
    out["physical_capacity"] = json!(candidate.physical_capacity);
    out["config_revision"] = json!(
        oracle::number(oracle::field(before, "capacity")?, "revision")?
            .checked_add(1)
            .context("G3 config revision overflow")?
    );
    Ok(out)
}
pub(super) fn capacity(
    before: &Value,
    after: &Value,
    candidate: &db::migration7::MigrationConfig,
    c2: &str,
) -> Result<()> {
    data(before, after, c2, false)?;
    let prior: db::migration7::MigrationConfig = typed(before, "config")?;
    let expected = candidate_for(&prior)?;
    oracle::exact(
        "capacity/exact positive candidate",
        &serde_json::to_value(expected)?,
        &serde_json::to_value(candidate)?,
    )?;
    oracle::exact(
        "capacity/config",
        &serde_json::to_value(candidate)?,
        oracle::field(after, "config")?,
    )?;
    oracle::exact(
        "capacity/profile route/non-capacity fields",
        &desired_profile(before, candidate)?,
        oracle::field(after, "profile")?,
    )?;
    ensure!(
        after["capacity"]["revision"] == after["profile"]["config_revision"]
            && after["capacity"]["hash"] != before["capacity"]["hash"]
            && after["backend"]["physical_quota"] == candidate.physical_capacity,
        "G3 capacity actual revision/hash/backend"
    );
    let a = accounting(before)?;
    let b = accounting(after)?;
    ensure!(
        a.runtime == b.runtime,
        "G3 Capacity runtime accounting exact after routing bridge"
    );
    funding(&a.prepaid_metadata, &b.prepaid_metadata)
}
fn candidate_for(c: &db::migration7::MigrationConfig) -> Result<db::migration7::MigrationConfig> {
    candidate(&json!({"config":c}))
}
pub(super) fn solo_bridge(before: &Value, live: &Value, c2: &str, public: &[u8]) -> Result<Value> {
    let mut expected = before.clone();
    let current = oracle::row(live, c2)?;
    let locator = wire::ParsedRuntimeLocator2::parse(public)?;
    ensure!(
        current["own_locator_revision"] == locator.fields().revision
            && current["publication_ready"] == true,
        "G3 solo publication/endpoint bridge"
    );
    let row = expected["contexts"].get_mut(c2).context("G3 bridge C2")?;
    let prior = fixed(row)?;
    let mut e: store::ExistingState7 = typed(row, "existing")?;
    let mut m: store::L1Metadata = typed(row, "metadata")?;
    e.locator_revision[0] = locator.fields().revision;
    m.wall_high_water = oracle::number(current, "wall_high_water")?;
    row["routing"]["own"] = json!(public);
    row["existing"] = serde_json::to_value(&e)?;
    row["metadata"] = serde_json::to_value(&m)?;
    let mut f = prior.clone();
    f.own_locator = Some(fingerprint(public));
    f.existing = Some(fingerprint(&packed(&e)?));
    f.metadata = Some(fingerprint(&packed(&m)?));
    row["fixed"] = serde_json::to_value(&f)?;
    let slot: usize = oracle::number(oracle::field(row, "mapping")?, "slot")?.try_into()?;
    ensure!(slot < 2, "G3 bridge actual slot");
    let delta = i128::from(current_lengths(&f)?) - i128::from(current_lengths(&prior)?);
    let mut a = accounting(before)?;
    a.runtime.used[slot] = u64::try_from(i128::from(a.runtime.used[slot]) + delta)?;
    a.runtime.reserved[slot] = u64::try_from(i128::from(a.runtime.reserved[slot]) - delta)?;
    expected["accounting"] = serde_json::to_value(&a)?;
    expected["credits"]["used"] = json!([
        a.runtime.used[0]
            .checked_add(a.prepaid_metadata.used[0])
            .context("G3 bridge used overflow")?,
        a.runtime.used[1]
            .checked_add(a.prepaid_metadata.used[1])
            .context("G3 bridge used overflow")?
    ]);
    expected["credits"]["reserved"] = json!([
        a.runtime.reserved[0]
            .checked_add(a.prepaid_metadata.reserved[0])
            .context("G3 bridge reserve overflow")?,
        a.runtime.reserved[1]
            .checked_add(a.prepaid_metadata.reserved[1])
            .context("G3 bridge reserve overflow")?
    ]);
    expected["generation"] = live["root_generation"].clone();
    routing(before, &expected, c2, true)?;
    Ok(expected)
}
