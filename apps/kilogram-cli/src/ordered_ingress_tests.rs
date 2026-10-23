use super::*;
use kilogram_protocol::RatchetRecipient;

const LABEL: &str = "internal-ordered-single-chain";

struct Fixture {
    temp: tempfile::TempDir,
    source: PathBuf,
    recipient: PathBuf,
    events: Vec<AuthorizedEvent>,
    membership: ConversationMembershipSnapshot,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    source: PathBuf,
    recipient: PathBuf,
    membership: Vec<u8>,
    expected: Vec<(EventId, String)>,
}

impl Fixture {
    fn new(count: usize, limits: Limits) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let recipient = temp.path().join("recipient");
        let alice_root = AccountRootState::create(temp.path().join("alice-root"))?;
        let bob_root = AccountRootState::create(temp.path().join("bob-root"))?;
        let alice = DeviceState::load_or_create(&source)?;
        let bob = DeviceState::load_or_create(&recipient)?;
        let alice_cert = alice_root.issue_device_certificate(
            alice.identity().device_id(),
            alice.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let bob_cert = bob_root.issue_device_certificate(
            bob.identity().device_id(),
            bob.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let alice_authority = alice_root.authority_snapshot()?;
        let bob_list = bob_root.publish_device_list(std::slice::from_ref(&bob_cert))?;
        alice.install_certificate(&alice_cert)?;
        alice.install_own_authority_snapshot(&alice_authority)?;
        bob.install_certificate(&bob_cert)?;
        bob.install_own_authority_snapshot(bob_list.authority_snapshot())?;
        bob.pin_peer_authority_snapshot(&alice_authority)?;
        let conversation = ConversationId::from_label(LABEL);
        let membership = alice_root
            .create_conversation_membership(conversation.scope_id(), &[bob_root.account_id()])?;
        alice.install_conversation_membership(&membership)?;
        bob.install_conversation_membership(&membership)?;
        let pool = RatchetState::load_or_create(&recipient)?.prekey_pool(
            bob.identity(),
            4,
            unix_time_now()?,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        // Synthetic ordinary-peer control, captured before the recipient fence.
        // It creates a real B0/L0 on the SAME selected session for the tail test.
        let control = temp.path().join("bob-native-control/ratchet");
        fs::create_dir_all(&control)?;
        for file in fs::read_dir(recipient.join("ratchet"))? {
            let file = file?;
            if file.file_type()?.is_file() {
                fs::copy(file.path(), control.join(file.file_name()))?;
            }
        }
        let mut sender = RatchetState::load_or_create(&source)?;
        let mut events = Vec::new();
        let mut parents = Vec::new();
        let mut session_id = None;
        for index in 0..count {
            let (identity, ciphertext, operation) = sender.encrypt_with_pool(
                alice.identity(),
                &pool,
                &format!("exact-{index:03}"),
                unix_time_now()?,
            )?;
            session_id = Some(operation.session_id);
            let event = AuthorizedEvent::new(
                SignedEvent::sign_ratchet_text(
                    alice.identity(),
                    conversation,
                    index as u64 * 2 + 7,
                    parents,
                    bob_list.clone(),
                    identity,
                    vec![RatchetRecipient::new(
                        bob.identity().device_id(),
                        ciphertext,
                    )?],
                )?,
                alice_cert.clone(),
                alice_authority.clone(),
            )?;
            parents = vec![event.event().event_id()?];
            events.push(event);
        }
        open_event_store(&source)?.put_authorized_batch(&events, &membership)?;
        let contact = SignedRuntimeContact::sign(
            bob.identity(),
            bob_root.account_id(),
            alice_root.account_id(),
            alice.identity().device_id(),
            LABEL.into(),
            conversation,
            RoutePolicy::DirectOnly,
            temp.path().join("synthetic-descriptor-not-dialed"),
        )?;
        let contact_path = recipient.join(runtime_contact_relative_path(contact.contact_id()));
        fs::create_dir_all(contact_path.parent().context("contact parent")?)?;
        fs::write(contact_path, contact.encode()?)?;
        let vault = EncryptedStateVault::open_or_create(&recipient)?;
        vault.migrate_legacy_snapshot()?;
        drop(vault);
        let lease = Lease::acquire(&recipient)?;
        let bob = load_command_device_state(&recipient)?;
        let header = events[0]
            .event()
            .ratchet_message_for(bob.identity().device_id())?
            .1
            .ordering_header()?;
        initialize(
            &lease,
            &bob,
            Selection {
                conversation,
                peer_account: alice_root.account_id(),
                peer_device: alice.identity().device_id(),
                recipient: bob.identity().device_id(),
                recipient_account: bob_root.account_id(),
                session_id: session_id.context("session")?,
                chain: header.ratchet_key,
                genesis: events[0].event().event_id()?,
                limits,
            },
        )?;
        lease.finish()?;
        let manifest = Manifest {
            source: source.clone(),
            recipient: recipient.clone(),
            membership: membership.encode()?,
            expected: events
                .iter()
                .enumerate()
                .map(|(index, event)| Ok((event.event().event_id()?, format!("exact-{index:03}"))))
                .collect::<Result<Vec<_>>>()?,
        };
        fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&manifest)?,
        )?;
        Ok(Self {
            temp,
            source,
            recipient,
            events,
            membership,
        })
    }

