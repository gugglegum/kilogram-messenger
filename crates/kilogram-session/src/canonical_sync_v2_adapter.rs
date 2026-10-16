//! Dormant transport-independent state machine for canonical sync v2.
//!
//! This module deliberately has no `EventStore` implementation and is not
//! referenced by the CLI or Iroh transport. It proves the state transitions
//! needed by `kilogram/m1/sync/cbor/2` while leaving the active Postcard v6
//! runtime unchanged. Activation additionally requires a store that preserves
//! both typed event namespaces and a policy that verifies conversation
//! membership for both closed carrier variants.

use std::collections::{BTreeSet, HashSet};

use kilogram_identity::{DeviceId, DeviceIdentity};
use kilogram_protocol::{
    SyncSessionBinding,
    canonical_sync_history_v2::{CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2},
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
use kilogram_store::{CanonicalSyncV2CarrierStore, StoreError};
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

        let received_event_keys =
            validate_events(self.policy, self.conversation_id, &diff.content.events)?;
        self.store
            .put_events_v2(self.conversation_id, &diff.content.events)?;

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

        Ok(CanonicalSyncV2ClientBatchRound {
            batch,
            sent_event_keys,
            received_event_keys,
            more_available: diff.content.more_available,
        })
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
