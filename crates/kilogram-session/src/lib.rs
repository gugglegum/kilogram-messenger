use std::collections::HashSet;

use kilogram_identity::{
    AccountId, AuthorizedDevice, ConversationMembershipSnapshot, DeviceCapability, DeviceId,
    DeviceIdentity, IdentityError, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    AuthorizedEvent, ConversationId, EventId, MAX_SYNC_EVENTS_PER_BATCH, ProtocolError,
    SignedDeviceSessionAuthorization, SignedSyncInventory, SyncComplete, SyncDiff, SyncEventBatch,
    SyncRejected, SyncRejectionReason, SyncSessionBinding,
};
use kilogram_store::{EventStore, StoreError};
use thiserror::Error;

/// Maximum number of bounded reconciliation rounds accepted on one connection.
pub const MAX_SYNC_ROUNDS: usize = 64;

pub fn authorize_device_session(
    expected_account: AccountId,
    authorization: &SignedDeviceSessionAuthorization,
    required_capabilities: &[DeviceCapability],
    expected_session: SyncSessionBinding,
) -> Result<AuthorizedDevice, SessionError> {
    authorization.verify_for_session(expected_session)?;
    Ok(verify_device_authorization_with_snapshot(
        expected_account,
        authorization.certificate(),
        authorization.authority_snapshot(),
        required_capabilities,
    )?)
}

pub trait SessionStore {
    fn inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<EventId>, StoreError>;

