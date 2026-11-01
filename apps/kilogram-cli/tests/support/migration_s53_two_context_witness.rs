//! Stopped G3 reads: one repository open per actual owner, no Native construction.
use super::*;
use kilogram_identity::AccountAuthoritySnapshot;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq)]
struct PrivateRepair {
    slot: u8,
    free: u64,
    outstanding: u64,
    pending: u64,
    head: Option<u64>,
    grant: Option<(old::Id, u64, u64, [u8; 32])>,
}
impl Drop for PrivateRepair {
    fn drop(&mut self) {
        if let Some((_, _, _, key)) = &mut self.grant {
            zeroize::Zeroize::zeroize(key);
        }
    }
}
#[derive(Clone)]
pub(super) struct Checkpoint {
    public: [Value; 3],
    repair: [Vec<PrivateRepair>; 3],
}
impl std::ops::Deref for Checkpoint {
    type Target = [Value; 3];
    fn deref(&self) -> &Self::Target {
        &self.public
    }
}
impl serde::Serialize for Checkpoint {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.public, serializer)
    }
}
pub(super) fn same_repair(before: &Checkpoint, after: &Checkpoint, owner: usize) -> Result<()> {
    ensure!(
        before.repair.get(owner).context("G3 repair owner")?
            == after.repair.get(owner).context("G3 repair owner")?,
        "G3 owner{owner} private repair fields changed"
    );
    Ok(())
}
pub(super) fn same_repair_slot(
    before: &Checkpoint,
    after: &Checkpoint,
    owner: usize,
    slot: u8,
) -> Result<()> {
    let a = before.repair[owner]
        .iter()
        .find(|r| r.slot == slot)
        .context("G3 prior repair slot")?;
    let b = after.repair[owner]
        .iter()
        .find(|r| r.slot == slot)
        .context("G3 current repair slot")?;
    ensure!(a == b, "G3 owner{owner}/slot{slot} private repair changed");
    Ok(())
}

