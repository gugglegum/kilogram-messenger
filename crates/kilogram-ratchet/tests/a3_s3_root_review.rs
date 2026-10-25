//! Independent review of Guard2 against real native sessions under delayed delivery.
//! This checks component ordering; it is not protocol, persistence, or policy evidence.
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession, Packet},
    ordered_v2::{EventMeta, Guard2, Seen, SeenObservation},
};
use std::collections::{BTreeMap, VecDeque};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type Id = [u8; 32];

struct Envelope {
    meta: EventMeta,
    id: Id,
    packet: Packet,
    plain: Vec<u8>,
}
struct Probe {
    accounts: [OrderedAccount; 2],
    sessions: [Option<OrderedSession>; 2],
    guards: [Guard2; 2],
    seen: [BTreeMap<(u8, Id), Seen>; 2],
    pending: [VecDeque<Envelope>; 2],
    steps: [u64; 2],
    seed: u32,
    sent: u64,
    received: u64,
    restores: u64,
    rotations: u64,
    max_pending: usize,
}
impl Probe {
    fn new(seed: u32) -> TestResult<Self> {
        let (a, _) = OrderedAccount::fresh(false);
        let (b, otk) = OrderedAccount::fresh(true);
        let session = a.outbound(b.curve_key(), otk.ok_or("missing reserved OTK")?)?;
        Ok(Self {
            accounts: [a, b],
            sessions: [Some(session), None],
            guards: [Guard2::new(0)?, Guard2::new(1)?],
            seen: [BTreeMap::new(), BTreeMap::new()],
            pending: [VecDeque::new(), VecDeque::new()],
            steps: [0, 0],
            seed,
            sent: 0,
            received: 0,
            restores: 0,
            rotations: 0,
            max_pending: 0,
        })
    }
    fn observation(&self, node: usize, meta: &EventMeta) -> SeenObservation {
        self.seen[node]
            .get(&(meta.role, meta.key))
            .copied()
            .map_or(SeenObservation::Absent, SeenObservation::Present)
    }
    fn remember(&mut self, node: usize, meta: &EventMeta, id: Id) {
        self.seen[node]
            .entry((meta.role, meta.key))
            .or_insert(Seen {
                generation: meta.expected.g,
                first: id,
            });
        self.steps[node] += 1;
    }
    fn send(&mut self, role: usize) -> TestResult {
        let guard = &self.guards[role];
        let n = guard.own().next;
        let expected = guard.expected_send()?;
        let mut meta = EventMeta {
            role: role as u8,
            n,
            previous: guard.own().last,
            sequence: n,
            kind: if n == 0 { role as u8 + 1 } else { 3 },
            native_kind: if role == 0 && n == 0 { 0 } else { 1 },
            key: [0; 32],
            index: expected.index,
            expected,
        };
        let step = guard.preflight_send(&meta, self.steps[role], self.steps[role])?;
        let plain = format!("review:{}:{role}:{n}", self.seed).into_bytes();
        let mut trial = self.sessions[role].as_ref().ok_or("send session")?.fork();
        let packet = trial.encrypt(&plain)?;
        let header = packet.header()?;
        assert_eq!(packet.kind, meta.native_kind);
        assert_eq!(header.index, meta.index);
        meta.key = header.key;
        // Check the independently observed native sequence, including actual DH keys.
        let previous = guard.own();
        if previous.key == Some(header.key) {
            assert_eq!(header.index, previous.index + 1);
            assert_eq!(meta.expected.g, previous.g);
        } else {
            assert_eq!(header.index, 0);
            if previous.key.is_some() {
                assert_eq!(meta.expected.g, previous.g + 1);
                self.rotations += 1;
            }
        }
        let id = *blake3::hash(&packet.bytes).as_bytes();
        let mut trial_guard = guard.clone();
        trial_guard.apply(step, &meta, id, self.observation(role, &meta))?;
        self.sessions[role] = Some(trial);
        self.guards[role] = trial_guard;
        self.remember(role, &meta, id);
        self.pending[role].push_back(Envelope {
            meta,
            id,
            packet,
            plain,
        });
        self.max_pending = self.max_pending.max(self.pending[role].len());
        self.sent += 1;
        Ok(())
    }
    fn deliver(&mut self, sender: usize) -> TestResult {
        let Some(envelope) = self.pending[sender].pop_front() else {
            return Ok(());
        };
        let receiver = 1 - sender;
        let observation = self.observation(receiver, &envelope.meta);
        let step = self.guards[receiver].preflight_receive(
            &envelope.meta,
            observation,
            self.steps[receiver],
            self.steps[receiver],
        )?;
        let mut account = self.accounts[receiver].fork();
        let (session, plain) = if let Some(session) = &self.sessions[receiver] {
            let mut trial = session.fork();
            let plain = trial.decrypt(&envelope.packet)?;
            (trial, plain)
        } else {
            account.inbound(self.accounts[sender].curve_key(), &envelope.packet)?
        };
        assert_eq!(plain, envelope.plain);
        assert_eq!(
            session.sid(),
            self.sessions[sender]
                .as_ref()
                .ok_or("sender session")?
                .sid()
        );
        let mut guard = self.guards[receiver].clone();
        guard.apply(step, &envelope.meta, envelope.id, observation)?;
        self.accounts[receiver] = account;
        self.sessions[receiver] = Some(session);
        self.guards[receiver] = guard;
        self.remember(receiver, &envelope.meta, envelope.id);
        self.received += 1;
        Ok(())
    }
    fn reload(&mut self) -> TestResult {
        for role in 0..2 {
            let key = [41 + role as u8; 32];
            let old_curve = self.accounts[role].curve_key();
            let old_guard = self.guards[role].clone();
            self.accounts[role] =
                OrderedAccount::restore(&self.accounts[role].snapshot(&key), &key)?;
            let session = self.sessions[role].as_ref().ok_or("reload session")?;
            let sid = session.sid();
            let keys = session.keys();
            let restored = OrderedSession::restore(&session.snapshot(&key), &key)?;
            assert_eq!(restored.sid(), sid);
            assert_eq!(restored.keys(), keys);
            self.sessions[role] = Some(restored);
            self.guards[role] = Guard2::restore(&old_guard.encode()?, role as u8)?;
            assert_eq!(self.guards[role], old_guard);
            assert_eq!(self.accounts[role].curve_key(), old_curve);
            self.restores += 1;
        }
        Ok(())
    }
}

