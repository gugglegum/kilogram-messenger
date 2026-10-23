//! Real Olm; test-only signed frames and a persisted, single-session guard.
use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Result, bail, ensure};
use kilogram_identity::{DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vodozemac::{
    Curve25519PublicKey,
    olm::{Account, DecryptionError, Message, OlmMessage, Session, SessionConfig, SessionPickle},
};

const PICKLE_KEY: [u8; 32] = [71; 32]; // Synthetic fixture only.
const SIGN_DOMAIN: &[u8] = b"kilogram:a1-offline-spike:not-a-wire-protocol\0";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Binding {
    context: String,
    epoch: u64,
    session: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Meta {
    binding: Binding,
    author: DeviceId,
    n: u64,
    previous: Option<String>,
    kind: String,
}

#[derive(Serialize, Deserialize)]
struct Frame {
    meta: Meta,
    text: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct Envelope {
    meta: Meta,
    message_type: usize,
    ciphertext: Vec<u8>,
    signature: Vec<u8>,
}

impl Envelope {
    fn signed(meta: Meta, message: &OlmMessage, signer: &DeviceIdentity) -> Result<Self> {
        let (message_type, ciphertext) = message.to_parts();
        let mut value = Self {
            meta,
            message_type,
            ciphertext,
            signature: Vec::new(),
        };
        value.signature = signer.sign(&value.signing_bytes()?).to_vec();
        Ok(value)
    }

    fn signing_bytes(&self) -> Result<Vec<u8>> {
        let mut result = SIGN_DOMAIN.to_vec();
        result.extend(serde_json::to_vec(&(
            &self.meta,
            self.message_type,
            &self.ciphertext,
        ))?);
        Ok(result)
    }

    fn resign(&mut self, signer: &DeviceIdentity) -> Result<()> {
        self.signature = signer.sign(&self.signing_bytes()?).to_vec();
        Ok(())
    }

    fn id(&self) -> Result<String> {
        Ok(blake3::hash(&serde_json::to_vec(self)?)
            .to_hex()
            .to_string())
    }

    fn message(&self) -> Result<OlmMessage> {
        Ok(OlmMessage::from_parts(self.message_type, &self.ciphertext)?)
    }
}

fn header(message: &OlmMessage) -> &Message {
    match message {
        OlmMessage::Normal(message) => message,
        OlmMessage::PreKey(message) => message.message(),
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Frontier {
    next: u64,
    previous: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    binding: Binding,
    peer: DeviceId,
    send: Frontier,
    receive: Frontier,
    latest_key: Option<String>,
    chain_next: u64,
    recent_keys: Vec<String>,
    receipts: BTreeMap<u64, String>,
    readable: BTreeMap<u64, String>,
    pending: BTreeMap<u64, Envelope>,
    dh_transitions: usize,
    encrypted_pickle: String,
}

struct Guard {
    session: Session,
    state: Stored,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Readable,
    Pending,
    Replay,
    Invalid(&'static str),
}

impl Guard {
    fn new(session: Session, binding: Binding, peer: DeviceId) -> Self {
        Self {
            session,
            state: Stored {
                binding,
                peer,
                send: Frontier::default(),
                receive: Frontier::default(),
                latest_key: None,
                chain_next: 0,
                recent_keys: Vec::new(),
                receipts: BTreeMap::new(),
                readable: BTreeMap::new(),
                pending: BTreeMap::new(),
                dh_transitions: 0,
                encrypted_pickle: String::new(),
            },
        }
    }

    fn meta(&self, signer: &DeviceIdentity, kind: &str) -> Meta {
        Meta {
            binding: self.state.binding.clone(),
            author: signer.device_id(),
            n: self.state.send.next,
            previous: self.state.send.previous.clone(),
            kind: kind.into(),
        }
    }

    fn send(&mut self, signer: &DeviceIdentity, kind: &str, text: &str) -> Result<Envelope> {
        let meta = self.meta(signer, kind);
        let message = self.session.encrypt(serde_json::to_vec(&Frame {
            meta: meta.clone(),
            text: text.into(),
        })?)?;
        let envelope = Envelope::signed(meta, &message, signer)?;
        self.state.send.next += 1;
        self.state.send.previous = Some(envelope.id()?);
        Ok(envelope)
    }

    // Canonical secret-bearing native serde bytes, never Debug/encrypted-pickle bytes.
    // Pending is excluded here: accepting it may change the queue, not crypto/readable state.
    fn canonical(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&json!({
            "native": self.session.pickle(), "binding": self.state.binding,
            "send": self.state.send, "receive": self.state.receive,
            "latest_key": self.state.latest_key, "chain_next": self.state.chain_next,
            "recent_keys": self.state.recent_keys, "receipts": self.state.receipts,
            "readable": self.state.readable, "dh_transitions": self.state.dh_transitions,
        }))?)
    }

    fn reload(&mut self, file: &Path) -> Result<()> {
        let before = self.canonical()?;
        let pending_before = serde_json::to_vec(&self.state.pending)?;
        self.state.encrypted_pickle = self.session.pickle().encrypt(&PICKLE_KEY);
        fs::write(file, serde_json::to_vec(&self.state)?)?;
        let state: Stored = serde_json::from_slice(&fs::read(file)?)?;
        let session = Session::from_pickle(SessionPickle::from_encrypted(
            &state.encrypted_pickle,
            &PICKLE_KEY,
        )?);
        ensure!(
            session.session_id() == state.binding.session,
            "reload session binding"
        );
        self.session = session;
        self.state = state;
        ensure!(
            before == self.canonical()?,
            "pickle/reload changed canonical state"
        );
        ensure!(
            pending_before == serde_json::to_vec(&self.state.pending)?,
            "pending reload"
        );
        if let Some(key) = &self.state.latest_key {
            let oracle = native_heads(serde_json::to_value(self.session.pickle())?)?;
            ensure!(
                oracle
                    .iter()
                    .any(|head| &head.key == key && head.next == self.state.chain_next),
                "own persisted chain frontier disagrees with native pickle oracle"
            );
        }
        Ok(())
    }

    fn accept(&mut self, event: Envelope) -> Result<Outcome> {
        let result = self.apply(&event)?;
        if result == Outcome::Pending {
            ensure!(
                self.state.pending.len() < 8,
                "fixture pending capacity exceeded"
            );
            ensure!(
                serde_json::to_vec(&event)?.len() <= 16 * 1024,
                "fixture event cap"
            );
            self.state.pending.insert(event.meta.n, event);
        } else if result == Outcome::Readable {
            while let Some(next) = self.state.pending.get(&self.state.receive.next).cloned() {
                ensure!(
                    self.apply(&next)? == Outcome::Readable,
                    "invalid pending fixture"
                );
                self.state.pending.remove(&next.meta.n);
            }
        }
        Ok(result)
    }

    fn apply(&mut self, event: &Envelope) -> Result<Outcome> {
        let meta = &event.meta;
        if meta.author != self.state.peer
            || self
                .state
                .peer
                .verify(&event.signing_bytes()?, &event.signature)
                .is_err()
        {
            return Ok(Outcome::Invalid("signature"));
        }
        if meta.binding.context != self.state.binding.context {
            return Ok(Outcome::Invalid("context"));
        }
        if meta.binding.epoch != self.state.binding.epoch {
            return Ok(Outcome::Invalid("epoch"));
        }
        if meta.binding.session != self.state.binding.session {
            return Ok(Outcome::Invalid("session"));
        }
        let id = event.id()?;
        if meta.n < self.state.receive.next {
            return Ok(if self.state.receipts.get(&meta.n) == Some(&id) {
                Outcome::Replay
            } else {
                Outcome::Invalid("fork")
            });
        }
        if let Some(existing) = self.state.pending.get(&meta.n) {
            if existing.id()? != id {
                return Ok(Outcome::Invalid("fork"));
            }
            if meta.n != self.state.receive.next {
                return Ok(Outcome::Replay);
            }
        }
        if meta.n > self.state.receive.next {
            return Ok(Outcome::Pending);
        }
        if meta.previous != self.state.receive.previous {
            return Ok(Outcome::Invalid("predecessor"));
        }
        let message = match event.message() {
            Ok(message) => message,
            Err(_) => return Ok(Outcome::Invalid("encoding")),
        };
        if let OlmMessage::PreKey(prekey) = &message {
            if prekey.session_keys() != self.session.session_keys() {
                return Ok(Outcome::Invalid("session-keys"));
            }
        }
        let key = header(&message).ratchet_key().to_base64();
        let index = header(&message).chain_index();
        let same_chain = self.state.latest_key.as_ref() == Some(&key);
        if same_chain && index != self.state.chain_next {
            return Ok(Outcome::Invalid("same-chain-index"));
        }
        if !same_chain {
            if self.state.recent_keys.contains(&key) {
                return Ok(Outcome::Invalid("old-chain"));
            }
            if index != 0 {
                return Ok(Outcome::Invalid("new-chain-index"));
            }
        }
        let mut trial = Session::from_pickle(self.session.pickle());
        let plaintext = match trial.decrypt(&message) {
            Ok(plaintext) => plaintext,
            Err(_) => return Ok(Outcome::Invalid("crypto")),
        };
        let frame: Frame = match serde_json::from_slice(&plaintext) {
            Ok(frame) => frame,
            Err(_) => return Ok(Outcome::Invalid("inner-binding")),
        };
        if frame.meta != *meta {
            return Ok(Outcome::Invalid("inner-binding"));
        }
        self.session = trial; // Commit only after the inner binding check.
        if !same_chain {
            self.state.dh_transitions += 1;
            self.state.recent_keys.push(key.clone());
            if self.state.recent_keys.len() > 5 {
                self.state.recent_keys.remove(0);
            }
        }
        self.state.latest_key = Some(key);
        self.state.chain_next = index + 1;
        self.state.receive.next += 1;
        self.state.receive.previous = Some(id.clone());
        self.state.receipts.insert(meta.n, id);
        if meta.kind == "data" {
            self.state.readable.insert(meta.n, frame.text);
        }
        Ok(Outcome::Readable)
    }
}

struct Pair {
    alice: Guard,
    bob: Guard,
    alice_signer: DeviceIdentity,
    bob_signer: DeviceIdentity,
    genesis: Envelope,
}

impl Pair {
    fn new(context: &str) -> Result<Self> {
        let alice_account = Account::new();
        let mut bob_account = Account::new();
        bob_account.generate_one_time_keys(1);
        let otk = *bob_account
            .one_time_keys()
            .values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("OTK"))?;
        let session = alice_account.create_outbound_session(
            SessionConfig::version_1(),
            bob_account.curve25519_key(),
            otk,
        )?;
        let alice_signer = DeviceIdentity::from_secret_bytes([11; 32]);
        let bob_signer = DeviceIdentity::from_secret_bytes([12; 32]);
        let binding = Binding {
            context: context.into(),
            epoch: 1,
            session: session.session_id(),
        };
        // Trusted fixture chooses one initiator, one OTK/base key/session, before writers.
        // Both actual Device signatures bind these exact public inputs; no certificates here.
        let agreement = serde_json::to_vec(&json!({
            "binding": binding, "roles": ["alice-initiator", "bob-responder"],
            "devices": [alice_signer.device_id(), bob_signer.device_id()],
            "initiator_identity": alice_account.curve25519_key(),
            "responder_identity": bob_account.curve25519_key(),
            "session_keys": session.session_keys(), "suite": "OlmV1",
        }))?;
        alice_signer
            .device_id()
            .verify(&agreement, &alice_signer.sign(&agreement))?;
        bob_signer
            .device_id()
            .verify(&agreement, &bob_signer.sign(&agreement))?;
        let mut alice = Guard::new(session, binding.clone(), bob_signer.device_id());
        let genesis = alice.send(&alice_signer, "handshake", "init")?;
        alice_signer
            .device_id()
            .verify(&genesis.signing_bytes()?, &genesis.signature)?;
        let message = genesis.message()?;
        let OlmMessage::PreKey(prekey) = &message else {
            bail!("expected prekey genesis")
        };
        ensure!(prekey.session_id() == binding.session && prekey.one_time_key() == otk);
        let created = bob_account.create_inbound_session(
            SessionConfig::version_1(),
            alice_account.curve25519_key(),
            prekey,
        )?;
        let frame: Frame = serde_json::from_slice(&created.plaintext)?;
        ensure!(frame.meta == genesis.meta && frame.text == "init");
        let mut bob = Guard::new(created.session, binding, alice_signer.device_id());
        bob.state.receive = Frontier {
            next: 1,
            previous: Some(genesis.id()?),
        };
        bob.state.receipts.insert(0, genesis.id()?);
        bob.state.latest_key = Some(prekey.message().ratchet_key().to_base64());
        bob.state.recent_keys.push(
            bob.state
                .latest_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("key"))?,
        );
        bob.state.chain_next = 1;
        let confirmation = bob.send(&bob_signer, "handshake", "confirm")?;
        ensure!(alice.accept(confirmation)? == Outcome::Readable);
        Ok(Self {
            alice,
            bob,
            alice_signer,
            bob_signer,
            genesis,
        })
    }
}

fn unchanged(guard: &mut Guard, event: Envelope, expected: Outcome, name: &str) -> Result<()> {
    let before = guard.canonical()?;
    let pending = serde_json::to_vec(&guard.state.pending)?;
    ensure!(
        guard.accept(event)? == expected,
        "unexpected rejection: {name}"
    );
    ensure!(
        guard.canonical()? == before && serde_json::to_vec(&guard.state.pending)? == pending,
        "rejection/replay mutated state: {name}"
    );
    println!("f1.reject_or_replay.{name}=expected; canonical_state_unchanged=true");
    Ok(())
}

fn forged(sender: &Guard, signer: &DeviceIdentity, inner: Meta, skip: usize) -> Result<Envelope> {
    let mut trial = Session::from_pickle(sender.session.pickle());
    for _ in 0..skip {
        let _ = trial.encrypt(b"not-delivered-fixture")?;
    }
    let message = trial.encrypt(serde_json::to_vec(&Frame {
        meta: inner,
        text: "bad".into(),
    })?)?;
    Envelope::signed(sender.meta(signer, "data"), &message, signer)
}

fn negative_checks(file: &Path) -> Result<()> {
    let mut pair = Pair::new("negative-context")?;
    let before_first = pair.alice.meta(&pair.alice_signer, "data");
    unchanged(
        &mut pair.bob,
        forged(&pair.alice, &pair.alice_signer, before_first, 1)?,
        Outcome::Invalid("new-chain-index"),
        "new_chain_nonzero_index",
    )?;
    let first = pair.alice.send(&pair.alice_signer, "data", "valid")?;
    ensure!(pair.bob.accept(first.clone())? == Outcome::Readable);
    let meta = pair.alice.meta(&pair.alice_signer, "data");
    unchanged(
        &mut pair.bob,
        forged(&pair.alice, &pair.alice_signer, meta.clone(), 1)?,
        Outcome::Invalid("same-chain-index"),
        "same_chain_wrong_index",
    )?;
    let mut inner = meta.clone();
    inner.n += 1;
    unchanged(
        &mut pair.bob,
        forged(&pair.alice, &pair.alice_signer, inner, 0)?,
        Outcome::Invalid("inner-binding"),
        "signed_wrong_ordinal",
    )?;
    let mut inner = meta.clone();
    inner.binding.context = "wrong-inner-context".into();
    unchanged(
        &mut pair.bob,
        forged(&pair.alice, &pair.alice_signer, inner, 0)?,
        Outcome::Invalid("inner-binding"),
        "signed_inner_binding_rollback",
    )?;
    for (name, reason) in [
        ("wrong_context", "context"),
        ("wrong_epoch", "epoch"),
        ("double_predecessor", "predecessor"),
        ("bad_signature", "signature"),
        ("bad_mac", "crypto"),
    ] {
        let mut event = forged(&pair.alice, &pair.alice_signer, meta.clone(), 0)?;
        match name {
            "wrong_context" => event.meta.binding.context = "other".into(),
            "wrong_epoch" => event.meta.binding.epoch += 1,
            "double_predecessor" => event.meta.previous = None,
            "bad_mac" => {
                if let Some(byte) = event.ciphertext.last_mut() {
                    *byte ^= 1;
                }
            }
            _ => {}
        }
        event.resign(&pair.alice_signer)?;
        if name == "bad_signature" {
            event.signature[0] ^= 1;
        }
        unchanged(&mut pair.bob, event, Outcome::Invalid(reason), name)?;
    }
    let competing = Pair::new("negative-context")?;
    unchanged(
        &mut pair.bob,
        competing.genesis.clone(),
        Outcome::Invalid("session"),
        "competing_session",
    )?;
    let mut relabelled = competing.genesis;
    relabelled.meta = meta.clone();
    relabelled.resign(&pair.alice_signer)?;
    unchanged(
        &mut pair.bob,
        relabelled,
        Outcome::Invalid("session-keys"),
        "foreign_session_relabelled",
    )?;
    unchanged(
        &mut pair.bob,
        first.clone(),
        Outcome::Replay,
        "old_event_replay",
    )?;
    let mut fork = first;
    fork.ciphertext[0] ^= 1;
    fork.resign(&pair.alice_signer)?;
    unchanged(
        &mut pair.bob,
        fork,
        Outcome::Invalid("fork"),
        "same_slot_fork",
    )?;
    let mut old_chain = pair.genesis;
    old_chain.meta = meta;
    old_chain.resign(&pair.alice_signer)?;
    unchanged(
        &mut pair.bob,
        old_chain,
        Outcome::Invalid("old-chain"),
        "old_chain_new_position",
    )?;
    pair.bob.reload(file)?;
    Ok(())
}

#[derive(Debug)]
struct NativeHead {
    key: String,
    next: u64,
    skipped: Vec<u64>,
}

#[derive(Debug, PartialEq, Eq)]
enum LegacyAdmission {
    SameChainReady,
    RetainedSkippedKey,
    MissingSameChainPrefix,
    UnavailableKey,
    UnresolvedLegacyTail,
}

fn legacy_admission(heads: &[NativeHead], message: &OlmMessage) -> LegacyAdmission {
    let key = header(message).ratchet_key().to_base64();
    let index = header(message).chain_index();
    match heads.iter().find(|head| head.key == key) {
        None => LegacyAdmission::UnresolvedLegacyTail,
        Some(head) if index == head.next => LegacyAdmission::SameChainReady,
        Some(head) if index > head.next => LegacyAdmission::MissingSameChainPrefix,
        Some(head) if head.skipped.contains(&index) => LegacyAdmission::RetainedSkippedKey,
        Some(_) => LegacyAdmission::UnavailableKey,
    }
}

fn exact_keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("native schema: object"))?;
    ensure!(
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key)),
        "unsupported vodozemac 0.10.0 serde shape"
    );
    Ok(())
}

