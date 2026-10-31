//! Borrowed, read-only logical witnesses. No owner, Native constructor or commit.
use super::*;
use kilogram_identity::{AccountAuthoritySnapshot, AccountId};

pub struct Digest(blake3::Hasher);
impl Digest {
    pub fn new(domain: &[u8]) -> Self {
        let mut h = blake3::Hasher::new();
        h.update(domain);
        Self(h)
    }
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }
    pub fn value(&mut self, value: &impl serde::Serialize) -> Result<()> {
        self.bytes(&serde_json::to_vec(value)?);
        Ok(())
    }
    pub fn finish(self) -> String {
        self.0.finalize().to_hex().to_string()
    }
}
pub fn check(until: Instant) -> Result<()> {
    ensure!(Instant::now() < until, "cold reader deadline");
    Ok(())
}

pub fn authority(candidate: &[u8], floor: &[u8], account: AccountId) -> Result<()> {
    ensure!(
        candidate.len() <= wire::MAX_AUTH && floor.len() <= wire::MAX_AUTH,
        "authority cap"
    );
    let a = AccountAuthoritySnapshot::decode_and_verify(candidate)?;
    let f = AccountAuthoritySnapshot::decode_and_verify(floor)?;
    ensure!(
        a.encode()? == candidate && f.encode()? == floor,
        "authority exact encoding"
    );
    ensure!(
        a.account_id() == account && f.account_id() == account,
        "authority account"
    );
    ensure!(a.revision() >= f.revision(), "authority rollback");
    ensure!(
        a.revision() != f.revision() || candidate == floor,
        "authority equal revision conflict"
    );
    Ok(())
}

pub struct Scope {
    pub verified: wire::VerifiedExistingScope1,
    pub raw: Vec<u8>,
    pub head: Head,
}
pub fn scope(repo: &mut store::L1Repository, c: u8, local: usize) -> Result<Scope> {
    let spec = repo.spec(c)?;
    let parsed = Spec::parse(&spec)?;
    let head = Head::parse(
        &repo.head(c)?,
        parsed.c,
        parsed.cert_refs()?[local],
        local as u64,
    )?;
    ensure!(head.phase == 3, "original Ready phase");
    let offer = repo.signed_control(c, head.offer.context("original Offer")?)?;
    let grant = repo.signed_control(c, head.grant.context("original Grant")?)?;
    let pair = repo
        .control(c, store::L1Control::InitialPair)?
        .context("original pair")?;
    let mut genesis = Vec::new();
    let mut ready = Vec::new();
    for role in 0..2 {
        genesis.push(old::b(
            repo.event_at(c, role, 0)?.context("original genesis")?,
        ));
        ready.push(old::b(
            repo.control(c, store::L1Control::Ready(role))?
                .context("original Ready")?,
        ));
    }
    let raw = old::encode(
        &old::a([
            old::u(1),
            old::b(spec),
            old::b(offer),
            old::b(grant),
            old::b(pair),
            old::a(genesis),
            old::a(ready),
        ]),
        wire::MAX_EXISTING_ORIGIN,
    )?;
    let verified = wire::VerifiedExistingScope1::verify_export(&raw)?;
    ensure!(
        head.c == verified.c()
            && head.d == Some(verified.d())
            && head.e == Some(verified.e())
            && head.sid.as_deref() == Some(verified.sid()),
        "Head original scope"
    );
    Ok(Scope {
        verified,
        raw,
        head,
    })
}

