//! Synchronous owner actor. Native workspaces never cross a network await.
use super::{
    admin,
    handshake::{self, Credentials, Spec},
    ratchet::Guard,
    state::{self, Head, PrivateState},
};
use anyhow::{Context, Result, ensure};
use kilogram_identity::{
    AccountAuthoritySnapshot, DeviceCapability, DeviceState,
    verify_device_authorization_with_snapshot,
};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Id, Value, a, b, u};
use kilogram_ratchet::ordered::Packet;
use kilogram_state::{OrderedMutation, OrderedVault};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> Result<u64> {
    let real = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let Some(path) = std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE") else {
        return Ok(real);
    };
    ensure!(cfg!(debug_assertions), "OD1 test clock is debug only");
    let offset = std::fs::read_to_string(path)?.trim().parse::<u64>()?;
    real.checked_add(offset).context("OD1 test clock overflow")
}
pub fn prefix(c: Id) -> String {
    format!("runtime/od1/contexts/{}/", state::hex(&c))
}
pub fn control_path(c: Id, id: Id) -> String {
    format!("{}controls/{}", prefix(c), state::hex(&id))
}
pub fn ready_path(c: Id, role: u64) -> String {
    format!("{}ready/{role}", prefix(c))
}
pub fn event_path(c: Id, id: Id) -> String {
    format!("events/od1/{}/{}", state::hex(&c), state::hex(&id))
}
pub fn ordinal_path(c: Id, role: u64, n: u64) -> String {
    format!("{}lane/{role}/{n:020}", prefix(c))
}
pub fn pending_path(c: Id, n: u64) -> String {
    format!("{}pending/{n:020}", prefix(c))
}
pub fn projection_path(c: Id, id: Id) -> String {
    format!("local-messages/od1/{}/{}", state::hex(&c), state::hex(&id))
}
pub fn outbox_path(c: Id, n: u64) -> String {
    format!("{}outbox/{n:020}", prefix(c))
}

