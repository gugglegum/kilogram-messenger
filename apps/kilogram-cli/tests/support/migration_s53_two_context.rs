//! Integrated G3 scenario; execution still requires explicit managed admission.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
use std::io::Write;
#[path = "migration_s53_two_context_oracle.rs"]
mod oracle;
#[path = "migration_s53_two_context_persistence.rs"]
mod persistence;
#[path = "migration_s53_two_context_finish.rs"]
mod finish;
#[path = "migration_s53_two_context_routes.rs"]
mod routes;
#[cfg(test)]
#[path = "migration_s53_two_context_tests.rs"]
mod tests;
#[path = "migration_s53_two_context_witness.rs"]
mod witness;

#[derive(Default)]
struct Owners {
    children: [Option<OwnedChild>; 3],
    required: [bool; 3],
    relay: Option<OwnedChild>,
}
impl Owners {
    fn alive(&mut self) -> Result<()> {
        s53_deadline::check()?;
        oracle::owner_presence(self.children.each_ref().map(Option::is_some), self.required)?;
        for child in self.children.iter_mut().flatten() {
            child.alive()?;
        }
        if let Some(child) = self.relay.as_mut() {
            child.alive()?;
        }
        Ok(())
    }
    fn require(&mut self, wanted: [bool; 3]) -> Result<()> {
        ensure!(self.required == wanted, "G3 explicit running owner subset");
        self.alive()
    }
    fn insert(&mut self, owner: usize, child: OwnedChild) -> Result<()> {
        ensure!(
            owner < 3 && self.children[owner].is_none(),
            "G3 unique actual child owner"
        );
        self.children[owner] = Some(child);
        self.required[owner] = true;
        Ok(())
    }
    fn start(&mut self, nodes: &[Node; 3], label: &str) -> Result<()> {
        self.start_subset(nodes, label, [true; 3])
    }
    fn start_subset(&mut self, nodes: &[Node; 3], label: &str, wanted: [bool; 3]) -> Result<()> {
        self.stopped()?;
        for (owner, node) in nodes.iter().enumerate().filter(|(i, _)| wanted[*i]) {
            self.alive()?;
            self.insert(owner, node.start(&format!("{label}-{owner}"))?)?;
        }
        self.require(wanted)
    }
    fn stop(&mut self) -> Result<()> {
        for (owner, child) in self.children.iter_mut().enumerate() {
            if let Some(mut child) = child.take() {
                child.kill_wait()?;
            }
            self.required[owner] = false;
        }
        self.stopped()
    }
    fn stopped(&self) -> Result<()> {
        oracle::stopped_owners(self.children.each_ref().map(Option::is_some))
    }
    fn wait<T>(
        &mut self,
        seconds: u64,
        mut observe: impl FnMut() -> Result<Option<T>>,
    ) -> Result<T> {
        let until = Instant::now() + Duration::from_secs(seconds);
        loop {
            s53_deadline::check()?;
            self.alive()?;
            if let Some(v) = observe()? {
                return Ok(v);
            }
            ensure!(Instant::now() < until, "G3 bounded real-owner observation");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn pids(&self) -> Vec<(usize, u32)> {
        self.children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.as_ref().map(|c| (i, c.0.id())))
            .collect()
    }
}
fn hash(raw: &[u8]) -> String {
    blake3::hash(raw).to_hex().to_string()
}
fn phase(managed: &Managed, name: &str, mut value: Value) -> Result<()> {
    s53_deadline::check()?;
    public_fingerprints(&mut value)?;
    let raw = serde_json::to_vec(
        &json!({"case":managed.case,"attempt":managed.run.file_name().context("G3 attempt")?.to_str().context("G3 UTF8")?,"inventory_identity":std::env::var("KILOGRAM_S53_INVENTORY_ID")?,"phase":name,"classification":"G3 public observation; comparisons may follow; not PASS","observations":value}),
    )?;
    ensure!(raw.len() <= 128 * 1024, "G3 bounded phase");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(managed.run.join(format!("g3-{name}.json")))?;
    f.write_all(&raw)?;
    f.sync_all()?;
    Ok(())
}
fn mapping<'a>(maps: &'a [oracle::Mapping], owner: usize, c: &str) -> Result<&'a oracle::Mapping> {
    maps.iter()
        .find(|m| m.owner == owner && m.context == c)
        .context("G3 actual owner/context mapping")
}
fn view(nodes: &[Node; 3], maps: &[oracle::Mapping], owner: usize, c: &str) -> Result<Node> {
    Ok(nodes[owner].view(c, mapping(maps, owner, c)?.slot))
}
fn statuses(owners: &mut Owners, nodes: &[Node; 3]) -> Result<[Value; 3]> {
    owners.require([true; 3])?;
    Ok([nodes[0].status()?, nodes[1].status()?, nodes[2].status()?])
}
fn admitted(
    owners: &mut Owners,
    nodes: &[Node; 3],
    maps: &[oracle::Mapping],
    only: Option<&str>,
) -> Result<[Option<Value>; 3]> {
    let active = owners.required;
    ensure!(active[1] && active[2], "G3 admission requires real X/Z");
    owners.wait(45, || {
        let full = samples(nodes, active)?;
        for m in maps
            .iter()
            .filter(|m| active[m.owner] && only.is_none_or(|c| c == m.context))
        {
            let status = full[m.owner].as_ref().context("G3 actual admitted owner")?;
            let r = oracle::row(status, &m.context)?;
            if oracle::admitted(status, &m.context).is_err()
                || r["counts"]["queued"] != 0
                || r["counts"]["pending"] != 0
                || r["counts"]["debt"] != 0
                || r["control_debt"] != false
            {
                return Ok(None);
            }
        }
        Ok(Some(full))
    })
}
fn exchange(owners: &mut Owners, pair: &mut Pair) -> Result<()> {
    owners.wait(15, || {
        Ok(pair
            .nodes
            .iter()
            .all(|n| n.locators.join(format!("{}.locator2", pair.c)).is_file())
            .then_some(()))
    })?;
    owners.alive()?;
    pair.exchange2()?;
    owners.alive()
}
fn clear_public(owners: &Owners, nodes: &[Node; 3], maps: &[oracle::Mapping]) -> Result<()> {
    owners.stopped()?;
    for m in maps {
        let path = nodes[m.owner]
            .locators
            .join(format!("{}.locator2", m.context));
        match std::fs::remove_file(path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
#[derive(serde::Serialize)]
struct Live {
    status: [Option<Value>; 3],
    history: std::collections::BTreeMap<String, oracle::History>,
}
impl Live {
    fn status(&self, owner: usize) -> Result<&Value> {
        self.status
            .get(owner)
            .and_then(Option::as_ref)
            .context("G3 no live status for stopped owner")
    }
}
fn samples(nodes: &[Node; 3], active: [bool; 3]) -> Result<[Option<Value>; 3]> {
    let mut result = [None, None, None];
    for (i, node) in nodes.iter().enumerate().filter(|(i, _)| active[*i]) {
        result[i] = Some(node.status()?);
    }
    Ok(result)
}
fn key(owner: usize, c: &str) -> String {
    format!("{owner}/{c}")
}
fn live(
    owners: &mut Owners,
    nodes: &[Node; 3],
    maps: &[oracle::Mapping],
    managed: &Managed,
    name: &str,
) -> Result<Live> {
    owners.alive()?;
    let active = owners.required;
    ensure!(
        active[1] && (active[2] || active == [false, true, false]),
        "G3 actual live owner subset"
    );
    let status = samples(nodes, active)?;
    // Persist the public sample even when a subsequent resource assertion fails.
    phase(
        managed,
        &format!("{name}-status-observed"),
        json!({"status":status,"owned":owners.pids()}),
    )?;
    let mut history = std::collections::BTreeMap::new();
    for (owner, full) in status
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.as_ref().map(|v| (i, v)))
    {
        let enabled = maps
            .iter()
            .filter(|m| m.owner == owner)
            .map(|m| m.slot)
            .collect::<Vec<_>>();
        oracle::resources(full, &enabled)?;
    }
    for m in maps.iter().filter(|m| active[m.owner]) {
        let next: [u64; 2] = serde_json::from_value(
            oracle::field(
                oracle::row(
                    status[m.owner]
                        .as_ref()
                        .context("G3 actual history owner")?,
                    &m.context,
                )?,
                "next",
            )?
            .clone(),
        )?;
        owners.alive()?;
        history.insert(
            key(m.owner, &m.context),
            oracle::history(
                view(nodes, maps, m.owner, &m.context)?.history(32, None)?,
                next,
            )?,
        );
    }
    Ok(Live { status, history })
}
fn joined(live: &Live, stopped: &[Value; 3], maps: &[oracle::Mapping]) -> Result<()> {
    for m in maps.iter().filter(|m| live.status[m.owner].is_some()) {
        let saved = &stopped[m.owner]["contexts"][&m.context]["stable"];
        let row = oracle::row(live.status(m.owner)?, &m.context)?;
        for field in [
            "native_hash",
            "native_generation",
            "next",
            "q",
            "cuts",
            "current_consent",
            "current_consent2",
        ] {
            oracle::exact(
                &format!("owner{}/{}/{field}", m.owner, m.context),
                oracle::field(row, field)?,
                oracle::field(saved, field)?,
            )?;
        }
        oracle::exact(
            &format!("owner{}/{}/history", m.owner, m.context),
            &serde_json::to_value(&live.history[&key(m.owner, &m.context)])?,
            oracle::field(saved, "history")?,
        )?;
    }
    Ok(())
}
fn original(live: &Live, maps: &[oracle::Mapping], contexts: &[String; 2]) -> Result<()> {
    for (index, c) in contexts.iter().enumerate() {
        let author = mapping(maps, 1, c)?.role;
        let histories = maps
            .iter()
            .filter(|m| &m.context == c)
            .map(|m| &live.history[&key(m.owner, c)])
            .collect::<Vec<_>>();
        ensure!(
            histories.len() == 2 && histories[0] == histories[1] && histories[0].len() == 1,
            "G3 identical single original per context"
        );
        let row = histories[0].values().next().context("G3 original row")?;
        ensure!(
            row.format == 1
                && row.proof_kind == "LegacySnapshot"
                && row.role == author
                && row.ordinal == 1
                && row.request == format!("ec00000000000000{:016x}", index + 1)
                && row.text == format!("genuine C source {}", index + 1)
                && row.delivered,
            "G3 exact original fixture text"
        );
    }
    Ok(())
}
fn request(node: &Node, id: u64) -> Result<Option<oracle::Request>> {
    oracle::request(json_cli(
        &current(),
        &[
            "ordered".into(),
            "request-status3".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            node.slot.to_string(),
            "--request-id".into(),
            oracle::request_id(id),
        ],
    )?)
}
fn stop_read(
    managed: &Managed,
    name: &str,
    owners: &mut Owners,
    nodes: &[Node; 3],
    maps: &[oracle::Mapping],
    contexts: &[String; 2],
    expected: &[oracle::Directed],
) -> Result<witness::Checkpoint> {
    phase(
        managed,
        &format!("{name}-before-stop"),
        json!({"owned":owners.pids()}),
    )?;
    owners.stop()?;
    let saved = witness::collect(
        owners,
        nodes,
        maps,
        &managed.run.join("two-c"),
        contexts,
        expected,
        None,
    )?;
    phase(
        managed,
        &format!("{name}-stopped-observed"),
        json!({"state":saved}),
    )?;
    Ok(saved)
}

pub(super) fn run() -> Result<()> {
    let managed = Managed::new("F-s53-two-c-routes-integrated-01", 420)?;
    let base = managed.run.join("two-c");
    std::fs::create_dir(&base)?;
    let mut old = [
        OldNode::enrolled(&base, 0)?,
        OldNode::enrolled(&base, 1)?,
        OldNode::enrolled(&base, 2)?,
    ];
    old.sort_by(|a, b| a.device.cmp(&b.device));
    ensure!(
        old.iter()
            .map(|n| &n.account)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 3
            && old
                .iter()
                .map(|n| &n.device)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 3,
        "G3 fresh distinct owners"
    );
    let [y, x, z] = &old;
    let contexts = [
        obligations::context_between(y, x, &base, "first")?,
        obligations::context_between(x, z, &base, "second")?,
    ];
    y.profile(&contexts[0])?;
    x.profile(&contexts[0])?;
    z.profile(&contexts[1])?;
    let mut profile: Value = serde_json::from_slice(&std::fs::read(&x.profile)?)?;
    profile["enabled_contexts"] = json!(contexts);
    std::fs::write(&x.profile, serde_json::to_vec_pretty(&profile)?)?;
    let mut owners = Owners::default();
    for (owner, node) in old.iter().enumerate() {
        owners.alive()?;
        let mut c = spawn(
            &frozen(),
            &node.runtime(),
            &base,
            &format!("old-{owner}"),
            None,
            None,
        )?;
        wait_file(&mut c, &node.ipc, 15)?;
        owners.insert(owner, c)?;
    }
    import_peer(y, x, &contexts[0])?;
    import_peer(x, y, &contexts[0])?;
    import_peer(x, z, &contexts[1])?;
    import_peer(z, x, &contexts[1])?;
    isolation::old_queue(x, &contexts[0], 1)?;
    isolation::old_queue(x, &contexts[1], 2)?;
    let legacy = owners.wait(25, || {
        let full = [old_status(y)?, old_status(x)?, old_status(z)?];
        for c in &contexts {
            let r = oracle::row(&full[1], c)?;
            if r["phase"] != 3 || r["send_next"] != 2 || r["unacked"] != 0 {
                return Ok(None);
            }
        }
        if oracle::row(&full[0], &contexts[0])?["receive_next"] != 2
            || oracle::row(&full[2], &contexts[1])?["receive_next"] != 2
        {
            return Ok(None);
        }
        Ok(Some(full))
    })?;
    phase(
        &managed,
        "legacy-delivered",
        json!({"status":legacy,"owned":owners.pids()}),
    )?;
    owners.stop()?;
    let mut nodes = [
        Node::from_old(y, &contexts[0])?,
        Node::from_old(x, &contexts[0])?,
        Node::from_old(z, &contexts[1])?,
    ];
    let mut ready = Vec::new();
    for n in &mut nodes {
        ready.push(n.migrate()?);
    }
    owners.start(&nodes, "selected")?;
    let full = statuses(&mut owners, &nodes)?;
    phase(
        &managed,
        "ready-status-observed",
        json!({"ready":ready,"status":full,"owned":owners.pids()}),
    )?;
    let maps = oracle::mappings(&full, &contexts)?;
    phase(
        &managed,
        "mapping-observed",
        json!({"ready":ready,"mappings":maps,"status":full,"owned":owners.pids()}),
    )?;
    oracle::ready_mappings(&ready, &maps)?;
    let mut first = view_pair(
        &base,
        [
            view(&nodes, &maps, 0, &contexts[0])?,
            view(&nodes, &maps, 1, &contexts[0])?,
        ],
        [y.root.clone(), x.root.clone()],
        &contexts[0],
    );
    let mut second = view_pair(
        &base,
        [
            view(&nodes, &maps, 1, &contexts[1])?,
            view(&nodes, &maps, 2, &contexts[1])?,
        ],
        [x.root.clone(), z.root.clone()],
        &contexts[1],
    );
    owners.alive()?;
    first.ceremony("c1", "absent", 30 * 86400)?;
    owners.alive()?;
    second.ceremony("c2", "absent", 30 * 86400)?;
    first.upgrade(1, 51)?;
    // G3 initial readiness uses an explicit A trigger; retain X/B intent.
    first.upgrade(0, 53)?;
    second.upgrade(0, 52)?;
    exchange(&mut owners, &mut first)?;
    exchange(&mut owners, &mut second)?;
    admitted(&mut owners, &nodes, &maps, None)
        .context("G3 initial C1/C2 admission after explicit A triggers")?;
    let initial = live(&mut owners, &nodes, &maps, &managed, "selected")?;
    phase(
        &managed,
        "selected-live-observed",
        serde_json::to_value(&initial)?,
    )?;
    original(&initial, &maps, &contexts)?;
    let baseline = stop_read(
        &managed,
        "baseline",
        &mut owners,
        &nodes,
        &maps,
        &contexts,
        &[],
    )?;
    joined(&initial, &baseline, &maps)?;
    clear_public(&owners, &nodes, &maps)?;
    owners.start(&nodes, "pin-baseline")?;
    exchange(&mut owners, &mut first)?;
    exchange(&mut owners, &mut second)?;
    admitted(&mut owners, &nodes, &maps, None)?;
    let before = live(&mut owners, &nodes, &maps, &managed, "before-pin")?;
    phase(
        &managed,
        "before-pin-observed",
        serde_json::to_value(&before)?,
    )?;
    joined(&before, &baseline, &maps)?;
    let root = kilogram_identity::AccountRootState::load(&y.root)?;
    root.revoke_device(y.device.parse()?)?;
    let pin = root.authority_snapshot()?;
    let authority = base.join("revoked-y.authority");
    let raw = pin.encode()?;
    ensure!(
        pin.account_id().as_bytes() == &state::parse_id(&y.account)?,
        "G3 revoked subject account"
    );
    let checked = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&raw)?;
    ensure!(
        kilogram_identity::verify_device_authorization_with_snapshot(
            checked.account_id(),
            &wire::VerifiedExistingScope1::verify_export(&std::fs::read(
                base.join("c1.origin.cbor")
            )?)?
            .peers()[mapping(&maps, 0, &contexts[0])?.role as usize]
                .certificate()
                .clone(),
            &checked,
            &kilogram_identity::DeviceCapability::MESSAGING
        )
        .is_err(),
        "G3 Y actually revoked"
    );
    std::fs::write(&authority, &raw)?;
    drop(root);
    phase(
        &managed,
        "pin-input-observed",
        json!({"subject":y.account,"revision":pin.revision(),"hash":hash(&raw)}),
    )?;
    cli(
        &current(),
        &[
            "ordered".into(),
            "authority3-import".into(),
            "--ipc-file".into(),
            text(&nodes[1].ipc),
            "--snapshot-file".into(),
            text(&authority),
        ],
    )?;
    owners.wait(10, || {
        let full = nodes[1].status()?;
        Ok(oracle::paused(oracle::row(&full, &contexts[0])?).then_some(full))
    })?;
    let paused = live(&mut owners, &nodes, &maps, &managed, "pin-paused")?;
    phase(
        &managed,
        "pin-paused-observed",
        serde_json::to_value(&paused)?,
    )?;
    oracle::pin(before.status(1)?, paused.status(1)?, &contexts)?;
    for m in &maps {
        oracle::exact(
            &format!("{}/pin/history", key(m.owner, &m.context)),
            &serde_json::to_value(&before.history[&key(m.owner, &m.context)])?,
            &serde_json::to_value(&paused.history[&key(m.owner, &m.context)])?,
        )?;
    }
    let c1 = view(&nodes, &maps, 1, &contexts[0])?;
    let rejected_before = request(&c1, 499)?;
    let rejection = c1
        .send(499, "G3 rejected C1 send")
        .err()
        .context("G3 revoked C1 send must reject")?;
    let rejected_after = request(&c1, 499)?;
    let no_op = live(&mut owners, &nodes, &maps, &managed, "rejected-c1")?;
    phase(
        &managed,
        "rejected-c1-observed",
        json!({"before":rejected_before,"after":rejected_after,"error":rejection.to_string().chars().take(512).collect::<String>(),"live":no_op}),
    )?;
    ensure!(
        rejected_before.is_none() && rejected_after.is_none(),
        "G3 rejected request never enqueued"
    );
    for m in &maps {
        oracle::exact(
            &format!("{}/rejected/data", key(m.owner, &m.context)),
            &oracle::stable(oracle::row(paused.status(m.owner)?, &m.context)?)?,
            &oracle::stable(oracle::row(no_op.status(m.owner)?, &m.context)?)?,
        )?;
        oracle::exact(
            "rejected/history",
            &serde_json::to_value(&paused.history[&key(m.owner, &m.context)])?,
            &serde_json::to_value(&no_op.history[&key(m.owner, &m.context)])?,
        )?;
    }
    let sender = view(&nodes, &maps, 1, &contexts[1])?;
    let m = mapping(&maps, 1, &contexts[1])?;
    let pre_row = oracle::row(no_op.status(1)?, &contexts[1])?;
    let next: [u64; 2] = serde_json::from_value(pre_row["next"].clone())?;
    let tails: [Option<String>; 2] =
        serde_json::from_value(baseline[1]["contexts"][&contexts[1]]["stable"]["tails"].clone())?;
    let body = "G3 C2 after C1 pause";
    let sent = sender.send(500, body)?;
    let mut prior = oracle::sent(&sent)?;
    let delivered = owners.wait(25, || {
        let actual = request(&sender, 500)?.context("G3 queued request disappeared")?;
        oracle::request_stable(&prior, &actual)?;
        prior = actual.clone();
        Ok(actual.delivered.then_some(actual))
    })?;
    let directed = oracle::Directed {
        context: contexts[1].clone(),
        author: 1,
        role: m.role,
        id: 500,
        body: body.into(),
        ordinal: next[m.role as usize],
        sequence: oracle::number(&baseline[1], "next_author")?,
        previous: tails[m.role as usize]
            .as_deref()
            .map(state::parse_id)
            .transpose()?,
        saved: delivered,
    };
    admitted(&mut owners, &nodes, &maps, Some(&contexts[1]))?;
    let after = live(&mut owners, &nodes, &maps, &managed, "delivery500")?;
    phase(
        &managed,
        "delivery500-live-observed",
        json!({"expectation":directed,"send":sent,"live":after}),
    )?;
    oracle::progress(&no_op.status, &after.status, &maps, &directed)?;
    for m in maps.iter().filter(|m| m.context == contexts[1]) {
        directed.history(
            &no_op.history[&key(m.owner, &m.context)],
            &after.history[&key(m.owner, &m.context)],
        )?;
    }
    for m in maps.iter().filter(|m| m.context == contexts[0]) {
        oracle::exact(
            "C1 after C2/data",
            &oracle::stable(oracle::row(no_op.status(m.owner)?, &m.context)?)?,
            &oracle::stable(oracle::row(after.status(m.owner)?, &m.context)?)?,
        )?;
        oracle::exact(
            "C1 after C2/history",
            &serde_json::to_value(&no_op.history[&key(m.owner, &m.context)])?,
            &serde_json::to_value(&after.history[&key(m.owner, &m.context)])?,
        )?;
    }
    ensure!(
        oracle::paused(oracle::row(after.status(1)?, &contexts[0])?),
        "G3 C1 remains paused through C2 progress"
    );
    let prior_public = [
        routes::read_public(&nodes[1], &contexts[1])?,
        routes::read_public(&nodes[2], &contexts[1])?,
    ];
    let prior_profiles: [Value; 2] = [
        serde_json::from_slice(&std::fs::read(&nodes[1].profile)?)?,
        serde_json::from_slice(&std::fs::read(&nodes[2].profile)?)?,
    ];
    phase(
        &managed,
        "relay-transition-before-stop",
        json!({"public_C2":prior_public,"profiles_XZ":prior_profiles,"live":after,"owned":owners.pids()}),
    )?;
    let saved = stop_read(
        &managed,
        "delivery500",
        &mut owners,
        &nodes,
        &maps,
        &contexts,
        std::slice::from_ref(&directed),
    )?;
    joined(&after, &saved, &maps)?;
    routes::stored_join(&saved, &contexts[1], &prior_public)?;
    for owner in [0, 1] {
        oracle::exact(
            &format!("owner{owner}/C1 stopped data"),
            &baseline[owner]["contexts"][&contexts[0]]["stable"],
            &saved[owner]["contexts"][&contexts[0]]["stable"],
        )?;
    }
    oracle::exact(
        "X stored Y authority",
        &json!({"revision":pin.revision(),"hash":hash(&raw)}),
        &saved[1]["authority"][&y.account],
    )?;
    phase(
        &managed,
        "slice-a-observations-complete",
        json!({"mappings":maps,"state":saved,"ordinary_occupancy_only":true,"actual_owners_reaped":true,"remaining":"unreviewed slice B route/capacity/final restart"}),
    )?;
    routes::remove_public(&owners, &nodes, &maps, &contexts[1])?;
    phase(
        &managed,
        "relay-public-files-removed",
        json!({"owners_stopped":true,"removed_context":contexts[1],"preserved_Y":saved[0]}),
    )?;
    let url = routes::start_relay(&mut owners, &base)?;
    phase(
        &managed,
        "relay-started",
        json!({"url":url,"pid":owners.relay.as_ref().context("G3 actual relay")?.0.id()}),
    )?;
    for (i, owner) in [1, 2].into_iter().enumerate() {
        owners.stopped()?;
        let candidate = routes::relay_overlay(&prior_profiles[i], &url)?;
        std::fs::write(
            &nodes[owner].profile,
            serde_json::to_vec_pretty(&candidate)?,
        )?;
        let actual: Value = serde_json::from_slice(&std::fs::read(&nodes[owner].profile)?)?;
        phase(
            &managed,
            &format!("relay-profile-{owner}-observed"),
            json!({"before":prior_profiles[i],"after":actual}),
        )?;
        oracle::exact(
            &format!("owner{owner}/only RelayOnly overlay"),
            &candidate,
            &actual,
        )?;
        ensure!(
            Profile3::load(&nodes[owner].profile)?
                .route_config()?
                .effective()?
                .as_str()
                == "relay-only",
            "G3 effective RelayOnly profile"
        );
    }
    owners.start_subset(&nodes, "relay", [false, true, true])?;
    owners.require([false, true, true])?;
    owners.wait(15, || {
        Ok([1, 2]
            .iter()
            .all(|i| {
                nodes[*i]
                    .locators
                    .join(format!("{}.locator2", contexts[1]))
                    .is_file()
            })
            .then_some(()))
    })?;
    let relay_public = [
        routes::read_public(&nodes[1], &contexts[1])?,
        routes::read_public(&nodes[2], &contexts[1])?,
    ];
    phase(
        &managed,
        "relay-publications-observed",
        json!({"public_C2":relay_public,"owned":owners.pids(),"relay_pid":owners.relay.as_ref().context("G3 relay")?.0.id(),"url":url,"recreated_after_owned_removal":true}),
    )?;
    let scope =
        wire::VerifiedExistingScope1::verify_export(&std::fs::read(base.join("c2.origin.cbor"))?)?;
    let locators = [
        routes::relay_locator(
            &relay_public[0],
            scope.c(),
            scope.peers(),
            mapping(&maps, 1, &contexts[1])?.role,
            &url,
            routes::wall()?,
        )?,
        routes::relay_locator(
            &relay_public[1],
            scope.c(),
            scope.peers(),
            mapping(&maps, 2, &contexts[1])?.role,
            &url,
            routes::wall()?,
        )?,
    ];
    for i in 0..2 {
        routes::revision(&prior_public[i], &relay_public[i])?;
    }
    exchange(&mut owners, &mut second)?;
    let mut last_status = None;
    let mut last_error = None;
    let path_wait = owners.wait(45, || {
        let actual = samples(&nodes, [false, true, true])?;
        let checked = routes::both(&actual, &maps, &contexts[1], &locators);
        last_error = checked.as_ref().err().map(|e| e.to_string());
        last_status = Some(actual);
        Ok(checked.is_ok().then_some(()))
    });
    phase(
        &managed,
        "relay-paths-observed",
        json!({"status":last_status,"diagnostic":last_error,"owned":owners.pids()}),
    )?;
    path_wait?;
    admitted(&mut owners, &nodes, &maps, Some(&contexts[1]))?;
    let pre501 = live(&mut owners, &nodes, &maps, &managed, "before501")?;
    phase(
        &managed,
        "before501-live-observed",
        serde_json::to_value(&pre501)?,
    )?;
    routes::both(&pre501.status, &maps, &contexts[1], &locators)?;
    joined(&pre501, &saved, &maps)?;
    for m in maps.iter().filter(|m| pre501.status[m.owner].is_some()) {
        oracle::exact(
            &format!("{}/RelayOnly/data", key(m.owner, &m.context)),
            &oracle::stable(oracle::row(after.status(m.owner)?, &m.context)?)?,
            &oracle::stable(oracle::row(pre501.status(m.owner)?, &m.context)?)?,
        )?;
        oracle::exact(
            &format!("{}/RelayOnly/policy", key(m.owner, &m.context)),
            oracle::field(
                oracle::row(after.status(m.owner)?, &m.context)?,
                "policy_generation",
            )?,
            oracle::field(
                oracle::row(pre501.status(m.owner)?, &m.context)?,
                "policy_generation",
            )?,
        )?;
    }
    ensure!(
        oracle::paused(oracle::row(pre501.status(1)?, &contexts[0])?),
        "G3 persisted C1 pin pause under RelayOnly"
    );
    for owner in [1, 2] {
        for stat in [
            "committed_encrypt",
            "committed_decrypt",
            "materialized",
            "readable",
            "account_constructors",
            "outbound_constructors",
            "inbound_constructors",
            "aborted_packet_calls",
            "ambiguous_packet_calls",
        ] {
            ensure!(
                pre501.status(owner)?["stats"][stat] == 0,
                "G3 no application replay before501 owner{owner}/{stat}"
            );
        }
    }
    let sender501 = view(&nodes, &maps, 2, &contexts[1])?;
    let author = mapping(&maps, 2, &contexts[1])?;
    let next501: [u64; 2] =
        serde_json::from_value(oracle::row(pre501.status(2)?, &contexts[1])?["next"].clone())?;
    let tails501: [Option<String>; 2] =
        serde_json::from_value(saved[2]["contexts"][&contexts[1]]["stable"]["tails"].clone())?;
    ensure!(
        request(&sender501, 501)?.is_none(),
        "G3 fresh distinct request501"
    );
    // Routing/admission performs no application materialization; saved global author
    // sequence is joined to this no-replay pre-send frontier, not a round ordinal.
    let mut second_directed = oracle::Directed {
        context: contexts[1].clone(),
        author: 2,
        role: author.role,
        id: 501,
        body: "G3 C2 relay delivery".into(),
        ordinal: next501[author.role as usize],
        sequence: oracle::number(&saved[2], "next_author")?,
        previous: tails501[author.role as usize]
            .as_deref()
            .map(state::parse_id)
            .transpose()?,
        saved: oracle::sent(&sender501.send(501, "G3 C2 relay delivery")?)?,
    };
    let mut prior501 = second_directed.saved.clone();
    second_directed.saved = owners.wait(25, || {
        let actual = request(&sender501, 501)?.context("G3 request501 disappeared")?;
        oracle::request_stable(&prior501, &actual)?;
        prior501 = actual.clone();
        Ok(actual.delivered.then_some(actual))
    })?;
    admitted(&mut owners, &nodes, &maps, Some(&contexts[1]))?;
    let after501 = live(&mut owners, &nodes, &maps, &managed, "delivery501")?;
    phase(
        &managed,
        "delivery501-live-observed",
        json!({"expectation":second_directed,"live":after501}),
    )?;
    routes::both(&after501.status, &maps, &contexts[1], &locators)?;
    oracle::progress(&pre501.status, &after501.status, &maps, &second_directed)?;
    for m in maps.iter().filter(|m| m.context == contexts[1]) {
        second_directed.history(
            &pre501.history[&key(m.owner, &m.context)],
            &after501.history[&key(m.owner, &m.context)],
        )?;
        ensure!(
            after501.history[&key(m.owner, &m.context)].len() == 3,
            "G3 exactly original/500/501"
        );
    }
    oracle::exact(
        "X/C1 during501/data",
        &oracle::stable(oracle::row(pre501.status(1)?, &contexts[0])?)?,
        &oracle::stable(oracle::row(after501.status(1)?, &contexts[0])?)?,
    )?;
    oracle::exact(
        "X/C1 during501/history",
        &serde_json::to_value(&pre501.history[&key(1, &contexts[0])])?,
        &serde_json::to_value(&after501.history[&key(1, &contexts[0])])?,
    )?;
    ensure!(
        oracle::paused(oracle::row(after501.status(1)?, &contexts[0])?),
        "G3 C1 still paused after501"
    );
    let expected = [directed, second_directed];
    phase(
        &managed,
        "delivery501-before-stop",
        json!({"owned":owners.pids(),"relay_pid":owners.relay.as_ref().context("G3 actual relay")?.0.id()}),
    )?;
    owners.stop()?;
    owners.alive()?;
    let final_saved = witness::collect(
        &owners,
        &nodes,
        &maps,
        &base,
        &contexts,
        &expected,
        Some(&saved),
    )?;
    phase(
        &managed,
        "delivery501-stopped-observed",
        json!({"state":final_saved,"Y_evidence":"preserved earlier stopped witness; Y was never restarted or queried live"}),
    )?;
    owners.alive()?;
    joined(&after501, &final_saved, &maps)?;
    routes::stored_join(&final_saved, &contexts[1], &relay_public)?;
    oracle::exact(
        "X/C1 stopped after501",
        &saved[1]["contexts"][&contexts[0]]["stable"],
        &final_saved[1]["contexts"][&contexts[0]]["stable"],
    )?;
    oracle::exact(
        "X persisted Y authority after501",
        &saved[1]["authority"][&y.account],
        &final_saved[1]["authority"][&y.account],
    )?;
    phase(
        &managed,
        "slice-b1-observations-complete",
        json!({"state":final_saved,"node_owners_reaped":true,"relay_kept_alive":true}),
    )?;
    let observations = finish::run(
        &managed,
        &mut owners,
        &nodes,
        &maps,
        &contexts,
        final_saved,
        expected.into_iter().collect(),
    )?;
    managed.finish(observations)
}