    fn events_by_id(
        &self,
        conversation_id: ConversationId,
        event_ids: &[EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError>;

    fn put_events(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError>;
}

impl SessionStore for EventStore {
    fn inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<EventId>, StoreError> {
        self.authorized_inventory(conversation_id, membership)
    }

    fn events_by_id(
        &self,
        conversation_id: ConversationId,
        event_ids: &[EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        self.authorized_events_by_id(conversation_id, event_ids, membership)
    }

    fn put_events(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError> {
        self.put_authorized_batch(events, membership)
    }
}

pub struct SyncClient<'a, S: SessionStore + ?Sized = EventStore> {
    identity: &'a DeviceIdentity,
    store: &'a S,
    conversation_id: ConversationId,
    membership: &'a ConversationMembershipSnapshot,
    session_binding: SyncSessionBinding,
    expected_responder: DeviceId,
}

impl<'a, S: SessionStore + ?Sized> SyncClient<'a, S> {
    pub fn new(
        identity: &'a DeviceIdentity,
        store: &'a S,
        conversation_id: ConversationId,
        membership: &'a ConversationMembershipSnapshot,
        session_binding: SyncSessionBinding,
        expected_responder: DeviceId,
    ) -> Self {
        Self {
            identity,
            store,
            conversation_id,
            membership,
            session_binding,
            expected_responder,
        }
    }

    pub fn begin_round(&self) -> Result<ClientInventoryRound, SessionError> {
        self.membership.verify()?;
        if self.membership.conversation_id() != self.conversation_id.scope_id() {
            return Err(SessionError::ConversationMismatch);
        }
        let event_ids = self
            .store
            .inventory(self.conversation_id, self.membership)?;
        let inventory = SignedSyncInventory::sign(
            self.identity,
            self.conversation_id,
            self.session_binding,
            event_ids.clone(),
        )?;
        Ok(ClientInventoryRound {
            inventory,
            advertised_event_ids: event_ids.into_iter().collect(),
        })
    }

    pub fn accept_diff(
        &self,
        round: ClientInventoryRound,
        diff: SyncDiff,
    ) -> Result<ClientBatchRound, SessionError> {
        diff.verify_for_session(self.session_binding, self.expected_responder)?;
        if diff.conversation_id() != self.conversation_id {
            return Err(SessionError::ConversationMismatch);
        }
        if let Some(event_id) = diff
            .requested_event_ids()
            .iter()
            .find(|event_id| !round.advertised_event_ids.contains(event_id))
        {
            return Err(SessionError::UnadvertisedEventRequested(*event_id));
        }

        let received_event_ids = event_ids(diff.events())?;
        for event in diff.events() {
            event.verify_for_membership(self.membership)?;
        }
        self.store.put_events(diff.events(), self.membership)?;
        let events_for_responder = self.store.events_by_id(
            self.conversation_id,
            diff.requested_event_ids(),
            self.membership,
        )?;
        let sent_event_ids = event_ids(&events_for_responder)?;
        let more_available = diff.more_available();
        let batch = SyncEventBatch::new(self.conversation_id, events_for_responder)?;

        Ok(ClientBatchRound {
            batch,
            sent_event_ids,
            received_event_ids,
            more_available,
        })
    }

    pub fn accept_complete(
        &self,
        round: ClientBatchRound,
        complete: SyncComplete,
    ) -> Result<SyncRoundStats, SessionError> {
        if complete.conversation_id() != self.conversation_id {
            return Err(SessionError::ConversationMismatch);
        }
        if !same_event_ids(complete.stored_event_ids(), &round.sent_event_ids) {
            return Err(SessionError::CompletionEventIdsMismatch);
        }
        if complete.more_available() != round.more_available {
            return Err(SessionError::ContinuationMismatch);
        }
        Ok(SyncRoundStats {
            sent_events: round.sent_event_ids.len(),
            received_events: round.received_event_ids.len(),
            more_available: round.more_available,
        })
    }
}

pub struct ClientInventoryRound {
    inventory: SignedSyncInventory,
    advertised_event_ids: HashSet<EventId>,
}

impl ClientInventoryRound {
    pub fn inventory(&self) -> &SignedSyncInventory {
        &self.inventory
    }

    pub fn inventory_event_count(&self) -> usize {
        self.advertised_event_ids.len()
    }
}

pub struct ClientBatchRound {
    batch: SyncEventBatch,
    sent_event_ids: Vec<EventId>,
    received_event_ids: Vec<EventId>,
    more_available: bool,
}

impl ClientBatchRound {
    pub fn batch(&self) -> &SyncEventBatch {
        &self.batch
    }
}

pub struct SyncServer<'a, S: SessionStore + ?Sized = EventStore> {
    identity: &'a DeviceIdentity,
    store: &'a S,
    session_binding: SyncSessionBinding,
    allowed_requester: DeviceId,
    membership: &'a ConversationMembershipSnapshot,
}

impl<'a, S: SessionStore + ?Sized> SyncServer<'a, S> {
    pub fn new(
        identity: &'a DeviceIdentity,
        store: &'a S,
        session_binding: SyncSessionBinding,
        allowed_requester: DeviceId,
        membership: &'a ConversationMembershipSnapshot,
    ) -> Self {
        Self {
            identity,
            store,
            session_binding,
            allowed_requester,
            membership,
        }
    }

    pub fn accept_inventory(
        &self,
        inventory: &SignedSyncInventory,
    ) -> Result<ServerInventoryOutcome, SessionError> {
        inventory.verify_for_session(self.session_binding)?;
        let conversation_id = inventory.conversation_id();
        self.membership.verify()?;
        if self.membership.conversation_id() != conversation_id.scope_id() {
            return Err(SessionError::ConversationMismatch);
        }
        if inventory.requester_device_id() != self.allowed_requester {
            return Ok(ServerInventoryOutcome::Rejected(SyncRejected::new(
                conversation_id,
                SyncRejectionReason::RequesterNotAllowed,
            )));
        }
        let plan = plan_sync(
            self.store,
            conversation_id,
            inventory.event_ids(),
            MAX_SYNC_EVENTS_PER_BATCH,
            self.membership,
        )?;
        let requested_event_ids = plan.requested_from_remote;
        let sent_events = plan.events_for_remote.len();
        let more_available = plan.more_available;
        let diff = SyncDiff::sign(
            self.identity,
            conversation_id,
            self.session_binding,
            requested_event_ids.clone(),
            plan.events_for_remote,
            more_available,
        )?;
        Ok(ServerInventoryOutcome::Accepted(Box::new(
            ServerInventoryRound {
                conversation_id,
                requested_event_ids,
                sent_events,
                more_available,
                diff,
            },
        )))
    }

    pub fn complete_round(
        &self,
        round: ServerInventoryRound,
        batch: SyncEventBatch,
    ) -> Result<ServerCompletion, SessionError> {
        if batch.conversation_id() != round.conversation_id {
            return Err(SessionError::ConversationMismatch);
        }
        let supplied_event_ids = event_ids(batch.events())?;
        if !same_event_ids(&supplied_event_ids, &round.requested_event_ids) {
            return Err(SessionError::BatchEventIdsMismatch);
        }
        let events = batch.into_events();
        for event in &events {
            event.verify_for_membership(self.membership)?;
        }
        self.store.put_events(&events, self.membership)?;

        let response = SyncComplete::new(
            round.conversation_id,
            supplied_event_ids.clone(),
            round.more_available,
        )?;
        Ok(ServerCompletion {
            response,
            stats: SyncRoundStats {
                sent_events: round.sent_events,
                received_events: supplied_event_ids.len(),
                more_available: round.more_available,
            },
        })
    }
}

pub enum ServerInventoryOutcome {
    Accepted(Box<ServerInventoryRound>),
    Rejected(SyncRejected),
}

pub struct ServerInventoryRound {
    conversation_id: ConversationId,
    requested_event_ids: Vec<EventId>,
    sent_events: usize,
    more_available: bool,
    diff: SyncDiff,
}

impl ServerInventoryRound {
    pub fn diff(&self) -> &SyncDiff {
        &self.diff
    }
}

pub struct ServerCompletion {
    response: SyncComplete,
    stats: SyncRoundStats,
}

impl ServerCompletion {
    pub fn response(&self) -> &SyncComplete {
        &self.response
    }

    pub fn stats(&self) -> SyncRoundStats {
        self.stats
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncRoundStats {
    pub sent_events: usize,
    pub received_events: usize,
    pub more_available: bool,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("sync protocol validation failed")]
    Protocol(#[from] ProtocolError),

    #[error("account device authorization failed")]
    Authorization(#[from] IdentityError),

    #[error("sync event store operation failed")]
    Store(#[from] StoreError),

    #[error("sync message belongs to a different conversation")]
    ConversationMismatch,

    #[error("responder requested event {0} outside the signed inventory")]
    UnadvertisedEventRequested(EventId),

    #[error("sync event batch does not exactly satisfy the requested event IDs")]
    BatchEventIdsMismatch,

    #[error("sync completion does not confirm the exact sent event IDs")]
    CompletionEventIdsMismatch,

    #[error("sync completion disagrees about continuation state")]
    ContinuationMismatch,
}

fn event_ids(events: &[AuthorizedEvent]) -> Result<Vec<EventId>, ProtocolError> {
    events
        .iter()
        .map(|event| event.event().event_id())
        .collect()
}

fn same_event_ids(left: &[EventId], right: &[EventId]) -> bool {
    left.len() == right.len()
        && left.iter().copied().collect::<HashSet<_>>()
            == right.iter().copied().collect::<HashSet<_>>()
}

struct SyncPlan {
    requested_from_remote: Vec<EventId>,
    events_for_remote: Vec<AuthorizedEvent>,
    more_available: bool,
}

fn plan_sync<S: SessionStore + ?Sized>(
    store: &S,
    conversation_id: ConversationId,
    remote_inventory: &[EventId],
    maximum_events_per_direction: usize,
    membership: &ConversationMembershipSnapshot,
) -> Result<SyncPlan, StoreError> {
    let local_inventory = store.inventory(conversation_id, membership)?;
    let local_ids: HashSet<_> = local_inventory.iter().copied().collect();
    let remote_ids: HashSet<_> = remote_inventory.iter().copied().collect();

    let mut requested_from_remote: Vec<_> = remote_ids.difference(&local_ids).copied().collect();
    requested_from_remote.sort_by_cached_key(ToString::to_string);
    let mut events_for_remote_ids: Vec<_> = local_ids.difference(&remote_ids).copied().collect();
    events_for_remote_ids.sort_by_cached_key(ToString::to_string);

    let more_available = requested_from_remote.len() > maximum_events_per_direction
        || events_for_remote_ids.len() > maximum_events_per_direction;
    requested_from_remote.truncate(maximum_events_per_direction);
    events_for_remote_ids.truncate(maximum_events_per_direction);
    let events_for_remote =
        store.events_by_id(conversation_id, &events_for_remote_ids, membership)?;

    Ok(SyncPlan {
        requested_from_remote,
        events_for_remote,
        more_available,
    })
}

#[cfg(test)]
mod tests {
    use kilogram_protocol::RatchetRecipient;
    use std::{cell::RefCell, error::Error};

    use kilogram_identity::{
        AccountAuthoritySnapshot, AccountRootState, DeviceCertificate, DeviceEncryptionIdentity,
    };
    use kilogram_protocol::SignedEvent;
    use kilogram_ratchet::RatchetState;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn account_authorization_accepts_new_device_and_rejects_revocation()
    -> Result<(), Box<dyn Error>> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let requester = DeviceIdentity::generate()?;
        let requester_encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            requester.device_id(),
            requester_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let first_snapshot = root.authority_snapshot()?;
        let session = SyncSessionBinding::from_transport_label("authorized-listener");
        let authorization = SignedDeviceSessionAuthorization::sign(
            &requester,
            certificate,
            first_snapshot,
            session,
        )?;

        let authorized = authorize_device_session(
            root.account_id(),
            &authorization,
            &DeviceCapability::MESSAGING,
            session,
        )?;
        assert_eq!(authorized.device_id(), requester.device_id());

        let revocation = root.revoke_device(requester.device_id())?;
        let revoked_snapshot = root.authority_snapshot()?;
        let revoked_authorization = SignedDeviceSessionAuthorization::sign(
            &requester,
            authorization.certificate().clone(),
            revoked_snapshot,
            session,
        )?;
        assert_eq!(revocation.device_id(), requester.device_id());
        assert!(matches!(
            authorize_device_session(
                root.account_id(),
                &revoked_authorization,
                &DeviceCapability::MESSAGING,
                session,
            ),
            Err(SessionError::Authorization(IdentityError::DeviceRevoked(device_id)))
                if device_id == requester.device_id()
        ));
        assert!(matches!(
            authorize_device_session(
                root.account_id(),
                &authorization,
                &DeviceCapability::MESSAGING,
                SyncSessionBinding::from_transport_label("other-listener"),
            ),
            Err(SessionError::Protocol(
                ProtocolError::DeviceAuthorizationSessionMismatch
            ))
        ));
        Ok(())
    }

    #[test]
    fn sync_client_rejects_an_event_from_a_non_member_account() -> Result<(), Box<dyn Error>> {
        let store = MemoryStore::default();
        let client_identity = DeviceIdentity::generate()?;
        let responder_identity = DeviceIdentity::generate()?;
        let outsider_identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("non-member-sync-event");
        let (client_account, _, _) = credential_for(&client_identity)?;
        let (responder_account, _, _) = credential_for(&responder_identity)?;
        let (outsider_account, outsider_certificate, outsider_snapshot) =
            credential_for(&outsider_identity)?;
        let membership = membership_for(conversation_id, &[client_account, responder_account])?;
        let session_binding = SyncSessionBinding::from_transport_label("membership-listener");
        let client = SyncClient::new(
            &client_identity,
            &store,
            conversation_id,
            &membership,
            session_binding,
            responder_identity.device_id(),
        );
        let round = client.begin_round()?;
        let outsider_event = authorized_text(
            &outsider_identity,
            &outsider_certificate,
            &outsider_snapshot,
            conversation_id,
            0,
            "not allowed",
        )?;
        let diff = SyncDiff::sign(
            &responder_identity,
            conversation_id,
            session_binding,
            Vec::new(),
            vec![outsider_event],
            false,
        )?;

        assert!(matches!(
            client.accept_diff(round, diff),
            Err(SessionError::Protocol(ProtocolError::Identity(
                IdentityError::AccountNotConversationMember(account_id)
            ))) if account_id == outsider_account
        ));
        Ok(())
    }

    #[test]
    fn more_than_one_batch_converges_bidirectionally() -> Result<(), Box<dyn Error>> {
        let client_store = MemoryStore::default();
        let server_store = MemoryStore::default();
        let client_identity = DeviceIdentity::generate()?;
        let server_identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("multi-round");
        let (client_account, client_certificate, client_snapshot) =
            credential_for(&client_identity)?;
        let (server_account, server_certificate, server_snapshot) =
            credential_for(&server_identity)?;
        let membership = membership_for(conversation_id, &[client_account, server_account])?;
        let session_binding = SyncSessionBinding::from_transport_label("test-listener");

        let shared = authorized_text(
            &client_identity,
            &client_certificate,
            &client_snapshot,
            conversation_id,
            0,
            "shared",
        )?;
        client_store.put_events(std::slice::from_ref(&shared), &membership)?;
        server_store.put_events(std::slice::from_ref(&shared), &membership)?;

        for sequence in 1..=70 {
            client_store.put_events(
                &[authorized_text(
                    &client_identity,
                    &client_certificate,
                    &client_snapshot,
                    conversation_id,
                    sequence,
                    &format!("client-{sequence}"),
                )?],
                &membership,
            )?;
        }
        for sequence in 0..70 {
            server_store.put_events(
                &[authorized_text(
                    &server_identity,
                    &server_certificate,
                    &server_snapshot,
                    conversation_id,
                    sequence,
                    &format!("server-{sequence}"),
                )?],
                &membership,
            )?;
        }

        let client = SyncClient::new(
            &client_identity,
            &client_store,
            conversation_id,
            &membership,
            session_binding,
            server_identity.device_id(),
        );
        let server = SyncServer::new(
            &server_identity,
            &server_store,
            session_binding,
            client_identity.device_id(),
            &membership,
        );
        let mut rounds = 0;
        let mut client_sent = 0;
        let mut client_received = 0;

        loop {
            let client_inventory = client.begin_round()?;
            let server_round = match server.accept_inventory(client_inventory.inventory())? {
                ServerInventoryOutcome::Accepted(round) => round,
                ServerInventoryOutcome::Rejected(rejected) => {
                    return Err(format!("unexpected rejection: {:?}", rejected.reason()).into());
                }
            };
            let client_batch = client.accept_diff(client_inventory, server_round.diff().clone())?;
            let server_completion =
                server.complete_round(*server_round, client_batch.batch().clone())?;
            let stats =
                client.accept_complete(client_batch, server_completion.response().clone())?;
            rounds += 1;
            client_sent += stats.sent_events;
            client_received += stats.received_events;
            if !stats.more_available {
                break;
            }
        }

        assert_eq!(rounds, 2);
        assert_eq!(client_sent, 70);
        assert_eq!(client_received, 70);
        assert_eq!(
            client_store.inventory(conversation_id, &membership)?,
            server_store.inventory(conversation_id, &membership)?
        );
        Ok(())
    }

    #[test]
    fn completed_round_resumes_with_a_new_transport_session() -> Result<(), Box<dyn Error>> {
        let client_store = MemoryStore::default();
        let server_store = MemoryStore::default();
        let client_identity = DeviceIdentity::generate()?;
        let server_identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("resume-after-reconnect");
        let (client_account, client_certificate, client_snapshot) =
            credential_for(&client_identity)?;
        let (server_account, server_certificate, server_snapshot) =
            credential_for(&server_identity)?;
        let membership = membership_for(conversation_id, &[client_account, server_account])?;

        let shared = authorized_text(
            &client_identity,
            &client_certificate,
            &client_snapshot,
            conversation_id,
            0,
            "shared",
        )?;
        client_store.put_events(std::slice::from_ref(&shared), &membership)?;
        server_store.put_events(std::slice::from_ref(&shared), &membership)?;
        for sequence in 1..=70 {
            client_store.put_events(
                &[authorized_text(
                    &client_identity,
                    &client_certificate,
                    &client_snapshot,
                    conversation_id,
                    sequence,
                    &format!("client-{sequence}"),
                )?],
                &membership,
            )?;
        }
        for sequence in 0..70 {
            server_store.put_events(
                &[authorized_text(
                    &server_identity,
                    &server_certificate,
                    &server_snapshot,
                    conversation_id,
                    sequence,
                    &format!("server-{sequence}"),
                )?],
                &membership,
            )?;
        }

        let first_binding = SyncSessionBinding::from_transport_label("listener-before-restart");
        let first_client = SyncClient::new(
            &client_identity,
            &client_store,
            conversation_id,
            &membership,
            first_binding,
            server_identity.device_id(),
        );
        let first_server = SyncServer::new(
            &server_identity,
            &server_store,
            first_binding,
            client_identity.device_id(),
            &membership,
        );
        let first_inventory = first_client.begin_round()?;
        let first_server_round = match first_server.accept_inventory(first_inventory.inventory())? {
            ServerInventoryOutcome::Accepted(round) => round,
            ServerInventoryOutcome::Rejected(rejected) => {
                return Err(format!("unexpected rejection: {:?}", rejected.reason()).into());
            }
        };
        let first_client_batch =
            first_client.accept_diff(first_inventory, first_server_round.diff().clone())?;
        let first_completion =
            first_server.complete_round(*first_server_round, first_client_batch.batch().clone())?;
        let first_stats = first_client
            .accept_complete(first_client_batch, first_completion.response().clone())?;
        assert_eq!(first_stats.sent_events, MAX_SYNC_EVENTS_PER_BATCH);
        assert_eq!(first_stats.received_events, MAX_SYNC_EVENTS_PER_BATCH);
        assert!(first_stats.more_available);

        let resumed_binding = SyncSessionBinding::from_transport_label("listener-after-restart");
        let resumed_client = SyncClient::new(
            &client_identity,
            &client_store,
            conversation_id,
            &membership,
            resumed_binding,
            server_identity.device_id(),
        );
        let resumed_server = SyncServer::new(
            &server_identity,
            &server_store,
            resumed_binding,
            client_identity.device_id(),
            &membership,
        );
        let resumed_inventory = resumed_client.begin_round()?;
        let resumed_server_round =
            match resumed_server.accept_inventory(resumed_inventory.inventory())? {
                ServerInventoryOutcome::Accepted(round) => round,
                ServerInventoryOutcome::Rejected(rejected) => {
                    return Err(format!("unexpected rejection: {:?}", rejected.reason()).into());
                }
            };
        let resumed_client_batch =
            resumed_client.accept_diff(resumed_inventory, resumed_server_round.diff().clone())?;
        let resumed_completion = resumed_server
            .complete_round(*resumed_server_round, resumed_client_batch.batch().clone())?;
        let resumed_stats = resumed_client
            .accept_complete(resumed_client_batch, resumed_completion.response().clone())?;

        assert_eq!(resumed_stats.sent_events, 6);
        assert_eq!(resumed_stats.received_events, 6);
        assert!(!resumed_stats.more_available);
        assert_eq!(
            client_store.inventory(conversation_id, &membership)?,
            server_store.inventory(conversation_id, &membership)?
        );
        Ok(())
    }

    #[test]
    fn authorized_new_requester_is_accepted_without_prior_events() -> Result<(), Box<dyn Error>> {
        let store = MemoryStore::default();
        let requester = DeviceIdentity::generate()?;
        let responder = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("authorization");
        let (requester_account, _, _) = credential_for(&requester)?;
        let (responder_account, _, _) = credential_for(&responder)?;
        let membership = membership_for(conversation_id, &[requester_account, responder_account])?;
        let session_binding = SyncSessionBinding::from_transport_label("test-listener");
        let inventory =
            SignedSyncInventory::sign(&requester, conversation_id, session_binding, Vec::new())?;
        let server = SyncServer::new(
            &responder,
            &store,
            session_binding,
            requester.device_id(),
            &membership,
        );

        assert!(matches!(
            server.accept_inventory(&inventory)?,
            ServerInventoryOutcome::Accepted(_)
        ));

        let other = DeviceIdentity::generate()?;
        let other_inventory =
            SignedSyncInventory::sign(&other, conversation_id, session_binding, Vec::new())?;
        let ServerInventoryOutcome::Rejected(rejection) =
            server.accept_inventory(&other_inventory)?
        else {
            return Err("an unauthenticated requester was accepted".into());
        };
        assert_eq!(rejection.reason(), SyncRejectionReason::RequesterNotAllowed);
        Ok(())
    }

    #[derive(Default)]
    struct MemoryStore {
        events: RefCell<Vec<AuthorizedEvent>>,
    }

    impl SessionStore for MemoryStore {
        fn inventory(
            &self,
            conversation_id: ConversationId,
            _membership: &ConversationMembershipSnapshot,
        ) -> Result<Vec<EventId>, StoreError> {
            let mut event_ids: Vec<_> = self
                .events
                .borrow()
                .iter()
                .filter(|event| event.event().conversation_id() == conversation_id)
                .map(|event| event.event().event_id())
                .collect::<Result<_, _>>()?;
            event_ids.sort_by_cached_key(ToString::to_string);
            Ok(event_ids)
        }

        fn events_by_id(
            &self,
            conversation_id: ConversationId,
            event_ids: &[EventId],
            _membership: &ConversationMembershipSnapshot,
        ) -> Result<Vec<AuthorizedEvent>, StoreError> {
            let events = self.events.borrow();
            event_ids
                .iter()
                .map(|requested_id| {
                    events
                        .iter()
                        .find(|event| {
                            event.event().conversation_id() == conversation_id
                                && event.event().event_id().ok() == Some(*requested_id)
                        })
                        .cloned()
                        .ok_or(StoreError::RequestedEventMissing {
                            conversation_id,
                            event_id: *requested_id,
                        })
                })
                .collect()
        }

        fn put_events(
            &self,
            new_events: &[AuthorizedEvent],
            membership: &ConversationMembershipSnapshot,
        ) -> Result<(), StoreError> {
            let mut events = self.events.borrow_mut();
            for event in new_events {
                event.verify_for_membership(membership)?;
                let event_id = event.event().event_id()?;
                if events
                    .iter()
                    .any(|existing| existing.event().event_id().ok() == Some(event_id))
                {
                    continue;
                }
                if let Some(existing) = events.iter().find(|existing| {
                    existing.event().conversation_id() == event.event().conversation_id()
                        && existing.event().author_device_id() == event.event().author_device_id()
                        && existing.event().author_sequence() == event.event().author_sequence()
                }) {
                    return Err(StoreError::WriterSequenceConflict {
                        author_device_id: event.event().author_device_id(),
                        author_sequence: event.event().author_sequence(),
                        existing_event_id: existing.event().event_id()?,
                        rejected_event_id: event_id,
                    });
                }
                events.push(event.clone());
            }
            Ok(())
        }
    }

    fn credential_for(
        identity: &DeviceIdentity,
    ) -> Result<(AccountId, DeviceCertificate, AccountAuthoritySnapshot), Box<dyn Error>> {
        let directory = tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        Ok((root.account_id(), certificate, root.authority_snapshot()?))
    }

    fn membership_for(
        conversation_id: ConversationId,
        members: &[AccountId],
    ) -> Result<ConversationMembershipSnapshot, Box<dyn Error>> {
        let directory = tempdir()?;
        let owner = AccountRootState::create(directory.path())?;
        Ok(owner.create_conversation_membership(conversation_id.scope_id(), members)?)
    }

    fn authorized_text(
        identity: &DeviceIdentity,
        certificate: &DeviceCertificate,
        authority_snapshot: &AccountAuthoritySnapshot,
        conversation_id: ConversationId,
        sequence: u64,
        body: &str,
    ) -> Result<AuthorizedEvent, Box<dyn Error>> {
        let sender_directory = tempdir()?;
        let peer_directory = tempdir()?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_root = AccountRootState::create(peer_directory.path().join("account"))?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_device_list =
            peer_root.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        let peer_bundle =
            RatchetState::load_or_create(peer_directory.path())?.prekey_bundle(&peer_identity)?;
        let (sender_ratchet_identity, ciphertext, _) = RatchetState::load_or_create(
            sender_directory.path(),
        )?
        .encrypt(identity, &peer_bundle, body)?;
        Ok(AuthorizedEvent::new(
            SignedEvent::sign_ratchet_text(
                identity,
                conversation_id,
                sequence,
                Vec::new(),
                peer_device_list,
                sender_ratchet_identity,
                vec![RatchetRecipient::new(
                    peer_identity.device_id(),
                    ciphertext,
                )?],
            )?,
            certificate.clone(),
            authority_snapshot.clone(),
        )?)
    }
}
