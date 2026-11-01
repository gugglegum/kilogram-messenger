//! G2 stopped read witnesses. No mutation or Native opening/constructors.
use super::*;
use kilogram_identity::AccountAuthoritySnapshot;

fn credential(
    repo: &mut store::L1Repository,
    reference: old::Id,
) -> Result<wire::RootVerifiedCredential> {
    let raw = repo.auth(0, reference)?.context("G2 captured credential")?;
    let cred = wire::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
    ensure!(cred.reference() == reference, "G2 credential reference");
    let floor = repo.authority(*cred.certificate().account_id().as_bytes())?;
    let authority = AccountAuthoritySnapshot::decode_and_verify(&floor)?;
    ensure!(
        cred.authority().revision() >= authority.revision()
            && (cred.authority().revision() != authority.revision()
                || cred.authority().encode()? == floor),
        "G2 authority floor"
    );
    Ok(cred)
}
pub(super) fn stopped(pair: &Pair, requests: &[oracle::Request]) -> Result<[Value; 2]> {
    ensure!(
        pair.children.iter().all(Option::is_none),
        "G2 stopped owners before repository read"
    );
    let origin = std::fs::read(pair.base.join("initial.origin.cbor"))?;
    let scope = wire::VerifiedExistingScope1::verify_export(&origin)?;
    let mut reports = Vec::new();
    for node in &pair.nodes {
        s53_deadline::check()?;
        let profile_raw = std::fs::read(&node.profile)?;
        let profile = Profile3::load(&node.profile)?;
        let config: db::migration7::MigrationConfig =
            serde_json::from_slice(&std::fs::read(&node.branch.config)?)?;
        let root = kilogram_state::StateDirectoryLock::acquire(&profile.state_dir)?;
        let mut repo = store::L1Repository::open_ready(
            root,
            db::ExpectedOwner {
                account: state::parse_id(&profile.account)?,
                claim: state::parse_id(&profile.claim)?,
            },
            state::parse_id(&profile.device)?,
            config.clone(),
        )?;
        ensure!(
            repo.contexts().len() == 1
                && repo.source_id() == state::parse_id(&profile.source_id)?
                && repo.migration_id() == state::parse_id(&profile.migration_id)?,
            "G2 exact fresh repository origin"
        );
        let selected_state = repo.existing_state(0)?;
        ensure!(
            selected_state.mode == store::ExistingMode::Active,
            "G2 stopped Active"
        );
        let q = selected_state.q.context("G2 selected Q")?;
        let raw_proof = repo.existing_control(0, q)?;
        let proof = wire::ParsedModeProof2::parse(&raw_proof)?;
        ensure!(
            proof.cuts().map(Some) == selected_state.cuts,
            "G2 saved proof cuts"
        );
        let selected = scope.selection(proof)?;
        ensure!(
            selected.proof().q() == q && selected.c() == state::parse_id(&pair.c)?,
            "G2 selected scope/Q"
        );
        let committed_raw =
            repo.existing_control(0, selected_state.committed.context("G2 Committed")?)?;
        let committed = wire::ParsedControl2::parse(&committed_raw)?;
        ensure!(
            Some(committed.id()) == selected_state.committed,
            "G2 Committed reference"
        );
        committed.require_kind(21)?;
        committed.verify_committed(&selected, &scope.peers()[1])?;
        let snap = repo.snapshot(0)?;
        let role = repo.contexts()[0].1;
        let spec = kilogram_cli::ordered_dialog::handshake::Spec::parse(&repo.spec(0)?)?;
        let head = state::Head::parse(
            &snap.head,
            selected.c(),
            spec.cert_refs()?[role as usize],
            u64::from(role),
        )?;
        let native = snap.native.as_deref().context("G2 opaque Native")?;
        ensure!(
            head.blob == Some(old::hash(store::MIXED_NATIVE_ID, native)),
            "G2 authenticated Native hash"
        );
        let history = oracle::history(
            serde_json::from_slice(&repo.history(0, None, 32)?)?,
            snap.meta.next,
            2 + requests.len(),
        )?;
        let mut records = Vec::new();
        let mut own = Vec::new();
        let mut delivered_requests = std::collections::BTreeMap::new();
        for row in history.values() {
            let raw = repo
                .event_at(0, row.role, row.ordinal)?
                .context("G2 persisted source")?;
            let id = state::parse_id(&row.id)?;
            let projection = repo.projection(0, id)?;
            ensure!(projection.text == row.text, "G2 stored exact body");
            let mut record =
                json!({"id":row.id,"source":hash(&raw),"projection":hash(&projection.encode()?)});
            if row.format == 2 {
                let round = requests
                    .iter()
                    .position(|r| r.event.as_deref() == Some(row.id.as_str()))
                    .context("G2 no unissued Event2")?;
                let parsed = wire::ParsedEvent2::parse(&raw)?;
                ensure!(parsed.id() == id, "G2 signed source ID");
                let author = credential(&mut repo, parsed.core().author_auth)?;
                let verified = parsed.verify(&selected, &author)?;
                let core = verified.event().core();
                let previous_n = core.n.checked_sub(1).context("G2 preceding ordinal")?;
                let previous_raw = repo
                    .event_at(0, core.role, previous_n)?
                    .context("G2 preceding lane source")?;
                let previous_id = if round == 0 {
                    let parsed = old::Event::parse(
                        &previous_raw,
                        scope.peers()[core.role as usize].certificate().device_id(),
                    )?;
                    let previous_author = credential(&mut repo, parsed.core.author_auth)?;
                    let previous = scope.verify_event(&previous_raw, &previous_author)?;
                    ensure!(
                        previous.event().core.role == u64::from(core.role)
                            && previous.event().core.n == previous_n
                            && previous.event().core.kind == 3,
                        "G2 authenticated preceding Text1"
                    );
                    previous.event().id()?
                } else {
                    let parsed = wire::ParsedEvent2::parse(&previous_raw)?;
                    let previous_author = credential(&mut repo, parsed.core().author_auth)?;
                    let previous = parsed.verify(&selected, &previous_author)?;
                    ensure!(
                        previous.event().core().role == core.role
                            && previous.event().core().n == previous_n
                            && previous.event().core().request == oracle::request_bytes(round - 1),
                        "G2 authenticated preceding Event2"
                    );
                    previous.event().id()
                };
                ensure!(
                    core.previous == Some(previous_id),
                    "G2 signed previous matches authenticated preceding lane source"
                );
                ensure!(
                    core.role == row.role
                        && core.n == row.ordinal
                        && core.kind == 3
                        && core.request == oracle::request_bytes(round)
                        && core.sequence == row.sequence,
                    "G2 signed exact Event2"
                );
                let receipt = repo
                    .receipt(0, row.role, row.ordinal)?
                    .context("G2 actual Receipt2")?;
                let parsed_receipt = wire::ParsedReceipt2::parse(&receipt)?;
                let receiver = credential(&mut repo, parsed_receipt.fields().receiver_auth)?;
                parsed_receipt.verify(&verified, &selected, &receiver)?;
                let proof = repo.data_proof(0, id)?.context("G2 FullMixed proof")?;
                let mut proof_credentials = Vec::new();
                for reference in [
                    Some(proof.original),
                    proof.admission,
                    proof.final_auth,
                    proof.receiver,
                    proof.receiver_overlay,
                ]
                .into_iter()
                .flatten()
                {
                    let captured = credential(&mut repo, reference)?;
                    ensure!(
                        selected.proof().certs().contains(&captured.cert_ref()),
                        "G2 captured FullMixed credential selected peer"
                    );
                    proof_credentials.push(state::hex(&reference));
                }
                record["receipt"] = json!(hash(&receipt));
                record["proof"] = json!(hash(&serde_json::to_vec(&proof)?));
                record["verified_credentials"] = json!(proof_credentials);
                record["signed_previous"] = json!(state::hex(&previous_id));
                if row.role == role {
                    let saved = repo
                        .request(0, oracle::request_bytes(round))?
                        .context("G2 saved own request")?;
                    ensure!(
                        saved.queue_sequence == Some(requests[round].queue)
                            && saved.event == Some(id)
                            && saved.text_hash
                                == old::hash(b"kg:od1:request-text\0", row.text.as_bytes()),
                        "G2 saved Some(queue)/ID/body"
                    );
                    own.push(serde_json::to_value(&saved)?);
                    delivered_requests.insert(
                        row.request.clone(),
                        oracle::Request {
                            queue: saved.queue_sequence.context("G2 stored queue")?,
                            event: Some(state::hex(&saved.event.context("G2 stored EventId")?)),
                            delivered: true,
                        },
                    );
                }
            } else {
                let event = old::Event::parse(
                    &raw,
                    scope.peers()[row.role as usize].certificate().device_id(),
                )?;
                ensure!(event.id()? == id, "G2 original source ID");
                if row.role == role {
                    let mut id = [0; 16];
                    for (i, byte) in id.iter_mut().enumerate() {
                        *byte = u8::from_str_radix(&row.request[i * 2..i * 2 + 2], 16)?;
                    }
                    own.push(serde_json::to_value(
                        repo.request(0, id)?.context("G2 original own request")?,
                    )?);
                }
            }
            records.push(record);
        }
        let credit = serde_json::to_value(repo.credits()?)?;
        let obligations = oracle::ledger(&credit)?;
        let accounting = repo.accounting()?;
        let (revision, config_hash) = repo.config_status()?;
        let backend = repo.backend_stats()?;
        let stable = json!({"source":state::hex(&repo.source_id()),"migration":state::hex(&repo.migration_id()),"head":hash(&snap.head),"native":hash(native),"native_hash":head.blob.map(|id|state::hex(&id)),"native_generation":head.native_generation,"next":snap.meta.next,"tails":snap.meta.tails,"next_queue":snap.meta.next_queue,"q":state::hex(&q),"cuts":selected_state.cuts,"proof":hash(&raw_proof),"committed":hash(&committed_raw),"history":history,"requests":own,"delivered_requests":delivered_requests,"records":records,"obligations":obligations});
        let shared_records = records
            .iter()
            .map(|r| json!({"id":r["id"],"source":r["source"],"receipt":r.get("receipt")}))
            .collect::<Vec<_>>();
        reports.push(json!({"stable":stable,"shared_records":shared_records,"accounting":accounting,"profile":hash(&profile_raw),"capacity":{"revision":revision,"hash":state::hex(&config_hash),"quotas":config.logical_quota,"physical":backend.physical_quota,"configured_physical":config.physical_capacity},"credits":credit,"generation":repo.generation()}));
    }
    reports
        .try_into()
        .map_err(|_| anyhow::anyhow!("G2 two stopped reports"))
}
pub(super) fn capacity_only(before: &[Value; 2], after: &[Value; 2]) -> Result<()> {
    for role in 0..2 {
        let old = oracle::accounting(&before[role], role)?;
        let new = oracle::accounting(&after[role], role)?;
        oracle::exact_at(
            &format!("owner{role}/runtime-accounting"),
            &serde_json::to_value(&old.runtime)?,
            &serde_json::to_value(&new.runtime)?,
        )?;
        oracle::exact_at(
            &format!("owner{role}/metadata/records"),
            &json!(old.prepaid_metadata.records),
            &json!(new.prepaid_metadata.records),
        )?;
        oracle::exact_at(
            &format!("owner{role}/metadata/funded"),
            &json!(oracle::funded(&old.prepaid_metadata)?),
            &json!(oracle::funded(&new.prepaid_metadata)?),
        )?;
        oracle::exact_at(
            &format!("owner{role}/stable"),
            oracle::field(&before[role], "stable")?,
            oracle::field(&after[role], "stable")?,
        )?;
    }
    Ok(())
}
pub(super) fn check(reports: &[Value; 2]) -> Result<()> {
    for (role, report) in reports.iter().enumerate() {
        oracle::accounting(report, role)?;
    }
    oracle::exact_at(
        "owners0-1/shared-signed-records",
        oracle::field(&reports[0], "shared_records")?,
        oracle::field(&reports[1], "shared_records")?,
    )
}
pub(super) fn resumed(before: &[Value; 2], after: &[Value; 2]) -> Result<()> {
    capacity_only(before, after)?;
    for role in 0..2 {
        for field in ["accounting", "credits", "capacity"] {
            oracle::exact_at(
                &format!("owner{role}/no-capacity/{field}"),
                oracle::field(&before[role], field)?,
                oracle::field(&after[role], field)?,
            )?;
        }
    }
    Ok(())
}
pub(super) fn persisted(before: &[Value; 2], after: &[Value; 2]) -> Result<()> {
    resumed(before, after)?;
    for role in 0..2 {
        oracle::exact_at(
            &format!("owner{role}/restart/profile"),
            oracle::field(&before[role], "profile")?,
            oracle::field(&after[role], "profile")?,
        )?;
    }
    Ok(())
}
