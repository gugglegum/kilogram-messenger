//! Internal, disposable, known-genesis single-chain integration. No profile migration.
//! The lease excludes every other CLI writer; the account fence rejects legacy ratchets.
use super::*;
use kilogram_protocol::EventId;
use kilogram_ratchet::SingleChainReceiveGuard;

const STATE_PATH: &str = "ratchet/ordered-ingress.state";
const DOMAIN: &[u8] = b"kilogram:internal-ordered-ingress-state:v1\0";
const MAX_CHECKPOINT: usize = 4 * 1024 * 1024;
const MAX_EVENT: usize = 8 * 1024;
const MAX_CIPHERTEXT: usize = 2 * 1024;
const COMMIT_LEASE: u64 = 256 * 1024;

// Existing CLI commands on an already protected disposable profile. Every other
// writer/transport path is refused; ordinary profiles never enter this branch.
pub(super) async fn run_command(command: Command, lock: StateDirectoryLock) -> Result<()> {
    let lease = Lease::adopt(lock)?;
    let root = lease.lock.root();
    let device = load_command_device_state(root)?;
    let selected = read(root, device.identity().device_id())?.selection;
    let result = async { match command {
        Command::Sync {
            state_dir,
            ticket,
            ticket_file,
            conversation,
            max_rounds,
            expect_account,
        } => {
            ensure!(
                ConversationId::from_label(&conversation) == selected.conversation
                    && expect_account == selected.peer_account,
                "unsupported protected sync context"
            );
            sync_with_ordered_lease(
                state_dir,
                ticket,
                ticket_file,
                conversation,
                max_rounds,
                expect_account,
                Some(&lease),
            )
            .await
            .map(|_| ())
        }
        Command::History {
            state_dir,
            conversation,
        } => {
            ensure!(
                ConversationId::from_label(&conversation) == selected.conversation,
                "unsupported protected history context"
            );
            show_history(state_dir, conversation)
        }
        _ => Err(anyhow::anyhow!(
            "internal single-chain profile supports only ordered Sync and readable History; direct/mailbox/runtime/other writers forbidden"
        )),
    } }.await;
    combine_operation_and_mirror(result, lease.finish())
}

pub(super) fn require_readable_convergence(root: &Path) -> Result<()> {
    let device = load_command_device_state(root)?;
    let checkpoint = read(root, device.identity().device_id())?;
    ensure!(
        checkpoint.pending.is_empty(),
        "ordered sync has durable pending/quarantine, not readable convergence"
    );
    Ok(())
}

pub(super) fn verify_selected_peer(root: &Path, peer: DeviceId) -> Result<()> {
    let device = load_command_device_state(root)?;
    ensure!(
        read(root, device.identity().device_id())?
            .selection
            .peer_device
            == peer,
        "unsupported peer device on protected session"
    );
    Ok(())
}

pub(super) struct Lease {
    lock: StateDirectoryLock,
    mirror: Option<VaultDualWriteGuard>,
}

impl Lease {
    #[cfg(test)]
    pub(super) fn acquire(root: &Path) -> Result<Self> {
        Self::adopt(StateDirectoryLock::acquire(root)?)
    }

    fn adopt(lock: StateDirectoryLock) -> Result<Self> {
        let mirror = VaultDualWriteGuard::prepare(lock.root())?
            .context("ordered ingress requires an encrypted vault")?;
        Ok(Self {
            lock,
            mirror: Some(mirror),
        })
    }

    pub(super) fn finish(mut self) -> Result<()> {
        self.mirror
            .take()
            .context("ordered lease already finished")?
            .finish()
    }

