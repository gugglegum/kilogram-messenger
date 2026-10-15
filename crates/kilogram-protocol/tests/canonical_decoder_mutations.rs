use kilogram_protocol::canonical_account_v1::{
    decode_and_verify_signed_account_device_list_v1,
    decode_and_verify_signed_conversation_membership_v1,
};
use kilogram_protocol::canonical_authority_v1::{
    decode_and_verify_signed_device_certificate_v1,
    decode_and_verify_signed_device_session_authorization_v1,
};
use kilogram_protocol::canonical_device_link_v1::decode_and_verify_signed_device_link_authorization_v1;
use kilogram_protocol::canonical_event_v1::{
    decode_and_verify_authorized_event_v1, decode_and_verify_signed_event_v1,
};
use kilogram_protocol::canonical_mailbox_gossip_v1::decode_and_verify_mailbox_provider_gossip_frame_for_id_v1;
use kilogram_protocol::canonical_mailbox_provisioning_v1::{
    decode_and_verify_signed_local_mailbox_binding_v1,
    decode_and_verify_signed_mailbox_capability_acknowledgement_v1,
    decode_and_verify_signed_mailbox_capability_update_v1,
    decode_and_verify_signed_mailbox_peer_offer_v1,
};
use kilogram_protocol::canonical_mailbox_transport_v1::decode_and_verify_mailbox_peer_request_v1;
use kilogram_protocol::canonical_mailbox_v1::{
    decode_and_verify_signed_mailbox_delete_receipt_v1,
    decode_and_verify_signed_mailbox_read_authorization_v1,
    decode_and_verify_signed_mailbox_storage_offer_v1,
    decode_and_verify_signed_mailbox_stored_receipt_v1,
    decode_and_verify_signed_mailbox_write_authorization_v1,
};
use kilogram_protocol::canonical_prekey_selection_v1::decode_and_verify_prekey_selection_transcript_v1;
use kilogram_protocol::canonical_publication_conflict_v1::{
    decode_and_verify_signed_publication_conflict_proof_v1,
    decode_and_verify_signed_ticket_publication_observation_v1,
    decode_and_verify_signed_ticket_publication_v1,
};
use kilogram_protocol::canonical_ratchet_v1::{
    decode_and_verify_account_prekey_directory_v1, decode_and_verify_signed_prekey_pool_v1,
};
use kilogram_protocol::canonical_recovery_exchange_v1::{
    decode_and_verify_signed_history_recovery_link_v1,
    decode_and_verify_signed_history_recovery_plan_v1,
    decode_and_verify_signed_history_recovery_response_v1,
};
use kilogram_protocol::canonical_recovery_v1::decode_and_verify_signed_history_recovery_checkpoint_v1;
use kilogram_protocol::canonical_runtime_mailbox_v1::{
    CanonicalSignedRuntimeMailboxDispatchV1, decode_and_verify_runtime_mailbox_payload_v1,
    decode_and_verify_signed_runtime_local_mailbox_binding_v1,
    decode_and_verify_signed_runtime_mailbox_dispatch_v1,
    decode_and_verify_signed_runtime_peer_mailbox_binding_v1,
    verify_runtime_mailbox_payload_for_dispatch_v1,
};
use kilogram_protocol::canonical_sync_session_v1::decode_and_verify_signed_sync_session_checkpoint_v1;
use kilogram_protocol::canonical_sync_v1::{
    decode_and_verify_signed_sync_diff_v1, decode_and_verify_sync_client_frame_v1,
    decode_and_verify_sync_server_frame_v1,
};
use kilogram_protocol::canonical_ticket_publication_v1::{
    decode_and_verify_signed_ticket_publication_write_authorization_v1,
    decode_and_verify_ticket_publication_capability_v1,
};
use serde_json::Value;

const MINIMUM_MUTATIONS: usize = 500;
const EXPECTED_MUTATIONS: usize = 10_583;
const MAXIMUM_MUTATIONS: usize = 12_000;
const MAXIMUM_GRID_POSITIONS: usize = 128;