pub fn b0(
    repo: &mut store::L1Repository,
    profile_raw: &[u8],
    config_raw: &[u8],
    expected: &fixed::Expected,
    proofs: &[Vec<u8>; 2],
    until: Instant,
) -> Result<String> {
    check(until)?;
    let c = 0;
    let local = expected.local;
    ensure!(
        repo.contexts() == [(expected.c, local as u8)],
        "exact original context map"
    );
    ensure!(
        repo.initialized()? && repo.history_initialized(c)?,
        "initialized saved history"
    );
    let scope = scope(repo, c, local)?;
    expected.scope(&scope.verified)?;
    let native = repo.snapshot(c)?;
    let bytes = native.native.context("opaque original Native")?;
    let native_hash = old::hash(state::NATIVE_TAG, &bytes);
    ensure!(
        native.head == repo.head(c)?
            && scope.head.native_generation == expected.native_generation
            && state::hex(&native_hash) == expected.native_hash,
        "original opaque Native witness"
    );
    ensure!(
        scope.head.blob == Some(native_hash),
        "Head opaque Native digest"
    );
    let meta = repo.metadata(c)?;
    ensure!(meta.next == expected.next, "original frontiers");
    ensure!(
        meta.request_index_ready,
        "saved complete local request index"
    );
    let counts = repo.counts(c)?;
    ensure!(
        counts.sources == expected.next.iter().sum::<u64>()
            && counts.receipts == expected.next.map(|n| n - 1)
            && counts.queued == 0
            && counts.pending == 0
            && counts.debt == 0,
        "original source/receipt/queue counts"
    );
    let existing = repo.existing_state(c)?;
    ensure!(
        existing.mode == store::ExistingMode::L1
            && existing.q.is_none()
            && existing.cuts == [None, None]
            && existing.previous == [None, None]
            && existing.propose.is_none()
            && existing.accept.is_none()
            && existing.select.is_none()
            && existing.committed.is_none()
            && existing.selected_consent.is_none()
            && existing.current_consent.is_none()
            && existing.local_intent.is_none()
            && existing.local_outcomes.is_empty()
            && existing.peer_intent.is_none()
            && existing.peer_q_hint.is_none()
            && !existing.control_debt,
        "no Q/cuts/intents/transition"
    );
    let mut digest = Digest::new(b"kg:s53:cold:B0:1\0");
    digest.bytes(profile_raw);
    digest.bytes(config_raw);
    digest.bytes(&scope.raw);
    digest.bytes(&native.head);
    digest.bytes(&bytes);
    digest.value(&native.meta)?;
    digest.value(&meta)?;
    digest.value(&existing)?;
    digest.value(&counts)?;
    digest.value(&repo.contexts())?;
    digest.value(&repo.generation())?;
    digest.bytes(&repo.last_commit()?);
    digest.value(&repo.config_revision())?;
    digest.bytes(&repo.source_id());
    digest.bytes(&repo.migration_id());
    digest.value(&repo.next_author_sequence()?)?;
    digest.value(&(
        repo.initialized()?,
        repo.history_initialized(c)?,
        repo.requires_reopen(),
    ))?;
    for control in [
        store::L1Control::Consent,
        store::L1Control::InitialPair,
        store::L1Control::Ready(0),
        store::L1Control::Ready(1),
        store::L1Control::CurrentAuth,
        store::L1Control::BlockedFork,
        store::L1Control::Locator(false),
        store::L1Control::Locator(true),
    ] {
        digest.value(&repo.control(c, control)?)?;
    }
    for (role, proof) in proofs.iter().enumerate() {
        let peer = &scope.verified.peers()[role];
        let account = peer.certificate().account_id();
        let floor = repo.authority(*account.as_bytes())?;
        authority(proof, &floor, account)?;
        let current = wire::RootVerifiedCredential::from_parts(
            peer.certificate().clone(),
            AccountAuthoritySnapshot::decode_and_verify(proof)?,
        )?;
        ensure!(
            current.cert_ref() == scope.verified.certs()[role],
            "current original certificate"
        );
        digest.bytes(&floor);
        digest.bytes(proof);
    }
    Ok(digest.finish())
}

pub fn credential(
    repo: &mut store::L1Repository,
    reference: old::Id,
) -> Result<(Vec<u8>, wire::RootVerifiedCredential)> {
    let raw = repo
        .auth(0, reference)?
        .context("saved credential reference")?;
    let verified = wire::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
    ensure!(
        verified.reference() == reference,
        "saved credential exact reference"
    );
    Ok((raw, verified))
}
pub fn verified_source(
    scope: &wire::VerifiedExistingScope1,
    raw: &[u8],
    author_raw: &[u8],
    role: usize,
    n: u64,
) -> Result<wire::VerifiedLegacyEvent> {
    let author = wire::ParsedCredentialBundle::parse(author_raw)?.verify_root()?;
    let event = scope.verify_event(raw, &author)?;
    ensure!(
        event.event().core.role == role as u64
            && event.event().core.n == n
            && event.event().core.kind == if n == 0 { role as u64 + 1 } else { 3 },
        "exact original lane/ordinal/kind"
    );
    Ok(event)
}
pub fn verified_receipt(
    scope: &wire::VerifiedExistingScope1,
    source: &wire::VerifiedLegacyEvent,
    raw: &[u8],
    receiver_raw: &[u8],
) -> Result<()> {
    let receiver = wire::ParsedCredentialBundle::parse(receiver_raw)?.verify_root()?;
    wire::ParsedReceipt1::parse(raw)?.verify_pre_q(source, scope, &receiver)?;
    Ok(())
}
pub fn local_request(
    local: usize,
    role: usize,
    actual: Option<&store::L1Request>,
    id: old::Id,
    text: &str,
) -> Result<()> {
    if local == role {
        let request = actual.context("local request required")?;
        ensure!(
            request.event == Some(id)
                && request.queue_sequence.is_none()
                && request.text_hash == old::hash(b"kg:od1:request-text\0", text.as_bytes()),
            "local request source/text"
        );
    }
    Ok(())
}
pub fn absent_request(actual: Option<&store::L1Request>) -> Result<()> {
    ensure!(actual.is_none(), "future local request present");
    Ok(())
}