#[derive(Clone)]
pub struct Lease {
    pub binding: Id,
    pub generation: u64,
    pub deadline: u64,
}
pub struct Outgoing {
    pub kind: u64,
    pub request: [u8; 16],
    pub payload: Value,
    pub auth: Id,
    pub sequence: u64,
}
pub struct Engine {
    pub store: OrderedVault,
    pub device: DeviceState,
    pub nonce: Id,
    pub contexts: BTreeMap<Id, Spec>,
    pub leases: BTreeMap<Id, Lease>,
    policy_cache: RefCell<BTreeMap<Id, (Id, u64, u64)>>,
    credential_cache: RefCell<BTreeMap<Id, Credentials>>,
    pub(super) history_cache: RefCell<BTreeMap<Id, super::outbox::HistorySnapshot>>,
}
#[derive(Default)]
pub struct Changes(BTreeMap<String, Option<Vec<u8>>>);
impl Changes {
    pub fn put(&mut self, path: String, raw: Vec<u8>) {
        self.0.insert(path, Some(raw));
    }
    pub fn remove(&mut self, path: String) {
        self.0.insert(path, None);
    }
    pub fn mutations(self) -> Vec<OrderedMutation> {
        self.0
            .into_iter()
            .map(|(p, v)| match v {
                Some(v) => OrderedMutation::put(p, v),
                None => OrderedMutation::remove(p),
            })
            .collect()
    }
}
impl Engine {
    pub fn new(store: OrderedVault) -> Result<Self> {
        let device = admin::device(&store)?;
        let nonce = admin::claim(&store)?.0;
        let contexts = admin::specs(&store)?
            .into_iter()
            .map(|s| (s.c, s))
            .collect();
        Ok(Self {
            store,
            device,
            nonce,
            contexts,
            leases: BTreeMap::new(),
            policy_cache: RefCell::new(BTreeMap::new()),
            credential_cache: RefCell::new(BTreeMap::new()),
            history_cache: RefCell::new(BTreeMap::new()),
        })
    }
    pub fn spec(&self, c: Id) -> Result<&Spec> {
        self.contexts.get(&c).context("unknown OD1 context")
    }
    pub fn role(&self, c: Id) -> Result<u64> {
        self.spec(c)?.role(self.device.identity().device_id())
    }
    pub fn head(&self, c: Id) -> Result<Head> {
        let role = self.role(c)?;
        let cert = self.spec(c)?.peers[role as usize].cert_ref()?;
        Head::parse(
            &admin::required(&self.store, &state::head_path(c))?,
            c,
            cert,
            role,
        )
    }
    pub fn policy(&self, c: Id) -> Result<u64> {
        ensure!(
            self.store
                .get(&format!("{}blocked-fork", prefix(c)))?
                .is_none(),
            "Blocked: signed fork evidence"
        );
        let spec = self.spec(c)?;
        let role = self.role(c)?;
        let consent = admin::required(&self.store, &admin::consent_path(c))?;
        let own = admin::required(&self.store, "account-authority.snapshot")?;
        let peer = admin::required(
            &self.store,
            &format!(
                "peer-authority/{}.snapshot",
                spec.peers[1 - role as usize].cert.account_id()
            ),
        )?;
        let digest = wire::hash(
            b"kg:od1:verified-policy-cache\0",
            &wire::encode(&a([b(&consent), b(&own), b(&peer)]), 40 * 1024)?,
        );
        if let Some((prior, nb, exp)) = self.policy_cache.borrow().get(&c)
            && *prior == digest
        {
            ensure!(
                now()?.saturating_add(120) >= *nb && now()? < *exp,
                "AuthBlocked: consent interval"
            );
            return Ok(*exp);
        }
        let (not_before, expires) = spec.verify_consent(&consent)?;
        for r in 0..=1 {
            let cert = &spec.peers[r].cert;
            let path = if r == role as usize {
                "account-authority.snapshot".to_owned()
            } else {
                format!("peer-authority/{}.snapshot", cert.account_id())
            };
            let snapshot =
                AccountAuthoritySnapshot::decode_and_verify(&admin::required(&self.store, &path)?)?;
            verify_device_authorization_with_snapshot(
                cert.account_id(),
                cert,
                &snapshot,
                &DeviceCapability::MESSAGING,
            )
            .context("AuthBlocked: current pinned authority")?;
        }
        ensure!(
            now()?.saturating_add(120) >= not_before && now()? < expires,
            "AuthBlocked: consent interval"
        );
        self.policy_cache
            .borrow_mut()
            .insert(c, (digest, not_before, expires));
        Ok(expires)
    }
    pub fn credential(&self, raw: &[u8]) -> Result<Credentials> {
        let id = wire::hash(b"", raw);
        if let Some(value) = self.credential_cache.borrow().get(&id) {
            ensure!(value.raw == raw, "credential cache hash mismatch");
            return Ok(value.clone());
        }
        let value = Credentials::parse(raw)?;
        let mut cache = self.credential_cache.borrow_mut();
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(id, value.clone());
        Ok(value)
    }
    pub fn own_credentials(&self) -> Result<Credentials> {
        let raw = wire::encode(
            &a([
                u(1),
                b(admin::required(&self.store, "device-certificate.cert")?),
                b(admin::required(&self.store, "account-authority.snapshot")?),
            ]),
            wire::MAX_AUTH,
        )?;
        self.credential(&raw)
    }
    pub fn pin_authority(&mut self, raw: &[u8]) -> Result<()> {
        let snapshot = AccountAuthoritySnapshot::decode_and_verify(raw)?;
        ensure!(snapshot.encode()? == raw, "authority exact encoding");
        let own = admin::claim(&self.store)?.1;
        ensure!(
            snapshot.account_id() == own
                || self.contexts.values().any(|s| s
                    .peers
                    .iter()
                    .any(|p| p.cert.account_id() == snapshot.account_id())),
            "unpinned authority account"
        );
        let path = if snapshot.account_id() == own {
            "account-authority.snapshot".to_owned()
        } else {
            format!("peer-authority/{}.snapshot", snapshot.account_id())
        };
        let prior = admin::required(&self.store, &path)?;
        let previous = AccountAuthoritySnapshot::decode_and_verify(&prior)?;
        ensure!(
            snapshot.revision() >= previous.revision(),
            "authority rollback"
        );
        if snapshot.revision() == previous.revision() {
            ensure!(prior == raw, "authority revision conflict");
            return Ok(());
        }
        let mut changes = Changes::default();
        changes.put(path, raw.to_vec());
        let affected = self
            .contexts
            .values()
            .filter(|s| {
                s.peers
                    .iter()
                    .any(|p| p.cert.account_id() == snapshot.account_id())
            })
            .map(|s| s.c)
            .collect::<Vec<_>>();
        for c in &affected {
            let mut head = self.head(*c)?;
            head.bump(false)?;
            changes.put(state::head_path(*c), head.encode()?);
        }
        admin::commit(&mut self.store, changes.mutations())?;
        for c in affected {
            self.leases.remove(&c);
            self.policy_cache.borrow_mut().remove(&c);
        }
        Ok(())
    }
    pub fn check_lease(&self, c: Id, binding: Id) -> Result<()> {
        self.policy(c)?;
        let lease = self
            .leases
            .get(&c)
            .context("AuthBlocked: no admitted connection")?;
        ensure!(
            lease.binding == binding && now()? < lease.deadline,
            "AuthBlocked: stale/expired connection"
        );
        Ok(())
    }
    pub fn close_lease(&mut self, c: Id, generation: u64) {
        if self
            .leases
            .get(&c)
            .is_some_and(|l| l.generation == generation)
        {
            self.leases.remove(&c);
        }
    }
    pub fn native(&self, head: &Head) -> Result<PrivateState> {
        let private = PrivateState::open(
            head,
            self.nonce,
            self.device.encryption(),
            &admin::required(&self.store, &state::native_path(head.c))?,
        )?;
        let offer = self.control(head.c, head.offer.context("missing selected Offer")?)?;
        let body = wire::control(&offer, 1, self.spec(head.c)?.peers[0].cert.device_id())?;
        let offer_fields = body.fields(8)?;
        let own_curve = if head.role == 0 {
            offer_fields[5].fixed::<32>()?
        } else {
            let grant = self.control(head.c, head.grant.context("missing Grant")?)?;
            wire::control(&grant, 2, self.spec(head.c)?.peers[1].cert.device_id())?.fields(8)?[4]
                .fixed::<32>()?
        };
        ensure!(
            private.account.curve_key() == own_curve,
            "OD1 Account/transcript mismatch"
        );
        if let Some(grant_id) = head.grant {
            let grant = self.control(head.c, grant_id)?;
            let (d, e) = handshake::epoch(self.spec(head.c)?, &offer, &grant)?;
            ensure!(
                head.d == Some(d) && head.e == Some(e),
                "OD1 selected domain mismatch"
            );
            let gb = wire::control(&grant, 2, self.spec(head.c)?.peers[1].cert.device_id())?;
            let otk = gb.fields(8)?[5].fixed::<32>()?;
            if let Some(session) = private.session.as_ref() {
                ensure!(
                    session.keys()[0] == offer_fields[5].fixed::<32>()? && session.keys()[2] == otk,
                    "OD1 selected SessionKeys"
                );
            }
            if let Some(reservation) = private.reservation {
                ensure!(reservation.2 == otk, "OD1 reserved OTK binding");
            }
        }
        Ok(private)
    }
    pub fn control(&self, c: Id, id: Id) -> Result<Vec<u8>> {
        let raw = admin::required(&self.store, &control_path(c, id))?;
        ensure!(
            wire::hash(wire::CONTROL_ID, &raw) == id,
            "OD1 control ID mismatch"
        );
        Ok(raw)
    }
    pub fn event_at(&self, c: Id, role: u64, n: u64) -> Result<Option<Event>> {
        let Some(id) = self.store.get(&ordinal_path(c, role, n))? else {
            return Ok(None);
        };
        let id: Id = id.as_slice().try_into().context("OD1 ordinal ID length")?;
        let raw = admin::required(&self.store, &event_path(c, id))?;
        let event = Event::parse(&raw, self.spec(c)?.peers[role as usize].cert.device_id())?;
        ensure!(
            event.id()? == id && event.core.c == c && event.core.role == role && event.core.n == n,
            "OD1 ledger index mismatch"
        );
        Ok(Some(event))
    }
    pub fn active(&self, c: Id) -> Result<bool> {
        Ok(self.head(c)?.phase == 3
            && self.store.get(&ready_path(c, 0))?.is_some()
            && self.store.get(&ready_path(c, 1))?.is_some())
    }
    pub fn save(
        &mut self,
        head: &mut Head,
        private: Option<&PrivateState>,
        mut changes: Changes,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        let native_changed = private.is_some();
        if let Some(private) = private {
            let raw = private.seal(head, self.nonce, self.device.encryption().public_key())?;
            changes.put(state::native_path(head.c), raw);
            self.policy(head.c)?;
            let lease = self
                .leases
                .get(&head.c)
                .context("AuthBlocked: no admitted native writer")?;
            ensure!(
                now()? < lease.deadline,
                "AuthBlocked: lease expired before native commit"
            );
        }
        changes.put(state::head_path(head.c), head.encode()?);
        let receipt = self
            .store
            .commit(self.store.generation(), changes.mutations())?;
        if !receipt.visible {
            self.store
                .recover()
                .context("committed primary requires shadow recovery")?;
        }
        super::test_support::timing(
            head.c,
            started.elapsed(),
            receipt.encoded_delta_bytes,
            receipt.generation,
            native_changed,
        );
        Ok(())
    }
    pub(super) fn sequence(&self, changes: &mut Changes) -> Result<u64> {
        let current = match self.store.get("next-sequence")? {
            Some(v) => std::str::from_utf8(&v)?.trim().parse::<u64>()?,
            None => 0,
        };
        changes.put(
            "next-sequence".to_owned(),
            format!(
                "{}\n",
                current
                    .checked_add(1)
                    .context("device sequence exhausted")?
            )
            .into_bytes(),
        );
        Ok(current)
    }
    pub fn add_auth(&self, changes: &mut Changes, raw: &[u8]) -> Result<Id> {
        let auth = self.credential(raw)?;
        let id = auth.reference();
        changes.put(admin::auth_path(id), raw.to_vec());
        Ok(id)
    }
    pub fn initial_auth(&self, c: Id, role: u64) -> Result<Vec<u8>> {
        let pair = wire::decode(
            &admin::required(&self.store, &format!("{}initial-pair", prefix(c)))?,
            192 * 1024,
        )?;
        let frame = wire::Frame::parse(pair.fields(6)?[2 + role as usize].bytes()?)?;
        Ok(frame.body.fields(11)?[7].bytes()?.to_vec())
    }
    pub fn record_event(&self, event: &Event, changes: &mut Changes) -> Result<()> {
        let id = event.id()?;
        let c = event.core.c;
        changes.put(event_path(c, id), event.raw()?);
        changes.put(ordinal_path(c, event.core.role, event.core.n), id.to_vec());
        let signer = self.spec(c)?.peers[event.core.role as usize]
            .cert
            .device_id();
        let path = format!(
            "runtime/od1/authored/{}/{:020}",
            signer, event.core.sequence
        );
        if let Some(old) = self.store.get(&path)? {
            ensure!(old == id, "author_sequence fork");
        }
        changes.put(path, id.to_vec());
        Ok(())
    }
    pub fn encrypt(
        &self,
        head: &Head,
        private: &mut PrivateState,
        message: Outgoing,
    ) -> Result<Event> {
        let Outgoing {
            kind,
            request,
            payload,
            auth,
            sequence,
        } = message;
        let guard = private.guard.as_ref().context("OD1 selected guard")?;
        let mut parents = Vec::new();
        if kind == 3 {
            if guard.own.next > 1 {
                parents.extend(guard.own.last);
            }
            if guard.peer.next > 1 {
                parents.extend(guard.peer.last);
            }
            parents.sort();
            parents.dedup();
        }
        let core = wire::Core {
            c: head.c,
            d: head.d.context("domain")?,
            e: head.e.context("epoch")?,
            sid: head.sid.clone().context("SID")?,
            role: head.role,
            n: guard.own.next,
            previous: guard.own.last,
            sequence,
            kind,
            request,
            parents,
            expected: guard.expected_send()?,
            author_auth: auth,
        };
        let raw = Event::inner(&core, payload)?;
        let packet = private
            .session
            .as_mut()
            .context("native session")?
            .encrypt(&raw)?;
        let header = packet.header()?;
        let mut event = Event {
            core,
            native_kind: u64::from(packet.kind),
            key: header.key,
            index: header.index,
            ciphertext: packet.bytes,
            signature: Vec::new(),
        };
        event.sign(self.device.identity())?;
        private
            .guard
            .as_mut()
            .context("guard")?
            .commit_send(&event, event.id()?)?;
        Ok(event)
    }
    pub fn ensure_offer(&mut self, c: Id, pair: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A offers");
        let mut head = self.head(c)?;
        if let Some(id) = head.offer {
            return self.control(c, id);
        }
        let consent = admin::required(&self.store, &admin::consent_path(c))?;
        let t = handshake::verify_initial_pair(
            self.spec(c)?,
            pair,
            Some(wire::hash(handshake::CONSENT, &consent)),
        )?;
        let (private, _) = PrivateState::fresh(false)?;
        let refs = self.spec(c)?.cert_refs()?;
        let offer = wire::signed_control(
            1,
            a([
                b(c),
                b(refs[0]),
                b(refs[1]),
                b(t),
                b(admin::random::<32>()?),
                b(private.account.curve_key()),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?;
        let id = wire::hash(wire::CONTROL_ID, &offer);
        head.phase = 1;
        head.offer = Some(id);
        head.bump(true)?;
        let mut changes = Changes::default();
        changes.put(control_path(c, id), offer.clone());
        changes.put(format!("{}initial-pair", prefix(c)), pair.to_vec());
        self.save(&mut head, Some(&private), changes)?;
        super::test_support::checkpoint("after-offer")?;
        Ok(offer)
    }
    pub fn grant(&mut self, c: Id, offer: &[u8], pair: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 1, "only B grants");
        let mut head = self.head(c)?;
        let offer_id = wire::hash(wire::CONTROL_ID, offer);
        let t = handshake::verify_initial_pair(self.spec(c)?, pair, None)?;
        let oa = wire::control(offer, 1, self.spec(c)?.peers[0].cert.device_id())?;
        let oa = oa.fields(8)?;
        let refs = self.spec(c)?.cert_refs()?;
        ensure!(
            oa[0].fixed::<32>()? == c
                && oa[1].fixed::<32>()? == refs[0]
                && oa[2].fixed::<32>()? == refs[1]
                && oa[3].fixed::<32>()? == t
                && oa[6].uint()? == 1
                && oa[7].uint()? == 1,
            "Offer scope/initial transcript"
        );
        if let Some(id) = head.offer {
            if id != offer_id {
                let old = self.control(c, id)?;
                self.signed_fork(c, "Offer", &old, offer)?;
            }
            return self.control(c, head.grant.context("Grant absent")?);
        }
        let (mut private, otk) = PrivateState::fresh(true)?;
        let otk = otk.context("reserved OTK")?;
        let grant = wire::signed_control(
            2,
            a([
                b(c),
                b(offer_id),
                b(t),
                b(admin::random::<32>()?),
                b(private.account.curve_key()),
                b(otk),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?;
        let id = wire::hash(wire::CONTROL_ID, &grant);
        let (d, e) = handshake::epoch(self.spec(c)?, offer, &grant)?;
        private.reservation = Some((offer_id, id, otk, 0));
        head.phase = 2;
        head.offer = Some(offer_id);
        head.grant = Some(id);
        head.d = Some(d);
        head.e = Some(e);
        head.bump(true)?;
        let mut changes = Changes::default();
        changes.put(control_path(c, offer_id), offer.to_vec());
        changes.put(control_path(c, id), grant.clone());
        changes.put(format!("{}initial-pair", prefix(c)), pair.to_vec());
        self.save(&mut head, Some(&private), changes)?;
        super::test_support::checkpoint("after-grant")?;
        Ok(grant)
    }
    pub fn init(&mut self, c: Id, grant: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A selects outbound");
        let mut head = self.head(c)?;
        let grant_id = wire::hash(wire::CONTROL_ID, grant);
        let offer_id = head.offer.context("Offer absent")?;
        let offer = self.control(c, offer_id)?;
        let (d, e) = handshake::epoch(self.spec(c)?, &offer, grant)?;
        if let Some(id) = head.grant {
            if id != grant_id {
                let old = self.control(c, id)?;
                self.signed_fork(c, "Grant", &old, grant)?;
            }
            return self
                .event_at(c, 0, 0)?
                .context("durable Init missing")?
                .raw()
                .map_err(Into::into);
        }
        let gb = wire::control(grant, 2, self.spec(c)?.peers[1].cert.device_id())?;
        let mut private = self.native(&head)?;
        private.session = Some(
            private
                .account
                .outbound(gb.fields(8)?[4].fixed()?, gb.fields(8)?[5].fixed()?)?,
        );
        private.guard = Some(Guard::new(0));
        head.phase = 3;
        head.grant = Some(grant_id);
        head.d = Some(d);
        head.e = Some(e);
        head.sid = Some(private.session.as_ref().context("session")?.sid());
        head.bump(true)?;
        let pair = admin::required(&self.store, &format!("{}initial-pair", prefix(c)))?;
        let t = handshake::verify_initial_pair(self.spec(c)?, &pair, None)?;
        let mut changes = Changes::default();
        let auth = self.add_auth(&mut changes, &self.initial_auth(c, 0)?)?;
        let keys = private.session.as_ref().context("session")?.keys();
        let seq = self.sequence(&mut changes)?;
        let mut request = [0; 16];
        request.copy_from_slice(&offer_id[..16]);
        let event = self.encrypt(
            &head,
            &mut private,
            Outgoing {
                kind: 1,
                request,
                payload: a([b(offer_id), b(grant_id), b(t), a(keys.iter().map(b))]),
                auth,
                sequence: seq,
            },
        )?;
        self.record_event(&event, &mut changes)?;
        changes.put(control_path(c, grant_id), grant.to_vec());
        super::test_support::checkpoint("before-init-commit")?;
        self.save(&mut head, Some(&private), changes)?;
        super::test_support::checkpoint("after-init-commit")?;
        Ok(event.raw()?)
    }
    pub fn confirm(&mut self, c: Id, init: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 1, "only B accepts prekey");
        let mut head = self.head(c)?;
        let event = Event::parse(init, self.spec(c)?.peers[0].cert.device_id())?;
        self.check_scope(&head, &event, false)?;
        ensure!(
            event.core.kind == 1
                && event.core.n == 0
                && event.core.request.as_slice() == &head.offer.context("Offer")?[..16]
                && event.core.parents.is_empty(),
            "Init genesis kind/ordinal/request/parents"
        );
        if head.phase == 3 {
            let old = self.event_at(c, 0, 0)?.context("A0")?;
            if old.id()? != event.id()? {
                self.signed_fork(c, "Init", &old.raw()?, init)?;
            }
            return Ok(self.event_at(c, 1, 0)?.context("B0")?.raw()?);
        }
        ensure!(head.phase == 2, "Grant required");
        let mut private = self.native(&head)?;
        let offer_id = head.offer.context("Offer")?;
        let grant_id = head.grant.context("Grant")?;
        let offer = self.control(c, offer_id)?;
        let offer_body = wire::control(&offer, 1, self.spec(c)?.peers[0].cert.device_id())?;
        let packet = Packet {
            kind: 0,
            bytes: event.ciphertext.clone(),
        };
        let header = packet.header()?;
        let keys = header.prekey_keys.context("Init public tuple")?;
        ensure!(
            keys[0] == offer_body.fields(8)?[5].fixed::<32>()?
                && keys[2] == private.reservation.context("OTK")?.2,
            "Init prekey bindings"
        );
        let mut guard = Guard::new(1);
        guard.check_receive(&event)?;
        let (session, plain) = private.account.inbound(keys[0], &packet)?;
        ensure!(
            session.sid() == event.core.sid && session.keys() == keys,
            "Init native SID/keys"
        );
        let inner = event.verify_inner(&plain)?;
        let f = inner.fields(4)?;
        ensure!(
            f[0].fixed::<32>()? == offer_id
                && f[1].fixed::<32>()? == grant_id
                && f[2] == offer_body.fields(8)?[3]
                && f[3] == a(keys.iter().map(b)),
            "Init inner transcript"
        );
        let mut changes = Changes::default();
        let author_auth = self.add_auth(&mut changes, &self.initial_auth(c, 0)?)?;
        ensure!(event.core.author_auth == author_auth, "Init auth reference");
        self.record_event(&event, &mut changes)?;
        guard.commit_receive(&event, event.id()?)?;
        private.session = Some(session);
        private.guard = Some(guard);
        private.reservation.as_mut().context("OTK")?.3 = 1;
        head.phase = 3;
        head.sid = Some(event.core.sid.clone());
        head.bump(true)?;
        let auth = self.add_auth(&mut changes, &self.initial_auth(c, 1)?)?;
        let seq = self.sequence(&mut changes)?;
        let mut request = [0; 16];
        request.copy_from_slice(&grant_id[..16]);
        let b0 = self.encrypt(
            &head,
            &mut private,
            Outgoing {
                kind: 2,
                request,
                payload: a([
                    b(offer_id),
                    b(grant_id),
                    b(event.core.sid.as_bytes()),
                    b(event.id()?),
                ]),
                auth,
                sequence: seq,
            },
        )?;
        self.record_event(&b0, &mut changes)?;
        let ready = self.make_ready(&head, event.id()?, b0.id()?)?;
        changes.put(ready_path(c, 1), ready);
        super::test_support::checkpoint("before-confirm-commit")?;
        self.save(&mut head, Some(&private), changes)?;
        super::test_support::checkpoint("after-confirm-commit")?;
        Ok(b0.raw()?)
    }
    pub fn receive_confirm(&mut self, c: Id, raw: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A consumes Confirm");
        let mut head = self.head(c)?;
        let event = Event::parse(raw, self.spec(c)?.peers[1].cert.device_id())?;
        self.check_scope(&head, &event, true)?;
        ensure!(
            event.core.kind == 2
                && event.core.n == 0
                && event.core.request.as_slice() == &head.grant.context("Grant")?[..16]
                && event.core.parents.is_empty(),
            "Confirm genesis kind/ordinal/request/parents"
        );
        if let Some(old) = self.event_at(c, 1, 0)? {
            if old.id()? != event.id()? {
                self.signed_fork(c, "Confirm", &old.raw()?, raw)?;
            }
            return admin::required(&self.store, &ready_path(c, 0));
        }
        let mut private = self.native(&head)?;
        private
            .guard
            .as_ref()
            .context("guard")?
            .check_receive(&event)?;
        let plaintext = private
            .session
            .as_mut()
            .context("session")?
            .decrypt(&Packet {
                kind: 1,
                bytes: event.ciphertext.clone(),
            })?;
        let inner = event.verify_inner(&plaintext)?;
        let f = inner.fields(4)?;
        let a0 = self.event_at(c, 0, 0)?.context("A0")?;
        ensure!(
            f[0].fixed::<32>()? == head.offer.context("Offer")?
                && f[1].fixed::<32>()? == head.grant.context("Grant")?
                && f[2].text()? == head.sid.as_deref().context("SID")?
                && f[3].fixed::<32>()? == a0.id()?,
            "Confirm inner transcript"
        );
        let mut changes = Changes::default();
        let auth = self.add_auth(&mut changes, &self.initial_auth(c, 1)?)?;
        ensure!(event.core.author_auth == auth, "Confirm auth reference");
        self.record_event(&event, &mut changes)?;
        private
            .guard
            .as_mut()
            .context("guard")?
            .commit_receive(&event, event.id()?)?;
        let ready = self.make_ready(&head, a0.id()?, event.id()?)?;
        changes.put(ready_path(c, 0), ready.clone());
        head.bump(true)?;
        self.save(&mut head, Some(&private), changes)?;
        super::test_support::checkpoint("after-receive-confirm")?;
        Ok(ready)
    }
    fn make_ready(&self, head: &Head, a0: Id, b0: Id) -> Result<Vec<u8>> {
        Ok(wire::signed_control(
            3,
            a([
                b(head.d.context("domain")?),
                b(head.e.context("epoch")?),
                b(head.sid.as_deref().context("SID")?.as_bytes()),
                b(a0),
                b(b0),
                u(head.role),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?)
    }
    pub fn ready(&mut self, c: Id, raw: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        let mut head = self.head(c)?;
        ensure!(head.phase == 3, "NeedHandshake");
        let peer = 1 - head.role;
        let body = wire::control(raw, 3, self.spec(c)?.peers[peer as usize].cert.device_id())?;
        let a0 = self.event_at(c, 0, 0)?.context("NeedHandshake: A0")?;
        let b0 = self.event_at(c, 1, 0)?.context("NeedHandshake: B0")?;
        let f = body.fields(8)?;
        ensure!(
            Some(f[0].fixed::<32>()?) == head.d
                && Some(f[1].fixed::<32>()?) == head.e
                && f[2].text()? == head.sid.as_deref().context("SID")?
                && f[5].uint()? == peer
                && f[6].uint()? == 1
                && f[7].uint()? == 1,
            "Ready scope/role/suite"
        );
        if f[3].fixed::<32>()? != a0.id()? || f[4].fixed::<32>()? != b0.id()? {
            let prior = self
                .store
                .get(&ready_path(c, peer))?
                .unwrap_or(if peer == 0 { a0.raw()? } else { b0.raw()? });
            self.signed_fork(c, "Ready genesis", &prior, raw)?;
        }
        ensure!(
            body == a([
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                b(head.sid.as_deref().context("SID")?.as_bytes()),
                b(a0.id()?),
                b(b0.id()?),
                u(peer),
                u(1),
                u(1)
            ]),
            "Ready bindings"
        );
        if let Some(old) = self.store.get(&ready_path(c, peer))? {
            if old != raw {
                self.signed_fork(c, "Ready", &old, raw)?;
            }
        } else {
            let mut changes = Changes::default();
            changes.put(ready_path(c, peer), raw.to_vec());
            head.bump(false)?;
            self.save(&mut head, None, changes)?;
        }
        super::test_support::checkpoint("after-ready")?;
        admin::required(&self.store, &ready_path(c, head.role))
    }
    pub fn check_scope(&self, head: &Head, event: &Event, selected: bool) -> Result<()> {
        if !(event.core.c == head.c
            && head.d == Some(event.core.d)
            && head.e == Some(event.core.e)
            && (!selected || head.sid.as_ref() == Some(&event.core.sid)))
        {
            return Err(wire::Error::Invalid("event context/domain/epoch/SID binding").into());
        }
        Ok(())
    }
    pub fn signed_fork(&mut self, c: Id, reason: &str, old: &[u8], new: &[u8]) -> Result<()> {
        let path = format!("{}blocked-fork", prefix(c));
        if self.store.get(&path)?.is_none() {
            ensure!(
                old.len() <= 32 * 1024 && new.len() <= 32 * 1024 && reason.len() <= 128,
                "fork proof bound"
            );
            let mut changes = Changes::default();
            changes.put(
                path,
                wire::encode(&a([u(1), b(reason.as_bytes()), b(old), b(new)]), 68 * 1024)?,
            );
            let mut head = self.head(c)?;
            head.bump(false)?;
            self.save(&mut head, None, changes)?;
        }
        anyhow::bail!("Blocked: signed {reason} fork; original ciphertext retained")
    }
    pub fn find_event_raw(&self, id: Id) -> Result<Vec<u8>> {
        for c in self.contexts.keys() {
            if let Some(raw) = self.store.get(&event_path(*c, id))? {
                return Ok(raw);
            }
            for path in self.store.paths(&format!("{}pending-state/", prefix(*c))) {
                let state = wire::decode(&admin::required(&self.store, &path)?, 2048)?;
                if state.fields(8)?[1].fixed::<32>()? == id {
                    let n = path
                        .rsplit('/')
                        .next()
                        .context("pending proof ordinal")?
                        .parse()?;
                    return admin::required(&self.store, &pending_path(*c, n));
                }
            }
        }
        anyhow::bail!("operational: indexed author event missing")
    }
}
