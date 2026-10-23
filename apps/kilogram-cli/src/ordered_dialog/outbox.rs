//! Stable queue IDs, immutable materialization and receipt reconciliation.
use super::{
    admin,
    engine::{Changes, Engine, outbox_path, prefix, projection_path},
    state,
};
use anyhow::{Context, Result, ensure};
use kilogram_crypto::SealedMessage;
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Id, Value, a, b, u};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

const LOCAL: &[u8] = b"kg:od1:local-projection\0";
pub(super) struct HistoryEntry {
    id: Id,
    role: u64,
    n: u64,
    sequence: u64,
    request: [u8; 16],
}
pub(super) struct HistorySnapshot {
    paths: Vec<String>,
    entries: Vec<HistoryEntry>,
}
pub fn queue_path(c: Id, request: [u8; 16], generation: u64) -> String {
    format!(
        "{}queue/{generation:020}-{}",
        prefix(c),
        state::hex(&request)
    )
}
pub fn request_path(c: Id, request: [u8; 16]) -> String {
    format!("{}requests/{}", prefix(c), state::hex(&request))
}
pub fn receipt_path(c: Id, lane: u64, n: u64) -> String {
    format!("{}receipts/{lane}/{n:020}", prefix(c))
}

impl Engine {
    fn seal_local(&self, value: Value, aad: Value, cap: usize) -> Result<Vec<u8>> {
        let value = Zeroizing::new(value);
        let raw = Zeroizing::new(wire::encode(&value, cap)?);
        let aad = wire::encode(&aad, 1024)?;
        let sealed = self
            .device
            .encryption()
            .public_key()
            .seal(&raw, LOCAL, &aad)?;
        Ok(wire::encode(
            &a([u(1), b(sealed.encapsulated_key), b(sealed.ciphertext)]),
            cap,
        )?)
    }
    fn open_local(&self, raw: &[u8], aad: Value, cap: usize) -> Result<Zeroizing<Value>> {
        let value = wire::decode(raw, cap)?;
        let f = value.fields(3)?;
        ensure!(f[0].uint()? == 1, "local projection format");
        let sealed = SealedMessage {
            encapsulated_key: f[1].fixed::<32>()?.to_vec(),
            ciphertext: f[2].bytes()?.to_vec(),
        };
        let raw = Zeroizing::new(self.device.encryption().open(
            &sealed,
            LOCAL,
            &wire::encode(&aad, 1024)?,
        )?);
        Ok(Zeroizing::new(wire::decode(&raw, cap)?))
    }
    pub fn projection(&self, c: Id, id: Id) -> Result<(String, [u8; 16])> {
        let head = self.head(c)?;
        let d = head.d.context("projection domain")?;
        let e = head.e.context("projection epoch")?;
        let raw = admin::required(&self.store, &projection_path(c, id))?;
        let value = self.open_local(&raw, a([u(1), b(c), b(d), b(e), b(id)]), 8192)?;
        let f = value.fields(7)?;
        ensure!(
            f[0].uint()? == 1
                && f[1].fixed::<32>()? == c
                && f[2].fixed::<32>()? == d
                && f[3].fixed::<32>()? == e
                && f[4].fixed::<32>()? == id,
            "projection binding"
        );
        Ok((f[5].text()?.to_owned(), f[6].fixed()?))
    }
    pub fn project(&self, event: &Event, text: &str, changes: &mut Changes) -> Result<()> {
        ensure!(text.len() <= 4096, "text cap");
        let core = &event.core;
        let id = event.id()?;
        let raw = self.seal_local(
            a([
                u(1),
                b(core.c),
                b(core.d),
                b(core.e),
                b(id),
                b(text.as_bytes()),
                b(core.request),
            ]),
            a([u(1), b(core.c), b(core.d), b(core.e), b(id)]),
            8192,
        )?;
        changes.put(projection_path(core.c, id), raw);
        Ok(())
    }
    pub fn send_capacity(&self, c: Id) -> Result<u64> {
        let head = self.head(c)?;
        let next = if head.phase == 3 {
            self.native(&head)?.guard.context("guard")?.own.next
        } else {
            // n=0 is reserved for the local handshake genesis.
            1
        };
        Ok(wire::LIFETIME.saturating_sub(next))
    }
    pub fn send_blocked(&self, c: Id) -> Result<bool> {
        let remaining = self.send_capacity(c)?;
        Ok(remaining == 0
            || self.store.paths(&format!("{}queue/", prefix(c))).len() as u64 > remaining)
    }
    pub fn queue(&mut self, c: Id, request: [u8; 16], body: &str) -> Result<bool> {
        self.policy(c)?;
        ensure!(body.len() <= 4096, "Text body max4096 UTF8 bytes");
        if let Some(id) = self.store.get(&request_path(c, request))? {
            let id: Id = id.as_slice().try_into().context("request mapping ID")?;
            let (prior, prior_request) = self.projection(c, id)?;
            ensure!(
                prior_request == request && prior == body,
                "RequestIdConflict"
            );
            return Ok(false);
        }
        let suffix = format!("-{}", state::hex(&request));
        if let Some(path) = self
            .store
            .paths(&format!("{}queue/", prefix(c)))
            .into_iter()
            .find(|p| p.ends_with(&suffix))
        {
            let raw = admin::required(&self.store, &path)?;
            let value = self.open_local(&raw, a([u(1), b(c), b(request), u(0)]), 8192)?;
            ensure!(value.fields(4)?[3].text()? == body, "RequestIdConflict");
            return Ok(false);
        }
        let queued = self.store.paths(&format!("{}queue/", prefix(c))).len();
        // Check only NEW requests: accepted queue entries and committed request
        // IDs retain their idempotent result even after lifetime exhaustion.
        ensure!(
            (queued as u64) < self.send_capacity(c)?,
            "UpgradeRequired: send lane lifetime capacity"
        );
        ensure!(queued < 128, "StorageBlocked: unmaterialized queue");
        let raw = self.seal_local(
            a([u(1), b(c), b(request), b(body.as_bytes())]),
            a([u(1), b(c), b(request), u(0)]),
            8192,
        )?;
        let mut head = self.head(c)?;
        head.bump(false)?;
        let mut changes = Changes::default();
        changes.put(queue_path(c, request, head.generation), raw);
        self.save(&mut head, None, changes)?;
        super::test_support::checkpoint("after-queue")?;
        Ok(true)
    }
    pub fn unacked(&self, c: Id) -> Result<Vec<(u64, Id)>> {
        self.store
            .paths(&format!("{}outbox/", prefix(c)))
            .iter()
            .filter_map(|path| {
                let result = (|| {
                    let raw = admin::required(&self.store, path)?;
                    let value = wire::decode(&raw, 1024)?;
                    let f = value.fields(3)?;
                    ensure!(f[0].uint()? == 1, "outbox format");
                    if f[2].uint()? == 1 {
                        return Ok(None);
                    }
                    ensure!(f[2].uint()? == 0, "outbox state");
                    let n = path
                        .rsplit('/')
                        .next()
                        .context("outbox ordinal")?
                        .parse::<u64>()?;
                    Ok(Some((n, f[1].fixed()?)))
                })();
                match result {
                    Ok(Some(v)) => Some(Ok(v)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            })
            .collect()
    }
    pub fn materialize_one(&mut self, c: Id, binding: Id) -> Result<Option<Event>> {
        self.check_lease(c, binding)?;
        ensure!(self.active(c)?, "NeedHandshake");
        if self.unacked(c)?.len() >= 128 {
            return Ok(None);
        }
        let Some(path) = self
            .store
            .paths(&format!("{}queue/", prefix(c)))
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let name = path.rsplit('-').next().context("queue request ID")?;
        ensure!(name.len() == 32, "queue ID shape");
        let mut request = [0; 16];
        for (i, v) in request.iter_mut().enumerate() {
            *v = u8::from_str_radix(&name[i * 2..i * 2 + 2], 16)?;
        }
        let value = self.open_local(
            &admin::required(&self.store, &path)?,
            a([u(1), b(c), b(request), u(0)]),
            8192,
        )?;
        let f = value.fields(4)?;
        ensure!(
            f[0].uint()? == 1 && f[1].fixed::<32>()? == c && f[2].fixed::<16>()? == request,
            "queued body binding"
        );
        let text = f[3].text()?;
        ensure!(text.len() <= 4096, "queued body cap");
        let mut head = self.head(c)?;
        let mut native = self.native(&head)?;
        if native.guard.as_ref().context("guard")?.own.next >= wire::LIFETIME {
            // send_blocked() exposes UpgradeRequired without terminating the
            // connection: receive, old ciphertext and ACK repair remain usable.
            return Ok(None);
        }
        let credential = self.own_credentials()?;
        ensure!(
            credential.cert_ref()? == head.local_cert,
            "own epoch certificate changed"
        );
        let mut changes = Changes::default();
        let auth = self.add_auth(&mut changes, &credential.raw)?;
        let seq = self.sequence(&mut changes)?;
        let event = self.encrypt(
            &head,
            &mut native,
            super::engine::Outgoing {
                kind: 3,
                request,
                payload: a([b(text.as_bytes())]),
                auth,
                sequence: seq,
            },
        )?;
        let id = event.id()?;
        self.record_event(&event, &mut changes)?;
        self.project(&event, text, &mut changes)?;
        changes.put(request_path(c, request), id.to_vec());
        changes.put(
            outbox_path(c, event.core.n),
            wire::encode(&a([u(1), b(id), u(0)]), 1024)?,
        );
        changes.remove(path);
        head.bump(true)?;
        self.save(&mut head, Some(&native), changes)?;
        super::test_support::checkpoint("after-materialization")?;
        Ok(Some(event))
    }
    pub fn make_receipt(&self, event: &Event, changes: &mut Changes) -> Result<Vec<u8>> {
        ensure!(event.core.kind == 3, "control genesis has no message ACK");
        let credentials = self.own_credentials()?;
        let auth = self.add_auth(changes, &credentials.raw)?;
        let body = a([
            u(1),
            b(event.core.d),
            b(event.core.e),
            u(event.core.role),
            u(event.core.n),
            b(event.id()?),
            b(credentials.cert_ref()?),
            b(auth),
        ]);
        let signature =
            self.device
                .identity()
                .sign(&wire::signing_bytes(wire::RECEIPT_SIGN, &body, 1024)?);
        Ok(wire::encode(&a([body, b(signature)]), 1024)?)
    }
    pub fn accept_receipt(
        &mut self,
        c: Id,
        raw: &[u8],
        auth_raw: &[u8],
        binding: Id,
    ) -> Result<bool> {
        self.check_lease(c, binding)?;
        let mut head = self.head(c)?;
        let value = wire::decode(raw, 1024)?;
        let r = value.fields(2)?;
        let f = r[0].fields(8)?;
        let peer = 1 - head.role;
        let cert = &self.spec(c)?.peers[peer as usize].cert;
        cert.device_id().verify(
            &wire::signing_bytes(wire::RECEIPT_SIGN, &r[0], 1024)?,
            r[1].bytes()?,
        )?;
        let auth = self.credential(auth_raw)?;
        ensure!(
            auth.cert_ref()? == self.spec(c)?.peers[peer as usize].cert_ref()?
                && auth.reference() == f[7].fixed::<32>()?,
            "receipt authorization reference"
        );
        ensure!(
            f[0].uint()? == 1
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == head.role
                && f[4].uint()? > 0
                && f[6].fixed::<32>()? == auth.cert_ref()?,
            "receipt domain/lane/signer"
        );
        let n = f[4].uint()?;
        let id = f[5].fixed()?;
        let event = self
            .event_at(c, head.role, n)?
            .context("receipt unknown event")?;
        ensure!(
            event.id()? == id && event.core.kind == 3,
            "receipt exact source ID"
        );
        let status = wire::decode(&admin::required(&self.store, &outbox_path(c, n))?, 1024)?;
        if status.fields(3)?[2].uint()? == 1 {
            return Ok(false);
        }
        let mut changes = Changes::default();
        self.add_auth(&mut changes, auth_raw)?;
        changes.put(
            outbox_path(c, n),
            wire::encode(&a([u(1), b(id), u(1)]), 1024)?,
        );
        changes.put(receipt_path(c, head.role, n), raw.to_vec());
        head.bump(false)?;
        self.save(&mut head, None, changes)?;
        Ok(true)
    }
    /// Bounded deterministic topological order of locally readable user events.
    pub fn history(&self, c: Id, offset: usize, limit: usize) -> Result<Value> {
        ensure!(limit > 0 && limit <= 8, "OD1 history page max8");
        let paths = self.store.paths(&format!("events/od1/{}/", state::hex(&c)));
        let rebuild = self
            .history_cache
            .borrow()
            .get(&c)
            .is_none_or(|s| s.paths != paths);
        if rebuild {
            // Immutable, signature-verified metadata is cached at the L1 bound
            // (1024 encrypted frames/context). Plaintexts are never cached here.
            // New event paths invalidate the order. Projections still authenticate
            // against current primary records and scope AAD on every page read.
            let head = self.head(c)?;
            let mut events = BTreeMap::new();
            for path in &paths {
                let raw = admin::required(&self.store, path)?;
                let value = wire::decode(&raw, wire::MAX_EVENT)?;
                let role = value.fields(5)?[1].fields(14)?[5].uint()?;
                ensure!(role <= 1, "history role");
                let event =
                    Event::parse(&raw, self.spec(c)?.peers[role as usize].cert.device_id())?;
                self.check_scope(&head, &event, true)?;
                if event.core.kind == 3 {
                    events.insert(event.id()?, event);
                }
            }
            let mut ordered = Vec::new();
            let mut emitted = std::collections::BTreeSet::new();
            while !events.is_empty() {
                let id = events
                    .iter()
                    .find(|(_, e)| e.core.parents.iter().all(|p| emitted.contains(p)))
                    .map(|(id, _)| *id)
                    .context("invalid readable causal graph")?;
                let event = events.remove(&id).context("history event")?;
                emitted.insert(id);
                ordered.push(HistoryEntry {
                    id,
                    role: event.core.role,
                    n: event.core.n,
                    sequence: event.core.sequence,
                    request: event.core.request,
                });
            }
            self.history_cache.borrow_mut().insert(
                c,
                HistorySnapshot {
                    paths,
                    entries: ordered,
                },
            );
        }
        let cache = self.history_cache.borrow();
        let snapshot = cache.get(&c).context("history snapshot")?;
        let total = snapshot.entries.len();
        let items = snapshot
            .entries
            .iter()
            .skip(offset)
            .take(limit)
            .map(|e| -> Result<Value> {
                let id = e.id;
                let (text, request) = self.projection(c, id)?;
                ensure!(request == e.request, "local projection request binding");
                Ok(a([
                    b(id),
                    u(e.role),
                    u(e.n),
                    u(e.sequence),
                    b(request),
                    b(text.as_bytes()),
                ]))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(a([
            b(b"ordered-dialog-v1"),
            u(total as u64),
            u(offset as u64),
            a(items),
        ]))
    }
}
