[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$officialPath = Join-Path $workspace 'BLAKE3-REFERENCE-VECTORS.json'
$referencePath = Join-Path $PSScriptRoot 'lib\blake3-reference.mjs'
$verifierPath = Join-Path $PSScriptRoot 'verify-kilogram-blake3-conformance.mjs'
$testPath = Join-Path $workspace 'crates\kilogram-protocol\tests\canonical_decoder_mutations.rs'
$ratchetReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ratchet-reference.mjs'
$recoveryReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-reference.mjs'
$eventReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-event-reference.mjs'
$prekeySelectionReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-prekey-selection-reference.mjs'
$syncReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-reference.mjs'
$syncSessionReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-session-reference.mjs'
$recoveryExchangeReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-exchange-reference.mjs'
$mailboxReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-reference.mjs'
$mailboxTransportReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-transport-reference.mjs'
$mailboxGossipReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-gossip-reference.mjs'
$mailboxProvisioningReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-provisioning-reference.mjs'
$runtimeMailboxReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-mailbox-reference.mjs'
$runtimeIpcReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-reference.mjs'
$runtimeIpcReadReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-read-reference.mjs'
$runtimeIpcContactReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-contact-reference.mjs'
$runtimeIpcDirectoryReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-directory-reference.mjs'
$runtimeIpcQueueReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-queue-reference.mjs'
$runtimeIpcOutboxReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-outbox-reference.mjs'
$runtimeIpcProviderReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-provider-reference.mjs'
$runtimeIpcMailboxStatusReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-mailbox-status-reference.mjs'
$runtimeIpcMailboxMutationReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-mailbox-mutation-reference.mjs'
$runtimeIpcPublicationReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-reference.mjs'
$runtimeIpcPublicationNetworkReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-network-reference.mjs'
$runtimeIpcPublicationConflictReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-conflict-reference.mjs'
$runtimeIpcOwnDeviceAnnouncementReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-announcement-reference.mjs'
$runtimeIpcOwnDeviceDiscoveryReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-discovery-reference.mjs'
$runtimeIpcOwnDeviceRosterReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-roster-reference.mjs'
$runtimeIpcRecoveryPlanReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-recovery-plan-reference.mjs'
$runtimeIpcRecoveryExecutionReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-recovery-execution-reference.mjs'
$runtimeIpcTicketAutomationReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-ticket-automation-reference.mjs'
$ticketPublicationReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ticket-publication-reference.mjs'
$publicationConflictReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-publication-conflict-reference.mjs'
$endpointAnnouncementReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-endpoint-announcement-reference.mjs'
$endpointAnnouncementTransportReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-endpoint-announcement-transport-reference.mjs'
$historyRewrapReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-rewrap-reference.mjs'
$accountRecoveryReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-account-recovery-reference.mjs'
$accountRecoveryPolicyReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-account-recovery-policy-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0131-m115-independent-digests-and-decoder-mutations.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}
function Assert-Manifest {
    param([object]$Manifest)
    if ([UInt64]$Manifest.schema_version -ne 1 -or [string]$Manifest.milestone -cne 'M1.1.5' -or
        [string]$Manifest.status -cne 'bounded-regression-corpus' -or [bool]$Manifest.active_runtime_changed) {
        throw 'decoder mutation manifest header changed'
    }
    if ([int]$Manifest.seed -ne 1262836037 -or [int]$Manifest.minimum_mutations -ne 500 -or
        [int]$Manifest.expected_mutations -ne 31851 -or
        [int]$Manifest.maximum_mutations -ne 35000) { throw 'decoder mutation bounds changed' }
    $expectedFixtures = @(
        'signed-device-certificate-v1|CANONICAL-AUTHORITY-VECTORS.json|/expected/certificate/signed_cbor_hex|decode_and_verify_signed_device_certificate_v1',
        'signed-device-session-authorization-v1|CANONICAL-AUTHORITY-VECTORS.json|/expected/session_authorization/signed_cbor_hex|decode_and_verify_signed_device_session_authorization_v1',
        'signed-prekey-pool-v1|CANONICAL-RATCHET-VECTORS.json|/expected/prekey_pool/signed_cbor_hex|decode_and_verify_signed_prekey_pool_v1',
        'account-prekey-directory-v1|CANONICAL-RATCHET-VECTORS.json|/expected/account_prekey_directory_cbor_hex|decode_and_verify_account_prekey_directory_v1',
        'signed-history-recovery-checkpoint-v1|CANONICAL-RECOVERY-VECTORS.json|/expected/successor_checkpoint/signed_cbor_hex|decode_and_verify_signed_history_recovery_checkpoint_v1'
        'signed-history-recovery-link-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/link/signed_cbor_hex|decode_and_verify_signed_history_recovery_link_v1'
        'signed-history-recovery-plan-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/plan/signed_cbor_hex|decode_and_verify_signed_history_recovery_plan_v1'
        'signed-history-recovery-response-v1|CANONICAL-RECOVERY-EXCHANGE-VECTORS.json|/expected/page_response/signed_cbor_hex|decode_and_verify_signed_history_recovery_response_v1'
        'signed-account-device-list-v1|CANONICAL-ACCOUNT-VECTORS.json|/expected/account_device_list/signed_cbor_hex|decode_and_verify_signed_account_device_list_v1'
        'signed-conversation-membership-v1|CANONICAL-ACCOUNT-VECTORS.json|/expected/conversation_membership/signed_cbor_hex|decode_and_verify_signed_conversation_membership_v1'
        'signed-device-link-authorization-v1|CANONICAL-DEVICE-LINK-VECTORS.json|/expected/authorization/signed_cbor_hex|decode_and_verify_signed_device_link_authorization_v1'
        'signed-acknowledgement-event-v1|CANONICAL-EVENT-VECTORS.json|/expected/acknowledgement_event/signed_cbor_hex|decode_and_verify_signed_event_v1'
        'authorized-text-event-v1|CANONICAL-EVENT-VECTORS.json|/expected/authorized_text_event_cbor_hex|decode_and_verify_authorized_event_v1'
        'prekey-selection-transcript-v1|CANONICAL-PREKEY-SELECTION-VECTORS.json|/expected/transcript_cbor_hex|decode_and_verify_prekey_selection_transcript_v1'
        'signed-sync-diff-v1|CANONICAL-SYNC-VECTORS.json|/expected/diff/signed_cbor_hex|decode_and_verify_signed_sync_diff_v1'
        'sync-client-frame-v1|CANONICAL-SYNC-VECTORS.json|/expected/client_frames/inventory_cbor_hex|decode_and_verify_sync_client_frame_v1'
        'sync-server-frame-v1|CANONICAL-SYNC-VECTORS.json|/expected/server_frames/diff_cbor_hex|decode_and_verify_sync_server_frame_v1'
        'signed-sync-session-checkpoint-v1|CANONICAL-SYNC-SESSION-VECTORS.json|/expected/checkpoints/3/signed_cbor_hex|decode_and_verify_signed_sync_session_checkpoint_v1'
        'signed-mailbox-write-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/write_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_write_authorization_v1'
        'signed-mailbox-read-list-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/read_list_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_read_authorization_v1'
        'signed-mailbox-read-delete-authorization-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/read_delete_authorization/signed_cbor_hex|decode_and_verify_signed_mailbox_read_authorization_v1'
        'signed-mailbox-storage-offer-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/storage_offer/signed_cbor_hex|decode_and_verify_signed_mailbox_storage_offer_v1'
        'signed-mailbox-stored-receipt-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/stored_receipt/signed_cbor_hex|decode_and_verify_signed_mailbox_stored_receipt_v1'
        'signed-mailbox-delete-receipt-v1|CANONICAL-MAILBOX-VECTORS.json|/expected/delete_receipt/signed_cbor_hex|decode_and_verify_signed_mailbox_delete_receipt_v1'
        'mailbox-peer-put-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/put/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
        'mailbox-peer-list-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/list/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
        'mailbox-peer-delete-request-v1|CANONICAL-MAILBOX-TRANSPORT-VECTORS.json|/expected/peer_requests/delete/cbor_hex|decode_and_verify_mailbox_peer_request_v1'
        'mailbox-provider-gossip-frame-v1|CANONICAL-MAILBOX-GOSSIP-VECTORS.json|/expected/request/frame_cbor_hex|decode_and_verify_mailbox_provider_gossip_frame_for_id_v1'
        'signed-local-mailbox-binding-v1|CANONICAL-MAILBOX-PROVISIONING-VECTORS.json|/expected/signed_local_binding/signed_cbor_hex|decode_and_verify_signed_local_mailbox_binding_v1'
        'signed-mailbox-peer-offer-v1|CANONICAL-MAILBOX-PROVISIONING-VECTORS.json|/expected/signed_exact_peer_offer/signed_cbor_hex|decode_and_verify_signed_mailbox_peer_offer_v1'
        'signed-mailbox-capability-update-v1|CANONICAL-MAILBOX-PROVISIONING-VECTORS.json|/expected/activation_update/signed_cbor_hex|decode_and_verify_signed_mailbox_capability_update_v1'
        'signed-mailbox-capability-acknowledgement-v1|CANONICAL-MAILBOX-PROVISIONING-VECTORS.json|/expected/activation_acknowledgement/signed_cbor_hex|decode_and_verify_signed_mailbox_capability_acknowledgement_v1'
        'signed-runtime-local-mailbox-binding-v1|CANONICAL-RUNTIME-MAILBOX-VECTORS.json|/expected/signed_local_binding/signed_cbor_hex|decode_and_verify_signed_runtime_local_mailbox_binding_v1'
        'signed-runtime-peer-mailbox-binding-v1|CANONICAL-RUNTIME-MAILBOX-VECTORS.json|/expected/signed_peer_binding/signed_cbor_hex|decode_and_verify_signed_runtime_peer_mailbox_binding_v1'
        'runtime-mailbox-payload-v1|CANONICAL-RUNTIME-MAILBOX-VECTORS.json|/expected/payload/cbor_hex|decode_and_verify_runtime_mailbox_payload_v1'
        'signed-runtime-mailbox-dispatch-v1|CANONICAL-RUNTIME-MAILBOX-VECTORS.json|/expected/signed_dispatch/signed_cbor_hex|decode_and_verify_signed_runtime_mailbox_dispatch_v1'
        'ticket-publication-capability-v1|CANONICAL-TICKET-PUBLICATION-VECTORS.json|/expected/rotated_capability/cbor_hex|decode_and_verify_ticket_publication_capability_v1'
        'signed-ticket-publication-write-authorization-v1|CANONICAL-TICKET-PUBLICATION-VECTORS.json|/expected/authorization/signed_cbor_hex|decode_and_verify_signed_ticket_publication_write_authorization_v1'
        'signed-ticket-publication-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/first_publication/signed_cbor_hex|decode_and_verify_signed_ticket_publication_v1'
        'signed-ticket-publication-observation-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/first_observation/signed_cbor_hex|decode_and_verify_signed_ticket_publication_observation_v1'
        'signed-publication-conflict-proof-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/conflict_proof/signed_cbor_hex|decode_and_verify_signed_publication_conflict_proof_v1'
        'signed-publication-conflict-resolution-request-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/resolution_request/signed_cbor_hex|decode_and_verify_signed_publication_conflict_resolution_request_v1'
        'root-signed-publication-conflict-resolution-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/resolution/signed_cbor_hex|decode_and_verify_root_signed_publication_conflict_resolution_v1'
        'publication-conflict-resolution-response-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/resolution_response/cbor_hex|decode_and_verify_publication_conflict_resolution_response_v1'
        'publication-conflict-response-claim-v1|CANONICAL-PUBLICATION-CONFLICT-VECTORS.json|/expected/response_claim/cbor_hex|decode_and_verify_publication_conflict_claim_v1'
        'signed-accepted-endpoint-observation-v1|CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json|/expected/accepted_observation/signed_cbor_hex|decode_and_verify_signed_accepted_endpoint_observation_v1'
        'signed-endpoint-announcement-bundle-v1|CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json|/expected/signed_bundle/signed_cbor_hex|decode_and_verify_signed_endpoint_announcement_bundle_v1'
        'encrypted-endpoint-announcement-bundle-v1|CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json|/expected/encrypted_bundle/cbor_hex|decode_and_verify_encrypted_endpoint_announcement_bundle_v1'
        'signed-endpoint-announcement-acknowledgement-v1|CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json|/expected/acknowledgement/signed_cbor_hex|decode_and_verify_signed_endpoint_announcement_acknowledgement_v1'
        'signed-history-rewrap-request-v1|CANONICAL-HISTORY-REWRAP-VECTORS.json|/expected/request/signed_cbor_hex|decode_and_verify_signed_history_rewrap_request_v1'
        'signed-history-rewrap-entry-v1|CANONICAL-HISTORY-REWRAP-VECTORS.json|/expected/entry/signed_cbor_hex|decode_and_verify_signed_history_rewrap_entry_v1'
        'history-rewrap-bundle-v1|CANONICAL-HISTORY-REWRAP-VECTORS.json|/expected/bundle/cbor_hex|decode_and_verify_history_rewrap_bundle_v1'
        'signed-history-rewrap-transfer-v1|CANONICAL-HISTORY-REWRAP-VECTORS.json|/expected/transfer/signed_cbor_hex|decode_and_verify_signed_history_rewrap_transfer_v1'
        'signed-account-recovery-package-v1|CANONICAL-ACCOUNT-RECOVERY-VECTORS.json|/expected/package/signed_cbor_hex|decode_and_verify_signed_account_recovery_package_v1'
        'signed-account-recovery-witness-v1|CANONICAL-ACCOUNT-RECOVERY-VECTORS.json|/expected/witness/signed_cbor_hex|decode_and_verify_signed_account_recovery_witness_v1'
        'account-recovery-approval-request-v1|CANONICAL-ACCOUNT-RECOVERY-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_account_recovery_approval_request_v1'
        'signed-account-recovery-approval-v1|CANONICAL-ACCOUNT-RECOVERY-VECTORS.json|/expected/approval/signed_cbor_hex|decode_and_verify_signed_account_recovery_approval_v1'
        'account-recovery-policy-transition-request-v1|CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_account_recovery_policy_transition_request_v1'
        'signed-account-recovery-policy-transition-approval-v1|CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json|/expected/approvals/0/signed_cbor_hex|decode_and_verify_signed_account_recovery_policy_transition_approval_v1'
        'account-recovery-policy-transition-certificate-v1|CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json|/expected/certificate/cbor_hex|decode_and_verify_account_recovery_policy_transition_certificate_v1'
        'signed-runtime-ipc-descriptor-v1|CANONICAL-RUNTIME-IPC-VECTORS.json|/expected/descriptor/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_descriptor_v1'
        'runtime-ipc-core-request-v1|CANONICAL-RUNTIME-IPC-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_core_request_v1'
        'signed-runtime-ipc-core-response-v1|CANONICAL-RUNTIME-IPC-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_core_response_v1'
        'runtime-ipc-conversation-list-page-request-v1|CANONICAL-RUNTIME-IPC-READ-VECTORS.json|/expected/conversation_list/request/cbor_hex|decode_and_verify_runtime_ipc_read_request_v1'
        'signed-runtime-ipc-conversation-list-page-response-v1|CANONICAL-RUNTIME-IPC-READ-VECTORS.json|/expected/conversation_list/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_read_response_v1'
        'runtime-ipc-history-page-request-v1|CANONICAL-RUNTIME-IPC-READ-VECTORS.json|/expected/history_page/request/cbor_hex|decode_and_verify_runtime_ipc_read_request_v1'
        'signed-runtime-ipc-history-page-response-v1|CANONICAL-RUNTIME-IPC-READ-VECTORS.json|/expected/history_page/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_read_response_v1'
        'runtime-ipc-add-contact-request-v1|CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json|/expected/add_contact/request/cbor_hex|decode_and_verify_runtime_ipc_contact_request_v1'
        'signed-runtime-ipc-add-contact-response-v1|CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json|/expected/add_contact/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_contact_response_v1'
        'runtime-ipc-device-directory-status-request-v1|CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json|/expected/directory_status/request/cbor_hex|decode_and_verify_runtime_ipc_contact_request_v1'
        'signed-runtime-ipc-device-directory-status-response-v1|CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json|/expected/directory_status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_contact_response_v1'
        'runtime-ipc-device-directory-apply-request-v1|CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_directory_request_v1'
        'signed-runtime-ipc-device-directory-apply-response-v1|CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_directory_response_v1'
        'runtime-ipc-queue-message-request-v1|CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_queue_request_v1'
        'signed-runtime-ipc-message-queued-response-v1|CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_queue_response_v1'
        'runtime-ipc-outbox-status-page-request-v1|CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_outbox_request_v1'
        'signed-runtime-ipc-outbox-status-page-response-v1|CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_outbox_response_v1'
        'runtime-ipc-provider-import-request-v1|CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_provider_request_v1'
        'signed-runtime-ipc-provider-import-response-v1|CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_provider_response_v1'
        'runtime-ipc-provider-selection-request-v1|CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json|/expected/selection_request/cbor_hex|decode_and_verify_runtime_ipc_provider_request_v1'
        'signed-runtime-ipc-provider-selection-response-v1|CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json|/expected/selection_response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_provider_response_v1'
        'runtime-ipc-mailbox-capability-status-page-request-v1|CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_mailbox_status_request_v1'
        'signed-runtime-ipc-mailbox-capability-status-page-response-v1|CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1'
        'runtime-ipc-mailbox-capability-create-request-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/create/request/cbor_hex|decode_and_verify_runtime_ipc_mailbox_mutation_request_v1'
        'signed-runtime-ipc-mailbox-capability-create-response-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/create/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1'
        'runtime-ipc-mailbox-capability-rotate-request-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/rotate/request/cbor_hex|decode_and_verify_runtime_ipc_mailbox_mutation_request_v1'
        'signed-runtime-ipc-mailbox-capability-rotate-response-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/rotate/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1'
        'runtime-ipc-mailbox-capability-revoke-request-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/revoke/request/cbor_hex|decode_and_verify_runtime_ipc_mailbox_mutation_request_v1'
        'signed-runtime-ipc-mailbox-capability-revoke-response-v1|CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json|/expected/revoke/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1'
        'runtime-ipc-publication-rotate-request-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json|/expected/request/cbor_hex|decode_and_verify_runtime_ipc_publication_request_v1'
        'signed-runtime-ipc-publication-rotate-response-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json|/expected/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_publication_response_v1'
        'runtime-ipc-publication-network-publish-request-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json|/expected/publish/request/cbor_hex|decode_and_verify_runtime_ipc_publication_network_request_v1'
        'signed-runtime-ipc-publication-network-publish-response-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json|/expected/publish/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_publication_network_response_v1'
        'runtime-ipc-publication-network-refresh-request-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json|/expected/refresh/request/cbor_hex|decode_and_verify_runtime_ipc_publication_network_request_v1'
        'signed-runtime-ipc-publication-network-refresh-response-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json|/expected/refresh/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_publication_network_response_v1'
        'runtime-ipc-publication-conflict-create-request-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json|/expected/create/request/cbor_hex|decode_and_verify_runtime_ipc_publication_conflict_request_v1'
        'signed-runtime-ipc-publication-conflict-create-response-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json|/expected/create/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1'
        'runtime-ipc-publication-conflict-apply-request-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json|/expected/apply/request/cbor_hex|decode_and_verify_runtime_ipc_publication_conflict_request_v1'
        'signed-runtime-ipc-publication-conflict-apply-response-v1|CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json|/expected/apply/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1'
        'runtime-ipc-own-device-announcement-configure-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json|/expected/configure/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_announcement_request_v1'
        'signed-runtime-ipc-own-device-announcement-configure-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json|/expected/configure/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_announcement_response_v1'
        'runtime-ipc-own-device-announcement-status-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json|/expected/status/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_announcement_request_v1'
        'signed-runtime-ipc-own-device-announcement-status-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json|/expected/status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_announcement_response_v1'
        'runtime-ipc-own-device-discovery-configure-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json|/expected/configure/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_discovery_request_v1'
        'signed-runtime-ipc-own-device-discovery-configure-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json|/expected/configure/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1'
        'runtime-ipc-own-device-discovery-status-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json|/expected/status/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_discovery_request_v1'
        'signed-runtime-ipc-own-device-discovery-status-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json|/expected/status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1'
        'runtime-ipc-own-device-roster-configure-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json|/expected/configure/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_roster_request_v1'
        'signed-runtime-ipc-own-device-roster-configure-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json|/expected/configure/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1'
        'runtime-ipc-own-device-roster-status-request-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json|/expected/status/request/cbor_hex|decode_and_verify_runtime_ipc_own_device_roster_request_v1'
        'signed-runtime-ipc-own-device-roster-status-response-v1|CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json|/expected/status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1'
        'runtime-ipc-recovery-plan-approve-request-v1|CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json|/expected/approve/request/cbor_hex|decode_and_verify_runtime_ipc_recovery_plan_request_v1'
        'signed-runtime-ipc-recovery-plan-approve-response-v1|CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json|/expected/approve/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1'
        'runtime-ipc-recovery-plan-cancel-request-v1|CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json|/expected/cancel/request/cbor_hex|decode_and_verify_runtime_ipc_recovery_plan_request_v1'
        'signed-runtime-ipc-recovery-plan-cancel-response-v1|CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json|/expected/cancel/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1'
        'runtime-ipc-recovery-execution-start-request-v1|CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json|/expected/start/request/cbor_hex|decode_and_verify_runtime_ipc_recovery_execution_request_v1'
        'signed-runtime-ipc-recovery-execution-start-response-v1|CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json|/expected/start/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1'
        'runtime-ipc-recovery-execution-status-request-v1|CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json|/expected/status/request/cbor_hex|decode_and_verify_runtime_ipc_recovery_execution_request_v1'
        'signed-runtime-ipc-recovery-execution-status-response-v1|CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json|/expected/status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1'
        'runtime-ipc-ticket-automation-configure-request-v1|CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json|/expected/configure/request/cbor_hex|decode_and_verify_runtime_ipc_ticket_automation_request_v1'
        'signed-runtime-ipc-ticket-automation-configure-response-v1|CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json|/expected/configure/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1'
        'runtime-ipc-ticket-automation-status-request-v1|CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json|/expected/status/request/cbor_hex|decode_and_verify_runtime_ipc_ticket_automation_request_v1'
        'signed-runtime-ipc-ticket-automation-status-response-v1|CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json|/expected/status/response/signed_cbor_hex|decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1'
    )
    $actualFixtures = @($Manifest.fixtures | ForEach-Object { "$($_.id)|$($_.vector_file)|$($_.json_pointer)|$($_.decoder)" })
    if (($actualFixtures -join "`n") -cne ($expectedFixtures -join "`n")) { throw 'decoder fixture inventory changed' }
    $runtimePayloadFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'runtime-mailbox-payload-v1' })
    if ($runtimePayloadFixture.Count -ne 1 -or
        [string]$runtimePayloadFixture[0].context_verifier -cne 'verify_runtime_mailbox_payload_for_dispatch_v1') {
        throw 'runtime mailbox payload mutation context changed'
    }
    $publicationClaimFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'publication-conflict-response-claim-v1' })
    if ($publicationClaimFixture.Count -ne 1 -or
        [string]$publicationClaimFixture[0].context_verifier -cne 'verify_publication_conflict_claim_for_response_v1') {
        throw 'publication-conflict claim mutation context changed'
    }
    $endpointEnvelopeFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'encrypted-endpoint-announcement-bundle-v1' })
    if ($endpointEnvelopeFixture.Count -ne 1 -or
        [string]$endpointEnvelopeFixture[0].context_verifier -cne 'open_endpoint_announcement_bundle_v1') {
        throw 'endpoint-announcement envelope mutation context changed'
    }
    $endpointAcknowledgementFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-endpoint-announcement-acknowledgement-v1' })
    if ($endpointAcknowledgementFixture.Count -ne 1 -or
        [string]$endpointAcknowledgementFixture[0].context_verifier -cne 'verify_endpoint_announcement_acknowledgement_for_v1') {
        throw 'endpoint-announcement acknowledgement mutation context changed'
    }
    $historyEntryFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-history-rewrap-entry-v1' })
    if ($historyEntryFixture.Count -ne 1 -or
        [string]$historyEntryFixture[0].context_verifier -cne 'open_history_rewrap_entry_v1') {
        throw 'history-rewrap entry mutation context changed'
    }
    $historyTransferFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-history-rewrap-transfer-v1' })
    if ($historyTransferFixture.Count -ne 1 -or
        [string]$historyTransferFixture[0].context_verifier -cne 'verify_history_rewrap_transfer_for_request_v1') {
        throw 'history-rewrap transfer mutation context changed'
    }
    $accountRecoveryApprovalFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-account-recovery-approval-v1' })
    if ($accountRecoveryApprovalFixture.Count -ne 1 -or
        [string]$accountRecoveryApprovalFixture[0].context_verifier -cne 'verify_account_recovery_approval_for_request_v1') {
        throw 'Account recovery approval mutation context changed'
    }
    $policyRequestFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'account-recovery-policy-transition-request-v1' })
    if ($policyRequestFixture.Count -ne 1 -or
        [string]$policyRequestFixture[0].context_verifier -cne 'account_recovery_policy_transition_request_id_v1') {
        throw 'Account recovery policy request mutation context changed'
    }
    $policyApprovalFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-account-recovery-policy-transition-approval-v1' })
    if ($policyApprovalFixture.Count -ne 1 -or
        [string]$policyApprovalFixture[0].context_verifier -cne 'verify_account_recovery_policy_transition_approval_for_request_v1') {
        throw 'Account recovery policy approval mutation context changed'
    }
    $runtimeRequestFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'runtime-ipc-core-request-v1' })
    if ($runtimeRequestFixture.Count -ne 1 -or
        [string]$runtimeRequestFixture[0].context_verifier -cne 'verify_runtime_ipc_core_request_for_descriptor_v1+verify_runtime_ipc_core_response_for_request_v1') {
        throw 'runtime IPC request mutation context changed'
    }
    $runtimeResponseFixture = @($Manifest.fixtures | Where-Object { $_.id -ceq 'signed-runtime-ipc-core-response-v1' })
    if ($runtimeResponseFixture.Count -ne 1 -or
        [string]$runtimeResponseFixture[0].context_verifier -cne 'verify_runtime_ipc_core_response_for_request_v1') {
        throw 'runtime IPC response mutation context changed'
    }
    foreach ($id in @('runtime-ipc-conversation-list-page-request-v1', 'runtime-ipc-history-page-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne 'verify_runtime_ipc_read_request_for_descriptor_v1+verify_runtime_ipc_read_response_for_request_v1') {
            throw "runtime IPC read request mutation context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-conversation-list-page-response-v1', 'signed-runtime-ipc-history-page-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne 'verify_runtime_ipc_read_response_for_request_v1') {
            throw "runtime IPC read response mutation context changed: $id"
        }
    }
    $directoryRequestFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'runtime-ipc-device-directory-apply-request-v1' })
    if ($directoryRequestFixture.Count -ne 1 -or
        [string]$directoryRequestFixture[0].context_verifier -cne
            'verify_runtime_ipc_directory_request_for_descriptor_v1+verify_runtime_ipc_directory_response_for_request_v1') {
        throw 'runtime IPC directory request mutation context changed'
    }
    $directoryResponseFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'signed-runtime-ipc-device-directory-apply-response-v1' })
    if ($directoryResponseFixture.Count -ne 1 -or
        [string]$directoryResponseFixture[0].context_verifier -cne
            'verify_runtime_ipc_directory_response_for_request_v1') {
        throw 'runtime IPC directory response mutation context changed'
    }
    $queueRequestFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'runtime-ipc-queue-message-request-v1' })
    if ($queueRequestFixture.Count -ne 1 -or
        [string]$queueRequestFixture[0].context_verifier -cne
            'verify_runtime_ipc_queue_request_for_descriptor_v1+verify_runtime_ipc_queue_response_for_request_v1') {
        throw 'runtime IPC queue request mutation context changed'
    }
    $queueResponseFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'signed-runtime-ipc-message-queued-response-v1' })
    if ($queueResponseFixture.Count -ne 1 -or
        [string]$queueResponseFixture[0].context_verifier -cne
            'verify_runtime_ipc_queue_response_for_request_v1') {
        throw 'runtime IPC queue response mutation context changed'
    }
    $outboxRequestFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'runtime-ipc-outbox-status-page-request-v1' })
    if ($outboxRequestFixture.Count -ne 1 -or
        [string]$outboxRequestFixture[0].context_verifier -cne
            'verify_runtime_ipc_outbox_request_for_descriptor_v1+verify_runtime_ipc_outbox_response_for_request_v1') {
        throw 'runtime IPC outbox request mutation context changed'
    }
    $outboxResponseFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'signed-runtime-ipc-outbox-status-page-response-v1' })
    if ($outboxResponseFixture.Count -ne 1 -or
        [string]$outboxResponseFixture[0].context_verifier -cne
            'verify_runtime_ipc_outbox_response_for_request_v1') {
        throw 'runtime IPC outbox response mutation context changed'
    }
    foreach ($id in @('runtime-ipc-provider-import-request-v1',
        'runtime-ipc-provider-selection-request-v1')) {
        $providerRequestFixture = @($Manifest.fixtures |
            Where-Object { $_.id -ceq $id })
        if ($providerRequestFixture.Count -ne 1 -or
            [string]$providerRequestFixture[0].context_verifier -cne
                'verify_runtime_ipc_provider_request_for_descriptor_v1+verify_runtime_ipc_provider_response_for_request_v1') {
            throw "runtime IPC provider request mutation context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-provider-import-response-v1',
        'signed-runtime-ipc-provider-selection-response-v1')) {
        $providerResponseFixture = @($Manifest.fixtures |
            Where-Object { $_.id -ceq $id })
        if ($providerResponseFixture.Count -ne 1 -or
            [string]$providerResponseFixture[0].context_verifier -cne
                'verify_runtime_ipc_provider_response_for_request_v1') {
            throw "runtime IPC provider response mutation context changed: $id"
        }
    }
    $mailboxStatusRequestFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'runtime-ipc-mailbox-capability-status-page-request-v1' })
    if ($mailboxStatusRequestFixture.Count -ne 1 -or
        [string]$mailboxStatusRequestFixture[0].context_verifier -cne
            'verify_runtime_ipc_mailbox_status_request_for_descriptor_v1+verify_runtime_ipc_mailbox_status_response_for_request_v1') {
        throw 'runtime IPC mailbox status request mutation context changed'
    }
    $mailboxStatusResponseFixture = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'signed-runtime-ipc-mailbox-capability-status-page-response-v1' })
    if ($mailboxStatusResponseFixture.Count -ne 1 -or
        [string]$mailboxStatusResponseFixture[0].context_verifier -cne
            'verify_runtime_ipc_mailbox_status_response_for_request_v1') {
        throw 'runtime IPC mailbox status response mutation context changed'
    }
    foreach ($id in @('runtime-ipc-mailbox-capability-create-request-v1',
        'runtime-ipc-mailbox-capability-rotate-request-v1',
        'runtime-ipc-mailbox-capability-revoke-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1+verify_runtime_ipc_mailbox_mutation_response_for_request_v1') {
            throw "runtime IPC mailbox mutation request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-mailbox-capability-create-response-v1',
        'signed-runtime-ipc-mailbox-capability-rotate-response-v1',
        'signed-runtime-ipc-mailbox-capability-revoke-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_mailbox_mutation_response_for_request_v1') {
            throw "runtime IPC mailbox mutation response context changed: $id"
        }
    }
    $publicationRequest = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'runtime-ipc-publication-rotate-request-v1' })
    if ($publicationRequest.Count -ne 1 -or
        [string]$publicationRequest[0].context_verifier -cne
            'verify_runtime_ipc_publication_request_for_descriptor_v1+verify_runtime_ipc_publication_response_for_request_v1') {
        throw 'runtime IPC publication request context changed'
    }
    $publicationResponse = @($Manifest.fixtures |
        Where-Object { $_.id -ceq 'signed-runtime-ipc-publication-rotate-response-v1' })
    if ($publicationResponse.Count -ne 1 -or
        [string]$publicationResponse[0].context_verifier -cne
            'verify_runtime_ipc_publication_response_for_request_v1') {
        throw 'runtime IPC publication response context changed'
    }
    foreach ($id in @('runtime-ipc-publication-network-publish-request-v1',
        'runtime-ipc-publication-network-refresh-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_publication_network_request_for_descriptor_v1+verify_runtime_ipc_publication_network_response_for_request_v1') {
            throw "runtime IPC publication-network request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-publication-network-publish-response-v1',
        'signed-runtime-ipc-publication-network-refresh-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_publication_network_response_for_request_v1') {
            throw "runtime IPC publication-network response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-publication-conflict-create-request-v1',
        'runtime-ipc-publication-conflict-apply-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_publication_conflict_request_for_descriptor_v1+verify_runtime_ipc_publication_conflict_response_for_request_v1') {
            throw "runtime IPC publication-conflict request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-publication-conflict-create-response-v1',
        'signed-runtime-ipc-publication-conflict-apply-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_publication_conflict_response_for_request_v1') {
            throw "runtime IPC publication-conflict response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-own-device-announcement-configure-request-v1',
        'runtime-ipc-own-device-announcement-status-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_announcement_request_for_descriptor_v1+verify_runtime_ipc_own_device_announcement_response_for_request_v1') {
            throw "runtime IPC own-device-announcement request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-own-device-announcement-configure-response-v1',
        'signed-runtime-ipc-own-device-announcement-status-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_announcement_response_for_request_v1') {
            throw "runtime IPC own-device-announcement response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-own-device-discovery-configure-request-v1',
        'runtime-ipc-own-device-discovery-status-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1+verify_runtime_ipc_own_device_discovery_response_for_request_v1') {
            throw "runtime IPC own-device-discovery request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-own-device-discovery-configure-response-v1',
        'signed-runtime-ipc-own-device-discovery-status-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_discovery_response_for_request_v1') {
            throw "runtime IPC own-device-discovery response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-own-device-roster-configure-request-v1',
        'runtime-ipc-own-device-roster-status-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_roster_request_for_descriptor_v1+verify_runtime_ipc_own_device_roster_response_for_request_v1') {
            throw "runtime IPC own-device-roster request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-own-device-roster-configure-response-v1',
        'signed-runtime-ipc-own-device-roster-status-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_own_device_roster_response_for_request_v1') {
            throw "runtime IPC own-device-roster response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-recovery-plan-approve-request-v1',
        'runtime-ipc-recovery-plan-cancel-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_recovery_plan_request_for_descriptor_v1+verify_runtime_ipc_recovery_plan_response_for_request_v1') {
            throw "runtime IPC recovery-plan request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-recovery-plan-approve-response-v1',
        'signed-runtime-ipc-recovery-plan-cancel-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_recovery_plan_response_for_request_v1') {
            throw "runtime IPC recovery-plan response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-recovery-execution-start-request-v1',
        'runtime-ipc-recovery-execution-status-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_recovery_execution_request_for_descriptor_v1+verify_runtime_ipc_recovery_execution_response_for_request_v1') {
            throw "runtime IPC recovery-execution request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-recovery-execution-start-response-v1',
        'signed-runtime-ipc-recovery-execution-status-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_recovery_execution_response_for_request_v1') {
            throw "runtime IPC recovery-execution response context changed: $id"
        }
    }
    foreach ($id in @('runtime-ipc-ticket-automation-configure-request-v1',
        'runtime-ipc-ticket-automation-status-request-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_ticket_automation_request_for_descriptor_v1+verify_runtime_ipc_ticket_automation_response_for_request_v1') {
            throw "runtime IPC ticket-automation request context changed: $id"
        }
    }
    foreach ($id in @('signed-runtime-ipc-ticket-automation-configure-response-v1',
        'signed-runtime-ipc-ticket-automation-status-response-v1')) {
        $fixture = @($Manifest.fixtures | Where-Object { $_.id -ceq $id })
        if ($fixture.Count -ne 1 -or
            [string]$fixture[0].context_verifier -cne
                'verify_runtime_ipc_ticket_automation_response_for_request_v1') {
            throw "runtime IPC ticket-automation response context changed: $id"
        }
    }
    $expectedClasses = @(
        'bounded-strict-prefix-truncation-grid', 'bounded-low-bit-flip-grid', 'trailing-byte',
        'non-minimal-top-level-version', 'indefinite-top-level-array',
        'reserved-top-level-additional-information', 'oversized-declared-byte-string'
    )
    if ((@($Manifest.mutation_classes) -join "`n") -cne ($expectedClasses -join "`n")) {
        throw 'decoder mutation classes changed'
    }
    if ([int]$Manifest.resource_bounds.maximum_fixture_bytes -ne 524288 -or
        [int]$Manifest.resource_bounds.maximum_positions_per_grid_per_fixture -ne 128 -or
        [int]$Manifest.resource_bounds.maximum_total_mutations -ne 35000 -or
        [bool]$Manifest.resource_bounds.network_allowed -or
        [bool]$Manifest.resource_bounds.randomness_allowed) { throw 'decoder resource bounds changed' }
}

