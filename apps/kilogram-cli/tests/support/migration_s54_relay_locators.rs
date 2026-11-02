//! Public authenticated Locator2 cuts for both contexts, never private-source export.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
#[derive(Clone)]
pub(in super::super) struct Locator {
    pub revision: u64,
    pub endpoint: old::Id,
    pub raw: Vec<u8>,
}
pub(in super::super) struct Publications {
    pub rows: [[Locator; 2]; 2],
}
pub(super) fn verify(
    raw: &[u8],
    context: old::Id,
    peers: &[v2::RootVerifiedCredential; 2],
    owner: usize,
    now: u64,
    url: Option<&str>,
    prior: Option<&Locator>,
) -> Result<Locator> {
    let parsed = authenticated(raw, context, peers, owner, url)?;
    let f = parsed.fields();
    ensure!(
        f.not_before <= now && now < f.expires && f.revision > 0,
        "actual current signed locator interval/revision"
    );
    if let Some(prior) = prior {
        ensure!(
            f.revision > prior.revision && raw != prior.raw.as_slice(),
            "strict route-change revision/content, not stale direct bytes"
        );
    }
    Ok(Locator {
        revision: f.revision,
        endpoint: f.endpoint,
        raw: raw.into(),
    })
}
fn authenticated(
    raw: &[u8],
    context: old::Id,
    peers: &[v2::RootVerifiedCredential; 2],
    owner: usize,
    url: Option<&str>,
) -> Result<v2::ParsedRuntimeLocator2> {
    ensure!(
        owner < 2 && raw.len() <= 8192,
        "bounded actual locator/role"
    );
    let parsed = v2::ParsedRuntimeLocator2::parse(raw)?;
    parsed.verify(context, &peers[owner], &peers[1 - owner])?;
    let f = parsed.fields();
    if let Some(url) = url {
        relay_owner::url(url)?;
        ensure!(
            f.relay.as_deref() == Some(url) && f.ips.is_empty(),
            "authenticated URL-only exact owned relay"
        );
    } else {
        ensure!(
            f.relay.is_none() && !f.ips.is_empty(),
            "actual prior DirectOnly publication"
        );
    }
    Ok(parsed)
}
pub(super) fn verify_same_route(
    raw: &[u8],
    context: old::Id,
    peers: &[v2::RootVerifiedCredential; 2],
    owner: usize,
    now: u64,
    url: &str,
    prior: &Locator,
) -> Result<Locator> {
    // Prior is authenticated evidence, not a new observation at `now`.
    // It may have expired while the original owners were stopped.
    let signed_prior = authenticated(&prior.raw, context, peers, owner, Some(url))?;
    let old = signed_prior.fields();
    ensure!(
        old.revision == prior.revision && old.endpoint == prior.endpoint,
        "prior locator metadata must match its actual authenticated bytes"
    );
    let current = verify(raw, context, peers, owner, now, Some(url), None)?;
    ensure!(
        current.revision >= old.revision
            && (current.revision != old.revision || current.raw == prior.raw),
        "same-route revision cannot regress or change signed content at an equal revision"
    );
    Ok(current)
}
fn scope(s: &control::Controller, context: usize) -> Result<v2::VerifiedExistingScope1> {
    let node = s.view(0, context)?;
    let raw = kilogram_cli::ordered_dialog::admin::read_public(
        &node
            .locators
            .join(&s.origin.contexts[context])
            .join("existing-origin.cbor"),
        v2::MAX_EXISTING_ORIGIN,
    )?;
    let scope = v2::VerifiedExistingScope1::verify_export(&raw)?;
    ensure!(
        state::hex(&scope.c()) == s.origin.contexts[context],
        "original public context scope"
    );
    for owner in 0..2 {
        let peer = scope.peers()[owner].certificate();
        let node = s.view(owner, context)?;
        ensure!(
            state::hex(peer.account_id().as_bytes()) == node.branch.account
                && state::hex(peer.device_id().as_bytes()) == node.branch.device,
            "original public scope/role/account/device"
        );
    }
    Ok(scope)
}
pub(in super::super) fn read(
    s: &mut control::Controller,
    url: Option<&str>,
    prior: Option<&Publications>,
) -> Result<Publications> {
    read_cut(s, url, Comparison::Strict(prior))
}
pub(in super::super) fn read_same_route(
    s: &mut control::Controller,
    url: &str,
    prior: &Publications,
) -> Result<Publications> {
    read_cut(s, Some(url), Comparison::SameRoute(prior))
}
enum Comparison<'a> {
    Strict(Option<&'a Publications>),
    SameRoute(&'a Publications),
}
fn read_cut(
    s: &mut control::Controller,
    url: Option<&str>,
    comparison: Comparison<'_>,
) -> Result<Publications> {
    s.check()?;
    let scopes = [scope(s, 0)?, scope(s, 1)?];
    let mut rows = Vec::new();
    for owner in 0..2 {
        let mut contexts = Vec::new();
        for (context, scope) in scopes.iter().enumerate() {
            s.check()?;
            let node = s.view(owner, context)?;
            let raw = kilogram_cli::ordered_dialog::admin::read_public(
                &node
                    .locators
                    .join(format!("{}.locator2", s.origin.contexts[context])),
                8192,
            )?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            contexts.push(match comparison {
                Comparison::Strict(prior) => verify(
                    &raw,
                    scope.c(),
                    scope.peers(),
                    owner,
                    now,
                    url,
                    prior.map(|p| &p.rows[owner][context]),
                )?,
                Comparison::SameRoute(prior) => verify_same_route(
                    &raw,
                    scope.c(),
                    scope.peers(),
                    owner,
                    now,
                    url.context("explicit same-route relay URL")?,
                    &prior.rows[owner][context],
                )?,
            });
        }
        let pair: [Locator; 2] = contexts
            .try_into()
            .map_err(|_| anyhow::anyhow!("same two contexts"))?;
        ensure!(
            pair[0].endpoint == pair[1].endpoint,
            "both contexts published from same actual endpoint"
        );
        rows.push(pair);
    }
    s.check()?;
    Ok(Publications {
        rows: rows
            .try_into()
            .map_err(|_| anyhow::anyhow!("same two owners"))?,
    })
}
pub(in super::super) fn admitted(
    row: &Value,
    route: &Value,
    own: &Locator,
    peer: &Locator,
    head: Option<(old::Id, u64, u64)>,
    relay: bool,
) -> Result<bool> {
    ensure!(
        row["mode"] == "Active"
            && row["policy_error"].is_null()
            && row["handoff_policy_error"].is_null()
            && route["slot"] == row["slot"]
            && route["policy"] == if relay { "relay-only" } else { "direct-only" },
        "actual Active/current permission/Ready slot/configured phase route"
    );
    let path = if relay { "relay" } else { "direct" };
    if row["lease_current"] != true || route["selected_path"].is_null() {
        return Ok(false);
    }
    ensure!(
        route["selected_path"] == path,
        "actual selected phase path, not configuration alone"
    );
    Ok(row["lease_family"] == 2
        && row["publication_ready"] == true
        && row["publication_error"].is_null()
        && row["existing_origin_ready"] == true
        && row["origin_publication_error"].is_null()
        && row["own_locator_revision"] == own.revision
        && row["peer_locator_revision"] == peer.revision
        && head.is_some_and(|(_, g, _)| row["lease_generation"] == g))
}
impl Publications {
    pub fn facts(&self) -> Value {
        json!(self.rows.each_ref().map(|r|r.each_ref().map(|v|json!({"revision":v.revision,"endpoint":state::hex(&v.endpoint),"raw_blake3":blake3::hash(&v.raw).to_hex().to_string()}))))
    }
}
