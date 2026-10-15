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
use kilogram_protocol::canonical_prekey_selection_v1::decode_and_verify_prekey_selection_transcript_v1;
use kilogram_protocol::canonical_ratchet_v1::{
    decode_and_verify_account_prekey_directory_v1, decode_and_verify_signed_prekey_pool_v1,
};
use kilogram_protocol::canonical_recovery_exchange_v1::{
    decode_and_verify_signed_history_recovery_link_v1,
    decode_and_verify_signed_history_recovery_plan_v1,
    decode_and_verify_signed_history_recovery_response_v1,
};
use kilogram_protocol::canonical_recovery_v1::decode_and_verify_signed_history_recovery_checkpoint_v1;
use kilogram_protocol::canonical_sync_session_v1::decode_and_verify_signed_sync_session_checkpoint_v1;
use kilogram_protocol::canonical_sync_v1::{
    decode_and_verify_signed_sync_diff_v1, decode_and_verify_sync_client_frame_v1,
    decode_and_verify_sync_server_frame_v1,
};
use serde_json::Value;

const MINIMUM_MUTATIONS: usize = 500;
const EXPECTED_MUTATIONS: usize = 4_698;
const MAXIMUM_MUTATIONS: usize = 5_000;
const MAXIMUM_GRID_POSITIONS: usize = 128;

#[derive(Clone, Copy)]
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
}

fn accepts(decoder: Decoder, bytes: &[u8]) -> bool {
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
    ];
    let mut mutations = 0usize;
    for (name, decoder, valid) in fixtures {
        assert!(
            accepts(decoder, &valid),
            "golden {name} fixture was rejected"
        );
        assert!(
            valid.len() <= 512 * 1024,
            "golden {name} fixture is unbounded"
        );

        for length in bounded_positions(valid.len()) {
            assert!(
                !accepts(decoder, &valid[..length]),
                "{name} accepted truncation at {length}"
            );
            mutations += 1;
        }
        for index in bounded_positions(valid.len()) {
            let mut changed = valid.clone();
            changed[index] ^= 1;
            assert!(
                !accepts(decoder, &changed),
                "{name} accepted bit flip at {index}"
            );
            mutations += 1;
        }
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(
            !accepts(decoder, &trailing),
            "{name} accepted trailing data"
        );
        mutations += 1;

        let mut non_minimal = Vec::with_capacity(valid.len() + 1);
        non_minimal.push(valid[0]);
        non_minimal.extend_from_slice(&[0x18, 0x01]);
        non_minimal.extend_from_slice(&valid[2..]);
        assert!(
            !accepts(decoder, &non_minimal),
            "{name} accepted non-minimal version"
        );
        mutations += 1;

        let mut indefinite = valid.clone();
        indefinite[0] = 0x9f;
        indefinite.push(0xff);
        assert!(
            !accepts(decoder, &indefinite),
            "{name} accepted indefinite array"
        );
        mutations += 1;

        let mut reserved = valid.clone();
        reserved[0] = 0x9c;
        assert!(
            !accepts(decoder, &reserved),
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
    ] {
        assert!(!accepts(decoder, &oversized));
        mutations += 1;
    }
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
