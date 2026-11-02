//! Exactly one owner repository/lock lifetime, extensible by the later Native slice.
use super::*;
use zeroize::Zeroize;
struct Page(Value);
impl std::ops::Deref for Page {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.0
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        if let Some(rows) = self.0["items"].as_array_mut() {
            for row in rows {
                if let Value::String(text) = &mut row["text"] {
                    text.zeroize();
                }
            }
        }
    }
}
pub(super) struct Input<'a> {
    pub owner: usize,
    pub closure: &'a control::Closure,
    pub maps: &'a [BTreeMap<String, Value>; 2],
    pub public: &'a Value,
    pub profiles: &'a Value,
    pub immutable: &'a offline::retained::Immutable,
    pub held: &'a Value,
    pub ack: &'a observations::AckObservation,
}
fn tick(relay: &mut Option<relay_owner::Relay>, until: Instant) -> Result<()> {
    s53_deadline::check()?;
    ensure!(
        Instant::now() < until,
        "original controller absolute read deadline"
    );
    relay
        .as_mut()
        .context("same retained relay throughout read")?
        .child
        .alive()
}
fn credential(
    repo: &mut store::L1Repository,
    slot: u8,
    id: old::Id,
) -> Result<v2::RootVerifiedCredential> {
    Ok(v2::ParsedCredentialBundle::parse(
        &repo.auth(slot, id)?.context("actual stored credential")?,
    )?
    .verify_root()?)
}
fn author(raw: &[u8], format: u8) -> Result<old::Id> {
    if format == 1 {
        Ok(old::Core::parse(&old::decode(raw, v2::MAX_EVENT)?.fields(5)?[1])?.author_auth)
    } else {
        Ok(v2::ParsedEvent2::parse(raw)?.core().author_auth)
    }
}
fn receiver(raw: &[u8]) -> Result<old::Id> {
    let value = old::decode(raw, v2::MAX_RECEIPT)?;
    let body = &value.fields(2)?[0];
    match body
        .list()?
        .first()
        .context("actual independent receipt version")?
        .uint()?
    {
        1 => Ok(body.fields(8)?[7].fixed()?),
        2 => Ok(v2::ParsedReceipt2::parse(raw)?.fields().receiver_auth),
        _ => anyhow::bail!("unsupported actual receipt version"),
    }
}
fn history(
    s: &mut control::Controller,
    repo: &mut store::L1Repository,
    slot: u8,
    context: usize,
    expected: &BTreeMap<String, Value>,
    graph: &graph::Context,
) -> Result<old::Id> {
    let mut cursor = None;
    let mut stamp = None;
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for _ in 0..=limit(context) * 2 {
        s.check()?;
        tick(&mut s.relay, s.until)?;
        let raw = Zeroizing::new(repo.history(slot, cursor.as_ref(), 32)?);
        ensure!(raw.len() <= 48 * 1024, "bounded actual history page");
        let page = Page(serde_json::from_slice(&raw)?);
        let identity = (
            page["view"]
                .as_str()
                .context("actual history view")?
                .to_owned(),
            page["F"].clone(),
        );
        ensure!(
            identity.1 == json!([limit(context) + 1, limit(context) + 1]),
            "complete terminal history frontiers"
        );
        if let Some(previous) = &stamp {
            ensure!(previous == &identity, "same actual history view/frontiers");
        } else {
            stamp = Some(identity);
        }
        let rows = page["items"].as_array().context("actual history rows")?;
        ensure!(rows.len() <= 32, "actual history bounded page");
        for row in rows {
            s53_deadline::check()?;
            let id = row["id"].as_str().context("actual history ID")?;
            let original = expected
                .get(id)
                .context("actual history must belong to retained final map")?;
            ordered.push(gates::history_row(row, original)?);
            ensure!(
                ordered.len() <= usize::try_from(limit(context) * 2)?,
                "bounded complete history"
            );
        }
        if page["finished"] == true {
            ensure!(
                ordered == graph.ordered && ordered.len() == expected.len(),
                "complete actual history order equals graph and retained final associations"
            );
            return Ok(digest(&old::encode(
                &old::a(ordered.iter().map(old::b)),
                48 * 1024,
            )?));
        }
        let next = page["cursor"].as_str().context("actual progress cursor")?;
        ensure!(
            !rows.is_empty() && seen.insert(next.to_owned()),
            "bounded actual cursor progress"
        );
        cursor = Some(db::HistoryCursor::from_base64(next)?);
    }
    anyhow::bail!("bounded complete history EOF missing")
}
pub(super) fn read(
    s: &mut control::Controller,
    input: Input<'_>,
    accounting: &mut accounting::Draft,
) -> Result<Owner> {
    let Input {
        owner,
        closure,
        maps,
        public,
        profiles,
        immutable,
        held,
        ack,
    } = input;
    s.check()?;
    s.stopped_reconciled()?;
    let node = s.view(owner, 0)?;
    source::owned(&node)?;
    let profile_raw = kilogram_cli::ordered_dialog::admin::read_public(&node.profile, 64 * 1024)?;
    let profile_value: Value = serde_json::from_slice(&profile_raw)?;
    let saved = profiles
        .as_array()
        .context("actual retained route profile facts")?
        .get(owner)
        .context("original profile owner")?;
    ensure!(
        saved["owner"] == owner
            && saved["profile"] == text(&node.profile)
            && saved["after_blake3"] == state::hex(&digest(&serde_json::to_vec(&profile_value)?)),
        "exact same-run effective relay profile/config"
    );
    let profile = Profile3::load(&node.profile)?;
    let relay = s.relay.as_ref().context("same live owned relay")?;
    ensure!(
        profile.account == node.branch.account
            && profile.device == node.branch.device
            && profile.state_dir.canonicalize()? == node.branch.state.canonicalize()?
            && profile.ipc_file == node.ipc
            && profile.locator_directory == node.locators
            && profile.route_config()?.effective()?.as_str() == "relay-only"
            && profile.route_config()?.relay.as_deref() == relay.url.as_deref(),
        "original canonical node/profile/relay mapping"
    );
    let mut repo = profile.open(&node.profile)?;
    let original = s.original[owner]
        .as_ref()
        .context("original immutable owner witness")?;
    let migration = s.migration[owner]
        .as_ref()
        .context("original immutable migration witness")?;
    let ready = s
        .rows
        .get(&format!("ready-{owner}"))
        .context("original same-run migration Ready mapping")?;
    ensure!(
        repo.source_id() == closure.key.owner
            && repo.source_id() == original.source_id
            && repo.source_id() == migration.source_id
            && ready["source_id"] == state::hex(&repo.source_id())
            && ready["migration_id"] == state::hex(&repo.migration_id()),
        "original source/migration/Ready owner binding"
    );
    let material = repo.owner_material()?;
    original.terminal_identity(&node, &material)?;
    migration.terminal_identity(&node, &material)?;
    let contexts = [
        state::parse_id(&s.origin.contexts[0])?,
        state::parse_id(&s.origin.contexts[1])?,
    ];
    let slots = gates::mapping(owner, repo.contexts(), contexts, ready)?;
    accounting.begin_owner(owner as u8, repo.source_id(), slots)?;
    ensure!(
        profile
            .enabled_contexts
            .iter()
            .map(|c| state::parse_id(c))
            .collect::<Result<BTreeSet<_>>>()?
            == BTreeSet::from(contexts),
        "both original contexts enabled"
    );
    let mut origins = Vec::new();
    let mut proofs = Vec::new();
    let mut selected = Vec::new();
    let mut metadata = Vec::new();
    let mut heads = Vec::new();
    for c in 0..2 {
        tick(&mut s.relay, s.until)?;
        let slot = slots[c];
        let spec_raw = repo.spec(slot)?;
        let spec = Spec::parse(&spec_raw)?;
        ensure!(
            spec.c == contexts[c] && spec.role(profile.device.parse()?)? == owner as u64,
            "original actual Spec role"
        );
        let head_raw = repo.head(slot)?;
        let head = Head::parse(
            &head_raw,
            contexts[c],
            spec.cert_refs()?[owner],
            owner as u64,
        )?;
        ensure!(head.phase == 3, "actual ready session Head");
        let origin = offline::existing_scope(&mut repo, slot, &head, &spec_raw)?;
        ensure!(
            head.d == Some(origin.d())
                && head.e == Some(origin.e())
                && head.sid.as_deref() == Some(origin.sid()),
            "actual current Head binds authenticated original session"
        );
        original.terminal_scope(c, &spec_raw, &origin)?;
        migration.terminal_scope(c, &spec_raw, &origin)?;
        let mode = repo.existing_state(slot)?;
        gates::active(
            c,
            owner,
            &gates::Scope {
                c: origin.c(),
                d: origin.d(),
                e: origin.e(),
                sid: origin.sid(),
            },
            &mode,
            &public[c],
        )?;
        let q = mode.q.context("actual selected Q")?;
        let proof = v2::ParsedModeProof2::parse(&repo.existing_control(slot, q)?)?;
        let selection = origin.selection(proof.clone())?;
        let consent = v2::ParsedConsent2::parse(&repo.existing_control(
            slot,
            mode.current_consent.context("actual current consent")?,
        )?)?;
        origin.verify_consent(&consent, origin.peers())?;
        ensure!(
            consent.reference() == mode.current_consent.context("actual consent ID")?,
            "stored consent key/content"
        );
        let proof_value = old::decode(proof.raw(), v2::MAX_PROOF)?;
        let proof_fields = proof_value.fields(6)?;
        ensure!(
            proof_fields[1].uint()? == 1 && proof_fields[5].bytes()? == consent.raw(),
            "actual Q embeds same current selected consent bytes"
        );
        // Full selected proof already authenticates its P/A/K chain and consent.
        // Compare actual retained refs, then authenticate the extra Committed control.
        for (index, reference) in [mode.propose, mode.accept, mode.select]
            .into_iter()
            .enumerate()
        {
            let reference = reference.context("selected signed control ref")?;
            let raw = repo.existing_control(slot, reference)?;
            ensure!(
                v2::ParsedControl2::parse(&raw)?.id() == reference
                    && proof_fields[index + 2].bytes()? == raw,
                "stored selected control key/content and exact authenticated Q chain"
            );
        }
        let reference = mode.committed.context("actual Committed")?;
        let committed = v2::ParsedControl2::parse(&repo.existing_control(slot, reference)?)?;
        ensure!(committed.id() == reference, "actual Committed ref");
        committed.verify_committed(&selection, &origin.peers()[1])?;
        if c == 0 {
            immutable.terminal_scope(
                owner,
                &node,
                repo.source_id(),
                repo.migration_id(),
                &origin,
                q,
            )?;
        }
        let meta = repo.metadata(slot)?;
        gates::counts(
            c,
            &repo.counts(slot)?,
            &repo.source_counts(slot)?,
            &meta,
            &repo.repair_state(slot)?,
            &mode,
        )?;
        ensure!(
            repo.queue_page(slot, None, 1)?.is_empty()
                && repo.pending_page(slot, None, 1)?.is_empty()
                && repo.debt_page(slot, None, 1)?.is_empty(),
            "bounded EOF of all empty work indexes"
        );
        for role in 0..2u8 {
            let raw = repo
                .event_at(slot, role, 0)?
                .context("actual original genesis")?;
            ensure!(
                old::hash(old::EVENT_ID, &raw) == origin.old_control_ids()[role as usize + 2],
                "actual genesis source binding"
            );
            let receipt = repo.receipt(slot, role, 0)?;
            ensure!(
                receipt.is_none()
                    && repo.event_at(slot, role, limit(c) + 1)?.is_none()
                    && repo.receipt(slot, role, limit(c) + 1)?.is_none(),
                "no genesis receipt or immediate suffix; exact root counts exclude all distant extras"
            );
            original.terminal_source(c, role, 0, &raw, None)?;
            migration.terminal_source(c, role, 0, &raw, None)?;
            let event = old::Event::parse(&raw, spec.peers[role as usize].cert.device_id())?;
            accounting.genesis(accounting::Atom::legacy(c as u8, &raw, &event, None)?)?;
        }
        origins.push(origin);
        proofs.push(proof);
        selected.push(selection);
        metadata.push(meta);
        heads.push((head_raw, head));
    }
    let mut graph = graph::Graph::new(
        [&origins[0], &origins[1]],
        proofs
            .try_into()
            .map_err(|_| anyhow::anyhow!("two selected proofs"))?,
    )?;
    let mut ack_rows = BTreeMap::new();
    let mut witness_hash = blake3::Hasher::new();
    witness_hash.update(b"kg:s54:terminal-witness:v1\0");
    ensure!(
        ack.context == 0 && ack.lane == 0 && !ack.receipts.is_empty() && ack.receipts.len() <= 32,
        "original bounded ACK batch"
    );
    for c in 0..2 {
        let slot = slots[c];
        ensure!(
            maps[c].len() == usize::try_from(limit(c) * 2)?,
            "original complete context history associations"
        );
        for role in 0..2u8 {
            for n in 1..=limit(c) {
                tick(&mut s.relay, s.until)?;
                let request = oracle::Request::new(c as u8, role, n.try_into()?)?;
                let originals = maps[c]
                    .values()
                    .filter(|r| r["role"] == role && r["ordinal"] == n)
                    .collect::<Vec<_>>();
                ensure!(
                    originals.len() == 1,
                    "unique retained original Text position"
                );
                let event = state::parse_id(
                    originals[0]["id"]
                        .as_str()
                        .context("original actual EventID")?,
                )?;
                ensure!(
                    originals[0]["request"] == source::request_id(request)
                        && originals[0]["text"] == request.body(),
                    "original request/body history association"
                );
                let raw = repo
                    .event_at(slot, role, n)?
                    .context("actual terminal source")?;
                let receipt = repo
                    .receipt(slot, role, n)?
                    .context("actual saved/accepted terminal receipt")?;
                let author = credential(
                    &mut repo,
                    slot,
                    author(&raw, selected[c].proof().format_for(role, n)?)?,
                )?;
                let receiver = credential(&mut repo, slot, receiver(&receipt)?)?;
                let mut projection = repo.projection(slot, event)?;
                let body = Zeroizing::new(std::mem::take(&mut projection.text));
                let projection_request =
                    gates::projection(&projection, repo.migration_id(), repo.source_id())?;
                let row = records::verify(
                    &records::Expected {
                        contexts: &contexts,
                        request,
                        event,
                        d: origins[c].d(),
                        e: origins[c].e(),
                        sid: origins[c].sid(),
                        selection: selected[c].proof().q(),
                    },
                    &records::Projection {
                        event,
                        request: projection_request,
                        text: &body,
                    },
                    &selected[c],
                    records::Credentials {
                        author: &author,
                        receiver: &receiver,
                    },
                    &raw,
                    &receipt,
                )?;
                gates::own_request(owner, &row, repo.request(slot, row.request)?.as_ref())?;
                original.terminal_source(c, role, n, &raw, Some(&receipt))?;
                migration.terminal_source(c, role, n, &raw, Some(&receipt))?;
                if c == 0 {
                    immutable.terminal_record(&row)?;
                    if role == 1 && n == 640 {
                        gates::held(&row, held)?;
                    }
                    if role == 0 && n == 24 {
                        ensure!(
                            s.rows["saved-r1"]["receipt_blake3"] == state::hex(&row.receipt_hash)
                                && s.rows["saved-r1"]["event"] == state::hex(&row.event)
                                && s.rows["accepted-r1"]["receipt_blake3"]
                                    == state::hex(&row.receipt_hash),
                            "original R1 exact saved/accepted bytes/hash"
                        );
                    }
                    for observed in ack
                        .receipts
                        .iter()
                        .filter(|r| r.lane == role && r.ordinal == n)
                    {
                        ensure!(
                            observed.event == row.event
                                && observed.source_format == row.format
                                && observed.encoding == row.receipt_format
                                && observed.digest == row.receipt_hash,
                            "original ACK tuple/source/receipt association"
                        );
                        ensure!(
                            ack_rows
                                .insert(
                                    (role, n),
                                    v2::TaggedItem {
                                        format: row.receipt_format,
                                        raw: receipt.clone()
                                    }
                                )
                                .is_none(),
                            "unique original ACK tuple"
                        );
                    }
                }
                witness_hash.update(&row.event);
                witness_hash.update(&row.receipt_hash);
                accounting.text(c as u8, &row)?;
                graph.insert(row)?;
            }
        }
    }
    let actual = ack
        .receipts
        .iter()
        .map(|r| {
            ack_rows
                .remove(&(r.lane, r.ordinal))
                .context("original ACK source exists in terminal inventory")
        })
        .collect::<Result<Vec<_>>>()?;
    offline::retained::saved_ack(ack, &actual)?;
    witness_hash.update(&ack.digest);
    let graph = graph.finish()?;
    let mut histories = Vec::new();
    let mut native = Vec::new();
    let mut lineages = Vec::new();
    for c in 0..2 {
        let tails = metadata[c]
            .tails
            .each_ref()
            .map(|id| id.context("actual terminal tail"))
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        graph.stored(
            c,
            metadata[c].next,
            tails
                .try_into()
                .map_err(|_| anyhow::anyhow!("two actual tails"))?,
        )?;
        tick(&mut s.relay, s.until)?;
        let snapshot = repo.snapshot(slots[c])?;
        ensure!(
            snapshot.head == heads[c].0,
            "same already-verified actual Head before Native checkpoint"
        );
        let checkpoint = checkpoint::read(checkpoint::Input {
            material: &material,
            source: repo.source_id(),
            head: &heads[c].1,
            origin: &origins[c],
            selected: &selected[c],
            snapshot: &snapshot,
            meta: &metadata[c],
            graph: &graph.contexts[c],
            original,
            migration,
            context: c,
            live: &public[c]["native"],
        })?;
        lineages.push(checkpoint.lineage(
            c as u8,
            slots[c],
            repo.source_id(),
            &origins[c],
            &graph.contexts[c],
        )?);
        native.push(checkpoint);
        tick(&mut s.relay, s.until)?;
    }
    // Original witnesses are no longer borrowed when Controller checks run
    // during history paging; the same repository/lock remains owned throughout.
    for c in 0..2 {
        histories.push(history(
            s,
            &mut repo,
            slots[c],
            c,
            &maps[c],
            &graph.contexts[c],
        )?);
    }
    ensure!(
        repo.generation() == closure.generation
            && repo.last_commit()? == closure.operation
            && closure.point == [672, 672],
        "exact normally reconciled actual stopped root/last_commit/source point"
    );
    let result = Owner {
        identity: Identity {
            source: repo.source_id(),
            migration: repo.migration_id(),
            account: state::parse_id(&profile.account)?,
            claim: state::parse_id(&profile.claim)?,
            device: state::parse_id(&profile.device)?,
            slots,
            profile: digest(&serde_json::to_vec(&profile_value)?),
            config_revision: repo.config_revision(),
        },
        graph,
        closure: closure.clone(),
        generation: repo.generation(),
        last_commit: repo.last_commit()?,
        history: histories
            .try_into()
            .map_err(|_| anyhow::anyhow!("two actual histories"))?,
        witnesses: *witness_hash.finalize().as_bytes(),
        native: native
            .try_into()
            .map_err(|_| anyhow::anyhow!("two Native contexts at the same owner opening"))?,
    };
    drop(repo);
    s.check()?;
    accounting.finish_owner(
        &result.graph,
        lineages
            .try_into()
            .map_err(|_| anyhow::anyhow!("two verified owner/context lineages"))?,
    )?;
    Ok(result)
}
