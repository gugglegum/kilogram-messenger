//! Dormant transport-independent state machine for canonical sync v2.
//!
//! This module deliberately has no `EventStore` implementation and is not
//! referenced by the CLI or Iroh transport. It proves the state transitions
//! needed by `kilogram/m1/sync/cbor/2` while leaving the active Postcard v6
//! runtime unchanged. Activation additionally requires a store that preserves
//! both typed event namespaces and a policy that verifies conversation
//! membership for both closed carrier variants.

use std::collections::{BTreeSet, HashSet};

use kilogram_identity::{AccountId, ConversationMembershipSnapshot, DeviceId, DeviceIdentity};
use kilogram_protocol::{
    SyncSessionBinding,
    canonical_account_v1::{
        CanonicalSignedConversationMembershipV1, verify_signed_conversation_membership_v1,
    },
    canonical_event_v1::{verify_authorized_event_for_membership_v1, verify_authorized_event_v1},
    canonical_sync_history_v2::{CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2},
    canonical_sync_responder_session_v2::{
        CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderCheckpointContentV2,
        CanonicalSyncResponderNextActionV2, CanonicalSyncResponderSessionV2Error,
        sync_responder_checkpoint_id_v2, sync_responder_checkpoint_signing_bytes_v2,
        verify_signed_sync_responder_checkpoint_v2,
    },
    canonical_sync_session_v2::{
        CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2, CanonicalSignedSyncSessionCheckpointV2,
        CanonicalSyncNextActionV2, CanonicalSyncSessionCheckpointContentV2,
        CanonicalSyncSessionV2Error, sync_session_checkpoint_id_v2,
        sync_session_checkpoint_signing_bytes_v2, verify_signed_sync_session_checkpoint_v2,
    },
    canonical_sync_v2::{
        CanonicalSignedSyncDiffV2, CanonicalSignedSyncInventoryV2, CanonicalSyncClientFrameV2,
        CanonicalSyncCompleteV2, CanonicalSyncDiffContentV2, CanonicalSyncEventBatchV2,
        CanonicalSyncInventoryContentV2, CanonicalSyncRejectedV2, CanonicalSyncRejectionReasonV2,
        CanonicalSyncServerFrameV2, CanonicalSyncV2Error, MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS, encode_sync_complete_v2,
        encode_sync_event_batch_v2, sync_diff_signing_bytes_v2, sync_inventory_signing_bytes_v2,
        verify_signed_sync_diff_v2, verify_signed_sync_inventory_v2, verify_sync_frame_session_v2,
    },
};
use kilogram_store::{
    CanonicalSyncV2CarrierStore, CanonicalSyncV2LegacyImportPlan,
    CanonicalSyncV2StoredResponderCheckpoint, CanonicalSyncV2StoredResponderRuntime,
    CanonicalSyncV2StoredSessionCheckpoint, CanonicalSyncV2StoredSessionRuntime, StoreError,
};
use thiserror::Error;

/// Storage contract required before canonical sync v2 can be activated.
///
/// Implementations must preserve the full typed key `(namespace, event_id)`.
/// `put_events_v2` must be atomic and idempotent for byte-identical carriers;
/// conflicting carriers for an existing typed key must fail closed.
pub trait CanonicalSyncV2Store {
    fn inventory_v2(
        &self,
        conversation_id: [u8; 32],
    ) -> Result<Vec<CanonicalSyncEventKeyV2>, StoreError>;

    fn events_by_key_v2(
        &self,
        conversation_id: [u8; 32],
        event_keys: &[CanonicalSyncEventKeyV2],
    ) -> Result<Vec<CanonicalSyncEventCarrierV2>, StoreError>;

    fn put_events_v2(
        &self,
        conversation_id: [u8; 32],
        events: &[CanonicalSyncEventCarrierV2],
    ) -> Result<(), StoreError>;
}

impl CanonicalSyncV2Store for CanonicalSyncV2CarrierStore {
    fn inventory_v2(
        &self,
        conversation_id: [u8; 32],
    ) -> Result<Vec<CanonicalSyncEventKeyV2>, StoreError> {
        self.inventory(conversation_id)
    }

    fn events_by_key_v2(
        &self,
        conversation_id: [u8; 32],
        event_keys: &[CanonicalSyncEventKeyV2],
    ) -> Result<Vec<CanonicalSyncEventCarrierV2>, StoreError> {
        self.events_by_key(conversation_id, event_keys)
    }

    fn put_events_v2(
        &self,
        conversation_id: [u8; 32],
        events: &[CanonicalSyncEventCarrierV2],
    ) -> Result<(), StoreError> {
        self.put_batch(conversation_id, events)
    }
}

/// Conversation-specific authorization boundary for the closed event carrier.
///
/// Wire decoding verifies each embedded signature and exact representation.
/// An implementation of this trait must additionally verify that the author
/// Account is a member of the named conversation under the appropriate legacy
/// or canonical membership snapshot before storage is allowed.
pub trait CanonicalSyncV2EventPolicy {
    fn verify_event(
        &self,
        conversation_id: [u8; 32],
        event: &CanonicalSyncEventCarrierV2,
    ) -> Result<(), CanonicalSyncV2PolicyError>;
}

#[derive(Clone, Debug)]
enum CanonicalSyncV2MembershipSnapshot {
    Legacy(ConversationMembershipSnapshot),
    Canonical(CanonicalSignedConversationMembershipV1),
}

/// Verified conversation-membership policy for either installed snapshot format.
///
/// One exact owner-signed snapshot authorizes both closed carrier variants. This
/// avoids requiring historical conversation owners to re-sign installed
/// Postcard membership state merely so a canonical event can be checked, while
/// still supporting a future canonical membership snapshot without accepting
/// format sniffing or a parse-error fallback.
#[derive(Clone, Debug)]
pub struct CanonicalSyncV2MembershipPolicy {
    snapshot: CanonicalSyncV2MembershipSnapshot,
}

impl CanonicalSyncV2MembershipPolicy {
    pub fn from_legacy(
        membership: ConversationMembershipSnapshot,
    ) -> Result<Self, CanonicalSyncV2PolicyError> {
        membership.verify().map_err(|error| {
            CanonicalSyncV2PolicyError::rejected(format!(
                "legacy conversation membership snapshot verification failed: {error}"
            ))
        })?;
        Ok(Self {
            snapshot: CanonicalSyncV2MembershipSnapshot::Legacy(membership),
        })
    }

    pub fn from_canonical(
        membership: CanonicalSignedConversationMembershipV1,
    ) -> Result<Self, CanonicalSyncV2PolicyError> {
        verify_signed_conversation_membership_v1(&membership).map_err(|error| {
            CanonicalSyncV2PolicyError::rejected(format!(
                "canonical conversation membership snapshot verification failed: {error}"
            ))
        })?;
        Ok(Self {
            snapshot: CanonicalSyncV2MembershipSnapshot::Canonical(membership),
        })
    }

    pub fn conversation_id(&self) -> [u8; 32] {
        match &self.snapshot {
            CanonicalSyncV2MembershipSnapshot::Legacy(membership) => {
                *membership.conversation_id().as_bytes()
            }
            CanonicalSyncV2MembershipSnapshot::Canonical(membership) => {
                membership.content.conversation_id
            }
        }
    }

    pub fn revision(&self) -> u64 {
        match &self.snapshot {
            CanonicalSyncV2MembershipSnapshot::Legacy(membership) => membership.revision(),
            CanonicalSyncV2MembershipSnapshot::Canonical(membership) => membership.content.revision,
        }
    }
}

