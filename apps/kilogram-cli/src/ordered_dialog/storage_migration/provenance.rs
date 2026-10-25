//! Production source checks. No caller-supplied provenance bit or mutable Engine.
use super::{
    super::{
        admin, engine,
        handshake::{self, Credentials, Descriptor, Spec},
        state::{self, Head, PrivateState},
    },
    fold,
};
use anyhow::{Context, Result, ensure};
use kilogram_crypto::SealedMessage;
use kilogram_identity::{AccountAuthoritySnapshot, AccountId, DeviceId, DeviceState};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Id, a, b, u};
use kilogram_ratchet::ordered_v2::ImportedL1;
use kilogram_state::indexed_dialog::migration7::LegacyRead;
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

pub(super) struct ContextOrigin {
    pub spec: Spec,
    pub head: Head,
    pub readable: [Vec<fold::ReadableMeta>; 2],
    pub imported: Option<ImportedL1>,
    pub pending: Vec<PendingOrigin>,
    pub queue: usize,
    pub receipt_refs: BTreeMap<Id, Id>,
    pub delivered: BTreeSet<Id>,
}
pub(super) struct PendingOrigin {
    pub event: Id,
    pub ordinal: u64,
    pub original: Id,
    pub admission: Id,
    pub head: bool,
    pub quarantined: bool,
}
pub(super) struct VerifiedSource {
    pub nonce: Id,
    pub device: DeviceState,
    pub enrollment: Credentials,
    pub contexts: BTreeMap<Id, ContextOrigin>,
}
fn required(source: &dyn LegacyRead, path: &str) -> Result<Zeroizing<Vec<u8>>> {
    Ok(source.required(path)?)
}
fn credential(source: &dyn LegacyRead, id: Id, spec: &Spec, role: u64) -> Result<Credentials> {
    let cred = Credentials::parse(&required(source, &admin::auth_path(id))?)?;
    ensure!(
        cred.reference() == id && cred.cert_ref()? == spec.peers[role as usize].cert_ref()?,
        "historical credential role/reference"
    );
    Ok(cred)
}
fn control(source: &dyn LegacyRead, c: Id, id: Id) -> Result<Zeroizing<Vec<u8>>> {
    let raw = required(source, &engine::control_path(c, id))?;
    ensure!(
        wire::hash(wire::CONTROL_ID, &raw) == id,
        "immutable control ID"
    );
    Ok(raw)
}
fn peer_authority(raw: &[u8], path: &str, peers: &BTreeSet<Id>) -> Result<()> {
    let snapshot = AccountAuthoritySnapshot::decode_and_verify(raw)?;
    ensure!(
        snapshot.encode()? == raw
            && path == format!("peer-authority/{}.snapshot", snapshot.account_id())
            && peers.contains(snapshot.account_id().as_bytes()),
        "archived peer authority account/path/registered peer"
    );
    // This is a signed historical witness. Its revocations must not be treated
    // as current permission, nor erase an otherwise readable source history.
    Ok(())
}
fn archived_context(source: &dyn LegacyRead, spec: &Spec, head: &Head) -> Result<()> {
    for entry in source
        .entries()
        .iter()
        .filter(|e| e.context == Some(spec.c) && matches!(e.kind, 21 | 23 | 26 | 27))
    {
        let raw = required(source, &entry.path)?;
        match entry.kind {
            21 => {
                let id = state::parse_id(entry.path.rsplit('/').next().context("control ID")?)?;
                ensure!(
                    head.offer == Some(id) || head.grant == Some(id),
                    "unexplained archived control"
                );
                // selected_native verifies both selected signatures, scope and
                // transcript/epoch. No unselected opaque control may be added.
                control(source, spec.c, id)?;
            }
            23 => {
                let role = if entry.path == format!("{}local-locator", engine::prefix(spec.c)) {
                    head.role
                } else {
                    ensure!(
                        entry.path == format!("{}peer-locator", engine::prefix(spec.c)),
                        "archived locator path"
                    );
                    1 - head.role
                };
                Descriptor::parse(spec, role, &raw)?;
                // Intentionally no valid_now: expiry only affects a live endpoint.
            }
            26 => {
                handshake::verify_initial_pair(spec, &raw, None)?;
            }
            27 => {
                let value = wire::decode(&raw, 68 * 1024)?;
                let f = value.fields(4)?;
                let reason = std::str::from_utf8(f[1].bytes()?)?;
                ensure!(
                    f[0].uint()? == 1 && reason.len() <= 128,
                    "fork witness shape"
                );
                for (position, proof) in [(2, f[2].bytes()?), (3, f[3].bytes()?)] {
                    ensure!(proof.len() <= 32 * 1024, "fork witness proof cap");
                    match reason {
                        "Offer" => {
                            let body = wire::control(proof, 1, spec.peers[0].cert.device_id())?;
                            let fields = body.fields(8)?;
                            let refs = spec.cert_refs()?;
                            ensure!(
                                fields[0].fixed::<32>()? == spec.c
                                    && fields[1].fixed::<32>()? == refs[0]
                                    && fields[2].fixed::<32>()? == refs[1]
                                    && fields[6].uint()? == 1
                                    && fields[7].uint()? == 1,
                                "fork control scope"
                            );
                        }
                        "Grant" => {
                            let offer = control(source, spec.c, head.offer.context("fork Offer")?)?;
                            handshake::epoch(spec, &offer, proof)?;
                        }
                        "Ready" | "Ready genesis" => {
                            let role = 1 - head.role;
                            let signer = spec.peers[role as usize].cert.device_id();
                            // If no Ready was saved yet, the old side of a
                            // Ready-genesis witness is the actual peer genesis.
                            let body = match wire::control(proof, 3, signer) {
                                Ok(body) => body,
                                Err(_) if reason == "Ready genesis" && position == 2 => {
                                    let event = Event::parse(proof, signer)?;
                                    ensure!(
                                        event.core.c == spec.c
                                            && event.core.role == role
                                            && event.core.n == 0
                                            && event.core.kind == role + 1,
                                        "fork Ready original genesis"
                                    );
                                    continue;
                                }
                                Err(error) => return Err(error.into()),
                            };
                            let fields = body.fields(8)?;
                            ensure!(
                                Some(fields[0].fixed::<32>()?) == head.d
                                    && Some(fields[1].fixed::<32>()?) == head.e
                                    && fields[2].text()?
                                        == head.sid.as_deref().context("fork SID")?
                                    && fields[5].uint()? == role
                                    && fields[6].uint()? == 1
                                    && fields[7].uint()? == 1,
                                "fork Ready scope"
                            );
                        }
                        "Init" | "Confirm" => {
                            let role = u64::from(reason == "Confirm");
                            let event =
                                Event::parse(proof, spec.peers[role as usize].cert.device_id())?;
                            let core = &event.core;
                            let selected = if role == 0 { head.offer } else { head.grant }
                                .context("fork genesis selected control")?;
                            ensure!(
                                head.phase == 3
                                    && head.role == 1 - role
                                    && core.c == spec.c
                                    && Some(core.d) == head.d
                                    && Some(core.e) == head.e
                                    && (role == 0 || head.sid.as_ref() == Some(&core.sid))
                                    && core.kind == role + 1
                                    && core.n == 0
                                    && core.request.as_slice() == &selected[..16]
                                    && core.parents.is_empty(),
                                "fork genesis capture scope"
                            );
                            let saved: Id =
                                required(source, &engine::ordinal_path(spec.c, role, 0))?
                                    .as_slice()
                                    .try_into()?;
                            if position == 2 {
                                ensure!(
                                    core.role == role
                                        && event.id()? == saved
                                        && required(source, &engine::event_path(spec.c, saved))?
                                            .as_slice()
                                            == proof,
                                    "fork original saved genesis"
                                );
                            } else {
                                // The old writer durably captures a distinct
                                // signed conflict before its Guard checks role.
                                // It is evidence, never a readable Event origin.
                                ensure!(event.id()? != saved, "fork distinct genesis conflict");
                            }
                        }
                        "ordinal" | "pending ordinal" | "author sequence" => {
                            let role = 1 - head.role;
                            let event =
                                Event::parse(proof, spec.peers[role as usize].cert.device_id())?;
                            ensure!(
                                (reason == "author sequence" && position == 2)
                                    || (event.core.c == spec.c && event.core.role == role),
                                "fork Event scope"
                            );
                        }
                        _ => anyhow::bail!("unknown archived fork witness"),
                    }
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}
fn selected_native(
    source: &dyn LegacyRead,
    device: &DeviceState,
    nonce: Id,
    spec: &Spec,
    head: &Head,
) -> Result<Option<PrivateState>> {
    if head.phase == 0 {
        ensure!(
            source.get(&state::native_path(spec.c))?.is_none(),
            "absent phase cannot have Native"
        );
        ensure!(
            source
                .get(&format!("{}initial-pair", engine::prefix(spec.c)))?
                .is_none(),
            "orphan initial selection"
        );
        return Ok(None);
    }
    let private = PrivateState::open(
        head,
        nonce,
        device.encryption(),
        &required(source, &state::native_path(spec.c))?,
    )?;
    let offer = control(source, spec.c, head.offer.context("Offer")?)?;
    let offer_body = wire::control(&offer, 1, spec.peers[0].cert.device_id())?;
    let oa = offer_body.fields(8)?;
    let pair = required(source, &format!("{}initial-pair", engine::prefix(spec.c)))?;
    let transcript = handshake::verify_initial_pair(spec, &pair, None)?;
    let refs = spec.cert_refs()?;
    ensure!(
        oa[0].fixed::<32>()? == spec.c
            && oa[1].fixed::<32>()? == refs[0]
            && oa[2].fixed::<32>()? == refs[1]
            && oa[3].fixed::<32>()? == transcript
            && oa[6].uint()? == 1
            && oa[7].uint()? == 1,
        "selected Offer scope/transcript"
    );
    oa[4].fixed::<32>()?;
    let own_curve = if head.role == 0 {
        oa[5].fixed::<32>()?
    } else {
        let grant = control(source, spec.c, head.grant.context("Grant")?)?;
        let gb = wire::control(&grant, 2, spec.peers[1].cert.device_id())?;
        gb.fields(8)?[4].fixed::<32>()?
    };
    ensure!(
        private.account.curve_key() == own_curve,
        "actual Native Account/control curve"
    );
    if let Some(grant_id) = head.grant {
        let grant = control(source, spec.c, grant_id)?;
        let (d, e) = handshake::epoch(spec, &offer, &grant)?;
        ensure!(
            head.d == Some(d) && head.e == Some(e),
            "actual selected D/E"
        );
        let gb = wire::control(&grant, 2, spec.peers[1].cert.device_id())?;
        let otk = gb.fields(8)?[5].fixed::<32>()?;
        if let Some(session) = private.session.as_ref() {
            ensure!(
                session.keys()[0] == oa[5].fixed::<32>()?
                    && session.keys()[2] == otk
                    && Some(session.sid()) == head.sid,
                "actual Native SessionKeys/SID"
            );
        }
        if let Some(reservation) = private.reservation {
            ensure!(reservation.2 == otk, "actual reserved OTK");
        }
    }
    Ok(Some(private))
}
fn event(
    source: &dyn LegacyRead,
    spec: &Spec,
    head: &Head,
    role: u64,
    n: u64,
    id: Id,
) -> Result<Event> {
    let raw = required(source, &engine::event_path(spec.c, id))?;
    let event = Event::parse(&raw, spec.peers[role as usize].cert.device_id())?;
    let c = &event.core;
    ensure!(
        event.id()? == id
            && c.c == spec.c
            && c.role == role
            && c.n == n
            && c.n < 512
            && Some(c.d) == head.d
            && Some(c.e) == head.e
            && Some(c.sid.as_str()) == head.sid.as_deref(),
        "exact immutable Event1 source"
    );
    credential(source, c.author_auth, spec, role)?;
    let packet = kilogram_ratchet::ordered::Packet {
        kind: event.native_kind.try_into()?,
        bytes: event.ciphertext.clone(),
    }
    .header()?;
    ensure!(
        packet.key == event.key
            && packet.index == event.index
            && event.native_kind == if role == 0 && n == 0 { 0 } else { 1 },
        "actual signed Native header/kind"
    );
    let authored = format!(
        "runtime/od1/authored/{}/{:020}",
        spec.peers[role as usize].cert.device_id(),
        c.sequence
    );
    ensure!(
        required(source, &authored)?.as_slice() == id,
        "global signer/sequence binding"
    );
    if n == 0 {
        let expected =
            if role == 0 { head.offer } else { head.grant }.context("genesis selected control")?;
        ensure!(
            c.kind == role + 1 && c.request == expected[..16] && c.parents.is_empty(),
            "canonical legacy genesis"
        );
    } else {
        ensure!(c.kind == 3, "legacy Text kind");
    }
    if role == 0 && n == 0 {
        let native = kilogram_ratchet::ordered::Packet {
            kind: event.native_kind.try_into()?,
            bytes: event.ciphertext.clone(),
        }
        .header()?;
        ensure!(
            native.prekey_sid.as_deref() == head.sid.as_deref(),
            "Init prekey SID"
        );
    }
    Ok(event)
}
pub(super) fn projection(
    source: &dyn LegacyRead,
    device: &DeviceState,
    head: &Head,
    id: Id,
) -> Result<(String, [u8; 16])> {
    let raw = required(source, &engine::projection_path(head.c, id))?;
    projection_raw(device, head, id, &raw)
}
pub(super) fn projection_raw(
    device: &DeviceState,
    head: &Head,
    id: Id,
    raw: &[u8],
) -> Result<(String, [u8; 16])> {
    let d = head.d.context("projection D")?;
    let e = head.e.context("projection E")?;
    let envelope = wire::decode(raw, 8192)?;
    let f = envelope.fields(3)?;
    ensure!(f[0].uint()? == 1, "legacy projection version");
    let plain = Zeroizing::new(device.encryption().open(
        &SealedMessage {
            encapsulated_key: f[1].fixed::<32>()?.to_vec(),
            ciphertext: f[2].bytes()?.to_vec(),
        },
        b"kg:od1:local-projection\0",
        &wire::encode(&a([u(1), b(head.c), b(d), b(e), b(id)]), 1024)?,
    )?);
    let value = Zeroizing::new(wire::decode(&plain, 8192)?);
    let f = value.fields(7)?;
    ensure!(
        f[0].uint()? == 1
            && f[1].fixed::<32>()? == head.c
            && f[2].fixed::<32>()? == d
            && f[3].fixed::<32>()? == e
            && f[4].fixed::<32>()? == id,
        "projection exact binding"
    );
    let text = f[5].text()?.to_owned();
    ensure!(text.len() <= 4096, "projection UTF8 cap");
    Ok((text, f[6].fixed()?))
}
fn receipt(
    source: &dyn LegacyRead,
    spec: &Spec,
    head: &Head,
    m: &fold::ReadableMeta,
) -> Result<Option<Id>> {
    let Some(raw) = source.get(&super::super::outbox::receipt_path(
        spec.c,
        u64::from(m.event.role),
        m.event.n,
    ))?
    else {
        return Ok(None);
    };
    ensure!(m.event.kind == 3, "genesis has no message receipt");
    let v = wire::decode(&raw, 1024)?;
    let r = v.fields(2)?;
    let f = r[0].fields(8)?;
    let receiver = 1 - u64::from(m.event.role);
    let auth = f[7].fixed::<32>()?;
    let cred = credential(source, auth, spec, receiver)?;
    let expected = a([
        u(1),
        b(head.d.context("receipt D")?),
        b(head.e.context("receipt E")?),
        u(m.event.role.into()),
        u(m.event.n),
        b(m.id),
        b(cred.cert_ref()?),
        b(auth),
    ]);
    ensure!(r[0] == expected, "exact Receipt1 source/receiver");
    cred.cert.device_id().verify(
        &wire::signing_bytes(wire::RECEIPT_SIGN, &r[0], 1024)?,
        r[1].bytes()?,
    )?;
    Ok(Some(auth))
}
impl VerifiedSource {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!(self.contexts.values().map(|c|serde_json::json!({"context":super::super::state::hex(&c.spec.c),"role":c.head.role,"native_phase":c.head.phase,
            "readable":[c.readable[0].len(),c.readable[1].len()],"queue":c.queue,"receipts":c.receipt_refs.len(),
            "pending":c.pending.iter().map(|p|serde_json::json!({"event_id":super::super::state::hex(&p.event),"ordinal":p.ordinal,"original":super::super::state::hex(&p.original),"admission":super::super::state::hex(&p.admission),"head":p.head,"quarantined":p.quarantined})).collect::<Vec<_>>()})).collect::<Vec<_>>())
    }
    pub fn facts(
        &self,
        source: &dyn LegacyRead,
    ) -> Result<kilogram_state::indexed_dialog::migration7::MigrationOriginFacts> {
        use kilogram_state::indexed_dialog::migration7::{
            MigrationContextFacts, MigrationOriginFacts,
        };
        let mut contexts = Vec::new();
        for origin in self.contexts.values() {
            let raw = source.required(&state::head_path(origin.spec.c))?;
            let seen = origin
                .imported
                .as_ref()
                .map(|i| {
                    i.seen
                        .iter()
                        .map(|(role, key, value)| (*role, *key, value.generation, value.first))
                        .collect()
                })
                .unwrap_or_default();
            contexts.push(MigrationContextFacts {
                context: origin.spec.c,
                role: origin.head.role.try_into()?,
                devices: [
                    *origin.spec.peers[0].cert.device_id().as_bytes(),
                    *origin.spec.peers[1].cert.device_id().as_bytes(),
                ],
                certificates: origin.spec.cert_refs()?,
                head_hash: *blake3::hash(&raw).as_bytes(),
                native_hash: origin.head.blob,
                next: [
                    origin.readable[0].len() as u64,
                    origin.readable[1].len() as u64,
                ],
                seen,
                ordinary_pending: origin.pending.iter().filter(|p| !p.head).count() as u64,
                head_pending: origin.pending.iter().find(|p| p.head).map(|p| p.ordinal),
            });
        }
        Ok(MigrationOriginFacts {
            source_id: source.source_id(),
            account: *self.enrollment.cert.account_id().as_bytes(),
            device: *self.device.identity().device_id().as_bytes(),
            claim: self.nonce,
            contexts,
        })
    }
    pub fn verify(
        source: &dyn LegacyRead,
        account: AccountId,
        expected_device: DeviceId,
    ) -> Result<Self> {
        Self::verify_scope(source, account, expected_device, None)
    }
    pub fn verify_history(
        source: &dyn LegacyRead,
        account: AccountId,
        device: DeviceId,
        context: Id,
    ) -> Result<Self> {
        Self::verify_scope(source, account, device, Some(context))
    }
    fn verify_scope(
        source: &dyn LegacyRead,
        account: AccountId,
        expected_device: DeviceId,
        selected: Option<Id>,
    ) -> Result<Self> {
        let native_before = kilogram_ratchet::ordered::native_call_counts();
        let raw = required(source, admin::CLAIM)?;
        let claim = wire::decode(&raw, 1024)?;
        let cf = claim.fields(5)?;
        ensure!(
            cf[0].uint()? == 1
                && cf[1].uint()? == 4
                && cf[3].fixed::<32>()? == *account.as_bytes()
                && cf[4].fixed::<32>()? == *expected_device.as_bytes(),
            "expected existing owner/claim"
        );
        let nonce = cf[2].fixed::<32>()?;
        ensure!(nonce != [0; 32], "missing original claim nonce");
        let signing = required(source, "device-secret.key")?;
        let encryption = required(source, "device-encryption-secret.key")?;
        let device = DeviceState::from_secret_material(
            source.root(),
            signing.as_slice().try_into()?,
            encryption.as_slice().try_into()?,
        );
        let cert = required(source, "device-certificate.cert")?;
        let authority = required(source, "account-authority.snapshot")?;
        let enrollment = Credentials::decode(&wire::encode(
            &a([u(1), b(&cert[..]), b(&authority[..])]),
            wire::MAX_AUTH,
        )?)?;
        ensure!(
            device.identity().device_id() == expected_device
                && enrollment.cert.device_id() == expected_device
                && enrollment.cert.account_id() == account
                && enrollment.cert.encryption_public_key() == device.encryption().public_key(),
            "saved Device secrets/certificate"
        );
        ensure!(
            required(source, "ratchet/account.pickle")?.as_slice()
                == kilogram_state::ORDERED_ACCOUNT_FENCE
                && required(source, "ratchet/pickle-secret.key")?.as_slice() == [0; 32],
            "exact archived ratchet fences"
        );
        let mut contexts = BTreeMap::new();
        let mut peers = BTreeSet::new();
        let mut certificates = BTreeSet::from([enrollment.cert_ref()?]);
        let mut authored = BTreeMap::<String, Id>::new();
        let mut own_sequences = BTreeSet::new();
        for entry in source
            .entries()
            .iter()
            .filter(|e| e.kind == 8 && selected.is_none_or(|c| e.context == Some(c)))
        {
            let spec = Spec::parse(&required(source, &entry.path)?)?;
            ensure!(Some(spec.c) == entry.context, "spec source context");
            let role = spec.role(expected_device)?;
            ensure!(
                spec.peers[role as usize].cert_ref()? == enrollment.cert_ref()?,
                "context own certificate"
            );
            spec.verify_consent(&required(source, &admin::consent_path(spec.c))?)?;
            let head = Head::parse(
                &required(source, &state::head_path(spec.c))?,
                spec.c,
                spec.peers[role as usize].cert_ref()?,
                role,
            )?;
            let private = selected_native(source, &device, nonce, &spec, &head)?;
            archived_context(source, &spec, &head)?;
            peers.insert(*spec.peers[(1 - role) as usize].cert.account_id().as_bytes());
            certificates.extend(spec.cert_refs()?);
            let mut readable = [Vec::new(), Vec::new()];
            let mut delivered = BTreeSet::new();
            let mut receipt_refs = BTreeMap::new();
            if let Some(p) = private.as_ref().filter(|p| p.guard.is_some()) {
                let guard = p.guard.as_ref().context("Guard1")?;
                for r in 0..2u64 {
                    let lane = if r == role { &guard.own } else { &guard.peer };
                    for n in 0..lane.next {
                        let id: Id = required(source, &engine::ordinal_path(spec.c, r, n))?
                            .as_slice()
                            .try_into()?;
                        let event = event(source, &spec, &head, r, n, id)?;
                        let key = format!(
                            "runtime/od1/authored/{}/{:020}",
                            spec.peers[r as usize].cert.device_id(),
                            event.core.sequence
                        );
                        ensure!(
                            authored.insert(key, id).is_none(),
                            "global authored sequence bijection"
                        );
                        if r == role {
                            ensure!(
                                own_sequences.insert(event.core.sequence),
                                "unique own sequence"
                            );
                        }
                        if r == 0 && n == 0 {
                            let packet = kilogram_ratchet::ordered::Packet {
                                kind: event.native_kind.try_into()?,
                                bytes: event.ciphertext.clone(),
                            }
                            .header()?;
                            ensure!(
                                packet.prekey_keys
                                    == Some(p.session.as_ref().context("Native session")?.keys()),
                                "Native/Init exact public SessionKeys"
                            );
                        }
                        let meta = fold::ReadableMeta::of(&event)?;
                        if event.core.kind == 3 {
                            let (_, request) = projection(source, &device, &head, id)?;
                            ensure!(request == event.core.request, "projection request binding");
                            let got = receipt(source, &spec, &head, &meta)?;
                            if r == role {
                                let raw = required(source, &engine::outbox_path(spec.c, n))?;
                                let value = wire::decode(&raw, 1024)?;
                                let f = value.fields(3)?;
                                ensure!(
                                    f[0].uint()? == 1
                                        && f[1].fixed::<32>()? == id
                                        && f[2].uint()? <= 1,
                                    "outbox exact source"
                                );
                                if f[2].uint()? == 1 {
                                    ensure!(
                                        got.is_some(),
                                        "Delivered needs actual opposite receipt"
                                    );
                                    delivered.insert(id);
                                } else {
                                    ensure!(got.is_none(), "undelivered cannot hide saved receipt");
                                }
                                ensure!(
                                    required(
                                        source,
                                        &super::super::outbox::request_path(
                                            spec.c,
                                            event.core.request
                                        )
                                    )?
                                    .as_slice()
                                        == id,
                                    "immutable request mapping"
                                );
                            } else {
                                ensure!(got.is_some(), "readable peer Text requires saved receipt");
                                let raw = required(
                                    source,
                                    &super::super::ingress::ack_intent(spec.c, r, n),
                                )?;
                                let ack = wire::decode(&raw, 1024)?;
                                let f = ack.fields(3)?;
                                let receipt = required(
                                    source,
                                    &super::super::outbox::receipt_path(spec.c, r, n),
                                )?;
                                ensure!(
                                    f[0].uint()? == 1
                                        && f[1].fixed::<32>()?
                                            == wire::hash(wire::RECEIPT_ID, &receipt)
                                        && f[2].uint()? <= 2,
                                    "ack intent exact actual receipt"
                                );
                            }
                            if let Some(auth) = got {
                                receipt_refs.insert(id, auth);
                            }
                        }
                        readable[r as usize].push(meta);
                    }
                }
            }
            let imported = private
                .as_ref()
                .and_then(|p| p.guard.as_ref())
                .map(|g| fold::fold(g, &readable))
                .transpose()?;
            let mut pending = Vec::new();
            let mut ordinary = 0;
            let mut has_head = false;
            for item in source
                .entries()
                .iter()
                .filter(|e| e.context == Some(spec.c) && e.kind == 14)
            {
                let n: u64 = item
                    .path
                    .rsplit('/')
                    .next()
                    .context("pending ordinal")?
                    .parse()?;
                let raw = required(source, &item.path)?;
                let event = Event::parse(&raw, spec.peers[(1 - role) as usize].cert.device_id())?;
                let id = event.id()?;
                let core = &event.core;
                ensure!(
                    core.c == spec.c
                        && core.role == 1 - role
                        && core.n == n
                        && core.n < 512
                        && core.kind == 3
                        && Some(core.d) == head.d
                        && Some(core.e) == head.e
                        && Some(core.sid.as_str()) == head.sid.as_deref(),
                    "pending exact source scope"
                );
                ensure!(
                    n >= readable[(1 - role) as usize].len() as u64,
                    "pending/readable overlap"
                );
                let key = format!(
                    "runtime/od1/authored/{}/{:020}",
                    spec.peers[(1 - role) as usize].cert.device_id(),
                    core.sequence
                );
                ensure!(
                    required(source, &key)?.as_slice() == id && authored.insert(key, id).is_none(),
                    "pending global authored sequence"
                );
                let state = wire::decode(
                    &required(source, &super::super::ingress::pending_state(spec.c, n))?,
                    2048,
                )?;
                let f = state.fields(8)?;
                ensure!(
                    f[0].uint()? == 1
                        && f[1].fixed::<32>()? == id
                        && f[2].uint()? <= 1
                        && f[3].uint()? <= 1
                        && f[7].uint()? <= 64 * 1024,
                    "pending admission shape"
                );
                let original = f[4].fixed::<32>()?;
                let admission = f[5].fixed::<32>()?;
                ensure!(core.author_auth == original, "pending exact O capture");
                credential(source, original, &spec, 1 - role)?;
                credential(source, admission, &spec, 1 - role)?;
                let charge = raw
                    .len()
                    .checked_add(required(source, &admin::auth_path(original))?.len())
                    .and_then(|n| n.checked_add(1024))
                    .context("pending original charge")?;
                let charge = charge
                    .checked_add(required(source, &admin::auth_path(admission))?.len())
                    .context("pending admission charge")?;
                ensure!(
                    f[7].uint()? == charge as u64
                        && (f[3].uint()? == 0) == f[6].bytes()?.is_empty(),
                    "actual pending charge/quarantine capture"
                );
                let head_slot = f[2].uint()? == 1;
                if head_slot {
                    ensure!(!has_head, "one reserved head");
                    has_head = true;
                } else {
                    ordinary += 1;
                }
                pending.push(PendingOrigin {
                    event: id,
                    ordinal: n,
                    original,
                    admission,
                    head: head_slot,
                    quarantined: f[3].uint()? == 1,
                });
            }
            ensure!(ordinary <= 64, "ordinary pending limit");
            let queue = source
                .entries()
                .iter()
                .filter(|e| e.context == Some(spec.c) && e.kind == 13)
                .count();
            ensure!(
                queue <= 128 && readable[role as usize].len().max(1) + queue <= 512,
                "legacy queue lifetime reservation"
            );
            let ids = readable
                .iter()
                .flatten()
                .map(|m| m.id)
                .collect::<BTreeSet<_>>();
            let texts = readable
                .iter()
                .flatten()
                .filter(|m| m.event.kind == 3)
                .count();
            for (kind, count) in [
                (11, ids.len()),
                (12, texts),
                (16, ids.len()),
                (15, pending.len()),
                (17, receipt_refs.len()),
                (18, readable[(1 - role) as usize].len().saturating_sub(1)),
                (19, readable[role as usize].len().saturating_sub(1)),
                (20, readable[role as usize].len().saturating_sub(1)),
            ] {
                ensure!(
                    source
                        .entries()
                        .iter()
                        .filter(|e| e.context == Some(spec.c) && e.kind == kind)
                        .count()
                        == count,
                    "source cross-index cardinality"
                );
            }
            for r in 0..2u64 {
                if let Some(raw) = source.get(&engine::ready_path(spec.c, r))? {
                    ensure!(
                        head.phase == 3 && !readable[0].is_empty() && !readable[1].is_empty(),
                        "Ready requires both actual genesis"
                    );
                    let body = wire::control(&raw, 3, spec.peers[r as usize].cert.device_id())?;
                    ensure!(
                        body == a([
                            b(head.d.context("Ready D")?),
                            b(head.e.context("Ready E")?),
                            b(head.sid.as_ref().context("Ready SID")?.as_bytes()),
                            b(readable[0][0].id),
                            b(readable[1][0].id),
                            u(r),
                            u(1),
                            u(1)
                        ]),
                        "exact Ready1 binding"
                    );
                }
            }
            let mut queued_requests = BTreeSet::new();
            for item in source
                .entries()
                .iter()
                .filter(|e| e.context == Some(spec.c) && e.kind == 13)
            {
                let name = item.path.rsplit('-').next().context("queue request")?;
                ensure!(name.len() == 32, "queue request width");
                let mut request = [0; 16];
                for (i, x) in request.iter_mut().enumerate() {
                    *x = u8::from_str_radix(&name[2 * i..2 * i + 2], 16)?;
                }
                ensure!(
                    queued_requests.insert(request)
                        && source
                            .get(&super::super::outbox::request_path(spec.c, request))?
                            .is_none(),
                    "queue/request uniqueness"
                );
                let raw = required(source, &item.path)?;
                let envelope = wire::decode(&raw, 8192)?;
                let f = envelope.fields(3)?;
                ensure!(f[0].uint()? == 1, "queue envelope");
                let plain = Zeroizing::new(device.encryption().open(
                    &SealedMessage {
                        encapsulated_key: f[1].fixed::<32>()?.to_vec(),
                        ciphertext: f[2].bytes()?.to_vec(),
                    },
                    b"kg:od1:local-projection\0",
                    &wire::encode(&a([u(1), b(spec.c), b(request), u(0)]), 1024)?,
                )?);
                let value = Zeroizing::new(wire::decode(&plain, 8192)?);
                let f = value.fields(4)?;
                ensure!(
                    f[0].uint()? == 1
                        && f[1].fixed::<32>()? == spec.c
                        && f[2].fixed::<16>()? == request
                        && f[3].text()?.len() <= 4096,
                    "queue exact C/request/UTF8"
                );
            }
            ensure!(
                contexts
                    .insert(
                        spec.c,
                        ContextOrigin {
                            spec,
                            head,
                            readable,
                            imported,
                            pending,
                            queue,
                            receipt_refs,
                            delivered
                        }
                    )
                    .is_none(),
                "duplicate source context"
            );
        }
        if selected.is_none() {
            ensure!(
                contexts.len() <= 2
                    && source
                        .entries()
                        .iter()
                        .all(|e| e.context.is_none_or(|c| contexts.contains_key(&c))),
                "orphan source context"
            );
            ensure!(
                source.entries().iter().filter(|e| e.kind == 28).count() == authored.len(),
                "no orphan authored entries"
            );
            for entry in source.entries().iter().filter(|e| e.kind == 28) {
                ensure!(
                    authored.get(&entry.path).is_some_and(|id| source
                        .get(&entry.path)
                        .is_ok_and(|v| v.is_some_and(|raw| raw.as_slice() == id))),
                    "exact global author membership"
                );
            }
            let next = source
                .get("next-sequence")?
                .map(|v| {
                    std::str::from_utf8(&v)
                        .context("sequence UTF8")?
                        .trim()
                        .parse::<u64>()
                        .context("sequence uint")
                })
                .transpose()?
                .unwrap_or(0);
            ensure!(
                next == own_sequences.len() as u64
                    && own_sequences
                        .iter()
                        .enumerate()
                        .all(|(n, value)| n as u64 == *value),
                "exact global own sequence prefix"
            );
            for entry in source.entries().iter().filter(|e| e.kind == 7) {
                let cred = Credentials::decode(&required(source, &entry.path)?)?;
                ensure!(
                    entry.path == admin::auth_path(cred.reference())
                        && certificates.contains(&cred.cert_ref()?),
                    "source auth catalog key/registered certificate"
                );
            }
            for entry in source.entries().iter().filter(|e| e.kind == 31) {
                peer_authority(&required(source, &entry.path)?, &entry.path, &peers)?;
            }
        } else {
            ensure!(
                selected.is_some_and(|id| contexts.contains_key(&id)),
                "HistoryUnavailable: unknown context"
            );
            // A C-scoped history read does not fetch another context's authority.
            for peer in &peers {
                let path = format!("peer-authority/{}.snapshot", AccountId::from_bytes(*peer));
                if let Some(raw) = source.get(&path)? {
                    peer_authority(&raw, &path, &peers)?;
                }
            }
        }
        ensure!(
            kilogram_ratchet::ordered::native_call_counts() == native_before,
            "migration must not call packet crypto/constructors"
        );
        Ok(Self {
            nonce,
            device,
            enrollment,
            contexts,
        })
    }
}