#[derive(Clone)]
enum Decoder {
    Certificate,
    Session,
    Pool,
    Directory,
    Recovery,
    RecoveryLink,
    RecoveryPlan,
    RecoveryResponse,
    DeviceList,
    Membership,
    DeviceLink,
    SignedEvent,
    AuthorizedEvent,
    PrekeySelection,
    SignedSyncDiff,
    SyncClientFrame,
    SyncServerFrame,
    SyncSessionCheckpoint,
    MailboxWriteAuthorization,
    MailboxReadListAuthorization,
    MailboxReadDeleteAuthorization,
    MailboxStorageOffer,
    MailboxStoredReceipt,
    MailboxDeleteReceipt,
    MailboxPeerRequest,
    LocalMailboxBinding,
    MailboxPeerOffer,
    MailboxCapabilityUpdate,
    MailboxCapabilityAcknowledgement,
    RuntimeLocalMailboxBinding,
    RuntimePeerMailboxBinding,
    RuntimeMailboxPayload {
        dispatch: Box<CanonicalSignedRuntimeMailboxDispatchV1>,
    },
    RuntimeMailboxDispatch,
    TicketPublicationCapability,
    TicketPublicationWriteAuthorization,
    TicketPublication,
    TicketPublicationObservation,
    PublicationConflictProof,
    MailboxGossipFrame {
        expected_id: [u8; 32],
        now_unix_seconds: u64,
    },
}