    fn resign(&self, original: &AuthorizedEvent, wrong_mac: bool) -> Result<AuthorizedEvent> {
        let alice = load_command_device_state(&self.source)?;
        let recipient = load_command_device_state(&self.recipient)?;
        let (identity, ciphertext) = original
            .event()
            .ratchet_message_for(recipient.identity().device_id())?;
        let mut ciphertext = serde_json::to_value(ciphertext)?;
        if wrong_mac {
            let bytes = ciphertext["ciphertext"]
                .as_array_mut()
                .context("cipher bytes")?;
            let last = bytes.last_mut().context("last byte")?;
            *last = serde_json::json!(last.as_u64().context("byte")? ^ 1);
        }
        let ciphertext = serde_json::from_value(ciphertext)?;
        AuthorizedEvent::new(
            SignedEvent::sign_ratchet_text(
                alice.identity(),
                original.event().conversation_id(),
                original.event().author_sequence() + 1000,
                original.event().parents().to_vec(),
                original.event().recipient_device_list()?.clone(),
                identity.clone(),
                vec![RatchetRecipient::new(
                    recipient.identity().device_id(),
                    ciphertext,
                )?],
            )?,
            original.author_certificate().clone(),
            original.author_authority_snapshot().clone(),
        )
        .map_err(Into::into)
    }
}

fn with_store<T>(
    root: &Path,
    lease: &Lease,
    action: impl FnOnce(&DecryptingSessionStore<'_>) -> Result<T>,
) -> Result<T> {
    let device = load_command_device_state(root)?;
    let events = open_event_store(root)?;
    let projections = open_local_message_store(root)?;
    let mut store = DecryptingSessionStore::new(
        root,
        &events,
        &projections,
        &device,
        device.load_certificate()?.account_id(),
        open_immutable_read_repositories(root)?,
    );
    store.ordered_lease = Some(lease);
    action(&store)
}

fn state(root: &Path) -> Result<Checkpoint> {
    read(
        root,
        load_command_device_state(root)?.identity().device_id(),
    )
}

fn native_digest(root: &Path) -> Result<blake3::Hash> {
    let vault = EncryptedStateVault::open_existing(root)?;
    let read = vault.read_mutable_primary_canary(&[StateRecordKind::Ratchet])?;
    let mut hasher = blake3::Hasher::new();
    for record in read
        .records()
        .iter()
        .filter(|record| record.relative_path() != STATE_PATH)
    {
        hasher.update(record.relative_path().as_bytes());
        hasher.update(record.content());
    }
    Ok(hasher.finalize())
}

#[test]
fn ordered_ingress_s1_pending_invalid_reopen_keeps_valid_prefix() -> Result<()> {
    let fixture = Fixture::new(3, Limits::default())?;
    let bad = fixture.resign(&fixture.events[1], true)?;
    let raw = bad.encode()?;
    let before;
    {
        let lease = Lease::acquire(&fixture.recipient)?;
        before = native_digest(&fixture.recipient)?;
        let result = with_store(&fixture.recipient, &lease, |store| {
            put(
                store,
                &lease,
                std::slice::from_ref(&bad),
                &fixture.membership,
            )
        })?;
        ensure!(result.drain == Drain::Waiting && result.durable_ids == [bad.event().event_id()?]);
        ensure!(native_digest(&fixture.recipient)? == before);
        lease.finish()?;
    }
    let lease = Lease::acquire(&fixture.recipient)?;
    let result = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &fixture.events[..1], &fixture.membership)
    })?;
    ensure!(
        result.drain
            == (Drain::Quarantined {
                index: 1,
                committed_prefix: 1
            })
    );
    ensure!(!result.complete(&[fixture.events[0].event().event_id()?]));
    let checkpoint = state(&fixture.recipient)?;
    ensure!(
        checkpoint.next == 1
            && checkpoint.readable.len() == 1
            && checkpoint.pending[&1].bytes == raw
    );
    ensure!(matches!(
        checkpoint.pending[&1].state,
        EntryState::Quarantined(_)
    ));
    let snapshot = native_digest(&fixture.recipient)?;
    let generation = checkpoint.generation;
    for _ in 0..3 {
        let retry = with_store(&fixture.recipient, &lease, |store| {
            put(store, &lease, &fixture.events[..1], &fixture.membership)
        })?;
        ensure!(
            retry.drain
                == (Drain::Quarantined {
                    index: 1,
                    committed_prefix: 0
                })
        );
        ensure!(
            native_digest(&fixture.recipient)? == snapshot
                && state(&fixture.recipient)?.generation == generation
        );
    }
    let history = collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?;
    ensure!(history.total_messages == 1 && history.messages[0].body == "exact-000");
    let ack_ids = &checkpoint.readable;
    ensure!(ack_ids.len() == 1 && !ack_ids.contains_key(&1));
    lease.finish()?;
    println!(
        "ordered_s1=passed; durable_quarantine=true; prefix=1; bad_bytes_retained=true; retry_decrypts=0; full_completion=false"
    );
    Ok(())
}