foreach ($path in @($manifestPath, $officialPath, $referencePath, $verifierPath, $testPath, $ratchetReferencePath, $recoveryReferencePath, $eventReferencePath, $prekeySelectionReferencePath, $syncReferencePath, $syncSessionReferencePath, $recoveryExchangeReferencePath, $mailboxReferencePath, $mailboxTransportReferencePath, $mailboxGossipReferencePath, $mailboxProvisioningReferencePath, $runtimeMailboxReferencePath, $runtimeIpcReferencePath, $runtimeIpcReadReferencePath, $runtimeIpcContactReferencePath, $runtimeIpcDirectoryReferencePath, $runtimeIpcQueueReferencePath, $runtimeIpcOutboxReferencePath, $runtimeIpcProviderReferencePath, $runtimeIpcMailboxStatusReferencePath, $runtimeIpcMailboxMutationReferencePath, $runtimeIpcPublicationReferencePath, $runtimeIpcPublicationNetworkReferencePath, $runtimeIpcPublicationConflictReferencePath, $runtimeIpcOwnDeviceAnnouncementReferencePath, $runtimeIpcOwnDeviceDiscoveryReferencePath, $runtimeIpcOwnDeviceRosterReferencePath, $runtimeIpcRecoveryPlanReferencePath, $runtimeIpcRecoveryExecutionReferencePath, $runtimeIpcTicketAutomationReferencePath, $ticketPublicationReferencePath, $publicationConflictReferencePath, $endpointAnnouncementReferencePath, $endpointAnnouncementTransportReferencePath, $historyRewrapReferencePath, $accountRecoveryReferencePath, $accountRecoveryPolicyReferencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical conformance file is missing: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$official = Get-Content -LiteralPath $officialPath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$verifier = Get-Content -LiteralPath $verifierPath -Raw
$test = Get-Content -LiteralPath $testPath -Raw
$ratchetReference = Get-Content -LiteralPath $ratchetReferencePath -Raw
$recoveryReference = Get-Content -LiteralPath $recoveryReferencePath -Raw
$eventReference = Get-Content -LiteralPath $eventReferencePath -Raw
$prekeySelectionReference = Get-Content -LiteralPath $prekeySelectionReferencePath -Raw
$syncSessionReference = Get-Content -LiteralPath $syncSessionReferencePath -Raw
$recoveryExchangeReference = Get-Content -LiteralPath $recoveryExchangeReferencePath -Raw
$mailboxReference = Get-Content -LiteralPath $mailboxReferencePath -Raw
$mailboxTransportReference = Get-Content -LiteralPath $mailboxTransportReferencePath -Raw
$mailboxGossipReference = Get-Content -LiteralPath $mailboxGossipReferencePath -Raw
$mailboxProvisioningReference = Get-Content -LiteralPath $mailboxProvisioningReferencePath -Raw
$runtimeMailboxReference = Get-Content -LiteralPath $runtimeMailboxReferencePath -Raw
$runtimeIpcReference = Get-Content -LiteralPath $runtimeIpcReferencePath -Raw
$runtimeIpcReadReference = Get-Content -LiteralPath $runtimeIpcReadReferencePath -Raw
$runtimeIpcContactReference = Get-Content -LiteralPath $runtimeIpcContactReferencePath -Raw
$runtimeIpcDirectoryReference = Get-Content -LiteralPath $runtimeIpcDirectoryReferencePath -Raw
$runtimeIpcQueueReference = Get-Content -LiteralPath $runtimeIpcQueueReferencePath -Raw
$runtimeIpcOutboxReference = Get-Content -LiteralPath $runtimeIpcOutboxReferencePath -Raw
$runtimeIpcProviderReference = Get-Content -LiteralPath $runtimeIpcProviderReferencePath -Raw
$runtimeIpcMailboxStatusReference = Get-Content -LiteralPath $runtimeIpcMailboxStatusReferencePath -Raw
$runtimeIpcMailboxMutationReference = Get-Content -LiteralPath $runtimeIpcMailboxMutationReferencePath -Raw
$runtimeIpcPublicationReference = Get-Content -LiteralPath $runtimeIpcPublicationReferencePath -Raw
$runtimeIpcPublicationNetworkReference = Get-Content -LiteralPath $runtimeIpcPublicationNetworkReferencePath -Raw
$runtimeIpcPublicationConflictReference = Get-Content -LiteralPath $runtimeIpcPublicationConflictReferencePath -Raw
$runtimeIpcOwnDeviceAnnouncementReference = Get-Content -LiteralPath $runtimeIpcOwnDeviceAnnouncementReferencePath -Raw
$runtimeIpcOwnDeviceDiscoveryReference = Get-Content -LiteralPath $runtimeIpcOwnDeviceDiscoveryReferencePath -Raw
$runtimeIpcOwnDeviceRosterReference = Get-Content -LiteralPath $runtimeIpcOwnDeviceRosterReferencePath -Raw
$runtimeIpcRecoveryPlanReference = Get-Content -LiteralPath $runtimeIpcRecoveryPlanReferencePath -Raw
$runtimeIpcRecoveryExecutionReference = Get-Content -LiteralPath $runtimeIpcRecoveryExecutionReferencePath -Raw
$runtimeIpcTicketAutomationReference = Get-Content -LiteralPath $runtimeIpcTicketAutomationReferencePath -Raw
$ticketPublicationReference = Get-Content -LiteralPath $ticketPublicationReferencePath -Raw
$publicationConflictReference = Get-Content -LiteralPath $publicationConflictReferencePath -Raw
$endpointAnnouncementReference = Get-Content -LiteralPath $endpointAnnouncementReferencePath -Raw
$endpointAnnouncementTransportReference = Get-Content -LiteralPath $endpointAnnouncementTransportReferencePath -Raw
$historyRewrapReference = Get-Content -LiteralPath $historyRewrapReferencePath -Raw
$accountRecoveryReference = Get-Content -LiteralPath $accountRecoveryReferencePath -Raw
$accountRecoveryPolicyReference = Get-Content -LiteralPath $accountRecoveryPolicyReferencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Manifest $manifest

if ([UInt64]$official.schema_version -ne 1 -or
    [string]$official.source -cne 'https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json' -or
    [string]$official.input_pattern -cne 'byte_at_index_modulo_251' -or
    [int]$official.output_bytes_checked -ne 32) { throw 'official BLAKE3 vector metadata changed' }
$expectedLengths = @(0, 1, 1023, 1024, 1025, 2048)
$actualLengths = @($official.cases | ForEach-Object { [int]$_.input_len })
if (($actualLengths -join ',') -cne ($expectedLengths -join ',')) { throw 'official BLAKE3 case inventory changed' }
foreach ($case in $official.cases) {
    if ([string]$case.hash_hex -cnotmatch '^[0-9a-f]{64}$') { throw "invalid BLAKE3 hash for length $($case.input_len)" }
}
foreach ($required in @('const CHUNK_LEN = 1024', 'function compress(', 'function chunkOutput(',
    'function parentOutput(', 'export function blake3Hash(')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "BLAKE3 reference is missing '$required'" }
}
foreach ($forbidden in @('node:crypto', 'https:', 'fetch(', 'Invoke-WebRequest', 'npm', 'require(')) {
    if ($reference.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "BLAKE3 reference contains forbidden dependency/network surface '$forbidden'"
    }
}
foreach ($required in @('official_vectors_checked=', 'kilogram_digest_vectors_checked=5',
    'CANONICAL-WIRE-VECTORS.json', 'CANONICAL-RATCHET-VECTORS.json', 'CANONICAL-RECOVERY-VECTORS.json')) {
    if ($verifier.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "digest verifier is missing '$required'" }
}
foreach ($fixture in $manifest.fixtures) {
    if ($test.IndexOf([string]$fixture.decoder, [StringComparison]::Ordinal) -lt 0) {
        throw "mutation test is missing decoder '$($fixture.decoder)'"
    }
}
foreach ($required in @('MAXIMUM_GRID_POSITIONS: usize = 128', 'MINIMUM_MUTATIONS: usize = 500',
    'EXPECTED_MUTATIONS: usize = 31_851', 'MAXIMUM_MUTATIONS: usize = 35_000',
    'bounded_positions(', 'verify_runtime_mailbox_payload_for_dispatch_v1',
    'verify_publication_conflict_claim_for_response_v1',
    'open_endpoint_announcement_bundle_v1',
    'verify_endpoint_announcement_acknowledgement_for_v1',
    'open_history_rewrap_entry_v1',
    'verify_history_rewrap_transfer_for_request_v1',
    'verify_account_recovery_approval_for_request_v1',
    'account_recovery_policy_transition_request_id_v1',
    'verify_account_recovery_policy_transition_approval_for_request_v1',
    'decode_and_verify_account_recovery_policy_transition_certificate_v1',
    'verify_runtime_ipc_core_request_for_descriptor_v1',
    'verify_runtime_ipc_core_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_core_response_v1',
    'verify_runtime_ipc_read_request_for_descriptor_v1',
    'verify_runtime_ipc_read_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_read_response_v1',
    'verify_runtime_ipc_contact_request_for_descriptor_v1',
    'verify_runtime_ipc_contact_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_contact_response_v1',
    'verify_runtime_ipc_queue_request_for_descriptor_v1',
    'verify_runtime_ipc_queue_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_queue_response_v1',
    'verify_runtime_ipc_outbox_request_for_descriptor_v1',
    'verify_runtime_ipc_outbox_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_outbox_response_v1',
    'verify_runtime_ipc_provider_request_for_descriptor_v1',
    'verify_runtime_ipc_provider_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_provider_response_v1',
    'verify_runtime_ipc_mailbox_status_request_for_descriptor_v1',
    'verify_runtime_ipc_mailbox_status_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1',
    'verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1',
    'verify_runtime_ipc_mailbox_mutation_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1',
    'verify_runtime_ipc_publication_request_for_descriptor_v1',
    'verify_runtime_ipc_publication_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_publication_response_v1',
    'verify_runtime_ipc_publication_network_request_for_descriptor_v1',
    'verify_runtime_ipc_publication_network_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_publication_network_response_v1',
    'verify_runtime_ipc_publication_conflict_request_for_descriptor_v1',
    'verify_runtime_ipc_publication_conflict_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1',
    'verify_runtime_ipc_own_device_announcement_request_for_descriptor_v1',
    'verify_runtime_ipc_own_device_announcement_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_own_device_announcement_response_v1',
    'verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1',
    'verify_runtime_ipc_own_device_discovery_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1',
    'verify_runtime_ipc_own_device_roster_request_for_descriptor_v1',
    'verify_runtime_ipc_own_device_roster_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1',
    'verify_runtime_ipc_recovery_plan_request_for_descriptor_v1',
    'verify_runtime_ipc_recovery_plan_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1',
    'verify_runtime_ipc_recovery_execution_request_for_descriptor_v1',
    'verify_runtime_ipc_recovery_execution_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1',
    'verify_runtime_ipc_ticket_automation_request_for_descriptor_v1',
    'verify_runtime_ipc_ticket_automation_response_for_request_v1',
    'decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1',
    'canonical_decoder_mutations=')) {
    if ($test.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "mutation test is missing '$required'" }
}
foreach ($referenceSource in @($ratchetReference, $recoveryReference, $eventReference, $prekeySelectionReference, $syncSessionReference, $recoveryExchangeReference, $mailboxReference, $mailboxTransportReference, $mailboxGossipReference, $mailboxProvisioningReference, $runtimeMailboxReference, $runtimeIpcReference, $runtimeIpcReadReference, $runtimeIpcContactReference, $runtimeIpcDirectoryReference, $runtimeIpcQueueReference, $runtimeIpcOutboxReference, $runtimeIpcProviderReference, $runtimeIpcMailboxStatusReference, $runtimeIpcMailboxMutationReference, $runtimeIpcPublicationReference, $runtimeIpcPublicationNetworkReference, $runtimeIpcPublicationConflictReference, $runtimeIpcOwnDeviceAnnouncementReference, $runtimeIpcOwnDeviceDiscoveryReference, $runtimeIpcOwnDeviceRosterReference, $runtimeIpcRecoveryPlanReference, $runtimeIpcRecoveryExecutionReference, $runtimeIpcTicketAutomationReference, $ticketPublicationReference, $publicationConflictReference, $endpointAnnouncementReference, $endpointAnnouncementTransportReference, $historyRewrapReference, $accountRecoveryReference, $accountRecoveryPolicyReference)) {
    if ($referenceSource.IndexOf('independent_blake3=verified', [StringComparison]::Ordinal) -lt 0 -or
        $referenceSource.IndexOf('blake3Hash(', [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical reference does not independently verify BLAKE3'
    }
}
foreach ($required in @('not a substitute for', 'deterministic, offline and bounded', 'does not activate canonical CBOR')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw "RFC is missing '$required'" }
}

$output = @(& node $verifierPath $workspace 2>&1)
if ($LASTEXITCODE -ne 0 -or 'blake3_reference_conformance=verified' -notin @($output | ForEach-Object { [string]$_ })) {
    throw "independent BLAKE3 conformance failed`n$($output -join [Environment]::NewLine)"
}
if ($SelfTest) {
    $changed = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.maximum_mutations = 8001
    Assert-Rejected { Assert-Manifest $changed } 'changed_mutation_bound'
    $removed = $manifest | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $removed.fixtures = @($removed.fixtures | Select-Object -Skip 1)
    Assert-Rejected { Assert-Manifest $removed } 'removed_decoder_fixture'
    Write-Output 'canonical_conformance_self_test=passed'
}
Write-Output 'canonical_conformance_boundary=verified'
Write-Output 'independent_blake3=verified'
Write-Output 'official_blake3_vectors=6/6'
Write-Output 'bounded_decoder_mutations=31851'
Write-Output 'coverage_guided_fuzzing=pending-before-activation'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