fn accepts(decoder: &Decoder, bytes: &[u8]) -> bool {
    match decoder {
        Decoder::Certificate => decode_and_verify_signed_device_certificate_v1(bytes).is_ok(),
        Decoder::Session => decode_and_verify_signed_device_session_authorization_v1(bytes).is_ok(),
        Decoder::Pool => decode_and_verify_signed_prekey_pool_v1(bytes).is_ok(),
        Decoder::Directory => decode_and_verify_account_prekey_directory_v1(bytes).is_ok(),
        Decoder::Recovery => decode_and_verify_signed_history_recovery_checkpoint_v1(bytes).is_ok(),
        Decoder::RecoveryLink => decode_and_verify_signed_history_recovery_link_v1(bytes).is_ok(),
        Decoder::RecoveryPlan => decode_and_verify_signed_history_recovery_plan_v1(bytes).is_ok(),
        Decoder::RecoveryResponse => {
            decode_and_verify_signed_history_recovery_response_v1(bytes).is_ok()
        }
        Decoder::DeviceList => decode_and_verify_signed_account_device_list_v1(bytes).is_ok(),
        Decoder::Membership => decode_and_verify_signed_conversation_membership_v1(bytes).is_ok(),
        Decoder::DeviceLink => decode_and_verify_signed_device_link_authorization_v1(bytes).is_ok(),
        Decoder::SignedEvent => decode_and_verify_signed_event_v1(bytes).is_ok(),
        Decoder::AuthorizedEvent => decode_and_verify_authorized_event_v1(bytes).is_ok(),
        Decoder::PrekeySelection => decode_and_verify_prekey_selection_transcript_v1(bytes).is_ok(),
        Decoder::SignedSyncDiff => decode_and_verify_signed_sync_diff_v1(bytes).is_ok(),
        Decoder::SyncClientFrame => decode_and_verify_sync_client_frame_v1(bytes).is_ok(),
        Decoder::SyncServerFrame => decode_and_verify_sync_server_frame_v1(bytes).is_ok(),
        Decoder::SyncSessionCheckpoint => {
            decode_and_verify_signed_sync_session_checkpoint_v1(bytes).is_ok()
        }
        Decoder::MailboxWriteAuthorization => {
            decode_and_verify_signed_mailbox_write_authorization_v1(bytes).is_ok()
        }
        Decoder::MailboxReadListAuthorization | Decoder::MailboxReadDeleteAuthorization => {
            decode_and_verify_signed_mailbox_read_authorization_v1(bytes).is_ok()
        }
        Decoder::MailboxStorageOffer => {
            decode_and_verify_signed_mailbox_storage_offer_v1(bytes).is_ok()
        }
        Decoder::MailboxStoredReceipt => {
            decode_and_verify_signed_mailbox_stored_receipt_v1(bytes).is_ok()
        }
        Decoder::MailboxDeleteReceipt => {
            decode_and_verify_signed_mailbox_delete_receipt_v1(bytes).is_ok()
        }
        Decoder::MailboxPeerRequest => decode_and_verify_mailbox_peer_request_v1(bytes).is_ok(),
        Decoder::LocalMailboxBinding => {
            decode_and_verify_signed_local_mailbox_binding_v1(bytes).is_ok()
        }
        Decoder::MailboxPeerOffer => decode_and_verify_signed_mailbox_peer_offer_v1(bytes).is_ok(),
        Decoder::MailboxCapabilityUpdate => {
            decode_and_verify_signed_mailbox_capability_update_v1(bytes).is_ok()
        }
        Decoder::MailboxCapabilityAcknowledgement => {
            decode_and_verify_signed_mailbox_capability_acknowledgement_v1(bytes).is_ok()
        }
        Decoder::RuntimeLocalMailboxBinding => {
            decode_and_verify_signed_runtime_local_mailbox_binding_v1(bytes).is_ok()
        }
        Decoder::RuntimePeerMailboxBinding => {
            decode_and_verify_signed_runtime_peer_mailbox_binding_v1(bytes).is_ok()
        }
        Decoder::RuntimeMailboxPayload { dispatch } => {
            decode_and_verify_runtime_mailbox_payload_v1(bytes)
                .and_then(|payload| {
                    verify_runtime_mailbox_payload_for_dispatch_v1(&payload, dispatch)
                })
                .is_ok()
        }
        Decoder::RuntimeMailboxDispatch => {
            decode_and_verify_signed_runtime_mailbox_dispatch_v1(bytes).is_ok()
        }
        Decoder::TicketPublicationCapability => {
            decode_and_verify_ticket_publication_capability_v1(bytes).is_ok()
        }
        Decoder::TicketPublicationWriteAuthorization => {
            decode_and_verify_signed_ticket_publication_write_authorization_v1(bytes).is_ok()
        }
        Decoder::TicketPublication => decode_and_verify_signed_ticket_publication_v1(bytes).is_ok(),
        Decoder::TicketPublicationObservation => {
            decode_and_verify_signed_ticket_publication_observation_v1(bytes).is_ok()
        }
        Decoder::PublicationConflictProof => {
            decode_and_verify_signed_publication_conflict_proof_v1(bytes).is_ok()
        }
        Decoder::MailboxGossipFrame {
            expected_id,
            now_unix_seconds,
        } => decode_and_verify_mailbox_provider_gossip_frame_for_id_v1(
            bytes,
            *now_unix_seconds,
            *expected_id,
        )
        .is_ok(),
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !value.len().is_multiple_of(2) {
        return Err("hex value has odd length".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
        .collect()
}

fn pointer_hex(document: &Value, pointer: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    decode_hex(
        document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or("vector JSON pointer is missing")?,
    )
}

fn bounded_positions(length: usize) -> Vec<usize> {
    if length <= MAXIMUM_GRID_POSITIONS {
        return (0..length).collect();
    }
    (0..MAXIMUM_GRID_POSITIONS)
        .map(|index| index * length / MAXIMUM_GRID_POSITIONS)
        .collect()
}

#[test]
fn bounded_golden_decoder_mutation_corpus_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let authority: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-AUTHORITY-VECTORS.json"
    )))?;
    let ratchet: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RATCHET-VECTORS.json"
    )))?;
    let recovery: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RECOVERY-VECTORS.json"
    )))?;
    let recovery_exchange: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RECOVERY-EXCHANGE-VECTORS.json"
    )))?;
    let account: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-ACCOUNT-VECTORS.json"
    )))?;
    let device_link: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-DEVICE-LINK-VECTORS.json"
    )))?;
    let event: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-EVENT-VECTORS.json"
    )))?;
    let prekey_selection: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-PREKEY-SELECTION-VECTORS.json"
    )))?;
    let sync: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-VECTORS.json"
    )))?;
    let sync_session: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-SESSION-VECTORS.json"
    )))?;
    let mailbox: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-MAILBOX-VECTORS.json"
    )))?;
    let mailbox_transport: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-MAILBOX-TRANSPORT-VECTORS.json"
    )))?;
    let mailbox_gossip: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-MAILBOX-GOSSIP-VECTORS.json"
    )))?;
    let mailbox_provisioning: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"
    )))?;
    let runtime_mailbox: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-MAILBOX-VECTORS.json"
    )))?;
    let ticket_publication: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-TICKET-PUBLICATION-VECTORS.json"
    )))?;
    let publication_conflict: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
    )))?;
    let gossip_frame_id: [u8; 32] = pointer_hex(&mailbox_gossip, "/expected/request/frame_id_hex")?
        .try_into()
        .map_err(|_| "gossip frame ID length changed")?;
    let runtime_dispatch_bytes = pointer_hex(
        &runtime_mailbox,
        "/expected/signed_dispatch/signed_cbor_hex",
    )?;
    let runtime_dispatch =
        decode_and_verify_signed_runtime_mailbox_dispatch_v1(&runtime_dispatch_bytes)?;
    let fixtures = [
        (
            "certificate",
            Decoder::Certificate,
            pointer_hex(&authority, "/expected/certificate/signed_cbor_hex")?,
        ),
        (
            "session",
            Decoder::Session,
            pointer_hex(
                &authority,
                "/expected/session_authorization/signed_cbor_hex",
            )?,
        ),
        (
            "pool",
            Decoder::Pool,
            pointer_hex(&ratchet, "/expected/prekey_pool/signed_cbor_hex")?,
        ),
        (
            "directory",
            Decoder::Directory,
            pointer_hex(&ratchet, "/expected/account_prekey_directory_cbor_hex")?,
        ),
        (
            "recovery",
            Decoder::Recovery,
            pointer_hex(&recovery, "/expected/successor_checkpoint/signed_cbor_hex")?,
        ),
        (
            "recovery-link",
            Decoder::RecoveryLink,
            pointer_hex(&recovery_exchange, "/expected/link/signed_cbor_hex")?,
        ),
        (
            "recovery-plan",
            Decoder::RecoveryPlan,
            pointer_hex(&recovery_exchange, "/expected/plan/signed_cbor_hex")?,
        ),
        (
            "recovery-response",
            Decoder::RecoveryResponse,
            pointer_hex(
                &recovery_exchange,
                "/expected/page_response/signed_cbor_hex",
            )?,
        ),
        (
            "device-list",
            Decoder::DeviceList,
            pointer_hex(&account, "/expected/account_device_list/signed_cbor_hex")?,
        ),
        (
            "membership",
            Decoder::Membership,
            pointer_hex(
                &account,
                "/expected/conversation_membership/signed_cbor_hex",
            )?,
        ),
        (
            "device-link",
            Decoder::DeviceLink,
            pointer_hex(&device_link, "/expected/authorization/signed_cbor_hex")?,
        ),
        (
            "signed-acknowledgement-event",
            Decoder::SignedEvent,
            pointer_hex(&event, "/expected/acknowledgement_event/signed_cbor_hex")?,
        ),
        (
            "authorized-text-event",
            Decoder::AuthorizedEvent,
            pointer_hex(&event, "/expected/authorized_text_event_cbor_hex")?,
        ),
        (
            "prekey-selection-transcript",
            Decoder::PrekeySelection,
            pointer_hex(&prekey_selection, "/expected/transcript_cbor_hex")?,
        ),
        (
            "signed-sync-diff",
            Decoder::SignedSyncDiff,
            pointer_hex(&sync, "/expected/diff/signed_cbor_hex")?,
        ),
        (
            "sync-client-frame",
            Decoder::SyncClientFrame,
            pointer_hex(&sync, "/expected/client_frames/inventory_cbor_hex")?,
        ),
        (
            "sync-server-frame",
            Decoder::SyncServerFrame,
            pointer_hex(&sync, "/expected/server_frames/diff_cbor_hex")?,
        ),
        (
            "sync-session-checkpoint",
            Decoder::SyncSessionCheckpoint,
            pointer_hex(&sync_session, "/expected/checkpoints/3/signed_cbor_hex")?,
        ),
        (
            "mailbox-write-authorization",
            Decoder::MailboxWriteAuthorization,
            pointer_hex(&mailbox, "/expected/write_authorization/signed_cbor_hex")?,
        ),
        (
            "mailbox-read-list-authorization",
            Decoder::MailboxReadListAuthorization,
            pointer_hex(
                &mailbox,
                "/expected/read_list_authorization/signed_cbor_hex",
            )?,
        ),
        (
            "mailbox-read-delete-authorization",
            Decoder::MailboxReadDeleteAuthorization,
            pointer_hex(
                &mailbox,
                "/expected/read_delete_authorization/signed_cbor_hex",
            )?,
        ),
        (
            "mailbox-storage-offer",
            Decoder::MailboxStorageOffer,
            pointer_hex(&mailbox, "/expected/storage_offer/signed_cbor_hex")?,
        ),
        (
            "mailbox-stored-receipt",
            Decoder::MailboxStoredReceipt,
            pointer_hex(&mailbox, "/expected/stored_receipt/signed_cbor_hex")?,
        ),
        (
            "mailbox-delete-receipt",
            Decoder::MailboxDeleteReceipt,
            pointer_hex(&mailbox, "/expected/delete_receipt/signed_cbor_hex")?,
        ),
        (
            "mailbox-peer-put-request",
            Decoder::MailboxPeerRequest,
            pointer_hex(&mailbox_transport, "/expected/peer_requests/put/cbor_hex")?,
        ),
        (
            "mailbox-peer-list-request",
            Decoder::MailboxPeerRequest,
            pointer_hex(&mailbox_transport, "/expected/peer_requests/list/cbor_hex")?,
        ),
        (
            "mailbox-peer-delete-request",
            Decoder::MailboxPeerRequest,
            pointer_hex(
                &mailbox_transport,
                "/expected/peer_requests/delete/cbor_hex",
            )?,
        ),
        (
            "mailbox-provider-gossip-request",
            Decoder::MailboxGossipFrame {
                expected_id: gossip_frame_id,
                now_unix_seconds: mailbox_gossip["input"]["request_created_at"]
                    .as_u64()
                    .ok_or("gossip request time is missing")?,
            },
            pointer_hex(&mailbox_gossip, "/expected/request/frame_cbor_hex")?,
        ),
        (
            "signed-local-mailbox-binding",
            Decoder::LocalMailboxBinding,
            pointer_hex(
                &mailbox_provisioning,
                "/expected/signed_local_binding/signed_cbor_hex",
            )?,
        ),
        (
            "signed-mailbox-peer-offer",
            Decoder::MailboxPeerOffer,
            pointer_hex(
                &mailbox_provisioning,
                "/expected/signed_exact_peer_offer/signed_cbor_hex",
            )?,
        ),
        (
            "signed-mailbox-capability-update",
            Decoder::MailboxCapabilityUpdate,
            pointer_hex(
                &mailbox_provisioning,
                "/expected/activation_update/signed_cbor_hex",
            )?,
        ),
        (
            "signed-mailbox-capability-acknowledgement",
            Decoder::MailboxCapabilityAcknowledgement,
            pointer_hex(
                &mailbox_provisioning,
                "/expected/activation_acknowledgement/signed_cbor_hex",
            )?,
        ),
        (
            "signed-runtime-local-mailbox-binding",
            Decoder::RuntimeLocalMailboxBinding,
            pointer_hex(
                &runtime_mailbox,
                "/expected/signed_local_binding/signed_cbor_hex",
            )?,
        ),
        (
            "signed-runtime-peer-mailbox-binding",
            Decoder::RuntimePeerMailboxBinding,
            pointer_hex(
                &runtime_mailbox,
                "/expected/signed_peer_binding/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-mailbox-payload",
            Decoder::RuntimeMailboxPayload {
                dispatch: Box::new(runtime_dispatch.clone()),
            },
            pointer_hex(&runtime_mailbox, "/expected/payload/cbor_hex")?,
        ),
        (
            "signed-runtime-mailbox-dispatch",
            Decoder::RuntimeMailboxDispatch,
            pointer_hex(
                &runtime_mailbox,
                "/expected/signed_dispatch/signed_cbor_hex",
            )?,
        ),
        (
            "ticket-publication-capability",
            Decoder::TicketPublicationCapability,
            pointer_hex(&ticket_publication, "/expected/rotated_capability/cbor_hex")?,
        ),
        (
            "signed-ticket-publication-write-authorization",
            Decoder::TicketPublicationWriteAuthorization,
            pointer_hex(
                &ticket_publication,
                "/expected/authorization/signed_cbor_hex",
            )?,
        ),
        (
            "signed-ticket-publication",
            Decoder::TicketPublication,
            pointer_hex(
                &publication_conflict,
                "/expected/first_publication/signed_cbor_hex",
            )?,
        ),
        (
            "signed-ticket-publication-observation",
            Decoder::TicketPublicationObservation,
            pointer_hex(
                &publication_conflict,
                "/expected/first_observation/signed_cbor_hex",
            )?,
        ),
        (
            "signed-publication-conflict-proof",
            Decoder::PublicationConflictProof,
            pointer_hex(
                &publication_conflict,
                "/expected/conflict_proof/signed_cbor_hex",
            )?,
        ),
    ];
    let mut mutations = 0usize;
    for (name, decoder, valid) in fixtures {
        assert!(
            accepts(&decoder, &valid),
            "golden {name} fixture was rejected"
        );
        assert!(
            valid.len() <= 512 * 1024,
            "golden {name} fixture is unbounded"
        );

        for length in bounded_positions(valid.len()) {
            assert!(
                !accepts(&decoder, &valid[..length]),
                "{name} accepted truncation at {length}"
            );
            mutations += 1;
        }
        for index in bounded_positions(valid.len()) {
            let mut changed = valid.clone();
            changed[index] ^= 1;
            assert!(
                !accepts(&decoder, &changed),
                "{name} accepted bit flip at {index}"
            );
            mutations += 1;
        }
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(
            !accepts(&decoder, &trailing),
            "{name} accepted trailing data"
        );
        mutations += 1;

        let mut non_minimal = Vec::with_capacity(valid.len() + 1);
        non_minimal.push(valid[0]);
        non_minimal.extend_from_slice(&[0x18, 0x01]);
        non_minimal.extend_from_slice(&valid[2..]);
        assert!(
            !accepts(&decoder, &non_minimal),
            "{name} accepted non-minimal version"
        );
        mutations += 1;

        let mut indefinite = valid.clone();
        indefinite[0] = 0x9f;
        indefinite.push(0xff);
        assert!(
            !accepts(&decoder, &indefinite),
            "{name} accepted indefinite array"
        );
        mutations += 1;

        let mut reserved = valid.clone();
        reserved[0] = 0x9c;
        assert!(
            !accepts(&decoder, &reserved),
            "{name} accepted reserved CBOR head"
        );
        mutations += 1;
    }

    let oversized = [
        0x83, 0x01, 0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    for decoder in [
        Decoder::Certificate,
        Decoder::Session,
        Decoder::Pool,
        Decoder::Directory,
        Decoder::Recovery,
        Decoder::RecoveryLink,
        Decoder::RecoveryPlan,
        Decoder::RecoveryResponse,
        Decoder::DeviceList,
        Decoder::Membership,
        Decoder::DeviceLink,
        Decoder::SignedEvent,
        Decoder::AuthorizedEvent,
        Decoder::PrekeySelection,
        Decoder::SignedSyncDiff,
        Decoder::SyncClientFrame,
        Decoder::SyncServerFrame,
        Decoder::SyncSessionCheckpoint,
        Decoder::MailboxWriteAuthorization,
        Decoder::MailboxReadListAuthorization,
        Decoder::MailboxReadDeleteAuthorization,
        Decoder::MailboxStorageOffer,
        Decoder::MailboxStoredReceipt,
        Decoder::MailboxDeleteReceipt,
        Decoder::MailboxPeerRequest,
        Decoder::LocalMailboxBinding,
        Decoder::MailboxPeerOffer,
        Decoder::MailboxCapabilityUpdate,
        Decoder::MailboxCapabilityAcknowledgement,
        Decoder::RuntimeLocalMailboxBinding,
        Decoder::RuntimePeerMailboxBinding,
        Decoder::RuntimeMailboxDispatch,
        Decoder::TicketPublicationCapability,
        Decoder::TicketPublicationWriteAuthorization,
        Decoder::TicketPublication,
        Decoder::TicketPublicationObservation,
        Decoder::PublicationConflictProof,
        Decoder::MailboxGossipFrame {
            expected_id: [0; 32],
            now_unix_seconds: 1_800_000_030,
        },
    ] {
        assert!(!accepts(&decoder, &oversized));
        mutations += 1;
    }
    let runtime_payload_decoder = Decoder::RuntimeMailboxPayload {
        dispatch: Box::new(runtime_dispatch),
    };
    assert!(!accepts(&runtime_payload_decoder, &oversized));
    mutations += 1;
    assert!(
        mutations >= MINIMUM_MUTATIONS,
        "mutation corpus became too small: {mutations}"
    );
    assert!(
        mutations <= MAXIMUM_MUTATIONS,
        "mutation corpus became unbounded: {mutations}"
    );
    assert_eq!(
        mutations, EXPECTED_MUTATIONS,
        "mutation corpus inventory changed"
    );
    eprintln!("canonical_decoder_mutations={mutations}");
    Ok(())
}
