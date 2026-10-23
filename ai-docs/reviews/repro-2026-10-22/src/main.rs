//! Offline component-level audit reproducer. No sockets or product-source changes.
//! Uses the real SyncServer batch planner, SignedEvent, EventStore, and Olm ratchet.
use std::{error::Error, fs, path::Path};

use kilogram_identity::{AccountRootState, DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity};
use kilogram_protocol::{AuthorizedEvent, ConversationId, RatchetRecipient, SignedEvent, SignedSyncInventory, SyncSessionBinding};
use kilogram_ratchet::{RatchetError, RatchetState, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS, PREKEY_CLOCK_SKEW_SECONDS, unix_time_now};
use kilogram_session::{ServerInventoryOutcome, SyncServer};
use kilogram_store::EventStore;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn clone_initial_ratchet_files(source: &Path, destination: &Path) -> Result<()> {
    let source = source.join("ratchet");
    let destination = destination.join("ratchet");
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn run_backlog_probe() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let alice_dir = tmp.path().join("alice");
    let bob_dir = tmp.path().join("bob");
    let control_dir = tmp.path().join("bob-ordered-control");
    let alice = DeviceIdentity::generate()?;
    let bob = DeviceIdentity::generate()?;
    let alice_root = AccountRootState::create(tmp.path().join("alice-root"))?;
    let bob_root = AccountRootState::create(tmp.path().join("bob-root"))?;
    let alice_enc = DeviceEncryptionIdentity::generate()?;
    let bob_enc = DeviceEncryptionIdentity::generate()?;
    let alice_cert = alice_root.issue_device_certificate(alice.device_id(), alice_enc.public_key(), &DeviceCapability::MESSAGING)?;
    let bob_cert = bob_root.issue_device_certificate(bob.device_id(), bob_enc.public_key(), &DeviceCapability::MESSAGING)?;
    let bob_list = bob_root.publish_device_list(&[bob_cert])?;
    let alice_authority = alice_root.authority_snapshot()?;
    let conversation = ConversationId::from_label("astra-offline-backlog-audit");
    let membership = alice_root.create_conversation_membership(conversation.scope_id(), &[bob_root.account_id()])?;
    let now = unix_time_now()?;
    let bob_pool = RatchetState::load_or_create(&bob_dir)?.prekey_pool(&bob, 16, now, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS)?;
    clone_initial_ratchet_files(&bob_dir, &control_dir)?;
    let mut sender = RatchetState::load_or_create(&alice_dir)?;
    let source = EventStore::open(tmp.path().join("events"))?;
    let mut events = Vec::new();
    let mut parents = Vec::new();
    for sequence in 0..256_u64 {
        let text = format!("offline message {sequence}");
        let (identity, ciphertext, _) = sender.encrypt_with_pool(&alice, &bob_pool, &text, now)?;
        let event = SignedEvent::sign_ratchet_text(&alice, conversation, sequence, parents, bob_list.clone(), identity, vec![RatchetRecipient::new(bob.device_id(), ciphertext)?])?;
        parents = vec![event.event_id()?];
        events.push(AuthorizedEvent::new(event, alice_cert.clone(), alice_authority.clone())?);
    }
    source.put_authorized_batch(&events, &membership)?;

    // Positive control: same ciphertexts and initial recipient keys, chronological order.
    let mut control = RatchetState::load_or_create(&control_dir)?;
    for event in &events {
        let (identity, ciphertext) = event.event().ratchet_message_for(bob.device_id())?;
        let (body, _) = control.decrypt(&bob, identity, ciphertext)?;
        assert_eq!(body.as_str(), format!("offline message {}", event.event().author_sequence()));
    }
    println!("ordered_control_decrypted={}", events.len());

    let binding = SyncSessionBinding::from_transport_label("offline-component-probe");
    let server = SyncServer::new(&alice, &source, binding, bob.device_id(), &membership);
    let empty_inventory = SignedSyncInventory::sign(&bob, conversation, binding, Vec::new())?;
    let ServerInventoryOutcome::Accepted(first) = server.accept_inventory(&empty_inventory)? else {
        return Err("first inventory unexpectedly rejected".into());
    };
    let mut first_events = first.diff().events().to_vec();
    // Identical per-batch ordering to CLI DecryptingSessionStore::ensure_local_projections.
    first_events.sort_by_key(|event| (event.event().author_device_id(), event.event().author_sequence()));
    let mut receiver = RatchetState::load_or_create(&bob_dir)?;
    let mut received_ids = Vec::new();
    let mut first_sequences = Vec::new();
    for event in &first_events {
        let (identity, ciphertext) = event.event().ratchet_message_for(bob.device_id())?;
        receiver.decrypt(&bob, identity, ciphertext)?;
        received_ids.push(event.event().event_id()?);
        first_sequences.push(event.event().author_sequence());
    }
    println!("hash_order_first_batch_decrypted={}", first_events.len());
    println!("hash_order_first_batch_sequences={first_sequences:?}");
    drop(receiver); // Prove the loss survives normal persistent-state reload.
    let mut receiver = RatchetState::load_or_create(&bob_dir)?;
    let inventory = SignedSyncInventory::sign(&bob, conversation, binding, received_ids)?;
    let ServerInventoryOutcome::Accepted(second) = server.accept_inventory(&inventory)? else {
        return Err("second inventory unexpectedly rejected".into());
    };
    let mut second_events = second.diff().events().to_vec();
    second_events.sort_by_key(|event| (event.event().author_device_id(), event.event().author_sequence()));
    let mut failed = None;
    for event in &second_events {
        let (identity, ciphertext) = event.event().ratchet_message_for(bob.device_id())?;
        if let Err(error) = receiver.decrypt(&bob, identity, ciphertext) {
            failed = Some((event.event().author_sequence(), error));
            break;
        }
    }
    let Some((sequence, error)) = failed else {
        return Err("hash batch unexpectedly avoided the missing-key failure; rerun to assess".into());
    };
    println!("hash_order_second_batch_failure_sequence={sequence}");
    println!("hash_order_second_batch_failure={error:?}");
    println!("backlog_missing_key_reproduced=true");

    // The sender already has a persistent session. An expired pool still blocks it.
    assert!(sender.has_session(bob.device_id()));
    let expired_now = bob_pool.expires_at_unix_seconds() + PREKEY_CLOCK_SKEW_SECONDS + 1;
    match sender.encrypt_with_pool(&alice, &bob_pool, "established but stale pool", expired_now) {
        Err(error @ RatchetError::PrekeyPoolExpired { .. }) => {
            println!("established_session_expired_pool_rejected={error:?}");
        }
        _ => return Err("expected expired pool rejection for an established session".into()),
    }
    Ok(())
}

fn main() -> Result<()> {
    run_backlog_probe()
}
