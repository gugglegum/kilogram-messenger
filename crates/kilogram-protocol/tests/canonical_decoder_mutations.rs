use kilogram_crypto::DeviceEncryptionIdentity;
use kilogram_protocol::canonical_account_recovery_policy_v1::{
    CanonicalAccountRecoveryPolicyTransitionRequestV1,
    account_recovery_policy_transition_request_id_v1,
    decode_and_verify_account_recovery_policy_transition_certificate_v1,
    decode_and_verify_account_recovery_policy_transition_request_v1,
    decode_and_verify_signed_account_recovery_policy_transition_approval_v1,
    verify_account_recovery_policy_transition_approval_for_request_v1,
};
use kilogram_protocol::canonical_account_recovery_v1::{
    CanonicalAccountRecoveryApprovalRequestV1, account_recovery_request_id_v1,
    decode_and_verify_account_recovery_approval_request_v1,
    decode_and_verify_signed_account_recovery_approval_v1,
    decode_and_verify_signed_account_recovery_package_v1,
    decode_and_verify_signed_account_recovery_witness_v1,
    verify_account_recovery_approval_for_request_v1,
};
use kilogram_protocol::canonical_account_v1::{
    CanonicalSignedAccountDeviceListV1, decode_and_verify_signed_account_device_list_v1,
    decode_and_verify_signed_conversation_membership_v1,
};
use kilogram_protocol::canonical_authority_v1::{
    decode_and_verify_signed_device_certificate_v1,
    decode_and_verify_signed_device_session_authorization_v1,
};
use kilogram_protocol::canonical_device_link_v1::decode_and_verify_signed_device_link_authorization_v1;
use kilogram_protocol::canonical_endpoint_announcement_transport_v1::{
    decode_and_verify_encrypted_endpoint_announcement_bundle_v1,
    decode_and_verify_signed_endpoint_announcement_acknowledgement_v1,
    open_endpoint_announcement_bundle_v1, verify_endpoint_announcement_acknowledgement_for_v1,
};
use kilogram_protocol::canonical_endpoint_announcement_v1::{
    CanonicalSignedEndpointAnnouncementBundleV1,
    decode_and_verify_signed_accepted_endpoint_observation_v1,
    decode_and_verify_signed_endpoint_announcement_bundle_v1,
};
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
use kilogram_protocol::canonical_publication_resolution_v1::{
    CanonicalPublicationConflictResolutionResponseV1,
    decode_and_verify_publication_conflict_claim_v1,
    decode_and_verify_publication_conflict_resolution_response_v1,
    decode_and_verify_root_signed_publication_conflict_resolution_v1,
    decode_and_verify_signed_publication_conflict_resolution_request_v1,
    verify_publication_conflict_claim_for_response_v1,
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
use kilogram_protocol::canonical_rewrap_v1::{
    CanonicalHistoryRewrapManifestV1, CanonicalSignedHistoryRewrapRequestV1,
    decode_and_verify_history_rewrap_bundle_v1, decode_and_verify_history_rewrap_manifest_v1,
    decode_and_verify_signed_history_rewrap_entry_v1,
    decode_and_verify_signed_history_rewrap_request_v1,
    decode_and_verify_signed_history_rewrap_transfer_v1, open_history_rewrap_entry_v1,
    verify_history_rewrap_transfer_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_contact_v1::{
    CanonicalRuntimeIpcContactRequestV1, CanonicalSignedRuntimeIpcContactResponseV1,
    decode_and_verify_runtime_ipc_contact_request_v1,
    decode_and_verify_signed_runtime_ipc_contact_response_v1,
    verify_runtime_ipc_contact_request_for_descriptor_v1,
    verify_runtime_ipc_contact_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_directory_v1::{
    CanonicalRuntimeIpcDirectoryRequestV1, CanonicalSignedRuntimeIpcDirectoryResponseV1,
    decode_and_verify_runtime_ipc_directory_request_v1,
    decode_and_verify_signed_runtime_ipc_directory_response_v1,
    verify_runtime_ipc_directory_request_for_descriptor_v1,
    verify_runtime_ipc_directory_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_mailbox_mutation_v1::{
    CanonicalRuntimeIpcMailboxMutationRequestV1,
    CanonicalSignedRuntimeIpcMailboxMutationResponseV1,
    decode_and_verify_runtime_ipc_mailbox_mutation_request_v1,
    decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1,
    verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1,
    verify_runtime_ipc_mailbox_mutation_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_mailbox_status_v1::{
    CanonicalRuntimeIpcMailboxStatusRequestV1, CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
    decode_and_verify_runtime_ipc_mailbox_status_request_v1,
    decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1,
    verify_runtime_ipc_mailbox_status_request_for_descriptor_v1,
    verify_runtime_ipc_mailbox_status_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_outbox_v1::{
    CanonicalRuntimeIpcOutboxRequestV1, CanonicalSignedRuntimeIpcOutboxResponseV1,
    decode_and_verify_runtime_ipc_outbox_request_v1,
    decode_and_verify_signed_runtime_ipc_outbox_response_v1,
    verify_runtime_ipc_outbox_request_for_descriptor_v1,
    verify_runtime_ipc_outbox_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_provider_v1::{
    CanonicalRuntimeIpcProviderRequestV1, CanonicalSignedRuntimeIpcProviderResponseV1,
    decode_and_verify_runtime_ipc_provider_request_v1,
    decode_and_verify_signed_runtime_ipc_provider_response_v1,
    verify_runtime_ipc_provider_request_for_descriptor_v1,
    verify_runtime_ipc_provider_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_publication_conflict_v1::{
    CanonicalRuntimeIpcPublicationConflictRequestV1,
    CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
    decode_and_verify_runtime_ipc_publication_conflict_request_v1,
    decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1,
    verify_runtime_ipc_publication_conflict_request_for_descriptor_v1,
    verify_runtime_ipc_publication_conflict_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_publication_network_v1::{
    CanonicalRuntimeIpcPublicationNetworkRequestV1,
    CanonicalSignedRuntimeIpcPublicationNetworkResponseV1,
    decode_and_verify_runtime_ipc_publication_network_request_v1,
    decode_and_verify_signed_runtime_ipc_publication_network_response_v1,
    verify_runtime_ipc_publication_network_request_for_descriptor_v1,
    verify_runtime_ipc_publication_network_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_publication_v1::{
    CanonicalRuntimeIpcPublicationRequestV1, CanonicalSignedRuntimeIpcPublicationResponseV1,
    decode_and_verify_runtime_ipc_publication_request_v1,
    decode_and_verify_signed_runtime_ipc_publication_response_v1,
    verify_runtime_ipc_publication_request_for_descriptor_v1,
    verify_runtime_ipc_publication_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_queue_v1::{
    CanonicalRuntimeIpcQueueRequestV1, CanonicalSignedRuntimeIpcQueueResponseV1,
    decode_and_verify_runtime_ipc_queue_request_v1,
    decode_and_verify_signed_runtime_ipc_queue_response_v1,
    verify_runtime_ipc_queue_request_for_descriptor_v1,
    verify_runtime_ipc_queue_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_read_v1::{
    CanonicalRuntimeIpcReadRequestV1, CanonicalSignedRuntimeIpcReadResponseV1,
    decode_and_verify_runtime_ipc_read_request_v1,
    decode_and_verify_signed_runtime_ipc_read_response_v1,
    verify_runtime_ipc_read_request_for_descriptor_v1,
    verify_runtime_ipc_read_response_for_request_v1,
};
use kilogram_protocol::canonical_runtime_ipc_v1::{
    CanonicalRuntimeIpcCoreRequestV1, CanonicalSignedRuntimeIpcCoreResponseV1,
    CanonicalSignedRuntimeIpcDescriptorV1, decode_and_verify_runtime_ipc_core_request_v1,
    decode_and_verify_signed_runtime_ipc_core_response_v1,
    decode_and_verify_signed_runtime_ipc_descriptor_v1,
    verify_runtime_ipc_core_request_for_descriptor_v1,
    verify_runtime_ipc_core_response_for_request_v1,
};
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
const EXPECTED_MUTATIONS: usize = 25_623;
const MAXIMUM_MUTATIONS: usize = 30_000;
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
    RuntimeIpcDescriptor,
    RuntimeIpcRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcCoreResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcResponse {
        request: Box<CanonicalRuntimeIpcCoreRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcReadRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcReadResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcReadResponse {
        request: Box<CanonicalRuntimeIpcReadRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcContactRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcContactResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcContactResponse {
        request: Box<CanonicalRuntimeIpcContactRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcDirectoryRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
        response: Box<CanonicalSignedRuntimeIpcDirectoryResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcDirectoryResponse {
        request: Box<CanonicalRuntimeIpcDirectoryRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
    },
    RuntimeIpcQueueRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcQueueResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcQueueResponse {
        request: Box<CanonicalRuntimeIpcQueueRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcOutboxRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcOutboxResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcOutboxResponse {
        request: Box<CanonicalRuntimeIpcOutboxRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcProviderRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcProviderResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcProviderResponse {
        request: Box<CanonicalRuntimeIpcProviderRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcMailboxStatusRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcMailboxStatusResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcMailboxStatusResponse {
        request: Box<CanonicalRuntimeIpcMailboxStatusRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcMailboxMutationRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcMailboxMutationResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcMailboxMutationResponse {
        request: Box<CanonicalRuntimeIpcMailboxMutationRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcPublicationRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcPublicationResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcPublicationResponse {
        request: Box<CanonicalRuntimeIpcPublicationRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcPublicationNetworkRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcPublicationNetworkResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcPublicationNetworkResponse {
        request: Box<CanonicalRuntimeIpcPublicationNetworkRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    RuntimeIpcPublicationConflictRequest {
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
        response: Box<CanonicalSignedRuntimeIpcPublicationConflictResponseV1>,
        now_unix_seconds: u64,
    },
    RuntimeIpcPublicationConflictResponse {
        request: Box<CanonicalRuntimeIpcPublicationConflictRequestV1>,
        descriptor: Box<CanonicalSignedRuntimeIpcDescriptorV1>,
    },
    TicketPublicationCapability,
    TicketPublicationWriteAuthorization,
    TicketPublication,
    TicketPublicationObservation,
    PublicationConflictProof,
    PublicationConflictResolutionRequest,
    PublicationConflictResolution,
    PublicationConflictResolutionResponse,
    PublicationConflictClaim {
        response: Box<CanonicalPublicationConflictResolutionResponseV1>,
    },
    AcceptedEndpointObservation,
    EndpointAnnouncementBundle,
    EndpointAnnouncementEnvelope {
        recipient_secret: [u8; 32],
        now_unix_seconds: u64,
    },
    EndpointAnnouncementAcknowledgement {
        session_binding: [u8; 32],
        bundle: Box<CanonicalSignedEndpointAnnouncementBundleV1>,
    },
    HistoryRewrapRequest,
    HistoryRewrapEntry {
        manifest: Box<CanonicalHistoryRewrapManifestV1>,
        recipient_secret: [u8; 32],
    },
    HistoryRewrapBundle,
    HistoryRewrapTransfer {
        request: Box<CanonicalSignedHistoryRewrapRequestV1>,
        session_binding: [u8; 32],
    },
    AccountRecoveryPackage,
    AccountRecoveryWitness,
    AccountRecoveryRequest {
        expected_id: [u8; 32],
    },
    AccountRecoveryApproval {
        request: Box<CanonicalAccountRecoveryApprovalRequestV1>,
        now_unix_seconds: u64,
    },
    AccountRecoveryPolicyRequest {
        expected_id: [u8; 32],
    },
    AccountRecoveryPolicyApproval {
        request: Box<CanonicalAccountRecoveryPolicyTransitionRequestV1>,
    },
    AccountRecoveryPolicyCertificate,
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
        Decoder::RuntimeIpcDescriptor => {
            decode_and_verify_signed_runtime_ipc_descriptor_v1(bytes).is_ok()
        }
        Decoder::RuntimeIpcRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_core_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_core_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_core_response_for_request_v1(response, &request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_core_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_core_response_for_request_v1(&response, request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcReadRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_read_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_read_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_read_response_for_request_v1(response, &request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcReadResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_read_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_read_response_for_request_v1(&response, request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcContactRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_contact_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_contact_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_contact_response_for_request_v1(response, &request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcContactResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_contact_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_contact_response_for_request_v1(&response, request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcDirectoryRequest {
            descriptor,
            current_device_list,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_directory_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_directory_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    current_device_list,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_directory_response_for_request_v1(
                    response,
                    &request,
                    descriptor,
                    current_device_list,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcDirectoryResponse {
            request,
            descriptor,
            current_device_list,
        } => decode_and_verify_signed_runtime_ipc_directory_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_directory_response_for_request_v1(
                    &response,
                    request,
                    descriptor,
                    current_device_list,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcQueueRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_queue_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_queue_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_queue_response_for_request_v1(response, &request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcQueueResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_queue_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_queue_response_for_request_v1(&response, request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcOutboxRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_outbox_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_outbox_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_outbox_response_for_request_v1(response, &request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcOutboxResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_outbox_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_outbox_response_for_request_v1(&response, request, descriptor)
            })
            .is_ok(),
        Decoder::RuntimeIpcProviderRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_provider_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_provider_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_provider_response_for_request_v1(
                    response,
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcProviderResponse {
            request,
            descriptor,
            now_unix_seconds,
        } => decode_and_verify_signed_runtime_ipc_provider_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_provider_response_for_request_v1(
                    &response,
                    request,
                    descriptor,
                    *now_unix_seconds,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcMailboxStatusRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_mailbox_status_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_mailbox_status_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_mailbox_status_response_for_request_v1(
                    response, &request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcMailboxStatusResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_mailbox_status_response_for_request_v1(
                    &response, request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcMailboxMutationRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_mailbox_mutation_response_for_request_v1(
                    response, &request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcMailboxMutationResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_mailbox_mutation_response_for_request_v1(
                    &response, request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_publication_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_publication_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_publication_response_for_request_v1(
                    response, &request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_publication_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_publication_response_for_request_v1(
                    &response, request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationNetworkRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_publication_network_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_publication_network_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_publication_network_response_for_request_v1(
                    response, &request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationNetworkResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_publication_network_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_publication_network_response_for_request_v1(
                    &response, request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationConflictRequest {
            descriptor,
            response,
            now_unix_seconds,
        } => decode_and_verify_runtime_ipc_publication_conflict_request_v1(bytes)
            .and_then(|request| {
                verify_runtime_ipc_publication_conflict_request_for_descriptor_v1(
                    &request,
                    descriptor,
                    *now_unix_seconds,
                )?;
                verify_runtime_ipc_publication_conflict_response_for_request_v1(
                    response, &request, descriptor,
                )
            })
            .is_ok(),
        Decoder::RuntimeIpcPublicationConflictResponse {
            request,
            descriptor,
        } => decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1(bytes)
            .and_then(|response| {
                verify_runtime_ipc_publication_conflict_response_for_request_v1(
                    &response, request, descriptor,
                )
            })
            .is_ok(),
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
        Decoder::PublicationConflictResolutionRequest => {
            decode_and_verify_signed_publication_conflict_resolution_request_v1(bytes).is_ok()
        }
        Decoder::PublicationConflictResolution => {
            decode_and_verify_root_signed_publication_conflict_resolution_v1(bytes).is_ok()
        }
        Decoder::PublicationConflictResolutionResponse => {
            decode_and_verify_publication_conflict_resolution_response_v1(bytes).is_ok()
        }
        Decoder::PublicationConflictClaim { response } => {
            decode_and_verify_publication_conflict_claim_v1(bytes)
                .and_then(|claim| {
                    verify_publication_conflict_claim_for_response_v1(&claim, response)
                })
                .is_ok()
        }
        Decoder::AcceptedEndpointObservation => {
            decode_and_verify_signed_accepted_endpoint_observation_v1(bytes).is_ok()
        }
        Decoder::EndpointAnnouncementBundle => {
            decode_and_verify_signed_endpoint_announcement_bundle_v1(bytes).is_ok()
        }
        Decoder::EndpointAnnouncementEnvelope {
            recipient_secret,
            now_unix_seconds,
        } => decode_and_verify_encrypted_endpoint_announcement_bundle_v1(bytes)
            .and_then(|envelope| {
                open_endpoint_announcement_bundle_v1(
                    &envelope,
                    &DeviceEncryptionIdentity::from_secret_bytes(*recipient_secret),
                    *now_unix_seconds,
                )
            })
            .is_ok(),
        Decoder::EndpointAnnouncementAcknowledgement {
            session_binding,
            bundle,
        } => decode_and_verify_signed_endpoint_announcement_acknowledgement_v1(bytes)
            .and_then(|acknowledgement| {
                verify_endpoint_announcement_acknowledgement_for_v1(
                    &acknowledgement,
                    *session_binding,
                    bundle,
                )
            })
            .is_ok(),
        Decoder::HistoryRewrapRequest => {
            decode_and_verify_signed_history_rewrap_request_v1(bytes).is_ok()
        }
        Decoder::HistoryRewrapEntry {
            manifest,
            recipient_secret,
        } => decode_and_verify_signed_history_rewrap_entry_v1(bytes)
            .and_then(|entry| {
                open_history_rewrap_entry_v1(
                    &entry,
                    manifest,
                    &DeviceEncryptionIdentity::from_secret_bytes(*recipient_secret),
                )
            })
            .is_ok(),
        Decoder::HistoryRewrapBundle => decode_and_verify_history_rewrap_bundle_v1(bytes).is_ok(),
        Decoder::HistoryRewrapTransfer {
            request,
            session_binding,
        } => decode_and_verify_signed_history_rewrap_transfer_v1(bytes)
            .and_then(|transfer| {
                verify_history_rewrap_transfer_for_request_v1(&transfer, request, *session_binding)
            })
            .is_ok(),
        Decoder::AccountRecoveryPackage => {
            decode_and_verify_signed_account_recovery_package_v1(bytes).is_ok()
        }
        Decoder::AccountRecoveryWitness => {
            decode_and_verify_signed_account_recovery_witness_v1(bytes).is_ok()
        }
        Decoder::AccountRecoveryRequest { expected_id } => {
            decode_and_verify_account_recovery_approval_request_v1(bytes)
                .and_then(|request| account_recovery_request_id_v1(&request))
                .is_ok_and(|request_id| request_id == *expected_id)
        }
        Decoder::AccountRecoveryApproval {
            request,
            now_unix_seconds,
        } => decode_and_verify_signed_account_recovery_approval_v1(bytes)
            .and_then(|approval| {
                verify_account_recovery_approval_for_request_v1(
                    &approval,
                    request,
                    *now_unix_seconds,
                )
            })
            .is_ok(),
        Decoder::AccountRecoveryPolicyRequest { expected_id } => {
            decode_and_verify_account_recovery_policy_transition_request_v1(bytes)
                .and_then(|request| account_recovery_policy_transition_request_id_v1(&request))
                .is_ok_and(|request_id| request_id == *expected_id)
        }
        Decoder::AccountRecoveryPolicyApproval { request } => {
            decode_and_verify_signed_account_recovery_policy_transition_approval_v1(bytes)
                .and_then(|approval| {
                    verify_account_recovery_policy_transition_approval_for_request_v1(
                        &approval, request,
                    )
                })
                .is_ok()
        }
        Decoder::AccountRecoveryPolicyCertificate => {
            decode_and_verify_account_recovery_policy_transition_certificate_v1(bytes).is_ok()
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
    let runtime_ipc: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
    )))?;
    let runtime_ipc_read: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-READ-VECTORS.json"
    )))?;
    let runtime_ipc_contact: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
    )))?;
    let runtime_ipc_directory: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json"
    )))?;
    let runtime_ipc_queue: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json"
    )))?;
    let runtime_ipc_outbox: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json"
    )))?;
    let runtime_ipc_provider: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json"
    )))?;
    let runtime_ipc_mailbox_status: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json"
    )))?;
    let runtime_ipc_mailbox_mutation: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json"
    )))?;
    let runtime_ipc_publication: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json"
    )))?;
    let runtime_ipc_publication_network: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json"
    )))?;
    let runtime_ipc_publication_conflict: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json"
    )))?;
    let ticket_publication: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-TICKET-PUBLICATION-VECTORS.json"
    )))?;
    let publication_conflict: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
    )))?;
    let endpoint_announcement: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
    )))?;
    let endpoint_announcement_transport: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json"
    )))?;
    let history_rewrap: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-HISTORY-REWRAP-VECTORS.json"
    )))?;
    let account_recovery: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-ACCOUNT-RECOVERY-VECTORS.json"
    )))?;
    let account_recovery_policy: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json"
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
    let runtime_ipc_descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&pointer_hex(
        &runtime_ipc,
        "/expected/descriptor/signed_cbor_hex",
    )?)?;
    let runtime_ipc_request = decode_and_verify_runtime_ipc_core_request_v1(&pointer_hex(
        &runtime_ipc,
        "/expected/request/cbor_hex",
    )?)?;
    let runtime_ipc_response = decode_and_verify_signed_runtime_ipc_core_response_v1(
        &pointer_hex(&runtime_ipc, "/expected/response/signed_cbor_hex")?,
    )?;
    let runtime_ipc_valid_at = runtime_ipc["expected"]["context"]["valid_at_unix_seconds"]
        .as_u64()
        .ok_or("runtime IPC valid time is missing")?;
    let runtime_ipc_list_request = decode_and_verify_runtime_ipc_read_request_v1(&pointer_hex(
        &runtime_ipc_read,
        "/expected/conversation_list/request/cbor_hex",
    )?)?;
    let runtime_ipc_list_response =
        decode_and_verify_signed_runtime_ipc_read_response_v1(&pointer_hex(
            &runtime_ipc_read,
            "/expected/conversation_list/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_history_request = decode_and_verify_runtime_ipc_read_request_v1(&pointer_hex(
        &runtime_ipc_read,
        "/expected/history_page/request/cbor_hex",
    )?)?;
    let runtime_ipc_history_response =
        decode_and_verify_signed_runtime_ipc_read_response_v1(&pointer_hex(
            &runtime_ipc_read,
            "/expected/history_page/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_read_valid_at =
        runtime_ipc_read["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC read valid time is missing")?;
    let runtime_ipc_add_contact_request =
        decode_and_verify_runtime_ipc_contact_request_v1(&pointer_hex(
            &runtime_ipc_contact,
            "/expected/add_contact/request/cbor_hex",
        )?)?;
    let runtime_ipc_add_contact_response =
        decode_and_verify_signed_runtime_ipc_contact_response_v1(&pointer_hex(
            &runtime_ipc_contact,
            "/expected/add_contact/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_directory_status_request =
        decode_and_verify_runtime_ipc_contact_request_v1(&pointer_hex(
            &runtime_ipc_contact,
            "/expected/directory_status/request/cbor_hex",
        )?)?;
    let runtime_ipc_directory_status_response =
        decode_and_verify_signed_runtime_ipc_contact_response_v1(&pointer_hex(
            &runtime_ipc_contact,
            "/expected/directory_status/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_contact_valid_at =
        runtime_ipc_contact["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC contact valid time is missing")?;
    let runtime_ipc_directory_current_device_list =
        decode_and_verify_signed_account_device_list_v1(&pointer_hex(
            &runtime_ipc_directory,
            "/expected/current_device_list/signed_cbor_hex",
        )?)?;
    let runtime_ipc_directory_request = decode_and_verify_runtime_ipc_directory_request_v1(
        &pointer_hex(&runtime_ipc_directory, "/expected/request/cbor_hex")?,
    )?;
    let runtime_ipc_directory_response =
        decode_and_verify_signed_runtime_ipc_directory_response_v1(&pointer_hex(
            &runtime_ipc_directory,
            "/expected/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_directory_valid_at =
        runtime_ipc_directory["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC directory valid time is missing")?;
    let runtime_ipc_queue_request = decode_and_verify_runtime_ipc_queue_request_v1(&pointer_hex(
        &runtime_ipc_queue,
        "/expected/request/cbor_hex",
    )?)?;
    let runtime_ipc_queue_response = decode_and_verify_signed_runtime_ipc_queue_response_v1(
        &pointer_hex(&runtime_ipc_queue, "/expected/response/signed_cbor_hex")?,
    )?;
    let runtime_ipc_queue_valid_at =
        runtime_ipc_queue["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC queue valid time is missing")?;
    let runtime_ipc_outbox_request = decode_and_verify_runtime_ipc_outbox_request_v1(
        &pointer_hex(&runtime_ipc_outbox, "/expected/request/cbor_hex")?,
    )?;
    let runtime_ipc_outbox_response = decode_and_verify_signed_runtime_ipc_outbox_response_v1(
        &pointer_hex(&runtime_ipc_outbox, "/expected/response/signed_cbor_hex")?,
    )?;
    let runtime_ipc_outbox_valid_at =
        runtime_ipc_outbox["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC outbox valid time is missing")?;
    let runtime_ipc_provider_request = decode_and_verify_runtime_ipc_provider_request_v1(
        &pointer_hex(&runtime_ipc_provider, "/expected/request/cbor_hex")?,
    )?;
    let runtime_ipc_provider_response = decode_and_verify_signed_runtime_ipc_provider_response_v1(
        &pointer_hex(&runtime_ipc_provider, "/expected/response/signed_cbor_hex")?,
    )?;
    let runtime_ipc_provider_selection_request =
        decode_and_verify_runtime_ipc_provider_request_v1(&pointer_hex(
            &runtime_ipc_provider,
            "/expected/selection_request/cbor_hex",
        )?)?;
    let runtime_ipc_provider_selection_response =
        decode_and_verify_signed_runtime_ipc_provider_response_v1(&pointer_hex(
            &runtime_ipc_provider,
            "/expected/selection_response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_provider_valid_at =
        runtime_ipc_provider["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC provider valid time is missing")?;
    let runtime_ipc_mailbox_status_descriptor =
        decode_and_verify_signed_runtime_ipc_descriptor_v1(&pointer_hex(
            &runtime_ipc_mailbox_status,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_status_request =
        decode_and_verify_runtime_ipc_mailbox_status_request_v1(&pointer_hex(
            &runtime_ipc_mailbox_status,
            "/expected/request/cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_status_response =
        decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1(&pointer_hex(
            &runtime_ipc_mailbox_status,
            "/expected/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_status_valid_at =
        runtime_ipc_mailbox_status["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC mailbox status valid time is missing")?;
    let runtime_ipc_mailbox_mutation_descriptor =
        decode_and_verify_signed_runtime_ipc_descriptor_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_create_request =
        decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/create/request/cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_create_response =
        decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/create/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_rotate_request =
        decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/rotate/request/cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_rotate_response =
        decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/rotate/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_revoke_request =
        decode_and_verify_runtime_ipc_mailbox_mutation_request_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/revoke/request/cbor_hex",
        )?)?;
    let runtime_ipc_mailbox_revoke_response =
        decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1(&pointer_hex(
            &runtime_ipc_mailbox_mutation,
            "/expected/revoke/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_request = decode_and_verify_runtime_ipc_publication_request_v1(
        &pointer_hex(&runtime_ipc_publication, "/expected/request/cbor_hex")?,
    )?;
    let runtime_ipc_publication_response =
        decode_and_verify_signed_runtime_ipc_publication_response_v1(&pointer_hex(
            &runtime_ipc_publication,
            "/expected/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_valid_at =
        runtime_ipc_publication["expected"]["context"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC publication valid time is missing")?;
    let runtime_ipc_publication_network_publish_request =
        decode_and_verify_runtime_ipc_publication_network_request_v1(&pointer_hex(
            &runtime_ipc_publication_network,
            "/expected/publish/request/cbor_hex",
        )?)?;
    let runtime_ipc_publication_network_publish_response =
        decode_and_verify_signed_runtime_ipc_publication_network_response_v1(&pointer_hex(
            &runtime_ipc_publication_network,
            "/expected/publish/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_network_refresh_request =
        decode_and_verify_runtime_ipc_publication_network_request_v1(&pointer_hex(
            &runtime_ipc_publication_network,
            "/expected/refresh/request/cbor_hex",
        )?)?;
    let runtime_ipc_publication_network_refresh_response =
        decode_and_verify_signed_runtime_ipc_publication_network_response_v1(&pointer_hex(
            &runtime_ipc_publication_network,
            "/expected/refresh/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_descriptor =
        decode_and_verify_signed_runtime_ipc_descriptor_v1(&pointer_hex(
            &runtime_ipc_publication_conflict,
            "/expected/descriptor_signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_create_request =
        decode_and_verify_runtime_ipc_publication_conflict_request_v1(&pointer_hex(
            &runtime_ipc_publication_conflict,
            "/expected/create/request/cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_create_response =
        decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1(&pointer_hex(
            &runtime_ipc_publication_conflict,
            "/expected/create/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_apply_request =
        decode_and_verify_runtime_ipc_publication_conflict_request_v1(&pointer_hex(
            &runtime_ipc_publication_conflict,
            "/expected/apply/request/cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_apply_response =
        decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1(&pointer_hex(
            &runtime_ipc_publication_conflict,
            "/expected/apply/response/signed_cbor_hex",
        )?)?;
    let runtime_ipc_publication_conflict_create_valid_at =
        runtime_ipc_publication_conflict["input"]["create_issued_at_unix_seconds"]
            .as_u64()
            .ok_or("runtime IPC publication-conflict create valid time is missing")?;
    let runtime_ipc_publication_conflict_apply_valid_at = runtime_ipc_publication_conflict["input"]
        ["apply_issued_at_unix_seconds"]
        .as_u64()
        .ok_or("runtime IPC publication-conflict apply valid time is missing")?;
    let publication_resolution_response_bytes = pointer_hex(
        &publication_conflict,
        "/expected/resolution_response/cbor_hex",
    )?;
    let publication_resolution_response =
        decode_and_verify_publication_conflict_resolution_response_v1(
            &publication_resolution_response_bytes,
        )?;
    let endpoint_bundle = decode_and_verify_signed_endpoint_announcement_bundle_v1(&pointer_hex(
        &endpoint_announcement,
        "/expected/signed_bundle/signed_cbor_hex",
    )?)?;
    let endpoint_recipient_secret: [u8; 32] = pointer_hex(
        &endpoint_announcement,
        "/input/recipient_encryption_secret_seed_hex",
    )?
    .try_into()
    .map_err(|_| "endpoint recipient encryption secret length changed")?;
    let endpoint_session_binding: [u8; 32] = pointer_hex(
        &endpoint_announcement_transport,
        "/input/session_binding_hex",
    )?
    .try_into()
    .map_err(|_| "endpoint session binding length changed")?;
    let endpoint_valid_at =
        endpoint_announcement["expected"]["signed_bundle"]["valid_at_unix_seconds"]
            .as_u64()
            .ok_or("endpoint bundle valid time is missing")?;
    let history_rewrap_request = decode_and_verify_signed_history_rewrap_request_v1(&pointer_hex(
        &history_rewrap,
        "/expected/request/signed_cbor_hex",
    )?)?;
    let history_rewrap_manifest = decode_and_verify_history_rewrap_manifest_v1(&pointer_hex(
        &history_rewrap,
        "/expected/manifest/cbor_hex",
    )?)?;
    let history_rewrap_recipient_secret: [u8; 32] = pointer_hex(
        &endpoint_announcement,
        "/input/recipient_encryption_secret_seed_hex",
    )?
    .try_into()
    .map_err(|_| "history-rewrap recipient encryption secret length changed")?;
    let history_rewrap_session_binding: [u8; 32] =
        pointer_hex(&history_rewrap, "/input/session_binding_hex")?
            .try_into()
            .map_err(|_| "history-rewrap session binding length changed")?;
    let account_recovery_request = decode_and_verify_account_recovery_approval_request_v1(
        &pointer_hex(&account_recovery, "/expected/request/cbor_hex")?,
    )?;
    let account_recovery_request_id = account_recovery_request_id_v1(&account_recovery_request)?;
    let account_recovery_valid_at = account_recovery["input"]["approved_at_unix_seconds"]
        .as_u64()
        .ok_or("Account recovery approval time is missing")?;
    let account_recovery_policy_request =
        decode_and_verify_account_recovery_policy_transition_request_v1(&pointer_hex(
            &account_recovery_policy,
            "/expected/request/cbor_hex",
        )?)?;
    let account_recovery_policy_request_id =
        account_recovery_policy_transition_request_id_v1(&account_recovery_policy_request)?;
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
            "signed-runtime-ipc-descriptor",
            Decoder::RuntimeIpcDescriptor,
            pointer_hex(&runtime_ipc, "/expected/descriptor/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-core-request",
            Decoder::RuntimeIpcRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_response.clone()),
                now_unix_seconds: runtime_ipc_valid_at,
            },
            pointer_hex(&runtime_ipc, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-core-response",
            Decoder::RuntimeIpcResponse {
                request: Box::new(runtime_ipc_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(&runtime_ipc, "/expected/response/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-conversation-list-page-request",
            Decoder::RuntimeIpcReadRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_list_response.clone()),
                now_unix_seconds: runtime_ipc_read_valid_at,
            },
            pointer_hex(
                &runtime_ipc_read,
                "/expected/conversation_list/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-conversation-list-page-response",
            Decoder::RuntimeIpcReadResponse {
                request: Box::new(runtime_ipc_list_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_read,
                "/expected/conversation_list/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-history-page-request",
            Decoder::RuntimeIpcReadRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_history_response.clone()),
                now_unix_seconds: runtime_ipc_read_valid_at,
            },
            pointer_hex(&runtime_ipc_read, "/expected/history_page/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-history-page-response",
            Decoder::RuntimeIpcReadResponse {
                request: Box::new(runtime_ipc_history_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_read,
                "/expected/history_page/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-add-contact-request",
            Decoder::RuntimeIpcContactRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_add_contact_response.clone()),
                now_unix_seconds: runtime_ipc_contact_valid_at,
            },
            pointer_hex(
                &runtime_ipc_contact,
                "/expected/add_contact/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-add-contact-response",
            Decoder::RuntimeIpcContactResponse {
                request: Box::new(runtime_ipc_add_contact_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_contact,
                "/expected/add_contact/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-device-directory-status-request",
            Decoder::RuntimeIpcContactRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_directory_status_response.clone()),
                now_unix_seconds: runtime_ipc_contact_valid_at,
            },
            pointer_hex(
                &runtime_ipc_contact,
                "/expected/directory_status/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-device-directory-status-response",
            Decoder::RuntimeIpcContactResponse {
                request: Box::new(runtime_ipc_directory_status_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_contact,
                "/expected/directory_status/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-device-directory-apply-request",
            Decoder::RuntimeIpcDirectoryRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                current_device_list: Box::new(runtime_ipc_directory_current_device_list.clone()),
                response: Box::new(runtime_ipc_directory_response.clone()),
                now_unix_seconds: runtime_ipc_directory_valid_at,
            },
            pointer_hex(&runtime_ipc_directory, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-device-directory-apply-response",
            Decoder::RuntimeIpcDirectoryResponse {
                request: Box::new(runtime_ipc_directory_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                current_device_list: Box::new(runtime_ipc_directory_current_device_list.clone()),
            },
            pointer_hex(&runtime_ipc_directory, "/expected/response/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-queue-message-request",
            Decoder::RuntimeIpcQueueRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_queue_response.clone()),
                now_unix_seconds: runtime_ipc_queue_valid_at,
            },
            pointer_hex(&runtime_ipc_queue, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-message-queued-response",
            Decoder::RuntimeIpcQueueResponse {
                request: Box::new(runtime_ipc_queue_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(&runtime_ipc_queue, "/expected/response/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-outbox-status-page-request",
            Decoder::RuntimeIpcOutboxRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_outbox_response.clone()),
                now_unix_seconds: runtime_ipc_outbox_valid_at,
            },
            pointer_hex(&runtime_ipc_outbox, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-outbox-status-page-response",
            Decoder::RuntimeIpcOutboxResponse {
                request: Box::new(runtime_ipc_outbox_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(&runtime_ipc_outbox, "/expected/response/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-provider-import-request",
            Decoder::RuntimeIpcProviderRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_provider_response.clone()),
                now_unix_seconds: runtime_ipc_provider_valid_at,
            },
            pointer_hex(&runtime_ipc_provider, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-provider-import-response",
            Decoder::RuntimeIpcProviderResponse {
                request: Box::new(runtime_ipc_provider_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                now_unix_seconds: runtime_ipc_provider_valid_at,
            },
            pointer_hex(&runtime_ipc_provider, "/expected/response/signed_cbor_hex")?,
        ),
        (
            "runtime-ipc-provider-selection-request",
            Decoder::RuntimeIpcProviderRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_provider_selection_response.clone()),
                now_unix_seconds: runtime_ipc_provider_valid_at,
            },
            pointer_hex(
                &runtime_ipc_provider,
                "/expected/selection_request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-provider-selection-response",
            Decoder::RuntimeIpcProviderResponse {
                request: Box::new(runtime_ipc_provider_selection_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                now_unix_seconds: runtime_ipc_provider_valid_at,
            },
            pointer_hex(
                &runtime_ipc_provider,
                "/expected/selection_response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-mailbox-capability-status-page-request",
            Decoder::RuntimeIpcMailboxStatusRequest {
                descriptor: Box::new(runtime_ipc_mailbox_status_descriptor.clone()),
                response: Box::new(runtime_ipc_mailbox_status_response.clone()),
                now_unix_seconds: runtime_ipc_mailbox_status_valid_at,
            },
            pointer_hex(&runtime_ipc_mailbox_status, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-mailbox-capability-status-page-response",
            Decoder::RuntimeIpcMailboxStatusResponse {
                request: Box::new(runtime_ipc_mailbox_status_request.clone()),
                descriptor: Box::new(runtime_ipc_mailbox_status_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_mailbox_status,
                "/expected/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-mailbox-capability-create-request",
            Decoder::RuntimeIpcMailboxMutationRequest {
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
                response: Box::new(runtime_ipc_mailbox_create_response.clone()),
                now_unix_seconds: 1_800_001_000,
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/create/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-mailbox-capability-create-response",
            Decoder::RuntimeIpcMailboxMutationResponse {
                request: Box::new(runtime_ipc_mailbox_create_request.clone()),
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/create/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-mailbox-capability-rotate-request",
            Decoder::RuntimeIpcMailboxMutationRequest {
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
                response: Box::new(runtime_ipc_mailbox_rotate_response.clone()),
                now_unix_seconds: 1_800_001_010,
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/rotate/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-mailbox-capability-rotate-response",
            Decoder::RuntimeIpcMailboxMutationResponse {
                request: Box::new(runtime_ipc_mailbox_rotate_request.clone()),
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/rotate/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-mailbox-capability-revoke-request",
            Decoder::RuntimeIpcMailboxMutationRequest {
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
                response: Box::new(runtime_ipc_mailbox_revoke_response.clone()),
                now_unix_seconds: 1_800_002_000,
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/revoke/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-mailbox-capability-revoke-response",
            Decoder::RuntimeIpcMailboxMutationResponse {
                request: Box::new(runtime_ipc_mailbox_revoke_request.clone()),
                descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_mailbox_mutation,
                "/expected/revoke/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-publication-rotate-request",
            Decoder::RuntimeIpcPublicationRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_publication_response.clone()),
                now_unix_seconds: runtime_ipc_publication_valid_at,
            },
            pointer_hex(&runtime_ipc_publication, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-runtime-ipc-publication-rotate-response",
            Decoder::RuntimeIpcPublicationResponse {
                request: Box::new(runtime_ipc_publication_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_publication,
                "/expected/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-publication-network-publish-request",
            Decoder::RuntimeIpcPublicationNetworkRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_publication_network_publish_response.clone()),
                now_unix_seconds: 1_800_002_190,
            },
            pointer_hex(
                &runtime_ipc_publication_network,
                "/expected/publish/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-publication-network-publish-response",
            Decoder::RuntimeIpcPublicationNetworkResponse {
                request: Box::new(runtime_ipc_publication_network_publish_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_publication_network,
                "/expected/publish/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-publication-network-refresh-request",
            Decoder::RuntimeIpcPublicationNetworkRequest {
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
                response: Box::new(runtime_ipc_publication_network_refresh_response.clone()),
                now_unix_seconds: 1_800_002_205,
            },
            pointer_hex(
                &runtime_ipc_publication_network,
                "/expected/refresh/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-publication-network-refresh-response",
            Decoder::RuntimeIpcPublicationNetworkResponse {
                request: Box::new(runtime_ipc_publication_network_refresh_request.clone()),
                descriptor: Box::new(runtime_ipc_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_publication_network,
                "/expected/refresh/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-publication-conflict-create-request",
            Decoder::RuntimeIpcPublicationConflictRequest {
                descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
                response: Box::new(runtime_ipc_publication_conflict_create_response.clone()),
                now_unix_seconds: runtime_ipc_publication_conflict_create_valid_at,
            },
            pointer_hex(
                &runtime_ipc_publication_conflict,
                "/expected/create/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-publication-conflict-create-response",
            Decoder::RuntimeIpcPublicationConflictResponse {
                request: Box::new(runtime_ipc_publication_conflict_create_request.clone()),
                descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_publication_conflict,
                "/expected/create/response/signed_cbor_hex",
            )?,
        ),
        (
            "runtime-ipc-publication-conflict-apply-request",
            Decoder::RuntimeIpcPublicationConflictRequest {
                descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
                response: Box::new(runtime_ipc_publication_conflict_apply_response.clone()),
                now_unix_seconds: runtime_ipc_publication_conflict_apply_valid_at,
            },
            pointer_hex(
                &runtime_ipc_publication_conflict,
                "/expected/apply/request/cbor_hex",
            )?,
        ),
        (
            "signed-runtime-ipc-publication-conflict-apply-response",
            Decoder::RuntimeIpcPublicationConflictResponse {
                request: Box::new(runtime_ipc_publication_conflict_apply_request.clone()),
                descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
            },
            pointer_hex(
                &runtime_ipc_publication_conflict,
                "/expected/apply/response/signed_cbor_hex",
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
        (
            "signed-publication-conflict-resolution-request",
            Decoder::PublicationConflictResolutionRequest,
            pointer_hex(
                &publication_conflict,
                "/expected/resolution_request/signed_cbor_hex",
            )?,
        ),
        (
            "root-signed-publication-conflict-resolution",
            Decoder::PublicationConflictResolution,
            pointer_hex(
                &publication_conflict,
                "/expected/resolution/signed_cbor_hex",
            )?,
        ),
        (
            "publication-conflict-resolution-response",
            Decoder::PublicationConflictResolutionResponse,
            pointer_hex(
                &publication_conflict,
                "/expected/resolution_response/cbor_hex",
            )?,
        ),
        (
            "publication-conflict-response-claim",
            Decoder::PublicationConflictClaim {
                response: Box::new(publication_resolution_response.clone()),
            },
            pointer_hex(&publication_conflict, "/expected/response_claim/cbor_hex")?,
        ),
        (
            "signed-accepted-endpoint-observation",
            Decoder::AcceptedEndpointObservation,
            pointer_hex(
                &endpoint_announcement,
                "/expected/accepted_observation/signed_cbor_hex",
            )?,
        ),
        (
            "signed-endpoint-announcement-bundle",
            Decoder::EndpointAnnouncementBundle,
            pointer_hex(
                &endpoint_announcement,
                "/expected/signed_bundle/signed_cbor_hex",
            )?,
        ),
        (
            "encrypted-endpoint-announcement-bundle",
            Decoder::EndpointAnnouncementEnvelope {
                recipient_secret: endpoint_recipient_secret,
                now_unix_seconds: endpoint_valid_at,
            },
            pointer_hex(
                &endpoint_announcement_transport,
                "/expected/encrypted_bundle/cbor_hex",
            )?,
        ),
        (
            "signed-endpoint-announcement-acknowledgement",
            Decoder::EndpointAnnouncementAcknowledgement {
                session_binding: endpoint_session_binding,
                bundle: Box::new(endpoint_bundle.clone()),
            },
            pointer_hex(
                &endpoint_announcement_transport,
                "/expected/acknowledgement/signed_cbor_hex",
            )?,
        ),
        (
            "signed-history-rewrap-request",
            Decoder::HistoryRewrapRequest,
            pointer_hex(&history_rewrap, "/expected/request/signed_cbor_hex")?,
        ),
        (
            "signed-history-rewrap-entry",
            Decoder::HistoryRewrapEntry {
                manifest: Box::new(history_rewrap_manifest.clone()),
                recipient_secret: history_rewrap_recipient_secret,
            },
            pointer_hex(&history_rewrap, "/expected/entry/signed_cbor_hex")?,
        ),
        (
            "history-rewrap-bundle",
            Decoder::HistoryRewrapBundle,
            pointer_hex(&history_rewrap, "/expected/bundle/cbor_hex")?,
        ),
        (
            "signed-history-rewrap-transfer",
            Decoder::HistoryRewrapTransfer {
                request: Box::new(history_rewrap_request.clone()),
                session_binding: history_rewrap_session_binding,
            },
            pointer_hex(&history_rewrap, "/expected/transfer/signed_cbor_hex")?,
        ),
        (
            "signed-account-recovery-package",
            Decoder::AccountRecoveryPackage,
            pointer_hex(&account_recovery, "/expected/package/signed_cbor_hex")?,
        ),
        (
            "signed-account-recovery-witness",
            Decoder::AccountRecoveryWitness,
            pointer_hex(&account_recovery, "/expected/witness/signed_cbor_hex")?,
        ),
        (
            "account-recovery-approval-request",
            Decoder::AccountRecoveryRequest {
                expected_id: account_recovery_request_id,
            },
            pointer_hex(&account_recovery, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-account-recovery-approval",
            Decoder::AccountRecoveryApproval {
                request: Box::new(account_recovery_request.clone()),
                now_unix_seconds: account_recovery_valid_at,
            },
            pointer_hex(&account_recovery, "/expected/approval/signed_cbor_hex")?,
        ),
        (
            "account-recovery-policy-transition-request",
            Decoder::AccountRecoveryPolicyRequest {
                expected_id: account_recovery_policy_request_id,
            },
            pointer_hex(&account_recovery_policy, "/expected/request/cbor_hex")?,
        ),
        (
            "signed-account-recovery-policy-transition-approval",
            Decoder::AccountRecoveryPolicyApproval {
                request: Box::new(account_recovery_policy_request.clone()),
            },
            pointer_hex(
                &account_recovery_policy,
                "/expected/approvals/0/signed_cbor_hex",
            )?,
        ),
        (
            "account-recovery-policy-transition-certificate",
            Decoder::AccountRecoveryPolicyCertificate,
            pointer_hex(&account_recovery_policy, "/expected/certificate/cbor_hex")?,
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
        Decoder::RuntimeIpcDescriptor,
        Decoder::RuntimeIpcRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_response),
            now_unix_seconds: runtime_ipc_valid_at,
        },
        Decoder::RuntimeIpcResponse {
            request: Box::new(runtime_ipc_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcReadRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_list_response),
            now_unix_seconds: runtime_ipc_read_valid_at,
        },
        Decoder::RuntimeIpcReadResponse {
            request: Box::new(runtime_ipc_list_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcReadRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_history_response),
            now_unix_seconds: runtime_ipc_read_valid_at,
        },
        Decoder::RuntimeIpcReadResponse {
            request: Box::new(runtime_ipc_history_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcContactRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_add_contact_response),
            now_unix_seconds: runtime_ipc_contact_valid_at,
        },
        Decoder::RuntimeIpcContactResponse {
            request: Box::new(runtime_ipc_add_contact_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcContactRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_directory_status_response),
            now_unix_seconds: runtime_ipc_contact_valid_at,
        },
        Decoder::RuntimeIpcContactResponse {
            request: Box::new(runtime_ipc_directory_status_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcDirectoryRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            current_device_list: Box::new(runtime_ipc_directory_current_device_list.clone()),
            response: Box::new(runtime_ipc_directory_response.clone()),
            now_unix_seconds: runtime_ipc_directory_valid_at,
        },
        Decoder::RuntimeIpcQueueRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_queue_response),
            now_unix_seconds: runtime_ipc_queue_valid_at,
        },
        Decoder::RuntimeIpcQueueResponse {
            request: Box::new(runtime_ipc_queue_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcOutboxRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_outbox_response),
            now_unix_seconds: runtime_ipc_outbox_valid_at,
        },
        Decoder::RuntimeIpcOutboxResponse {
            request: Box::new(runtime_ipc_outbox_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcProviderRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_provider_response),
            now_unix_seconds: runtime_ipc_provider_valid_at,
        },
        Decoder::RuntimeIpcProviderResponse {
            request: Box::new(runtime_ipc_provider_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            now_unix_seconds: runtime_ipc_provider_valid_at,
        },
        Decoder::RuntimeIpcMailboxStatusRequest {
            descriptor: Box::new(runtime_ipc_mailbox_status_descriptor.clone()),
            response: Box::new(runtime_ipc_mailbox_status_response),
            now_unix_seconds: runtime_ipc_mailbox_status_valid_at,
        },
        Decoder::RuntimeIpcMailboxStatusResponse {
            request: Box::new(runtime_ipc_mailbox_status_request),
            descriptor: Box::new(runtime_ipc_mailbox_status_descriptor),
        },
        Decoder::RuntimeIpcMailboxMutationRequest {
            descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor.clone()),
            response: Box::new(runtime_ipc_mailbox_create_response),
            now_unix_seconds: 1_800_001_000,
        },
        Decoder::RuntimeIpcMailboxMutationResponse {
            request: Box::new(runtime_ipc_mailbox_create_request),
            descriptor: Box::new(runtime_ipc_mailbox_mutation_descriptor),
        },
        Decoder::RuntimeIpcPublicationRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_publication_response),
            now_unix_seconds: runtime_ipc_publication_valid_at,
        },
        Decoder::RuntimeIpcPublicationResponse {
            request: Box::new(runtime_ipc_publication_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcPublicationNetworkRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_publication_network_publish_response),
            now_unix_seconds: 1_800_002_190,
        },
        Decoder::RuntimeIpcPublicationNetworkResponse {
            request: Box::new(runtime_ipc_publication_network_publish_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcPublicationNetworkRequest {
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
            response: Box::new(runtime_ipc_publication_network_refresh_response),
            now_unix_seconds: 1_800_002_205,
        },
        Decoder::RuntimeIpcPublicationNetworkResponse {
            request: Box::new(runtime_ipc_publication_network_refresh_request),
            descriptor: Box::new(runtime_ipc_descriptor.clone()),
        },
        Decoder::RuntimeIpcPublicationConflictRequest {
            descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
            response: Box::new(runtime_ipc_publication_conflict_create_response),
            now_unix_seconds: runtime_ipc_publication_conflict_create_valid_at,
        },
        Decoder::RuntimeIpcPublicationConflictResponse {
            request: Box::new(runtime_ipc_publication_conflict_create_request),
            descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
        },
        Decoder::RuntimeIpcPublicationConflictRequest {
            descriptor: Box::new(runtime_ipc_publication_conflict_descriptor.clone()),
            response: Box::new(runtime_ipc_publication_conflict_apply_response),
            now_unix_seconds: runtime_ipc_publication_conflict_apply_valid_at,
        },
        Decoder::RuntimeIpcPublicationConflictResponse {
            request: Box::new(runtime_ipc_publication_conflict_apply_request),
            descriptor: Box::new(runtime_ipc_publication_conflict_descriptor),
        },
        Decoder::RuntimeIpcDirectoryResponse {
            request: Box::new(runtime_ipc_directory_request),
            descriptor: Box::new(runtime_ipc_descriptor),
            current_device_list: Box::new(runtime_ipc_directory_current_device_list),
        },
        Decoder::TicketPublicationCapability,
        Decoder::TicketPublicationWriteAuthorization,
        Decoder::TicketPublication,
        Decoder::TicketPublicationObservation,
        Decoder::PublicationConflictProof,
        Decoder::PublicationConflictResolutionRequest,
        Decoder::PublicationConflictResolution,
        Decoder::PublicationConflictResolutionResponse,
        Decoder::PublicationConflictClaim {
            response: Box::new(publication_resolution_response),
        },
        Decoder::AcceptedEndpointObservation,
        Decoder::EndpointAnnouncementBundle,
        Decoder::EndpointAnnouncementEnvelope {
            recipient_secret: endpoint_recipient_secret,
            now_unix_seconds: endpoint_valid_at,
        },
        Decoder::EndpointAnnouncementAcknowledgement {
            session_binding: endpoint_session_binding,
            bundle: Box::new(endpoint_bundle),
        },
        Decoder::HistoryRewrapRequest,
        Decoder::HistoryRewrapEntry {
            manifest: Box::new(history_rewrap_manifest),
            recipient_secret: history_rewrap_recipient_secret,
        },
        Decoder::HistoryRewrapBundle,
        Decoder::HistoryRewrapTransfer {
            request: Box::new(history_rewrap_request),
            session_binding: history_rewrap_session_binding,
        },
        Decoder::AccountRecoveryPackage,
        Decoder::AccountRecoveryWitness,
        Decoder::AccountRecoveryRequest {
            expected_id: [0; 32],
        },
        Decoder::AccountRecoveryApproval {
            request: Box::new(account_recovery_request),
            now_unix_seconds: account_recovery_valid_at,
        },
        Decoder::AccountRecoveryPolicyRequest {
            expected_id: [0; 32],
        },
        Decoder::AccountRecoveryPolicyApproval {
            request: Box::new(account_recovery_policy_request),
        },
        Decoder::AccountRecoveryPolicyCertificate,
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