#[test]
fn a3_s3_root_native_delayed_delivery_schedules() -> TestResult {
    let started = std::time::Instant::now();
    let mut totals = [0u64; 4];
    let mut max_pending = 0;
    for seed in 1u32..=32 {
        let mut p = Probe::new(seed)?;
        p.send(0)?;
        p.deliver(0)?;
        p.send(1)?;
        p.deliver(1)?;
        let mut random = seed;
        for tick in 0..64 {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            let action = (random & 3) as usize;
            let role = action & 1;
            if action < 2 && p.pending[role].len() < 16 {
                p.send(role)?;
            } else {
                p.deliver(role)?;
            }
            if tick % 7 == 0 {
                p.reload()?;
            }
        }
        for role in 0..2 {
            while !p.pending[role].is_empty() {
                p.deliver(role)?;
            }
        }
        p.reload()?;
        assert_eq!(p.sent, p.received);
        assert_eq!(p.guards[0].own(), p.guards[1].peer());
        assert_eq!(p.guards[1].own(), p.guards[0].peer());
        assert_eq!(p.seen[0], p.seen[1]);
        totals[0] += p.sent;
        totals[1] += p.received;
        totals[2] += p.restores;
        totals[3] += p.rotations;
        max_pending = max_pending.max(p.max_pending);
    }
    assert!(max_pending > 1);
    assert!(totals[3] > 32);
    assert!(started.elapsed().as_secs() < 600);
    println!(
        "root_review schedules=32 sent={} received={} successful_message_calls={} node_restores={} sending_DH_rotations={} max_pending={} elapsed_seconds={:.6}",
        totals[0],
        totals[1],
        totals[0] + totals[1],
        totals[2],
        totals[3],
        max_pending,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