// Version-pinned serde adapter. It proves local chain cursors/retained skipped indexes ONLY.
// It cannot import application ordinals, sender tail lengths or absent/evicted keys.
fn native_heads(value: Value) -> Result<Vec<NativeHead>> {
    exact_keys(
        &value,
        &[
            "session_keys",
            "sending_ratchet",
            "receiving_chains",
            "config",
        ],
    )?;
    let _: SessionPickle = serde_json::from_value(value.clone())?;
    exact_keys(&value["receiving_chains"], &["inner"])?;
    let chains = value["receiving_chains"]["inner"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("chains"))?;
    ensure!(chains.len() <= 5);
    let mut result = Vec::new();
    for chain in chains {
        exact_keys(
            chain,
            &[
                "ratchet_key",
                "hkdf_ratchet",
                "skipped_message_keys",
                "ratchet_count",
            ],
        )?;
        exact_keys(&chain["hkdf_ratchet"], &["key", "index"])?;
        exact_keys(&chain["skipped_message_keys"], &["inner"])?;
        let key: Curve25519PublicKey = serde_json::from_value(chain["ratchet_key"].clone())?;
        let _: [u8; 32] = serde_json::from_value(chain["hkdf_ratchet"]["key"].clone())?;
        let next = chain["hkdf_ratchet"]["index"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("index"))?;
        let skipped_values = chain["skipped_message_keys"]["inner"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("skipped"))?;
        ensure!(skipped_values.len() <= 40);
        let mut skipped = Vec::new();
        for entry in skipped_values {
            exact_keys(entry, &["key", "index"])?;
            let _: [u8; 32] = serde_json::from_value(entry["key"].clone())?;
            let index = entry["index"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("skipped index"))?;
            ensure!(index < next && !skipped.contains(&index));
            skipped.push(index);
        }
        result.push(NativeHead {
            key: key.to_base64(),
            next,
            skipped,
        });
    }
    Ok(result)
}