fn credential(
    repo: &mut store::L1Repository,
    slot: u8,
    reference: old::Id,
    current: bool,
) -> Result<wire::RootVerifiedCredential> {
    let raw = repo
        .auth(slot, reference)?
        .context("G3 captured credential")?;
    let cred = wire::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
    ensure!(
        cred.reference() == reference,
        "G3 exact credential reference"
    );
    if current {
        let raw = repo.authority(*cred.certificate().account_id().as_bytes())?;
        let floor = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
        ensure!(
            cred.authority().revision() >= floor.revision()
                && (cred.authority().revision() != floor.revision()
                    || cred.authority().encode()? == raw),
            "G3 current C2 authority floor"
        );
    }
    Ok(cred)
}
pub(super) fn collect(
    owners: &Owners,
    nodes: &[Node; 3],
    mappings: &[oracle::Mapping],
    base: &Path,
    contexts: &[String; 2],
    expected: &[oracle::Directed],
    prior_y: Option<&Checkpoint>,
) -> Result<Checkpoint> {
    owners.stopped()?;
    let scopes = ["c1", "c2"].map(|name| -> Result<_> {
        wire::VerifiedExistingScope1::verify_export(&std::fs::read(
            base.join(format!("{name}.origin.cbor")),
        )?)
        .map_err(Into::into)
    });
    let [one, two] = scopes;
    let scopes = [one?, two?];
    for (i, scope) in scopes.iter().enumerate() {
        ensure!(
            scope.c() == state::parse_id(&contexts[i])?,
            "G3 separate origin scope"
        );
    }
    let mut reports = Vec::new();
    let mut private_repair: [Vec<PrivateRepair>; 3] = std::array::from_fn(|_| vec![]);
    for (owner, node) in nodes.iter().enumerate() {
        s53_deadline::check()?;
        if owner == 0
            && let Some(saved) = prior_y
        {
            ensure!(saved[0]["owner"] == 0, "G3 preserved stopped Y identity");
            reports.push(saved[0].clone());
            private_repair[0] = saved.repair[0].clone();
            continue;
        }
        let profile = Profile3::load(&node.profile)?;
        ensure!(
            profile.account == node.branch.account && profile.device == node.branch.device,
            "G3 owner{owner} fixture/profile identity"
        );
        let mut repo = profile.open(&node.profile)?;
        let own = mappings
            .iter()
            .filter(|m| m.owner == owner)
            .collect::<Vec<_>>();
        ensure!(
            repo.contexts().len() == own.len(),
            "G3 owner{owner} exact repository contexts"
        );
        let mut context_rows = BTreeMap::new();
        let mut accounts = BTreeSet::new();
        for m in own {
            let slot = m.slot;
            ensure!(
                repo.contexts().get(slot as usize) == Some(&(state::parse_id(&m.context)?, m.role)),
                "G3 owner{owner} stored mapping"
            );
            let spec = kilogram_cli::ordered_dialog::handshake::Spec::parse(&repo.spec(slot)?)?;
            ensure!(
                spec.c == state::parse_id(&m.context)?
                    && spec.role(profile.device.parse()?)? == u64::from(m.role),
                "G3 actual Spec role/device"
            );
            accounts.extend(spec.peers.iter().map(|p| *p.cert.account_id().as_bytes()));
            let certs = spec.cert_refs()?;
            let peer = &spec.peers[1 - m.role as usize].cert;
            let identity = json!({"account":profile.account,"device":profile.device,"own_certificate":state::hex(&certs[m.role as usize]),"peer_certificate":state::hex(&certs[1-m.role as usize]),"peer_account":peer.account_id().to_string(),"peer_device":peer.device_id().to_string()});
            let index = contexts
                .iter()
                .position(|c| c == &m.context)
                .context("G3 scope index")?;
            let scope = &scopes[index];
            ensure!(
                spec.cert_refs()? == scope.certs(),
                "G3 exact original peers"
            );
            let selected = repo.existing_state(slot)?;
            ensure!(
                selected.mode == store::ExistingMode::Active,
                "G3 stopped selected Active"
            );
            let q = selected.q.context("G3 Q")?;
            let proof_raw = repo.existing_control(slot, q)?;
            let proof = wire::ParsedModeProof2::parse(&proof_raw)?;
            ensure!(proof.cuts().map(Some) == selected.cuts, "G3 proof cuts");
            let selection = scope.selection(proof)?;
            let consent_ref = selected.current_consent.context("G3 current Consent2")?;
            let consent = wire::ParsedConsent2::parse(&repo.existing_control(slot, consent_ref)?)?;
            scope.verify_consent(&consent, scope.peers())?;
            ensure!(
                consent.reference() == consent_ref,
                "G3 current Consent2 reference"
            );
            // The verified existing ModeProof schema embeds its selected Consent2 at field5.
            let proof_value = old::decode(&proof_raw, wire::MAX_PROOF)?;
            let embedded = wire::ParsedConsent2::parse(proof_value.fields(6)?[5].bytes()?)?;
            ensure!(
                selected.selected_consent == Some(embedded.reference())
                    && embedded.reference() == consent_ref,
                "G3 stored selected/current Consent2 join"
            );
            let committed =
                repo.existing_control(slot, selected.committed.context("G3 Committed")?)?;
            let parsed = wire::ParsedControl2::parse(&committed)?;
            ensure!(
                Some(parsed.id()) == selected.committed,
                "G3 Committed reference"
            );
            parsed.require_kind(21)?;
            parsed.verify_committed(&selection, &scope.peers()[1])?;
            let snap = repo.snapshot(slot)?;
            let head = state::Head::parse(
                &snap.head,
                scope.c(),
                spec.cert_refs()?[m.role as usize],
                u64::from(m.role),
            )?;
            ensure!(
                head.d == Some(scope.d())
                    && head.e == Some(scope.e())
                    && head.sid.as_deref() == Some(scope.sid()),
                "G3 original Head/selection identity"
            );
            let native = snap.native.as_deref().context("G3 opaque Native")?;
            ensure!(
                head.blob == Some(old::hash(store::MIXED_NATIVE_ID, native)),
                "G3 Native/head hash"
            );
            let history = oracle::history(
                serde_json::from_slice(&repo.history(slot, None, 32)?)?,
                snap.meta.next,
            )?;
            let mut sources = Vec::new();
            for role in 0..2usize {
                let next = snap.meta.next[role];
                ensure!(
                    (1..=6).contains(&next),
                    "G3 tiny source frontier includes genesis"
                );
                let ordinal = next - 1;
                let source = repo
                    .event_at(slot, role as u8, ordinal)?
                    .context("G3 signed last source, including ordinal0")?;
                let value = old::decode(&source, wire::MAX_EVENT)?;
                let format: u8 = value.fields(5)?[0].uint()?.try_into()?;
                let reference = if format == 1 {
                    old::Event::parse(&source, scope.peers()[role].certificate().device_id())?
                        .core
                        .author_auth
                } else {
                    wire::ParsedEvent2::parse(&source)?.core().author_auth
                };
                let auth = credential(&mut repo, slot, reference, false)?;
                let event = wire::TaggedItem {
                    format,
                    raw: source,
                }
                .verify_event(&selection, &auth)?;
                let (id, n, kind) = match event {
                    wire::MixedEvent::Legacy(e) => {
                        (e.event().id()?, e.event().core.n, e.event().core.kind)
                    }
                    wire::MixedEvent::Current(e) => (
                        e.event().id(),
                        e.event().core().n,
                        u64::from(e.event().core().kind),
                    ),
                };
                ensure!(
                    n == ordinal && snap.meta.tails[role] == Some(id),
                    "G3 actual signed source/tail correspondence"
                );
                if ordinal == 0 {
                    ensure!(
                        id == scope.old_control_ids()[2 + role] && kind != 3,
                        "G3 actual ordinal-zero control source"
                    );
                }
                sources.push(json!({"role":role,"next":next,"last":state::hex(&id),"ordinal":ordinal,"kind":kind}));
            }
            let mut records = Vec::new();
            let mut requests = BTreeMap::new();
            for row in history.values() {
                let raw = repo
                    .event_at(slot, row.role, row.ordinal)?
                    .context("G3 signed source")?;
                let id = state::parse_id(&row.id)?;
                let projection = repo.projection(slot, id)?;
                ensure!(projection.text == row.text, "G3 projection/body");
                let receipt = repo
                    .receipt(slot, row.role, row.ordinal)?
                    .context("G3 stored delivery receipt")?;
                let mut record = json!({"id":row.id,"source":hash(&raw),"projection":hash(&projection.encode()?),"receipt":hash(&receipt)});
                if row.format == 1 {
                    let parsed = old::Event::parse(
                        &raw,
                        scope.peers()[row.role as usize].certificate().device_id(),
                    )?;
                    let author = credential(&mut repo, slot, parsed.core.author_auth, false)?;
                    let source = scope.verify_event(&raw, &author)?;
                    ensure!(
                        source.event().id()? == id
                            && source.event().core.role == u64::from(row.role)
                            && source.event().core.n == row.ordinal
                            && source.event().core.sequence == row.sequence
                            && source.event().core.kind == 3
                            && source.event().core.request == request_from_hex(&row.request)?,
                        "G3 historical signed source association"
                    );
                    let decoded = old::decode(&receipt, 4096)?;
                    let receiver = credential(
                        &mut repo,
                        slot,
                        decoded.fields(2)?[0].fields(8)?[7].fixed()?,
                        false,
                    )?;
                    wire::ParsedReceipt1::parse(&receipt)?
                        .verify_pre_q(&source, scope, &receiver)?;
                    ensure!(
                        row.ordinal < selection.proof().proof().cuts()[row.role as usize]
                            && row.proof_kind == "LegacySnapshot"
                            && repo.data_proof(slot, id)?.is_none(),
                        "G3 historical snapshot is below cut, not full new proof"
                    );
                    record["historical_origin"] = json!(hash(&std::fs::read(
                        base.join(format!("c{}.origin.cbor", index + 1))
                    )?));
                } else {
                    let wanted = expected
                        .iter()
                        .find(|e| {
                            e.context == m.context
                                && e.saved.event.as_deref() == Some(row.id.as_str())
                        })
                        .context("G3 no unissued Event2")?;
                    ensure!(
                        mapping(mappings, wanted.author, &wanted.context)?.role == wanted.role
                            && (m.owner == wanted.author) == (m.role == wanted.role),
                        "G3 directed author/owner role"
                    );
                    let parsed = wire::ParsedEvent2::parse(&raw)?;
                    let author = credential(&mut repo, slot, parsed.core().author_auth, true)?;
                    let verified = parsed.verify(&selection, &author)?;
                    wanted.core(verified.event().core())?;
                    ensure!(
                        verified.event().id() == id
                            && row.sequence == verified.event().core().sequence
                            && row.ordinal == verified.event().core().n
                            && row.role == verified.event().core().role,
                        "G3 history/signed source join"
                    );
                    if let Some(previous) = wanted.previous {
                        let previous_raw = repo
                            .event_at(
                                slot,
                                wanted.role,
                                wanted
                                    .ordinal
                                    .checked_sub(1)
                                    .context("G3 predecessor ordinal")?,
                            )?
                            .context("G3 preceding signed source")?;
                        let prior = wire::TaggedItem {
                            format: old::decode(&previous_raw, wire::MAX_EVENT)?.fields(5)?[0]
                                .uint()?
                                .try_into()?,
                            raw: previous_raw,
                        };
                        let reference = if prior.format == 1 {
                            old::Event::parse(
                                &prior.raw,
                                scope.peers()[wanted.role as usize]
                                    .certificate()
                                    .device_id(),
                            )?
                            .core
                            .author_auth
                        } else {
                            wire::ParsedEvent2::parse(&prior.raw)?.core().author_auth
                        };
                        let auth = credential(&mut repo, slot, reference, false)?;
                        let preceding = prior.verify_event(&selection, &auth)?;
                        let actual = match preceding {
                            wire::MixedEvent::Legacy(e) => e.event().id()?,
                            wire::MixedEvent::Current(e) => e.event().id(),
                        };
                        ensure!(actual == previous, "G3 authenticated previous tail");
                    }
                    let parsed = wire::ParsedReceipt2::parse(&receipt)?;
                    let receiver =
                        credential(&mut repo, slot, parsed.fields().receiver_auth, true)?;
                    parsed.verify(&verified, &selection, &receiver)?;
                    let refs = repo.data_proof(slot, id)?.context("G3 FullMixed proof")?;
                    oracle::proof_joins(
                        &refs,
                        author.reference(),
                        receiver.reference(),
                        old::hash(b"", &receipt),
                        row.role == m.role,
                    )?;
                    for (reference, cert) in [
                        (Some(refs.original), author.cert_ref()),
                        (refs.admission, author.cert_ref()),
                        (refs.final_auth, author.cert_ref()),
                        (refs.receiver, receiver.cert_ref()),
                        (refs.receiver_overlay, receiver.cert_ref()),
                    ] {
                        if let Some(reference) = reference {
                            ensure!(
                                credential(&mut repo, slot, reference, true)?.cert_ref() == cert,
                                "G3 proof capture selected peer"
                            );
                        }
                    }
                    record["proof"] = json!(hash(&serde_json::to_vec(&refs)?));
                    record["signed_previous"] = json!(wanted.previous.map(|id| state::hex(&id)));
                }
                if row.role == m.role {
                    let saved = repo
                        .request(slot, request_from_hex(&row.request)?)?
                        .context("G3 own request")?;
                    oracle::stored_own_request(&saved, id, &row.text, row.format).with_context(
                        || {
                            format!(
                                "G3 owner{owner}/context{}/slot{slot}/format{} own request",
                                m.context, row.format
                            )
                        },
                    )?;
                    if row.format == 2 {
                        oracle::stored_request(
                            &saved,
                            expected
                                .iter()
                                .find(|e| {
                                    e.context == m.context
                                        && e.saved.event.as_deref() == Some(row.id.as_str())
                                })
                                .context("G3 stored directed request")?,
                        )?;
                    }
                    requests.insert(
                        row.request.clone(),
                        json!({"queue":saved.queue_sequence,"event":row.id,"delivered":true}),
                    );
                }
                records.push(record);
            }
            let repair = repo.repair_state(slot)?;
            private_repair[owner].push(PrivateRepair {
                slot,
                free: repair.repair_free,
                outstanding: repair.outstanding,
                pending: repair.pending,
                head: repair.head,
                grant: repair.grant,
            });
            let repair = json!({"repair_free":repair.repair_free,"outstanding":repair.outstanding,"pending":repair.pending,"head":repair.head,"grant_present":repair.grant.is_some()});
            let routing = if m.context == contexts[1] {
                Some(routes::stored(&mut repo, slot, scope, m.role)?)
            } else {
                None
            };
            context_rows.insert(m.context.clone(), json!({"mapping":m,"identity":identity,"routing":routing,"existing":selected,"metadata":snap.meta,"fixed":repo.fixed_records(slot)?,"history_initialized":repo.history_initialized(slot)?,"sources":sources,"policy_generation":snap.meta.policy_generation,"repair":repair,"stable":{"context":m.context,"role":m.role,"head":hash(&snap.head),"native":hash(native),"native_hash":head.blob.map(|id|state::hex(&id)),"native_generation":head.native_generation,"next":snap.meta.next,"tails":snap.meta.tails.map(|id| id.map(|id| state::hex(&id))),"next_queue":snap.meta.next_queue,"q":state::hex(&q),"cuts":selected.cuts,"current_consent":state::hex(&consent_ref),"current_consent2":state::hex(&consent_ref),"selected_consent":selected.selected_consent.map(|id| state::hex(&id)),"proof":hash(&proof_raw),"committed":hash(&committed),"history":history,"requests":requests,"records":records}}));
        }
        let mut authority = BTreeMap::new();
        for account in accounts {
            let raw = repo.authority(account)?;
            let pin = AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            ensure!(
                pin.account_id().as_bytes() == &account && pin.encode()? == raw,
                "G3 exact stored authority"
            );
            authority.insert(
                state::hex(&account),
                json!({"revision":pin.revision(),"hash":hash(&raw)}),
            );
        }
        let config: db::migration7::MigrationConfig =
            serde_json::from_slice(&std::fs::read(&node.branch.config)?)?;
        ensure!(
            config.logical_quota == profile.logical_quota
                && config.physical_capacity == profile.physical_capacity
                && config.output_profile == node.profile
                && config.control_ipc_file == node.ipc
                && config.locator_directory == node.locators,
            "G3 actual Node config/profile correspondence"
        );
        let (revision, config_hash) = repo.config_status()?;
        reports.push(json!({"owner":owner,"contexts":context_rows,"authority":authority,"next_author":repo.next_author_sequence()?,"initialized":repo.initialized()?,"profile":serde_json::from_slice::<Value>(&std::fs::read(&node.profile)?)?,"config":config,"capacity":{"revision":revision,"hash":state::hex(&config_hash)},"backend":repo.backend_stats()?,"credits":repo.credits()?,"accounting":repo.accounting()?,"generation":repo.generation()}));
    }
    Ok(Checkpoint {
        public: reports
            .try_into()
            .map_err(|_| anyhow::anyhow!("G3 three stopped owners"))?,
        repair: private_repair,
    })
}
fn request_from_hex(text: &str) -> Result<[u8; 16]> {
    ensure!(
        text.len() == 32
            && text
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "G3 request encoding"
    );
    let mut out = [0; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)?;
    }
    Ok(out)
}
#[cfg(test)]
#[test]
fn g3_private_repair_key_drift_is_detected_without_serializing_or_hashing_it() -> Result<()> {
    let mut before = Checkpoint {
        public: [Value::Null, Value::Null, Value::Null],
        repair: std::array::from_fn(|_| vec![]),
    };
    before.repair[1].push(PrivateRepair {
        slot: 0,
        free: 64,
        outstanding: 0,
        pending: 0,
        head: None,
        grant: Some(([1; 32], 1, 2, [7; 32])),
    });
    let mut after = before.clone();
    if let Some((_, _, _, key)) = &mut after.repair[1][0].grant {
        key[0] ^= 1;
    }
    assert!(same_repair(&before, &after, 1).is_err());
    assert!(same_repair_slot(&before, &after, 1, 0).is_err());
    assert_eq!(
        serde_json::to_value(&before)?,
        serde_json::to_value(&after)?
    );
    Ok(())
}
