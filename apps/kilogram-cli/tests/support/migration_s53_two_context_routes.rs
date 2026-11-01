//! G3-local public Locator2 observations; no product writer or synthetic runtime.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;

#[cfg(test)]
#[path = "migration_s53_two_context_route_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    RelayOnly,
    Auto,
}
impl Mode {
    pub fn policy(self) -> &'static str {
        match self {
            Self::RelayOnly => "relay-only",
            Self::Auto => "auto",
        }
    }
    pub fn path(self) -> &'static str {
        match self {
            Self::RelayOnly => "relay",
            Self::Auto => "direct",
        }
    }
}

pub(super) fn wall() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}
pub(super) fn read_public(node: &Node, c: &str) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    std::fs::File::open(node.locators.join(format!("{c}.locator2")))?
        .take(8193)
        .read_to_end(&mut raw)?;
    ensure!(raw.len() <= 8192, "G3 public Locator2 cap");
    Ok(raw)
}
pub(super) fn scoped(
    raw: &[u8],
    c: old::Id,
    peers: &[wire::RootVerifiedCredential; 2],
    role: u8,
) -> Result<wire::ParsedRuntimeLocator2> {
    ensure!(role < 2, "G3 locator issuer role");
    let locator = wire::ParsedRuntimeLocator2::parse(raw)?;
    locator.verify(c, &peers[role as usize], &peers[1 - role as usize])?;
    Ok(locator)
}
pub(super) fn relay_locator(
    raw: &[u8],
    c: old::Id,
    peers: &[wire::RootVerifiedCredential; 2],
    role: u8,
    url: &str,
    now: u64,
) -> Result<wire::ParsedRuntimeLocator2> {
    route_locator(raw, c, peers, role, url, now, Mode::RelayOnly)
}
pub(super) fn route_locator(
    raw: &[u8],
    c: old::Id,
    peers: &[wire::RootVerifiedCredential; 2],
    role: u8,
    url: &str,
    now: u64,
    mode: Mode,
) -> Result<wire::ParsedRuntimeLocator2> {
    let locator = scoped(raw, c, peers, role)?;
    let f = locator.fields();
    ensure!(
        f.not_before <= now && now < f.expires,
        "G3 Locator2 current interval"
    );
    ensure!(
        f.relay.as_deref() == Some(url)
            && match mode {
                Mode::RelayOnly => f.ips.is_empty(),
                Mode::Auto =>
                    !f.ips.is_empty() && f.ips.iter().all(|a| a.ip().is_loopback() && a.port() != 0),
            },
        "G3 exact route URL/mode-specific IP policy"
    );
    Ok(locator)
}
pub(super) fn revision(before: &[u8], after: &[u8]) -> Result<()> {
    let a = wire::ParsedRuntimeLocator2::parse(before)?;
    let b = wire::ParsedRuntimeLocator2::parse(after)?;
    ensure!(
        b.fields().revision >= a.fields().revision
            && (b.fields().revision != a.fields().revision || before == after),
        "G3 nonregressing Locator2 revision/equal bytes"
    );
    Ok(())
}
pub(super) fn route_row(status: &Value, slot: u8) -> Result<&Value> {
    let rows = oracle::field(status, "routes")?
        .as_array()
        .context("G3 routes array")?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let s = oracle::number(row, "slot")?;
        ensure!(s < 2 && seen.insert(s), "G3 unique explicit route slots");
    }
    rows.iter()
        .find(|row| row["slot"] == slot)
        .context("G3 required route slot absent")
}
pub(super) fn selected(
    status: &Value,
    m: &oracle::Mapping,
    own: &wire::ParsedRuntimeLocator2,
    peer: &wire::ParsedRuntimeLocator2,
) -> Result<()> {
    selected_mode(status, m, own, peer, Mode::RelayOnly)
}
pub(super) fn selected_mode(
    status: &Value,
    m: &oracle::Mapping,
    own: &wire::ParsedRuntimeLocator2,
    peer: &wire::ParsedRuntimeLocator2,
    mode: Mode,
) -> Result<()> {
    let context = oracle::row(status, &m.context)?;
    ensure!(
        oracle::number(context, "slot")? == u64::from(m.slot),
        "G3 route/context mapping"
    );
    let route = route_row(status, m.slot)?;
    ensure!(
        route["policy"] == mode.policy() && route["selected_path"] == mode.path(),
        "G3 both actual selected mode/path"
    );
    oracle::admitted(status, &m.context)?;
    // Status producer binds publication_ready to own locator.endpoint == runtime endpoint.
    ensure!(
        oracle::number(context, "own_locator_revision")? == own.fields().revision
            && oracle::number(context, "peer_locator_revision")? == peer.fields().revision,
        "G3 current own/imported peer locator revisions"
    );
    Ok(())
}
pub(super) fn both(
    status: &[Option<Value>; 3],
    maps: &[oracle::Mapping],
    c: &str,
    locators: &[wire::ParsedRuntimeLocator2; 2],
) -> Result<()> {
    both_mode(status, maps, c, locators, Mode::RelayOnly)
}
pub(super) fn both_mode(
    status: &[Option<Value>; 3],
    maps: &[oracle::Mapping],
    c: &str,
    locators: &[wire::ParsedRuntimeLocator2; 2],
    mode: Mode,
) -> Result<()> {
    for (i, owner) in [1, 2].into_iter().enumerate() {
        selected_mode(
            status[owner]
                .as_ref()
                .context("G3 actual X/Z route owner")?,
            mapping(maps, owner, c)?,
            &locators[i],
            &locators[1 - i],
            mode,
        )?;
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stored {
    pub own: Vec<u8>,
    pub peer: Vec<u8>,
}
pub(super) fn stored(
    repo: &mut store::L1Repository,
    slot: u8,
    scope: &wire::VerifiedExistingScope1,
    role: u8,
) -> Result<Stored> {
    let saved = Stored {
        own: repo
            .existing_locator(slot, true)?
            .context("G3 stored own Locator2")?,
        peer: repo
            .existing_locator(slot, false)?
            .context("G3 stored peer Locator2")?,
    };
    scoped(&saved.own, scope.c(), scope.peers(), role)?;
    scoped(&saved.peer, scope.c(), scope.peers(), 1 - role)?;
    Ok(saved)
}
pub(super) fn stored_join(saved: &[Value; 3], c: &str, public: &[Vec<u8>; 2]) -> Result<()> {
    for (i, owner) in [1, 2].into_iter().enumerate() {
        let actual: Stored =
            serde_json::from_value(saved[owner]["contexts"][c]["routing"].clone())?;
        ensure!(
            actual.own == public[i] && actual.peer == public[1 - i],
            "G3 owner{owner}/{c} exact stored own/imported peer bytes"
        );
    }
    Ok(())
}
pub(super) fn relay_overlay(before: &Value, url: &str) -> Result<Value> {
    overlay(before, url, Mode::RelayOnly)
}
pub(super) fn overlay(before: &Value, url: &str, mode: Mode) -> Result<Value> {
    wire::canonical_runtime_relay(url)?;
    let mut candidate = before.clone();
    candidate["route"] = json!({"policy":mode.policy(),"relay":url,"bind":match mode {Mode::RelayOnly=>None,Mode::Auto=>Some("127.0.0.1:0")}});
    Ok(candidate)
}
pub(super) fn remove_public(
    owners: &Owners,
    nodes: &[Node; 3],
    maps: &[oracle::Mapping],
    c: &str,
) -> Result<()> {
    owners.stopped()?;
    for owner in [1, 2] {
        mapping(maps, owner, c)?;
        let path = nodes[owner].locators.join(format!("{c}.locator2"));
        ensure!(path.is_file(), "G3 owned prior C2 public file exists");
        std::fs::remove_file(&path)?;
        ensure!(
            !path.exists(),
            "G3 removed owned C2 publication while stopped"
        );
    }
    Ok(())
}
pub(super) fn start_relay(owners: &mut Owners, base: &Path) -> Result<String> {
    owners.stopped()?;
    ensure!(owners.relay.is_none(), "G3 unique relay child");
    let ready = base.join("g3-relay.ready");
    let stop = base.join("g3-relay.stop");
    ensure!(
        !ready.exists() && !stop.exists(),
        "G3 fresh relay public paths"
    );
    let stderr = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(base.join("g3-relay.stderr.log"))?;
    owners.relay = Some(OwnedChild(
        command(
            &workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe"),
        )
        .arg(&ready)
        .arg(&stop)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?,
    ));
    owners.wait(15, || Ok(ready.is_file().then_some(())))?;
    let url = std::fs::read_to_string(ready)?;
    wire::canonical_runtime_relay(&url)?;
    ensure!(
        url.starts_with("http://127.0.0.1:") && url.ends_with('/'),
        "G3 owned loopback relay URL"
    );
    Ok(url)
}