#[test]
fn ordered_ingress_s2_exact_ids_partial_and_restart() -> Result<()> {
    let limits = Limits {
        ordinary_slots: 2,
        ..Limits::default()
    };
    let fixture = Fixture::new(5, limits)?;
    let lease = Lease::acquire(&fixture.recipient)?;
    let page = [
        fixture.events[4].clone(),
        fixture.events[3].clone(),
        fixture.events[2].clone(),
    ];
    let result = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &page, &fixture.membership)
    })?;
    ensure!(
        result.durable_ids.len() == 2
            && result
                .event_error
                .as_ref()
                .is_some_and(|error| error.contains("Backpressure"))
    );
    let expected = page
        .iter()
        .map(|event| event.event().event_id())
        .collect::<Result<Vec<_>, _>>()?;
    ensure!(!result.complete(&expected));
    lease.finish()?;
    let lease = Lease::acquire(&fixture.recipient)?;
    let checkpoint = state(&fixture.recipient)?;
    ensure!(
        checkpoint.pending.len() == 2 && checkpoint.next == 0 && checkpoint.readable.is_empty()
    );
    with_store(&fixture.recipient, &lease, |store| {
        let inventory = store.inventory(
            fixture.events[0].event().conversation_id(),
            &fixture.membership,
        )?;
        ensure!(
            inventory.len() == 2
                && inventory.contains(&expected[0])
                && inventory.contains(&expected[1])
        );
        let duplicate = put(store, &lease, &page[..1], &fixture.membership)?;
        ensure!(duplicate.complete(&expected[..1]));
        ensure!(
            store.put_events(&page, &fixture.membership).is_err(),
            "partial admission must fail the real SessionStore round"
        );
        let saved = state(&fixture.recipient)?;
        ensure!(
            saved.pending[&4].bytes == page[0].encode()?
                && saved.pending[&3].bytes == page[1].encode()?
        );
        let conflict = fixture.resign(&page[0], false)?;
        ensure!(conflict.event().event_id()? != page[0].event().event_id()?);
        let conflict = put(store, &lease, &[conflict], &fixture.membership)?;
        ensure!(
            conflict.durable_ids.is_empty()
                && conflict
                    .event_error
                    .as_ref()
                    .is_some_and(|error| error.contains("Conflict"))
        );
        Ok(())
    })?;
    lease.finish()?;
    println!(
        "ordered_s2=passed; partial_round=no_SyncComplete; restart_inventory=exact_two_EventIds; ordinal_conflict_not_duplicate=true"
    );
    Ok(())
}

#[test]
fn ordered_ingress_reopen_boundaries_and_isolation() -> Result<()> {
    for point in [
        Fault::PendingCommitted,
        Fault::TrialDecrypted,
        Fault::ReadableCommitted,
    ] {
        let fixture = Fixture::new(2, Limits::default())?;
        let lease = Lease::acquire(&fixture.recipient)?;
        ensure!(matches!(
            StateDirectoryLock::acquire(&fixture.recipient),
            Err(kilogram_state::StateError::AlreadyLocked { .. })
        ));
        ensure!(matches!(
            RatchetState::load_or_create(&fixture.recipient),
            Err(kilogram_ratchet::RatchetError::OrderedIngressViolation(
                "account access fence"
            ))
        ));
        with_store(&fixture.recipient, &lease, |store| {
            let ordinary = DecryptingSessionStore::new(
                &fixture.recipient,
                store.event_writes,
                store.local_message_writes,
                store.device_state,
                store.local_account_id,
                open_immutable_read_repositories(&fixture.recipient)?,
            );
            let error = ordinary
                .put_events(&fixture.events[..1], &fixture.membership)
                .err()
                .context("unguarded SessionStore bypass")?;
            ensure!(
                matches!(error, StoreError::Io(ref cause)
                if cause.to_string().contains("exclusive ordered-ingress lease")),
                "{error:?}"
            );
            Ok(())
        })?;
        let before = native_digest(&fixture.recipient)?;
        INJECTED.with(|value| value.set(Some(point)));
        let injected = with_store(&fixture.recipient, &lease, |store| {
            store
                .put_events(&fixture.events[..1], &fixture.membership)
                .map_err(Into::into)
        })
        .err()
        .context("fault seam did not fire")?;
        ensure!(format!("{injected:#}").contains("injected ordered ingress reopen boundary"));
        drop(lease); // Reopen recovers existing transaction/primary-shadow/mirror seams.
        let lease = Lease::acquire(&fixture.recipient)?;
        let checkpoint = state(&fixture.recipient)?;
        if point == Fault::ReadableCommitted {
            ensure!(
                checkpoint.next == 1
                    && checkpoint.readable.len() == 1
                    && checkpoint.pending.is_empty()
            );
            let digest = native_digest(&fixture.recipient)?;
            with_store(&fixture.recipient, &lease, |store| {
                store
                    .put_events(&fixture.events[..1], &fixture.membership)
                    .map_err(Into::into)
            })?;
            ensure!(native_digest(&fixture.recipient)? == digest);
        } else {
            ensure!(
                checkpoint.next == 0
                    && checkpoint.pending.len() == 1
                    && checkpoint.readable.is_empty()
            );
            ensure!(native_digest(&fixture.recipient)? == before);
            with_store(&fixture.recipient, &lease, |store| {
                store
                    .put_events(&fixture.events[..1], &fixture.membership)
                    .map_err(Into::into)
            })?;
            ensure!(state(&fixture.recipient)?.next == 1);
        }
        with_store(&fixture.recipient, &lease, |store| {
            let checkpoint = state(&fixture.recipient)?;
            let receipt = &checkpoint.readable[&0];
            let acknowledgement = store.events_by_id(
                checkpoint.selection.conversation,
                &[receipt.ack],
                &fixture.membership,
            )?;
            ensure!(
                matches!(acknowledgement[0].event().payload(), EventPayload::Acknowledgement { acknowledged_event_id }
                if *acknowledged_event_id == receipt.id)
            );
            ensure!(acknowledgement[0].event().parents() == [receipt.id]);
            let stored = open_immutable_read_repositories(&fixture.recipient)?
                .events
                .load_authorized_conversation(
                    checkpoint.selection.conversation,
                    &fixture.membership,
                )?;
            ensure!(stored.len() == 2, "duplicate event/ACK intent");
            let history = collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?;
            ensure!(history.total_messages == 1 && history.messages[0].body == "exact-000");
            Ok(())
        })?;
        lease.finish()?;
    }
    println!(
        "ordered_reopen=passed; pending,trial,readable-before-ACK; actual_vault=true; double_decrypt=false; all_power_loss_windows_proven=false"
    );
    Ok(())
}

