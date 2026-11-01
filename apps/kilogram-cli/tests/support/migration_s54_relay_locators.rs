//! Public authenticated Locator2 cuts for both contexts, never private-source export.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
#[derive(Clone)]
pub(super) struct Locator {
    pub revision: u64,
    pub endpoint: old::Id,
    pub raw: Vec<u8>,
}
pub(super) struct Publications {
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
    ensure!(
        owner < 2 && raw.len() <= 8192,
        "bounded actual locator/role"
    );
    let parsed = v2::ParsedRuntimeLocator2::parse(raw)?;
    parsed.verify(context, &peers[owner], &peers[1 - owner])?;
    let f = parsed.fields();
    ensure!(
        f.not_before <= now && now < f.expires && f.revision > 0,
        "actual current signed locator interval/revision"
    );
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
pub(super) fn read(
    s: &mut control::Controller,
    url: Option<&str>,
    prior: Option<&Publications>,
) -> Result<Publications> {
    s.check()?;
    let scopes = [scope(s, 0)?, scope(s, 1)?];
    let mut rows = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
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
            contexts.push(verify(
                &raw,
                scope.c(),
                scope.peers(),
                owner,
                now,
                url,
                prior.map(|p| &p.rows[owner][context]),
            )?);
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
    Ok(Publications {
        rows: rows
            .try_into()
            .map_err(|_| anyhow::anyhow!("same two owners"))?,
    })
}
pub(super) fn admitted(
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