pub struct Full {
    pub digest: String,
    pub history_digest: String,
    pub text_count: usize,
    pub pages: usize,
}
pub fn full(
    repo: &mut store::L1Repository,
    expected: &fixed::Expected,
    proofs: &[Vec<u8>; 2],
    until: Instant,
) -> Result<Full> {
    let scope = scope(repo, 0, expected.local)?;
    expected.scope(&scope.verified)?;
    let mut digest = Digest::new(b"kg:s53:cold:B1:1\0");
    digest.bytes(&scope.raw);
    let mut history = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for role in 0..2 {
        let mut previous = None;
        for n in 0..expected.next[role] {
            check(until)?;
            let raw = repo
                .event_at(0, role as u8, n)?
                .context("original bounded source missing")?;
            // Decode the signed source with the original role key to discover its
            // exact AuthRecords reference, then verify through the scope again.
            let parsed =
                old::Event::parse(&raw, scope.verified.peers()[role].certificate().device_id())?;
            let (auth, author) = credential(repo, parsed.core.author_auth)?;
            authority(
                &proofs[role],
                &author.authority().encode()?,
                author.certificate().account_id(),
            )?;
            let verified = verified_source(&scope.verified, &raw, &auth, role, n)?;
            let e = verified.event();
            let id = e.id()?;
            ensure!(
                ids.insert(id) && e.core.previous == previous,
                "source uniqueness/previous chain"
            );
            previous = Some(id);
            digest.bytes(&raw);
            digest.bytes(&auth);
            if n == 0 {
                continue;
            }
            let (request, text) = expected.text(role, n);
            ensure!(
                e.core.request == request,
                "original source request convention"
            );
            ensure!(
                !fixed::future_ids().contains(&e.core.request),
                "future source present"
            );
            let projection = repo.projection(0, id)?;
            ensure!(projection.text == text, "original saved body convention");
            digest.bytes(&projection.encode()?);
            // Imported snapshot and full-L1 proof pointers remain distinguished;
            // history() independently validates their paid authenticated bindings.
            let proof = repo.full_l1_proof(0, id)?;
            digest.value(&proof)?;
            if let Some(ref proof) = proof {
                for reference in std::iter::once(Some(proof.original))
                    .chain([
                        proof.admission,
                        proof.final_auth,
                        proof.receiver,
                        proof.receiver_overlay,
                    ])
                    .flatten()
                {
                    let (bytes, cred) = credential(repo, reference)?;
                    let r = scope
                        .verified
                        .certs()
                        .iter()
                        .position(|c| *c == cred.cert_ref())
                        .context("proof credential original peer")?;
                    authority(
                        &proofs[r],
                        &cred.authority().encode()?,
                        cred.certificate().account_id(),
                    )?;
                    digest.bytes(&bytes);
                }
            }
            let receipt = repo
                .receipt(0, role as u8, n)?
                .context("original Receipt1")?;
            if role == 1 && n == 17 {
                ensure!(
                    blake3::hash(&receipt).to_hex().as_str()
                        == "338a74bcbee1249cb1655fa21962833d6d680394769861ca2df4ab98475a4760",
                    "original B17 receipt byte pin"
                );
            }
            let decoded = old::decode(&receipt, 1024)?;
            let reference = decoded.fields(2)?[0].fields(8)?[7].fixed()?;
            let (receiver, cred) = credential(repo, reference)?;
            let opposite = 1 - role;
            ensure!(
                cred.cert_ref() == scope.verified.certs()[opposite],
                "opposite Receipt1 signer"
            );
            authority(
                &proofs[opposite],
                &cred.authority().encode()?,
                cred.certificate().account_id(),
            )?;
            verified_receipt(&scope.verified, &verified, &receipt, &receiver)?;
            digest.bytes(&receipt);
            digest.bytes(&receiver);
            // Only the local author's request index is a required stored witness.
            if role == expected.local {
                let local = repo.request(0, request)?;
                local_request(expected.local, role, local.as_ref(), id, &text)?;
                digest.value(&local)?;
            }
            history.insert(state::hex(&id),json!({"format":1,"id":state::hex(&id),"role":role,"ordinal":n,"sequence":e.core.sequence,"request":state::hex(&request),"text":text,"proof_kind":if proof.is_some(){"FullL1"}else{"LegacySnapshot"},"delivered":true}));
        }
        ensure!(
            repo.event_at(0, role as u8, expected.next[role])?.is_none(),
            "future lane source present"
        );
    }
    let future = fixed::future_ids()[expected.local];
    absent_request(repo.request(0, future)?.as_ref())?;
    let pages = super::history::read(repo, &history, expected.next, until, &mut digest)?;
    let mut hd = Digest::new(b"kg:s53:cold:history:1\0");
    hd.value(&history)?;
    Ok(Full {
        digest: digest.finish(),
        history_digest: hd.finish(),
        text_count: history.len(),
        pages,
    })
}
