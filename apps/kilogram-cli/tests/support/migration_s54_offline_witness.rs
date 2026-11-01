//! Bounded exclusive P7 source witness for this new attempt only, not a state export.
use super::*;
use kilogram_cli::ordered_dialog::{handshake::Spec, state::Head};
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_protocol::ordered_dialog_v2 as v2;
use kilogram_state::indexed_dialog::migration7::live1 as store;
use zeroize::Zeroizing;
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
pub(super) fn existing(
    repo: &mut store::L1Repository,
    slot: u8,
    head: &Head,
    spec: &[u8],
) -> Result<v2::VerifiedExistingScope1> {
    let mut genesis = Vec::new();
    let mut ready = Vec::new();
    for role in 0..2 {
        genesis.push(old::b(
            repo.event_at(slot, role, 0)?.context("original genesis")?,
        ));
        ready.push(old::b(
            repo.control(slot, store::L1Control::Ready(role))?
                .context("original Ready")?,
        ));
    }
    let raw = old::encode(
        &old::a([
            old::u(1),
            old::b(spec),
            old::b(repo.signed_control(slot, head.offer.context("original Offer")?)?),
            old::b(repo.signed_control(slot, head.grant.context("original Grant")?)?),
            old::b(
                repo.control(slot, store::L1Control::InitialPair)?
                    .context("original pair")?,
            ),
            old::a(genesis),
            old::a(ready),
        ]),
        v2::MAX_EXISTING_ORIGIN,
    )?;
    Ok(v2::VerifiedExistingScope1::verify_export(&raw)?)
}
fn native_content(
    material: &store::L1OwnerMaterial,
    source_id: old::Id,
    head: &Head,
    q: old::Id,
    snapshot: &store::L1Snapshot,
) -> Result<old::Id> {
    // Exact current Mixed7 read-only envelope/AAD from runtime7/native.rs.
    // No OrderedAccount/Session construction or packet operation is performed.
    let raw = snapshot.native.as_deref().context("actual headed Native")?;
    ensure!(
        raw.len() <= state::MAX_NATIVE && head.blob == Some(old::hash(store::MIXED_NATIVE_ID, raw)),
        "actual Native envelope pointer/hash"
    );
    let envelope = old::decode(raw, state::MAX_NATIVE)?;
    let f = envelope.fields(5)?;
    ensure!(
        f[0].uint()? == 7 && f[1].uint()? == 3 && f[2].uint()? == head.native_generation,
        "current Mixed7 envelope"
    );
    let aad = old::encode(
        &old::a([
            old::u(7),
            old::b(material.incarnation),
            old::b(material.nonce),
            old::b(source_id),
            old::b(head.c),
            old::b(head.local_cert),
            old::u(head.role),
            old::u(head.phase),
            old::u(head.native_generation),
            old::b(head.offer.context("Offer")?),
            old::b(head.grant.context("Grant")?),
            old::b(head.d.context("D")?),
            old::b(head.e.context("E")?),
            old::b(head.sid.as_ref().context("SID")?.as_bytes()),
            old::b(q),
        ]),
        2048,
    )?;
    let device = DeviceEncryptionIdentity::from_secret_bytes(*material.encryption);
    let plain = Zeroizing::new(device.open(
        &SealedMessage {
            encapsulated_key: f[3].bytes()?.to_vec(),
            ciphertext: f[4].bytes()?.to_vec(),
        },
        b"kg:od2:migrated-private\0",
        &aad,
    )?);
    let value = Zeroizing::new(old::decode(&plain, state::MAX_NATIVE)?);
    let p = value.fields(7)?;
    ensure!(
        p[0].uint()? == 7
            && p[1].uint()? == 3
            && !p[3].bytes()?.is_empty()
            && !p[4].bytes()?.is_empty()
            && p[6].option()?.is_none(),
        "actual A Mixed7 payload"
    );
    let _key = Zeroizing::new(p[2].fixed::<32>()?);
    let guard = kilogram_ratchet::ordered_v2::Guard2::restore(p[5].bytes()?, 0)?;
    ensure!(
        [guard.own().next, guard.peer().next] == snapshot.meta.next
            && [guard.own().last, guard.peer().last] == snapshot.meta.tails,
        "opened Native guard matches actual readable source frontier/tails"
    );
    Ok(digest(&plain))
}
pub(super) fn capture(
    node: &Node,
    contexts: &[String; 2],
    public: &[Value; 2],
) -> Result<Snapshot> {
    source::owned(node)?;
    ensure!(node.indexed, "exclusive current P7 source only");
    let before = kilogram_ratchet::ordered::native_call_counts();
    let profile = Profile3::load(&node.profile)?;
    let mut repo = profile.open(&node.profile)?;
    let context = state::parse_id(&contexts[0])?;
    ensure!(
        repo.contexts().len() == 2
            && repo.contexts().get(node.slot as usize) == Some(&(context, 0)),
        "actual original A C1 slot"
    );
    let slot = node.slot;
    let spec_raw = repo.spec(slot)?;
    let spec = Spec::parse(&spec_raw)?;
    ensure!(
        spec.role(profile.device.parse()?)? == 0
            && profile.account == node.branch.account
            && profile.device == node.branch.device,
        "same actual original A owner/profile"
    );
    let snapshot = repo.snapshot(slot)?;
    let head = Head::parse(&snapshot.head, context, spec.cert_refs()?[0], 0)?;
    let mode = repo.existing_state(slot)?;
    let counts = repo.counts(slot)?;
    ensure!(
        mode.mode == store::ExistingMode::Active
            && snapshot.meta.next == [641, 640]
            && counts.queued == 0
            && counts.pending == 0
            && counts.debt == 256
            && repo.queue_page(slot, None, 32)?.is_empty(),
        "actual durable offline256, not queued state"
    );
    ensure!(
        repo.event_at(slot, 1, 640)?.is_none(),
        "B640 remains absent at A"
    );
    let origin = existing(&mut repo, slot, &head, &spec_raw)?;
    let q = mode.q.context("actual selected Q")?;
    let selected = origin.selection(v2::ParsedModeProof2::parse(
        &repo.existing_control(slot, q)?,
    )?)?;
    let anchor_raw = repo
        .event_at(slot, 0, 384)?
        .context("actual pre-offline A384 anchor")?;
    let anchor_event = v2::ParsedEvent2::parse(&anchor_raw)?;
    let anchor_author = v2::ParsedCredentialBundle::parse(
        &repo
            .auth(slot, anchor_event.core().author_auth)?
            .context("anchor author")?,
    )?
    .verify_root()?;
    let anchor = anchor_event.verify(&selected, &anchor_author)?.event().id();
    let mut sources = Vec::with_capacity(256);
    let mut after = None;
    for _ in 0..8 {
        s53_deadline::check()?;
        let page = repo.debt_page(slot, after, 32)?;
        ensure!(page.len() == 32, "eight real debt pages32");
        for (ordinal, id) in page {
            ensure!(
                after.is_none_or(|previous| ordinal > previous),
                "strict actual debt cursor"
            );
            let raw = repo
                .event_at(slot, 0, ordinal)?
                .context("actual materialized offline source")?;
            let event = v2::ParsedEvent2::parse(&raw)?;
            let author = v2::ParsedCredentialBundle::parse(
                &repo
                    .auth(slot, event.core().author_auth)?
                    .context("actual source author")?,
            )?
            .verify_root()?;
            let verified = event.verify(&selected, &author)?;
            let core = verified.event().core();
            let request = oracle::Request::new(0, 0, ordinal.try_into()?)?;
            let request_id = source::parse_request_id(&source::request_id(request))?;
            let local = repo
                .request(slot, request_id)?
                .context("actual durable original request")?;
            let text = repo.projection(slot, id)?.text;
            ensure!(
                core.c == context
                    && core.role == 0
                    && core.n == ordinal
                    && core.kind == 3
                    && verified.event().id() == id
                    && core.request == request_id
                    && local.event == Some(id)
                    && local.text_hash == old::hash(b"kg:od1:request-text\0", text.as_bytes())
                    && text == request.body()
                    && repo.receipt(slot, 0, ordinal)?.is_none(),
                "actual source/request/body/receipt association"
            );
            sources.push(Source {
                ordinal,
                request: request_id,
                id,
                previous: core.previous.context("actual previous source")?,
                source_hash: digest(&raw),
                ciphertext_hash: digest(old::decode(&raw, v2::MAX_EVENT)?.fields(5)?[3].bytes()?),
                text_hash: local.text_hash,
            });
            after = Some(ordinal);
        }
    }
    ensure!(
        repo.debt_page(slot, after, 1)?.is_empty(),
        "no hidden extra debt after256"
    );
    let second = repo
        .contexts()
        .iter()
        .position(|(c, role)| state::hex(c) == contexts[1] && *role == 0)
        .context("actual same-owner C2 slot")? as u8;
    let c2 = repo.snapshot(second)?;
    let c2counts = repo.counts(second)?;
    ensure!(
        c2.meta.next == [1, 1]
            && c2counts.sources == 2
            && c2counts.queued == 0
            && c2counts.pending == 0
            && c2counts.debt == 0
            && repo.existing_state(second)?.mode == store::ExistingMode::L1,
        "same original C2 still zero Text"
    );
    let material = repo.owner_material()?;
    let content = native_content(&material, repo.source_id(), &head, q, &snapshot)?;
    let identity = digest(&serde_json::to_vec(
        &json!({"account":profile.account,"device":profile.device,"keys":public[0]["native"]["keys"],"spec":state::hex(&digest(&spec_raw))}),
    )?);
    let c2_identity = digest(&serde_json::to_vec(
        &json!({"context":public[1]["context"],"sid":public[1]["sid"],"d":public[1]["d"],"e":public[1]["e"],"keys":public[1]["native"]["keys"],"next":c2.meta.next}),
    )?);
    let out = Snapshot {
        state: node.branch.state.canonicalize()?,
        scope: Scope {
            source: repo.source_id(),
            migration: repo.migration_id(),
            context,
            q,
            d: head.d.context("actual D")?,
            e: head.e.context("actual E")?,
            sid: head.sid.context("actual SID")?,
            identity,
        },
        anchor,
        sources,
        native_content: content,
        native_seal: digest(snapshot.native.as_deref().context("actual opaque Native")?),
        c2_identity,
    };
    out.validate()?;
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "bounded witness causes no Native packet/constructor calls"
    );
    Ok(out)
}