fn raw_sender() -> Result<(Account, Account, Session)> {
    let alice = Account::new();
    let mut bob = Account::new();
    bob.generate_one_time_keys(1);
    let otk = *bob
        .one_time_keys()
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("OTK"))?;
    let session =
        alice.create_outbound_session(SessionConfig::version_1(), bob.curve25519_key(), otk)?;
    Ok((alice, bob, session))
}

fn inbound(
    bob: &mut Account,
    alice: &Account,
    message: &OlmMessage,
    expected: &[u8],
) -> Result<Session> {
    let OlmMessage::PreKey(prekey) = message else {
        bail!("prekey expected")
    };
    let created =
        bob.create_inbound_session(SessionConfig::version_1(), alice.curve25519_key(), prekey)?;
    ensure!(created.plaintext == expected, "inbound genesis plaintext");
    Ok(created.session)
}

fn legacy_checks(file: &Path) -> Result<()> {
    let (alice_account, mut bob_account, mut alice) = raw_sender()?;
    let mut messages = Vec::new();
    for n in 0..256 {
        messages.push(alice.encrypt(format!("legacy-{n}"))?);
    }
    // Deterministic delivery order, no application ordinal invented for legacy.
    let mut pending = BTreeMap::new();
    for n in (1..256).rev().chain(std::iter::once(0)) {
        pending.insert(header(&messages[n]).chain_index(), messages[n].clone());
    }
    let genesis = pending
        .remove(&0)
        .ok_or_else(|| anyhow::anyhow!("genesis"))?;
    let mut bob = inbound(&mut bob_account, &alice_account, &genesis, b"legacy-0")?;
    let mut count = 1;
    for n in 1..256_u64 {
        let message = pending
            .remove(&n)
            .ok_or_else(|| anyhow::anyhow!("prefix"))?;
        let known = native_heads(serde_json::to_value(bob.pickle())?)?;
        ensure!(
            known.len() == 1
                && known[0].next == n
                && known[0].skipped.is_empty()
                && legacy_admission(&known, &message) == LegacyAdmission::SameChainReady
        );
        ensure!(bob.decrypt(&message)? == format!("legacy-{n}").as_bytes());
        count += 1;
        if n % 64 == 0 {
            fs::write(file, bob.pickle().encrypt(&PICKLE_KEY))?;
            bob = Session::from_pickle(SessionPickle::from_encrypted(
                &fs::read_to_string(file)?,
                &PICKLE_KEY,
            )?);
        }
    }
    ensure!(count == 256 && pending.is_empty());
    println!(
        "f1.legacy_known_genesis_single_chain=256; native_reload=true; migration_generalized=false"
    );

    let (alice_account, mut bob_account, mut alice) = raw_sender()?;
    let k0 = alice.encrypt("K0")?;
    let k1 = alice.encrypt("K1-delayed")?;
    let mut bob = inbound(&mut bob_account, &alice_account, &k0, b"K0")?;
    let old_key = header(&k0).ratchet_key().to_base64();
    let b0 = bob.encrypt("B0")?;
    ensure!(alice.decrypt(&b0)? == b"B0");
    let l0 = alice.encrypt("L0")?;
    let before = serde_json::to_vec(&bob.pickle())?;
    let heads = native_heads(serde_json::to_value(bob.pickle())?)?;
    ensure!(heads.len() == 1 && heads[0].next == 1 && heads[0].skipped.is_empty());
    ensure!(
        !heads
            .iter()
            .any(|head| head.key == header(&l0).ratchet_key().to_base64())
    );
    // Explicit selected policy: an unseen legacy DH key has no authenticated old-tail closure.
    let classification = legacy_admission(&heads, &l0);
    ensure!(
        classification == LegacyAdmission::UnresolvedLegacyTail
            && before == serde_json::to_vec(&bob.pickle())?
    );
    println!(
        "f1.legacy_K0_K1_B0_L0=UnresolvedLegacyTail; local_next=1; sender_tail_length=unknown; canonical_state_unchanged=true"
    );
    // Unsafe native control demonstrates that the local cursor cannot protect the old tail.
    ensure!(bob.decrypt(&l0)? == b"L0");
    for _ in 0..7 {
        let reply = bob.encrypt("reply")?;
        alice.decrypt(&reply)?;
        let next = alice.encrypt("next")?;
        bob.decrypt(&next)?;
    }
    let heads = native_heads(serde_json::to_value(bob.pickle())?)?;
    ensure!(heads.len() == 5 && !heads.iter().any(|head| head.key == old_key));
    ensure!(
        matches!(bob.decrypt(&k1), Err(DecryptionError::InvalidMAC(_))),
        "unsafe control expected old-chain InvalidMAC"
    );
    println!(
        "f1.unsafe_native_control=old_chain_evicted; retained_chains=5; delayed_K1_unreadable=true"
    );

    let (alice_account, mut bob_account, mut alice) = raw_sender()?;
    let mut messages = Vec::new();
    for n in 0..6 {
        messages.push(alice.encrypt(format!("native-{n}"))?);
    }
    let mut bob = inbound(&mut bob_account, &alice_account, &messages[0], b"native-0")?;
    bob.decrypt(&messages[5])?;
    fs::write(file, bob.pickle().encrypt(&PICKLE_KEY))?;
    bob = Session::from_pickle(SessionPickle::from_encrypted(
        &fs::read_to_string(file)?,
        &PICKLE_KEY,
    )?);
    let native = serde_json::to_value(bob.pickle())?;
    let heads = native_heads(native.clone())?;
    ensure!(heads.len() == 1 && heads[0].next == 6 && heads[0].skipped == [1, 2, 3, 4]);
    ensure!(legacy_admission(&heads, &messages[1]) == LegacyAdmission::RetainedSkippedKey);
    ensure!(legacy_admission(&heads, &messages[0]) == LegacyAdmission::UnavailableKey);
    let _six = alice.encrypt("native-6")?;
    let seven = alice.encrypt("native-7")?;
    ensure!(legacy_admission(&heads, &seven) == LegacyAdmission::MissingSameChainPrefix);
    ensure!(bob.decrypt(&messages[1])? == b"native-1");
    let mut unsupported = native;
    unsupported
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("object"))?
        .insert("new-field".into(), json!(0));
    let error = native_heads(unsupported)
        .err()
        .ok_or_else(|| anyhow::anyhow!("schema drift accepted"))?;
    ensure!(
        error
            .to_string()
            .contains("unsupported vodozemac 0.10.0 serde shape")
    );
    println!(
        "f1.native_import=SameChainCursorOnly; next=6; skipped=[1,2,3,4]; one_retained_key_decrypted=true; unknown_schema=rejected"
    );
    println!(
        "f1.native_import_cannot_prove=application_ordinal,sender_tail_length,evicted_chains,global_genesis"
    );
    Ok(())
}

