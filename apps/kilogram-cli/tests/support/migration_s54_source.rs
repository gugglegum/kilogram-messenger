//! Exclusive bounded reads of this attempt only. No fixture copy or Native calls.
use super::*;
use kilogram_cli::ordered_dialog::{engine, handshake::Spec};
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use zeroize::Zeroizing;
#[cfg(test)]
#[path = "migration_s54_source_terminal_tests.rs"]
mod terminal_tests;

pub(super) fn owned(node: &Node) -> Result<()> {
    let root = workspace()
        .canonicalize()?
        .join(".tmp/migration-s54-integrated");
    let state = node.branch.state.canonicalize()?;
    ensure!(
        state.starts_with(&root) && state != root,
        "only this fresh S54 origin source"
    );
    Ok(())
}
fn hash(raw: &[u8]) -> String {
    blake3::hash(raw).to_hex().to_string()
}
struct SavedContext {
    c: old::Id,
    role: u8,
    spec: Vec<u8>,
    head: Vec<u8>,
    native: Vec<u8>,
    keys: Vec<String>,
    events: BTreeMap<(u8, u64), Vec<u8>>,
    history: BTreeMap<String, Value>,
    queue: BTreeMap<[u8; 16], Vec<u8>>,
    receipts: BTreeMap<(u8, u64), Vec<u8>>,
}
pub(super) struct Legacy {
    state: PathBuf,
    signing: Zeroizing<old::Id>,
    encryption: Zeroizing<old::Id>,
    contexts: Vec<SavedContext>,
    pub source_id: old::Id,
}
impl Legacy {
    pub fn terminal_native(&self, index: usize, sid: &str, keys: &[old::Id; 3]) -> Result<()> {
        let ctx = self
            .contexts
            .get(index)
            .context("original Native logical context")?;
        let head = old::decode(&ctx.head, 2048)?;
        let fields = head.fields(13)?;
        ensure!(
            fields[0].uint()? == 1
                && fields[1].fixed::<32>()? == ctx.c
                && fields[11]
                    .option()?
                    .context("original Native SID")?
                    .text()?
                    == sid
                && ctx.keys.len() == 3
                && keys
                    .iter()
                    .zip(&ctx.keys)
                    .all(|(actual, original)| state::hex(actual) == *original),
            "original immutable Native SID and all three session keys including base key"
        );
        Ok(())
    }
    pub fn terminal_identity(
        &self,
        node: &Node,
        material: &db::migration7::live1::L1OwnerMaterial,
    ) -> Result<()> {
        ensure!(
            node.branch.state.canonicalize()? == self.state.canonicalize()?
                && material.signing == self.signing
                && material.encryption == self.encryption,
            "original state/device immutable identity, no Head/Native comparison"
        );
        Ok(())
    }
    pub fn terminal_scope(
        &self,
        index: usize,
        spec: &[u8],
        origin: &kilogram_protocol::ordered_dialog_v2::VerifiedExistingScope1,
    ) -> Result<()> {
        let ctx = self
            .contexts
            .get(index)
            .context("original logical context witness")?;
        let parsed = Spec::parse(&ctx.spec)?;
        let head = state::Head::parse(
            &ctx.head,
            ctx.c,
            parsed.cert_refs()?[ctx.role as usize],
            ctx.role as u64,
        )?;
        ensure!(
            ctx.spec == spec
                && ctx.c == origin.c()
                && head.d == Some(origin.d())
                && head.e == Some(origin.e())
                && head.sid.as_deref() == Some(origin.sid())
                && head.offer == Some(origin.old_control_ids()[0])
                && head.grant == Some(origin.old_control_ids()[1]),
            "original immutable Spec/genesis session binding"
        );
        Ok(())
    }
    pub fn terminal_source(
        &self,
        index: usize,
        role: u8,
        n: u64,
        raw: &[u8],
        receipt: Option<&[u8]>,
    ) -> Result<()> {
        let ctx = self
            .contexts
            .get(index)
            .context("original immutable source context")?;
        if let Some(original) = ctx.events.get(&(role, n)) {
            ensure!(original == raw, "original Legacy source bytes");
        }
        if let Some(saved) = ctx.receipts.get(&(role, n)) {
            ensure!(
                receipt == Some(saved.as_slice()),
                "original saved Legacy receipt bytes"
            );
        }
        Ok(())
    }
    pub fn text_id(&self, role: u8, n: u64) -> Result<old::Id> {
        ensure!(n > 0, "original Text ordinal");
        let raw = self.contexts[0]
            .events
            .get(&(role, n))
            .context("actual original Text witness")?;
        Ok(old::hash(old::EVENT_ID, raw))
    }
    pub fn packet_effects(&self) -> usize {
        self.contexts.iter().map(|c| c.events.len()).sum()
    }
    pub fn capture(node: &Node, contexts: &[String; 2]) -> Result<Self> {
        owned(node)?;
        ensure!(!node.indexed, "actual original V4 source");
        let source = db::migration7::V4Source::open(
            kilogram_state::StateDirectoryLock::acquire(&node.branch.state)?,
            db::Config::default(),
        )?;
        let signing = Zeroizing::new(
            source
                .required("device-secret.key")?
                .as_slice()
                .try_into()?,
        );
        let encryption = Zeroizing::new(
            source
                .required("device-encryption-secret.key")?
                .as_slice()
                .try_into()?,
        );
        let device = DeviceEncryptionIdentity::from_secret_bytes(*encryption);
        let claim = old::decode(&source.required("runtime/od1/profile-claim")?, 1024)?.fields(5)?
            [2]
        .fixed()?;
        let mut saved = Vec::new();
        for (index, c) in contexts.iter().enumerate() {
            s53_deadline::check()?;
            let c = state::parse_id(c)?;
            let spec_raw = source
                .required(&format!("{}spec", engine::prefix(c)))?
                .to_vec();
            let spec = Spec::parse(&spec_raw)?;
            let role = spec.role(node.branch.device.parse()?)?;
            let head_raw = source.required(&state::head_path(c))?.to_vec();
            let head = state::Head::parse(&head_raw, c, spec.cert_refs()?[role as usize], role)?;
            let native = source.required(&state::native_path(c))?.to_vec();
            let private = state::PrivateState::open(&head, claim, &device, &native)?;
            let guard = private.guard.as_ref().context("genuine Ready Guard1")?;
            let mut next = [0; 2];
            next[role as usize] = guard.own.next;
            next[1 - role as usize] = guard.peer.next;
            ensure!(
                next.iter().all(|n| (1..=25).contains(n)) && (index == 0 || next == [1, 1]),
                "early legacy/C2 zero-Text bounds"
            );
            let mut events = BTreeMap::new();
            let mut history = BTreeMap::new();
            let mut receipts = BTreeMap::new();
            for lane in 0..2u8 {
                for n in 0..next[lane as usize] {
                    let id: old::Id = source
                        .required(&engine::ordinal_path(c, u64::from(lane), n))?
                        .as_slice()
                        .try_into()?;
                    let raw = source.required(&engine::event_path(c, id))?.to_vec();
                    let event =
                        old::Event::parse(&raw, spec.peers[lane as usize].cert.device_id())?;
                    ensure!(
                        event.id()? == id
                            && event.core.c == c
                            && event.core.role == u64::from(lane)
                            && event.core.n == n,
                        "immutable legacy source association"
                    );
                    if n > 0 {
                        let raw = source.required(&format!(
                            "local-messages/od1/{}/{}",
                            state::hex(&c),
                            state::hex(&id)
                        ))?;
                        let envelope = old::decode(&raw, 8192)?;
                        let fields = envelope.fields(3)?;
                        let plain = Zeroizing::new(device.open(
                            &SealedMessage {
                                encapsulated_key: fields[1].fixed::<32>()?.to_vec(),
                                ciphertext: fields[2].bytes()?.to_vec(),
                            },
                            b"kg:od1:local-projection\0",
                            &old::encode(
                                &old::a([
                                    old::u(1),
                                    old::b(c),
                                    old::b(head.d.context("D")?),
                                    old::b(head.e.context("E")?),
                                    old::b(id),
                                ]),
                                1024,
                            )?,
                        )?);
                        let decoded = Zeroizing::new(old::decode(&plain, 8192)?);
                        let f = decoded.fields(7)?;
                        ensure!(
                            f[0].uint()? == 1
                                && f[1].fixed::<32>()? == c
                                && f[2].fixed::<32>()? == head.d.context("D")?
                                && f[3].fixed::<32>()? == head.e.context("E")?
                                && f[4].fixed::<32>()? == id,
                            "actual projection binding"
                        );
                        let request = oracle::Request::new(index as u8, lane, n.try_into()?)?;
                        ensure!(
                            f[5].text()? == request.body()
                                && state::hex(&f[6].fixed::<16>()?) == request_id(request),
                            "original request/body source projection"
                        );
                        history.insert(state::hex(&id),json!({"format":1,"id":state::hex(&id),"role":lane,"ordinal":n,"request":request_id(request),"text":request.body()}));
                    }
                    events.insert((lane, n), raw);
                    if let Some(raw) = source.get(
                        &kilogram_cli::ordered_dialog::outbox::receipt_path(c, u64::from(lane), n),
                    )? {
                        receipts.insert((lane, n), raw.to_vec());
                    }
                }
            }
            let mut queue = BTreeMap::new();
            for entry in source
                .entries()
                .iter()
                .filter(|e| e.context == Some(c) && e.kind == 13)
            {
                ensure!(queue.len() < 2, "only original17/18 queued");
                let name = entry
                    .path
                    .rsplit('-')
                    .next()
                    .context("queue request suffix")?;
                let req = parse_request_id(name)?;
                ensure!(index == 0 && role == 0, "only original A C1 offline queue");
                let request = [17, 18]
                    .into_iter()
                    .map(|n| oracle::Request::new(0, 0, n))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .find(|r| request_id(*r) == name)
                    .context("exact original queued17/18 request")?;
                let sealed = source.required(&entry.path)?;
                let envelope = old::decode(&sealed, 8192)?;
                let f = envelope.fields(3)?;
                ensure!(f[0].uint()? == 1, "real sealed queue envelope");
                let plain = Zeroizing::new(device.open(
                    &SealedMessage {
                        encapsulated_key: f[1].fixed::<32>()?.to_vec(),
                        ciphertext: f[2].bytes()?.to_vec(),
                    },
                    b"kg:od1:local-projection\0",
                    &old::encode(
                        &old::a([old::u(1), old::b(c), old::b(req), old::u(0)]),
                        1024,
                    )?,
                )?);
                let value = Zeroizing::new(old::decode(&plain, 8192)?);
                let q = value.fields(4)?;
                ensure!(
                    q[0].uint()? == 1
                        && q[1].fixed::<32>()? == c
                        && q[2].fixed::<16>()? == req
                        && q[3].text()? == request.body(),
                    "same original sealed queue request/body"
                );
                ensure!(
                    queue.insert(req, sealed.to_vec()).is_none(),
                    "unique sealed queue"
                );
            }
            let keys = private
                .session
                .as_ref()
                .context("real Native session")?
                .keys()
                .iter()
                .map(|k| state::hex(k))
                .collect();
            saved.push(SavedContext {
                c,
                role: role.try_into()?,
                spec: spec_raw,
                head: head_raw,
                native,
                keys,
                events,
                history,
                queue,
                receipts,
            });
        }
        Ok(Self {
            state: node.branch.state.clone(),
            signing,
            encryption,
            contexts: saved,
            source_id: source.source_id(),
        })
    }
    pub fn after_migration(&self, node: &Node, ready: &Value) -> Result<intervals::Endpoint> {
        ensure!(
            node.branch.state == self.state
                && ready["zero_migration_native_calls"] == true
                && ready["source_id"] == state::hex(&self.source_id),
            "same original directory/source and zero migration crypto"
        );
        let profile = Profile3::load(&node.profile)?;
        let mut repo = profile.open(&node.profile)?;
        let material = repo.owner_material()?;
        ensure!(
            material.signing == self.signing && material.encryption == self.encryption,
            "same in-memory device secrets"
        );
        let mut lanes = Vec::new();
        for ctx in &self.contexts {
            let slot = oracle::ready_slot(ready, &state::hex(&ctx.c))?;
            let snapshot = repo.snapshot(slot)?;
            ensure!(
                repo.contexts().get(slot as usize) == Some(&(ctx.c, ctx.role)),
                "original Ready/context/role endpoint"
            );
            lanes.push(intervals::committed_lane(
                ctx.c,
                slot,
                ctx.role,
                &snapshot.meta,
            ));
            ensure!(
                repo.spec(slot)? == ctx.spec
                    && snapshot.head == ctx.head
                    && snapshot.native.as_ref() == Some(&ctx.native),
                "exact original Spec/Head/Native before first current start"
            );
            for ((role, n), raw) in &ctx.events {
                ensure!(
                    repo.event_at(slot, *role, *n)?.as_ref() == Some(raw),
                    "all original source bytes"
                );
            }
            let queue = repo
                .queue_page(slot, None, 32)?
                .into_iter()
                .map(|q| (q.request, q.sealed))
                .collect::<BTreeMap<_, _>>();
            ensure!(queue == ctx.queue, "sealed original queue bytes unchanged");
        }
        let endpoint = intervals::Endpoint {
            source: repo.source_id(),
            lanes: lanes
                .try_into()
                .map_err(|_| anyhow::anyhow!("two migration endpoints"))?,
        };
        endpoint.validate(endpoint.lanes[0].role)?;
        ensure!(
            endpoint.source == self.source_id,
            "original migration endpoint source"
        );
        Ok(endpoint)
    }
    pub fn identity(&self, node: &Node, rows: &[Value; 2]) -> Result<()> {
        ensure!(node.branch.state == self.state, "same original state path");
        for (index, row) in rows.iter().enumerate() {
            let c = &self.contexts[index];
            let spec = Spec::parse(&c.spec)?;
            let role = spec.role(node.branch.device.parse()?)?;
            let head = state::Head::parse(&c.head, c.c, spec.cert_refs()?[role as usize], role)?;
            ensure!(
                row["context"] == state::hex(&c.c)
                    && row["role"] == role
                    && row["d"] == json!(head.d.map(|id| state::hex(&id)))
                    && row["e"] == json!(head.e.map(|id| state::hex(&id)))
                    && row["sid"] == json!(head.sid)
                    && row["native"]["keys"] == json!(c.keys),
                "original C/D/E/SID/public Native keys"
            );
        }
        Ok(())
    }
    pub fn first_history(&self, rows: &[Value]) -> Result<()> {
        let current = history_map(rows)?;
        ensure!(
            current == self.contexts[0].history,
            "first indexed history is same readable original Text"
        );
        Ok(())
    }
    pub fn late_old(&self, actual: &[Value]) -> Result<Vec<Value>> {
        let map = history_map(actual)?;
        let mut observed = Vec::new();
        for n in 16..=24 {
            let expected = self.contexts[0]
                .history
                .values()
                .find(|r| r["role"] == 1 && r["ordinal"] == n)
                .context("actual original B16..24 source witness")?;
            let id = expected["id"].as_str().context("original late source ID")?;
            ensure!(
                map.get(id) == Some(expected),
                "same old B source becomes readable at current A"
            );
            observed.push(json!({"role":1,"ordinal":n,"event":id,"effect":"current-old-decrypt"}));
        }
        Ok(observed)
    }
    pub fn same_identity(&self, other: &Self) -> Result<()> {
        ensure!(
            self.state == other.state
                && self.signing == other.signing
                && self.encryption == other.encryption,
            "same original owner secrets in memory"
        );
        for (a, b) in self.contexts.iter().zip(&other.contexts) {
            ensure!(
                a.c == b.c && a.spec == b.spec && a.keys == b.keys,
                "same original context/spec/Native keys"
            );
            let spec = Spec::parse(&a.spec)?;
            let role = spec.role(
                kilogram_identity::DeviceIdentity::from_secret_bytes(*self.signing).device_id(),
            )?;
            let x = state::Head::parse(&a.head, a.c, spec.cert_refs()?[role as usize], role)?;
            let y = state::Head::parse(&b.head, b.c, spec.cert_refs()?[role as usize], role)?;
            ensure!(
                x.d == y.d && x.e == y.e && x.sid == y.sid,
                "same original D/E/SID"
            );
        }
        Ok(())
    }
    pub fn report(&self) -> Value {
        json!({"source":state::hex(&self.source_id),"contexts":self.contexts.iter().map(|c|json!({"context":state::hex(&c.c),"role":c.role,"spec":hash(&c.spec),"head":hash(&c.head),"native":hash(&c.native),"keys_fingerprint":hash(c.keys.join(",").as_bytes()),"readable_text":c.history.len(),"queued":c.queue.len()})).collect::<Vec<_>>()})
    }
    pub fn role(&self, owner: usize) -> Result<()> {
        ensure!(
            self.contexts.iter().all(|c| usize::from(c.role) == owner),
            "actual signed Spec and original device role in both contexts"
        );
        Ok(())
    }
    pub fn r1(&self, a: &Node) -> Result<Value> {
        let ctx = &self.contexts[0];
        let raw = ctx
            .receipts
            .get(&(0, 24))
            .context("actual nonempty saved R1(A24) at original B")?;
        let source = ctx.events.get(&(0, 24)).context("actual A24 at B")?;
        let spec = Spec::parse(&ctx.spec)?;
        let event = old::Event::parse(source, spec.peers[0].cert.device_id())?;
        let id = event.id()?;
        let value = old::decode(raw, 1024)?;
        let f = value.fields(2)?[0].fields(8)?;
        ensure!(
            f[0].uint()? == 1
                && f[1].fixed::<32>()? == event.core.d
                && f[2].fixed::<32>()? == event.core.e
                && f[3].uint()? == 0
                && f[4].uint()? == 24
                && f[5].fixed::<32>()? == id,
            "saved R1 exact source/context/lane/ordinal"
        );
        ensure!(
            f[6].fixed::<32>()? == spec.cert_refs()?[1],
            "actual saved receiver certificate"
        );
        spec.peers[1].cert.device_id().verify(
            &old::signing_bytes(old::RECEIPT_SIGN, &value.fields(2)?[0], 1024)?,
            value.fields(2)?[1].bytes()?,
        )?;
        let p = Profile3::load(&a.profile)?;
        let mut repo = p.open(&a.profile)?;
        ensure!(
            repo.debt_page(a.slot, None, 32)? == vec![(24, id)]
                && repo.event_at(a.slot, 0, 24)?.as_ref() == Some(source)
                && repo.receipt(a.slot, 0, 24)?.is_none(),
            "R1 retained at B with same actual nonempty A debt"
        );
        Ok(
            json!({"event":state::hex(&id),"receipt_blake3":hash(raw),"source_blake3":hash(source),"lane":0,"ordinal":24}),
        )
    }
}
pub(super) fn request_id(request: oracle::Request) -> String {
    format!("fa00000000000000{:016x}", request.numeric())
}
pub(super) fn parse_request_id(value: &str) -> Result<[u8; 16]> {
    ensure!(
        value.len() == 32
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "canonical request ID"
    );
    let mut out = [0; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&value[2 * i..2 * i + 2], 16)?;
    }
    Ok(out)
}
pub(super) fn history_map(rows: &[Value]) -> Result<BTreeMap<String, Value>> {
    ensure!(rows.len() <= 1344, "bounded readable history");
    let mut map = BTreeMap::new();
    for r in rows {
        let id = r["id"].as_str().context("actual history id")?;
        state::parse_id(id)?;
        let v = json!({"format":r["format"],"id":r["id"],"role":r["role"],"ordinal":r["ordinal"],"request":r["request"],"text":r["text"]});
        ensure!(
            map.insert(id.into(), v).is_none(),
            "duplicate actual history source"
        );
    }
    Ok(map)
}
#[derive(Clone, Debug)]
pub(super) struct End {
    pub point: (u64, u64),
    pub endpoint: intervals::Endpoint,
    pub generation: u64,
    pub operation: old::Id,
}
pub(super) fn point(
    node: &Node,
    contexts: &[String; 2],
) -> Result<(old::Id, (u64, u64), intervals::Endpoint)> {
    let end = end(node, contexts)?;
    Ok((end.endpoint.source, end.point, end.endpoint))
}
pub(super) fn end(node: &Node, contexts: &[String; 2]) -> Result<End> {
    read_end(node, contexts, None)
}
pub(super) fn normal_end(node: &Node, contexts: &[String; 2], source: old::Id) -> Result<End> {
    read_end(node, contexts, Some(source))
}
fn read_end(node: &Node, contexts: &[String; 2], expected: Option<old::Id>) -> Result<End> {
    owned(node)?;
    let p = Profile3::load(&node.profile)?;
    let mut repo = p.open(&node.profile)?;
    if let Some(source) = expected {
        ensure!(
            repo.source_id() == source
                && p.account == node.branch.account
                && p.device == node.branch.device,
            "same actual normally stopped source/profile owner"
        );
    }
    let mut lanes = Vec::new();
    for c in contexts {
        let slot = repo
            .contexts()
            .iter()
            .position(|(id, _)| state::hex(id) == *c)
            .context("actual stored context mapping")? as u8;
        let (_, role) = repo.contexts()[slot as usize];
        let snap = repo.snapshot(slot)?;
        lanes.push(intervals::committed_lane(
            state::parse_id(c)?,
            slot,
            role,
            &snap.meta,
        ));
    }
    let endpoint = intervals::Endpoint {
        source: repo.source_id(),
        lanes: lanes
            .try_into()
            .map_err(|_| anyhow::anyhow!("two committed context endpoints"))?,
    };
    Ok(End {
        point: endpoint.point()?,
        endpoint,
        generation: repo.generation(),
        operation: repo.last_commit()?,
    })
}
pub(super) fn debt(node: &Node, request: oracle::Request) -> Result<Value> {
    owned(node)?;
    let p = Profile3::load(&node.profile)?;
    let mut repo = p.open(&node.profile)?;
    let c = node.slot;
    let raw = repo
        .event_at(c, request.role, u64::from(request.number))?
        .context("real durable debt source")?;
    let event = kilogram_protocol::ordered_dialog_v2::ParsedEvent2::parse(&raw)?;
    let id = event.id();
    ensure!(
        repo.request(c, parse_request_id(&request_id(request))?)?
            .is_some_and(|r| r.event == Some(id))
            && repo.debt_page(c, None, 32)? == vec![(u64::from(request.number), id)]
            && repo
                .receipt(c, request.role, u64::from(request.number))?
                .is_none(),
        "one exact original request/debt and no fabricated receipt"
    );
    let core = event.core();
    ensure!(
        core.c == state::parse_id(&node.branch.context)?
            && core.role == request.role
            && core.n == u64::from(request.number),
        "B640 immutable association"
    );
    Ok(
        json!({"request":request_id(request),"id":state::hex(&id),"source_blake3":hash(&raw),"ciphertext_blake3":hash(old::decode(&raw,kilogram_protocol::ordered_dialog_v2::MAX_EVENT)?.fields(5)?[3].bytes()?),"receipt_expected":{"context":node.branch.context,"role":request.role,"ordinal":request.number,"event_id":state::hex(&id)}}),
    )
}
pub(super) fn current_old_encrypts(node: &Node) -> Result<Vec<Value>> {
    owned(node)?;
    let p = Profile3::load(&node.profile)?;
    let mut repo = p.open(&node.profile)?;
    let spec = Spec::parse(&repo.spec(node.slot)?)?;
    let mut out = Vec::new();
    for n in 17..=24u16 {
        let raw = repo
            .event_at(node.slot, 0, u64::from(n))?
            .context("actual current-generated old source")?;
        let event = old::Event::parse(&raw, spec.peers[0].cert.device_id())?;
        let request = oracle::Request::new(0, 0, n)?;
        let id = event.id()?;
        ensure!(
            event.core.c == spec.c
                && event.core.role == 0
                && event.core.n == u64::from(n)
                && event.core.request == parse_request_id(&request_id(request))?
                && repo.projection(node.slot, id)?.text == request.body(),
            "actual source-bound current old encrypt"
        );
        out.push(json!({"role":0,"ordinal":n,"event":state::hex(&id),"source_blake3":hash(&raw),"effect":"current-old-encrypt"}));
    }
    Ok(out)
}
pub(super) fn accepted_r1(node: &Node, original: &Value) -> Result<Value> {
    owned(node)?;
    let p = Profile3::load(&node.profile)?;
    let mut repo = p.open(&node.profile)?;
    let raw = repo
        .receipt(node.slot, 0, 24)?
        .context("actual accepted saved R1")?;
    ensure!(
        hash(&raw) == original["receipt_blake3"] && repo.debt_page(node.slot, None, 32)?.is_empty(),
        "exact saved R1 bytes retired original debt once"
    );
    Ok(json!({"receipt_blake3":hash(&raw),"event":original["event"],"debt":0}))
}
