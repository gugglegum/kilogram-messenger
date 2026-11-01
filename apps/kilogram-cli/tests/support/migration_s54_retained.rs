//! Terminal owned read: immutable ordinals, never P7's mutable Native/C2-zero snapshot.
use super::*;
use kilogram_cli::ordered_dialog::{handshake::Spec, state::Head};
use kilogram_protocol::ordered_dialog_v2 as v2;
use kilogram_state::indexed_dialog::migration7::live1 as store;

#[derive(Clone)]
pub(in super::super) struct Immutable {
    state: PathBuf,
    scope: Scope,
    anchor: old::Id,
    sources: Vec<Source>,
}
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
fn saved_ack(ack: &observations::AckObservation, actual: &[v2::TaggedItem]) -> Result<()> {
    ensure!(
        actual.len() == ack.receipts.len()
            && actual
                .iter()
                .zip(&ack.receipts)
                .all(|(raw, r)| raw.format == r.encoding && digest(&raw.raw) == r.digest),
        "each actual saved/accepted receipt byte digest matches observed tuple"
    );
    let batch = old::a(
        actual
            .iter()
            .map(|r| old::a([old::u(u64::from(r.format)), old::b(&r.raw)]))
            .collect::<Vec<_>>(),
    );
    ensure!(
        digest(&old::encode(&batch, 16 * 1024)?) == ack.digest,
        "actual saved/accepted receipt batch digest"
    );
    Ok(())
}
impl Immutable {
    pub(super) fn from_snapshot(s: &Snapshot) -> Self {
        Self {
            state: s.state.clone(),
            scope: s.scope.clone(),
            anchor: s.anchor,
            sources: s.sources.clone(),
        }
    }
    pub fn ids(&self) -> Vec<(u64, old::Id)> {
        self.sources.iter().map(|r| (r.ordinal, r.id)).collect()
    }
    fn same_row(&self, index: usize, actual: &Source) -> Result<()> {
        ensure!(
            self.sources.get(index) == Some(actual),
            "original immutable ordinal/request/chain/source/cipher/body commitment"
        );
        Ok(())
    }
    /// Caller proves both known kill/waits before opening either same directory.
    pub fn read(
        &self,
        s: &control::Controller,
        public: &[[Value; 2]; 2],
        owners: [old::Id; 2],
        c2: &[BTreeMap<String, Value>; 2],
        held: &Value,
        ack: &observations::AckObservation,
    ) -> Result<Value> {
        for owner in 0..2 {
            s.stopped(owner)?;
        }
        let before = kilogram_ratchet::ordered::native_call_counts();
        let mut proofs = Vec::new();
        let mut receipts: Option<Vec<old::Id>> = None;
        for owner in 0..2 {
            let node = s.view(owner, 0)?;
            source::owned(&node)?;
            let profile = Profile3::load(&node.profile)?;
            let mut repo = profile.open(&node.profile)?;
            ensure!(
                profile.account == node.branch.account
                    && profile.device == node.branch.device
                    && repo.source_id() == owners[owner],
                "same actual stopped owner/source/profile"
            );
            let slot = node.slot;
            let spec_raw = repo.spec(slot)?;
            let spec = Spec::parse(&spec_raw)?;
            let context = state::parse_id(&s.origin.contexts[0])?;
            ensure!(
                repo.contexts().len() == 2
                    && repo.contexts().get(slot as usize) == Some(&(context, owner as u8))
                    && spec.role(profile.device.parse()?)? == owner as u64,
                "actual original logical C1 slot/role"
            );
            let snap = repo.snapshot(slot)?;
            let head = Head::parse(&snap.head, context, spec.cert_refs()?[owner], owner as u64)?;
            let mode = repo.existing_state(slot)?;
            let q = mode.q.context("actual selected Q")?;
            ensure!(
                mode.mode == store::ExistingMode::Active
                    && context == self.scope.context
                    && q == self.scope.q
                    && head.d == Some(self.scope.d)
                    && head.e == Some(self.scope.e)
                    && head.sid.as_ref() == Some(&self.scope.sid),
                "same original selected C/D/E/SID/Q"
            );
            if owner == 0 {
                let identity = digest(&serde_json::to_vec(
                    &json!({"account":profile.account,"device":profile.device,"keys":public[owner][0]["native"]["keys"],"spec":state::hex(&digest(&spec_raw))}),
                )?);
                ensure!(
                    node.branch.state.canonicalize()? == self.state
                        && repo.source_id() == self.scope.source
                        && repo.migration_id() == self.scope.migration
                        && identity == self.scope.identity,
                    "original P7 state/migration/public identity"
                );
            }
            let counts = repo.counts(slot)?;
            ensure!(
                snap.meta.next == if owner == 0 { [641, 640] } else { [641, 641] }
                    && counts.queued == 0
                    && counts.pending == 0
                    && counts.debt == owner as u64
                    && counts.sources == 1281 + owner as u64
                    && repo.queue_page(slot, None, 1)?.is_empty(),
                "actual terminal frontiers/empty queues/debt0 or original1"
            );
            if owner == 0 {
                ensure!(
                    repo.debt_page(slot, None, 1)?.is_empty()
                        && repo.event_at(slot, 1, 640)?.is_none(),
                    "retired A debt and B640 still absent"
                );
            }
            let origin = witness::existing(&mut repo, slot, &head, &spec_raw)?;
            let selected = origin.selection(v2::ParsedModeProof2::parse(
                &repo.existing_control(slot, q)?,
            )?)?;
            let anchor = repo.event_at(slot, 0, 384)?.context("retained anchor384")?;
            ensure!(
                v2::ParsedEvent2::parse(&anchor)?.id() == self.anchor,
                "same original anchor384"
            );
            let mut receipt_hashes = Vec::with_capacity(256);
            for (index, expected) in self.sources.iter().enumerate() {
                s53_deadline::check()?;
                let n = expected.ordinal;
                let raw = repo
                    .event_at(slot, 0, n)?
                    .context("retained immutable ordinal, independent of debt index")?;
                let parsed = v2::ParsedEvent2::parse(&raw)?;
                let author = v2::ParsedCredentialBundle::parse(
                    &repo
                        .auth(slot, parsed.core().author_auth)?
                        .context("actual author credential")?,
                )?
                .verify_root()?;
                let verified = parsed.verify(&selected, &author)?;
                let core = verified.event().core();
                let id = verified.event().id();
                let projection = repo.projection(slot, id)?;
                ensure!(
                    core.c == context
                        && core.role == 0
                        && core.n == n
                        && core.kind == 3
                        && projection.text == oracle::Request::new(0, 0, n.try_into()?)?.body(),
                    "actual received source/body scope"
                );
                let actual = Source {
                    ordinal: n,
                    request: core.request,
                    id,
                    previous: core.previous.context("actual previous")?,
                    source_hash: digest(&raw),
                    ciphertext_hash: digest(
                        old::decode(&raw, v2::MAX_EVENT)?.fields(5)?[3].bytes()?,
                    ),
                    text_hash: old::hash(b"kg:od1:request-text\0", projection.text.as_bytes()),
                };
                self.same_row(index, &actual)?;
                if owner == 0 {
                    let request = repo
                        .request(slot, actual.request)?
                        .context("retained original sender request")?;
                    ensure!(
                        request.event == Some(id) && request.text_hash == actual.text_hash,
                        "same durable sender request association"
                    );
                }
                let receipt = repo
                    .receipt(slot, 0, n)?
                    .context("actual saved/accepted offline receipt")?;
                let parsed = v2::ParsedReceipt2::parse(&receipt)?;
                let receiver = v2::ParsedCredentialBundle::parse(
                    &repo
                        .auth(slot, parsed.fields().receiver_auth)?
                        .context("actual receipt receiver credential")?,
                )?
                .verify_root()?;
                let _verified = v2::TaggedItem {
                    format: 2,
                    raw: receipt.clone(),
                }
                .verify_receipt(
                    &v2::MixedEvent::Current(verified),
                    &selected,
                    &receiver,
                )?;
                receipt_hashes.push(digest(&receipt));
            }
            if let Some(previous) = &receipts {
                ensure!(
                    *previous == receipt_hashes,
                    "identical saved B/accepted A signed receipts"
                );
            } else {
                receipts = Some(receipt_hashes.clone());
            }
            let mut batch = Vec::new();
            for observed in &ack.receipts {
                let raw = repo
                    .event_at(slot, 0, observed.ordinal)?
                    .context("actual historical ACK source")?;
                let auth = if observed.source_format == 1 {
                    old::Event::parse(&raw, spec.peers[0].cert.device_id())?
                        .core
                        .author_auth
                } else {
                    v2::ParsedEvent2::parse(&raw)?.core().author_auth
                };
                let author = v2::ParsedCredentialBundle::parse(
                    &repo
                        .auth(slot, auth)?
                        .context("historical author credential")?,
                )?
                .verify_root()?;
                let source = v2::TaggedItem {
                    format: observed.source_format,
                    raw,
                }
                .verify_event(&selected, &author)?;
                let raw = repo
                    .receipt(slot, 0, observed.ordinal)?
                    .context("actual historical saved/accepted ACK receipt")?;
                let receiver_auth = if observed.encoding == 1 {
                    old::decode(&raw, 1024)?.fields(2)?[0].fields(8)?[7].fixed()?
                } else {
                    v2::ParsedReceipt2::parse(&raw)?.fields().receiver_auth
                };
                let receiver = v2::ParsedCredentialBundle::parse(
                    &repo
                        .auth(slot, receiver_auth)?
                        .context("historical receiver credential")?,
                )?
                .verify_root()?;
                let item = v2::TaggedItem {
                    format: observed.encoding,
                    raw,
                };
                let _verified = item.verify_receipt(&source, &selected, &receiver)?;
                batch.push(item);
            }
            saved_ack(ack, &batch)?;
            let second = s.view(owner, 1)?.slot;
            let c2spec = Spec::parse(&repo.spec(second)?)?;
            let c2snap = repo.snapshot(second)?;
            let c2counts = repo.counts(second)?;
            let c2head = Head::parse(
                &c2snap.head,
                state::parse_id(&s.origin.contexts[1])?,
                c2spec.cert_refs()?[owner],
                owner as u64,
            )?;
            ensure!(
                repo.contexts().get(second as usize) == Some(&(c2spec.c, owner as u8))
                    && state::hex(&c2spec.c) == s.origin.contexts[1]
                    && c2spec.role(profile.device.parse()?)? == owner as u64
                    && c2head.d.map(|id| state::hex(&id))
                        == public[owner][1]["d"].as_str().map(str::to_owned)
                    && c2head.e.map(|id| state::hex(&id))
                        == public[owner][1]["e"].as_str().map(str::to_owned)
                    && c2head.sid.as_deref() == public[owner][1]["sid"].as_str(),
                "same original stopped C2 identity/scope"
            );
            ensure!(
                repo.existing_state(second)?.mode == store::ExistingMode::L1
                    && c2snap.meta.next == [33, 33]
                    && c2counts.sources == 66
                    && c2counts.queued == 0
                    && c2counts.pending == 0
                    && c2counts.debt == 0,
                "terminal C2 L1 exact64, not original P7 zero state"
            );
            let mut actual_c2 = BTreeMap::new();
            for role in 0..2u8 {
                for n in 1..=32u16 {
                    let raw = repo
                        .event_at(second, role, n.into())?
                        .context("unchanged actual C2 Text")?;
                    let event =
                        old::Event::parse(&raw, c2spec.peers[role as usize].cert.device_id())?;
                    let request = oracle::Request::new(1, role, n)?;
                    let id = event.id()?;
                    let text = repo.projection(second, id)?.text;
                    ensure!(
                        event.core.c == c2spec.c
                            && event.core.role == role as u64
                            && event.core.n == u64::from(n)
                            && event.core.request
                                == source::parse_request_id(&source::request_id(request))?
                            && text == request.body(),
                        "same actual C2 source/request/body"
                    );
                    actual_c2.insert(state::hex(&id),json!({"format":1,"id":state::hex(&id),"role":role,"ordinal":n,"request":source::request_id(request),"text":text}));
                }
            }
            ensure!(
                actual_c2 == c2[owner],
                "exact immutable C2 source map preserved"
            );
            proofs.push(json!({"owner":owner,"source":state::hex(&repo.source_id()),"immutable_ordinals":[385,640],"verified_sources":self.sources.len(),"verified_receipts":receipt_hashes.len(),"receipt_digest":state::hex(&digest(&serde_json::to_vec(&receipt_hashes)?)),"c2_sources":actual_c2.len(),"debt":counts.debt}));
            drop(repo);
        }
        ensure!(
            source::debt(&s.view(1, 0)?, oracle::Request::new(0, 1, 640)?)? == *held,
            "exact original held B640 ciphertext/debt/no receipt"
        );
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == before,
            "terminal reader has zero Native constructors/packet calls"
        );
        Ok(
            json!({"classification":"owned stopped immutable-source proof","owners":proofs,"b640":held,"native_calls_unchanged":true}),
        )
    }
}

#[cfg(test)]
#[path = "migration_s54_retained_tests.rs"]
mod tests;