#[test]
fn ordered_ingress_s3_full_budget_and_unknown_tail() -> Result<()> {
    // Review S3: current obligations 138 fit cap 150; moving head 30 does not,
    // even though final history would occupy 144. Admission must report exhaustion.
    ensure!(138 < 150 && 144 < 150 && charge(46, 0, 30, 0) == Some(168));
    let fixture = Fixture::new(6, Limits::default())?;
    let lease = Lease::acquire(&fixture.recipient)?;
    with_store(&fixture.recipient, &lease, |store| {
        store.put_events(&fixture.events[3..], &fixture.membership)?;
        store
            .put_events(&fixture.events[..2], &fixture.membership)
            .map_err(Into::into)
    })?;
    let mut checkpoint = state(&fixture.recipient)?;
    ensure!(checkpoint.next == 2 && checkpoint.pending.len() == 3);
    let device = load_command_device_state(&fixture.recipient)?;
    let actual = EncryptedStateVault::open_existing(&fixture.recipient)?
        .verify()?
        .plaintext_bytes();
    // Account for the real existing vault and all retained entries, but leave
    // less than one full extra admission lease. This is deliberately allowed
    // to block the moving head even if the final six-message history fits.
    let blocked_limit = charge(
        actual,
        checkpoint.pending.len(),
        COMMIT_LEASE,
        MAX_CHECKPOINT as u64,
    )
    .context("budget overflow")?
        + 64 * 1024;
    checkpoint.selection.limits.vault_charge = blocked_limit;
    run_state_transaction(&fixture.recipient, |transaction| {
        write(
            &fixture.recipient,
            &mut checkpoint,
            device.identity(),
            transaction,
        )
    })?;
    let digest = native_digest(&fixture.recipient)?;
    let result = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &fixture.events[2..3], &fixture.membership)
    })?;
    ensure!(
        result.durable_ids.is_empty()
            && result
                .event_error
                .as_ref()
                .is_some_and(|error| error.contains("StorageBlocked"))
    );
    let retained = state(&fixture.recipient)?;
    ensure!(retained.next == 2 && retained.readable.len() == 2 && retained.pending.len() == 3);
    for index in 3..6 {
        ensure!(retained.pending[&(index as u64)].bytes == fixture.events[index].encode()?);
    }
    ensure!(native_digest(&fixture.recipient)? == digest);
    // K0 was received; K2 is still delayed. An ordinary synthetic Bob sends B0,
    // Alice receives it and emits valid L0 on the SAME Olm session, new DH chain.
    let mut control = RatchetState::load_or_create(fixture.temp.path().join("bob-native-control"))?;
    let alice = load_command_device_state(&fixture.source)?;
    let bob = load_command_device_state(&fixture.recipient)?;
    let (sender, k0) = fixture.events[0]
        .event()
        .ratchet_message_for(bob.identity().device_id())?;
    control.decrypt(bob.identity(), sender, k0)?;
    let mut sender_ratchet = RatchetState::load_or_create(&fixture.source)?;
    let bundle = sender_ratchet.prekey_bundle(alice.identity())?;
    let (reply_identity, b0, _) = control.encrypt(bob.identity(), &bundle, "B0")?;
    sender_ratchet.decrypt(alice.identity(), &reply_identity, &b0)?;
    let pool = SignedPrekeyPool::decode(&fs::read(
        fixture.recipient.join("ratchet/prekey-pool.bin"),
    )?)?;
    let (identity, ciphertext, operation) = sender_ratchet.encrypt_with_pool(
        alice.identity(),
        &pool,
        "unknown-tail",
        unix_time_now()?,
    )?;
    ensure!(
        operation.session_id == checkpoint.selection.session_id
            && ciphertext.ordering_header()?.ratchet_key != checkpoint.selection.chain
            && ciphertext.ordering_header()?.index == 0
    );
    let old = &fixture.events[1];
    let unknown = AuthorizedEvent::new(
        SignedEvent::sign_ratchet_text(
            alice.identity(),
            old.event().conversation_id(),
            9000,
            old.event().parents().to_vec(),
            old.event().recipient_device_list()?.clone(),
            identity,
            vec![RatchetRecipient::new(
                bob.identity().device_id(),
                ciphertext,
            )?],
        )?,
        old.author_certificate().clone(),
        old.author_authority_snapshot().clone(),
    )?;
    let result = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &[unknown], &fixture.membership)
    })?;
    ensure!(
        result
            .event_error
            .as_ref()
            .is_some_and(|error| error.contains("UnresolvedLegacyTail"))
    );
    ensure!(native_digest(&fixture.recipient)? == digest);
    // Only the fixture's explicit quota increase enables progress. Afterwards
    // verify against the REAL final vault that the previous budget did fit
    // the finished history; final size alone never proved admission capacity.
    let mut checkpoint = state(&fixture.recipient)?;
    checkpoint.selection.limits.vault_charge = Limits::default().vault_charge;
    run_state_transaction(&fixture.recipient, |transaction| {
        write(
            &fixture.recipient,
            &mut checkpoint,
            device.identity(),
            transaction,
        )
    })?;
    with_store(&fixture.recipient, &lease, |store| {
        store
            .put_events(&fixture.events[2..3], &fixture.membership)
            .map_err(Into::into)
    })?;
    let checkpoint = state(&fixture.recipient)?;
    ensure!(
        checkpoint.next == 6 && checkpoint.pending.is_empty() && checkpoint.readable.len() == 6
    );
    let final_bytes = EncryptedStateVault::open_existing(&fixture.recipient)?
        .verify()?
        .plaintext_bytes();
    let final_charge =
        charge(final_bytes, 0, COMMIT_LEASE, MAX_CHECKPOINT as u64).context("budget overflow")?;
    ensure!(
        final_charge <= blocked_limit,
        "final history did not fit the previously blocked budget"
    );
    let history = collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?;
    ensure!(history.total_messages == 6);
    lease.finish()?;
    println!(
        "ordered_s3=passed; moving_head_138_plus_30_gt_150=StorageBlocked; real_vault_pending_3_4_5_then_0_1_then_2=StorageBlocked; retained_bytes=exact; final_charge={final_charge}; blocked_limit={blocked_limit}; explicit_quota_increase_required=true; unknown_same_session_DH_tail=blocked"
    );
    Ok(())
}