    fn check(&self, root: &Path) -> Result<()> {
        ensure!(
            self.lock.root() == fs::canonicalize(root)?,
            "ordered lease root mismatch"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Limits {
    pub(super) ordinary_slots: usize,
    pub(super) pending_bytes: usize,
    pub(super) vault_charge: u64,
    pub(super) transient_charge: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            ordinary_slots: 512,
            pending_bytes: 2 * 1024 * 1024,
            vault_charge: 128 * 1024 * 1024,
            transient_charge: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Selection {
    pub(super) conversation: ConversationId,
    pub(super) peer_account: AccountId,
    pub(super) peer_device: DeviceId,
    pub(super) recipient: DeviceId,
    pub(super) recipient_account: AccountId,
    pub(super) session_id: String,
    pub(super) chain: [u8; 32],
    pub(super) genesis: EventId,
    pub(super) limits: Limits,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum EntryState {
    Pending,
    Quarantined(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pending {
    id: EventId,
    bytes: Vec<u8>,
    state: EntryState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Receipt {
    id: EventId,
    ack: EventId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Checkpoint {
    selection: Selection,
    generation: u64,
    next: u64,
    pending: BTreeMap<u64, Pending>,
    readable: BTreeMap<u64, Receipt>,
}

#[derive(Serialize, Deserialize)]
struct SignedCheckpoint {
    device: DeviceId,
    checkpoint: Checkpoint,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Drain {
    Waiting,
    Readable(usize),
    Quarantined { index: u64, committed_prefix: usize },
    StorageBlocked { committed_prefix: usize },
}

#[derive(Debug)]
pub(super) struct BatchResult {
    pub(super) durable_ids: Vec<EventId>,
    pub(super) event_error: Option<String>,
    pub(super) drain: Drain,
}

impl BatchResult {
    fn complete(&self, expected: &[EventId]) -> bool {
        self.event_error.is_none()
            && self.durable_ids == expected
            && !matches!(
                self.drain,
                Drain::Quarantined { .. } | Drain::StorageBlocked { .. }
            )
    }
}

fn signed_bytes(checkpoint: &Checkpoint) -> Result<Vec<u8>> {
    let mut bytes = DOMAIN.to_vec();
    bytes.extend(postcard::to_allocvec(checkpoint)?);
    Ok(bytes)
}

fn decode(bytes: &[u8], recipient: DeviceId) -> Result<Checkpoint> {
    ensure!(
        bytes.len() <= MAX_CHECKPOINT,
        "ordered checkpoint too large"
    );
    let signed: SignedCheckpoint = postcard::from_bytes(bytes)?;
    ensure!(
        signed.device == recipient && signed.checkpoint.selection.recipient == recipient,
        "ordered checkpoint identity mismatch"
    );
    recipient.verify(&signed_bytes(&signed.checkpoint)?, &signed.signature)?;
    let checkpoint = signed.checkpoint;
    ensure!(
        checkpoint.next <= 512 && checkpoint.readable.len() == checkpoint.next as usize,
        "ordered checkpoint frontier mismatch"
    );
    ensure!(
        checkpoint.pending.len() <= checkpoint.selection.limits.ordinary_slots + 1
            && checkpoint.selection.limits.ordinary_slots <= 512
            && checkpoint.selection.limits.pending_bytes <= 2 * 1024 * 1024
            && checkpoint.selection.limits.vault_charge <= 128 * 1024 * 1024
            && checkpoint.selection.limits.transient_charge <= 512 * 1024 * 1024,
        "ordered checkpoint limits"
    );
    Ok(checkpoint)
}

fn read(root: &Path, recipient: DeviceId) -> Result<Checkpoint> {
    let vault = EncryptedStateVault::open_existing(root)?;
    let primary = vault.read_mutable_primary_canary(&[StateRecordKind::Ratchet])?;
    let bytes = primary
        .records()
        .iter()
        .find(|record| record.relative_path() == STATE_PATH)
        .context("protected account has no authenticated ordered checkpoint")?
        .content();
    decode(bytes, recipient)
}

fn write(
    root: &Path,
    checkpoint: &mut Checkpoint,
    device: &DeviceIdentity,
    transaction: &mut CommandTransactionContext<'_>,
) -> Result<()> {
    transaction.prepare_ratchet_workspace()?;
    checkpoint.generation = checkpoint
        .generation
        .checked_add(1)
        .context("ordered generation exhausted")?;
    let signature = device.sign(&signed_bytes(checkpoint)?).to_vec();
    let bytes = postcard::to_allocvec(&SignedCheckpoint {
        device: device.device_id(),
        checkpoint: checkpoint.clone(),
        signature,
    })?;
    ensure!(bytes.len() <= MAX_CHECKPOINT, "ordered checkpoint capacity");
    let path = root.join(STATE_PATH);
    let parent = path.parent().context("ordered parent")?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)?
        .sync_all()?;
    Ok(())
}

fn directory_bytes(root: &Path) -> Result<u64> {
    let mut total = 0_u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        ensure!(
            !entry.file_type()?.is_symlink(),
            "ordered profile contains symlink"
        );
        total = total
            .checked_add(if meta.is_dir() {
                directory_bytes(&entry.path())?
            } else {
                meta.len()
            })
            .context("ordered disk accounting overflow")?;
    }
    Ok(total)
}

// This includes a complete future commit for each accepted entry and an additional
// moving head, plus a checkpoint allowance. It is conservative logical/staging
// admission, NOT a promise about future OS disk availability or redb allocation.
fn charge(base: u64, entries: usize, lease: u64, control: u64) -> Option<u64> {
    base.checked_mul(3)?
        .checked_add((entries as u64).checked_add(1)?.checked_mul(lease)?)?
        .checked_add(control)
}

fn has_capacity(root: &Path, checkpoint: &Checkpoint, entries: usize) -> Result<bool> {
    let logical = EncryptedStateVault::open_existing(root)?
        .verify()?
        .plaintext_bytes();
    let physical_files = directory_bytes(root)?;
    Ok(
        charge(logical, entries, COMMIT_LEASE, MAX_CHECKPOINT as u64)
            .is_some_and(|bytes| bytes <= checkpoint.selection.limits.vault_charge)
            && charge(physical_files, entries, COMMIT_LEASE, MAX_CHECKPOINT as u64)
                .is_some_and(|bytes| bytes <= checkpoint.selection.limits.transient_charge),
    )
}

fn validate_event(
    event: &AuthorizedEvent,
    selection: &Selection,
    membership: &ConversationMembershipSnapshot,
) -> Result<kilogram_ratchet::RatchetOrderingHeader> {
    event.verify_for_membership(membership)?;
    ensure!(
        event.author_account_id() == selection.peer_account
            && event.event().author_device_id() == selection.peer_device
            && event.event().conversation_id() == selection.conversation,
        "unsupported author/conversation on protected session"
    );
    ensure!(
        event.event().recipient_device_list()?.account_id() == selection.recipient_account
            && event.event().ratchet_recipients()?.len() == 1,
        "internal ingress requires one recipient"
    );
    let (_, ciphertext) = event.event().ratchet_message_for(selection.recipient)?;
    ensure!(
        ciphertext.ciphertext().len() <= MAX_CIPHERTEXT,
        "StorageBlocked: oversized legacy ciphertext"
    );
    let header = ciphertext.ordering_header()?;
    ensure!(
        header.ratchet_key == selection.chain,
        "UnresolvedLegacyTail: unknown DH chain"
    );
    ensure!(
        header
            .prekey_session_id
            .as_ref()
            .is_none_or(|id| id == &selection.session_id),
        "unsupported Olm session"
    );
    ensure!(header.index < 512, "internal single-chain lifetime bound");
    if header.index == 0 {
        ensure!(
            event.event().event_id()? == selection.genesis,
            "different genesis"
        );
    }
    Ok(header)
}

fn current_peer_authority(
    root: &Path,
    device: &DeviceState,
    account: AccountId,
) -> Result<AccountAuthoritySnapshot> {
    #[cfg(all(test, windows))]
    let _read_fault = tests::trust_read_fault(root)?;
    let trust = CommandTrustReadRepository::open(root, device)?;
    trust.load_peer_authority_snapshot(account)
}

fn check_authority(
    event: &AuthorizedEvent,
    pinned: &AccountAuthoritySnapshot,
) -> std::result::Result<(), kilogram_identity::IdentityError> {
    verify_device_authorization_with_snapshot(
        event.author_account_id(),
        event.author_certificate(),
        pinned,
        &[DeviceCapability::SignEvents],
    )?;
    Ok(())
}

fn authorize_current(root: &Path, device: &DeviceState, event: &AuthorizedEvent) -> Result<()> {
    let pinned = current_peer_authority(root, device, event.author_account_id())?;
    check_authority(event, &pinned)?;
    Ok(())
}

pub(super) fn put(
    store: &DecryptingSessionStore<'_>,
    lease: &Lease,
    events: &[AuthorizedEvent],
    membership: &ConversationMembershipSnapshot,
) -> Result<BatchResult> {
    lease.check(&store.state_directory)?;
    let root = &store.state_directory;
    let device = store.device_state;
    let mut checkpoint = read(root, device.identity().device_id())?;
    ensure!(
        checkpoint.selection.recipient_account == store.local_account_id,
        "protected recipient account mismatch"
    );
    let own_trust = CommandTrustReadRepository::open(root, device)?;
    let own_certificate = own_trust.load_certificate()?;
    let own_authority = own_trust.load_own_authority_snapshot(&own_certificate)?;
    ensure!(
        own_certificate.encode()?.len() + own_authority.encode()?.len() <= MAX_EVENT,
        "StorageBlocked: own ACK authorization exceeds the internal commit envelope bound"
    );
    let mut durable_ids = Vec::new();
    let mut event_error = None;
    // One bounded admission transaction. A quota/conflict commits the exact accepted prefix,
    // then returns an explicit failed round. No subset SyncComplete is constructed.
    let mut changed = false;
    for event in events {
        let raw = event.encode()?;
        let id = event.event().event_id()?;
        let candidate = (|| -> Result<bool> {
            ensure!(
                raw.len() <= MAX_EVENT,
                "StorageBlocked: oversized legacy event"
            );
            let header = validate_event(event, &checkpoint.selection, membership)?;
            authorize_current(root, device, event)?;
            if let Some(receipt) = checkpoint.readable.get(&header.index) {
                ensure!(receipt.id == id, "Conflict: ordinal is not EventId");
                let existing = store.event_writes.authorized_events_by_id(
                    checkpoint.selection.conversation,
                    &[id],
                    membership,
                )?;
                ensure!(
                    existing[0].encode()? == raw,
                    "conflicting authorization bytes"
                );
                return Ok(false);
            }
            if let Some(existing) = checkpoint.pending.get(&header.index) {
                ensure!(
                    existing.id == id && existing.bytes == raw,
                    "Conflict: ordinal is not EventId"
                );
                return Ok(false);
            }
            let ordinary = checkpoint
                .pending
                .keys()
                .filter(|index| **index != checkpoint.next)
                .count();
            ensure!(
                header.index == checkpoint.next
                    || ordinary < checkpoint.selection.limits.ordinary_slots,
                "Backpressure: ordinary pending capacity"
            );
            let pending_bytes: usize = checkpoint
                .pending
                .values()
                .map(|entry| entry.bytes.len())
                .sum();
            let head_reserve = if header.index == checkpoint.next {
                0
            } else {
                MAX_EVENT
            };
            ensure!(
                pending_bytes
                    .checked_add(raw.len())
                    .and_then(|bytes| bytes.checked_add(head_reserve))
                    .is_some_and(|bytes| bytes <= checkpoint.selection.limits.pending_bytes),
                "Backpressure: pending bytes/head reserve"
            );
            ensure!(
                has_capacity(root, &checkpoint, checkpoint.pending.len() + 1)?,
                "StorageBlocked: full commit reserve"
            );
            checkpoint.pending.insert(
                header.index,
                Pending {
                    id,
                    bytes: raw,
                    state: EntryState::Pending,
                },
            );
            Ok(true)
        })();
        match candidate {
            Ok(inserted) => {
                changed |= inserted;
                durable_ids.push(id);
            }
            Err(error) => {
                event_error = Some(format!("{error:#}"));
                break;
            }
        }
    }
    if changed {
        run_state_transaction(root, |transaction| {
            write(root, &mut checkpoint, device.identity(), transaction)
        })?;
    }
    fault(Fault::PendingCommitted)?;
    let drain = drain(store, &mut checkpoint, membership)?;
    Ok(BatchResult {
        durable_ids,
        event_error,
        drain,
    })
}

fn drain(
    store: &DecryptingSessionStore<'_>,
    checkpoint: &mut Checkpoint,
    membership: &ConversationMembershipSnapshot,
) -> Result<Drain> {
    let root = &store.state_directory;
    if checkpoint
        .pending
        .get(&checkpoint.next)
        .is_some_and(|entry| matches!(entry.state, EntryState::Quarantined(_)))
    {
        return Ok(Drain::Quarantined {
            index: checkpoint.next,
            committed_prefix: 0,
        });
    }
    if !checkpoint.pending.contains_key(&checkpoint.next) {
        return Ok(Drain::Waiting);
    }
    if !has_capacity(root, checkpoint, checkpoint.pending.len())? {
        return Ok(Drain::StorageBlocked {
            committed_prefix: 0,
        });
    }
    let device = store.device_state;
    let trust = CommandTrustReadRepository::open(root, device)?;
    let certificate = trust.load_certificate()?;
    let authority = trust.load_own_authority_snapshot(&certificate)?;
    let result = run_state_transaction(root, |transaction| {
        transaction.prepare_ratchet_workspace()?;
        let mut ratchet = RatchetState::load_for_ordered_ingress(root)?;
        let mut readable = Vec::new();
        let mut acknowledgements = Vec::new();
        let mut blocked = None;
        while let Some(entry) = checkpoint.pending.get(&checkpoint.next).cloned() {
            if let EntryState::Quarantined(_) = entry.state {
                blocked = Some(checkpoint.next);
                break;
            }
            let validated = (|| -> Result<AuthorizedEvent> {
                let event = AuthorizedEvent::decode_and_verify_author(&entry.bytes)?;
                validate_event(&event, &checkpoint.selection, membership)?;
                Ok(event)
            })();
            let event = match validated {
                Ok(event) => event,
                Err(error) => {
                    checkpoint
                        .pending
                        .get_mut(&checkpoint.next)
                        .context("missing quarantine entry")?
                        .state =
                        EntryState::Quarantined(format!("authorization/encoding: {error:#}"));
                    blocked = Some(checkpoint.next);
                    break;
                }
            };
            // A failure loading local trust (including corrupt/missing records)
            // is operational, regardless of its underlying error variant.
            let pinned = current_peer_authority(root, device, event.author_account_id())?;
            if let Err(error) = check_authority(&event, &pinned) {
                checkpoint
                    .pending
                    .get_mut(&checkpoint.next)
                    .context("missing quarantine entry")?
                    .state = EntryState::Quarantined(format!("authorization: {error}"));
                blocked = Some(checkpoint.next);
                break;
            }
            let (sender, ciphertext) = event
                .event()
                .ratchet_message_for(device.identity().device_id())?;
            let guard = SingleChainReceiveGuard {
                peer_device_id: checkpoint.selection.peer_device,
                session_id: checkpoint.selection.session_id.clone(),
                ratchet_key: checkpoint.selection.chain,
                next_index: checkpoint.next,
            };
            #[cfg(all(test, windows))]
            let write_fault = tests::native_write_fault(root)?;
            let decrypted =
                ratchet.decrypt_ordered_single_chain(device.identity(), sender, ciphertext, &guard);
            #[cfg(all(test, windows))]
            drop(write_fault);
            let decrypted = match decrypted {
                Ok((decrypted, _)) => decrypted,
                Err(error) if error.is_invalid_ordered_input() => {
                    checkpoint
                        .pending
                        .get_mut(&checkpoint.next)
                        .context("missing quarantine entry")?
                        .state = EntryState::Quarantined(format!("{error}"));
                    blocked = Some(checkpoint.next);
                    break;
                }
                Err(error) => {
                    return Err(anyhow::Error::new(error).context(
                        "ordered ingress operational failure; roll back native workspace",
                    ));
                }
            };
            fault(Fault::TrialDecrypted)?;
            let (_, projection) = ensure_received_local_text_projection(
                store.local_message_writes,
                device,
                store.local_account_id,
                event.event(),
                &decrypted,
            )?;
            transaction.register_store_receipt(&projection)?;
            let ack = AuthorizedEvent::new(
                SignedEvent::sign_acknowledgement(
                    device.identity(),
                    checkpoint.selection.conversation,
                    transaction.allocate_sequence(device)?,
                    vec![entry.id],
                    entry.id,
                )?,
                certificate.clone(),
                authority.clone(),
            )?;
            let ack_id = ack.event().event_id()?;
            checkpoint.readable.insert(
                checkpoint.next,
                Receipt {
                    id: entry.id,
                    ack: ack_id,
                },
            );
            checkpoint.pending.remove(&checkpoint.next);
            checkpoint.next += 1;
            readable.push(event);
            acknowledgements.push(ack);
        }
        // The invalid trial never persisted native session/account files. A valid prefix and
        // its projections, exact events, ACK intents and quarantine commit together here.
        let count = readable.len();
        readable.extend(acknowledgements);
        let receipt = store
            .event_writes
            .put_authorized_batch_with_receipt(&readable, membership)?;
        transaction.register_store_receipt(&receipt)?;
        write(root, checkpoint, device.identity(), transaction)?;
        Ok(match blocked {
            Some(index) => Drain::Quarantined {
                index,
                committed_prefix: count,
            },
            None => Drain::Readable(count),
        })
    })?;
    fault(Fault::ReadableCommitted)?;
    Ok(result)
}

pub(super) fn put_for_session(
    store: &DecryptingSessionStore<'_>,
    lease: &Lease,
    events: &[AuthorizedEvent],
    membership: &ConversationMembershipSnapshot,
) -> Result<()> {
    let result = put(store, lease, events, membership)?;
    let expected = events
        .iter()
        .map(|event| event.event().event_id())
        .collect::<Result<Vec<_>, _>>()?;
    ensure!(
        result.complete(&expected),
        "ordered ingress incomplete round: {result:?}; no SyncComplete"
    );
    Ok(())
}

pub(super) fn inventory(
    root: &Path,
    recipient: DeviceId,
    conversation: ConversationId,
    membership: &ConversationMembershipSnapshot,
) -> Result<Vec<EventId>> {
    let checkpoint = read(root, recipient)?;
    ensure!(
        checkpoint.selection.conversation == conversation,
        "unsupported protected conversation"
    );
    let mut ids = open_immutable_read_repositories(root)?
        .events
        .authorized_inventory(conversation, membership)?;
    ids.extend(checkpoint.pending.values().map(|entry| entry.id));
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub(super) fn events(
    root: &Path,
    recipient: DeviceId,
    conversation: ConversationId,
    ids: &[EventId],
    membership: &ConversationMembershipSnapshot,
) -> Result<Vec<AuthorizedEvent>> {
    let checkpoint = read(root, recipient)?;
    ensure!(
        checkpoint.selection.conversation == conversation,
        "unsupported protected conversation"
    );
    let repository = open_immutable_read_repositories(root)?;
    ids.iter()
        .map(|id| {
            if let Some(entry) = checkpoint.pending.values().find(|entry| &entry.id == id) {
                let event = AuthorizedEvent::decode_and_verify_author(&entry.bytes)?;
                event.verify_for_membership(membership)?;
                Ok(event)
            } else {
                repository
                    .events
                    .authorized_events_by_id(conversation, &[*id], membership)?
                    .pop()
                    .context("missing protected stored event")
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    PendingCommitted,
    TrialDecrypted,
    ReadableCommitted,
}

#[cfg(test)]
thread_local! { static INJECTED: std::cell::Cell<Option<Fault>> = const { std::cell::Cell::new(None) }; }

fn fault(point: Fault) -> Result<()> {
    #[cfg(test)]
    if INJECTED.with(|value| value.get() == Some(point)) {
        INJECTED.with(|value| value.set(None));
        bail!("injected ordered ingress reopen boundary");
    }
    let _ = point;
    Ok(())
}

// There is intentionally no production CLI activation/migration command.
#[cfg(test)]
fn initialize(lease: &Lease, device: &DeviceState, selection: Selection) -> Result<()> {
    let root = lease.lock.root();
    let read = open_immutable_read_repositories(root)?;
    ensure!(
        read.event_record_count == 0 && read.local_projection_record_count == 0,
        "only an empty disposable profile may be protected"
    );
    let mut checkpoint = Checkpoint {
        selection,
        generation: 0,
        next: 0,
        pending: BTreeMap::new(),
        readable: BTreeMap::new(),
    };
    run_state_transaction(root, |transaction| {
        let mut ratchet = transaction.load_ratchet_state()?;
        ratchet.protect_for_ordered_ingress()?;
        write(root, &mut checkpoint, device.identity(), transaction)
    })
}

#[cfg(test)]
#[path = "ordered_ingress_tests.rs"]
mod tests;