pub fn run(directory: &Path) -> Result<()> {
    let file = directory.join("f1-guard.json");
    let mut total = 0;
    let mut reloads = 0;
    for context in ["context-A", "context-B"] {
        let mut pair = Pair::new(context)?;
        let mut delayed: Option<Envelope> = None;
        let mut expected = Vec::new();
        for cycle in 0..8 {
            let mut messages = Vec::new();
            for index in 0..32 {
                let text = format!("{context}-{cycle}-{index}");
                expected.push(text.clone());
                messages.push(pair.alice.send(&pair.alice_signer, "data", &text)?);
            }
            if let Some(predecessor) = delayed.take() {
                let before = pair.bob.canonical()?;
                ensure!(header(&messages[0].message()?).chain_index() == 0);
                ensure!(pair.bob.accept(messages[0].clone())? == Outcome::Pending);
                ensure!(pair.bob.canonical()? == before, "waiting advanced crypto");
                pair.bob.reload(&file)?;
                reloads += 1;
                ensure!(pair.bob.canonical()? == before);
                ensure!(pair.bob.accept(predecessor)? == Outcome::Readable);
                ensure!(pair.bob.state.pending.is_empty());
            } else {
                ensure!(pair.bob.accept(messages[0].clone())? == Outcome::Readable);
            }
            for message in &messages[1..31] {
                ensure!(pair.bob.accept(message.clone())? == Outcome::Readable);
            }
            delayed = Some(messages[31].clone());
            let reply = pair.bob.send(
                &pair.bob_signer,
                "data",
                &format!("{context}-reply-{cycle}"),
            )?;
            ensure!(pair.alice.accept(reply)? == Outcome::Readable);
            pair.alice.reload(&file)?;
            pair.bob.reload(&file)?;
            reloads += 2;
        }
        ensure!(
            pair.bob
                .accept(delayed.ok_or_else(|| anyhow::anyhow!("delayed"))?)?
                == Outcome::Readable
        );
        pair.bob.reload(&file)?;
        reloads += 1;
        let actual = pair
            .bob
            .state
            .readable
            .values()
            .cloned()
            .collect::<Vec<_>>();
        ensure!(actual == expected && actual.len() == 256);
        ensure!(
            pair.alice
                .state
                .readable
                .values()
                .cloned()
                .collect::<Vec<_>>()
                == (0..8)
                    .map(|cycle| format!("{context}-reply-{cycle}"))
                    .collect::<Vec<_>>()
        );
        ensure!(pair.bob.state.dh_transitions >= 7);
        let heads = native_heads(serde_json::to_value(pair.bob.session.pickle())?)?;
        ensure!(heads.len() == 5 && heads.iter().all(|head| head.skipped.is_empty()));
        println!(
            "f1.{context}=256 exact plaintexts; dh_transitions={}; held_new_chain_successors=7; native_chains={}",
            pair.bob.state.dh_transitions,
            heads.len()
        );
        total += actual.len();
    }
    println!("f1.user_plaintexts={total}; contexts=2; guard_pickle_reloads={reloads}");
    negative_checks(&file)?;
    legacy_checks(&directory.join("f1-native.pickle"))?;
    println!(
        "f1.auth=real_Device_signatures,real_Olm_MAC,mutually_signed_test_transcript; trusted=credential_admission,role_election,OTK_reservation; production_wire_or_vault=false"
    );
    Ok(())
}