#[test]
#[ignore = "subprocess worker, explicitly launched by the 256-message acceptance test"]
fn ordered_ingress_process_worker() -> Result<()> {
    let Some(directory) = std::env::var_os("KILOGRAM_ORDERED_FIXTURE_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(directory);
    let manifest: Manifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    let membership = ConversationMembershipSnapshot::decode_and_verify(&manifest.membership)?;
    let lease = Lease::acquire(&manifest.recipient)?;
    let source = load_command_device_state(&manifest.source)?;
    let recipient = load_command_device_state(&manifest.recipient)?;
    if std::env::var("KILOGRAM_ORDERED_FIXTURE_PHASE")? == "page" {
        let binding =
            SyncSessionBinding::from_transport_label("internal-real-session-process-fixture");
        let source_store = open_event_store(&manifest.source)?;
        let server = SyncServer::new(
            source.identity(),
            &source_store,
            binding,
            recipient.identity().device_id(),
            &membership,
        );
        with_store(&manifest.recipient, &lease, |store| {
            let client = SyncClient::new(
                recipient.identity(),
                store,
                ConversationId::from_label(LABEL),
                &membership,
                binding,
                source.identity().device_id(),
            );
            let round = client.begin_round()?;
            let ServerInventoryOutcome::Accepted(remote) =
                server.accept_inventory(round.inventory())?
            else {
                bail!("inventory rejected")
            };
            let page_ids = remote
                .diff()
                .events()
                .iter()
                .map(|event| event.event().event_id())
                .collect::<Result<Vec<_>, _>>()?;
            let batch = client.accept_diff(round, remote.diff().clone())?;
            let completed = server.complete_round(*remote, batch.batch().clone())?;
            let stats = client.accept_complete(batch, completed.response().clone())?;
            ensure!(stats.received_events == page_ids.len());
            let inventory = store.inventory(ConversationId::from_label(LABEL), &membership)?;
            ensure!(page_ids.iter().all(|id| inventory.contains(id)));
            println!(
                "ordered_process_page=accepted; ciphertexts={}; next={}; exact_EventIds=true",
                page_ids.len(),
                state(&manifest.recipient)?.next
            );
            Ok(())
        })?;
    } else {
        let mut cursor = None;
        let mut actual = BTreeMap::new();
        loop {
            let page = collect_runtime_history_page(
                &manifest.recipient,
                LABEL,
                cursor,
                MAX_RUNTIME_HISTORY_PAGE_SIZE,
            )?;
            ensure!(page.total_messages as usize == manifest.expected.len());
            for message in page.messages {
                actual.insert(message.event_id, message.body);
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        ensure!(actual == manifest.expected.into_iter().collect());
        ensure!(
            state(&manifest.recipient)?.pending.is_empty()
                && state(&manifest.recipient)?.next == 256
        );
        println!(
            "ordered_process_history=256_exact_plaintexts; source=encrypted-vault; API=collect_runtime_history_page; pending=0"
        );
    }
    lease.finish()?;
    Ok(())
}

#[test]
fn ordered_ingress_256_signed_hash_pages_process_restart_history() -> Result<()> {
    let fixture = Fixture::new(256, Limits::default())?;
    let executable = std::env::current_exe()?; // Stable runner path, inherited by every child.
    for phase in ["page", "page", "page", "page", "history"] {
        let output = std::process::Command::new(&executable)
            .args([
                "ordered_ingress::tests::ordered_ingress_process_worker",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("KILOGRAM_ORDERED_FIXTURE_ROOT", fixture.temp.path())
            .env("KILOGRAM_ORDERED_FIXTURE_PHASE", phase)
            .output()?;
        println!("{}", String::from_utf8_lossy(&output.stdout));
        ensure!(
            output.status.success(),
            "ordered child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!(
        "ordered_process_acceptance=passed; pages=4; ciphertexts=256; actual_child_process_reopens=5; network_runtime_actors=false"
    );
    Ok(())
}

// Real Windows storage faults, confined to the disposable fixture and the
// current test thread. Guards release the fault before transaction rollback.
#[cfg(windows)]
thread_local! {
    static FAIL_NATIVE_ACCOUNT_WRITE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SESSION_SAVE_OBSERVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_TRUST_READ_AFTER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(windows)]
pub(super) struct NativeWriteFault {
    account: PathBuf,
    permissions: fs::Permissions,
    sessions: PathBuf,
}

#[cfg(windows)]
impl Drop for NativeWriteFault {
    fn drop(&mut self) {
        SESSION_SAVE_OBSERVED.with(|observed| {
            observed.set(fs::read_dir(&self.sessions).is_ok_and(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .any(|entry| entry.path().is_file())
            }));
        });
        let _ = fs::set_permissions(&self.account, self.permissions.clone());
    }
}

#[cfg(windows)]
pub(super) fn native_write_fault(root: &Path) -> Result<Option<NativeWriteFault>> {
    if !FAIL_NATIVE_ACCOUNT_WRITE.with(|armed| armed.replace(false)) {
        return Ok(None);
    }
    let account = root.join("ratchet/account.pickle");
    let permissions = fs::metadata(&account)?.permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&account, read_only)?;
    Ok(Some(NativeWriteFault {
        account,
        permissions,
        sessions: root.join("ratchet/sessions"),
    }))
}

#[cfg(windows)]
pub(super) fn trust_read_fault(root: &Path) -> Result<Option<fs::File>> {
    use std::os::windows::fs::OpenOptionsExt;
    let armed = FAIL_TRUST_READ_AFTER.with(|value| match value.get() {
        Some(0) => {
            value.set(None);
            true
        }
        Some(remaining) => {
            value.set(Some(remaining - 1));
            false
        }
        None => false,
    });
    if !armed {
        return Ok(None);
    }
    // The repository really reopens the encrypted vault and encounters a
    // sharing violation while reading its key; no synthetic bad-author error.
    Ok(Some(
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0x2 | 0x4)
            .open(root.join("state-vault.key"))?,
    ))
}

#[cfg(windows)]
fn operational_fault_reopens_without_quarantine(native_write: bool) -> Result<()> {
    let count = if native_write { 1 } else { 2 };
    let fixture = Fixture::new(count, Limits::default())?;
    let lease = Lease::acquire(&fixture.recipient)?;
    INJECTED.with(|value| value.set(Some(Fault::PendingCommitted)));
    let admitted = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &fixture.events, &fixture.membership)
    });
    ensure!(admitted.is_err_and(|error| {
        format!("{error:#}").contains("injected ordered ingress reopen boundary")
    }));
    drop(lease);
    let lease = Lease::acquire(&fixture.recipient)?;
    let before = native_digest(&fixture.recipient)?;
    let generation = EncryptedStateVault::open_existing(&fixture.recipient)?
        .verify()?
        .mirror_generation();
    if native_write {
        SESSION_SAVE_OBSERVED.with(|value| value.set(false));
        FAIL_NATIVE_ACCOUNT_WRITE.with(|value| value.set(true));
    } else {
        // K0 and its native/projection writes succeed in this transaction;
        // the second pending entry encounters the transient local read error.
        FAIL_TRUST_READ_AFTER.with(|value| value.set(Some(1)));
    }
    let result = with_store(&fixture.recipient, &lease, |store| {
        put(store, &lease, &[], &fixture.membership)
    });
    if native_write {
        let saved = SESSION_SAVE_OBSERVED.with(|value| value.get());
        println!("ordered_i1_native_session_save_observed={saved}");
        ensure!(saved, "fault did not follow real session save");
        ensure!(
            !fs::metadata(fixture.recipient.join("ratchet/account.pickle"))?
                .permissions()
                .readonly()
        );
    }
    println!("ordered_i1_operational_result={result:?}");
    ensure!(
        result.is_err(),
        "operational error became successful quarantine commit"
    );
    drop(lease);
    let lease = Lease::acquire(&fixture.recipient)?;
    let checkpoint = state(&fixture.recipient)?;
    ensure!(
        checkpoint.next == 0 && checkpoint.readable.is_empty() && checkpoint.pending.len() == count
    );
    for (index, event) in fixture.events.iter().enumerate() {
        let entry = &checkpoint.pending[&(index as u64)];
        ensure!(entry.state == EntryState::Pending && entry.bytes == event.encode()?);
    }
    ensure!(native_digest(&fixture.recipient)? == before);
    ensure!(
        EncryptedStateVault::open_existing(&fixture.recipient)?
            .verify()?
            .mirror_generation()
            == generation
    );
    ensure!(
        collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?.total_messages == 0
    );
    with_store(&fixture.recipient, &lease, |store| {
        ensure!(
            store
                .inventory(checkpoint.selection.conversation, &fixture.membership)?
                .len()
                == count
        );
        store.put_events(&[], &fixture.membership)?;
        let after = native_digest(&fixture.recipient)?;
        store.put_events(&fixture.events, &fixture.membership)?;
        ensure!(native_digest(&fixture.recipient)? == after);
        let events = open_immutable_read_repositories(&fixture.recipient)?
            .events
            .load_authorized_conversation(checkpoint.selection.conversation, &fixture.membership)?;
        ensure!(events.len() == count * 2, "duplicate or missing text/ACK");
        Ok(())
    })?;
    let history = collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?;
    ensure!(history.total_messages as usize == count);
    let actual: BTreeMap<_, _> = history
        .messages
        .iter()
        .map(|message| (message.event_id, message.body.clone()))
        .collect();
    let expected: BTreeMap<_, _> = fixture
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| Ok((event.event().event_id()?, format!("exact-{index:03}"))))
        .collect::<Result<_>>()?;
    ensure!(actual == expected);
    lease.finish()?;
    println!(
        "ordered_i1=passed; native_write_fault={native_write}; rollback_native_prefix=true; pending_exact=true; quarantine=false; retry_exact_once=true"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn ordered_ingress_i1_session_saved_then_account_write_fails() -> Result<()> {
    operational_fault_reopens_without_quarantine(true)
}

#[cfg(windows)]
#[test]
fn ordered_ingress_i1_trust_read_fault_after_valid_prefix() -> Result<()> {
    operational_fault_reopens_without_quarantine(false)
}

#[test]
fn ordered_ingress_i3_head_spends_byte_reserve_and_hard_cap_holds() -> Result<()> {
    let fixture = Fixture::new(5, Limits::default())?;
    let lease = Lease::acquire(&fixture.recipient)?;
    let device = load_command_device_state(&fixture.recipient)?;
    let future_bytes = fixture.events[1].encode()?.len();
    let head_bytes = fixture.events[0].encode()?.len();
    let cap = future_bytes + MAX_EVENT;
    let set_cap = |limit: usize| -> Result<()> {
        let mut checkpoint = state(&fixture.recipient)?;
        checkpoint.selection.limits.pending_bytes = limit;
        run_state_transaction(&fixture.recipient, |transaction| {
            write(
                &fixture.recipient,
                &mut checkpoint,
                device.identity(),
                transaction,
            )
        })
    };
    set_cap(cap)?;
    with_store(&fixture.recipient, &lease, |store| {
        store.put_events(&fixture.events[1..2], &fixture.membership)?;
        let ordinary = put(store, &lease, &fixture.events[2..3], &fixture.membership)?;
        ensure!(ordinary.event_error.is_some() && ordinary.durable_ids.is_empty());
        Ok(())
    })?;
    set_cap(future_bytes + head_bytes - 1)?;
    with_store(&fixture.recipient, &lease, |store| {
        let overflow = put(store, &lease, &fixture.events[..1], &fixture.membership)?;
        ensure!(overflow.event_error.is_some() && overflow.durable_ids.is_empty());
        ensure!(state(&fixture.recipient)?.pending[&1].bytes == fixture.events[1].encode()?);
        Ok(())
    })?;
    set_cap(cap)?;
    with_store(&fixture.recipient, &lease, |store| {
        store.put_events(&fixture.events[..1], &fixture.membership)?;
        ensure!(state(&fixture.recipient)?.next == 2);
        store.put_events(&fixture.events[3..4], &fixture.membership)?;
        let ordinary = put(store, &lease, &fixture.events[4..5], &fixture.membership)?;
        ensure!(ordinary.event_error.is_some() && ordinary.durable_ids.is_empty());
        ensure!(state(&fixture.recipient)?.pending[&3].bytes == fixture.events[3].encode()?);
        Ok(())
    })?;
    ensure!(
        collect_runtime_history_page(&fixture.recipient, LABEL, None, 100)?.total_messages == 2
    );
    lease.finish()?;
    println!(
        "ordered_i3=passed; pending_cap={cap}; future_bytes={future_bytes}; head_bytes={head_bytes}; head_uses_reserve=true; hard_cap_holds=true; later_future_reserve=true"
    );
    Ok(())
}

#[tokio::test]
async fn ordered_ingress_i4_runtime_profile_refuses_before_authority_mutation() -> Result<()> {
    for recover_shadow in [false, true] {
        let fixture = Fixture::new(1, Limits::default())?;
        let bob = load_command_device_state(&fixture.recipient)?;
        let root = AccountRootState::load(fixture.temp.path().join("bob-root"))?;
        let newer_device = DeviceState::load_or_create(fixture.temp.path().join("newer-device"))?;
        let newer_cert = root.issue_device_certificate(
            newer_device.identity().device_id(),
            newer_device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let own = CommandTrustReadRepository::open(&fixture.recipient, &bob)?;
        let cert = own.load_certificate()?;
        let old_authority = own.load_own_authority_snapshot(&cert)?.encode()?;
        let newer_list = root.publish_device_list(&[cert, newer_cert])?;
        ensure!(newer_list.authority_snapshot().encode()? != old_authority);
        let device_list_file = fixture.temp.path().join("newer-device-list.snapshot");
        fs::write(&device_list_file, newer_list.encode()?)?;
        let profile_file = fixture.temp.path().join("runtime.launch.json");
        let ipc = fixture.temp.path().join("runtime.ipc.json");
        let ticket = fixture.temp.path().join("runtime.ticket");
        create_runtime_launch_profile(
            profile_file.clone(),
            RuntimeLaunchSettings {
                state_dir: fixture.recipient.clone(),
                allowed_requester_account_id: root.account_id(),
                device_list_file,
                peer_prekey_pool_files: Vec::new(),
                ticket_file: Some(ticket.clone()),
                relay_wait_seconds: 0,
                route_policy: RuntimeIpcRoutePolicy::DirectOnly,
                relay_url: None,
                poll_milliseconds: 20,
                retry_base_seconds: 1,
                retry_max_seconds: 1,
                auto_sync_seconds: 0,
                ipc_file: ipc.clone(),
                volunteer_storage: None,
            },
        )?;
        if recover_shadow {
            let lease = Lease::acquire(&fixture.recipient)?;
            let mut checkpoint = state(&fixture.recipient)?;
            let mut transaction = StateTransaction::begin(&fixture.recipient)?;
            {
                let mut context =
                    CommandTransactionContext::new(&fixture.recipient, &mut transaction);
                write(
                    &fixture.recipient,
                    &mut checkpoint,
                    bob.identity(),
                    &mut context,
                )?;
            }
            let primary = PendingVaultPrimaryWrite::prepare(&fixture.recipient, &transaction)?
                .context("no real vault-primary commit")?;
            transaction.commit()?;
            drop(primary); // Existing committed-primary, unconfirmed-shadow recovery seam.
            drop(lease);
            fs::remove_file(fixture.recipient.join("ratchet/account.pickle"))?;
            ensure!(!RatchetState::is_ordered_ingress(&fixture.recipient)?);
        }
        let before = EncryptedStateVault::open_existing(&fixture.recipient)?.verify()?;
        let discovery_directory = fixture.temp.path().join("kilogram-discovered-own-devices");
        ensure!(!discovery_directory.exists());
        let result = crate::run_command(Command::RuntimeFromProfile { profile_file }).await;
        let after = EncryptedStateVault::open_existing(&fixture.recipient)?.verify()?;
        let own_after = CommandTrustReadRepository::open(&fixture.recipient, &bob)?
            .load_own_authority_snapshot(&own.load_certificate()?)?
            .encode()?;
        println!(
            "ordered_i4_recover_shadow={recover_shadow}; generation_before={}; generation_after={}; authority_changed={}",
            before.mirror_generation(),
            after.mirror_generation(),
            own_after != old_authority
        );
        ensure!(
            before.mirror_generation() == after.mirror_generation()
                && before.snapshot_id() == after.snapshot_id()
        );
        ensure!(own_after == old_authority);
        ensure!(RatchetState::is_ordered_ingress(&fixture.recipient)?);
        let error = result.err().context("protected runtime was allowed")?;
        ensure!(
            format!("{error:#}").contains("internal ordered profile forbids runtime preparation")
        );
        ensure!(
            !ipc.exists() && !ticket.exists() && !discovery_directory.exists(),
            "network/runtime startup was reached"
        );
    }
    println!(
        "ordered_i4=passed; newer_valid_authority_not_installed=true; recovered_tag_checked=true; endpoint_started=false"
    );
    Ok(())
}