impl CanonicalSyncV2EventPolicy for CanonicalSyncV2MembershipPolicy {
    fn verify_event(
        &self,
        conversation_id: [u8; 32],
        event: &CanonicalSyncEventCarrierV2,
    ) -> Result<(), CanonicalSyncV2PolicyError> {
        if self.conversation_id() != conversation_id || event.conversation_id() != &conversation_id
        {
            return Err(CanonicalSyncV2PolicyError::rejected(
                "carrier and membership conversation binding mismatch",
            ));
        }

        match (&self.snapshot, event) {
            (
                CanonicalSyncV2MembershipSnapshot::Legacy(membership),
                CanonicalSyncEventCarrierV2::LegacyPostcardV6(legacy),
            ) => legacy
                .active_authorized_event()
                .verify_for_membership(membership)
                .map_err(|error| {
                    CanonicalSyncV2PolicyError::rejected(format!(
                        "legacy carrier membership verification failed: {error}"
                    ))
                }),
            (
                CanonicalSyncV2MembershipSnapshot::Legacy(membership),
                CanonicalSyncEventCarrierV2::CanonicalCborV1(canonical),
            ) => {
                verify_authorized_event_v1(canonical).map_err(|error| {
                    CanonicalSyncV2PolicyError::rejected(format!(
                        "canonical carrier authorization verification failed: {error}"
                    ))
                })?;
                membership
                    .require_member(AccountId::from_bytes(
                        canonical.author_certificate.content.account_id,
                    ))
                    .map_err(|error| {
                        CanonicalSyncV2PolicyError::rejected(format!(
                            "canonical carrier legacy-membership verification failed: {error}"
                        ))
                    })
            }
            (
                CanonicalSyncV2MembershipSnapshot::Canonical(membership),
                CanonicalSyncEventCarrierV2::CanonicalCborV1(canonical),
            ) => {
                verify_authorized_event_for_membership_v1(canonical, membership).map_err(|error| {
                    CanonicalSyncV2PolicyError::rejected(format!(
                        "canonical carrier membership verification failed: {error}"
                    ))
                })
            }
            (
                CanonicalSyncV2MembershipSnapshot::Canonical(membership),
                CanonicalSyncEventCarrierV2::LegacyPostcardV6(legacy),
            ) => {
                verify_signed_conversation_membership_v1(membership).map_err(|error| {
                    CanonicalSyncV2PolicyError::rejected(format!(
                        "canonical membership snapshot verification failed: {error}"
                    ))
                })?;
                legacy
                    .active_authorized_event()
                    .verify_author()
                    .map_err(|error| {
                        CanonicalSyncV2PolicyError::rejected(format!(
                            "legacy carrier authorization verification failed: {error}"
                        ))
                    })?;
                if membership
                    .content
                    .members
                    .binary_search(legacy.author_account_id())
                    .is_err()
                {
                    return Err(CanonicalSyncV2PolicyError::rejected(
                        "legacy carrier author is not a canonical membership member",
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("canonical sync v2 event authorization policy rejected the carrier: {reason}")]
pub struct CanonicalSyncV2PolicyError {
    reason: String,
}

impl CanonicalSyncV2PolicyError {
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

pub struct CanonicalSyncV2Client<
    'a,
    S: CanonicalSyncV2Store + ?Sized,
    P: CanonicalSyncV2EventPolicy + ?Sized,
> {
    identity: &'a DeviceIdentity,
    store: &'a S,
    policy: &'a P,
    conversation_id: [u8; 32],
    session_binding: SyncSessionBinding,
    expected_responder: DeviceId,
}

impl<'a, S: CanonicalSyncV2Store + ?Sized, P: CanonicalSyncV2EventPolicy + ?Sized>
    CanonicalSyncV2Client<'a, S, P>
{
    pub fn new(
        identity: &'a DeviceIdentity,
        store: &'a S,
        policy: &'a P,
        conversation_id: [u8; 32],
        session_binding: SyncSessionBinding,
        expected_responder: DeviceId,
    ) -> Self {
        Self {
            identity,
            store,
            policy,
            conversation_id,
            session_binding,
            expected_responder,
        }
    }

    pub fn begin_round(
        &self,
    ) -> Result<CanonicalSyncV2ClientInventoryRound, CanonicalSyncV2SessionError> {
        let event_keys = normalized_inventory(
            self.store.inventory_v2(self.conversation_id)?,
            "client inventory",
        )?;
        let content = CanonicalSyncInventoryContentV2 {
            conversation_id: self.conversation_id,
            requester_device_id: *self.identity.device_id().as_bytes(),
            session_binding: *self.session_binding.as_bytes(),
            event_keys: event_keys.clone(),
        };
        let inventory = CanonicalSignedSyncInventoryV2 {
            signature: self
                .identity
                .sign(&sync_inventory_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_inventory_v2(&inventory)?;
        Ok(CanonicalSyncV2ClientInventoryRound {
            inventory,
            advertised_event_keys: event_keys.into_iter().collect(),
        })
    }

    pub fn accept_diff(
        &self,
        round: CanonicalSyncV2ClientInventoryRound,
        diff: CanonicalSignedSyncDiffV2,
    ) -> Result<CanonicalSyncV2ClientBatchRound, CanonicalSyncV2SessionError> {
        let (round, received_events) = self.prepare_diff(round, diff)?;
        self.store
            .put_events_v2(self.conversation_id, &received_events)?;
        Ok(round)
    }

    fn prepare_diff(
        &self,
        round: CanonicalSyncV2ClientInventoryRound,
        diff: CanonicalSignedSyncDiffV2,
    ) -> Result<
        (
            CanonicalSyncV2ClientBatchRound,
            Vec<CanonicalSyncEventCarrierV2>,
        ),
        CanonicalSyncV2SessionError,
    > {
        verify_sync_frame_session_v2(&round.inventory, &diff)?;
        if diff.content.responder_device_id != *self.expected_responder.as_bytes() {
            return Err(CanonicalSyncV2SessionError::UnexpectedResponder);
        }
        if diff.content.conversation_id != self.conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        if diff.content.session_binding != *self.session_binding.as_bytes() {
            return Err(CanonicalSyncV2SessionError::SessionBindingMismatch);
        }
        if let Some(key) = diff
            .content
            .requested_event_keys
            .iter()
            .find(|key| !round.advertised_event_keys.contains(key))
        {
            return Err(CanonicalSyncV2SessionError::UnadvertisedEventRequested(
                *key,
            ));
        }

        let received_events = diff.content.events.clone();
        let received_event_keys =
            validate_events(self.policy, self.conversation_id, &received_events)?;

        let events_for_responder = load_exact_events(
            self.store,
            self.policy,
            self.conversation_id,
            &diff.content.requested_event_keys,
        )?;
        let sent_event_keys = event_keys(&events_for_responder)?;
        let batch = CanonicalSyncEventBatchV2 {
            conversation_id: self.conversation_id,
            events: events_for_responder,
        };
        encode_sync_event_batch_v2(&batch)?;

        Ok((
            CanonicalSyncV2ClientBatchRound {
                batch,
                sent_event_keys,
                received_event_keys,
                more_available: diff.content.more_available,
            },
            received_events,
        ))
    }

    pub fn accept_complete(
        &self,
        round: CanonicalSyncV2ClientBatchRound,
        complete: CanonicalSyncCompleteV2,
    ) -> Result<CanonicalSyncV2RoundStats, CanonicalSyncV2SessionError> {
        encode_sync_complete_v2(&complete)?;
        if complete.conversation_id != self.conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        if complete.stored_event_keys != round.sent_event_keys {
            return Err(CanonicalSyncV2SessionError::CompletionEventKeysMismatch);
        }
        if complete.more_available != round.more_available {
            return Err(CanonicalSyncV2SessionError::ContinuationMismatch);
        }
        Ok(CanonicalSyncV2RoundStats {
            sent_events: round.sent_event_keys.len(),
            received_events: round.received_event_keys.len(),
            more_available: round.more_available,
        })
    }
}

impl<'a, P: CanonicalSyncV2EventPolicy + ?Sized>
    CanonicalSyncV2Client<'a, CanonicalSyncV2CarrierStore, P>
{
    /// Creates the dormant client and its first signed checkpoint only after
    /// the exact local legacy-import plan is durably complete.
    pub fn bootstrap_from_completed_legacy_import(
        identity: &'a DeviceIdentity,
        store: &'a CanonicalSyncV2CarrierStore,
        policy: &'a P,
        plan: &CanonicalSyncV2LegacyImportPlan,
        session_binding: SyncSessionBinding,
        expected_responder: DeviceId,
    ) -> Result<(Self, CanonicalSyncV2StoredSessionCheckpoint), CanonicalSyncV2SessionError> {
        let conversation_id = *plan.conversation_id().as_bytes();
        let content = CanonicalSyncSessionCheckpointContentV2 {
            protocol_identifier: *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2,
            conversation_id,
            local_device_id: *identity.device_id().as_bytes(),
            peer_device_id: *expected_responder.as_bytes(),
            session_binding: *session_binding.as_bytes(),
            checkpoint_sequence: 1,
            sync_round: 1,
            predecessor_checkpoint_id: None,
            received_event_keys: Vec::new(),
            sent_event_keys: Vec::new(),
            local_more_available: false,
            peer_more_available: false,
            next_action: CanonicalSyncNextActionV2::SendInventory,
            paused: false,
        };
        let checkpoint = CanonicalSignedSyncSessionCheckpointV2 {
            signature: identity.sign(&sync_session_checkpoint_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_session_checkpoint_v2(&checkpoint)?;
        let stored = store.install_legacy_import_session_bootstrap(plan, &checkpoint)?;
        Ok((
            Self::new(
                identity,
                store,
                policy,
                conversation_id,
                session_binding,
                expected_responder,
            ),
            stored,
        ))
    }

    /// Restores the exact durable head and pending client frame for this
    /// dormant `/cbor/2` client identity.
    pub fn restore_durable_runtime(
        &self,
    ) -> Result<CanonicalSyncV2StoredSessionRuntime, CanonicalSyncV2SessionError> {
        self.store
            .session_runtime_state(
                self.conversation_id,
                *self.identity.device_id().as_bytes(),
                *self.expected_responder.as_bytes(),
                *self.session_binding.as_bytes(),
            )?
            .ok_or(CanonicalSyncV2SessionError::DurableSessionMissing)
    }

    /// Converts verified durable state into the only safe next client action.
    /// Pending frames are returned byte-equivalent to the frame stored before
    /// an uncertain network write; they are never rebuilt from current state.
    pub fn durable_action(
        &self,
        runtime: &CanonicalSyncV2StoredSessionRuntime,
    ) -> Result<CanonicalSyncV2DurableClientAction, CanonicalSyncV2SessionError> {
        self.verify_durable_runtime_identity(runtime)?;
        let content = &runtime.checkpoint().checkpoint().content;
        let action = match (content.next_action, runtime.pending_client_frame()) {
            (CanonicalSyncNextActionV2::SendInventory, None) => {
                CanonicalSyncV2DurableClientAction::PrepareInventory
            }
            (
                CanonicalSyncNextActionV2::ReceiveDiff,
                Some(CanonicalSyncClientFrameV2::Inventory(inventory)),
            ) => CanonicalSyncV2DurableClientAction::TransmitInventory(inventory.clone()),
            (
                CanonicalSyncNextActionV2::SendEventBatch,
                Some(CanonicalSyncClientFrameV2::Events(batch)),
            ) => CanonicalSyncV2DurableClientAction::TransmitEventBatch(batch.clone()),
            (
                CanonicalSyncNextActionV2::ReceiveComplete,
                Some(CanonicalSyncClientFrameV2::Events(batch)),
            ) => CanonicalSyncV2DurableClientAction::RetransmitEventBatch(batch.clone()),
            (CanonicalSyncNextActionV2::Complete, None) => {
                CanonicalSyncV2DurableClientAction::Complete
            }
            (CanonicalSyncNextActionV2::Rejected, None) => {
                CanonicalSyncV2DurableClientAction::Rejected
            }
            (action, _) => {
                return Err(CanonicalSyncV2SessionError::DurableRuntimeFrameMismatch { action });
            }
        };
        if content.paused {
            Ok(CanonicalSyncV2DurableClientAction::Paused)
        } else {
            Ok(action)
        }
    }

    /// Builds and durably binds the exact signed Inventory before it may be
    /// written to a transport.
    pub fn begin_durable_round(
        &self,
        expected_runtime: &CanonicalSyncV2StoredSessionRuntime,
    ) -> Result<
        (
            CanonicalSyncV2ClientInventoryRound,
            CanonicalSyncV2StoredSessionRuntime,
        ),
        CanonicalSyncV2SessionError,
    > {
        self.require_durable_action(
            expected_runtime,
            CanonicalSyncV2DurableClientActionKind::PrepareInventory,
        )?;
        let round = self.begin_round()?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let successor = self.signed_durable_successor(
            expected_runtime,
            previous.sync_round,
            previous.received_event_keys.clone(),
            previous.sent_event_keys.clone(),
            false,
            false,
            CanonicalSyncNextActionV2::ReceiveDiff,
        )?;
        let frame = round.frame();
        let runtime = self.store.commit_session_runtime_transition(
            expected_runtime,
            &successor,
            Some(&frame),
            &[],
        )?;
        Ok((round, runtime))
    }

    /// Verifies one signed Diff, prepares the exact EventBatch and atomically
    /// commits received carriers, cumulative evidence, successor and frame.
    pub fn accept_durable_diff(
        &self,
        expected_runtime: &CanonicalSyncV2StoredSessionRuntime,
        diff: CanonicalSignedSyncDiffV2,
    ) -> Result<
        (
            CanonicalSyncV2ClientBatchRound,
            CanonicalSyncV2StoredSessionRuntime,
        ),
        CanonicalSyncV2SessionError,
    > {
        let inventory = match self.durable_action(expected_runtime)? {
            CanonicalSyncV2DurableClientAction::TransmitInventory(inventory) => inventory,
            other => {
                return Err(CanonicalSyncV2SessionError::UnexpectedDurableAction {
                    expected: "transmit-inventory",
                    actual: other.kind().label(),
                });
            }
        };
        let inventory_round = CanonicalSyncV2ClientInventoryRound {
            advertised_event_keys: inventory.content.event_keys.iter().copied().collect(),
            inventory,
        };
        let (batch_round, received_events) = self.prepare_diff(inventory_round, diff)?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let received_event_keys = merge_cumulative_event_keys(
            &previous.received_event_keys,
            &batch_round.received_event_keys,
        );
        let sent_event_keys =
            merge_cumulative_event_keys(&previous.sent_event_keys, &batch_round.sent_event_keys);
        let successor = self.signed_durable_successor(
            expected_runtime,
            previous.sync_round,
            received_event_keys,
            sent_event_keys,
            true,
            batch_round.more_available,
            CanonicalSyncNextActionV2::SendEventBatch,
        )?;
        let frame = batch_round.frame();
        let runtime = self.store.commit_session_runtime_transition(
            expected_runtime,
            &successor,
            Some(&frame),
            &received_events,
        )?;
        Ok((batch_round, runtime))
    }

    /// Records that the exact durable EventBatch has crossed the local send
    /// boundary. A crash before this transition resends `SendEventBatch`; a
    /// crash after it resends the same frame while awaiting Complete.
    pub fn mark_durable_batch_sent(
        &self,
        expected_runtime: &CanonicalSyncV2StoredSessionRuntime,
    ) -> Result<CanonicalSyncV2StoredSessionRuntime, CanonicalSyncV2SessionError> {
        self.require_durable_action(
            expected_runtime,
            CanonicalSyncV2DurableClientActionKind::TransmitEventBatch,
        )?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let successor = self.signed_durable_successor(
            expected_runtime,
            previous.sync_round,
            previous.received_event_keys.clone(),
            previous.sent_event_keys.clone(),
            previous.local_more_available,
            previous.peer_more_available,
            CanonicalSyncNextActionV2::ReceiveComplete,
        )?;
        self.store
            .commit_session_runtime_transition(
                expected_runtime,
                &successor,
                expected_runtime.pending_client_frame(),
                &[],
            )
            .map_err(Into::into)
    }

    /// Verifies Complete against the exact persisted batch and commits either
    /// a terminal checkpoint or the next round's `SendInventory` checkpoint.
    pub fn accept_durable_complete(
        &self,
        expected_runtime: &CanonicalSyncV2StoredSessionRuntime,
        complete: CanonicalSyncCompleteV2,
    ) -> Result<
        (
            CanonicalSyncV2RoundStats,
            CanonicalSyncV2StoredSessionRuntime,
        ),
        CanonicalSyncV2SessionError,
    > {
        let batch = match self.durable_action(expected_runtime)? {
            CanonicalSyncV2DurableClientAction::RetransmitEventBatch(batch) => batch,
            other => {
                return Err(CanonicalSyncV2SessionError::UnexpectedDurableAction {
                    expected: "retransmit-event-batch",
                    actual: other.kind().label(),
                });
            }
        };
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let batch_round = CanonicalSyncV2ClientBatchRound {
            sent_event_keys: event_keys(&batch.events)?,
            received_event_keys: self.current_round_received_event_keys(expected_runtime)?,
            more_available: previous.peer_more_available,
            batch,
        };
        let stats = self.accept_complete(batch_round, complete)?;
        let (sync_round, local_more_available, peer_more_available, next_action) =
            if stats.more_available {
                (
                    previous
                        .sync_round
                        .checked_add(1)
                        .ok_or(CanonicalSyncV2SessionError::DurableCheckpointSequenceOverflow)?,
                    previous.local_more_available,
                    previous.peer_more_available,
                    CanonicalSyncNextActionV2::SendInventory,
                )
            } else {
                (
                    previous.sync_round,
                    false,
                    false,
                    CanonicalSyncNextActionV2::Complete,
                )
            };
        let successor = self.signed_durable_successor(
            expected_runtime,
            sync_round,
            previous.received_event_keys.clone(),
            previous.sent_event_keys.clone(),
            local_more_available,
            peer_more_available,
            next_action,
        )?;
        let runtime = self.store.commit_session_runtime_transition(
            expected_runtime,
            &successor,
            None,
            &[],
        )?;
        Ok((stats, runtime))
    }

    fn require_durable_action(
        &self,
        runtime: &CanonicalSyncV2StoredSessionRuntime,
        expected: CanonicalSyncV2DurableClientActionKind,
    ) -> Result<(), CanonicalSyncV2SessionError> {
        let actual = self.durable_action(runtime)?;
        if actual.kind() != expected {
            return Err(CanonicalSyncV2SessionError::UnexpectedDurableAction {
                expected: expected.label(),
                actual: actual.kind().label(),
            });
        }
        Ok(())
    }

    fn verify_durable_runtime_identity(
        &self,
        runtime: &CanonicalSyncV2StoredSessionRuntime,
    ) -> Result<(), CanonicalSyncV2SessionError> {
        let content = &runtime.checkpoint().checkpoint().content;
        if content.conversation_id != self.conversation_id
            || content.local_device_id != *self.identity.device_id().as_bytes()
            || content.peer_device_id != *self.expected_responder.as_bytes()
            || content.session_binding != *self.session_binding.as_bytes()
        {
            return Err(CanonicalSyncV2SessionError::DurableSessionIdentityMismatch);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_durable_successor(
        &self,
        previous: &CanonicalSyncV2StoredSessionRuntime,
        sync_round: u64,
        received_event_keys: Vec<CanonicalSyncEventKeyV2>,
        sent_event_keys: Vec<CanonicalSyncEventKeyV2>,
        local_more_available: bool,
        peer_more_available: bool,
        next_action: CanonicalSyncNextActionV2,
    ) -> Result<CanonicalSignedSyncSessionCheckpointV2, CanonicalSyncV2SessionError> {
        self.verify_durable_runtime_identity(previous)?;
        let previous_checkpoint = previous.checkpoint().checkpoint();
        let checkpoint_sequence = previous_checkpoint
            .content
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(CanonicalSyncV2SessionError::DurableCheckpointSequenceOverflow)?;
        let content = CanonicalSyncSessionCheckpointContentV2 {
            protocol_identifier: previous_checkpoint.content.protocol_identifier,
            conversation_id: previous_checkpoint.content.conversation_id,
            local_device_id: previous_checkpoint.content.local_device_id,
            peer_device_id: previous_checkpoint.content.peer_device_id,
            session_binding: previous_checkpoint.content.session_binding,
            checkpoint_sequence,
            sync_round,
            predecessor_checkpoint_id: Some(sync_session_checkpoint_id_v2(previous_checkpoint)?),
            received_event_keys,
            sent_event_keys,
            local_more_available,
            peer_more_available,
            next_action,
            paused: false,
        };
        let successor = CanonicalSignedSyncSessionCheckpointV2 {
            signature: self
                .identity
                .sign(&sync_session_checkpoint_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_session_checkpoint_v2(&successor)?;
        Ok(successor)
    }

    fn current_round_received_event_keys(
        &self,
        expected_runtime: &CanonicalSyncV2StoredSessionRuntime,
    ) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncV2SessionError> {
        self.verify_durable_runtime_identity(expected_runtime)?;
        let current = &expected_runtime.checkpoint().checkpoint().content;
        let chain = self.store.session_checkpoint_chain(
            self.conversation_id,
            *self.identity.device_id().as_bytes(),
            *self.expected_responder.as_bytes(),
            *self.session_binding.as_bytes(),
        )?;
        let baseline = chain
            .iter()
            .find(|checkpoint| checkpoint.checkpoint().content.sync_round == current.sync_round)
            .ok_or(CanonicalSyncV2SessionError::DurableRoundBaselineMissing)?;
        Ok(current
            .received_event_keys
            .iter()
            .copied()
            .filter(|key| {
                baseline
                    .checkpoint()
                    .content
                    .received_event_keys
                    .binary_search(key)
                    .is_err()
            })
            .collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncV2DurableClientAction {
    PrepareInventory,
    TransmitInventory(CanonicalSignedSyncInventoryV2),
    TransmitEventBatch(CanonicalSyncEventBatchV2),
    RetransmitEventBatch(CanonicalSyncEventBatchV2),
    Paused,
    Complete,
    Rejected,
}

impl CanonicalSyncV2DurableClientAction {
    pub fn frame(&self) -> Option<CanonicalSyncClientFrameV2> {
        match self {
            Self::TransmitInventory(inventory) => {
                Some(CanonicalSyncClientFrameV2::Inventory(inventory.clone()))
            }
            Self::TransmitEventBatch(batch) | Self::RetransmitEventBatch(batch) => {
                Some(CanonicalSyncClientFrameV2::Events(batch.clone()))
            }
            Self::PrepareInventory | Self::Paused | Self::Complete | Self::Rejected => None,
        }
    }

    fn kind(&self) -> CanonicalSyncV2DurableClientActionKind {
        match self {
            Self::PrepareInventory => CanonicalSyncV2DurableClientActionKind::PrepareInventory,
            Self::TransmitInventory(_) => CanonicalSyncV2DurableClientActionKind::TransmitInventory,
            Self::TransmitEventBatch(_) => {
                CanonicalSyncV2DurableClientActionKind::TransmitEventBatch
            }
            Self::RetransmitEventBatch(_) => {
                CanonicalSyncV2DurableClientActionKind::RetransmitEventBatch
            }
            Self::Paused => CanonicalSyncV2DurableClientActionKind::Paused,
            Self::Complete => CanonicalSyncV2DurableClientActionKind::Complete,
            Self::Rejected => CanonicalSyncV2DurableClientActionKind::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanonicalSyncV2DurableClientActionKind {
    PrepareInventory,
    TransmitInventory,
    TransmitEventBatch,
    RetransmitEventBatch,
    Paused,
    Complete,
    Rejected,
}

impl CanonicalSyncV2DurableClientActionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::PrepareInventory => "prepare-inventory",
            Self::TransmitInventory => "transmit-inventory",
            Self::TransmitEventBatch => "transmit-event-batch",
            Self::RetransmitEventBatch => "retransmit-event-batch",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Rejected => "rejected",
        }
    }
}

fn merge_cumulative_event_keys(
    previous: &[CanonicalSyncEventKeyV2],
    additional: &[CanonicalSyncEventKeyV2],
) -> Vec<CanonicalSyncEventKeyV2> {
    previous
        .iter()
        .chain(additional)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub struct CanonicalSyncV2ClientInventoryRound {
    inventory: CanonicalSignedSyncInventoryV2,
    advertised_event_keys: HashSet<CanonicalSyncEventKeyV2>,
}

impl CanonicalSyncV2ClientInventoryRound {
    pub fn inventory(&self) -> &CanonicalSignedSyncInventoryV2 {
        &self.inventory
    }

    pub fn frame(&self) -> CanonicalSyncClientFrameV2 {
        CanonicalSyncClientFrameV2::Inventory(self.inventory.clone())
    }
}

pub struct CanonicalSyncV2ClientBatchRound {
    batch: CanonicalSyncEventBatchV2,
    sent_event_keys: Vec<CanonicalSyncEventKeyV2>,
    received_event_keys: Vec<CanonicalSyncEventKeyV2>,
    more_available: bool,
}

impl CanonicalSyncV2ClientBatchRound {
    pub fn batch(&self) -> &CanonicalSyncEventBatchV2 {
        &self.batch
    }

    pub fn frame(&self) -> CanonicalSyncClientFrameV2 {
        CanonicalSyncClientFrameV2::Events(self.batch.clone())
    }
}

pub struct CanonicalSyncV2Server<
    'a,
    S: CanonicalSyncV2Store + ?Sized,
    P: CanonicalSyncV2EventPolicy + ?Sized,
> {
    identity: &'a DeviceIdentity,
    store: &'a S,
    policy: &'a P,
    session_binding: SyncSessionBinding,
    allowed_requester: DeviceId,
    conversation_id: [u8; 32],
}

impl<'a, S: CanonicalSyncV2Store + ?Sized, P: CanonicalSyncV2EventPolicy + ?Sized>
    CanonicalSyncV2Server<'a, S, P>
{
    pub fn new(
        identity: &'a DeviceIdentity,
        store: &'a S,
        policy: &'a P,
        conversation_id: [u8; 32],
        session_binding: SyncSessionBinding,
        allowed_requester: DeviceId,
    ) -> Self {
        Self {
            identity,
            store,
            policy,
            session_binding,
            allowed_requester,
            conversation_id,
        }
    }

    pub fn accept_inventory(
        &self,
        inventory: &CanonicalSignedSyncInventoryV2,
    ) -> Result<CanonicalSyncV2ServerInventoryOutcome, CanonicalSyncV2SessionError> {
        verify_signed_sync_inventory_v2(inventory)?;
        if inventory.content.conversation_id != self.conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        if inventory.content.session_binding != *self.session_binding.as_bytes() {
            return Err(CanonicalSyncV2SessionError::SessionBindingMismatch);
        }
        if inventory.content.requester_device_id != *self.allowed_requester.as_bytes() {
            return Ok(CanonicalSyncV2ServerInventoryOutcome::Rejected(
                CanonicalSyncRejectedV2 {
                    conversation_id: self.conversation_id,
                    reason: CanonicalSyncRejectionReasonV2::RequesterNotAllowed,
                },
            ));
        }

        let plan = plan_sync(
            self.store,
            self.policy,
            self.conversation_id,
            &inventory.content.event_keys,
        )?;
        let content = CanonicalSyncDiffContentV2 {
            conversation_id: self.conversation_id,
            responder_device_id: *self.identity.device_id().as_bytes(),
            session_binding: *self.session_binding.as_bytes(),
            requested_event_keys: plan.requested_from_remote.clone(),
            events: plan.events_for_remote,
            more_available: plan.more_available,
        };
        let diff = CanonicalSignedSyncDiffV2 {
            signature: self.identity.sign(&sync_diff_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_diff_v2(&diff)?;
        Ok(CanonicalSyncV2ServerInventoryOutcome::Accepted(Box::new(
            CanonicalSyncV2ServerInventoryRound {
                requested_event_keys: plan.requested_from_remote,
                sent_events: diff.content.events.len(),
                more_available: diff.content.more_available,
                diff,
            },
        )))
    }

    pub fn complete_round(
        &self,
        round: CanonicalSyncV2ServerInventoryRound,
        batch: CanonicalSyncEventBatchV2,
    ) -> Result<CanonicalSyncV2ServerCompletion, CanonicalSyncV2SessionError> {
        encode_sync_event_batch_v2(&batch)?;
        if batch.conversation_id != self.conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        let supplied_event_keys =
            validate_events(self.policy, self.conversation_id, &batch.events)?;
        if supplied_event_keys != round.requested_event_keys {
            return Err(CanonicalSyncV2SessionError::BatchEventKeysMismatch);
        }
        self.store
            .put_events_v2(self.conversation_id, &batch.events)?;

        let response = CanonicalSyncCompleteV2 {
            conversation_id: self.conversation_id,
            stored_event_keys: supplied_event_keys.clone(),
            more_available: round.more_available,
        };
        encode_sync_complete_v2(&response)?;
        Ok(CanonicalSyncV2ServerCompletion {
            response,
            stats: CanonicalSyncV2RoundStats {
                sent_events: round.sent_events,
                received_events: supplied_event_keys.len(),
                more_available: round.more_available,
            },
        })
    }
}

impl<'a, P: CanonicalSyncV2EventPolicy + ?Sized>
    CanonicalSyncV2Server<'a, CanonicalSyncV2CarrierStore, P>
{
    /// Creates the durable responder journal only after the exact local
    /// legacy import has a completed atomic checkpoint.
    pub fn bootstrap_responder_from_completed_legacy_import(
        identity: &'a DeviceIdentity,
        store: &'a CanonicalSyncV2CarrierStore,
        policy: &'a P,
        plan: &CanonicalSyncV2LegacyImportPlan,
        session_binding: SyncSessionBinding,
        allowed_requester: DeviceId,
    ) -> Result<(Self, CanonicalSyncV2StoredResponderCheckpoint), CanonicalSyncV2SessionError> {
        let conversation_id = *plan.conversation_id().as_bytes();
        let content = CanonicalSyncResponderCheckpointContentV2 {
            protocol_identifier: *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2,
            conversation_id,
            responder_device_id: *identity.device_id().as_bytes(),
            requester_device_id: *allowed_requester.as_bytes(),
            session_binding: *session_binding.as_bytes(),
            checkpoint_sequence: 1,
            sync_round: 1,
            predecessor_checkpoint_id: None,
            received_event_keys: Vec::new(),
            sent_event_keys: Vec::new(),
            continuation_required: false,
            next_action: CanonicalSyncResponderNextActionV2::ReceiveInventory,
            paused: false,
        };
        let checkpoint = CanonicalSignedSyncResponderCheckpointV2 {
            signature: identity.sign(&sync_responder_checkpoint_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_responder_checkpoint_v2(&checkpoint)?;
        let stored = store.install_legacy_import_responder_bootstrap(plan, &checkpoint)?;
        Ok((
            Self::new(
                identity,
                store,
                policy,
                conversation_id,
                session_binding,
                allowed_requester,
            ),
            stored,
        ))
    }

    pub fn restore_durable_responder_runtime(
        &self,
    ) -> Result<CanonicalSyncV2StoredResponderRuntime, CanonicalSyncV2SessionError> {
        self.store
            .responder_runtime_state(
                self.conversation_id,
                *self.identity.device_id().as_bytes(),
                *self.allowed_requester.as_bytes(),
                *self.session_binding.as_bytes(),
            )?
            .ok_or(CanonicalSyncV2SessionError::DurableSessionMissing)
    }

    /// Maps the exact verified responder checkpoint/frame pair to its only
    /// safe restart action. No Diff or Complete is rebuilt after a crash.
    pub fn durable_responder_action(
        &self,
        runtime: &CanonicalSyncV2StoredResponderRuntime,
    ) -> Result<CanonicalSyncV2DurableResponderAction, CanonicalSyncV2SessionError> {
        self.verify_durable_responder_identity(runtime)?;
        let content = &runtime.checkpoint().checkpoint().content;
        let action = match (content.next_action, runtime.pending_server_frame()) {
            (CanonicalSyncResponderNextActionV2::ReceiveInventory, None) => {
                CanonicalSyncV2DurableResponderAction::AwaitInventory
            }
            (
                CanonicalSyncResponderNextActionV2::SendDiff,
                Some(CanonicalSyncServerFrameV2::Diff(diff)),
            ) => CanonicalSyncV2DurableResponderAction::TransmitDiff(diff.clone()),
            (
                CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
                Some(CanonicalSyncServerFrameV2::Diff(diff)),
            ) => CanonicalSyncV2DurableResponderAction::RetransmitDiff(diff.clone()),
            (
                CanonicalSyncResponderNextActionV2::SendComplete,
                Some(CanonicalSyncServerFrameV2::Complete(complete)),
            ) => CanonicalSyncV2DurableResponderAction::TransmitComplete(complete.clone()),
            (CanonicalSyncResponderNextActionV2::Complete, None) => {
                CanonicalSyncV2DurableResponderAction::Complete
            }
            (
                CanonicalSyncResponderNextActionV2::Rejected,
                Some(CanonicalSyncServerFrameV2::Rejected(rejected)),
            ) => CanonicalSyncV2DurableResponderAction::TransmitRejection(rejected.clone()),
            (action, _) => {
                return Err(
                    CanonicalSyncV2SessionError::DurableResponderRuntimeFrameMismatch { action },
                );
            }
        };
        if content.paused {
            Ok(CanonicalSyncV2DurableResponderAction::Paused)
        } else {
            Ok(action)
        }
    }

    /// Verifies an Inventory and atomically journals the exact Diff (or exact
    /// rejection) before that response may cross the transport boundary.
    pub fn accept_durable_inventory(
        &self,
        expected_runtime: &CanonicalSyncV2StoredResponderRuntime,
        inventory: &CanonicalSignedSyncInventoryV2,
    ) -> Result<CanonicalSyncV2StoredResponderRuntime, CanonicalSyncV2SessionError> {
        self.require_durable_responder_action(
            expected_runtime,
            CanonicalSyncV2DurableResponderActionKind::AwaitInventory,
        )?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        match self.accept_inventory(inventory)? {
            CanonicalSyncV2ServerInventoryOutcome::Accepted(round) => {
                let sent_event_keys = merge_cumulative_event_keys(
                    &previous.sent_event_keys,
                    &event_keys(&round.diff.content.events)?,
                );
                let successor = self.signed_durable_responder_successor(
                    expected_runtime,
                    previous.sync_round,
                    previous.received_event_keys.clone(),
                    sent_event_keys,
                    round.more_available,
                    CanonicalSyncResponderNextActionV2::SendDiff,
                )?;
                let frame = round.frame();
                self.store
                    .commit_responder_runtime_transition(
                        expected_runtime,
                        &successor,
                        Some(&frame),
                        &[],
                    )
                    .map_err(Into::into)
            }
            CanonicalSyncV2ServerInventoryOutcome::Rejected(rejected) => {
                let successor = self.signed_durable_responder_successor(
                    expected_runtime,
                    previous.sync_round,
                    previous.received_event_keys.clone(),
                    previous.sent_event_keys.clone(),
                    false,
                    CanonicalSyncResponderNextActionV2::Rejected,
                )?;
                let frame = CanonicalSyncServerFrameV2::Rejected(rejected);
                self.store
                    .commit_responder_runtime_transition(
                        expected_runtime,
                        &successor,
                        Some(&frame),
                        &[],
                    )
                    .map_err(Into::into)
            }
        }
    }

    /// Marks the exact persisted Diff as sent while retaining it for an
    /// uncertain-send retry until the matching EventBatch is committed.
    pub fn mark_durable_diff_sent(
        &self,
        expected_runtime: &CanonicalSyncV2StoredResponderRuntime,
    ) -> Result<CanonicalSyncV2StoredResponderRuntime, CanonicalSyncV2SessionError> {
        self.require_durable_responder_action(
            expected_runtime,
            CanonicalSyncV2DurableResponderActionKind::TransmitDiff,
        )?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let successor = self.signed_durable_responder_successor(
            expected_runtime,
            previous.sync_round,
            previous.received_event_keys.clone(),
            previous.sent_event_keys.clone(),
            previous.continuation_required,
            CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
        )?;
        self.store
            .commit_responder_runtime_transition(
                expected_runtime,
                &successor,
                expected_runtime.pending_server_frame(),
                &[],
            )
            .map_err(Into::into)
    }

    /// Verifies the EventBatch against the requested keys in the exact durable
    /// Diff, then atomically commits carriers, evidence and exact Complete.
    pub fn accept_durable_event_batch(
        &self,
        expected_runtime: &CanonicalSyncV2StoredResponderRuntime,
        batch: CanonicalSyncEventBatchV2,
    ) -> Result<
        (
            CanonicalSyncV2RoundStats,
            CanonicalSyncV2StoredResponderRuntime,
        ),
        CanonicalSyncV2SessionError,
    > {
        let diff = match self.durable_responder_action(expected_runtime)? {
            CanonicalSyncV2DurableResponderAction::RetransmitDiff(diff) => diff,
            other => {
                return Err(CanonicalSyncV2SessionError::UnexpectedDurableAction {
                    expected: "retransmit-diff",
                    actual: other.kind().label(),
                });
            }
        };
        encode_sync_event_batch_v2(&batch)?;
        if batch.conversation_id != self.conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        let supplied_event_keys =
            validate_events(self.policy, self.conversation_id, &batch.events)?;
        if supplied_event_keys != diff.content.requested_event_keys {
            return Err(CanonicalSyncV2SessionError::BatchEventKeysMismatch);
        }
        let response = CanonicalSyncCompleteV2 {
            conversation_id: self.conversation_id,
            stored_event_keys: supplied_event_keys.clone(),
            more_available: diff.content.more_available,
        };
        encode_sync_complete_v2(&response)?;
        let stats = CanonicalSyncV2RoundStats {
            sent_events: diff.content.events.len(),
            received_events: supplied_event_keys.len(),
            more_available: diff.content.more_available,
        };
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let received_event_keys =
            merge_cumulative_event_keys(&previous.received_event_keys, &supplied_event_keys);
        let successor = self.signed_durable_responder_successor(
            expected_runtime,
            previous.sync_round,
            received_event_keys,
            previous.sent_event_keys.clone(),
            diff.content.more_available,
            CanonicalSyncResponderNextActionV2::SendComplete,
        )?;
        let frame = CanonicalSyncServerFrameV2::Complete(response);
        let runtime = self.store.commit_responder_runtime_transition(
            expected_runtime,
            &successor,
            Some(&frame),
            &batch.events,
        )?;
        Ok((stats, runtime))
    }

    /// Records the Complete send boundary. A continuation advances exactly
    /// one round; a converged session becomes terminal and clears the frame.
    pub fn mark_durable_complete_sent(
        &self,
        expected_runtime: &CanonicalSyncV2StoredResponderRuntime,
    ) -> Result<CanonicalSyncV2StoredResponderRuntime, CanonicalSyncV2SessionError> {
        self.require_durable_responder_action(
            expected_runtime,
            CanonicalSyncV2DurableResponderActionKind::TransmitComplete,
        )?;
        let previous = &expected_runtime.checkpoint().checkpoint().content;
        let (sync_round, continuation_required, next_action) = if previous.continuation_required {
            (
                previous
                    .sync_round
                    .checked_add(1)
                    .ok_or(CanonicalSyncV2SessionError::DurableCheckpointSequenceOverflow)?,
                false,
                CanonicalSyncResponderNextActionV2::ReceiveInventory,
            )
        } else {
            (
                previous.sync_round,
                false,
                CanonicalSyncResponderNextActionV2::Complete,
            )
        };
        let successor = self.signed_durable_responder_successor(
            expected_runtime,
            sync_round,
            previous.received_event_keys.clone(),
            previous.sent_event_keys.clone(),
            continuation_required,
            next_action,
        )?;
        self.store
            .commit_responder_runtime_transition(expected_runtime, &successor, None, &[])
            .map_err(Into::into)
    }

    fn verify_durable_responder_identity(
        &self,
        runtime: &CanonicalSyncV2StoredResponderRuntime,
    ) -> Result<(), CanonicalSyncV2SessionError> {
        let content = &runtime.checkpoint().checkpoint().content;
        if content.conversation_id != self.conversation_id
            || content.responder_device_id != *self.identity.device_id().as_bytes()
            || content.requester_device_id != *self.allowed_requester.as_bytes()
            || content.session_binding != *self.session_binding.as_bytes()
        {
            return Err(CanonicalSyncV2SessionError::DurableSessionIdentityMismatch);
        }
        Ok(())
    }

    fn require_durable_responder_action(
        &self,
        runtime: &CanonicalSyncV2StoredResponderRuntime,
        expected: CanonicalSyncV2DurableResponderActionKind,
    ) -> Result<(), CanonicalSyncV2SessionError> {
        let actual = self.durable_responder_action(runtime)?;
        if actual.kind() != expected {
            return Err(CanonicalSyncV2SessionError::UnexpectedDurableAction {
                expected: expected.label(),
                actual: actual.kind().label(),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn signed_durable_responder_successor(
        &self,
        previous: &CanonicalSyncV2StoredResponderRuntime,
        sync_round: u64,
        received_event_keys: Vec<CanonicalSyncEventKeyV2>,
        sent_event_keys: Vec<CanonicalSyncEventKeyV2>,
        continuation_required: bool,
        next_action: CanonicalSyncResponderNextActionV2,
    ) -> Result<CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncV2SessionError> {
        self.verify_durable_responder_identity(previous)?;
        let previous_checkpoint = previous.checkpoint().checkpoint();
        let checkpoint_sequence = previous_checkpoint
            .content
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(CanonicalSyncV2SessionError::DurableCheckpointSequenceOverflow)?;
        let content = CanonicalSyncResponderCheckpointContentV2 {
            protocol_identifier: previous_checkpoint.content.protocol_identifier,
            conversation_id: previous_checkpoint.content.conversation_id,
            responder_device_id: previous_checkpoint.content.responder_device_id,
            requester_device_id: previous_checkpoint.content.requester_device_id,
            session_binding: previous_checkpoint.content.session_binding,
            checkpoint_sequence,
            sync_round,
            predecessor_checkpoint_id: Some(sync_responder_checkpoint_id_v2(previous_checkpoint)?),
            received_event_keys,
            sent_event_keys,
            continuation_required,
            next_action,
            paused: false,
        };
        let successor = CanonicalSignedSyncResponderCheckpointV2 {
            signature: self
                .identity
                .sign(&sync_responder_checkpoint_signing_bytes_v2(&content)?),
            content,
        };
        verify_signed_sync_responder_checkpoint_v2(&successor)?;
        Ok(successor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncV2DurableResponderAction {
    AwaitInventory,
    TransmitDiff(CanonicalSignedSyncDiffV2),
    RetransmitDiff(CanonicalSignedSyncDiffV2),
    TransmitComplete(CanonicalSyncCompleteV2),
    TransmitRejection(CanonicalSyncRejectedV2),
    Paused,
    Complete,
}

impl CanonicalSyncV2DurableResponderAction {
    pub fn frame(&self) -> Option<CanonicalSyncServerFrameV2> {
        match self {
            Self::TransmitDiff(diff) | Self::RetransmitDiff(diff) => {
                Some(CanonicalSyncServerFrameV2::Diff(diff.clone()))
            }
            Self::TransmitComplete(complete) => {
                Some(CanonicalSyncServerFrameV2::Complete(complete.clone()))
            }
            Self::TransmitRejection(rejected) => {
                Some(CanonicalSyncServerFrameV2::Rejected(rejected.clone()))
            }
            Self::AwaitInventory | Self::Paused | Self::Complete => None,
        }
    }

    fn kind(&self) -> CanonicalSyncV2DurableResponderActionKind {
        match self {
            Self::AwaitInventory => CanonicalSyncV2DurableResponderActionKind::AwaitInventory,
            Self::TransmitDiff(_) => CanonicalSyncV2DurableResponderActionKind::TransmitDiff,
            Self::RetransmitDiff(_) => CanonicalSyncV2DurableResponderActionKind::RetransmitDiff,
            Self::TransmitComplete(_) => {
                CanonicalSyncV2DurableResponderActionKind::TransmitComplete
            }
            Self::TransmitRejection(_) => {
                CanonicalSyncV2DurableResponderActionKind::TransmitRejection
            }
            Self::Paused => CanonicalSyncV2DurableResponderActionKind::Paused,
            Self::Complete => CanonicalSyncV2DurableResponderActionKind::Complete,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanonicalSyncV2DurableResponderActionKind {
    AwaitInventory,
    TransmitDiff,
    RetransmitDiff,
    TransmitComplete,
    TransmitRejection,
    Paused,
    Complete,
}

impl CanonicalSyncV2DurableResponderActionKind {
    const fn label(self) -> &'static str {
        match self {
            Self::AwaitInventory => "await-inventory",
            Self::TransmitDiff => "transmit-diff",
            Self::RetransmitDiff => "retransmit-diff",
            Self::TransmitComplete => "transmit-complete",
            Self::TransmitRejection => "transmit-rejection",
            Self::Paused => "paused",
            Self::Complete => "complete",
        }
    }
}

pub enum CanonicalSyncV2ServerInventoryOutcome {
    Accepted(Box<CanonicalSyncV2ServerInventoryRound>),
    Rejected(CanonicalSyncRejectedV2),
}

impl CanonicalSyncV2ServerInventoryOutcome {
    pub fn rejected_frame(&self) -> Option<CanonicalSyncServerFrameV2> {
        match self {
            Self::Accepted(_) => None,
            Self::Rejected(rejected) => {
                Some(CanonicalSyncServerFrameV2::Rejected(rejected.clone()))
            }
        }
    }
}

pub struct CanonicalSyncV2ServerInventoryRound {
    requested_event_keys: Vec<CanonicalSyncEventKeyV2>,
    sent_events: usize,
    more_available: bool,
    diff: CanonicalSignedSyncDiffV2,
}

impl CanonicalSyncV2ServerInventoryRound {
    pub fn diff(&self) -> &CanonicalSignedSyncDiffV2 {
        &self.diff
    }

    pub fn frame(&self) -> CanonicalSyncServerFrameV2 {
        CanonicalSyncServerFrameV2::Diff(self.diff.clone())
    }
}

pub struct CanonicalSyncV2ServerCompletion {
    response: CanonicalSyncCompleteV2,
    stats: CanonicalSyncV2RoundStats,
}

impl CanonicalSyncV2ServerCompletion {
    pub fn response(&self) -> &CanonicalSyncCompleteV2 {
        &self.response
    }

    pub fn frame(&self) -> CanonicalSyncServerFrameV2 {
        CanonicalSyncServerFrameV2::Complete(self.response.clone())
    }

    pub fn stats(&self) -> CanonicalSyncV2RoundStats {
        self.stats
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2RoundStats {
    pub sent_events: usize,
    pub received_events: usize,
    pub more_available: bool,
}

#[derive(Debug, Error)]
pub enum CanonicalSyncV2SessionError {
    #[error(transparent)]
    Protocol(#[from] CanonicalSyncV2Error),

    #[error(transparent)]
    Checkpoint(#[from] CanonicalSyncSessionV2Error),

    #[error(transparent)]
    ResponderCheckpoint(#[from] CanonicalSyncResponderSessionV2Error),

    #[error(transparent)]
    Store(#[from] StoreError),

    #[error(transparent)]
    Policy(#[from] CanonicalSyncV2PolicyError),

    #[error("canonical sync v2 message belongs to a different conversation")]
    ConversationMismatch,

    #[error("canonical sync v2 message belongs to a different transport session")]
    SessionBindingMismatch,

    #[error("canonical sync v2 diff was signed by an unexpected responder")]
    UnexpectedResponder,

    #[error("responder requested typed Event key {0:?} outside the signed inventory")]
    UnadvertisedEventRequested(CanonicalSyncEventKeyV2),

    #[error("{field} contains duplicate typed Event key {key:?}")]
    DuplicateStoreEventKey {
        field: &'static str,
        key: CanonicalSyncEventKeyV2,
    },

    #[error("{field} contains {actual} typed Event keys; maximum is {maximum}")]
    StoreInventoryTooLarge {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("canonical sync v2 store did not return exactly the requested typed Event keys")]
    StoreEventKeysMismatch,

    #[error("canonical sync v2 event batch does not exactly satisfy requested typed Event keys")]
    BatchEventKeysMismatch,

    #[error("canonical sync v2 completion does not confirm exact sent typed Event keys")]
    CompletionEventKeysMismatch,

    #[error("canonical sync v2 completion disagrees about continuation state")]
    ContinuationMismatch,

    #[error("durable canonical sync v2 session is missing")]
    DurableSessionMissing,

    #[error("durable canonical sync v2 runtime belongs to another exact session identity")]
    DurableSessionIdentityMismatch,

    #[error("durable canonical sync v2 pending frame does not match checkpoint action {action:?}")]
    DurableRuntimeFrameMismatch { action: CanonicalSyncNextActionV2 },

    #[error("durable canonical sync v2 expected action {expected}, found {actual}")]
    UnexpectedDurableAction {
        expected: &'static str,
        actual: &'static str,
    },

    #[error("durable canonical sync v2 checkpoint sequence overflowed")]
    DurableCheckpointSequenceOverflow,

    #[error("durable canonical sync v2 round has no verified checkpoint baseline")]
    DurableRoundBaselineMissing,

    #[error(
        "durable canonical sync v2 responder pending frame does not match checkpoint action {action:?}"
    )]
    DurableResponderRuntimeFrameMismatch {
        action: CanonicalSyncResponderNextActionV2,
    },
}

struct CanonicalSyncV2Plan {
    requested_from_remote: Vec<CanonicalSyncEventKeyV2>,
    events_for_remote: Vec<CanonicalSyncEventCarrierV2>,
    more_available: bool,
}

fn plan_sync<S: CanonicalSyncV2Store + ?Sized, P: CanonicalSyncV2EventPolicy + ?Sized>(
    store: &S,
    policy: &P,
    conversation_id: [u8; 32],
    remote_inventory: &[CanonicalSyncEventKeyV2],
) -> Result<CanonicalSyncV2Plan, CanonicalSyncV2SessionError> {
    let local_inventory =
        normalized_inventory(store.inventory_v2(conversation_id)?, "server inventory")?;
    let local_keys: BTreeSet<_> = local_inventory.into_iter().collect();
    let remote_keys: BTreeSet<_> = remote_inventory.iter().copied().collect();
    let mut requested_from_remote: Vec<_> = remote_keys.difference(&local_keys).copied().collect();
    let mut events_for_remote_keys: Vec<_> = local_keys.difference(&remote_keys).copied().collect();
    let more_available = requested_from_remote.len() > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH
        || events_for_remote_keys.len() > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH;
    requested_from_remote.truncate(MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH);
    events_for_remote_keys.truncate(MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH);
    let events_for_remote =
        load_exact_events(store, policy, conversation_id, &events_for_remote_keys)?;
    Ok(CanonicalSyncV2Plan {
        requested_from_remote,
        events_for_remote,
        more_available,
    })
}

fn normalized_inventory(
    mut keys: Vec<CanonicalSyncEventKeyV2>,
    field: &'static str,
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncV2SessionError> {
    if keys.len() > MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS {
        return Err(CanonicalSyncV2SessionError::StoreInventoryTooLarge {
            field,
            actual: keys.len(),
            maximum: MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
        });
    }
    keys.sort_unstable();
    if let Some(pair) = keys.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(CanonicalSyncV2SessionError::DuplicateStoreEventKey {
            field,
            key: pair[0],
        });
    }
    Ok(keys)
}

fn load_exact_events<S: CanonicalSyncV2Store + ?Sized, P: CanonicalSyncV2EventPolicy + ?Sized>(
    store: &S,
    policy: &P,
    conversation_id: [u8; 32],
    requested_keys: &[CanonicalSyncEventKeyV2],
) -> Result<Vec<CanonicalSyncEventCarrierV2>, CanonicalSyncV2SessionError> {
    let events = store.events_by_key_v2(conversation_id, requested_keys)?;
    validate_events(policy, conversation_id, &events)?;

    let mut keyed_events = events
        .into_iter()
        .map(|event| {
            event
                .key()
                .map(|key| (key, event))
                .map_err(CanonicalSyncV2Error::from)
                .map_err(CanonicalSyncV2SessionError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed_events.sort_unstable_by_key(|(key, _)| *key);
    if keyed_events
        .iter()
        .map(|(key, _)| *key)
        .ne(requested_keys.iter().copied())
    {
        return Err(CanonicalSyncV2SessionError::StoreEventKeysMismatch);
    }
    Ok(keyed_events.into_iter().map(|(_, event)| event).collect())
}

fn validate_events<P: CanonicalSyncV2EventPolicy + ?Sized>(
    policy: &P,
    conversation_id: [u8; 32],
    events: &[CanonicalSyncEventCarrierV2],
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncV2SessionError> {
    let mut keys = Vec::with_capacity(events.len());
    for event in events {
        if event.conversation_id() != &conversation_id {
            return Err(CanonicalSyncV2SessionError::ConversationMismatch);
        }
        event.encode().map_err(CanonicalSyncV2Error::from)?;
        policy.verify_event(conversation_id, event)?;
        keys.push(event.key().map_err(CanonicalSyncV2Error::from)?);
    }
    Ok(keys)
}

fn event_keys(
    events: &[CanonicalSyncEventCarrierV2],
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncV2SessionError> {
    events
        .iter()
        .map(|event| {
            event
                .key()
                .map_err(CanonicalSyncV2Error::from)
                .map_err(CanonicalSyncV2SessionError::from)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, error::Error, io};

    use kilogram_identity::{AccountRootState, ConversationScopeId};
    use kilogram_protocol::canonical_account_v1::{
        CanonicalConversationMembershipContentV1, CanonicalSignedConversationMembershipV1,
        conversation_membership_signing_bytes_v1,
    };
    use kilogram_protocol::canonical_sync_history_v2::CanonicalSyncHistoryV2Error;
    use kilogram_protocol::canonical_sync_v2::{
        decode_and_verify_sync_client_frame_v2, decode_and_verify_sync_server_frame_v2,
        encode_sync_client_frame_v2, encode_sync_server_frame_v2,
    };
    use serde_json::Value;
    use tempfile::tempdir;

    use super::*;

    struct EncodingPolicy;

    impl CanonicalSyncV2EventPolicy for EncodingPolicy {
        fn verify_event(
            &self,
            conversation_id: [u8; 32],
            event: &CanonicalSyncEventCarrierV2,
        ) -> Result<(), CanonicalSyncV2PolicyError> {
            if event.conversation_id() != &conversation_id {
                return Err(CanonicalSyncV2PolicyError::rejected(
                    "conversation mismatch",
                ));
            }
            event
                .encode()
                .map_err(|error| CanonicalSyncV2PolicyError::rejected(error.to_string()))?;
            Ok(())
        }
    }

    struct RejectAllPolicy;

    impl CanonicalSyncV2EventPolicy for RejectAllPolicy {
        fn verify_event(
            &self,
            _conversation_id: [u8; 32],
            _event: &CanonicalSyncEventCarrierV2,
        ) -> Result<(), CanonicalSyncV2PolicyError> {
            Err(CanonicalSyncV2PolicyError::rejected(
                "test membership rejection",
            ))
        }
    }

    struct MemoryStore {
        conversation_id: [u8; 32],
        events: RefCell<BTreeMap<CanonicalSyncEventKeyV2, CanonicalSyncEventCarrierV2>>,
        inventory_override: RefCell<Option<Vec<CanonicalSyncEventKeyV2>>>,
        omit_reads: bool,
    }

    impl MemoryStore {
        fn new(
            conversation_id: [u8; 32],
            events: Vec<CanonicalSyncEventCarrierV2>,
        ) -> Result<Self, CanonicalSyncHistoryV2Error> {
            let events = events
                .into_iter()
                .map(|event| -> Result<_, CanonicalSyncHistoryV2Error> {
                    Ok((event.key()?, event))
                })
                .collect::<Result<_, _>>()?;
            Ok(Self {
                conversation_id,
                events: RefCell::new(events),
                inventory_override: RefCell::new(None),
                omit_reads: false,
            })
        }

        fn io_error(message: impl Into<String>) -> StoreError {
            StoreError::Io(io::Error::new(io::ErrorKind::InvalidData, message.into()))
        }
    }

    impl CanonicalSyncV2Store for MemoryStore {
        fn inventory_v2(
            &self,
            conversation_id: [u8; 32],
        ) -> Result<Vec<CanonicalSyncEventKeyV2>, StoreError> {
            if conversation_id != self.conversation_id {
                return Err(Self::io_error("test conversation mismatch"));
            }
            if let Some(override_keys) = self.inventory_override.borrow().clone() {
                return Ok(override_keys);
            }
            Ok(self.events.borrow().keys().copied().collect())
        }

        fn events_by_key_v2(
            &self,
            conversation_id: [u8; 32],
            event_keys: &[CanonicalSyncEventKeyV2],
        ) -> Result<Vec<CanonicalSyncEventCarrierV2>, StoreError> {
            if conversation_id != self.conversation_id {
                return Err(Self::io_error("test conversation mismatch"));
            }
            if self.omit_reads {
                return Ok(Vec::new());
            }
            let events = self.events.borrow();
            event_keys
                .iter()
                .map(|key| {
                    events
                        .get(key)
                        .cloned()
                        .ok_or_else(|| Self::io_error("test event missing"))
                })
                .collect()
        }

        fn put_events_v2(
            &self,
            conversation_id: [u8; 32],
            events: &[CanonicalSyncEventCarrierV2],
        ) -> Result<(), StoreError> {
            if conversation_id != self.conversation_id {
                return Err(Self::io_error("test conversation mismatch"));
            }
            let mut stored = self.events.borrow_mut();
            for event in events {
                let key = event
                    .key()
                    .map_err(|error| Self::io_error(error.to_string()))?;
                if let Some(existing) = stored.get(&key) {
                    if existing != event {
                        return Err(Self::io_error("test immutable conflict"));
                    }
                } else {
                    stored.insert(key, event.clone());
                }
            }
            Ok(())
        }
    }

    fn vectors() -> Result<Value, Box<dyn Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
        )))?)
    }

    fn carrier(pointer: &str) -> Result<CanonicalSyncEventCarrierV2, Box<dyn Error>> {
        let document = vectors()?;
        let hex = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing carrier vector"))?;
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CanonicalSyncEventCarrierV2::decode_and_verify(&bytes)?)
    }

    fn carrier_author_account_id(carrier: &CanonicalSyncEventCarrierV2) -> [u8; 32] {
        match carrier {
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(legacy) => *legacy.author_account_id(),
            CanonicalSyncEventCarrierV2::CanonicalCborV1(canonical) => {
                canonical.author_certificate.content.account_id
            }
        }
    }

    fn legacy_membership(
        conversation_id: [u8; 32],
        members: &[[u8; 32]],
    ) -> Result<ConversationMembershipSnapshot, Box<dyn Error>> {
        let directory = tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let members = members
            .iter()
            .copied()
            .map(AccountId::from_bytes)
            .collect::<Vec<_>>();
        Ok(root.create_conversation_membership(
            ConversationScopeId::from_bytes(conversation_id),
            &members,
        )?)
    }

    fn canonical_membership(
        conversation_id: [u8; 32],
        members: &[[u8; 32]],
    ) -> Result<CanonicalSignedConversationMembershipV1, Box<dyn Error>> {
        let owner = DeviceIdentity::from_secret_bytes([0x93; 32]);
        let owner_account_id = *owner.device_id().as_bytes();
        let mut members = members.to_vec();
        members.push(owner_account_id);
        members.sort_unstable();
        members.dedup();
        let content = CanonicalConversationMembershipContentV1 {
            conversation_id,
            revision: 1,
            owner_account_id,
            members,
        };
        let signature = owner.sign(&conversation_membership_signing_bytes_v1(&content)?);
        Ok(CanonicalSignedConversationMembershipV1 { content, signature })
    }

    fn decode_client_frame(
        frame: CanonicalSyncClientFrameV2,
    ) -> Result<CanonicalSyncClientFrameV2, CanonicalSyncV2Error> {
        decode_and_verify_sync_client_frame_v2(&encode_sync_client_frame_v2(&frame)?)
    }

    fn decode_server_frame(
        frame: CanonicalSyncServerFrameV2,
    ) -> Result<CanonicalSyncServerFrameV2, CanonicalSyncV2Error> {
        decode_and_verify_sync_server_frame_v2(&encode_sync_server_frame_v2(&frame)?)
    }

    fn converge_one_carrier(
        carrier: CanonicalSyncEventCarrierV2,
        source_on_client: bool,
    ) -> Result<(), Box<dyn Error>> {
        let conversation_id = *carrier.conversation_id();
        let client_store = MemoryStore::new(
            conversation_id,
            if source_on_client {
                vec![carrier.clone()]
            } else {
                Vec::new()
            },
        )?;
        let server_store = MemoryStore::new(
            conversation_id,
            if source_on_client {
                Vec::new()
            } else {
                vec![carrier]
            },
        )?;
        let client_identity = DeviceIdentity::from_secret_bytes([0x21; 32]);
        let server_identity = DeviceIdentity::from_secret_bytes([0x41; 32]);
        let session_binding = SyncSessionBinding::from_transport_label("dormant-cbor-v2-test");
        let policy = EncodingPolicy;
        let client = CanonicalSyncV2Client::new(
            &client_identity,
            &client_store,
            &policy,
            conversation_id,
            session_binding,
            server_identity.device_id(),
        );
        let server = CanonicalSyncV2Server::new(
            &server_identity,
            &server_store,
            &policy,
            conversation_id,
            session_binding,
            client_identity.device_id(),
        );

        let client_inventory_round = client.begin_round()?;
        let inventory = match decode_client_frame(client_inventory_round.frame())? {
            CanonicalSyncClientFrameV2::Inventory(inventory) => inventory,
            _ => return Err(io::Error::other("unexpected client frame").into()),
        };
        let server_inventory_round = match server.accept_inventory(&inventory)? {
            CanonicalSyncV2ServerInventoryOutcome::Accepted(round) => *round,
            CanonicalSyncV2ServerInventoryOutcome::Rejected(_) => {
                return Err(io::Error::other("unexpected rejection").into());
            }
        };
        let diff = match decode_server_frame(server_inventory_round.frame())? {
            CanonicalSyncServerFrameV2::Diff(diff) => diff,
            _ => return Err(io::Error::other("unexpected server frame").into()),
        };
        let client_batch_round = client.accept_diff(client_inventory_round, diff)?;
        let batch = match decode_client_frame(client_batch_round.frame())? {
            CanonicalSyncClientFrameV2::Events(batch) => batch,
            _ => return Err(io::Error::other("unexpected client frame").into()),
        };
        let completion = server.complete_round(server_inventory_round, batch)?;
        assert_eq!(
            completion.stats(),
            CanonicalSyncV2RoundStats {
                sent_events: usize::from(!source_on_client),
                received_events: usize::from(source_on_client),
                more_available: false,
            }
        );
        let complete = match decode_server_frame(completion.frame())? {
            CanonicalSyncServerFrameV2::Complete(complete) => complete,
            _ => return Err(io::Error::other("unexpected server frame").into()),
        };
        assert_eq!(
            client.accept_complete(client_batch_round, complete)?,
            CanonicalSyncV2RoundStats {
                sent_events: usize::from(source_on_client),
                received_events: usize::from(!source_on_client),
                more_available: false,
            }
        );
        assert_eq!(
            client_store.inventory_v2(conversation_id)?,
            server_store.inventory_v2(conversation_id)?
        );
        Ok(())
    }

    #[test]
    fn full_frame_round_converges_legacy_and_canonical_carriers() -> Result<(), Box<dyn Error>> {
        for pointer in [
            "/expected/legacy_carrier_cbor_hex",
            "/expected/canonical_carrier_cbor_hex",
        ] {
            let event = carrier(pointer)?;
            converge_one_carrier(event.clone(), true)?;
            converge_one_carrier(event, false)?;
        }
        Ok(())
    }

    #[test]
    fn durable_typed_store_trait_survives_reopen() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let legacy = carrier("/expected/legacy_carrier_cbor_hex")?;
        let canonical = carrier("/expected/canonical_carrier_cbor_hex")?;
        let legacy_conversation_id = *legacy.conversation_id();
        let canonical_conversation_id = *canonical.conversation_id();
        let legacy_key = legacy.key()?;
        let canonical_key = canonical.key()?;
        let store = CanonicalSyncV2CarrierStore::open(directory.path())?;
        CanonicalSyncV2Store::put_events_v2(
            &store,
            legacy_conversation_id,
            std::slice::from_ref(&legacy),
        )?;
        CanonicalSyncV2Store::put_events_v2(
            &store,
            canonical_conversation_id,
            std::slice::from_ref(&canonical),
        )?;
        drop(store);

        let reopened = CanonicalSyncV2CarrierStore::open(directory.path())?;
        assert_eq!(
            CanonicalSyncV2Store::inventory_v2(&reopened, legacy_conversation_id)?,
            vec![legacy_key]
        );
        assert_eq!(
            CanonicalSyncV2Store::events_by_key_v2(
                &reopened,
                canonical_conversation_id,
                &[canonical_key]
            )?,
            vec![canonical]
        );
        assert_eq!(
            CanonicalSyncV2Store::events_by_key_v2(
                &reopened,
                legacy_conversation_id,
                &[legacy_key]
            )?,
            vec![legacy]
        );
        Ok(())
    }

    #[test]
    fn concrete_client_bootstrap_requires_the_completed_exact_legacy_import()
    -> Result<(), Box<dyn Error>> {
        let source_directory = tempdir()?;
        let target_directory = tempdir()?;
        let legacy = carrier("/expected/legacy_carrier_cbor_hex")?;
        let conversation_id = *legacy.conversation_id();
        let author_account_id = carrier_author_account_id(&legacy);
        let membership = legacy_membership(conversation_id, &[author_account_id])?;
        let authorized_event = match legacy {
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge) => {
                bridge.into_active_authorized_event()
            }
            CanonicalSyncEventCarrierV2::CanonicalCborV1(_) => {
                return Err(io::Error::other("legacy vector decoded as canonical carrier").into());
            }
        };
        let source = kilogram_store::EventStore::open(source_directory.path())?;
        source.put_authorized(&authorized_event, &membership)?;
        let plan = source.canonical_sync_v2_legacy_import_plan(
            authorized_event.event().conversation_id(),
            &membership,
        )?;
        let target = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let policy = CanonicalSyncV2MembershipPolicy::from_legacy(membership.clone())?;
        let identity = DeviceIdentity::from_secret_bytes([0x24; 32]);
        let peer = DeviceIdentity::from_secret_bytes([0x44; 32]);
        let session_binding = SyncSessionBinding::from_transport_label(
            "concrete-bootstrap-after-completed-legacy-import",
        );

        assert!(matches!(
            CanonicalSyncV2Client::bootstrap_from_completed_legacy_import(
                &identity,
                &target,
                &policy,
                &plan,
                session_binding,
                peer.device_id(),
            ),
            Err(CanonicalSyncV2SessionError::Store(
                StoreError::CanonicalSyncV2SessionBootstrapImportMissing
            ))
        ));

        let progress = target.import_legacy_page(
            &source,
            &membership,
            &plan,
            plan.start_cursor(),
            MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        )?;
        assert!(progress.is_complete());
        let (client, stored) = CanonicalSyncV2Client::bootstrap_from_completed_legacy_import(
            &identity,
            &target,
            &policy,
            &plan,
            session_binding,
            peer.device_id(),
        )?;
        assert_eq!(stored.import_plan_id(), plan.plan_id());
        assert_eq!(stored.checkpoint().content.checkpoint_sequence, 1);
        assert_eq!(stored.checkpoint().content.sync_round, 1);
        assert_eq!(
            stored.checkpoint().content.next_action,
            CanonicalSyncNextActionV2::SendInventory
        );
        assert_eq!(
            target.session_checkpoint_head(
                conversation_id,
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            )?,
            Some(stored)
        );

        let round = client.begin_round()?;
        verify_signed_sync_inventory_v2(round.inventory())?;
        assert_eq!(round.inventory().content.conversation_id, conversation_id);
        assert_eq!(
            round.inventory().content.event_keys,
            target.inventory(conversation_id)?
        );
        Ok(())
    }

    #[test]
    fn durable_client_transitions_resume_exact_frames_and_complete_two_rounds()
    -> Result<(), Box<dyn Error>> {
        let source_directory = tempdir()?;
        let target_directory = tempdir()?;
        let incoming = carrier("/expected/legacy_carrier_cbor_hex")?;
        let incoming_key = incoming.key()?;
        let conversation_id = *incoming.conversation_id();
        let author_account_id = carrier_author_account_id(&incoming);
        let membership = legacy_membership(conversation_id, &[author_account_id])?;
        let active_event = match incoming.clone() {
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge) => {
                bridge.into_active_authorized_event()
            }
            CanonicalSyncEventCarrierV2::CanonicalCborV1(_) => {
                return Err(io::Error::other("legacy vector decoded as canonical carrier").into());
            }
        };
        let source = kilogram_store::EventStore::open(source_directory.path())?;
        let plan = source.canonical_sync_v2_legacy_import_plan(
            active_event.event().conversation_id(),
            &membership,
        )?;
        let target = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let imported = target.import_legacy_page(
            &source,
            &membership,
            &plan,
            plan.start_cursor(),
            MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        )?;
        assert!(imported.is_complete());
        assert!(target.inventory(conversation_id)?.is_empty());

        let policy = CanonicalSyncV2MembershipPolicy::from_legacy(membership)?;
        let identity = DeviceIdentity::from_secret_bytes([0x25; 32]);
        let peer = DeviceIdentity::from_secret_bytes([0x45; 32]);
        let session_binding =
            SyncSessionBinding::from_transport_label("durable-client-two-round-resume");
        let (client, _) = CanonicalSyncV2Client::bootstrap_from_completed_legacy_import(
            &identity,
            &target,
            &policy,
            &plan,
            session_binding,
            peer.device_id(),
        )?;
        let initial = client.restore_durable_runtime()?;
        assert_eq!(
            client.durable_action(&initial)?,
            CanonicalSyncV2DurableClientAction::PrepareInventory
        );

        let (inventory_round, receive_diff) = client.begin_durable_round(&initial)?;
        assert_eq!(
            client.durable_action(&receive_diff)?.frame(),
            Some(inventory_round.frame())
        );
        let diff_content = CanonicalSyncDiffContentV2 {
            conversation_id,
            responder_device_id: *peer.device_id().as_bytes(),
            session_binding: *session_binding.as_bytes(),
            requested_event_keys: Vec::new(),
            events: vec![incoming.clone()],
            more_available: true,
        };
        let diff = CanonicalSignedSyncDiffV2 {
            signature: peer.sign(&sync_diff_signing_bytes_v2(&diff_content)?),
            content: diff_content,
        };
        let (batch_round, send_batch) = client.accept_durable_diff(&receive_diff, diff.clone())?;
        assert_eq!(target.inventory(conversation_id)?, vec![incoming_key]);
        assert_eq!(
            client.durable_action(&send_batch)?.frame(),
            Some(batch_round.frame())
        );
        let (_, exact_diff_retry) = client.accept_durable_diff(&receive_diff, diff)?;
        assert_eq!(exact_diff_retry, send_batch);

        let receive_complete = client.mark_durable_batch_sent(&send_batch)?;
        assert_eq!(
            client.durable_action(&receive_complete)?.frame(),
            Some(batch_round.frame())
        );
        assert_eq!(
            client.mark_durable_batch_sent(&send_batch)?,
            receive_complete
        );
        let first_complete = CanonicalSyncCompleteV2 {
            conversation_id,
            stored_event_keys: Vec::new(),
            more_available: true,
        };
        let (first_stats, second_round_ready) =
            client.accept_durable_complete(&receive_complete, first_complete)?;
        assert_eq!(
            first_stats,
            CanonicalSyncV2RoundStats {
                sent_events: 0,
                received_events: 1,
                more_available: true,
            }
        );
        assert_eq!(
            second_round_ready
                .checkpoint()
                .checkpoint()
                .content
                .sync_round,
            2
        );
        assert_eq!(
            client.durable_action(&second_round_ready)?,
            CanonicalSyncV2DurableClientAction::PrepareInventory
        );

        drop(target);
        let reopened = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let restarted = CanonicalSyncV2Client::new(
            &identity,
            &reopened,
            &policy,
            conversation_id,
            session_binding,
            peer.device_id(),
        );
        let restored = restarted.restore_durable_runtime()?;
        assert_eq!(restored, second_round_ready);
        let (second_inventory, second_receive_diff) = restarted.begin_durable_round(&restored)?;
        assert_eq!(
            second_inventory.inventory().content.event_keys,
            vec![incoming_key]
        );
        let empty_diff_content = CanonicalSyncDiffContentV2 {
            conversation_id,
            responder_device_id: *peer.device_id().as_bytes(),
            session_binding: *session_binding.as_bytes(),
            requested_event_keys: Vec::new(),
            events: Vec::new(),
            more_available: false,
        };
        let empty_diff = CanonicalSignedSyncDiffV2 {
            signature: peer.sign(&sync_diff_signing_bytes_v2(&empty_diff_content)?),
            content: empty_diff_content,
        };
        let (empty_batch, second_send_batch) =
            restarted.accept_durable_diff(&second_receive_diff, empty_diff)?;
        assert!(empty_batch.batch().events.is_empty());
        let second_receive_complete = restarted.mark_durable_batch_sent(&second_send_batch)?;
        let terminal_message = CanonicalSyncCompleteV2 {
            conversation_id,
            stored_event_keys: Vec::new(),
            more_available: false,
        };
        let (terminal_stats, terminal) = restarted
            .accept_durable_complete(&second_receive_complete, terminal_message.clone())?;
        assert_eq!(
            terminal_stats,
            CanonicalSyncV2RoundStats {
                sent_events: 0,
                received_events: 0,
                more_available: false,
            }
        );
        assert_eq!(
            restarted.durable_action(&terminal)?,
            CanonicalSyncV2DurableClientAction::Complete
        );
        let (_, exact_complete_retry) =
            restarted.accept_durable_complete(&second_receive_complete, terminal_message)?;
        assert_eq!(exact_complete_retry, terminal);
        assert_eq!(
            reopened
                .session_checkpoint_chain(
                    conversation_id,
                    *identity.device_id().as_bytes(),
                    *peer.device_id().as_bytes(),
                    *session_binding.as_bytes(),
                )?
                .len(),
            9
        );
        Ok(())
    }

    #[test]
    fn durable_responder_transitions_resume_exact_frames_and_commit_batch_atomically()
    -> Result<(), Box<dyn Error>> {
        let source_directory = tempdir()?;
        let target_directory = tempdir()?;
        let incoming = carrier("/expected/legacy_carrier_cbor_hex")?;
        let incoming_key = incoming.key()?;
        let conversation_id = *incoming.conversation_id();
        let author_account_id = carrier_author_account_id(&incoming);
        let membership = legacy_membership(conversation_id, &[author_account_id])?;
        let active_event = match incoming.clone() {
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge) => {
                bridge.into_active_authorized_event()
            }
            CanonicalSyncEventCarrierV2::CanonicalCborV1(_) => {
                return Err(io::Error::other("legacy vector decoded as canonical carrier").into());
            }
        };
        let source = kilogram_store::EventStore::open(source_directory.path())?;
        let plan = source.canonical_sync_v2_legacy_import_plan(
            active_event.event().conversation_id(),
            &membership,
        )?;
        let target = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let imported = target.import_legacy_page(
            &source,
            &membership,
            &plan,
            plan.start_cursor(),
            MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        )?;
        assert!(imported.is_complete());

        let policy = CanonicalSyncV2MembershipPolicy::from_legacy(membership)?;
        let responder = DeviceIdentity::from_secret_bytes([0x26; 32]);
        let requester = DeviceIdentity::from_secret_bytes([0x46; 32]);
        let session_binding = SyncSessionBinding::from_transport_label("durable-responder-restart");
        let receive_batch;
        {
            let (server, _) =
                CanonicalSyncV2Server::bootstrap_responder_from_completed_legacy_import(
                    &responder,
                    &target,
                    &policy,
                    &plan,
                    session_binding,
                    requester.device_id(),
                )?;
            let initial = server.restore_durable_responder_runtime()?;
            assert_eq!(
                server.durable_responder_action(&initial)?,
                CanonicalSyncV2DurableResponderAction::AwaitInventory
            );

            let inventory_content = CanonicalSyncInventoryContentV2 {
                conversation_id,
                requester_device_id: *requester.device_id().as_bytes(),
                session_binding: *session_binding.as_bytes(),
                event_keys: vec![incoming_key],
            };
            let inventory = CanonicalSignedSyncInventoryV2 {
                signature: requester.sign(&sync_inventory_signing_bytes_v2(&inventory_content)?),
                content: inventory_content,
            };
            let send_diff = server.accept_durable_inventory(&initial, &inventory)?;
            let diff = match server.durable_responder_action(&send_diff)? {
                CanonicalSyncV2DurableResponderAction::TransmitDiff(diff) => diff,
                other => {
                    return Err(io::Error::other(format!("unexpected action: {other:?}")).into());
                }
            };
            assert_eq!(diff.content.requested_event_keys, vec![incoming_key]);
            assert!(diff.content.events.is_empty());
            assert!(!diff.content.more_available);
            assert_eq!(
                server.accept_durable_inventory(&initial, &inventory)?,
                send_diff
            );

            receive_batch = server.mark_durable_diff_sent(&send_diff)?;
            assert_eq!(
                server.durable_responder_action(&receive_batch)?.frame(),
                Some(CanonicalSyncServerFrameV2::Diff(diff.clone()))
            );
            assert_eq!(server.mark_durable_diff_sent(&send_diff)?, receive_batch);
        }

        drop(target);
        let reopened = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let restarted = CanonicalSyncV2Server::new(
            &responder,
            &reopened,
            &policy,
            conversation_id,
            session_binding,
            requester.device_id(),
        );
        let restored = restarted.restore_durable_responder_runtime()?;
        assert_eq!(restored, receive_batch);

        let batch = CanonicalSyncEventBatchV2 {
            conversation_id,
            events: vec![incoming.clone()],
        };
        let (stats, send_complete) =
            restarted.accept_durable_event_batch(&restored, batch.clone())?;
        assert_eq!(
            stats,
            CanonicalSyncV2RoundStats {
                sent_events: 0,
                received_events: 1,
                more_available: false,
            }
        );
        assert_eq!(reopened.inventory(conversation_id)?, vec![incoming_key]);
        let complete = match restarted.durable_responder_action(&send_complete)? {
            CanonicalSyncV2DurableResponderAction::TransmitComplete(complete) => complete,
            other => return Err(io::Error::other(format!("unexpected action: {other:?}")).into()),
        };
        assert_eq!(complete.stored_event_keys, vec![incoming_key]);
        assert_eq!(
            restarted.accept_durable_event_batch(&restored, batch)?.1,
            send_complete
        );

        let terminal = restarted.mark_durable_complete_sent(&send_complete)?;
        assert_eq!(
            restarted.durable_responder_action(&terminal)?,
            CanonicalSyncV2DurableResponderAction::Complete
        );
        assert_eq!(
            restarted.mark_durable_complete_sent(&send_complete)?,
            terminal
        );
        assert_eq!(
            reopened
                .responder_checkpoint_chain(
                    conversation_id,
                    *responder.device_id().as_bytes(),
                    *requester.device_id().as_bytes(),
                    *session_binding.as_bytes(),
                )?
                .len(),
            5
        );
        Ok(())
    }

    #[test]
    fn concrete_membership_policy_accepts_both_carriers_with_both_snapshot_formats()
    -> Result<(), Box<dyn Error>> {
        for carrier in [
            carrier("/expected/legacy_carrier_cbor_hex")?,
            carrier("/expected/canonical_carrier_cbor_hex")?,
        ] {
            let conversation_id = *carrier.conversation_id();
            let author_account_id = carrier_author_account_id(&carrier);

            let legacy = CanonicalSyncV2MembershipPolicy::from_legacy(legacy_membership(
                conversation_id,
                &[author_account_id],
            )?)?;
            assert_eq!(legacy.conversation_id(), conversation_id);
            assert_eq!(legacy.revision(), 1);
            legacy.verify_event(conversation_id, &carrier)?;

            let canonical = CanonicalSyncV2MembershipPolicy::from_canonical(canonical_membership(
                conversation_id,
                &[author_account_id],
            )?)?;
            assert_eq!(canonical.conversation_id(), conversation_id);
            assert_eq!(canonical.revision(), 1);
            canonical.verify_event(conversation_id, &carrier)?;
        }
        Ok(())
    }

    #[test]
    fn concrete_membership_policy_rejects_nonmembers_mismatch_and_invalid_snapshot()
    -> Result<(), Box<dyn Error>> {
        let carrier = carrier("/expected/canonical_carrier_cbor_hex")?;
        let conversation_id = *carrier.conversation_id();

        let legacy =
            CanonicalSyncV2MembershipPolicy::from_legacy(legacy_membership(conversation_id, &[])?)?;
        assert!(legacy.verify_event(conversation_id, &carrier).is_err());
        assert!(legacy.verify_event([0x44; 32], &carrier).is_err());

        let mut invalid_legacy_bytes = legacy_membership(conversation_id, &[])?.encode()?;
        let Some(last_byte) = invalid_legacy_bytes.last_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a signed legacy membership encoded as empty bytes",
            )
            .into());
        };
        *last_byte ^= 1;
        assert!(ConversationMembershipSnapshot::decode_and_verify(&invalid_legacy_bytes).is_err());

        let canonical = CanonicalSyncV2MembershipPolicy::from_canonical(canonical_membership(
            conversation_id,
            &[],
        )?)?;
        assert!(canonical.verify_event(conversation_id, &carrier).is_err());

        let mut invalid =
            canonical_membership(conversation_id, &[carrier_author_account_id(&carrier)])?;
        invalid.signature[0] ^= 1;
        assert!(CanonicalSyncV2MembershipPolicy::from_canonical(invalid).is_err());
        Ok(())
    }

    #[test]
    fn membership_policy_rejection_prevents_storage() -> Result<(), Box<dyn Error>> {
        let event = carrier("/expected/legacy_carrier_cbor_hex")?;
        let conversation_id = *event.conversation_id();
        let client_store = MemoryStore::new(conversation_id, Vec::new())?;
        let server_store = MemoryStore::new(conversation_id, vec![event])?;
        let client_identity = DeviceIdentity::from_secret_bytes([0x22; 32]);
        let server_identity = DeviceIdentity::from_secret_bytes([0x42; 32]);
        let session_binding = SyncSessionBinding::from_transport_label("policy-rejection");
        let client = CanonicalSyncV2Client::new(
            &client_identity,
            &client_store,
            &RejectAllPolicy,
            conversation_id,
            session_binding,
            server_identity.device_id(),
        );
        let server = CanonicalSyncV2Server::new(
            &server_identity,
            &server_store,
            &EncodingPolicy,
            conversation_id,
            session_binding,
            client_identity.device_id(),
        );
        let client_round = client.begin_round()?;
        let server_round = match server.accept_inventory(client_round.inventory())? {
            CanonicalSyncV2ServerInventoryOutcome::Accepted(round) => *round,
            CanonicalSyncV2ServerInventoryOutcome::Rejected(_) => unreachable!(),
        };
        assert!(matches!(
            client.accept_diff(client_round, server_round.diff().clone()),
            Err(CanonicalSyncV2SessionError::Policy(_))
        ));
        assert!(client_store.inventory_v2(conversation_id)?.is_empty());
        Ok(())
    }

    #[test]
    fn duplicate_inventory_and_incomplete_store_reads_fail_closed() -> Result<(), Box<dyn Error>> {
        let event = carrier("/expected/legacy_carrier_cbor_hex")?;
        let key = event.key()?;
        let conversation_id = *event.conversation_id();
        let duplicate_store = MemoryStore::new(conversation_id, vec![event.clone()])?;
        *duplicate_store.inventory_override.borrow_mut() = Some(vec![key, key]);
        let identity = DeviceIdentity::from_secret_bytes([0x23; 32]);
        let peer = DeviceIdentity::from_secret_bytes([0x43; 32]);
        let session_binding = SyncSessionBinding::from_transport_label("store-fail-closed");
        let client = CanonicalSyncV2Client::new(
            &identity,
            &duplicate_store,
            &EncodingPolicy,
            conversation_id,
            session_binding,
            peer.device_id(),
        );
        assert!(matches!(
            client.begin_round(),
            Err(CanonicalSyncV2SessionError::DuplicateStoreEventKey { .. })
        ));

        let requester_store = MemoryStore::new(conversation_id, Vec::new())?;
        let responder_store = MemoryStore {
            omit_reads: true,
            ..MemoryStore::new(conversation_id, vec![event])?
        };
        let requester = CanonicalSyncV2Client::new(
            &identity,
            &requester_store,
            &EncodingPolicy,
            conversation_id,
            session_binding,
            peer.device_id(),
        );
        let responder = CanonicalSyncV2Server::new(
            &peer,
            &responder_store,
            &EncodingPolicy,
            conversation_id,
            session_binding,
            identity.device_id(),
        );
        let round = requester.begin_round()?;
        assert!(matches!(
            responder.accept_inventory(round.inventory()),
            Err(CanonicalSyncV2SessionError::StoreEventKeysMismatch)
        ));
        Ok(())
    }
}
