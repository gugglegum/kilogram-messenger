//! Read-only Mixed7 checkpoint. Original authenticated scope is required on the reader API.
use super::*;
use kilogram_crypto::{DeviceEncryptionIdentity, SealedMessage};
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession},
    ordered_v2::Guard2,
};
#[cfg(test)]
#[path = "migration_s54_native_tests.rs"]
mod tests;
const INFO: &[u8] = b"kg:od2:migrated-private\0";
struct Scope {
    origin: old::Id,
    c: old::Id,
    d: old::Id,
    e: old::Id,
    sid: String,
    certs: [old::Id; 2],
    offer: old::Id,
    grant: old::Id,
    curves: [old::Id; 2],
    otk: old::Id,
}
impl Scope {
    fn verified(origin: &v2::VerifiedExistingScope1) -> Result<Self> {
        let value = old::decode(origin.raw(), v2::MAX_EXISTING_ORIGIN)?;
        let f = value.fields(7)?;
        let offer = old::control(
            f[2].bytes()?,
            1,
            origin.peers()[0].certificate().device_id(),
        )?;
        let grant = old::control(
            f[3].bytes()?,
            2,
            origin.peers()[1].certificate().device_id(),
        )?;
        let ids = origin.old_control_ids();
        ensure!(
            old::hash(old::CONTROL_ID, f[2].bytes()?) == ids[0]
                && old::hash(old::CONTROL_ID, f[3].bytes()?) == ids[1],
            "actual original authenticated Native transcript refs"
        );
        Ok(Self {
            origin: digest(origin.raw()),
            c: origin.c(),
            d: origin.d(),
            e: origin.e(),
            sid: origin.sid().into(),
            certs: origin.certs(),
            offer: ids[0],
            grant: ids[1],
            curves: [offer.fields(8)?[5].fixed()?, grant.fields(8)?[4].fixed()?],
            otk: grant.fields(8)?[5].fixed()?,
        })
    }
}
pub(super) struct Input<'a> {
    pub material: &'a store::L1OwnerMaterial,
    pub source: old::Id,
    pub head: &'a Head,
    pub origin: &'a v2::VerifiedExistingScope1,
    pub selected: &'a v2::StaticSelection2,
    pub snapshot: &'a store::L1Snapshot,
    pub meta: &'a store::L1Metadata,
    pub graph: &'a graph::Context,
    pub original: &'a source::Legacy,
    pub migration: &'a source::Legacy,
    pub context: usize,
    pub live: &'a Value,
}
#[derive(Clone, Debug)]
pub(in super::super) struct Checkpoint {
    origin: old::Id,
    curve: old::Id,
    scope: old::Id,
    content: old::Id,
    account: old::Id,
    session: old::Id,
    guard: old::Id,
    reservation: old::Id,
    public_identity: old::Id,
    context: old::Id,
    role: u8,
    next: [u64; 2],
    tails: [Option<old::Id>; 2],
    pub seal: old::Id,
    pub native_generation: u64,
}
impl Checkpoint {
    pub fn lineage(
        &self,
        index: u8,
        slot: u8,
        source: old::Id,
        origin: &v2::VerifiedExistingScope1,
        graph: &graph::Context,
    ) -> Result<accounting::Lineage> {
        let scope = Scope::verified(origin)?;
        let ids = origin.old_control_ids();
        ensure!(
            self.context == origin.c()
                && self.origin == scope.origin
                && self.role < 2
                && self.curve == scope.curves[self.role as usize]
                && graph.context == self.context
                && graph.genesis == [ids[2], ids[3]],
            "same restored original account/session and authenticated genesis lineage"
        );
        Ok(accounting::Lineage {
            owner: self.role,
            source,
            index,
            context: self.context,
            slot,
            scope: self.origin,
            graph_scope: graph.scope_commitment,
            offer: ids[0],
            grant: ids[1],
            genesis: [ids[2], ids[3]],
            curve: self.curve,
            native_public: self.public_identity,
        })
    }
    pub fn same_content(&self, other: &Self) -> Result<()> {
        ensure!(
            self.scope == other.scope
                && self.origin == other.origin
                && self.curve == other.curve
                && self.content == other.content
                && self.account == other.account
                && self.session == other.session
                && self.guard == other.guard
                && self.reservation == other.reservation
                && self.public_identity == other.public_identity
                && self.context == other.context
                && self.role == other.role
                && self.next == other.next
                && self.tails == other.tails,
            "same owner/context full stored Native content, excluding random outer seal/generation"
        );
        Ok(())
    }
}
fn measured<T>(check: impl FnOnce() -> Result<T>) -> Result<T> {
    let before = kilogram_ratchet::ordered::native_call_counts();
    let result = check();
    ensure!(
        kilogram_ratchet::ordered::native_call_counts() == before,
        "Native checkpoint success/error path has zero constructors/packet calls"
    );
    result
}
fn public_identity(live: &Value, sid: &str, keys: &[old::Id; 3]) -> Result<()> {
    let saved = live["keys"]
        .as_array()
        .context("retained live session public keys")?;
    ensure!(
        live["sid"] == sid
            && saved.len() == 3
            && keys
                .iter()
                .zip(saved)
                .all(|(key, value)| *value == state::hex(key)),
        "retained actual live SID/all three session keys"
    );
    Ok(())
}
pub(super) fn read(input: Input<'_>) -> Result<Checkpoint> {
    measured(|| {
        s53_deadline::check()?;
        let i = input;
        let scope = Scope::verified(i.origin)?;
        ensure!(
            i.selected.c() == scope.c
                && i.selected.d() == scope.d
                && i.selected.e() == scope.e
                && i.selected.sid() == Some(scope.sid.as_str())
                && i.selected.proof().certs() == scope.certs,
            "same A5 authenticated selected Native scope"
        );
        ensure!(
            i.snapshot.head == i.head.encode()?
                && serde_json::to_vec(&i.snapshot.meta)? == serde_json::to_vec(i.meta)?
                && i.meta.next == i.graph.next
                && i.meta.tails == i.graph.tails.map(Some),
            "same verified snapshot Head/metadata and A4 derived terminal lanes"
        );
        let binding = Binding {
            material: i.material,
            source: i.source,
            head: i.head,
            q: i.selected.proof().q(),
            scope: &scope,
            meta: i.meta,
            raw: i
                .snapshot
                .native
                .as_deref()
                .context("actual current Native")?,
        };
        open(&binding, |sid, keys| {
            i.original.terminal_native(i.context, sid, keys)?;
            i.migration.terminal_native(i.context, sid, keys)?;
            public_identity(i.live, sid, keys)
        })
    })
}
struct Binding<'a> {
    material: &'a store::L1OwnerMaterial,
    source: old::Id,
    head: &'a Head,
    q: old::Id,
    scope: &'a Scope,
    meta: &'a store::L1Metadata,
    raw: &'a [u8],
}
fn aad(i: &Binding<'_>) -> Result<Vec<u8>> {
    let h = i.head;
    ensure!(
        h.phase == 3
            && h.role < 2
            && h.c == i.scope.c
            && h.local_cert == i.scope.certs[h.role as usize]
            && h.offer == Some(i.scope.offer)
            && h.grant == Some(i.scope.grant)
            && h.d == Some(i.scope.d)
            && h.e == Some(i.scope.e)
            && h.sid.as_deref() == Some(i.scope.sid.as_str()),
        "current Head and original authenticated transcript/scope"
    );
    h.validate()?;
    Ok(old::encode(
        &old::a([
            old::u(7),
            old::b(i.material.incarnation),
            old::b(i.material.nonce),
            old::b(i.source),
            old::b(h.c),
            old::b(h.local_cert),
            old::u(h.role),
            old::u(h.phase),
            old::u(h.native_generation),
            old::b(i.scope.offer),
            old::b(i.scope.grant),
            old::b(i.scope.d),
            old::b(i.scope.e),
            old::b(i.scope.sid.as_bytes()),
            old::b(i.q),
        ]),
        2048,
    )?)
}
fn frontiers(guard: &Guard2, role: u8, meta: &store::L1Metadata) -> Result<()> {
    ensure!(role < 2 && guard.role() == role, "actual guard owner role");
    let mut next = [0; 2];
    let mut tails = [None; 2];
    next[role as usize] = guard.own().next;
    next[1 - role as usize] = guard.peer().next;
    tails[role as usize] = guard.own().last;
    tails[1 - role as usize] = guard.peer().last;
    ensure!(
        meta.next == next && meta.tails == tails,
        "role-relative Native guard equals actual role-indexed stored lanes"
    );
    Ok(())
}
fn reservation(scope: &Scope, role: u8, value: &old::Value, keys: &[old::Id; 3]) -> Result<()> {
    let tuple = value
        .option()?
        .map(|v| -> Result<_> {
            let f = v.fields(4)?;
            Ok((
                f[0].fixed::<32>()?,
                f[1].fixed::<32>()?,
                f[2].fixed::<32>()?,
                f[3].uint()?,
            ))
        })
        .transpose()?;
    ensure!(
        keys[0] == scope.curves[0] && keys[2] == scope.otk,
        "actual session identity/OTK belongs to original transcript"
    );
    ensure!(
        match role {
            0 => tuple.is_none(),
            1 => tuple == Some((scope.offer, scope.grant, scope.otk, 1)),
            _ => false,
        },
        "A none/B original consumed Offer/Grant/OTK tuple1, not outstanding debt"
    );
    Ok(())
}
fn open(
    i: &Binding<'_>,
    identity: impl FnOnce(&str, &[old::Id; 3]) -> Result<()>,
) -> Result<Checkpoint> {
    measured(|| {
        s53_deadline::check()?;
        ensure!(
            i.raw.len() <= state::MAX_NATIVE
                && i.head.blob == Some(old::hash(store::MIXED_NATIVE_ID, i.raw)),
            "bounded actual Native envelope/pointer"
        );
        let envelope = old::decode(i.raw, state::MAX_NATIVE)?;
        let f = envelope.fields(5)?;
        ensure!(
            f[0].uint()? == 7 && f[1].uint()? == 3 && f[2].uint()? == i.head.native_generation,
            "Mixed7 envelope discriminator/native generation"
        );
        let associated = aad(i)?;
        let device = DeviceEncryptionIdentity::from_secret_bytes(*i.material.encryption);
        let plain = Zeroizing::new(device.open(
            &SealedMessage {
                encapsulated_key: f[3].bytes()?.to_vec(),
                ciphertext: f[4].bytes()?.to_vec(),
            },
            INFO,
            &associated,
        )?);
        let value = Zeroizing::new(old::decode(&plain, state::MAX_NATIVE)?);
        let p = value.fields(7)?;
        ensure!(
            p[0].uint()? == 7 && p[1].uint()? == 3,
            "Mixed7 authenticated private discriminator"
        );
        let key = Zeroizing::new(p[2].fixed::<32>()?);
        let account = OrderedAccount::restore(p[3].bytes()?, &key)?;
        let session = OrderedSession::restore(p[4].bytes()?, &key)?;
        let sid = session.sid();
        let keys = session.keys();
        let role = u8::try_from(i.head.role)?;
        ensure!(
            sid == i.scope.sid && account.curve_key() == i.scope.curves[role as usize],
            "restored original SID/account transcript identity"
        );
        reservation(i.scope, role, &p[6], &keys)?;
        identity(&sid, &keys)?;
        let guard = Guard2::restore(p[5].bytes()?, role)?;
        frontiers(&guard, role, i.meta)?;
        s53_deadline::check()?;
        // Hash actual stored encrypted pickle bytes and complete authenticated payload;
        // never snapshot/re-pickle restored objects to construct equality evidence.
        let scope = old::hash(
            b"kg:s54:native-scope:v1\0",
            &old::encode(
                &old::a([
                    old::b(i.scope.origin),
                    old::b(i.material.incarnation),
                    old::b(i.material.nonce),
                    old::b(i.source),
                    old::b(i.head.c),
                    old::b(i.head.local_cert),
                    old::u(i.head.role),
                    old::b(i.q),
                ]),
                1024,
            )?,
        );
        Ok(Checkpoint {
            origin: i.scope.origin,
            curve: account.curve_key(),
            scope,
            content: old::hash(b"kg:s54:native-payload:v1\0", &plain),
            account: digest(p[3].bytes()?),
            session: digest(p[4].bytes()?),
            guard: digest(p[5].bytes()?),
            reservation: digest(&old::encode(&p[6], 1024)?),
            public_identity: digest(&old::encode(
                &old::a([
                    old::b(sid.as_bytes()),
                    old::b(account.curve_key()),
                    old::a(keys.iter().map(old::b)),
                ]),
                1024,
            )?),
            context: i.head.c,
            role,
            next: i.meta.next,
            tails: i.meta.tails,
            seal: digest(i.raw),
            native_generation: i.head.native_generation,
        })
    })
}
