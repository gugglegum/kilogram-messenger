//! Ephemeral in-memory crypto setup only. Synthetic guard/identity binding, no real origin export.
use super::*;
use kilogram_ratchet::ordered_v2::{self as g2, Lane};
pub(super) struct Fixture {
    pub material: store::L1OwnerMaterial,
    pub source: old::Id,
    pub scope: Scope,
    pub q: old::Id,
    pub heads: [Head; 2],
    pub metas: [store::L1Metadata; 2],
    pub payloads: [Zeroizing<old::Value>; 2],
    pub keys: [old::Id; 3],
    pub accounts: [OrderedAccount; 2],
    pub sessions: [OrderedSession; 2],
}
fn lane(role: u8, next: u64) -> Lane {
    Lane {
        next,
        last: Some([40 + role; 32]),
        g: 0,
        key: Some([42 + role; 32]),
        index: next - 1,
        first: Some([44 + role; 32]),
        parent: None,
        sequence: Some(next - 1),
    }
}
pub(super) fn guard(role: u8) -> Result<Vec<u8>> {
    let lanes = [lane(0, 6), lane(1, 4)];
    // g0 turns are A0=0/B0=1. A has received B's first and must rotate;
    // B's own first follows A's, so B is active without a rotate cause.
    g2::compact_l1_checkpoint(g2::L1CheckpointFacts {
        role,
        own: lanes[role as usize].clone(),
        peer: lanes[1 - role as usize].clone(),
        rotate_due: role == 0,
        received_new_first: lanes[1 - role as usize].first,
        used_own_first: if role == 0 { lanes[0].first } else { None },
    })
    .map_err(Into::into)
}
impl Fixture {
    pub fn new() -> Result<Self> {
        // Constructors and minimal two-packet handshake are setup, outside the
        // measured helper bracket. No profile/file/network/1344-message replay.
        let (a, _) = OrderedAccount::fresh(false);
        let (mut b, otk) = OrderedAccount::fresh(true);
        let otk = otk.context("ephemeral setup OTK")?;
        let mut sa = a.outbound(b.curve_key(), otk)?;
        let initial = sa.encrypt(b"synthetic-init")?;
        let (mut sb, plain) = b.inbound(a.curve_key(), &initial)?;
        ensure!(plain == b"synthetic-init", "minimal setup plaintext");
        let reply = sb.encrypt(b"synthetic-confirm")?;
        ensure!(
            sa.decrypt(&reply)? == b"synthetic-confirm",
            "minimal setup reply"
        );
        let keys = sa.keys();
        ensure!(keys == sb.keys(), "all three setup session keys");
        let sid = sa.sid();
        let device = DeviceEncryptionIdentity::generate()?;
        let material = store::L1OwnerMaterial {
            account: [1; 32],
            nonce: [2; 32],
            incarnation: [3; 32],
            signing: Zeroizing::new([4; 32]),
            encryption: Zeroizing::new(device.secret_bytes()),
            certificate: vec![],
        };
        let scope = Scope {
            origin: [5; 32],
            c: [6; 32],
            d: [7; 32],
            e: [8; 32],
            sid: sid.clone(),
            certs: [[9; 32], [10; 32]],
            offer: [11; 32],
            grant: [12; 32],
            curves: [a.curve_key(), b.curve_key()],
            otk,
        };
        let heads = std::array::from_fn(|r| Head {
            c: scope.c,
            local_cert: scope.certs[r],
            role: r as u64,
            generation: 19,
            phase: 3,
            native_generation: 17,
            offer: Some(scope.offer),
            grant: Some(scope.grant),
            d: Some(scope.d),
            e: Some(scope.e),
            sid: Some(sid.clone()),
            blob: Some([0; 32]),
        });
        let metas = std::array::from_fn(|_| store::L1Metadata {
            policy_generation: 1,
            wall_high_water: 1,
            next: [6, 4],
            tails: [Some([40; 32]), Some([41; 32])],
            next_queue: 1,
            request_index_after: None,
            request_index_ready: true,
            ack_next: 1,
            peer_source_next: 1,
            peer_source_last: None,
        });
        let accounts = [a, b];
        let sessions = [sa, sb];
        let pickle = Zeroizing::new([13; 32]);
        let mut payloads = Vec::new();
        for r in 0..2 {
            payloads.push(Zeroizing::new(old::a([
                old::u(7),
                old::u(3),
                old::b(&pickle[..]),
                old::b(accounts[r].snapshot(&pickle)),
                old::b(sessions[r].snapshot(&pickle)),
                old::b(guard(r as u8)?),
                old::opt((r == 1).then(|| {
                    old::a([
                        old::b(scope.offer),
                        old::b(scope.grant),
                        old::b(otk),
                        old::u(1),
                    ])
                })),
            ])));
        }
        Ok(Self {
            material,
            source: [14; 32],
            scope,
            q: [15; 32],
            heads,
            metas,
            payloads: payloads
                .try_into()
                .map_err(|_| anyhow::anyhow!("two setup payloads"))?,
            keys,
            accounts,
            sessions,
        })
    }
    pub fn seal_plain(&self, role: usize, head: &mut Head, plain: &[u8]) -> Result<Vec<u8>> {
        let binding = Binding {
            material: &self.material,
            source: self.source,
            head,
            q: self.q,
            scope: &self.scope,
            meta: &self.metas[role],
            raw: &[],
        };
        let device = DeviceEncryptionIdentity::from_secret_bytes(*self.material.encryption);
        let sealed = device.public_key().seal(plain, INFO, &aad(&binding)?)?;
        let raw = old::encode(
            &old::a([
                old::u(7),
                old::u(3),
                old::u(head.native_generation),
                old::b(sealed.encapsulated_key),
                old::b(sealed.ciphertext),
            ]),
            state::MAX_NATIVE,
        )?;
        head.blob = Some(old::hash(store::MIXED_NATIVE_ID, &raw));
        Ok(raw)
    }
    pub fn seal(&self, role: usize, head: &mut Head, payload: &old::Value) -> Result<Vec<u8>> {
        let plain = Zeroizing::new(old::encode(payload, state::MAX_NATIVE)?);
        self.seal_plain(role, head, &plain)
    }
    pub fn check(&self, role: usize, head: &Head, raw: &[u8]) -> Result<Checkpoint> {
        let b = Binding {
            material: &self.material,
            source: self.source,
            head,
            q: self.q,
            scope: &self.scope,
            meta: &self.metas[role],
            raw,
        };
        open(&b, |sid, keys| {
            ensure!(
                sid == self.scope.sid && *keys == self.keys,
                "original synthetic SID/all session keys"
            );
            Ok(())
        })
    }
}
